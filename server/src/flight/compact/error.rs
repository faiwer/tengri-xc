use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompactError {
    #[error("track is empty; encoder requires at least one point")]
    EmptyTrack,

    #[error("inconsistent altitude columns: some points carry pressure altitude, others do not")]
    InconsistentPressureAlt,

    #[error("inconsistent TAS column: some points carry TAS, others do not")]
    InconsistentTas,

    #[error("compact track is malformed: missing initial fix at idx=0")]
    MissingInitialFix,

    #[error(
        "compact track is malformed: fix indices must be strictly increasing (got {prev} then {next})"
    )]
    UnorderedFixes { prev: u32, next: u32 },

    #[error("compact track is malformed: time_fix indices must be strictly increasing")]
    UnorderedTimeFixes,

    #[error("compact track is malformed: index {idx} out of range (track length is {len})")]
    IndexOutOfRange { idx: u32, len: u32 },
}
