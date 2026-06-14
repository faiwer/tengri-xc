use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ::tiff::ColorType;
use ::tiff::decoder::Decoder;
use ::tiff::tags::Tag;

use crate::dem::DemChunk;
use crate::geo::Bounds;
use crate::tif::error::TiffReadError;
use crate::tif::types::TifPixelMatrix;

use super::copy::{copy_chunk_slice, pixel_count};
use super::downscale::{downscale_to_dem_tile, validate_exact_region_dimensions};
use super::geo::{pixel_region_for_bounds, source_bounds};
use super::types::{TiledTifChunk, TiledTifInfo};

pub struct TiledTifReader {
    decoder: Decoder<BufReader<File>>,
    info: TiledTifInfo,
}

impl TiledTifReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TiffReadError> {
        let file = File::open(path)?;
        let mut decoder = Decoder::new(BufReader::new(file))?;
        let (width, height) = decoder.dimensions()?;
        let color_type = decoder.colortype()?;
        if !matches!(color_type, ColorType::Gray(16) | ColorType::Gray(32)) {
            return Err(TiffReadError::UnsupportedColorType(color_type));
        }

        let sample_format = decoder
            .find_tag_unsigned::<u16>(Tag::SampleFormat)?
            .unwrap_or(1);
        if !matches!(sample_format, 2 | 3) {
            return Err(TiffReadError::UnsupportedSampleType(match sample_format {
                1 => "uint",
                3 => "float",
                4 => "undefined",
                _ => "unknown",
            }));
        }

        let tile_width = decoder.find_tag_unsigned::<u32>(Tag::TileWidth)?.ok_or(
            TiffReadError::UnsupportedLayout(
                "unsupported TIFF layout; expected tiled TIFF with TileWidth",
            ),
        )?;
        let tile_height = decoder.find_tag_unsigned::<u32>(Tag::TileLength)?.ok_or(
            TiffReadError::UnsupportedLayout(
                "unsupported TIFF layout; expected tiled TIFF with TileLength",
            ),
        )?;

        if tile_width == 0 || tile_height == 0 {
            return Err(TiffReadError::UnsupportedLayout(
                "unsupported TIFF tile size; tile dimensions must be positive",
            ));
        }

        let tiles_across = width.div_ceil(tile_width);
        let tiles_down = height.div_ceil(tile_height);
        let scale = decoder.get_tag_f64_vec(Tag::ModelPixelScaleTag)?;
        let tiepoint = decoder.get_tag_f64_vec(Tag::ModelTiepointTag)?;
        if scale.len() < 2 || tiepoint.len() < 6 {
            return Err(TiffReadError::UnsupportedLayout(
                "unsupported GeoTIFF metadata; expected ModelPixelScale and ModelTiepoint",
            ));
        }

        let origin_lon = tiepoint[3] - tiepoint[0] * scale[0];
        let origin_lat = tiepoint[4] + tiepoint[1] * scale[1];
        let bounds = source_bounds(width, height, origin_lon, origin_lat, scale[0], scale[1]);

        Ok(Self {
            decoder,
            info: TiledTifInfo {
                width,
                height,
                tile_width,
                tile_height,
                tiles_across,
                tiles_down,
                origin_lon,
                origin_lat,
                pixel_width_degrees: scale[0],
                pixel_height_degrees: scale[1],
                bounds,
            },
        })
    }

    pub fn info(&self) -> TiledTifInfo {
        self.info
    }

    pub fn chunk_count(&self) -> u32 {
        self.info.tiles_across * self.info.tiles_down
    }

    pub fn read_chunk(&mut self, chunk_index: u32) -> Result<TiledTifChunk, TiffReadError> {
        let tile_x = chunk_index % self.info.tiles_across;
        let tile_y = chunk_index / self.info.tiles_across;
        let width = self
            .info
            .tile_width
            .min(self.info.width - tile_x * self.info.tile_width);
        let height = self
            .info
            .tile_height
            .min(self.info.height - tile_y * self.info.tile_height);
        let pixels = TifPixelMatrix::from_decoding_result(self.decoder.read_chunk(chunk_index)?)?;

        Ok(TiledTifChunk {
            tile_x,
            tile_y,
            width,
            height,
            pixels,
        })
    }

    pub fn read_region(&mut self, bounds: Bounds) -> Result<DemChunk, TiffReadError> {
        let region = pixel_region_for_bounds(bounds, self.info)?;
        validate_exact_region_dimensions(region)?;
        let mut pixels = vec![0; pixel_count(region.width, region.height)?];
        let first_tile_x = region.x / self.info.tile_width;
        let last_tile_x = (region.x + region.width - 1) / self.info.tile_width;
        let first_tile_y = region.y / self.info.tile_height;
        let last_tile_y = (region.y + region.height - 1) / self.info.tile_height;

        let mut chunks = Vec::new();
        for tile_y in first_tile_y..=last_tile_y {
            for tile_x in first_tile_x..=last_tile_x {
                let chunk_index = tile_y * self.info.tiles_across + tile_x;
                chunks.push(self.read_chunk(chunk_index)?);
            }
        }

        for chunk in &chunks {
            copy_chunk_slice(chunk, self.info, region, &mut pixels);
        }
        let (width, height, pixels) = downscale_to_dem_tile(region, pixels);

        Ok(DemChunk::from_i16(width, height, pixels))
    }
}
