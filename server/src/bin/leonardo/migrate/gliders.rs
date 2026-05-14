//! Resolve, dedupe, and insert one `gliders` row per pilot per distinct wing
//! seen in `leonardo_flights`.
//!
//! ## Inputs
//!
//! - The Leonardo source row's `cat`, `gliderBrandID`, `glider`,
//!   `gliderCertCategory`, `category`, plus the pilot's `userID`.
//! - `data/leo.brands.json` — Leo's `gliderBrandID` enum mirrored from
//!   `FN_brands.php`. Curated display names slugify into our `brands.id`s where
//!   we have a matching entry.
//! - `data/{hg,pg}.aliases.json` — regex-keyed `brand → model → [pattern]` maps
//!   that canonicalise raw `glider` strings within a resolved brand.
//! - `brands` and `glider_models` already loaded into Postgres (run `tengri
//!   import-gliders` first); we read them once at construction for kind-guard
//!   lookups and slug→display-name reverse maps.
//!
//! All three data files are read from disk at startup (not `include_str!`'d
//! into the binary, see `.cursor/rules/data-files.mdc`). A missing
//! `*.aliases.json` degrades to "no aliases this run"; `leo.brands.json` is the
//! master `gliderBrandID` → display-name map — every row needs it, so its
//! absence is a hard error at resolver construction.
//!
//! ## Resolution policy
//!
//! Trust the pilot's brand pick. `gliderBrandID > 0` and the slug resolves in
//! `brands`:
//!
//! - direct slug hit on `glider_models.id` (within `(brand, cat_kind)`) → fully
//!   resolved row.
//! - alias regex within `(brand, cat_kind)` → fully resolved row.
//! - otherwise → keep `brand_id`, `model_id = NULL`, `model_text =
//!   raw_glider_string`, `class = NULL` (with PG cert/tandem hints from
//!   `gliderCertCategory` and `category=3` if applicable). Same fallback
//!   whether the model name simply didn't match our aliases or we have no
//!   models catalogued for `(brand, cat_kind)` at all — we can't tell those
//!   apart, and the catalog is incomplete on purpose.
//!
//! Anything else (`gliderBrandID = 0`, leo id we don't have, slug not in
//! `brands`, `kind=other`) returns [`Err(SkipReason)`] so the operator can
//! categorise + fix the data upstream and re-run. The flights step is
//! idempotent so re-runs pick up the rows.
//!
//! Note: a pilot can pick a brand that has no models in our catalog for the
//! cat-derived kind. That row still imports — our catalog is incomplete on
//! purpose, and the brand might genuinely make wings of that kind we haven't
//! entered yet. Findable post-import with `WHERE model_id IS NULL`.
//!
//! ## Dedupe
//!
//! Per-pilot. The dedupe key is the full resolution tuple `(user_id, kind,
//! class, is_tandem, brand_id_or_text, model_id_or_text)` — exactly what we'd
//! insert. Same wing flown by two pilots → two `gliders` rows (one each). The
//! cache is run-scoped; cross-run idempotency comes from a `find_existing`
//! lookup on cache miss before the INSERT.

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{Context, anyhow};
use regex::{Regex, RegexBuilder};
use sqlx::PgPool;
use tengri_server::glider::import::slugify;

use super::super::shared::read_data_file;

/// Leo `cat` is a bitmask: airframe + propulsion bundled. We split it into the
/// two axes our schema cares about. Airframe wins for `kind` (paramotor falls
/// back to `pg`); propulsion drives the per-flight `propulsion` column.
const CAT_PG: u32 = 1;
const CAT_HG_FLEX: u32 = 2;
const CAT_HG_RIGID: u32 = 4;
const CAT_SP: u32 = 8;
const CAT_PARAMOTOR: u32 = 16;
const CAT_POWERED: u32 = 64;

/// Per-row fields the resolver pulls off the source `leonardo_flights` row.
/// Mirrors the columns added to the `SELECT` in
/// [`super::flights::SourceFlight`]; kept here so the resolver doesn't know the
/// rest of the row exists.
pub struct GliderInput<'a> {
    pub leo_flight_id: u64,
    pub user_id: i32,
    pub cat: u32,
    pub glider_brand_id: i32,
    pub glider_text: &'a str,
    pub glider_cert_category: u32,
    pub category: u32,
}

