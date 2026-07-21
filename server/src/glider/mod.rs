pub mod import;

/// Glider kinds with a curated brand/model catalog. `'other'` is excluded —
/// it's the catch-all `glider_kind` value the canonical catalog never uses.
pub const CATALOG_KINDS: [&str; 3] = ["hg", "pg", "sp"];

/// `launch_method` enum values (see `0009_gliders.sql`).
pub const LAUNCH_METHODS: [&str; 3] = ["foot", "winch", "aerotow"];

/// `propulsion` enum values (see `0009_gliders.sql`).
pub const PROPULSIONS: [&str; 3] = ["free", "self_launch", "powered"];
