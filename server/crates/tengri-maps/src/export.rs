//! `export-tree` command: build a `.tengri-map` tile-tree from a DEM or
//! imagery source.

use std::path::PathBuf;
use std::process;
use std::thread;

use tengri_maps::dem::{DemChunk, DemTree};
use tengri_maps::dir::DirImagerySource;
use tengri_maps::geo::Bounds;
use tengri_maps::matrix::Raster;
use tengri_maps::pmtiles::{PmtilesDemSource, PmtilesImagerySource};
use tengri_maps::tif::TifDemSource;
use tengri_maps::tree::TileSource;
use tengri_maps::webp::WebpTree;

pub fn export_tree(program: &str, mut args: impl Iterator<Item = std::ffi::OsString>) {
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help"))
        || first.as_deref() == Some(std::ffi::OsStr::new("-h"))
    {
        print_usage(program);
        return;
    }

    let Some(source_path) = first.map(PathBuf::from) else {
        print_usage(program);
        process::exit(2);
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        print_usage(program);
        process::exit(2);
    };
    let rest: Vec<_> = args.collect();
    let parsed = match parse_export_args(&rest) {
        Some(parsed) => parsed,
        None => {
            print_usage(program);
            process::exit(2);
        }
    };

    let Some(kind) = parsed.kind else {
        eprintln!("error: --kind dem|webp is required");
        print_usage(program);
        process::exit(2);
    };

    match kind {
        ExportKind::Dem => run_dem_export(source_path, output, &parsed),
        ExportKind::Webp => run_webp_export(source_path, output, &parsed),
    }
}

pub fn print_usage(program: &str) {
    eprintln!("usage: {program} <file.tif>");
    eprintln!(
        "       {program} export-tree <source-{{file|dir}}> <output.tengri-map> [min_lat min_lon max_lat max_lon] [threads]"
    );
    eprintln!("           --kind dem|webp [--min-zoom N] [--max-zoom N]");
    eprintln!("           [--quality N] [--passthrough]");
    eprintln!(
        "           [--prefixes p1,p2,...]   (directory imagery only; default: empty prefix ⇒ <y>.<ext>)"
    );
    eprintln!("       sources: .tif | .pmtiles | <dir>/<z>/<x>/<prefix><y>.{{webp,png,jpg}}");
}

fn run_dem_export(source_path: PathBuf, output: PathBuf, parsed: &ParsedExportArgs) {
    let source = match open_dem_source(source_path, parsed.bounds) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to open DEM tree source: {error}");
            process::exit(1);
        }
    };

    let mut builder = DemTree::builder(source)
        .output(output)
        .threads(parsed.threads)
        .min_zoom(parsed.min_zoom)
        .progress(std::io::stderr());
    if let Some(max_zoom) = parsed.max_zoom {
        builder = builder.max_zoom(max_zoom);
    }

    match builder.build() {
        Ok(report) => {
            println!("zoom: {}", report.zoom);
            println!("tiles-written: {}", report.tiles_written);
        }
        Err(error) => {
            eprintln!("failed to export DEM tree: {error}");
            process::exit(1);
        }
    }
}

fn run_webp_export(source_path: PathBuf, output: PathBuf, parsed: &ParsedExportArgs) {
    let source = match open_imagery_source(source_path, parsed.bounds, parsed.prefixes.clone()) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to open WebP tree source: {error}");
            process::exit(1);
        }
    };

    let mut builder = WebpTree::builder(source)
        .output(output)
        .threads(parsed.threads)
        .min_zoom(parsed.min_zoom)
        .quality(parsed.quality)
        .passthrough(parsed.passthrough)
        .progress(std::io::stderr());
    if let Some(max_zoom) = parsed.max_zoom {
        builder = builder.max_zoom(max_zoom);
    }

    match builder.build() {
        Ok(report) => {
            println!("zoom: {}", report.zoom);
            println!("tiles-written: {}", report.tiles_written);
        }
        Err(error) => {
            eprintln!("failed to export WebP tree: {error}");
            process::exit(1);
        }
    }
}

fn open_dem_source(
    path: PathBuf,
    bounds: Option<Bounds>,
) -> Result<Box<dyn TileSource<Tile = DemChunk>>, tengri_maps::tree::TileTreeError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    match extension {
        "pmtiles" => Ok(Box::new(PmtilesDemSource::open(path, bounds)?)),
        "tif" | "tiff" => Ok(Box::new(TifDemSource::open(path, bounds)?)),
        _ => Err(tengri_maps::tree::TileTreeError::Unsupported(
            "DEM source must be a .tif/.tiff or .pmtiles archive",
        )),
    }
}

