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
/// (notably [`Metadata`] or [`CompactTrack`]).
pub const VERSION: u16 = 4;

/// gzip level used for the body. Best gives ~37% extra reduction on
/// delta-encoded payloads vs `default`, at negligible cost for the small
/// files we produce (typical track ≈ 35 KB → ~22 KB).
const GZIP_LEVEL: Compression = Compression::best();

/// On-disk envelope: a compact track + sibling metadata. Metadata is
/// deliberately *not* nested inside [`CompactTrack`] — the latter is the
/// time + geometry payload, this struct is the transport wrapper.
///
/// `version` is duplicated inside the bincode body so the **HTTP wire form**
/// (just `gzip(bincode(TengriFile))`, no outer framing — see
/// [`Self::write_http`]) is still self-describing. On the disk form, the
/// outer header version is the source of truth; the inner field is checked
/// for consistency on read.
///
/// Disk layout written by [`Self::write`]:
/// ```text
/// [0..4]   MAGIC          "TNGR"
/// [4..6]   VERSION (u16)  little-endian
/// [6..]    gzipped bincode body  { version, metadata, track }
/// ```
///
/// HTTP wire form written by [`Self::write_http`]:
/// ```text
/// gzip( bincode { version, metadata, track } )
/// ```
/// Designed to be served with `Content-Encoding: gzip` so browsers
/// auto-decompress; the resulting bincode is self-versioned via the inner
/// `version` field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TengriFile {
    /// [`TengriError::UnsupportedVersion`].
    pub version: u16,
    pub metadata: Metadata,
    pub track: CompactTrack,
}

impl TengriFile {
    /// Construct a `TengriFile` for the current build's [`VERSION`]. Use this
    /// rather than building the struct literal directly so the version field
    /// stays in sync with the constant.
    pub fn new(metadata: Metadata, track: CompactTrack) -> Self {
        Self {
            version: VERSION,
            metadata,
            track,
        }
    }

    /// Write the on-disk form (magic + outer version + gzipped bincode body).
    pub fn write<W: Write>(&self, mut w: W) -> Result<(), TengriError> {
        w.write_all(&MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        write_gzipped_body(&mut w, self)?;
        Ok(())
    }

    /// Read the on-disk form. Validates magic + outer version, then decodes
    /// the gzipped bincode body. The inner `version` field is asserted to
    /// match the outer one (they're written together; a mismatch indicates
    /// corruption or a hand-crafted file).
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

        let file = read_gzipped_body(r)?;
        if file.version != VERSION {
            return Err(TengriError::UnsupportedVersion {
                found: file.version,
                expected: VERSION,
            });
        }
        Ok(file)
    }

    /// Write the HTTP wire form: just `gzip(bincode)`. No outer magic/version
    /// — the version lives inside the bincode body so the payload is still
    /// self-describing once decompressed. Intended to be served as a response
    /// body with `Content-Encoding: gzip`.
    pub fn write_http<W: Write>(&self, mut w: W) -> Result<(), TengriError> {
        write_gzipped_body(&mut w, self)
    }

    /// Read the HTTP wire form produced by [`Self::write_http`].
    pub fn read_http<R: Read>(r: R) -> Result<Self, TengriError> {
        let file = read_gzipped_body(r)?;
        if file.version != VERSION {
            return Err(TengriError::UnsupportedVersion {
                found: file.version,
                expected: VERSION,
            });
        }
        Ok(file)
    }

    /// Convenience: produce the HTTP wire form as a `Vec<u8>`. Used by the
    /// HTTP layer (which wants a contiguous buffer to hand to axum) and by
    /// the seed/encode paths that store the bytes in Postgres.
    pub fn to_http_bytes(&self) -> Result<Vec<u8>, TengriError> {
        let mut buf = Vec::new();
        self.write_http(&mut buf)?;
        Ok(buf)
    }
}

fn write_gzipped_body<W: Write>(w: W, file: &TengriFile) -> Result<(), TengriError> {
    let body = bincode::serde::encode_to_vec(file, standard())?;
    let mut gz = GzEncoder::new(w, GZIP_LEVEL);
    gz.write_all(&body)?;
    gz.finish()?;
    Ok(())
}

fn read_gzipped_body<R: Read>(r: R) -> Result<TengriFile, TengriError> {
    let mut body = Vec::new();
    GzDecoder::new(r).read_to_end(&mut body)?;
    let (file, _): (TengriFile, _) = bincode::serde::decode_from_slice(&body, standard())?;
    Ok(file)
}
