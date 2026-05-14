//! Resolve a Leonardo source row to `(brand_id, kind, model_id)` — the
//! composite FK from `flights` to `models`. For wings whose model isn't in our
//! canonical catalog, materialise a per-pilot custom `models` row (`user_id =
//! pilot`, `id = '<user_id>:<slugify(raw)>'`) so the flight has something to
//! point at. Brand resolution stays canonical-only: Leo's `gliderBrandID` is a
//! curated enum mirrored in `data/leo.brands.json`, so a brand we can't resolve
//! canonically is a `SkipReason`, not a custom row.
//!
//! ## Inputs
//!
//! - The Leonardo source row's `cat`, `gliderBrandID`, `glider`,
//!   `gliderCertCategory`, `category`, plus the pilot's `userID`.
//! - `data/leo.brands.json` — Leo's `gliderBrandID` enum mirrored from
//!   `FN_brands.php`. Curated display names slugify into our `brands.id`s where
//!   we have a matching canonical entry.
//! - `data/{hg,pg}.aliases.json` — regex-keyed `brand → model → [pattern]` maps
//!   that canonicalise raw `glider` strings within a resolved brand.
//! - `brands` and `models` already loaded into Postgres (run `tengri
//!   import-gliders` first); we read canonical rows once at construction for
//!   the direct-slug lookup and the reverse display-name map.
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
//! `brands` (canonical only):
//!
//! 1. Direct slug hit on `models.id` (within `(brand, cat_kind, user_id IS
//!    NULL)`) → canonical resolution.
//! 2. Alias regex within `(brand, cat_kind)` → canonical resolution.
//! 3. Otherwise → custom path. `INSERT INTO models … ON CONFLICT (brand_id,
//!    kind, id) DO NOTHING`: the id is deterministic
//!    (`<user_id>:<slugify(raw)>`), so cosmetic variants of the same raw text
//!    from the same pilot collide on the PK and dedupe naturally. A
//!    `CustomModelCreated` note surfaces in the run report — operator may want
//!    to extend the canonical catalog so the next run resolves it instead of
//!    accumulating customs.
//!
//! Anything else (`gliderBrandID = 0`, leo id we don't have, slug not in
//! `brands`, `kind=other`) returns [`Err(SkipReason)`] so the operator can
//! categorise + fix the data upstream and re-run. The flights step is
//! idempotent so re-runs pick up the rows.
//!
//! ## Custom-model class
//!
//! `models.class` is NOT NULL. For PG customs we pull a hint from
//! `gliderCertCategory`. For HG customs we have the rigid-vs-flex bit on `cat`
//! (so HG rigid → `'rigid'`), and for flex Leo's `category` enum splits
//! kingpost (`1`) from topless (`2`); `single_surface` isn't on Leo's form so
//! it never appears, and unset `category` lands as `'unknown'`. For SP customs
//! Leo records `cat=8` and nothing else. Anything indeterminate lands as
//! `class='unknown'` — a distinct enum value so the row migrates and the
//! pilot/operator can refine it later.
//!
//! ## Cache
//!
//! Run-scoped, keyed on `(user_id, brand_id, cat_kind, slugify(raw))`. Cosmetic
//! variants of the same raw text (whitespace / punctuation / case) all slugify
//! to the same key and share one entry, which matters most for the custom path
//! (saves repeat INSERTs across a long import). Cross-run idempotency comes
//! from `ON CONFLICT DO NOTHING` on the model PK.

use std::collections::HashMap;

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

/// Output of one resolution. `(brand_id, kind, model_id)` is the composite FK
/// that lands on the `flights` row. `propulsion` comes off the same source row
/// (split out here because `cat` carries propulsion bits and the resolver
/// already touches `cat`).
pub struct Resolved {
    pub brand_id: String,
    pub kind: &'static str,
    pub model_id: String,
    pub propulsion: &'static str,
    /// Non-fatal note for the run report. Emitted only on the *first*
    /// occurrence of a `(user, brand, kind, raw_slug)` bucket; subsequent cache
    /// hits return `None` to keep the report from listing the same custom model
    /// once per flight on it.
    pub note: Option<ResolveNote>,
}

#[derive(Debug, Clone)]
pub enum ResolveNote {
    /// Brand resolved canonically, model didn't — a per-pilot custom row was
    /// inserted into `models`. Operator action: extend the canonical catalog
    /// (`<kind>.json` + `tengri import-gliders`) if the raw is a real model we
    /// just don't have yet, so the next run resolves it canonically instead of
    /// accumulating customs.
    CustomModelCreated {
        brand: String,
        raw: String,
        kind: &'static str,
        class: &'static str,
    },
}

