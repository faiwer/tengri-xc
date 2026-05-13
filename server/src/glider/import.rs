//! Load brands + canonical glider models into the DB from a caller-supplied
//! `hg.json` or `pg.json` payload. UPSERT-based so re-runs pick up edits;
//! `class` / `is_tandem` changes on existing models fan out to every dependent
//! `gliders` row via the `sync_glider_denorm` trigger from migration
//! `0009_gliders.sql`. The whole thing runs in one transaction — a parse error
//! or constraint trip rolls everything back.
//!
//! One call handles one kind. IO is the caller's job; this module only sees
//! the JSON string.
//!
//! Brand and model PKs are slugs derived from the display name (`slugify`).
//! The JSON key/value pair is treated as `(display, [models])`; we slugify on
//! the way in and store `(id=slug, name=display)`.

use std::collections::HashSet;

use anyhow::{Context, anyhow};
use sqlx::{PgPool, Row};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Marker appended to a model name in the JSON to flag tandem variants (e.g.
/// `Atos VR 190¶tandem`). Stripped during parse; the boolean lands in
/// `glider_models.is_tandem`.
const TANDEM_SUFFIX: &str = "¶tandem";

pub struct Summary {
    pub brands_total: usize,
    pub brands_new: usize,
    pub brands_updated: usize,
    pub models_total: usize,
    pub models_new: usize,
    pub models_updated: usize,
    pub models_tandem: usize,
}

pub async fn run(pool: &PgPool, json: &str, kind: &str) -> anyhow::Result<Summary> {
    let kind: &'static str = match kind {
        "hg" => "hg",
        "pg" => "pg",
        "sp" => "sp",
        other => {
            return Err(anyhow!(
                "unknown glider kind `{other}` (expected `hg`, `pg`, or `sp`)"
            ));
        }
    };
    let records = parse_file(json, kind).with_context(|| format!("parsing {kind} JSON"))?;
    check_no_duplicates(&records)?;
    apply(pool, records).await
}

#[derive(Debug)]
struct ModelRecord {
    brand_id: String,
    brand_name: String,
    model_id: String,
    name: String,
    kind: &'static str,
    class: &'static str,
    is_tandem: bool,
}

/// `glider_models` has `PRIMARY KEY (brand_id, id)` and `UNIQUE (brand_id, name)`.
/// Catch dictionary duplicates here so the error names them up-front instead of
/// letting the UPSERT silently overwrite whichever it hits second. We check both
/// display-name and slug collisions: two models in the same brand whose names
/// differ only in diacritics would crash the slug PK at INSERT time, so we'd
/// rather fail in the parser with a useful message.
fn check_no_duplicates(records: &[ModelRecord]) -> anyhow::Result<()> {
    let mut seen_name: HashSet<(&str, &str)> = HashSet::with_capacity(records.len());
    let mut seen_slug: HashSet<(&str, &str)> = HashSet::with_capacity(records.len());
    for r in records {
        if !seen_name.insert((r.brand_name.as_str(), r.name.as_str())) {
            return Err(anyhow!(
                "duplicate `{brand}` / `{name}` in dictionary (likely listed under multiple class \
                 buckets); pick one and remove the others",
                brand = r.brand_name,
                name = r.name,
            ));
        }
        if !seen_slug.insert((r.brand_id.as_str(), r.model_id.as_str())) {
            return Err(anyhow!(
                "slug collision: `{brand}` / `{name}` → `{bslug}/{mslug}` (another model in \
                 the same brand slugifies to the same id); rename one",
                brand = r.brand_name,
                name = r.name,
                bslug = r.brand_id,
                mslug = r.model_id,
            ));
        }
    }
    Ok(())
}

fn parse_file(src: &str, kind: &'static str) -> anyhow::Result<Vec<ModelRecord>> {
    type File = std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>;
    let parsed: File = serde_json::from_str(src)?;
    let mut out = Vec::new();
    for (brand_name, classes) in parsed {
        let brand_id = slugify(&brand_name);
        if brand_id.is_empty() {
            return Err(anyhow!("brand `{brand_name}` slugifies to empty"));
        }
        for (class_key, models) in classes {
            let class = canonical_class(kind, &class_key).ok_or_else(|| {
                anyhow!(
                    "unknown class key `{class_key}` under brand `{brand_name}` (kind `{kind}`)"
                )
            })?;
            for raw_name in models {
                let (name, is_tandem) = strip_tandem(&raw_name);
                let model_id = slugify(name);
                if model_id.is_empty() {
                    return Err(anyhow!(
                        "model `{brand_name}` / `{name}` slugifies to empty"
                    ));
                }
                out.push(ModelRecord {
                    brand_id: brand_id.clone(),
                    brand_name: brand_name.clone(),
                    model_id,
                    name: name.to_string(),
                    kind,
                    class,
                    is_tandem,
                });
            }
        }
    }
    Ok(out)
}

