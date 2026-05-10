//! Tiny SQL builder layered on top of `sqlx::QueryBuilder`. Turns
//! the "string concatenation with manual `WHERE`/`AND` bookkeeping"
//! pattern into a fluent API while staying inside the sqlx ecosystem
//! (no ORM, no separate query AST).
//!
//! # Example
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

use sqlx::{
    Encode, Postgres, QueryBuilder, Type,
    postgres::{PgPool, PgRow},
};

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

pub struct Sql<'a> {
    select_cols: Vec<&'a str>,
    from: Option<&'a str>,
    joins: Vec<Join<'a>>,
    wheres: Vec<WherePart<'a>>,
    order_by: Vec<(&'a str, Order)>,
    limit: Option<i64>,
}

struct WherePart<'a> {
    fragment: &'a str,
    binds: Vec<Box<dyn BindOne<'a> + Send + 'a>>,
}

struct Join<'a> {
    kind: JoinKind,
    /// Right-hand side of the join, alias included (e.g. `"users u"`).
    table: &'a str,
    /// Raw `ON` predicate. No `$` binds — bound parameters in joins
    /// are vanishingly rare in this codebase; lift them into a
    /// `WHERE` if they're ever needed for an `INNER JOIN`.
    on: &'a str,
}

#[derive(Debug, Clone, Copy)]
enum JoinKind {
    Inner,
    Left,
}

impl JoinKind {
    fn keyword(self) -> &'static str {
        match self {
            JoinKind::Inner => " JOIN ",
            JoinKind::Left => " LEFT JOIN ",
        }
    }
}

/// Object-safe shim around the `Encode + Type` trait pair so we can
/// store heterogeneous binds in one `Vec`. `pub` only because it
/// leaks into [`IntoBinds`]'s return type; not part of the public
/// API.
#[doc(hidden)]
pub trait BindOne<'a>: Send {
    fn push(self: Box<Self>, qb: &mut QueryBuilder<'a, Postgres>);
}

impl<'a, T> BindOne<'a> for T
where
    T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
{
    fn push(self: Box<Self>, qb: &mut QueryBuilder<'a, Postgres>) {
        qb.push_bind(*self);
    }
}

/// Tuples up to 8 implement this. `$` placeholders in the fragment
/// pull from the tuple in declaration order.
pub trait IntoBinds<'a> {
    fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>>;
}

impl<'a> IntoBinds<'a> for () {
    fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>> {
        Vec::new()
    }
}

macro_rules! impl_into_binds {
    ($($T:ident),+ $(,)?) => {
        impl<'a, $($T,)+> IntoBinds<'a> for ($($T,)+)
        where
            $($T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,)+
        {
            fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>> {
                #[allow(non_snake_case)]
                let ($($T,)+) = self;
                let v: Vec<Box<dyn BindOne<'a> + Send + 'a>> =
                    vec![ $( Box::new($T) as Box<dyn BindOne<'a> + Send + 'a>, )+ ];
                v
            }
        }
    };
}

impl_into_binds!(T1);
impl_into_binds!(T1, T2);
impl_into_binds!(T1, T2, T3);
impl_into_binds!(T1, T2, T3, T4);
impl_into_binds!(T1, T2, T3, T4, T5);
impl_into_binds!(T1, T2, T3, T4, T5, T6);
impl_into_binds!(T1, T2, T3, T4, T5, T6, T7);
impl_into_binds!(T1, T2, T3, T4, T5, T6, T7, T8);

