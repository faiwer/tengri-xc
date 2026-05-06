use thiserror::Error;

#[derive(Debug, Error)]
pub enum IgcError {
    #[error("input is empty")]
    Empty,

    #[error("no B-records found")]
    NoFixes,

    #[error("invalid B-record at line {line}: {reason}")]
    InvalidBRecord { line: usize, reason: String },

    #[error("inconsistent altitude columns: some fixes have pressure altitude, others do not")]
    InconsistentPressureAlt,
}
