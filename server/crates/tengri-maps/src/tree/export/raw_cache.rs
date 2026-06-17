impl RawCache {
    pub(super) fn new(destination: &Path) -> Result<Self, TileTreeError> {
        Ok(Self {
        })
    }

    pub(super) fn spill<A: TileTreeExportAdapter>(
    ) -> Result<(), TileTreeError> {
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use crate::geo::XyzTile;
    use crate::tree::{
        CachedChild, TileKind, TileTreeError, TileTreeExportAdapter, XYZBounds,
    };

    struct FakeAdapter;

    impl TileTreeExportAdapter for FakeAdapter {
        type SourceTile = Vec<u8>;
        type Reader = ();

        fn tile_kind(&self) -> TileKind {
            TileKind::Dem
        }

        fn bounds(&self) -> XYZBounds {
            XYZBounds::new(0, 0, 0, 0, 0).unwrap()
        }

        fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
            Ok(())
        }

        fn try_read_source_tile(
            &self,
            _reader: &mut Self::Reader,
            _tile: XyzTile,
        ) -> Result<Option<Self::SourceTile>, TileTreeError> {
            Ok(None)
        }

        fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
            Ok(tile.clone())
        }

        fn write_raw_cache(
            &self,
            writer: &mut dyn Write,
            tile: &Self::SourceTile,
        ) -> Result<(), TileTreeError> {
            let len = tile.len() as u32;
            writer.write_all(&len.to_le_bytes())?;
            writer.write_all(tile)?;
            Ok(())
        }

        fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
            let mut len_bytes = [0u8; 4];
            reader.read_exact(&mut len_bytes)?;
            let len = u32::from_le_bytes(len_bytes) as usize;
            let mut bytes = vec![0u8; len];
            reader.read_exact(&mut bytes)?;
            Ok(bytes)
        }

        fn reduce_children_to_tile(
            &self,
            _tile: XyzTile,
            _children: &[CachedChild<Self::SourceTile>],
        ) -> Result<Self::SourceTile, TileTreeError> {
            unimplemented!()
        }
    }

    #[test]
    fn spill_then_load_yields_byte_equal_tiles() {
        let dest = std::env::temp_dir().join("tengri-raw-cache-test.tengri-dem");
        let mut cache = RawCache::new(&dest).unwrap();
        let adapter = FakeAdapter;
        let tiles: Vec<Vec<u8>> = (0..8u8)
            .map(|i| (0..((i as usize) * 11 + 1)).map(|j| (j as u8) ^ i).collect())
            .collect();
        cache.spill(&adapter, 5, 42, &tiles).unwrap();

        let loaded = cache.load_and_drop(42).unwrap();
        for (slot, expected) in tiles.iter().enumerate() {
            let mut slice = loaded.slot_bytes(slot as u32).unwrap();
            let actual = adapter.read_raw_cache(&mut slice).unwrap();
            assert_eq!(&actual, expected);
        }
    }
}
