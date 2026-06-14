use ::tiff::decoder::DecodingResult;

use super::error::TiffReadError;

#[derive(Debug)]
pub enum TifPixelMatrix {
    I16(Vec<i16>),
    I32(Vec<i32>),
    F32(Vec<f32>),
}

impl TifPixelMatrix {
    pub fn len(&self) -> usize {
        match self {
            TifPixelMatrix::I16(pixels) => pixels.len(),
            TifPixelMatrix::I32(pixels) => pixels.len(),
            TifPixelMatrix::F32(pixels) => pixels.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn from_decoding_result(result: DecodingResult) -> Result<Self, TiffReadError> {
        match result {
            DecodingResult::I16(pixels) => Ok(TifPixelMatrix::I16(pixels)),
            DecodingResult::I32(pixels) => Ok(TifPixelMatrix::I32(pixels)),
            DecodingResult::F32(pixels) => Ok(TifPixelMatrix::F32(pixels)),
            DecodingResult::U8(_) => Err(TiffReadError::UnsupportedSampleType("u8")),
            DecodingResult::U16(_) => Err(TiffReadError::UnsupportedSampleType("u16")),
            DecodingResult::U32(_) => Err(TiffReadError::UnsupportedSampleType("u32")),
            DecodingResult::U64(_) => Err(TiffReadError::UnsupportedSampleType("u64")),
            DecodingResult::I8(_) => Err(TiffReadError::UnsupportedSampleType("i8")),
            DecodingResult::I64(_) => Err(TiffReadError::UnsupportedSampleType("i64")),
            DecodingResult::F16(_) => Err(TiffReadError::UnsupportedSampleType("f16")),
            DecodingResult::F64(_) => Err(TiffReadError::UnsupportedSampleType("f64")),
        }
    }
}