/// Map a per-file class key (the JSON's nested-object key) to the canonical
/// `glider_class` enum value. PG keys use hyphens in the JSON; the SQL enum
/// uses underscores.
fn canonical_class(kind: &str, key: &str) -> Option<&'static str> {
    match (kind, key) {
        ("pg", "en-a") => Some("en_a"),
        ("pg", "en-b") => Some("en_b"),
        ("pg", "en-c") => Some("en_c"),
        ("pg", "en-d") => Some("en_d"),
        ("pg", "ccc") => Some("ccc"),
        ("hg", "single_surface") => Some("single_surface"),
        ("hg", "kingpost") => Some("kingpost"),
        ("hg", "topless") => Some("topless"),
        ("hg", "rigid") => Some("rigid"),
        ("sp", "standard") => Some("standard"),
        ("sp", "fifteen_metre") => Some("fifteen_metre"),
        ("sp", "eighteen_metre") => Some("eighteen_metre"),
        ("sp", "twenty_metre_two_seater") => Some("twenty_metre_two_seater"),
        ("sp", "open") => Some("open"),
        ("sp", "club") => Some("club"),
        ("sp", "motorglider") => Some("motorglider"),
        _ => None,
    }
}

fn strip_tandem(name: &str) -> (&str, bool) {
    match name.strip_suffix(TANDEM_SUFFIX) {
        Some(stem) => (stem, true),
        None => (name, false),
    }
}

/// `"Kühnle"` → `"kuhnle"`, `"Wills Wing"` → `"wills-wing"`, `"Sport (Спорт)"`
/// → `"sport"`. NFD-decomposes Latin diacritics, drops combining marks, and
/// collapses any run of non-`[a-z0-9]` characters into a single `-`. Cyrillic /
/// other non-Latin letters fall through the run-collapsing branch — fine for
/// our data where canonical names that pass through here are predominantly
/// Latin with the odd diacritic.
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_dash = true;
    for c in s.nfd() {
        if is_combining_mark(c) {
            continue;
        }
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

async fn apply(pool: &PgPool, records: Vec<ModelRecord>) -> anyhow::Result<Summary> {
    let mut brands: Vec<(&str, &str)> = records
        .iter()
        .map(|r| (r.brand_id.as_str(), r.brand_name.as_str()))
        .collect();
    brands.sort_unstable();
    brands.dedup();

    let mut tx = pool.begin().await.context("starting transaction")?;

    let mut brands_new = 0;
    let mut brands_updated = 0;

    for (id, name) in &brands {
        // `name = EXCLUDED.name` forces the UPDATE path so `xmax = 0` reports
        // INSERT vs UPDATE reliably (a true no-op ON CONFLICT branch wouldn't
        // touch xmax at all).
        let row = sqlx::query(
            "INSERT INTO brands (id, name) VALUES ($1, $2) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name \
             RETURNING (xmax = 0) AS inserted",
        )
        .bind(id)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| format!("upserting brand `{name}` ({id})"))?;

        if row.try_get::<bool, _>("inserted")? {
            brands_new += 1;
        } else {
            brands_updated += 1;
        }
    }

    let mut models_new = 0;
    let mut models_updated = 0;
    let mut models_tandem = 0;

    for r in &records {
        let row = sqlx::query(
            "INSERT INTO glider_models (brand_id, kind, id, name, class, is_tandem) \
             VALUES ($1, $2::glider_kind, $3, $4, $5::glider_class, $6) \
             ON CONFLICT (brand_id, kind, id) DO UPDATE SET \
                 name      = EXCLUDED.name, \
                 class     = EXCLUDED.class, \
                 is_tandem = EXCLUDED.is_tandem \
             RETURNING (xmax = 0) AS inserted",
        )
        .bind(&r.brand_id)
        .bind(r.kind)
        .bind(&r.model_id)
        .bind(&r.name)
        .bind(r.class)
        .bind(r.is_tandem)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| format!("upserting model `{}` / `{}`", r.brand_name, r.name))?;

        if row.try_get::<bool, _>("inserted")? {
            models_new += 1;
        } else {
            models_updated += 1;
        }
        if r.is_tandem {
            models_tandem += 1;
        }
    }

    tx.commit().await.context("committing transaction")?;

    Ok(Summary {
        brands_total: brands.len(),
        brands_new,
        brands_updated,
        models_total: records.len(),
        models_new,
        models_updated,
        models_tandem,
    })
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_examples() {
        assert_eq!(slugify("Aeros"), "aeros");
        assert_eq!(slugify("Wills Wing"), "wills-wing");
        assert_eq!(slugify("Kühnle"), "kuhnle");
        assert_eq!(
            slugify("Fliegerböhm & Flight Design"),
            "fliegerbohm-flight-design"
        );
        assert_eq!(slugify("Combat L 07"), "combat-l-07");
        assert_eq!(slugify("Sport (Спорт)"), "sport");
        assert_eq!(slugify("X-13"), "x-13");
        assert_eq!(slugify("Apco Aviation"), "apco-aviation");
        assert_eq!(slugify("APCO Aviation"), "apco-aviation");
    }
}