impl<'a> Sql<'a> {
    pub fn select(cols: &[&'a str]) -> Self {
        Self {
            select_cols: cols.to_vec(),
            from: None,
            joins: Vec::new(),
            wheres: Vec::new(),
            order_by: Vec::new(),
            limit: None,
        }
    }

    pub fn from(mut self, table: &'a str) -> Self {
        self.from = Some(table);
        self
    }

    pub fn join(mut self, table: &'a str, on: &'a str) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Inner,
            table,
            on,
        });
        self
    }

    pub fn left_join(mut self, table: &'a str, on: &'a str) -> Self {
        self.joins.push(Join {
            kind: JoinKind::Left,
            table,
            on,
        });
        self
    }

    pub fn order_by(mut self, col: &'a str, dir: Order) -> Self {
        self.order_by.push((col, dir));
        self
    }

    pub fn limit(mut self, n: impl Into<i64>) -> Self {
        self.limit = Some(n.into());
        self
    }

    pub fn and_where<B: IntoBinds<'a>>(&mut self, fragment: &'a str, binds: B) -> &mut Self {
        self.wheres.push(WherePart {
            fragment,
            binds: binds.into_binds(),
        });
        self
    }

    /// Render the SQL string only — for tests, `tracing` lines, and
    /// debugging. Consumes `self` because binds are boxed trait
    /// objects and can't be cloned through `BindOne`; the SQL is
    /// produced by the same `QueryBuilder` path the executor uses.
    pub fn to_sql(self) -> String {
        self.into_query_builder().into_sql()
    }

    /// Build a `sqlx::QueryBuilder` with all SQL written and all
    /// values bound, ready to `.build_query_as::<T>()`.
    fn into_query_builder(self) -> QueryBuilder<'a, Postgres> {
        let mut qb: QueryBuilder<'a, Postgres> = QueryBuilder::new("SELECT ");
        for (i, c) in self.select_cols.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(*c);
        }

        if let Some(table) = self.from {
            qb.push(" FROM ");
            qb.push(table);
        }

        for j in &self.joins {
            qb.push(j.kind.keyword());
            qb.push(j.table);
            qb.push(" ON ");
            qb.push(j.on);
        }

        for (i, w) in self.wheres.into_iter().enumerate() {
            qb.push(if i == 0 { " WHERE " } else { " AND " });
            qb.push("(");
            let parts: Vec<&str> = w.fragment.split('$').collect();
            assert_eq!(
                parts.len() - 1,
                w.binds.len(),
                "and_where: {} `$` placeholders but {} bind values: {:?}",
                parts.len() - 1,
                w.binds.len(),
                w.fragment,
            );
            qb.push(parts[0]);
            let mut binds = w.binds.into_iter();
            for part in &parts[1..] {
                let bind = binds.next().expect("counted above");
                bind.push(&mut qb);
                qb.push(*part);
            }
            qb.push(")");
        }

        if !self.order_by.is_empty() {
            qb.push(" ORDER BY ");
            for (i, (col, dir)) in self.order_by.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                qb.push(*col);
                qb.push(dir.keyword());
            }
        }

        if let Some(n) = self.limit {
            qb.push(" LIMIT ");
            qb.push_bind(n);
        }

        qb
    }

    pub async fn fetch_all<T>(self, pool: &PgPool) -> Result<Vec<T>, sqlx::Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, PgRow>,
    {
        let mut qb = self.into_query_builder();
        qb.build_query_as::<T>().fetch_all(pool).await
    }

    pub async fn fetch_optional<T>(self, pool: &PgPool) -> Result<Option<T>, sqlx::Error>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, PgRow>,
    {
        let mut qb = self.into_query_builder();
        qb.build_query_as::<T>().fetch_optional(pool).await
    }
}

// =============================================================================
// UPDATE builder
// =============================================================================
//
// Sibling of [`Sql`] that handles the "set this column conditionally, then
// write to one row" pattern that grew out of the partial-update endpoints
// (`PATCH /users/me` etc.). UPDATE shares only its WHERE shape with SELECT,
// so it gets its own struct rather than an enum-tagged variant of `Sql`.

/// Fluent UPDATE builder. Each [`set`](Self::set) call appends one
/// `col = $N` to the SET list, in declaration order; `$N` placeholder
/// numbering is assigned at render time.
///
/// # Example
///
/// ```ignore
/// let mut q = Update::new("users");
/// if let Some(name) = new_name { q.set("name", name); }
/// q.and_where("id = $", (user_id,));
/// q.execute(pool).await?;
/// ```
pub struct Update<'a> {
    table: &'a str,
    sets: Vec<SetPart<'a>>,
    wheres: Vec<WherePart<'a>>,
}

