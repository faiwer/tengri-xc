use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::decode::{decode_jpeg_bytes, decode_png_bytes};
use crate::geo::{Bounds, XyzTile, xyz_tiles_for_bounds};
use crate::matrix::Raster;
use crate::tree::{PassthroughCodec, TileSource, TileSourceReader, TileTreeError, XYZBounds};
use crate::webp::decode::decode_webp_bytes;

/// Loose-tile imagery source. Reads tiles from
/// `<root>/<z>/<x>/<prefix><y>.<ext>`, where `<prefix>` is concatenated
/// verbatim — any separator (`_`, `-`, …) belongs inside the prefix string.
/// Prefixes are tried in the configured order; the first prefix with a file for
/// `(z,x,y)` wins. Extensions are tried `webp → png → jpg → jpeg`.
///
/// Single-zoom only: the `<root>` must contain exactly one numeric subdirectory
/// (the source's `tile_bounds().zoom`). Pyramid mode is a planned extension.
pub struct DirImagerySource {
    root: PathBuf,
    prefixes: Vec<String>,
    tile_bounds: XYZBounds,
}

impl DirImagerySource {
    pub fn open(
        root: impl AsRef<Path>,
        prefixes: Vec<String>,
        bounds: Option<Bounds>,
    ) -> Result<Self, TileTreeError> {
        if prefixes.is_empty() {
            return Err(TileTreeError::InvalidBounds(
                "directory imagery source requires at least one prefix (empty string is fine)",
            ));
        }

        let root = root.as_ref().to_owned();
        let zoom = discover_single_zoom(&root)?;
        let present = scan_present_tiles(&root, zoom, &prefixes)?;
        if present.is_empty() {
            return Err(TileTreeError::InvalidBounds(
                "directory imagery source: no tiles matched any prefix",
            ));
        }

        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        for (x, y) in &present {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }

        if let Some(bounds) = bounds {
            let caller_tiles = xyz_tiles_for_bounds(bounds, zoom)?;
            let caller_hull = XYZBounds::from_tiles(zoom, &caller_tiles)?;
            min_x = min_x.max(u32::from(caller_hull.min_x));
            min_y = min_y.max(u32::from(caller_hull.min_y));
            max_x = max_x.min(u32::from(caller_hull.max_x));
            max_y = max_y.min(u32::from(caller_hull.max_y));
            if min_x > max_x || min_y > max_y {
                return Err(TileTreeError::InvalidBounds(
                    "directory imagery source: caller bounds do not intersect any tile on disk",
                ));
            }
        }

        let tile_bounds = XYZBounds::new(
            zoom,
            to_u16(min_x)?,
            to_u16(min_y)?,
            to_u16(max_x)?,
            to_u16(max_y)?,
        )?;

        // Dense-rectangle check: the exporter assumes every tile inside
        // `tile_bounds` is readable at the leaf zoom. Surface a sparse cache
        // up front instead of half-failing during export.
        assert_rectangle_is_dense(tile_bounds, &present)?;

        Ok(Self {
            root,
            prefixes,
            tile_bounds,
        })
    }
}

impl TileSource for DirImagerySource {
    type Tile = Raster;

    fn tile_bounds(&self) -> XYZBounds {
        self.tile_bounds
    }

    fn open_reader(&self) -> Result<Box<dyn TileSourceReader<Tile = Raster>>, TileTreeError> {
        Ok(Box::new(DirImageryReader {
            root: self.root.clone(),
            prefixes: self.prefixes.clone(),
        }))
    }

    fn raw_codec(&self) -> Option<PassthroughCodec> {
        // The cache *may* contain `.webp` files; whether a particular tile
        // actually came from one is decided per-read in `read_raw`.
        // TODO: Consider supporting multiple codecs if needed.
        Some(PassthroughCodec::Webp)
    }
}

struct DirImageryReader {
    root: PathBuf,
    prefixes: Vec<String>,
}

impl TileSourceReader for DirImageryReader {
    type Tile = Raster;

    fn read(&mut self, tile: XyzTile) -> Result<Raster, TileTreeError> {
        let (path, ext) = self.locate(tile)?;
        let bytes = std::fs::read(&path)?;
        match ext {
            ImageExt::Webp => decode_webp_bytes(&bytes),
            ImageExt::Png => decode_png_bytes(&bytes),
            ImageExt::Jpg => decode_jpeg_bytes(&bytes),
        }
    }

    fn read_raw(&mut self, tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
        let (path, ext) = self.locate(tile)?;
        if ext == ImageExt::Webp {
            Ok(Some(std::fs::read(&path)?))
        } else {
            Ok(None)
        }
    }
}

