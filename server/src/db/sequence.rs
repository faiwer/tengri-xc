use anyhow::{Context, ensure};

pub async fn advance_identity_sequence(
    pool: &sqlx::PgPool,
    table: &str,
    column: &str,
) -> anyhow::Result<()> {
    ensure_identifier(table)?;
    ensure_identifier(column)?;

    let sql = format!(
        "SELECT setval( \
             pg_get_serial_sequence($1, $2), \
             GREATEST((SELECT COALESCE(MAX({column}), 0) FROM {table}), 1), \
             true \
         )",
    );
    sqlx::query(&sql)
        .bind(table)
        .bind(column)
        .execute(pool)
        .await
        .with_context(|| format!("advancing {table}.{column} sequence"))?;
    Ok(())
}

fn ensure_identifier(identifier: &str) -> anyhow::Result<()> {
    ensure!(!identifier.is_empty(), "identifier must not be empty");
    ensure!(
        identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "identifier {identifier:?} must be ASCII alphanumeric or underscore"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_identifier() {
        ensure_identifier("users_2026").unwrap();
    }

    #[test]
    fn rejects_empty_identifier() {
        assert!(ensure_identifier("").is_err());
    }

    #[test]
    fn rejects_non_identifier_text() {
        assert!(ensure_identifier("users; DROP TABLE users").is_err());
    }
}
