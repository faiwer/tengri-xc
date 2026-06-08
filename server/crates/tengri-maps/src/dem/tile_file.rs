use std::io::{Read, Write};

use super::error::DemError;
use super::types::{CompressedDemTile, Fix};

pub fn write_tile(mut writer: impl Write, tile: &CompressedDemTile) -> Result<(), DemError> {
    writer.write_all(&tile.start.to_le_bytes())?;
    writer.write_all(&tile.width.to_le_bytes())?;
    writer.write_all(&tile.height.to_le_bytes())?;
    writer.write_all(&[tile.size_per_delta])?;
    writer.write_all(&(tile.fixes.len() as u32).to_le_bytes())?;
    for fix in tile.fixes.iter() {
        writer.write_all(&fix.idx.to_le_bytes())?;
        writer.write_all(&fix.elevation.to_le_bytes())?;
    }
    writer.write_all(&(tile.deltas.len() as u32).to_le_bytes())?;
    writer.write_all(&tile.deltas)?;
    Ok(())
}

pub fn read_tile(mut reader: impl Read) -> Result<CompressedDemTile, DemError> {
    let start = read_i16(&mut reader)?;
    let width = read_u16(&mut reader)?;
    let height = read_u16(&mut reader)?;
    let mut size_per_delta = [0];
    reader.read_exact(&mut size_per_delta)?;

    let fix_count = read_u32(&mut reader)? as usize;
    let mut fixes = Vec::with_capacity(fix_count);
    for _ in 0..fix_count {
        fixes.push(Fix {
            idx: read_u16(&mut reader)?,
            elevation: read_i16(&mut reader)?,
        });
    }

    let delta_len = read_u32(&mut reader)? as usize;
    let mut deltas = vec![0; delta_len];
    reader.read_exact(&mut deltas)?;

    Ok(CompressedDemTile {
        start,
        width,
        height,
        fixes: fixes.into_boxed_slice(),
        size_per_delta: size_per_delta[0],
        deltas: deltas.into_boxed_slice(),
    })
}

fn read_i16(reader: &mut impl Read) -> Result<i16, DemError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(i16::from_le_bytes(bytes))
}

fn read_u16(reader: &mut impl Read) -> Result<u16, DemError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut impl Read) -> Result<u32, DemError> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
