//! `SELECT … FROM … [JOIN …] WHERE … ORDER BY … LIMIT …` builder.
//! `Join` / `JoinKind` only matter to SELECT, so they live here
//! rather than in the shared scope.

use sqlx::{
    Postgres, QueryBuilder,
    postgres::{PgPool, PgRow},
};

use super::Order;
use super::binds::IntoBinds;
use super::where_clause::{WherePart, push_wheres};

pub struct Sql<'a> {
    select_cols: Vec<&'a str>,
    from: Option<&'a str>,
    joins: Vec<Join<'a>>,
    wheres: Vec<WherePart<'a>>,
    order_by: Vec<(&'a str, Order)>,
    limit: Option<i64>,
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

        push_wheres(&mut qb, self.wheres);

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
}
