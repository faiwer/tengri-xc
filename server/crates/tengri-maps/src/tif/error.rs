use std::fmt;

use ::tiff::{ColorType, TiffError};

use crate::constants::MAX_DEM_TILE_SIDE;

#[derive(Debug)]
pub enum TiffReadError {
    Io(std::io::Error),
    Decode(TiffError),
    ImageTooLarge,
    UnexpectedPixelCount { expected: usize, actual: usize },
    InvalidBounds(&'static str),
    RegionOutOfBounds,
    UnsupportedColorType(ColorType),
    UnsupportedLayout(&'static str),
    UnsupportedSampleType(&'static str),
}

impl fmt::Display for TiffReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TiffReadError::Io(error) => write!(formatter, "{error}"),
            TiffReadError::Decode(error) => write!(formatter, "{error}"),
            TiffReadError::ImageTooLarge => {
                write!(
                    formatter,
                    "in-memory TIFF raster dimensions must be 1..={MAX_DEM_TILE_SIDE}"
                )
            }
            TiffReadError::UnexpectedPixelCount { expected, actual } => write!(
                formatter,
                "decoded {actual} pixels, expected {expected} from image dimensions"
            ),
            TiffReadError::InvalidBounds(message) => write!(formatter, "invalid bounds: {message}"),
            TiffReadError::RegionOutOfBounds => {
                write!(formatter, "requested region is outside the TIFF extent")
            }
            TiffReadError::UnsupportedColorType(color_type) => write!(
                formatter,
                "unsupported TIFF color type {color_type:?}; only one-channel grayscale TIFFs are supported"
            ),
            TiffReadError::UnsupportedLayout(message) => write!(formatter, "{message}"),
            TiffReadError::UnsupportedSampleType(sample_type) => write!(
                formatter,
                "unsupported TIFF sample type {sample_type}; only signed i16/i32 or float32 grayscale TIFFs are supported"
            ),
        }
    }
}

impl std::error::Error for TiffReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TiffReadError::Io(error) => Some(error),
            TiffReadError::Decode(error) => Some(error),
            TiffReadError::ImageTooLarge
            | TiffReadError::UnexpectedPixelCount { .. }
            | TiffReadError::InvalidBounds(_)
            | TiffReadError::RegionOutOfBounds
            | TiffReadError::UnsupportedColorType(_)
            | TiffReadError::UnsupportedLayout(_)
            | TiffReadError::UnsupportedSampleType(_) => None,
        }
    }
}

impl From<std::io::Error> for TiffReadError {
    fn from(error: std::io::Error) -> Self {
        TiffReadError::Io(error)
    }
}

impl From<TiffError> for TiffReadError {
    fn from(error: TiffError) -> Self {
        TiffReadError::Decode(error)
    }
}
