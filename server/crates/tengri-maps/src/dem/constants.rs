/// Maximum side length for an in-memory DEM raster tile.
pub const MAX_DEM_TILE_SIDE: u16 = 256;
/// Elevation quantum in metres stored by the compressed DEM format.
pub const DEM_QUANTIZATION_METERS: u16 = 8;
/// Smallest signed delta width considered by the DEM bit-packer.
pub const MIN_DELTA_BITS: u8 = 2;
/// Largest signed delta width considered before storing a fix entry.
pub const MAX_DELTA_BITS: u8 = 8;