struct SetPart<'a> {
    col: &'a str,
    bind: Box<dyn BindOne<'a> + Send + 'a>,
}

impl<'a> Update<'a> {
    pub fn new(table: &'a str) -> Self {
        Self {
            table,
            sets: Vec::new(),
            wheres: Vec::new(),
        }
    }

    /// Append `col = $N` to the SET list. The value is bound at render
    /// time; placeholder numbering walks SET first, then WHERE.
    pub fn set<T>(&mut self, col: &'a str, value: T) -> &mut Self
    where
        T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
    {
        self.sets.push(SetPart {
            col,
            bind: Box::new(value),
        });
        self
    }

    /// Same shape as [`Sql::and_where`]: positional `$` placeholders,
    /// fragment wrapped in parens, multiple calls join with `AND`.
    pub fn and_where<B: IntoBinds<'a>>(&mut self, fragment: &'a str, binds: B) -> &mut Self {
        self.wheres.push(WherePart {
            fragment,
            binds: binds.into_binds(),
        });
        self
    }

    /// Render the SQL string only — for tests and debugging. Same
    /// caveat as [`Sql::to_sql`]: consumes `self` because binds are
    /// boxed trait objects.
    pub fn to_sql(self) -> String {
        self.into_query_builder().into_sql()
    }

    /// Run the UPDATE against a pool.
    ///
    /// Generic executor doesn't fit here: sqlx's `QueryBuilder::build`
    /// returns `Query<'args, …>` (not a narrowed borrow), so the bind
    /// lifetime can't shrink to match an arbitrary `'e` on the
    /// executor side. Two concrete entry points sidestep that.
    pub async fn execute(
        self,
        pool: &PgPool,
    ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        let mut qb = self.into_query_builder();
        qb.build().execute(pool).await
    }

    /// Run the UPDATE on a `Transaction` connection. Pass the
    /// transaction directly; the dereference to `&mut PgConnection`
    /// happens here so call sites read as `q.execute_tx(tx).await`.
    pub async fn execute_tx(
        self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
    ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        let mut qb = self.into_query_builder();
        qb.build().execute(&mut **tx).await
    }

    fn into_query_builder(self) -> QueryBuilder<'a, Postgres> {
        assert!(
            !self.sets.is_empty(),
            "Update::execute called with no SET clauses (would render \
             `UPDATE {} WHERE …` and Postgres would reject it). Guard \
             with an is-noop check at the call site.",
            self.table,
        );

        let mut qb: QueryBuilder<'a, Postgres> = QueryBuilder::new("UPDATE ");
        qb.push(self.table);
        qb.push(" SET ");
        for (i, part) in self.sets.into_iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(part.col);
            qb.push(" = ");
            part.bind.push(&mut qb);
        }

        push_wheres(&mut qb, self.wheres);
        qb
    }
}

// Render a vec of `WherePart`s into an existing builder. Pulled out
// because both `Sql` and `Update` need the same logic (paren-wrap
// each fragment, AND between, expand `$` to numbered placeholders).
fn push_wheres<'a>(qb: &mut QueryBuilder<'a, Postgres>, wheres: Vec<WherePart<'a>>) {
    for (i, w) in wheres.into_iter().enumerate() {
        qb.push(if i == 0 { " WHERE " } else { " AND " });
        qb.push("(");
        let parts: Vec<&str> = w.fragment.split('$').collect();
        assert_eq!(
            parts.len() - 1,
            w.binds.len(),
            "and_where: {} `$` placeholders but {} bind values: {:?}",
            parts.len() - 1,
            w.binds.len(),
            w.fragment,
        );
        qb.push(parts[0]);
        let mut binds = w.binds.into_iter();
        for part in &parts[1..] {
            let bind = binds.next().expect("counted above");
            bind.push(qb);
            qb.push(*part);
        }
        qb.push(")");
    }
}

// =============================================================================
// UPSERT builder
// =============================================================================
//
// Models `INSERT … ON CONFLICT (key) DO UPDATE SET col = EXCLUDED.col, …`,
// the only upsert flavour the codebase uses today. Conflict targets a
// single column; conflict action is always DO UPDATE with EXCLUDED-only
// assignments. Add `do_nothing()` / multi-column conflict / arbitrary
// SET expressions when a real call site asks for them.

