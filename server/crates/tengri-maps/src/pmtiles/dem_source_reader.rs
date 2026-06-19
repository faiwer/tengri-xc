use crate::{
    dem::DemChunk,
    geo::XyzTile,
    tree::{TileSourceReader, TileTreeError},
};

use super::terrarium::decode_terrarium_webp;
use ::pmtiles::{AsyncPmTilesReader, HashMapCache, MmapBackend, TileCoord};
use tokio::runtime::Runtime;

pub struct PmtilesDemSourceReader {
    pub reader: AsyncPmTilesReader<MmapBackend, HashMapCache>,
    pub runtime: Runtime,
}

impl TileSourceReader for PmtilesDemSourceReader {
    type Tile = DemChunk;

    fn read(&mut self, tile: XyzTile) -> Result<DemChunk, TileTreeError> {
        let coord = TileCoord::new(tile.z, tile.x, tile.y)?;
        let bytes = self
            .runtime
            .block_on(self.reader.get_tile_decompressed(coord))?
            .ok_or(TileTreeError::MissingTile {
                z: tile.z,
                x: to_u16(tile.x)?,
                y: to_u16(tile.y)?,
            })?;

        decode_terrarium_webp(&bytes)
    }
}

fn to_u16(value: u32) -> Result<u16, TileTreeError> {
    u16::try_from(value).map_err(|_| TileTreeError::InvalidBounds("tile coordinate exceeds u16"))
}