/// Output of one resolution. `glider_id` is what the flights row binds;
/// `propulsion` and `launch_method` are derived from the same source row (split
/// out here because `cat` carries propulsion bits and the resolver already
/// touches `cat`).
pub struct Resolved {
    pub glider_id: i32,
    pub propulsion: &'static str,
    /// Note about a non-fatal oddity worth surfacing in `Report::notes`. `None`
    /// when the row resolved cleanly with no caveats.
    pub note: Option<ResolveNote>,
}

#[derive(Debug)]
pub enum ResolveNote {
    /// Brand resolved but model didn't — neither a direct slug match nor any
    /// alias regex hit, and possibly no models catalogued for `(brand,
    /// cat_kind)` at all. Imported with `model_id = NULL`, `model_text = raw`.
    /// Operator action: extend `*.aliases.json` (or `<kind>.json`) if the raw
    /// spelling is a real model we don't have yet.
    ModelUnresolved {
        brand: String,
        raw: String,
        kind: &'static str,
    },
}

#[derive(Debug)]
pub struct SkipReason(String);

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Process-scoped, build-once. Holds the brand/model dictionaries, the alias
/// regexes, and the per-pilot dedupe cache. Single resolver per `leonardo
/// migrate` run.
pub struct GliderResolver {
    /// Leo `gliderBrandID` → display name. Values that slugify to a row in
    /// `brands` count as resolved; the rest fall through to the "leo brand X
    /// not in our dictionary" skip path. Includes Leo IDs we don't have a match
    /// for, so the skip reason can name them.
    leo_brands: HashMap<i32, String>,
    /// `brands.id` → `brands.name` for note formatting. Display name is what
    /// the operator sees in the report.
    brand_names: HashMap<String, String>,
    /// All canonical models, indexed by `(brand_id, kind)`. Used both for the
    /// direct slug hit path and to compile the alias regexes against real model
    /// rows; the per-model `class` / `is_tandem` also feeds the same columns on
    /// the resolved `gliders` row so the importer doesn't have to wait for the
    /// `sync_glider_denorm` trigger to backfill them.
    models_by_brand_kind: HashMap<(String, &'static str), Vec<ModelEntry>>,
    /// Compiled alias regexes per kind, listed per `(brand_id, model_id)`.
    /// Iterated in declaration order; first hit wins within a brand.
    alias_rules: HashMap<&'static str, Vec<AliasRule>>,
    /// Run-scoped per-pilot dedupe cache.
    cache: HashMap<DedupeKey, i32>,
}

struct AliasRule {
    /// Normalised brand display name from the alias JSON, slugified to the FK
    /// target.
    brand_id: String,
    model_id: String,
    pattern: Regex,
}

struct ModelEntry {
    id: String,
    class: &'static str,
    is_tandem: bool,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
struct DedupeKey {
    user_id: i32,
    kind: &'static str,
    class: Option<&'static str>,
    is_tandem: Option<bool>,
    brand_id: Option<String>,
    brand_text: Option<String>,
    model_id: Option<String>,
    model_text: Option<String>,
}

/// The 7 columns of `gliders` that `classify` produces, plus an optional rollup
/// note. `Resolver::resolve` builds a `DedupeKey` from these and either hits
/// the cache or hands the struct to `upsert_glider`.
struct Classified {
    kind: &'static str,
    class: Option<&'static str>,
    is_tandem: Option<bool>,
    brand_id: Option<String>,
    brand_text: Option<String>,
    model_id: Option<String>,
    model_text: Option<String>,
    note: Option<ResolveNote>,
}

impl GliderResolver {
    pub async fn build(pool: &PgPool) -> anyhow::Result<Self> {
        let leo_brands = parse_leo_brands()?;

        let brands = sqlx::query_as::<_, (String, String)>("SELECT id, name FROM brands")
            .fetch_all(pool)
            .await
            .context("loading brands")?;
        let brand_names: HashMap<String, String> = brands.into_iter().collect();

        let models = sqlx::query_as::<_, (String, String, String, String, bool)>(
            "SELECT brand_id, kind::text, id, class::text, is_tandem FROM glider_models",
        )
        .fetch_all(pool)
        .await
        .context("loading glider_models")?;

        let mut models_by_brand_kind: HashMap<(String, &'static str), Vec<ModelEntry>> =
            HashMap::new();
        for (brand_id, kind, id, class, is_tandem) in models {
            let kind = kind_str_to_static(&kind).ok_or_else(|| {
                anyhow!(
                    "unexpected glider_kind '{kind}' on {brand_id}/{id}; the resolver only \
                     handles pg/hg/sp/other"
                )
            })?;
            let class = canonical_class_str(&class).ok_or_else(|| {
                anyhow!(
                    "unexpected glider_class '{class}' on {brand_id}/{id}; resolver was built \
                     against an older schema"
                )
            })?;
            models_by_brand_kind
                .entry((brand_id, kind))
                .or_default()
                .push(ModelEntry {
                    id,
                    class,
                    is_tandem,
                });
        }

        let alias_rules = compile_alias_rules(&brand_names, &models_by_brand_kind)?;

        Ok(Self {
            leo_brands,
            brand_names,
            models_by_brand_kind,
            alias_rules,
            cache: HashMap::new(),
        })
    }

    pub async fn resolve(
        &mut self,
        pool: &PgPool,
        input: &GliderInput<'_>,
    ) -> Result<Resolved, SkipReason> {
        let cat_kind = airframe_kind(input.cat);
        let propulsion = propulsion_for(input.cat);

        if cat_kind == "other" {
            return Err(SkipReason(format!(
                "cat={} — kind=other has no brand mapping",
                input.cat
            )));
        }

        let classified = self.classify(cat_kind, input)?;

        let key = DedupeKey {
            user_id: input.user_id,
            kind: classified.kind,
            class: classified.class,
            is_tandem: classified.is_tandem,
            brand_id: classified.brand_id.clone(),
            brand_text: classified.brand_text.clone(),
            model_id: classified.model_id.clone(),
            model_text: classified.model_text.clone(),
        };
        if let Some(id) = self.cache.get(&key) {
            return Ok(Resolved {
                glider_id: *id,
                propulsion,
                note: classified.note,
            });
        }

        let glider_id = upsert_glider(pool, input.leo_flight_id, input.user_id, &classified)
            .await
            .map_err(|e| SkipReason(format!("upsert glider: {e:#}")))?;
        self.cache.insert(key, glider_id);

        Ok(Resolved {
            glider_id,
            propulsion,
            note: classified.note,
        })
    }

    /// Run the resolution policy. Returns the columns that will land on the
    /// `gliders` row, plus an optional `ResolveNote` for the rollup.
    fn classify(
        &self,
        cat_kind: &'static str,
        input: &GliderInput<'_>,
    ) -> Result<Classified, SkipReason> {
        // gliderBrandID=0 means the pilot didn't pick a brand. Per the policy
        // we don't guess; the operator fixes the source row.
        if input.glider_brand_id == 0 {
            return Err(SkipReason(
                "gliderBrandID = 0 — no brand picked, fix the source row".to_string(),
            ));
        }

        let leo_name = self.leo_brands.get(&input.glider_brand_id).ok_or_else(|| {
            SkipReason(format!(
                "unknown leo brand id {} — extend data/leo.brands.json",
                input.glider_brand_id
            ))
        })?;
        let brand_id_candidate = slugify(leo_name);
        if !self.brand_names.contains_key(&brand_id_candidate) {
            return Err(SkipReason(format!(
                "leo brand '{leo_name}' not in our dictionary — add to data/{cat_kind}.json"
            )));
        }

        // No kind guard: an empty `(brand, kind)` bucket in our catalog just
        // means we haven't entered the brand's models of that kind yet, not
        // that the brand doesn't make them. Such rows fall through to the
        // brand-only resolution below, same as rows where the brand+kind
        // matches but the specific model name doesn't.
        let raw = input.glider_text.trim();
        let slug = slugify(raw);
        if let Some(model) = self
            .models_by_brand_kind
            .get(&(brand_id_candidate.clone(), cat_kind))
            .and_then(|models| models.iter().find(|m| m.id == slug))
        {
            return Ok(Classified {
                kind: cat_kind,
                class: Some(model.class),
                is_tandem: Some(model.is_tandem),
                brand_id: Some(brand_id_candidate),
                brand_text: None,
                model_id: Some(model.id.clone()),
                model_text: None,
                note: None,
            });
        }
        if let Some(rule) = self.match_alias_in_brand(cat_kind, &brand_id_candidate, raw) {
            let (class, is_tandem) =
                self.class_tandem_for(&rule.brand_id, cat_kind, &rule.model_id);
            return Ok(Classified {
                kind: cat_kind,
                class,
                is_tandem,
                brand_id: Some(rule.brand_id.clone()),
                brand_text: None,
                model_id: Some(rule.model_id.clone()),
                model_text: None,
                note: None,
            });
        }

        // Brand resolved, model didn't (either no alias hit or no models
        // catalogued for `(brand, cat_kind)`). Pilot picked the brand — respect
        // it; we don't guess the model. PG flights still get a class hint from
        // `gliderCertCategory` and a tandem hint from `category=3`; HG/SP have
        // no per-flight cert signal so those columns stay NULL until the model
        // resolves.
        let brand_display = self
            .brand_names
            .get(&brand_id_candidate)
            .cloned()
            .unwrap_or_else(|| leo_name.clone());
        let class = if cat_kind == "pg" {
            pg_class_from_cert(input.glider_cert_category)
        } else {
            None
        };
        let is_tandem = (cat_kind == "pg" && input.category == 3).then_some(true);
        Ok(Classified {
            kind: cat_kind,
            class,
            is_tandem,
            brand_id: Some(brand_id_candidate),
            brand_text: None,
            model_id: None,
            model_text: Some(raw.to_string()),
            note: Some(ResolveNote::ModelUnresolved {
                brand: brand_display,
                raw: raw.to_string(),
                kind: cat_kind,
            }),
        })
    }

    fn match_alias_in_brand(
        &self,
        kind: &'static str,
        brand_id: &str,
        raw: &str,
    ) -> Option<&AliasRule> {
        self.alias_rules
            .get(kind)?
            .iter()
            .find(|r| r.brand_id == brand_id && r.pattern.is_match(raw))
    }

    fn class_tandem_for(
        &self,
        brand_id: &str,
        kind: &'static str,
        model_id: &str,
    ) -> (Option<&'static str>, Option<bool>) {
        self.models_by_brand_kind
            .get(&(brand_id.to_string(), kind))
            .and_then(|ms| ms.iter().find(|m| m.id == model_id))
            .map(|m| (Some(m.class), Some(m.is_tandem)))
            .unwrap_or((None, None))
    }
}

fn airframe_kind(cat: u32) -> &'static str {
    if cat & CAT_PG != 0 {
        "pg"
    } else if cat & (CAT_HG_FLEX | CAT_HG_RIGID) != 0 {
        "hg"
    } else if cat & CAT_SP != 0 {
        "sp"
    } else if cat & CAT_PARAMOTOR != 0 {
        // paramotor = paraglider + engine; airframe is PG by definition.
        "pg"
    } else {
        // cat=64 alone (powered, no airframe declared), cat=0, etc.
        "other"
    }
}

fn propulsion_for(cat: u32) -> &'static str {
    if cat & (CAT_PARAMOTOR | CAT_POWERED) != 0 {
        "powered"
    } else {
        "free"
    }
}

/// PG cert bitmask → our `glider_class`. Highest bit wins (a glider with both
/// EN-A and EN-B set was certified for both, but the higher class is the
/// meaningful one for filtering). DHV / AFNOR / CCC bits we don't translate yet
/// — none of those appear in the dump, and CCC is orthogonal to EN tiers
/// anyway.
fn pg_class_from_cert(cert: u32) -> Option<&'static str> {
    if cert & 256 != 0 {
        Some("en_d")
    } else if cert & 128 != 0 {
        Some("en_c")
    } else if cert & 64 != 0 {
        Some("en_b")
    } else if cert & 1 != 0 {
        Some("en_a")
    } else {
        None
    }
}