/// Fluent UPSERT builder. INSERT columns are added with [`value`](Self::value)
/// (or [`value_cast`](Self::value_cast) when a Postgres cast is required,
/// e.g. enum types); the conflict key with [`on_conflict`](Self::on_conflict);
/// the SET list on conflict with [`update_excluded`](Self::update_excluded).
///
/// # Example
///
/// ```ignore
/// let mut q = Upsert::into("user_profiles");
/// q.value("user_id", user_id);
/// q.value("country", country);
/// q.value_cast("sex", sex_str, "user_sex");
/// q.on_conflict("user_id");
/// if has_country { q.update_excluded("country"); }
/// if has_sex     { q.update_excluded("sex"); }
/// q.execute_tx(tx).await?;
/// ```
pub struct Upsert<'a> {
    table: &'a str,
    columns: Vec<UpsertColumn<'a>>,
    /// Single conflict-target column. Compound keys would be
    /// `Vec<&'a str>`; punted until needed.
    conflict: Option<&'a str>,
    update_cols: Vec<&'a str>,
}

struct UpsertColumn<'a> {
    name: &'a str,
    bind: Box<dyn BindOne<'a> + Send + 'a>,
    /// Postgres type to cast the placeholder to (`$N::cast`). Only
    /// needed for enum types where the driver can't infer the column
    /// type from the bound `&str`.
    cast: Option<&'a str>,
}

impl<'a> Upsert<'a> {
    pub fn into(table: &'a str) -> Self {
        Self {
            table,
            columns: Vec::new(),
            conflict: None,
            update_cols: Vec::new(),
        }
    }

    /// Append `(col, $N)` to the INSERT shape. Order matters — the
    /// rendered column list and VALUES list walk in declaration order.
    pub fn value<T>(&mut self, col: &'a str, v: T) -> &mut Self
    where
        T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
    {
        self.columns.push(UpsertColumn {
            name: col,
            bind: Box::new(v),
            cast: None,
        });
        self
    }

    /// Like [`value`](Self::value) but emits `$N::cast` so Postgres
    /// can resolve enum / domain columns from a generic bind type.
    pub fn value_cast<T>(&mut self, col: &'a str, v: T, cast: &'a str) -> &mut Self
    where
        T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
    {
        self.columns.push(UpsertColumn {
            name: col,
            bind: Box::new(v),
            cast: Some(cast),
        });
        self
    }

    /// Set the `ON CONFLICT (col)` target. Required.
    pub fn on_conflict(&mut self, col: &'a str) -> &mut Self {
        self.conflict = Some(col);
        self
    }

    /// Append `col = EXCLUDED.col` to the conflict-update SET list.
    /// Repeatable; declaration order is preserved.
    pub fn update_excluded(&mut self, col: &'a str) -> &mut Self {
        self.update_cols.push(col);
        self
    }

    pub fn to_sql(self) -> String {
        self.into_query_builder().into_sql()
    }

    pub async fn execute(
        self,
        pool: &PgPool,
    ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        let mut qb = self.into_query_builder();
        qb.build().execute(pool).await
    }

    pub async fn execute_tx(
        self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
    ) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
        let mut qb = self.into_query_builder();
        qb.build().execute(&mut **tx).await
    }

    fn into_query_builder(self) -> QueryBuilder<'a, Postgres> {
        assert!(
            !self.columns.is_empty(),
            "Upsert::execute called with no values for {} (would render \
             an empty INSERT and Postgres would reject it).",
            self.table,
        );
        let conflict = self.conflict.unwrap_or_else(|| {
            panic!(
                "Upsert::execute called without on_conflict() for {}. \
                 If you wanted a plain INSERT, this builder isn't the \
                 right tool yet.",
                self.table,
            )
        });
        assert!(
            !self.update_cols.is_empty(),
            "Upsert::execute called with no update_excluded() columns \
             for {}. DO NOTHING isn't supported yet — add an explicit \
             `do_nothing()` method when a use site needs it.",
            self.table,
        );

