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

    /// Returns the bounds that contains all the given tiles.
    pub fn from_tiles(zoom: u8, tiles: &[XyzTile]) -> Result<Self, TileTreeError> {
        if tiles.is_empty() {
            return Err(TileTreeError::InvalidBounds(
                "bounds need at least one tile",
            ));
        }

        let mut min_lng = u32::MAX;
        let mut min_lat = u32::MAX;
        let mut max_lng = 0;
        let mut max_lat = 0;
        for tile in tiles {
            if tile.z != zoom {
                return Err(TileTreeError::InvalidBounds(
                    "all tiles must use the same zoom",
                ));
            }
            min_lng = min_lng.min(tile.x);
            min_lat = min_lat.min(tile.y);
            max_lng = max_lng.max(tile.x);
            max_lat = max_lat.max(tile.y);
        }

        Self::new(
            zoom,
            to_u16(min_lng)?,
            to_u16(min_lat)?,
            to_u16(max_lng)?,
            to_u16(max_lat)?,
        )
    }

    pub fn validate(self) -> Result<(), TileTreeError> {
        if self.zoom > MAX_WEB_MERCATOR_TREE_ZOOM {
            return Err(TileTreeError::InvalidBounds(
                "zoom is higher than the tile tree limit",
            ));
        }
        if self.min_x > self.max_x || self.min_y > self.max_y {
            return Err(TileTreeError::InvalidBounds(
                "minimum tile coordinate exceeds maximum",
            ));
        }

        let max_coord = (1u32 << u32::from(self.zoom)) - 1;
        if u32::from(self.max_x) > max_coord || u32::from(self.max_y) > max_coord {
            return Err(TileTreeError::InvalidBounds(
                "tile coordinate exceeds zoom range",
            ));
        }

        Ok(())
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

    pub fn x_tiles_count(self) -> u64 {
        u64::from(self.max_x - self.min_x) + 1
    }

    pub fn y_tiles_count(self) -> u64 {
        u64::from(self.max_y - self.min_y) + 1
    }

    pub fn xy_tiles_count(self) -> u64 {
        self.x_tiles_count() * self.y_tiles_count()
    }

    pub fn contains(self, z: u8, lng: u16, lat: u16) -> bool {
        let Ok(bounds) = self.level_bounds(z) else {
            return false;
        };
        bounds.min_x <= lng
            && lng <= bounds.max_x
            && bounds.min_y <= lat
            && lat <= bounds.max_y
    }

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
            XYZBounds::new(3, 4, 2, 6, 3).unwrap(),
        );
    }

}
