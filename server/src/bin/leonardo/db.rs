//! `leonardo db <SQL>` — run an arbitrary SQL statement against the
//! configured Leonardo MySQL and print the result as a plain ASCII
//! table. Uses the same sqlx pool the rest of the binary uses, so
//! there's no host-side `mysql` client to install.
//!
//! Examples:
//!   leonardo db 'SHOW TABLES'
//!   leonardo db 'SELECT pilotID, FirstName FROM leonardo_pilots LIMIT 5'
//!
//! Values are rendered through MySQL's own `CAST(... AS CHAR)` so we
//! don't have to enumerate every column type sqlx might hand back —
//! the server gives us a string and we print it. NULLs come back as
//! SQL `NULL` and are printed verbatim.

use anyhow::{Context, anyhow};
use sqlx::{Column, Row};

use super::shared::connect_mysql_pool;

pub async fn run(sql: String) -> anyhow::Result<()> {
    let pool = connect_mysql_pool().await?;

    let rows = sqlx::query(&sql)
        .fetch_all(&pool)
        .await
        .with_context(|| format!("running query: {sql}"))?;

    if rows.is_empty() {
        println!("(0 rows)");
        return Ok(());
    }

    let columns: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_owned())
        .collect();

    let mut table: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut out = Vec::with_capacity(columns.len());
        for (i, _) in columns.iter().enumerate() {
            out.push(decode_cell(row, i, &columns[i])?);
        }
        table.push(out);
    }

    print_table(&columns, &table);
    println!(
        "({} row{})",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

/// Read a single cell as a printable string. sqlx-mysql is strict
/// about decode types, so we cycle through a few candidates and
/// return the first that succeeds:
///
/// 1. `String` — works for char/varchar/text and most metadata
///    columns. Handles UTF-8 transparently.
/// 2. `i64` / `u64` / `f64` — numeric scalars come back as their
///    Rust counterpart; we render them with the default `Display`.
///    Note we *don't* try to read `DECIMAL` as `f64`: sqlx refuses
///    that conversion (lossy), so DECIMALs go to step 3.
/// 3. `Vec<u8>` — the universal fallback. Many MySQL types
///    (BLOB, VARBINARY, DECIMAL, JSON, dates in some configs) come
///    out as raw bytes regardless of declared type. We unwrap to
///    UTF-8 when valid and `<n bytes>` otherwise so the table
///    doesn't get mangled by stray binary.
fn decode_cell(row: &sqlx::mysql::MySqlRow, i: usize, name: &str) -> anyhow::Result<String> {
    if let Ok(v) = row.try_get::<Option<String>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<u64>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<sqlx::types::BigDecimal>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveDateTime>, _>(i) {
        return Ok(option_to_string(v));
    }
    if let Ok(v) = row.try_get::<Option<chrono::NaiveDate>, _>(i) {
        return Ok(option_to_string(v));
    }

    let bytes: Option<Vec<u8>> = row
        .try_get(i)
        .map_err(|e| anyhow!("decoding column {name:?}: {e}"))?;
    Ok(match bytes {
        None => "NULL".to_owned(),
        Some(b) => match std::str::from_utf8(&b) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("<{} bytes>", b.len()),
        },
    })
}

fn option_to_string<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "NULL".to_owned())
}

fn print_table(headers: &[String], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    print_row(headers, &widths);
    println!(
        "{}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("-+-")
    );
    for row in rows {
        print_row(row, &widths);
    }
}

fn print_row(cells: &[String], widths: &[usize]) {
    let line = cells
        .iter()
        .zip(widths)
        .map(|(c, w)| format!("{c:<w$}", c = c, w = *w))
        .collect::<Vec<_>>()
        .join(" | ");
    println!("{line}");
}
