use std::io;

use png::{BitDepth, ColorType, Encoder};

use super::decompress::decompress_tile;
use super::tile_file::read_tile;
use crate::serve::{ServedTile, TileServeError, TileServeFormat};

const TERRARIUM_PNG_CONTENT_TYPE: &str = "image/png";

const RENDER_TILE_DIMENSION: u32 = 256;
const TERRARIUM_MIN: f64 = -32768.0;
const TERRARIUM_MAX: f64 = 32767.99609375;

pub(crate) struct DemTerrariumServeFormat;

impl TileServeFormat for DemTerrariumServeFormat {
    fn route_prefix(&self) -> &'static str {
        "/dem/"
    }

    fn file_extension(&self) -> &'static str {
        ".png"
    }

    fn render(&self, payload: &[u8]) -> Result<ServedTile, TileServeError> {
        Ok(ServedTile {
            content_type: TERRARIUM_PNG_CONTENT_TYPE,
            body: render_terrarium_png(payload)?,
        })
    }
}

fn render_terrarium_png(payload: &[u8]) -> Result<Vec<u8>, TileServeError> {
    let tile = read_tile(payload)?;
    let dem = decompress_tile(&tile)?;
    render_png(dem.width, dem.height, &dem.pixels)
}

fn render_png(width: u16, height: u16, elevations: &[i16]) -> Result<Vec<u8>, TileServeError> {
    let source_width = usize::from(width);
    let source_height = usize::from(height);
    let expected_len = source_width * source_height;
    if elevations.len() != expected_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "DEM elevation count does not match tile dimensions",
        )
        .into());
    }

    let mut pixels =
        Vec::with_capacity((RENDER_TILE_DIMENSION * RENDER_TILE_DIMENSION * 3) as usize);
    for output_y in 0..RENDER_TILE_DIMENSION {
        for output_x in 0..RENDER_TILE_DIMENSION {
            let source_x = source_coord(output_x, source_width);
            let source_y = source_coord(output_y, source_height);
            encode_terrarium_elevation(
                elevations,
                source_width,
                source_height,
                source_x,
                source_y,
                &mut pixels,
            );
        }
    }

    let mut png = Vec::new();
    let mut encoder = Encoder::new(&mut png, RENDER_TILE_DIMENSION, RENDER_TILE_DIMENSION);
    encoder.set_color(ColorType::Rgb);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&pixels)?;
    drop(writer);
    Ok(png)
}

fn encode_terrarium_elevation(
    elevations: &[i16],
    source_width: usize,
    source_height: usize,
    source_x: f64,
    source_y: f64,
    pixels: &mut Vec<u8>,
) {
    let x0 = source_x.floor() as usize;
    let y0 = source_y.floor() as usize;
    let x1 = (x0 + 1).min(source_width - 1);
    let y1 = (y0 + 1).min(source_height - 1);
    let x_weight = source_x - x0 as f64;
    let y_weight = source_y - y0 as f64;

    let nw = f64::from(elevations[y0 * source_width + x0]);
    let ne = f64::from(elevations[y0 * source_width + x1]);
    let sw = f64::from(elevations[y1 * source_width + x0]);
    let se = f64::from(elevations[y1 * source_width + x1]);
    let north = nw + (ne - nw) * x_weight;
    let south = sw + (se - sw) * x_weight;
    let elevation = (north + (south - north) * y_weight).clamp(TERRARIUM_MIN, TERRARIUM_MAX);
    let encoded = elevation + 32768.0;
    let whole = encoded.floor();
    pixels.push((whole / 256.0).floor() as u8);
    pixels.push((whole % 256.0).floor() as u8);
    pixels.push(((encoded - whole) * 256.0).floor() as u8);
}

fn source_coord(output_coord: u32, source_len: usize) -> f64 {
    if source_len == 1 {
        return 0.0;
    }

    let scale = source_len as f64 / f64::from(RENDER_TILE_DIMENSION);
    ((f64::from(output_coord) + 0.5) * scale - 0.5).clamp(0.0, source_len as f64 - 1.0)
}
