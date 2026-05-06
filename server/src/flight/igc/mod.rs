//! IGC parser (B-records only for v1; metadata records are skipped).

mod error;
mod parser;

pub use error::IgcError;
pub use parser::parse_str;
