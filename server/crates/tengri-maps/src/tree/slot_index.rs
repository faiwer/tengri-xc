use super::bounds::XYZBounds;
use super::error::TileTreeError;

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

