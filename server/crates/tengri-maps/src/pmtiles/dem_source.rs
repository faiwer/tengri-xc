use std::path::{Path, PathBuf};

use ::pmtiles::{AsyncPmTilesReader, HashMapCache, MmapBackend, TileType};
use tokio::runtime::{Builder, Runtime};
use super::dem_source_reader::PmtilesDemSourceReader;

use crate::dem::{DemSource, DemSourceReader};
use crate::geo::{Bounds, xyz_tiles_for_bounds};
use crate::tree::{MAX_WEB_MERCATOR_TREE_ZOOM, TileTreeError, WebMercatorTileBounds};

pub struct PmtilesDemSource {
    path: PathBuf,
    tile_bounds: WebMercatorTileBounds,
}

impl PmtilesDemSource {
    pub fn open(path: impl AsRef<Path>, bounds: Option<Bounds>) -> Result<Self, TileTreeError> {
        let path = path.as_ref().to_owned();
        let runtime = runtime()?;
        let reader = open_reader(&runtime, &path)?;
        let header = reader.get_header();
        if header.tile_type != TileType::Webp {
            return Err(TileTreeError::Unsupported(
                "Only WebP Terrarium .pmtiles are supported",
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
        let tile_bounds = WebMercatorTileBounds::from_tiles(zoom, &tiles)?;

        Ok(Self { path, tile_bounds })
    }
}

impl DemSource for PmtilesDemSource {
    fn tile_bounds(&self) -> WebMercatorTileBounds {
        self.tile_bounds
    }

    fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
        let runtime = runtime()?;
        let reader = open_reader(&runtime, &self.path)?;
        Ok(Box::new(PmtilesDemSourceReader { reader, runtime }))
    }

    fn reads_intermediate_tiles(&self) -> bool {
        // PM-tiles files store intermediate tiles, so we don't need to compute
        // them from the lower zoom levels.
        true
    }
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