#[derive(Debug)]
pub struct SkipReason(String);

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Process-scoped, build-once. Holds the canonical brand/model dictionaries,
/// the alias regexes, and the per-pilot resolution cache. Single resolver per
/// `leonardo migrate` run.
pub struct GliderResolver {
    /// Leo `gliderBrandID` → display name. Values that slugify to a row in
    /// `brands` (canonical) count as resolved; the rest fall through to the
    /// "leo brand X not in our dictionary" skip path. Includes Leo IDs we don't
    /// have a match for, so the skip reason can name them.
    leo_brands: HashMap<i32, String>,
    /// Canonical `brands.id` → `brands.name` for note formatting. Custom brands
    /// aren't in here — Leo never creates them (brand resolution is
    /// canonical-only).
    brand_names: HashMap<String, String>,
    /// Canonical models, indexed by `(brand_id, kind)`. Used for the direct
    /// slug-hit path and as the target set the alias regexes compile against.
    models_by_brand_kind: HashMap<(String, &'static str), Vec<ModelEntry>>,
    /// Compiled alias regexes per kind, listed per `(brand_id, model_id)`.
    /// Iterated in declaration order; first hit wins within a brand.
    alias_rules: HashMap<&'static str, Vec<AliasRule>>,
    /// Run-scoped resolution cache. Key includes `slugify(raw_glider_text)` so
    /// cosmetic variants of the same wing share an entry. Value is the resolved
    /// `model_id` (canonical or custom — distinguishable by the `<user_id>:`
    /// prefix if needed; downstream code doesn't need to care).
    cache: HashMap<CacheKey, String>,
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
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
struct CacheKey {
    user_id: i32,
    brand_id: String,
    kind: &'static str,
    raw_slug: String,
}

impl GliderResolver {
    pub async fn build(pool: &PgPool) -> anyhow::Result<Self> {
        let leo_brands = parse_leo_brands()?;

        let brands = sqlx::query_as::<_, (String, String)>(
            "SELECT id, name FROM brands WHERE user_id IS NULL",
        )
        .fetch_all(pool)
        .await
        .context("loading canonical brands")?;
        let brand_names: HashMap<String, String> = brands.into_iter().collect();

        let models = sqlx::query_as::<_, (String, String, String)>(
            "SELECT brand_id, kind::text, id FROM models WHERE user_id IS NULL",
        )
        .fetch_all(pool)
        .await
        .context("loading canonical models")?;

        let mut models_by_brand_kind: HashMap<(String, &'static str), Vec<ModelEntry>> =
            HashMap::new();
        for (brand_id, kind, id) in models {
            let kind = kind_str_to_static(&kind).ok_or_else(|| {
                anyhow!(
                    "unexpected glider_kind '{kind}' on {brand_id}/{id}; the resolver only \
                     handles pg/hg/sp/other"
                )
            })?;
            models_by_brand_kind
                .entry((brand_id, kind))
                .or_default()
                .push(ModelEntry { id });
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

        // Brand resolution. `gliderBrandID=0` means the pilot didn't pick a
        // brand; we don't guess. Unknown or unmapped ids → skip with a
        // categorised reason.
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
        let brand_id = slugify(leo_name);
        if !self.brand_names.contains_key(&brand_id) {
            return Err(SkipReason(format!(
                "leo brand '{leo_name}' not in our dictionary — add to data/{cat_kind}.json"
            )));
        }

        // Cache lookup. The key's `raw_slug` collapses cosmetic variants
        // (whitespace / punctuation / case) onto one entry.
        let raw = input.glider_text.trim();
        let raw_slug = slugify(raw);
        let cache_key = CacheKey {
            user_id: input.user_id,
            brand_id: brand_id.clone(),
            kind: cat_kind,
            raw_slug: raw_slug.clone(),
        };
        if let Some(model_id) = self.cache.get(&cache_key) {
            return Ok(Resolved {
                brand_id,
                kind: cat_kind,
                model_id: model_id.clone(),
                propulsion,
                note: None,
            });
        }

        // Canonical: direct slug hit.
        if let Some(model) = self
            .models_by_brand_kind
            .get(&(brand_id.clone(), cat_kind))
            .and_then(|models| models.iter().find(|m| m.id == raw_slug))
        {
            let model_id = model.id.clone();
            self.cache.insert(cache_key, model_id.clone());
            return Ok(Resolved {
                brand_id,
                kind: cat_kind,
                model_id,
                propulsion,
                note: None,
            });
        }

        // Canonical: alias regex within the resolved brand.
        if let Some(rule) = self.match_alias_in_brand(cat_kind, &brand_id, raw) {
            let model_id = rule.model_id.clone();
            self.cache.insert(cache_key, model_id.clone());
            return Ok(Resolved {
                brand_id,
                kind: cat_kind,
                model_id,
                propulsion,
                note: None,
            });
        }

        // Custom path. The id is deterministic (`<user>:<slug(raw)>`), so ON
        // CONFLICT DO NOTHING handles cross-run idempotency without a RETURNING
        // dance.
        let brand_display = self
            .brand_names
            .get(&brand_id)
            .cloned()
            .unwrap_or_else(|| leo_name.clone());
        let class = custom_class_for(
            cat_kind,
            input.cat,
            input.glider_cert_category,
            input.category,
        );
        let is_tandem = cat_kind == "pg" && input.category == 3;
        let custom_id = format!("{}:{}", input.user_id, raw_slug);
        insert_custom_model(
            pool,
            input.leo_flight_id,
            &brand_id,
            cat_kind,
            &custom_id,
            raw,
            class,
            is_tandem,
            input.user_id,
        )
        .await
        .map_err(|e| SkipReason(format!("inserting custom model: {e:#}")))?;
        self.cache.insert(cache_key, custom_id.clone());

        Ok(Resolved {
            brand_id,
            kind: cat_kind,
            model_id: custom_id,
            propulsion,
            note: Some(ResolveNote::CustomModelCreated {
                brand: brand_display,
                raw: raw.to_string(),
                kind: cat_kind,
                class,
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

/// `models.class` value for a custom row. PG: cert bits. HG: rigid bit on
/// `cat`, then Leo's `category` enum disambiguates flex (`1`=kingpost,
/// `2`=topless; `0`/anything else stays `'unknown'`, which also covers
/// `single_surface` — Leo's form doesn't offer it). SP: `'unknown'` outright —
/// Leo records `cat=8` with no FAI-class signal.
fn custom_class_for(cat_kind: &'static str, cat: u32, cert: u32, category: u32) -> &'static str {
    match cat_kind {
        "pg" => pg_class_from_cert(cert).unwrap_or("unknown"),
        "hg" => {
            if cat & CAT_HG_RIGID != 0 {
                "rigid"
            } else {
                match category {
                    1 => "kingpost",
                    2 => "topless",
                    _ => "unknown",
                }
            }
        }
        "sp" => "unknown",
        // Filtered out earlier by `airframe_kind(...) == "other"` returning a
        // SkipReason; never reaches the custom path.
        _ => unreachable!("custom_class_for called with cat_kind='{cat_kind}'"),
    }
}

/// PG cert bitmask → our `glider_class`. Highest bit wins. DHV / AFNOR /
/// CCC bits we don't translate yet — none of those appear in the dump, and
/// CCC is orthogonal to EN tiers anyway.
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
/// values in the upload form: `1`=foot, `2`=winch, `4`=microlight aircraft tow,
/// `8`=E-motor (powered self-launch). Only three of those are launch axes — `8`
/// is propulsion, the pilot still foot-launches a powered wing — so we collapse
/// it to `foot` and let `flights.propulsion` carry the engine info. NULL and
/// any unexpected value default to `foot`.
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
/// that don't exist in `brands` / `models` are a hard error so a typo in the
/// JSON doesn't silently drop rules.
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

/// Insert a per-pilot custom model. The PK on `(brand_id, kind, id)` makes this
/// idempotent across runs *and* dedupes cosmetic variants from the same pilot
/// within a run (same raw text → same slug → same `id`).
#[allow(clippy::too_many_arguments)]
async fn insert_custom_model(
    pool: &PgPool,
    leo_flight_id: u64,
    brand_id: &str,
    kind: &'static str,
    id: &str,
    name: &str,
    class: &'static str,
    is_tandem: bool,
    user_id: i32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO models (brand_id, kind, id, name, class, is_tandem, user_id) \
         VALUES ($1, $2::glider_kind, $3, $4, $5::glider_class, $6, $7) \
         ON CONFLICT (brand_id, kind, id) DO NOTHING",
    )
    .bind(brand_id)
    .bind(kind)
    .bind(id)
    .bind(name)
    .bind(class)
    .bind(is_tandem)
    .bind(user_id)
    .execute(pool)
    .await
    .with_context(|| format!("inserting custom model for leo flight {leo_flight_id}"))?;
    Ok(())
}
