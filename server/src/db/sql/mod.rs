//! Tiny SQL builder layered on top of `sqlx::QueryBuilder`. Turns
//! the "string concatenation with manual `WHERE`/`AND` bookkeeping"
//! pattern into a fluent API while staying inside the sqlx ecosystem
//! (no ORM, no separate query AST).
//!
//! Three builders, one per SQL shape the codebase actually writes:
//! - [`Sql`] — `SELECT … FROM … [JOIN …] WHERE … ORDER BY … LIMIT …`
//! - [`Update`] — `UPDATE … SET … WHERE …`
//! - [`Upsert`] — `INSERT … ON CONFLICT (…) DO UPDATE SET col = EXCLUDED.col, …`
//!
//! # Example (SELECT)
//!
//! ```ignore
//! use crate::db::{Sql, Order};
//!
//! let mut q = Sql::select(&["id", "name"])
//!     .from("users")
//!     .order_by("id", Order::Desc)
//!     .limit(25);
//!
//! if let Some(pat) = pattern {
//!     q.and_where("name ILIKE $", (pat,));
//! }
//!
//! let rows: Vec<(i32, String)> = q.fetch_all(pool).await?;
//! ```
//!
//! ## Placeholder model
//!
//! `and_where` fragments use `$` as a positional placeholder; each
//! `$` consumes one value from the bind tuple, in order. The builder
//! rewrites `$` to `$1`, `$2`, … at render time so callers don't
//! manage parameter numbering. Mismatch between `$` count and bind
//! count panics at execute time.
//!
//! Every `and_where` fragment is wrapped in `(...)` before joining
//! with `AND`. Without that, `name ILIKE $ OR email ILIKE $`
//! combined with a previous and-clause would silently parse as
//! `prev AND name ILIKE $ OR email ILIKE $` (wrong precedence).

mod binds;
mod select;
mod update;
mod upsert;
mod where_clause;

pub use binds::IntoBinds;
pub use select::Sql;
pub use update::Update;
pub use upsert::Upsert;

#[doc(hidden)]
pub use binds::BindOne;

#[derive(Debug, Clone, Copy)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    fn keyword(self) -> &'static str {
        match self {
            Order::Asc => " ASC",
            Order::Desc => " DESC",
        }
    }
}
