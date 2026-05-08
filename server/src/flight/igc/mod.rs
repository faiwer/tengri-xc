//! IGC parser (B-records only for v1; metadata records are skipped).

mod decode;
mod error;
mod parser;

pub use decode::decode_text;
pub use error::IgcError;
pub use parser::parse_str;
