use std::io::{Read, Write};

use super::bounds::XYZBounds;
use super::error::TileTreeError;
use super::index::TileTreeIndexEntry;
use super::metadata::{TileKind, TileTreeMetadata};

pub const MAGIC: [u8; 4] = *b"TTRE";
pub const VERSION: u8 = 2;
pub const HEADER_LEN: u64 = 15;
pub const INDEX_ENTRY_LEN: u64 = 12;

pub fn index_offset(slot: usize) -> u64 {
    HEADER_LEN + slot as u64 * INDEX_ENTRY_LEN
}

pub fn payload_offset(index_entries: u64) -> u64 {
    HEADER_LEN + index_entries * INDEX_ENTRY_LEN
}

pub fn write_header(
    writer: &mut impl Write,
    metadata: TileTreeMetadata,
) -> Result<(), TileTreeError> {
    writer.write_all(&MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&[metadata.tile_kind.to_u8()])?;
    let bounds = metadata.bounds;
    writer.write_all(&[bounds.zoom])?;
    writer.write_all(&bounds.min_x.to_le_bytes())?;
    writer.write_all(&bounds.min_y.to_le_bytes())?;
    writer.write_all(&bounds.max_x.to_le_bytes())?;
    writer.write_all(&bounds.max_y.to_le_bytes())?;
    Ok(())
}

pub fn read_header(reader: &mut impl Read) -> Result<TileTreeMetadata, TileTreeError> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(TileTreeError::CorruptFile("missing tile tree header magic"));
    }

    let version = read_u8(reader)?;
    if version != VERSION {
        return Err(TileTreeError::CorruptFile("unsupported tile tree version"));
    }

    let tile_kind = TileKind::from_u8(read_u8(reader)?)
        .ok_or(TileTreeError::CorruptFile("unsupported tree tile kind"))?;
    let bounds = XYZBounds::new(
        read_u8(reader)?,
        read_u16(reader)?,
        read_u16(reader)?,
        read_u16(reader)?,
        read_u16(reader)?,
    )?;
    Ok(TileTreeMetadata::new(tile_kind, bounds))
}

pub fn write_index_entry(
    writer: &mut impl Write,
    entry: TileTreeIndexEntry,
) -> Result<(), TileTreeError> {
    writer.write_all(&entry.offset.to_le_bytes())?;
    writer.write_all(&entry.length.to_le_bytes())?;
    Ok(())
}

pub fn read_index_entry(reader: &mut impl Read) -> Result<TileTreeIndexEntry, TileTreeError> {
    Ok(TileTreeIndexEntry {
        offset: read_u64(reader)?,
        length: read_u32(reader)?,
    })
}

pub fn write_magic(writer: &mut impl Write) -> Result<(), TileTreeError> {
    writer.write_all(&MAGIC)?;
    Ok(())
}

fn read_u8(reader: &mut impl Read) -> Result<u8, TileTreeError> {
    let mut bytes = [0; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn read_u16(reader: &mut impl Read) -> Result<u16, TileTreeError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut impl Read) -> Result<u32, TileTreeError> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, TileTreeError> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}
