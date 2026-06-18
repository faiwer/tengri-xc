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
    /// A leaf-only source was asked to produce its leaf at a zoom further below
    /// its native pixel pitch than it supports. E.g. `TifDemSource` can
    /// downsample at most one zoom level at a single leaf read; a `--max-zoom`
    /// cap that demands more pixels per tile than the native pitch can fit
    /// through the source's downsample path lands here. The orchestrator raises
    /// this at startup, before the first tile read.
    LeafZoomGapTooLarge {
        source_zoom: u8,
        requested_zoom: u8,
        max_supported_gap: u8,
    },
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
            TileTreeError::LeafZoomGapTooLarge {
                source_zoom,
                requested_zoom,
                max_supported_gap,
            } => {
                let gap = source_zoom.saturating_sub(*requested_zoom);
                write!(
                    formatter,
                    "requested leaf zoom {requested_zoom} is {gap} levels below the source's native zoom {source_zoom}; \
                    this source supports at most {max_supported_gap} downsample step(s) at the leaf — \
                    pick --max-zoom {} or higher",
                    source_zoom.saturating_sub(*max_supported_gap)
                )
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
            | TileTreeError::LeafZoomGapTooLarge { .. }
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
