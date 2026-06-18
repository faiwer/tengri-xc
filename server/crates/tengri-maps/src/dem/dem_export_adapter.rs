use std::io::{Read, Write};

use super::DemChunk;
use super::compress::compress_tile;
use super::pyramid::build_parent_chunk;
use super::resolution::cap_dem_matrix;
use super::source::{DemSource, DemSourceReader};
use super::tile_file::write_tile;
use crate::geo::XyzTile;
use crate::tree::{
    CachedChild, TileKind, TileTreeError, TileTreeExportAdapter, XYZBounds,
};

pub(super) struct DemExportAdapter<S> {
    pub(super) source: S,
}

impl<S: DemSource + 'static> TileTreeExportAdapter for DemExportAdapter<S> {
    type SourceTile = DemChunk;
    type Reader = Box<dyn DemSourceReader>;

    fn tile_kind(&self) -> TileKind {
        TileKind::Dem
    }

    fn bounds(&self) -> XYZBounds {
        self.source.tile_bounds()
    }

    fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
        self.source.open_reader()
    }

    fn supplies_all_zooms(&self) -> bool {
        // The DEM `reads_intermediate_tiles` flag is the strict
        // "dense pyramid" guarantee (see its docstring), so it maps
        // 1:1 to the orchestrator's cache-skip condition.
        self.source.reads_intermediate_tiles()
    }

    fn max_leaf_downsample_steps(&self) -> u8 {
        self.source.max_leaf_downsample_steps()
    }

    fn read_source_tile(
        &self,
        reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Self::SourceTile, TileTreeError> {
        // The orchestrator only calls this when it has already decided
        // the source has the tile (`supplies_all_zooms` or `tile.z` is
        // the leaf), so any error here is a real read failure or a
        // contract bug, not a "wrong zoom" we can silently reduce around.
        Ok(cap_dem_matrix(reader.read(tile)?)?)
    }

    fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
        let compressed = compress_tile(tile.clone())?;
        let mut payload = Vec::new();
        write_tile(&mut payload, &compressed)?;
        Ok(payload)
    }

    fn write_raw_cache(
        &self,
        writer: &mut dyn Write,
        tile: &Self::SourceTile,
    ) -> Result<(), TileTreeError> {
        writer.write_all(&tile.width.to_le_bytes())?;
        writer.write_all(&tile.height.to_le_bytes())?;
        for pixel in &tile.pixels {
            writer.write_all(&pixel.to_le_bytes())?;
        }
        Ok(())
    }

    fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
        let width = read_u16(reader)?;
        let height = read_u16(reader)?;
        let len = usize::from(width) * usize::from(height);
        let mut pixels = Vec::with_capacity(len);
        for _ in 0..len {
            pixels.push(read_i16(reader)?);
        }
        Ok(DemChunk {
            width,
            height,
            pixels,
        })
    }

    fn reduce_children_to_tile(
        &self,
        tile: XyzTile,
        children: &[CachedChild<Self::SourceTile>],
    ) -> Result<Self::SourceTile, TileTreeError> {
        build_parent_chunk(tile, children)
    }
}

fn read_u16(reader: &mut dyn Read) -> Result<u16, TileTreeError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_i16(reader: &mut dyn Read) -> Result<i16, TileTreeError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(i16::from_le_bytes(bytes))
}
