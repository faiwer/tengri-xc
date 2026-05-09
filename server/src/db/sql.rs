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
    wheres: Vec<WherePart<'a>>,
    order_by: Vec<(&'a str, Order)>,
    limit: Option<i64>,
}

struct WherePart<'a> {
    fragment: &'a str,
    binds: Vec<Box<dyn BindOne<'a> + Send + 'a>>,
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
            wheres: Vec::new(),
            order_by: Vec::new(),
            limit: None,
        }
    }

    pub fn from(mut self, table: &'a str) -> Self {
        self.from = Some(table);
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
    #[should_panic(expected = "and_where")]
    fn mismatched_placeholders_panic() {
        let mut q = Sql::select(&["id"]).from("users");
        q.and_where("a = $ AND b = $", (1_i32,));
        let _ = q.to_sql();
    }
}