fn open_imagery_source(
    path: PathBuf,
    bounds: Option<Bounds>,
    prefixes: Option<Vec<String>>,
) -> Result<Box<dyn TileSource<Tile = Raster>>, tengri_maps::tree::TileTreeError> {
    if path.is_dir() {
        // No --prefixes ⇒ single empty prefix, so the basename is just
        // `<y>.<ext>`. With --prefixes, each entry is concatenated verbatim in
        // front of `<y>` — any separator (`_`, `-`, …) lives inside the prefix.
        let prefixes = prefixes.unwrap_or_else(|| vec![String::new()]);
        return Ok(Box::new(DirImagerySource::open(path, prefixes, bounds)?));
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    match extension {
        "pmtiles" => Ok(Box::new(PmtilesImagerySource::open(path, bounds)?)),
        _ => Err(tengri_maps::tree::TileTreeError::Unsupported(
            "imagery source must be a directory of loose tiles or a .pmtiles archive",
        )),
    }
}

#[derive(Clone, Copy)]
enum ExportKind {
    Dem,
    Webp,
}

struct ParsedExportArgs {
    bounds: Option<Bounds>,
    threads: usize,
    min_zoom: u8,
    max_zoom: Option<u8>,
    kind: Option<ExportKind>,
    quality: u8,
    passthrough: bool,
    prefixes: Option<Vec<String>>,
}

/// Strip `--min-zoom`, `--max-zoom`, `--kind`, `--quality`,
/// `--passthrough`, and `--prefixes` flags from the argument tail, then
/// dispatch the remaining positional shape (bounds / threads).
fn parse_export_args(args: &[std::ffi::OsString]) -> Option<ParsedExportArgs> {
    let mut min_zoom: u8 = 0;
    let mut max_zoom: Option<u8> = None;
    let mut kind: Option<ExportKind> = None;
    let mut quality: u8 = 75;
    let mut passthrough = false;
    let mut prefixes: Option<Vec<String>> = None;
    let mut positional: Vec<std::ffi::OsString> = Vec::with_capacity(args.len());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let arg_str = arg.to_str();
        match arg_str {
            Some("--min-zoom") => min_zoom = parse_os_arg(iter.next()?)?,
            Some("--max-zoom") => max_zoom = Some(parse_os_arg(iter.next()?)?),
            Some("--kind") => kind = Some(parse_kind(iter.next()?)?),
            Some("--quality") => {
                let q: u8 = parse_os_arg(iter.next()?)?;
                if q > 100 {
                    return None;
                }
                quality = q;
            }
            Some("--passthrough") => passthrough = true,
            Some("--prefixes") => prefixes = Some(parse_prefixes(iter.next()?)?),
            _ => positional.push(arg.clone()),
        }
    }

    let (bounds, threads) = match positional.len() {
        0 => (None, default_thread_count()),
        1 => (None, parse_os_arg(&positional[0])?),
        4 | 5 => {
            let bounds = Bounds {
                min_lat: parse_os_arg(&positional[0])?,
                min_lon: parse_os_arg(&positional[1])?,
                max_lat: parse_os_arg(&positional[2])?,
                max_lon: parse_os_arg(&positional[3])?,
            };
            let threads = match positional.get(4) {
                Some(arg) => parse_os_arg(arg)?,
                None => default_thread_count(),
            };
            (Some(bounds), threads)
        }
        _ => return None,
    };
    Some(ParsedExportArgs {
        bounds,
        threads,
        min_zoom,
        max_zoom,
        kind,
        quality,
        passthrough,
        prefixes,
    })
}

/// Comma-separated prefix list, e.g. `eox_,bz_,at_`. Whitespace around
/// each entry is trimmed; empty entries collapse to a single empty prefix
/// (`""` ⇒ basename is just `<y>.<ext>`). Returns `None` if the argument
/// can't be decoded as UTF-8 or yields no entries.
fn parse_prefixes(arg: &std::ffi::OsString) -> Option<Vec<String>> {
    let raw = arg.to_str()?;
    let prefixes: Vec<String> = raw.split(',').map(|s| s.trim().to_owned()).collect();
    if prefixes.is_empty() {
        return None;
    }
    Some(prefixes)
}

fn parse_kind(arg: &std::ffi::OsString) -> Option<ExportKind> {
    match arg.to_str()? {
        "dem" => Some(ExportKind::Dem),
        "webp" => Some(ExportKind::Webp),
        _ => None,
    }
}

fn parse_os_arg<T: std::str::FromStr>(arg: &std::ffi::OsString) -> Option<T> {
    arg.to_str()?.parse().ok()
}

fn default_thread_count() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}
