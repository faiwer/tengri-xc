/// Decoded RGB(A) raster. Bytes are tightly packed row-major;
/// `pixels.len()` must equal `width * height * channels`.
#[derive(Debug, Clone)]
pub struct Raster {
    pub width: u16,
    pub height: u16,
    /// 3 (RGB) or 4 (RGBA).
    pub channels: u8,
    pub pixels: Vec<u8>,
}
