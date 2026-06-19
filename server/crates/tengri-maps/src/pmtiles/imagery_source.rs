use std::path::{Path, PathBuf};

use ::pmtiles::{AsyncPmTilesReader, HashMapCache, MmapBackend, TileCoord, TileType};
use image_webp::WebPDecoder;
use std::io::{BufReader, Cursor};
use tokio::runtime::{Builder, Runtime};

use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::geo::{Bounds, XyzTile, xyz_tiles_for_bounds};
use crate::matrix::{Raster, area_resample};
use crate::tree::{
    MAX_WEB_MERCATOR_TREE_ZOOM, PassthroughCodec, TileSource, TileSourceReader, TileTreeError,
    XYZBounds,
};

/// PMTiles WebP imagery source. The primary [`read`](TileSourceReader::read)
/// path always returns a decoded [`Raster`] (resampled to the archive's
/// tile-side when the source ships larger tiles); the secondary
/// [`read_raw`](TileSourceReader::read_raw) channel hands back the undecoded
/// PMTiles bytes for the WebP exporter's passthrough fast-path. Whether
/// passthrough actually fires (codec match + dim match) is decided by the
/// exporter, not here.
pub struct PmtilesImagerySource {
    path: PathBuf,
    tile_bounds: XYZBounds,
}

impl PmtilesImagerySource {
    pub fn open(path: impl AsRef<Path>, bounds: Option<Bounds>) -> Result<Self, TileTreeError> {
        let path = path.as_ref().to_owned();
        let runtime = runtime()?;
        let reader = open_reader(&runtime, &path)?;
        let header = reader.get_header();
        if header.tile_type != TileType::Webp {
            return Err(TileTreeError::Unsupported(
                "Only WebP-tiled .pmtiles are supported as imagery sources",
            ));
        }

        let zoom = header.max_zoom.min(MAX_WEB_MERCATOR_TREE_ZOOM);
        let source_bounds = Bounds {
            min_lat: header.min_latitude,
            min_lon: header.min_longitude,
            max_lat: header.max_latitude,
            max_lon: header.max_longitude,
        };
        let read_bounds = bounds.unwrap_or(source_bounds);
        let tiles = xyz_tiles_for_bounds(read_bounds, zoom)?;
        let tile_bounds = XYZBounds::from_tiles(zoom, &tiles)?;

        Ok(Self { path, tile_bounds })
    }
}

impl TileSource for PmtilesImagerySource {
    type Tile = Raster;

    fn tile_bounds(&self) -> XYZBounds {
        self.tile_bounds
    }

    fn open_reader(&self) -> Result<Box<dyn TileSourceReader<Tile = Raster>>, TileTreeError> {
        let runtime = runtime()?;
        let reader = open_reader(&runtime, &self.path)?;
        Ok(Box::new(PmtilesImageryReader { reader, runtime }))
    }

    fn reads_intermediate_tiles(&self) -> bool {
        // We support only dense PMTiles. They ship a dense pyramid: every zoom
        // is served directly, the orchestrator's reduce path never fires.
        true
    }

    fn raw_codec(&self) -> Option<PassthroughCodec> {
        // Thus far only WebP is supported. Validated earlier in `open`.
        Some(PassthroughCodec::Webp)
    }
}

struct PmtilesImageryReader {
    reader: AsyncPmTilesReader<MmapBackend, HashMapCache>,
    runtime: Runtime,
}

impl TileSourceReader for PmtilesImageryReader {
    type Tile = Raster;

    fn read(&mut self, tile: XyzTile) -> Result<Raster, TileTreeError> {
        let bytes = self.fetch_bytes(tile)?;
        let raster = decode_full(&bytes)?;
        Ok(area_resample_to_tile(&raster))
    }

    fn read_raw(&mut self, tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
        // Hand back the undecoded PMTiles bytes; the exporter validates the dim
        // before deciding to passthrough or fall back to `read`.
        Ok(Some(self.fetch_bytes(tile)?))
    }
}

impl PmtilesImageryReader {
    fn fetch_bytes(&mut self, tile: XyzTile) -> Result<Vec<u8>, TileTreeError> {
        let coord = TileCoord::new(tile.z, tile.x, tile.y)?;
        let bytes = self
            .runtime
            .block_on(self.reader.get_tile_decompressed(coord))?
            .ok_or(TileTreeError::MissingTile {
                z: tile.z,
                x: to_u16(tile.x)?,
                y: to_u16(tile.y)?,
            })?
            .to_vec();
        Ok(bytes)
    }
}

fn decode_full(bytes: &[u8]) -> Result<Raster, TileTreeError> {
    let mut decoder = WebPDecoder::new(BufReader::new(Cursor::new(bytes)))?;
    let (width, height) = decoder.dimensions();
    let buffer_size = decoder
        .output_buffer_size()
        .ok_or(TileTreeError::CorruptFile("WebP output buffer too large"))?;
    let channels: u8 = if decoder.has_alpha() { 4 } else { 3 };
    let mut pixels = vec![0u8; buffer_size];
    decoder.read_image(&mut pixels)?;
    Ok(Raster {
        width: width as u16,
        height: height as u16,
        channels,
        pixels,
    })
}

/// Area-resample an RGB(A) raster down to one archive tile-side per side.
fn area_resample_to_tile(raster: &Raster) -> Raster {
    let target = usize::from(MAX_DEM_TILE_SIDE);
    let source_w = usize::from(raster.width);
    let source_h = usize::from(raster.height);
    let dst_w = source_w.min(target);
    let dst_h = source_h.min(target);
    if dst_w == source_w && dst_h == source_h {
        return raster.clone();
    }
    let channels = usize::from(raster.channels);
    let resampled_planes: Vec<Vec<u8>> = (0..channels)
        .map(|ch| {
            let plane: Vec<u8> = raster
                .pixels
                .iter()
                .skip(ch)
                .step_by(channels)
                .copied()
                .collect();
            area_resample(&plane, source_w, source_h, dst_w, dst_h, |v| f64::from(v))
                .into_iter()
                .map(|v| v.round().clamp(0.0, 255.0) as u8)
                .collect()
        })
        .collect();
    let mut output = Vec::with_capacity(dst_w * dst_h * channels);

    for i in 0..(dst_w * dst_h) {
        for plane in &resampled_planes {
            output.push(plane[i]);
        }
    }

    Raster {
        width: dst_w as u16,
        height: dst_h as u16,
        channels: raster.channels,
        pixels: output,
    }
}

fn to_u16(value: u32) -> Result<u16, TileTreeError> {
    u16::try_from(value).map_err(|_| TileTreeError::InvalidBounds("tile coordinate exceeds u16"))
}

fn runtime() -> Result<Runtime, TileTreeError> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(TileTreeError::Io)
}

fn open_reader(
    runtime: &Runtime,
    path: &Path,
) -> Result<AsyncPmTilesReader<MmapBackend, HashMapCache>, TileTreeError> {
    runtime
        .block_on(AsyncPmTilesReader::new_with_cached_path(
            HashMapCache::default(),
            path,
        ))
        .map_err(TileTreeError::from)
}