        let mut qb: QueryBuilder<'a, Postgres> = QueryBuilder::new("INSERT INTO ");
        qb.push(self.table);
        qb.push(" (");
        for (i, c) in self.columns.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(c.name);
        }
        qb.push(") VALUES (");
        for (i, c) in self.columns.into_iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            c.bind.push(&mut qb);
            if let Some(cast) = c.cast {
                qb.push("::");
                qb.push(cast);
            }
        }
        qb.push(") ON CONFLICT (");
        qb.push(conflict);
        qb.push(") DO UPDATE SET ");
        for (i, col) in self.update_cols.into_iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(col);
            qb.push(" = EXCLUDED.");
            qb.push(col);
        }

        qb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_select_from_only() {
        let q = Sql::select(&["id", "name"]).from("users");
        assert_eq!(q.to_sql(), "SELECT id, name FROM users");
    }

    #[test]
    fn renders_with_order_and_limit() {
        let q = Sql::select(&["id"])
            .from("users")
            .order_by("created_at", Order::Desc)
            .order_by("id", Order::Desc)
            .limit(25_i64);
        assert_eq!(
            q.to_sql(),
            "SELECT id FROM users ORDER BY created_at DESC, id DESC LIMIT $1",
        );
    }

    #[test]
    fn renders_single_where_with_parens() {
        let mut q = Sql::select(&["id"]).from("users");
        q.and_where("id = $", (1_i32,));
        assert_eq!(q.to_sql(), "SELECT id FROM users WHERE (id = $1)");
    }

    #[test]
    fn multiple_wheres_join_with_and_each_wrapped() {
        let mut q = Sql::select(&["id"]).from("users");
        q.and_where("id = $", (1_i32,));
        q.and_where("name ILIKE $ OR email ILIKE $", ("a", "b"));
        assert_eq!(
            q.to_sql(),
            "SELECT id FROM users WHERE (id = $1) AND (name ILIKE $2 OR email ILIKE $3)",
        );
    }

    #[test]
    fn renumbers_placeholders_across_clauses() {
        let mut q = Sql::select(&["id"]).from("users");
        q.and_where("(a, b) < ($, $)", (1_i64, 2_i32));
        q.and_where("c = $", (3_i32,));
        assert_eq!(
            q.to_sql(),
            "SELECT id FROM users WHERE ((a, b) < ($1, $2)) AND (c = $3)",
        );
    }

    #[test]
    fn renders_inner_join() {
        let q = Sql::select(&["f.id", "u.name"])
            .from("flights f")
            .join("users u", "u.id = f.user_id");
        assert_eq!(
            q.to_sql(),
            "SELECT f.id, u.name FROM flights f JOIN users u ON u.id = f.user_id",
        );
    }

    #[test]
    fn renders_left_join() {
        let q = Sql::select(&["u.id", "p.country"])
            .from("users u")
            .left_join("user_profiles p", "p.user_id = u.id");
        assert_eq!(
            q.to_sql(),
            "SELECT u.id, p.country FROM users u LEFT JOIN user_profiles p ON p.user_id = u.id",
        );
    }

    #[test]
    fn joins_render_in_declaration_order_before_where() {
        let mut q = Sql::select(&["f.id"])
            .from("flights f")
            .join("users u", "u.id = f.user_id")
            .left_join("user_profiles p", "p.user_id = u.id");
        q.and_where("f.id = $", (7_i32,));
        assert_eq!(
            q.to_sql(),
            "SELECT f.id FROM flights f \
             JOIN users u ON u.id = f.user_id \
             LEFT JOIN user_profiles p ON p.user_id = u.id \
             WHERE (f.id = $1)",
        );
    }

    #[test]
    #[should_panic(expected = "and_where")]
    fn mismatched_placeholders_panic() {
        let mut q = Sql::select(&["id"]).from("users");
        q.and_where("a = $ AND b = $", (1_i32,));
        let _ = q.to_sql();
    }

    // ----- Update --------------------------------------------------------

    #[test]
    fn update_renders_single_set() {
        let mut q = Update::new("users");
        q.set("name", "Alice");
        q.and_where("id = $", (7_i32,));
        assert_eq!(q.to_sql(), "UPDATE users SET name = $1 WHERE (id = $2)",);
    }

    #[test]
    fn update_renders_multiple_sets_in_declaration_order() {
        let mut q = Update::new("user_preferences");
        q.set("time_format", "h12");
        q.set("units", "metric");
        q.and_where("user_id = $", (1_i32,));
        assert_eq!(
            q.to_sql(),
            "UPDATE user_preferences \
             SET time_format = $1, units = $2 \
             WHERE (user_id = $3)",
        );
    }

    #[test]
    fn update_supports_multiple_wheres_each_wrapped() {
        let mut q = Update::new("flights");
        q.set("status", "archived");
        q.and_where("user_id = $", (1_i32,));
        q.and_where("created_at < $", (1_700_000_000_i64,));
        assert_eq!(
            q.to_sql(),
            "UPDATE flights SET status = $1 WHERE (user_id = $2) AND (created_at < $3)",
        );
    }

    #[test]
    #[should_panic(expected = "no SET clauses")]
    fn update_with_empty_sets_panics() {
        let mut q = Update::new("users");
        q.and_where("id = $", (1_i32,));
        let _ = q.to_sql();
    }

    // ----- Upsert --------------------------------------------------------

    #[test]
    fn upsert_renders_single_value() {
        let mut q = Upsert::into("users");
        q.value("id", 1_i32);
        q.on_conflict("id");
        q.update_excluded("id");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO users (id) VALUES ($1) \
             ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id",
        );
    }

    #[test]
    fn upsert_renders_multiple_values_and_updates_in_order() {
        let mut q = Upsert::into("user_profiles");
        q.value("user_id", 7_i32);
        q.value("country", Some("KZ"));
        q.on_conflict("user_id");
        q.update_excluded("country");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO user_profiles (user_id, country) VALUES ($1, $2) \
             ON CONFLICT (user_id) DO UPDATE SET country = EXCLUDED.country",
        );
    }

    #[test]
    fn upsert_emits_cast_on_value_cast() {
        let mut q = Upsert::into("user_profiles");
        q.value("user_id", 1_i32);
        q.value_cast("sex", Some("male"), "user_sex");
        q.on_conflict("user_id");
        q.update_excluded("sex");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO user_profiles (user_id, sex) VALUES ($1, $2::user_sex) \
             ON CONFLICT (user_id) DO UPDATE SET sex = EXCLUDED.sex",
        );
    }

    #[test]
    fn upsert_renders_multiple_update_cols() {
        let mut q = Upsert::into("user_profiles");
        q.value("user_id", 1_i32);
        q.value("civl_id", Some(42_i32));
        q.value("country", Some("FR"));
        q.value_cast("sex", Some("female"), "user_sex");
        q.on_conflict("user_id");
        q.update_excluded("civl_id");
        q.update_excluded("country");
        q.update_excluded("sex");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO user_profiles (user_id, civl_id, country, sex) \
             VALUES ($1, $2, $3, $4::user_sex) \
             ON CONFLICT (user_id) DO UPDATE SET \
             civl_id = EXCLUDED.civl_id, \
             country = EXCLUDED.country, \
             sex = EXCLUDED.sex",
        );
    }

    #[test]
    #[should_panic(expected = "no values")]
    fn upsert_without_values_panics() {
        let mut q = Upsert::into("users");
        q.on_conflict("id");
        q.update_excluded("id");
        let _ = q.to_sql();
    }

    #[test]
    #[should_panic(expected = "without on_conflict")]
    fn upsert_without_on_conflict_panics() {
        let mut q = Upsert::into("users");
        q.value("id", 1_i32);
        q.update_excluded("id");
        let _ = q.to_sql();
    }

    #[test]
    #[should_panic(expected = "no update_excluded")]
    fn upsert_without_update_cols_panics() {
        let mut q = Upsert::into("users");
        q.value("id", 1_i32);
        q.on_conflict("id");
        let _ = q.to_sql();
    }
}