/// Translate Leo's `startType` enum to our `launch_method`. Leo offers four
/// values in the upload form: `1`=foot, `2`=winch, `4`=microlight aircraft
/// tow (their "Сверхлёгкий самолёт/ДП"), `8`=E-motor (powered self-launch).
/// Only three of those are launch axes — `8` is propulsion, the pilot still
/// foot-launches a powered wing — so we collapse it to `foot` and let
/// `flights.propulsion` carry the engine info. NULL and any unexpected value
/// also default to `foot` (the field is nullable in MySQL; we don't want to
/// skip rows over a stray surprise value).
pub fn launch_method_for(start_type: Option<u8>) -> &'static str {
    match start_type.unwrap_or(1) {
        2 => "winch",
        4 => "aerotow",
        _ => "foot", // "8" = E-powered self-launch
    }
}

fn kind_str_to_static(s: &str) -> Option<&'static str> {
    match s {
        "pg" => Some("pg"),
        "hg" => Some("hg"),
        "sp" => Some("sp"),
        "other" => Some("other"),
        _ => None,
    }
}

/// Pin the variants we expect off `glider_models.class::text` to `&'static str`
/// so the resolver can keep them on `ModelEntry`. Mirrors the enum in migration
/// `0010_gliders_user_id_and_sp_classes.sql`; any drift returns `None` and
/// surfaces at construction.
fn canonical_class_str(s: &str) -> Option<&'static str> {
    Some(match s {
        "en_a" => "en_a",
        "en_b" => "en_b",
        "en_c" => "en_c",
        "en_d" => "en_d",
        "ccc" => "ccc",
        "single_surface" => "single_surface",
        "kingpost" => "kingpost",
        "topless" => "topless",
        "rigid" => "rigid",
        "thirteen_point_five_metre" => "thirteen_point_five_metre",
        "standard" => "standard",
        "fifteen_metre" => "fifteen_metre",
        "eighteen_metre" => "eighteen_metre",
        "twenty_metre_two_seater" => "twenty_metre_two_seater",
        "open" => "open",
        "club" => "club",
        "microlift" => "microlift",
        "ultralight" => "ultralight",
        _ => return None,
    })
}

