//! `INSERT INTO … VALUES … [RETURNING …]` builder. Use this for strict
//! creates where a conflict should stay a database error.

use sqlx::{Decode, Encode, Postgres, QueryBuilder, Type, postgres::PgPool};

use super::binds::BindOne;

pub struct Insert<'a> {
    table: &'a str,
    columns: Vec<InsertColumn<'a>>,
    conflict_do_nothing: Option<&'a str>,
    returning: Vec<&'a str>,
}

struct InsertColumn<'a> {
    name: &'a str,
    bind: Box<dyn BindOne<'a> + Send + 'a>,
}

impl<'a> Insert<'a> {
    pub fn into(table: &'a str) -> Self {
        Self {
            table,
            columns: Vec::new(),
            conflict_do_nothing: None,
            returning: Vec::new(),
        }
    }

    pub fn value<T>(&mut self, col: &'a str, value: T) -> &mut Self
    where
        T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
    {
        self.columns.push(InsertColumn {
            name: col,
            bind: Box::new(value),
        });
        self
    }

    pub fn on_conflict_do_nothing(&mut self, col: &'a str) -> &mut Self {
        self.conflict_do_nothing = Some(col);
        self
    }

    pub fn returning(&mut self, col: &'a str) -> &mut Self {
        self.returning.push(col);
        self
    }

    pub fn to_sql(self) -> String {
        self.into_query_builder().into_sql()
    }

    pub async fn fetch_one_scalar<T>(self, pool: &PgPool) -> Result<T, sqlx::Error>
    where
        T: Send + Unpin + for<'r> Decode<'r, Postgres> + Type<Postgres>,
    {
        let mut qb = self.into_query_builder();
        qb.build_query_scalar::<T>().fetch_one(pool).await
    }

    pub async fn fetch_optional_scalar<T>(self, pool: &PgPool) -> Result<Option<T>, sqlx::Error>
    where
        T: Send + Unpin + for<'r> Decode<'r, Postgres> + Type<Postgres>,
    {
        let mut qb = self.into_query_builder();
        qb.build_query_scalar::<T>().fetch_optional(pool).await
    }

    fn into_query_builder(self) -> QueryBuilder<'a, Postgres> {
        assert!(
            !self.columns.is_empty(),
            "Insert::execute called with no values for {} (would render \
             an empty INSERT and Postgres would reject it).",
            self.table,
        );

        let mut qb: QueryBuilder<'a, Postgres> = QueryBuilder::new("INSERT INTO ");
        qb.push(self.table);
        qb.push(" (");
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(col.name);
        }
        qb.push(") VALUES (");
        for (i, col) in self.columns.into_iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            col.bind.push(&mut qb);
        }
        qb.push(")");

        if let Some(col) = self.conflict_do_nothing {
            qb.push(" ON CONFLICT (");
            qb.push(col);
            qb.push(") DO NOTHING");
        }

        if !self.returning.is_empty() {
            qb.push(" RETURNING ");
            for (i, col) in self.returning.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                qb.push(*col);
            }
        }

        qb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_values_in_declaration_order() {
        let mut q = Insert::into("users");
        q.value("name", "Alice");
        q.value("permissions", 1_i32);
        q.returning("id");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO users (name, permissions) VALUES ($1, $2) RETURNING id",
        );
    }

    #[test]
    fn renders_do_nothing_conflict() {
        let mut q = Insert::into("users");
        q.value("id", 7_i32);
        q.value("name", "Alice");
        q.on_conflict_do_nothing("id");
        q.returning("id");
        assert_eq!(
            q.to_sql(),
            "INSERT INTO users (id, name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING RETURNING id",
        );
    }

    #[test]
    #[should_panic(expected = "no values")]
    fn without_values_panics() {
        let mut q = Insert::into("users");
        q.returning("id");
        let _ = q.to_sql();
    }
}
