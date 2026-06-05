use thiserror::Error;

#[derive(Debug, Error)]
pub enum TengriError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("bincode encode error: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("bincode decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("not a .tengri file (bad magic)")]
    BadMagic,

    #[error("unsupported .tengri version {found}; this build supports {expected}")]
    UnsupportedVersion { found: u16, expected: u16 },
}
