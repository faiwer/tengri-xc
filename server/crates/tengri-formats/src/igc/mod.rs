//! IGC parser/writer (B-records plus minimal headers).

mod decode;
mod encode;
mod error;
mod parser;

pub use decode::decode_text;
pub use encode::write;
pub use error::IgcError;
pub use parser::parse_str;
