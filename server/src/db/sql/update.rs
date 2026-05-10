//! `UPDATE … SET … WHERE …` builder. Sibling of [`Sql`](super::Sql)
//! that handles the "set this column conditionally, then write to
//! one row" pattern that grew out of the partial-update endpoints
//! (`PATCH /users/me` etc.). UPDATE shares only its WHERE shape with
//! SELECT, so it gets its own struct rather than an enum-tagged
//! variant of `Sql`.

use sqlx::{Encode, Postgres, QueryBuilder, Type, postgres::PgPool};

use super::binds::{BindOne, IntoBinds};
use super::where_clause::{WherePart, push_wheres};

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

    /// Same shape as [`Sql::and_where`](super::Sql::and_where):
    /// positional `$` placeholders, fragment wrapped in parens,
    /// multiple calls join with `AND`.
    pub fn and_where<B: IntoBinds<'a>>(&mut self, fragment: &'a str, binds: B) -> &mut Self {
        self.wheres.push(WherePart {
            fragment,
            binds: binds.into_binds(),
        });
        self
    }

    /// Render the SQL string only — for tests and debugging. Same
    /// caveat as [`Sql::to_sql`](super::Sql::to_sql): consumes
    /// `self` because binds are boxed trait objects.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_single_set() {
        let mut q = Update::new("users");
        q.set("name", "Alice");
        q.and_where("id = $", (7_i32,));
        assert_eq!(q.to_sql(), "UPDATE users SET name = $1 WHERE (id = $2)");
    }

    #[test]
    fn renders_multiple_sets_in_declaration_order() {
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
    fn supports_multiple_wheres_each_wrapped() {
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
    fn empty_sets_panics() {
        let mut q = Update::new("users");
        q.and_where("id = $", (1_i32,));
        let _ = q.to_sql();
    }
}
