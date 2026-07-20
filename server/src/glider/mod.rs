pub mod import;

/// Glider kinds with a curated brand/model catalog. `'other'` is excluded —
/// it's the catch-all `glider_kind` value the canonical catalog never uses.
pub const CATALOG_KINDS: [&str; 3] = ["hg", "pg", "sp"];
