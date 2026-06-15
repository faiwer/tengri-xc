use std::fmt;

use crate::geo::XyzTileError;

type ExternalError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum TileTreeError {
    InvalidBounds(&'static str),
    TileOutOfBounds { z: u8, x: u16, y: u16 },
    DuplicateTile { z: u8, x: u16, y: u16 },
    MissingTile { z: u8, x: u16, y: u16 },
    MissingBuilderField(&'static str),
    TileTooLarge(u64),
    CorruptFile(&'static str),
    Unsupported(&'static str),
    WorkerPanicked,
    Io(std::io::Error),
    External(ExternalError),
    Xyz(XyzTileError),
}

impl TileTreeError {
    pub fn external(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::External(Box::new(error))
    }
}

impl fmt::Display for TileTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TileTreeError::InvalidBounds(message) => {
                write!(formatter, "invalid tile tree bounds: {message}")
            }
            TileTreeError::TileOutOfBounds { z, x: lng, y: lat } => {
                write!(
                    formatter,
                    "tile tree tile z={z} lng={lng} lat={lat} is outside the tree bounds"
                )
            }
            TileTreeError::DuplicateTile { z, x: lng, y: lat } => {
                write!(
                    formatter,
                    "tile tree tile z={z} lng={lng} lat={lat} was already written"
                )
            }
            TileTreeError::MissingTile { z, x: lng, y: lat } => {
                write!(
                    formatter,
                    "tile tree tile z={z} lng={lng} lat={lat} is missing"
                )
            }
            TileTreeError::MissingBuilderField(field) => {
                write!(formatter, "tile tree builder is missing {field}")
            }
            TileTreeError::TileTooLarge(size) => {
                write!(
                    formatter,
                    "tile tree payload is too large for the index: {size} bytes"
                )
            }
            TileTreeError::CorruptFile(message) => {
                write!(formatter, "corrupt tile tree file: {message}")
            }
            TileTreeError::Unsupported(message) => {
                write!(formatter, "unsupported by this build: {message}")
            }
            TileTreeError::WorkerPanicked => write!(formatter, "tile tree worker panicked"),
            TileTreeError::Io(error) => write!(formatter, "{error}"),
            TileTreeError::External(error) => write!(formatter, "{error}"),
            TileTreeError::Xyz(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TileTreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TileTreeError::Io(error) => Some(error),
            TileTreeError::External(error) => Some(error.as_ref()),
            TileTreeError::Xyz(error) => Some(error),
            TileTreeError::InvalidBounds(_)
            | TileTreeError::TileOutOfBounds { .. }
            | TileTreeError::DuplicateTile { .. }
            | TileTreeError::MissingTile { .. }
            | TileTreeError::MissingBuilderField(_)
            | TileTreeError::TileTooLarge(_)
            | TileTreeError::CorruptFile(_)
            | TileTreeError::Unsupported(_)
            | TileTreeError::WorkerPanicked => None,
        }
    }
}

impl From<std::io::Error> for TileTreeError {
    fn from(error: std::io::Error) -> Self {
        TileTreeError::Io(error)
    }
}

impl From<XyzTileError> for TileTreeError {
    fn from(error: XyzTileError) -> Self {
        TileTreeError::Xyz(error)
    }
}