impl DirImageryReader {
    fn locate(&self, tile: XyzTile) -> Result<(PathBuf, ImageExt), TileTreeError> {
        let dir = self.root.join(tile.z.to_string()).join(tile.x.to_string());
        for prefix in &self.prefixes {
            for (ext_str, ext) in EXT_ORDER {
                let basename = format!("{prefix}{y}.{ext_str}", y = tile.y);
                let candidate = dir.join(&basename);
                if candidate.is_file() {
                    return Ok((candidate, *ext));
                }
            }
        }
        Err(TileTreeError::MissingTile {
            z: tile.z,
            x: to_u16(tile.x)?,
            y: to_u16(tile.y)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageExt {
    Webp,
    Png,
    Jpg,
}

/// Probe order at read time. `webp` first because the WebP exporter's
/// passthrough fast-path can only fire when the source file is already WebP;
/// every other extension forces a decode + re-encode.
const EXT_ORDER: &[(&str, ImageExt)] = &[
    ("webp", ImageExt::Webp),
    ("png", ImageExt::Png),
    ("jpg", ImageExt::Jpg),
    ("jpeg", ImageExt::Jpg),
];

/// Read `<root>` and return the single numeric subdirectory's name as `u8`.
/// Pyramid mode (multiple zooms) is a planned extension; for now we refuse a
/// `<root>` that doesn't hold exactly one zoom.
fn discover_single_zoom(root: &Path) -> Result<u8, TileTreeError> {
    let mut zoom: Option<u8> = None;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        let Ok(z) = name_str.parse::<u8>() else {
            continue;
        };
        if zoom.replace(z).is_some() {
            return Err(TileTreeError::Unsupported(
                "directory imagery source: multi-zoom pyramid mode is not yet implemented",
            ));
        }
    }

    zoom.ok_or(TileTreeError::InvalidBounds(
        "directory imagery source: no numeric zoom subdirectory under <root>",
    ))
}

/// Walk `<root>/<zoom>/<x>/...` once. Collect `(x, y)` for every file whose
/// basename matches `<prefix><digits>.<ext>` for any configured prefix and
/// supported extension. The same `(x, y)` reported by multiple prefixes lands
/// once in the set — prefix priority only matters at read time.
fn scan_present_tiles(
    root: &Path,
    zoom: u8,
    prefixes: &[String],
) -> Result<HashSet<(u32, u32)>, TileTreeError> {
    let mut tiles = HashSet::new();
    let zoom_dir = root.join(zoom.to_string());
    for x_entry in std::fs::read_dir(&zoom_dir)? {
        let x_entry = x_entry?;
        if !x_entry.file_type()?.is_dir() {
            continue;
        }
        let x_name = x_entry.file_name();
        let Some(x_str) = x_name.to_str() else {
            continue;
        };
        let Ok(x) = x_str.parse::<u32>() else {
            continue;
        };

        for file_entry in std::fs::read_dir(x_entry.path())? {
            let file_entry = file_entry?;
            if !file_entry.file_type()?.is_file() {
                continue;
            }
            let file_name = file_entry.file_name();
            let Some(file_str) = file_name.to_str() else {
                continue;
            };
            if let Some(y) = match_basename(file_str, prefixes) {
                tiles.insert((x, y));
            }
        }
    }
    Ok(tiles)
}

/// Match `basename` against `<prefix><digits>.<ext>` for any configured prefix
/// and supported extension. Returns the parsed `y` on success.
fn match_basename(basename: &str, prefixes: &[String]) -> Option<u32> {
    let (stem, ext) = basename.rsplit_once('.')?;
    if !EXT_ORDER
        .iter()
        .any(|(known, _)| known.eq_ignore_ascii_case(ext))
    {
        return None;
    }
    for prefix in prefixes {
        if let Some(rest) = stem.strip_prefix(prefix.as_str()) {
            if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(y) = rest.parse::<u32>() {
                    return Some(y);
                }
            }
        }
    }
    None
}

/// Refuse a sparse rectangle up front so the export error surface is
/// "directory missing tile" rather than "exporter failed on tile (x,y) midway".
fn assert_rectangle_is_dense(
    bounds: XYZBounds,
    present: &HashSet<(u32, u32)>,
) -> Result<(), TileTreeError> {
    for y in bounds.min_y..=bounds.max_y {
        for x in bounds.min_x..=bounds.max_x {
            if !present.contains(&(u32::from(x), u32::from(y))) {
                eprintln!(
                    "directory imagery source: tile is missing inside the auto-derived bounding box \
                     (zoom={}, x={x}, y={y}); either crop with --bounds or fill the gap",
                    bounds.zoom,
                );
                return Err(TileTreeError::MissingTile {
                    z: bounds.zoom,
                    x,
                    y,
                });
            }
        }
    }
    Ok(())
}

fn to_u16(value: u32) -> Result<u16, TileTreeError> {
    u16::try_from(value).map_err(|_| TileTreeError::InvalidBounds("tile coordinate exceeds u16"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_basename_accepts_known_extensions_with_digit_suffix() {
        let prefixes = vec!["st_".to_owned()];
        assert_eq!(match_basename("st_23279.webp", &prefixes), Some(23279));
        assert_eq!(match_basename("st_0.png", &prefixes), Some(0));
        assert_eq!(match_basename("st_42.JPEG", &prefixes), Some(42));
    }

    #[test]
    fn match_basename_rejects_wrong_prefix_or_extension() {
        let prefixes = vec!["st_".to_owned()];
        assert_eq!(match_basename("eox_23279.webp", &prefixes), None);
        assert_eq!(match_basename("st_23279.tif", &prefixes), None);
        assert_eq!(match_basename("st_.webp", &prefixes), None);
        assert_eq!(match_basename("st_23a.webp", &prefixes), None);
    }

    #[test]
    fn match_basename_with_empty_prefix_matches_bare_digits() {
        let prefixes = vec![String::new()];
        assert_eq!(match_basename("23279.webp", &prefixes), Some(23279));
        // An "st_23279.webp" stem under the empty prefix would have to parse
        // "st_23279" as digits, which fails. Good — no false positives.
        assert_eq!(match_basename("st_23279.webp", &prefixes), None);
    }

    #[test]
    fn match_basename_walks_prefixes_in_order() {
        let prefixes = vec!["eox_".to_owned(), "st_".to_owned()];
        assert_eq!(match_basename("st_42.webp", &prefixes), Some(42));
        assert_eq!(match_basename("eox_42.webp", &prefixes), Some(42));
    }
}
