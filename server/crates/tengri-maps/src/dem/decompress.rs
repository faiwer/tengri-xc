use super::bitpack::BitReader;
use super::constants::{
    DEM_QUANTIZATION_METERS, MAX_DELTA_BITS, MAX_DEM_TILE_SIDE, MIN_DELTA_BITS,
};
use super::error::DemError;
use super::types::{CompressedDemTile, DemChunk, Fix};

pub fn decompress_tile(source: &CompressedDemTile) -> Result<DemChunk, DemError> {
    validate_delta_size(source.size_per_delta)?;
    validate_dimensions(source.width, source.height)?;
    validate_fixes(&source.fixes)?;

    let deltas = zstd::decode_all(source.deltas.as_ref())?;

    let width = usize::from(source.width);
    let height = usize::from(source.height);
    let point_count = width * height;
    let mut stored_elevations = Vec::with_capacity(point_count);
    stored_elevations.push(normalize_stored_elevation(i32::from(source.start)));

    let mut bits = BitReader::new(&deltas);
    let mut fix_idx = 0;

    for idx in 1..point_count {
        if source
            .fixes
            .get(fix_idx)
            .is_some_and(|fix| usize::from(fix.idx) == idx)
        {
            stored_elevations.push(normalize_stored_elevation(i32::from(
                source.fixes[fix_idx].elevation,
            )));
            fix_idx += 1;
            continue;
        }

        let delta = bits
            .read_signed(source.size_per_delta)
            .ok_or(DemError::MissingDelta { idx })?;
        let reference = i32::from(stored_elevations[reference_idx(idx, width)]);
        stored_elevations.push(normalize_stored_elevation(reference + i32::from(delta)));
    }
    let pixels = stored_elevations
        .into_iter()
        .map(|elevation| restore_elevation(elevation) as i16)
        .collect();

    Ok(DemChunk {
        width: source.width,
        height: source.height,
        pixels,
    })
}

fn validate_dimensions(width: u16, height: u16) -> Result<(), DemError> {
    if width == 0 || height == 0 || width > MAX_DEM_TILE_SIDE || height > MAX_DEM_TILE_SIDE {
        return Err(DemError::UnsupportedDimensions {
            width: u32::from(width),
            height: u32::from(height),
        });
    }

    Ok(())
}

fn validate_delta_size(size_per_delta: u8) -> Result<(), DemError> {
    match size_per_delta {
        MIN_DELTA_BITS..=MAX_DELTA_BITS => Ok(()),
        size => Err(DemError::InvalidDeltaSize(size)),
    }
}

fn validate_fixes(fixes: &[Fix]) -> Result<(), DemError> {
    let mut previous_idx = 0;
    for (offset, fix) in fixes.iter().enumerate() {
        if offset > 0 && fix.idx <= previous_idx {
            return Err(DemError::InvalidFixIndex {
                idx: fix.idx,
                previous_idx,
            });
        }
        previous_idx = fix.idx;
    }
    Ok(())
}

fn reference_idx(idx: usize, dimension: usize) -> usize {
    if idx % dimension == 0 {
        idx - dimension
    } else {
        idx - 1
    }
}

fn normalize_stored_elevation(elevation: i32) -> u16 {
    elevation.clamp(0, i32::from(i16::MAX)) as u16
}

fn restore_elevation(elevation: u16) -> u16 {
    i32::from(elevation)
        .saturating_mul(i32::from(DEM_QUANTIZATION_METERS))
        .clamp(0, i32::from(i16::MAX)) as u16
}
