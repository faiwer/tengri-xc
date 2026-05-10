//! `INSERT … ON CONFLICT (key) DO UPDATE SET col = EXCLUDED.col, …`
//! builder — the only upsert flavour the codebase uses today.
//! Conflict targets a single column; the conflict action is always
//! DO UPDATE with EXCLUDED-only assignments. Add `do_nothing()` /
//! multi-column conflict / arbitrary SET expressions when a real
//! call site asks for them.

use sqlx::{Encode, Postgres, QueryBuilder, Type, postgres::PgPool};

use super::binds::BindOne;

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
    fn renders_single_value() {
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
    fn renders_multiple_values_and_updates_in_order() {
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
    fn emits_cast_on_value_cast() {
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
    fn renders_multiple_update_cols() {
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
    fn without_values_panics() {
        let mut q = Upsert::into("users");
        q.on_conflict("id");
        q.update_excluded("id");
        let _ = q.to_sql();
    }

    #[test]
    #[should_panic(expected = "without on_conflict")]
    fn without_on_conflict_panics() {
        let mut q = Upsert::into("users");
        q.value("id", 1_i32);
        q.update_excluded("id");
        let _ = q.to_sql();
    }

    #[test]
    #[should_panic(expected = "no update_excluded")]
    fn without_update_cols_panics() {
        let mut q = Upsert::into("users");
        q.value("id", 1_i32);
        q.on_conflict("id");
        let _ = q.to_sql();
    }
}
