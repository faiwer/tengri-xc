use super::bounds::XYZBounds;
use super::error::TileTreeError;

/// Per-tree precomputed mapping from `(z, x, y)` to a flat slot index.
///
/// `XYZBounds` is a 10-byte geometric descriptor that's copied freely
/// (metadata, on-disk format, splits, adapters); the slot table is the derived,
/// per-tree structure that turns coordinates into indices in O(1). One
/// `SlotIndex` is built once per open tree (writer or reader) and reused for
/// every `add` / `read`.
pub struct SlotIndex {
    bounds: XYZBounds,
    levels: Box<[LevelDescriptor]>,
    total_entries: u64,
}

#[derive(Debug, Clone, Copy)]
struct LevelDescriptor {
    min_x: u16,
    min_y: u16,
    max_x: u16,
    max_y: u16,
    x_tiles_count: u32,
    /// Index of the first tile in the level in the flat array (the whole index
    /// array). Starts from 0 for the first level.
    start_idx: u64,
}

impl SlotIndex {
    pub fn new(bounds: XYZBounds) -> Result<Self, TileTreeError> {
        let mut levels = Vec::with_capacity(usize::from(bounds.zoom) + 1);
        for _ in 0..=bounds.zoom {
            levels.push(LevelDescriptor::EMPTY);
        }

        let mut total_entries = 0u64;
        for z in (0..=bounds.zoom).rev() {
            let level = bounds.level_bounds(z)?;
            let x_tiles_count = u32::try_from(level.x_tiles_count())
                .map_err(|_| TileTreeError::InvalidBounds("level row exceeds u32"))?;
            levels[usize::from(z)] = LevelDescriptor {
                min_x: level.min_x,
                min_y: level.min_y,
                max_x: level.max_x,
                max_y: level.max_y,
                x_tiles_count,
                start_idx: total_entries,
            };
            total_entries = total_entries
                .checked_add(level.xy_tiles_count())
                .ok_or(TileTreeError::InvalidBounds("index is too large"))?;
        }

        Ok(Self {
            bounds,
            levels: levels.into_boxed_slice(),
            total_entries,
        })
    }

    pub fn bounds(&self) -> XYZBounds {
        self.bounds
    }

    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Returns the index in the flat-array for tile `(z, x, y)`, or
    /// `TileOutOfBounds` if it doesn't lie within the tree.
    pub fn slot(&self, z: u8, x: u16, y: u16) -> Result<usize, TileTreeError> {
        let level = self
            .levels
            .get(usize::from(z))
            .ok_or(TileTreeError::TileOutOfBounds { z, x, y })?;
        if x < level.min_x || x > level.max_x || y < level.min_y || y > level.max_y {
            return Err(TileTreeError::TileOutOfBounds { z, x, y });
        }
        let row = u64::from(y - level.min_y);
        let column = u64::from(x - level.min_x);
        let slot = level.start_idx + row * u64::from(level.x_tiles_count) + column;
        usize::try_from(slot).map_err(|_| TileTreeError::InvalidBounds("index is too large"))
    }
}

impl LevelDescriptor {
    const EMPTY: Self = Self {
        min_x: 0,
        min_y: 0,
        max_x: 0,
        max_y: 0,
        x_tiles_count: 0,
        start_idx: 0,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_is_stable_across_level_arrays() {
        let bounds = XYZBounds::new(2, 1, 1, 2, 2).unwrap();
        let index = SlotIndex::new(bounds).unwrap();

        assert_eq!(index.slot(2, 1, 1).unwrap(), 0);
        assert_eq!(index.slot(2, 2, 2).unwrap(), 3);
        assert_eq!(index.slot(1, 0, 0).unwrap(), 4);
        assert_eq!(index.slot(0, 0, 0).unwrap(), 8);
        assert_eq!(index.total_entries(), 9);
    }

    #[test]
    fn slot_rejects_tiles_outside_pyramid() {
        let bounds = XYZBounds::new(2, 1, 1, 2, 2).unwrap();
        let index = SlotIndex::new(bounds).unwrap();

        assert!(matches!(
            index.slot(3, 0, 0),
            Err(TileTreeError::TileOutOfBounds { z: 3, .. })
        ));
        assert!(matches!(
            index.slot(2, 0, 0),
            Err(TileTreeError::TileOutOfBounds { z: 2, x: 0, y: 0 })
        ));
    }
}
