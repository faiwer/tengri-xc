use super::bounds::XYZBounds;
use super::format::VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Dem,
    Webp,
}

impl TileKind {
    pub(crate) fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Dem),
            2 => Some(Self::Webp),
            _ => None,
        }
    }

    pub(crate) fn to_u8(self) -> u8 {
        match self {
            Self::Dem => 1,
            Self::Webp => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileTreeMetadata {
    pub version: u8,
    pub tile_kind: TileKind,
    pub bounds: XYZBounds,
}

impl TileTreeMetadata {
    pub fn new(tile_kind: TileKind, bounds: XYZBounds) -> Self {
        Self {
            version: VERSION,
            tile_kind,
            bounds,
        }
    }
}
