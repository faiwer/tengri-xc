use std::io::{Read, Write};

use bincode::config::standard;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};

use super::error::TengriError;
use crate::flight::compact::CompactTrack;
use crate::flight::metadata::Metadata;

/// File magic: ASCII `"TNGR"`.
pub const MAGIC: [u8; 4] = *b"TNGR";

/// Current container version. Bump on any breaking change to either the
/// header layout, the compression scheme, or any nested `serde` shape
/// (notably [`Metadata`]).
pub const VERSION: u16 = 2;

/// gzip level used for the body. Best gives ~37% extra reduction on
/// delta-encoded payloads vs `default`, at negligible cost for the small
/// files we produce (typical track ≈ 35 KB → ~22 KB).
const GZIP_LEVEL: Compression = Compression::best();

/// On-disk archive: a compact track + sibling metadata. Metadata is
/// deliberately *not* nested inside [`CompactTrack`] — the latter is the
/// time + geometry payload, this struct is the transport wrapper.
///
/// Layout written by [`Self::write`]:
/// ```text
/// [0..4]   MAGIC          "TNGR"
/// [4..6]   VERSION (u16)  little-endian
/// [6..]    gzipped bincode body  { metadata, track }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TengriFile {
    pub metadata: Metadata,
    pub track: CompactTrack,
}

impl TengriFile {
    pub fn write<W: Write>(&self, mut w: W) -> Result<(), TengriError> {
        w.write_all(&MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;

        let body = bincode::serde::encode_to_vec(self, standard())?;
        let mut gz = GzEncoder::new(w, GZIP_LEVEL);
        gz.write_all(&body)?;
        gz.finish()?;
        Ok(())
    }

    pub fn read<R: Read>(mut r: R) -> Result<Self, TengriError> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(TengriError::BadMagic);
        }

        let mut ver = [0u8; 2];
        r.read_exact(&mut ver)?;
        let found = u16::from_le_bytes(ver);
        if found != VERSION {
            return Err(TengriError::UnsupportedVersion {
                found,
                expected: VERSION,
            });
        }

        let mut body = Vec::new();
        GzDecoder::new(r).read_to_end(&mut body)?;
        let (file, _): (Self, _) = bincode::serde::decode_from_slice(&body, standard())?;
        Ok(file)
    }
}
