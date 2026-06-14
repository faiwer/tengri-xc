#[derive(Debug)]
pub struct DemChunk {
    pub width: u16,
    pub height: u16,
    pub pixels: DemPixelMatrix,
}

#[derive(Debug)]
pub enum DemPixelMatrix {
    I16(Vec<i16>),
    I32(Vec<i32>),
    F32(Vec<f32>),
}

impl DemPixelMatrix {
    pub fn len(&self) -> usize {
        match self {
            DemPixelMatrix::I16(pixels) => pixels.len(),
            DemPixelMatrix::I32(pixels) => pixels.len(),
            DemPixelMatrix::F32(pixels) => pixels.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedDemTile {
    pub start: i16,
    pub width: u16,
    pub height: u16,
    pub fixes: Box<[Fix]>,
    pub size_per_delta: u8,
    /// Zstd-compressed packed deltas.
    pub deltas: Box<[u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fix {
    pub idx: u16,
    pub elevation: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncompressedDemTile {
    pub start: i16,
    pub fixes: Box<[Fix]>,
    pub width: u16,
    pub height: u16,
    pub elevations: Box<[u16]>,
}
