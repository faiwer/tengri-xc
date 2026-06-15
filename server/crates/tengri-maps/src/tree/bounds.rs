use super::error::TileTreeError;
use crate::geo::XyzTile;

pub const MAX_WEB_MERCATOR_TREE_ZOOM: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XYZBounds {
    pub zoom: u8,
    pub min_x: u16, // lng
    pub min_y: u16, // lng
    pub max_x: u16, // lat
    pub max_y: u16, // lng
}

impl XYZBounds {
    pub fn new(
        zoom: u8,
        min_lng: u16,
        min_lat: u16,
        max_lng: u16,
        max_lat: u16,
    ) -> Result<Self, TileTreeError> {
        let bounds = Self {
            zoom,
            min_x: min_lng,
            min_y: min_lat,
            max_x: max_lng,
            max_y: max_lat,
        };
        bounds.validate()?;
        Ok(bounds)
    }

    pub fn from_tiles(zoom: u8, tiles: &[XyzTile]) -> Result<Self, TileTreeError> {
    }

    pub fn validate(self) -> Result<(), TileTreeError> {
    }

    pub fn level_bounds(self, zoom: u8) -> Result<Self, TileTreeError> {
        if zoom > self.zoom {
            return Err(TileTreeError::InvalidBounds("level zoom exceeds tree zoom"));
        }

        let shift = u32::from(self.zoom - zoom);
        Self::new(
            zoom,
            self.min_x >> shift,
            self.min_y >> shift,
            self.max_x >> shift,
            self.max_y >> shift,
        )
    }

    pub fn lng_tiles(self) -> u64 {
    }

    pub fn lat_tiles(self) -> u64 {
    }

    pub fn tile_count(self) -> u64 {
    }

    pub fn contains(self, z: u8, lng: u16, lat: u16) -> bool {
    /// Returns Vec of { x, y, z } tiles at the given zoom level.
    pub fn tiles_at(self, zoom: u8) -> Result<Vec<XyzTile>, TileTreeError> {
        let bounds = self.level_bounds(zoom)?;
        let mut tiles = Vec::with_capacity(
            usize::try_from(bounds.xy_tiles_count())
                .map_err(|_| TileTreeError::InvalidBounds("level is too large"))?,
        );
        for lat in bounds.min_y..=bounds.max_y {
            for lng in bounds.min_x..=bounds.max_x {
                tiles.push(XyzTile {
                    z: zoom,
                    x: u32::from(lng),
                    y: u32::from(lat),
                });
            }
        }
        Ok(tiles)
    }
}

fn to_u16(value: u32) -> Result<u16, TileTreeError> {
    u16::try_from(value).map_err(|_| TileTreeError::InvalidBounds("tile coordinate exceeds u16"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_bounds_are_derived_by_shifting_edges() {
        let bounds = XYZBounds::new(4, 9, 4, 12, 7).unwrap();

        assert_eq!(
            bounds.level_bounds(3).unwrap(),
            XYZBounds {
                zoom: 3,
                min_x: 4,
                min_y: 2,
                max_x: 6,
                max_y: 3,
            }
        );
    }

    #[test]
    fn slot_is_stable_across_level_arrays() {
        let bounds = XYZBounds::new(2, 1, 1, 2, 2).unwrap();

        assert_eq!(bounds.slot(2, 1, 1).unwrap(), 0);
        assert_eq!(bounds.slot(2, 2, 2).unwrap(), 3);
        assert_eq!(bounds.slot(1, 0, 0).unwrap(), 4);
        assert_eq!(bounds.slot(0, 0, 0).unwrap(), 8);
        assert_eq!(bounds.total_index_entries().unwrap(), 9);
    }
}