/// Load `leo.brands.json`. The master `gliderBrandID` → display-name map;
/// without it every row skips with "unknown leo brand id N", so we hard-fail at
/// construction instead of letting the whole run drain into the skip report.
fn parse_leo_brands() -> anyhow::Result<HashMap<i32, String>> {
    const FILE: &str = "leo.brands.json";
    let src = read_data_file(FILE)?
        .ok_or_else(|| anyhow!("data/{FILE} missing — required for leonardo migrate"))?;
    let raw: HashMap<String, String> =
        serde_json::from_str(&src).with_context(|| format!("parsing data/{FILE}"))?;
    let mut out = HashMap::with_capacity(raw.len());
    for (k, v) in raw {
        let id: i32 = k
            .parse()
            .with_context(|| format!("data/{FILE} key '{k}' is not an int"))?;
        out.insert(id, v);
    }
    Ok(out)
}

fn compile_alias_rules(
    brand_names: &HashMap<String, String>,
    models_by_brand_kind: &HashMap<(String, &'static str), Vec<ModelEntry>>,
) -> anyhow::Result<HashMap<&'static str, Vec<AliasRule>>> {
    let mut out: HashMap<&'static str, Vec<AliasRule>> = HashMap::new();
    for kind in ["hg", "pg", "sp"] {
        out.insert(
            kind,
            compile_aliases_for_kind(kind, brand_names, models_by_brand_kind)?,
        );
    }
    Ok(out)
}

