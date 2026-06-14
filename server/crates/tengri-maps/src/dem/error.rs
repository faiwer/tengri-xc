use std::fmt;

use super::constants::{MAX_DELTA_BITS, MAX_DEM_TILE_SIDE, MIN_DELTA_BITS};

#[derive(Debug)]
pub enum DemError {
    UnsupportedDimensions { width: u32, height: u32 },
    InvalidDeltaSize(u8),
    InvalidFixIndex { idx: u16, previous_idx: u16 },
    MissingDelta { idx: usize },
    UnexpectedPixelCount { expected: usize, actual: usize },
    Io(std::io::Error),
}

impl fmt::Display for DemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DemError::UnsupportedDimensions { width, height } => write!(
                formatter,
                "unsupported DEM tile dimensions {width}x{height}; width and height must be 1..={MAX_DEM_TILE_SIDE}"
            ),
            DemError::InvalidDeltaSize(size) => {
                write!(
                    formatter,
                    "invalid DEM delta size {size}; expected {MIN_DELTA_BITS}..={MAX_DELTA_BITS}"
                )
            }
            DemError::InvalidFixIndex { idx, previous_idx } => write!(
                formatter,
                "fix index {idx} is not greater than previous fix index {previous_idx}"
            ),
            DemError::MissingDelta { idx } => {
                write!(
                    formatter,
                    "compressed DEM tile is missing delta for pixel {idx}"
                )
            }
            DemError::UnexpectedPixelCount { expected, actual } => write!(
                formatter,
                "decoded {actual} elevations, expected {expected} from tile dimensions"
            ),
            DemError::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for DemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DemError::Io(error) => Some(error),
            DemError::UnsupportedDimensions { .. }
            | DemError::InvalidDeltaSize(_)
            | DemError::InvalidFixIndex { .. }
            | DemError::MissingDelta { .. }
            | DemError::UnexpectedPixelCount { .. } => None,
        }
    }
}

impl From<std::io::Error> for DemError {
    fn from(error: std::io::Error) -> Self {
        DemError::Io(error)
    }
}

impl From<DemError> for crate::tree::TileTreeError {
    fn from(error: DemError) -> Self {
        crate::tree::TileTreeError::external(error)
    }
}