/// Read and parse `data/<kind>.aliases.json` into compiled regex rules. Missing
/// file → empty (no aliases for this kind this run); brand/model references
/// that don't exist in `brands` / `glider_models` are a hard error so a typo in
/// the JSON doesn't silently drop rules.
fn compile_aliases_for_kind(
    kind: &'static str,
    brand_names: &HashMap<String, String>,
    models_by_brand_kind: &HashMap<(String, &'static str), Vec<ModelEntry>>,
) -> anyhow::Result<Vec<AliasRule>> {
    let file = format!("{kind}.aliases.json");
    let Some(src) = read_data_file(&file)? else {
        eprintln!("note: data/{file} missing, no {kind} aliases this run");
        return Ok(Vec::new());
    };

    #[derive(serde::Deserialize)]
    struct File {
        model: HashMap<String, HashMap<String, Vec<String>>>,
    }
    let parsed: File =
        serde_json::from_str(&src).with_context(|| format!("parsing data/{file}"))?;
    let mut out = Vec::new();
    for (brand_name, models) in parsed.model {
        let brand_id = slugify(&brand_name);
        if !brand_names.contains_key(&brand_id) {
            return Err(anyhow!(
                "{file} references brand '{brand_name}' (slug '{brand_id}') \
                 but it's not in `brands` — add it to {kind}.json or fix the alias",
            ));
        }
        let kind_models = models_by_brand_kind
            .get(&(brand_id.clone(), kind))
            .ok_or_else(|| {
                anyhow!(
                    "{file} references brand '{brand_name}' but it has no \
                     {kind} models — wrong file?",
                )
            })?;
        for (model_name, patterns) in models {
            let model_id = slugify(&model_name);
            if !kind_models.iter().any(|m| m.id == model_id) {
                return Err(anyhow!(
                    "{file} references {brand_name}/{model_name} (slug \
                     {brand_id}/{model_id}) but no such {kind} model — add it to \
                     {kind}.json or fix the alias",
                ));
            }
            for pat in patterns {
                let re = RegexBuilder::new(&format!("(?i)^{pat}$"))
                    .build()
                    .with_context(|| {
                        format!("compiling alias regex for {brand_name}/{model_name}: '{pat}'")
                    })?;
                out.push(AliasRule {
                    brand_id: brand_id.clone(),
                    model_id: model_id.clone(),
                    pattern: re,
                });
            }
        }
    }
    Ok(out)
}

/// Look up an existing `gliders` row matching the resolution columns, or insert
/// a fresh one. Per-pilot dedupe lives in the in-memory `cache`; this function
/// handles cross-run idempotency (a previous `leonardo migrate` run inserted
/// the same wing).
async fn upsert_glider(
    pool: &PgPool,
    leo_flight_id: u64,
    user_id: i32,
    c: &Classified,
) -> anyhow::Result<i32> {
    static FIND_SQL: OnceLock<String> = OnceLock::new();
    let find_sql = FIND_SQL.get_or_init(|| {
        "SELECT id FROM gliders \
         WHERE user_id = $1 \
           AND kind = $2::glider_kind \
           AND class IS NOT DISTINCT FROM $3::glider_class \
           AND is_tandem IS NOT DISTINCT FROM $4 \
           AND brand_id IS NOT DISTINCT FROM $5 \
           AND brand_text IS NOT DISTINCT FROM $6 \
           AND model_id IS NOT DISTINCT FROM $7 \
           AND model_text IS NOT DISTINCT FROM $8 \
         LIMIT 1"
            .to_string()
    });
    let existing: Option<i32> = sqlx::query_scalar(find_sql)
        .bind(user_id)
        .bind(c.kind)
        .bind(c.class)
        .bind(c.is_tandem)
        .bind(c.brand_id.as_deref())
        .bind(c.brand_text.as_deref())
        .bind(c.model_id.as_deref())
        .bind(c.model_text.as_deref())
        .fetch_optional(pool)
        .await
        .with_context(|| format!("looking up existing glider for leo flight {leo_flight_id}"))?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gliders (user_id, kind, class, is_tandem, brand_id, brand_text, \
                              model_id, model_text) \
         VALUES ($1, $2::glider_kind, $3::glider_class, $4, $5, $6, $7, $8) \
         RETURNING id",
    )
    .bind(user_id)
    .bind(c.kind)
    .bind(c.class)
    .bind(c.is_tandem)
    .bind(c.brand_id.as_deref())
    .bind(c.brand_text.as_deref())
    .bind(c.model_id.as_deref())
    .bind(c.model_text.as_deref())
    .fetch_one(pool)
    .await
    .with_context(|| format!("inserting glider for leo flight {leo_flight_id}"))?;
    Ok(id)
}
