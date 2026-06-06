//! On-demand e2e fixtures for `tengri-scoring`.
//!
//! Each `tests/tracks/<bucket>/manifest.jsonc` declares per-IGC expected
//! results for the three scorable routes (`fai-t`, `free-t`, `fd`). Trials are
//! shaped like `<bucket>::<igc_stem>`, marked ignored, and compare scorer
//! output against the manifest under documented distance / score / time
//! tolerances. Run with:
//!
//! ```text
//! cargo test -p tengri-scoring --test e2e -- --ignored
//! ```
//!
//! With `TENGRI_FILL=1`, the harness instead walks every manifest, scores any
//! `*.igc` next to it that isn't yet in the file, and appends the fresh entries
//! — preserving prior content (including comments). Existing entries are never
//! overwritten. Run in release for trustworthy timings:
//!
//! ```text
//! TENGRI_FILL=1 cargo test -p tengri-scoring --test e2e --release
//! ```

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use libtest_mimic::{Arguments, Failed, Trial};
use serde::Deserialize;
use tengri_formats::{find_flight_window, parse_input, slice_flight_window};
use tengri_geo::PointE5;
use tengri_scoring::{
    Route, ScoringOutcome, ScoringTrack, evaluate_fai_triangle_lazy, evaluate_free_distance,
    evaluate_free_triangle_lazy,
};

/// Root anchored at compile time so the harness can be launched from any cwd
/// (`cargo test -p ...` from the workspace root, an editor's "run test"
/// button, etc.). Contents still load at runtime — see `parse_manifest` /
/// `load_scoring_track`.
const TRACKS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/tracks");

fn main() {
    if std::env::var_os("TENGRI_FILL").is_some() {
        let summary = run_fill();
        std::process::exit(if summary.had_failures { 1 } else { 0 });
    }

    let mut args = Arguments::from_args();
    // Trials measure wall-clock time, and `run_fill` records its baselines
    // serially; running trials in parallel would contend the CPU and inflate
    // timings out of proportion to the 30 % slack. Default to single-threaded
    // so trial measurements line up with the manifest. The user can still
    // override with `cargo test ... -- --test-threads N`.
    if args.test_threads.is_none() {
        args.test_threads = Some(1);
    }
    let trials = collect_trials();
    libtest_mimic::run(&args, trials).exit();
}

fn collect_trials() -> Vec<Trial> {
    let root = Path::new(TRACKS_DIR);
    if !root.is_dir() {
        return Vec::new();
    }

    let mut trials = Vec::new();
    let walker = walkdir::WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && entry.file_name() == "manifest.jsonc");

    for entry in walker {
        let manifest_path = entry.into_path();
        let bucket = bucket_name(root, &manifest_path);
        let manifest_dir = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        match parse_manifest(&manifest_path) {
            Ok(entries) => {
                for (igc_stem, expectations) in entries {
                    let trial_name = format!("{bucket}::{igc_stem}");
                    let case = TestCase {
                        trial_name: trial_name.clone(),
                        igc_path: manifest_dir.join(format!("{igc_stem}.igc")),
                        expectations,
                    };
                    trials.push(
                        Trial::test(trial_name, move || run_trial(&case)).with_ignored_flag(true),
                    );
                }
            }
            Err(err) => {
                let trial_name = format!("{bucket}::__manifest__");
                let display_path = manifest_path.display().to_string();
                let message = format!("{err:#}");
                trials.push(
                    Trial::test(trial_name, move || {
                        Err(Failed::from(format!(
                            "manifest parse error in {display_path}: {message}"
                        )))
                    })
                    .with_ignored_flag(true),
                );
            }
        }
    }

    trials
}

fn bucket_name(root: &Path, manifest_path: &Path) -> String {
    let parent = manifest_path.parent().unwrap_or(manifest_path);
    let rel = parent.strip_prefix(root).unwrap_or(parent);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("::")
}

#[derive(Debug)]
struct TestCase {
    trial_name: String,
    igc_path: PathBuf,
    expectations: FileExpectations,
}

#[derive(Debug, Deserialize)]
struct FileExpectations {
    #[serde(rename = "fai-t")]
    fai_triangle: RouteExpectations,
    #[serde(rename = "free-t")]
    free_triangle: RouteExpectations,
    fd: RouteExpectations,
}

#[derive(Debug, Deserialize)]
struct RouteExpectations {
    time: u32,
    distance: Option<u32>,
    score: Option<f64>,
}

impl FileExpectations {
    fn for_slot(&self, slot: Slot) -> &RouteExpectations {
        match slot {
            Slot::FaiTriangle => &self.fai_triangle,
            Slot::FreeTriangle => &self.free_triangle,
            Slot::FreeDistance => &self.fd,
        }
    }
}

fn parse_manifest(path: &Path) -> anyhow::Result<BTreeMap<String, FileExpectations>> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let entries: BTreeMap<String, FileExpectations> =
        json5::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    // Triangle scorers verify both `distance` and `score`, so the two have to
    // agree on shape. FD's score is a mechanical `distance_km × 1.0`, so the
    // harness skips it entirely (see `Slot::checks_score`) and the manifest
    // doesn't have to declare it.
    for (file, exp) in &entries {
        for (label, route) in [("fai-t", &exp.fai_triangle), ("free-t", &exp.free_triangle)] {
            if route.distance.is_some() != route.score.is_some() {
                return Err(anyhow!(
                    "file {file:?} route {label:?}: `distance` and `score` must both be \
                     present (Answer expected) or both absent (NoAnswer expected)"
                ));
            }
        }
    }
    Ok(entries)
}

fn run_trial(case: &TestCase) -> Result<(), Failed> {
    let track = load_scoring_track(&case.igc_path).map_err(|err| {
        Failed::from(format!(
            "{}: failed to load track from {}: {err:#}",
            case.trial_name,
            case.igc_path.display(),
        ))
    })?;

    let pass1 = score_pass(&track);
    let mut breaches = compare_pass(&case.expectations, &pass1);

    let needs_rerun: Vec<Slot> = breaches
        .iter()
        .filter(|b| b.kind == BreachKind::Time)
        .map(|b| b.slot)
        .collect();

    if !needs_rerun.is_empty() {
        let pass2 = score_pass(&track);
        breaches.retain(|b| b.kind != BreachKind::Time);
        for slot in needs_rerun {
            let exp = case.expectations.for_slot(slot);
            let actual_ms = pass2.timing_for(slot);
            if !time_pass(exp.time, actual_ms) {
                breaches.push(Breach::time(slot, exp.time, actual_ms, true));
            }
        }
    }

    if breaches.is_empty() {
        Ok(())
    } else {
        Err(Failed::from(format_breaches(&case.trial_name, &breaches)))
    }
}

fn load_scoring_track(igc_path: &Path) -> anyhow::Result<ScoringTrack> {
    let track = parse_input(igc_path).with_context(|| format!("parsing {}", igc_path.display()))?;
    let window = find_flight_window(&track)
        .with_context(|| format!("detecting flight window in {}", igc_path.display()))?;
    let track = slice_flight_window(track, window);
    Ok(ScoringTrack {
        points: track.points.iter().map(PointE5::from_e5_coords).collect(),
    })
}

/// Mirrors `evaluate_routes`' flow, but runs the three scorers serially so
/// per-route timing is captured even on `NoAnswer` outcomes — the production
/// API only attaches `scored_ms` to `Answer` payloads, which would leave the
/// triangle-no-answer time fields in the manifest unverifiable.
struct Pass {
    fd: ScoringOutcome<Route>,
    fd_ms: u32,
    free_t: ScoringOutcome<Route>,
    free_t_ms: u32,
    fai_t: ScoringOutcome<Route>,
    fai_t_ms: u32,
}

impl Pass {
    fn timing_for(&self, slot: Slot) -> u32 {
        match slot {
            Slot::FreeDistance => self.fd_ms,
            Slot::FreeTriangle => self.free_t_ms,
            Slot::FaiTriangle => self.fai_t_ms,
        }
    }

    fn outcome_for(&self, slot: Slot) -> &ScoringOutcome<Route> {
        match slot {
            Slot::FreeDistance => &self.fd,
            Slot::FreeTriangle => &self.free_t,
            Slot::FaiTriangle => &self.fai_t,
        }
    }
}

fn score_pass(track: &ScoringTrack) -> Pass {
    let started = Instant::now();
    let fd = evaluate_free_distance(track);
    let fd_ms = ceil_ms(started.elapsed());

    // Triangle scorers take FD as a lower bound; if FD didn't answer we pass
    // 0 so they fall through to whatever NoAnswer / Error path matches what
    // `evaluate_routes` would surface.
    let fd_distance = match &fd {
        ScoringOutcome::Answer(route) => route.distance,
        _ => 0,
    };

    let started = Instant::now();
    let free_t = evaluate_free_triangle_lazy(track, fd_distance);
    let free_t_ms = ceil_ms(started.elapsed());

    let started = Instant::now();
    let fai_t = evaluate_fai_triangle_lazy(track, fd_distance, None, None);
    let fai_t_ms = ceil_ms(started.elapsed());

    Pass {
        fd,
        fd_ms,
        free_t,
        free_t_ms,
        fai_t,
        fai_t_ms,
    }
}

fn compare_pass(exp: &FileExpectations, pass: &Pass) -> Vec<Breach> {
    let mut breaches = Vec::new();
    for slot in [Slot::FaiTriangle, Slot::FreeTriangle, Slot::FreeDistance] {
        let route_exp = exp.for_slot(slot);
        let outcome = pass.outcome_for(slot);
        let actual_ms = pass.timing_for(slot);
        match (route_exp.distance, outcome) {
            (Some(expected_d), ScoringOutcome::Answer(route)) => {
                if !distance_pass(expected_d, route.distance) {
                    breaches.push(Breach::distance(slot, expected_d, route.distance));
                }
                if slot.should_checks_score()
                    && let Some(expected_s) = route_exp.score
                    && !score_pass_check(expected_s, route.score)
                {
                    breaches.push(Breach::score(slot, expected_s, route.score));
                }
            }
            (Some(_), ScoringOutcome::NoAnswer) => {
                breaches.push(Breach::shape(
                    slot,
                    "expected Answer, got NoAnswer".to_owned(),
                ));
            }
            (Some(_), ScoringOutcome::Error(err)) => {
                breaches.push(Breach::shape(
                    slot,
                    format!("expected Answer, got Error: {err}"),
                ));
            }
            (None, ScoringOutcome::Answer(route)) => {
                breaches.push(Breach::shape(
                    slot,
                    format!(
                        "expected NoAnswer, got Answer(distance={} m, score={:.2})",
                        route.distance, route.score,
                    ),
                ));
            }
            (None, ScoringOutcome::NoAnswer) => {}
            (None, ScoringOutcome::Error(err)) => {
                breaches.push(Breach::shape(
                    slot,
                    format!("expected NoAnswer, got Error: {err}"),
                ));
            }
        }
        if !time_pass(route_exp.time, actual_ms) {
            breaches.push(Breach::time(slot, route_exp.time, actual_ms, false));
        }
    }
    breaches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    FreeDistance,
    FreeTriangle,
    FaiTriangle,
}

impl Slot {
    fn label(self) -> &'static str {
        match self {
            Self::FaiTriangle => "fai-t",
            Self::FreeTriangle => "free-t",
            Self::FreeDistance => "fd",
        }
    }

    /// FD's score is mechanically `distance_km × 1.0`; checking it adds nothing
    /// the distance check doesn't already cover. Triangle scorers apply
    /// route-specific factors and need a real comparison.
    fn should_checks_score(self) -> bool {
        !matches!(self, Self::FreeDistance)
    }
}

#[derive(Debug)]
struct Breach {
    slot: Slot,
    kind: BreachKind,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreachKind {
    Shape,
    Distance,
    Score,
    Time,
}

impl Breach {
    fn shape(slot: Slot, message: String) -> Self {
        Self {
            slot,
            kind: BreachKind::Shape,
            message,
        }
    }

    fn distance(slot: Slot, expected: u32, actual: u32) -> Self {
        let delta = expected.abs_diff(actual);
        let pct = (delta as f64 / expected.max(1) as f64) * 100.0;
        Self {
            slot,
            kind: BreachKind::Distance,
            message: format!(
                "distance {actual} m vs expected {expected} m (delta {delta} m / {pct:.3}%)"
            ),
        }
    }

    fn score(slot: Slot, expected: f64, actual: f64) -> Self {
        let delta = (expected - actual).abs();
        let pct = (delta / expected.abs().max(0.5)) * 100.0;
        Self {
            slot,
            kind: BreachKind::Score,
            message: format!(
                "score {actual:.2} vs expected {expected:.2} (delta {delta:.2} / {pct:.3}%)"
            ),
        }
    }

    fn time(slot: Slot, expected: u32, actual: u32, after_rerun: bool) -> Self {
        let pct = if expected > 0 {
            (actual as f64 / expected as f64 - 1.0) * 100.0
        } else {
            f64::INFINITY
        };
        let suffix = if after_rerun { ", after rerun" } else { "" };
        Self {
            slot,
            kind: BreachKind::Time,
            message: format!(
                "time {actual} ms vs expected {expected} ms (slack {pct:+.0}%{suffix})"
            ),
        }
    }
}

fn format_breaches(trial_name: &str, breaches: &[Breach]) -> String {
    let mut out = String::new();
    writeln!(out, "{trial_name}").unwrap();
    for slot in [Slot::FaiTriangle, Slot::FreeTriangle, Slot::FreeDistance] {
        for breach in breaches.iter().filter(|b| b.slot == slot) {
            writeln!(out, "  {}: {}", slot.label(), breach.message).unwrap();
        }
    }
    out.trim_end().to_owned()
}

fn distance_pass(expected_m: u32, actual_m: u32) -> bool {
    let delta = expected_m.abs_diff(actual_m) as f64;
    let pct = (delta / expected_m.max(1) as f64) * 100.0;
    !(delta > 500.0 || pct > 0.5) || (delta <= 150.0)
}

fn score_pass_check(expected: f64, actual: f64) -> bool {
    let delta = (expected - actual).abs();
    let pct = (delta / expected.abs().max(0.5)) * 100.0;
    delta <= 0.5 || pct <= 0.5
}

fn time_pass(expected_ms: u32, actual_ms: u32) -> bool {
    if actual_ms <= 5 {
        return true;
    }
    actual_ms as f64 <= expected_ms.max(1) as f64 * 1.30
}

fn ceil_ms(elapsed: Duration) -> u32 {
    let micros = elapsed.as_micros();
    micros.div_ceil(1000).min(u32::MAX as u128) as u32
}

/// Walks every `tests/tracks/<bucket>/manifest.jsonc`, scores any `*.igc` that
/// sits next to a manifest but isn't yet a key in it, and appends the fresh
/// entries before the manifest's outermost `}`. Existing entries (and any
/// comments around them) are not touched.
fn run_fill() -> FillSummary {
    let root = Path::new(TRACKS_DIR);
    let mut summary = FillSummary::default();
    if !root.is_dir() {
        eprintln!("[fill] tracks dir not found: {}", root.display());
        return summary;
    }

    let manifests = walkdir::WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.file_name() == "manifest.jsonc");

    for entry in manifests {
        let manifest_path = entry.into_path();
        match fill_manifest(&manifest_path) {
            Ok(0) => {}
            Ok(added) => {
                eprintln!(
                    "[fill] {}: +{added} {}",
                    manifest_path.display(),
                    if added == 1 { "entry" } else { "entries" },
                );
                summary.manifests_changed += 1;
                summary.entries_added += added;
            }
            Err(err) => {
                eprintln!("[fill] {}: {err:#}", manifest_path.display());
                summary.had_failures = true;
            }
        }
    }

    eprintln!(
        "[fill] {} manifest(s) updated, {} new {}",
        summary.manifests_changed,
        summary.entries_added,
        if summary.entries_added == 1 {
            "entry"
        } else {
            "entries"
        },
    );
    summary
}

#[derive(Default)]
struct FillSummary {
    manifests_changed: usize,
    entries_added: usize,
    had_failures: bool,
}

fn fill_manifest(manifest_path: &Path) -> anyhow::Result<usize> {
    let raw = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let entries: BTreeMap<String, FileExpectations> =
        json5::from_str(&raw).with_context(|| format!("parsing {}", manifest_path.display()))?;
    let manifest_dir = manifest_path
        .parent()
        .context("manifest has no parent directory")?;

    let missing: Vec<String> = list_igc_stems(manifest_dir)?
        .into_iter()
        .filter(|stem| !entries.contains_key(stem))
        .collect();
    if missing.is_empty() {
        return Ok(0);
    }

    let mut new_entries: Vec<(String, FileExpectations)> = Vec::with_capacity(missing.len());
    for stem in missing {
        let igc_path = manifest_dir.join(format!("{stem}.igc"));
        match score_for_fill(&igc_path) {
            Ok(exp) => {
                eprintln!("[fill]   + {stem}");
                new_entries.push((stem, exp));
            }
            Err(err) => eprintln!("[fill]   ! {stem}: {err:#}"),
        }
    }
    if new_entries.is_empty() {
        return Ok(0);
    }

    let updated = inject_entries(&raw, &new_entries)?;
    std::fs::write(manifest_path, updated)
        .with_context(|| format!("writing {}", manifest_path.display()))?;
    Ok(new_entries.len())
}

fn list_igc_stems(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut stems = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("igc") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.push(stem.to_owned());
        }
    }
    stems.sort();
    Ok(stems)
}

fn score_for_fill(igc_path: &Path) -> anyhow::Result<FileExpectations> {
    let track = load_scoring_track(igc_path)?;
    let pass = score_pass(&track);
    Ok(FileExpectations {
        fai_triangle: route_expectations_from(Slot::FaiTriangle, &pass.fai_t, pass.fai_t_ms),
        free_triangle: route_expectations_from(Slot::FreeTriangle, &pass.free_t, pass.free_t_ms),
        fd: route_expectations_from(Slot::FreeDistance, &pass.fd, pass.fd_ms),
    })
}

fn route_expectations_from(
    slot: Slot,
    outcome: &ScoringOutcome<Route>,
    ms: u32,
) -> RouteExpectations {
    let (distance, score) = match outcome {
        ScoringOutcome::Answer(route) => {
            let score = if slot.should_checks_score() {
                Some(route.score)
            } else {
                None
            };
            (Some(route.distance), score)
        }
        _ => (None, None),
    };
    RouteExpectations {
        time: ms.max(1),
        distance,
        score,
    }
}

/// Renders new entries and inserts them just before the manifest's outermost
/// closing `}`. Trailing-comma handling: JSON5 tolerates trailing commas, so
/// the only real requirement is to add a separating `,` between the previously
/// last entry and our first new one when the previous entry didn't already end
/// in a comma.
fn inject_entries(raw: &str, new_entries: &[(String, FileExpectations)]) -> anyhow::Result<String> {
    let close_idx = raw
        .rfind('}')
        .context("manifest does not contain a closing `}`")?;
    let leading_full = &raw[..close_idx];
    let trailing = &raw[close_idx..];
    let leading = leading_full.trim_end_matches(|c: char| c.is_whitespace());

    // Empty manifest looks like `{ }` or `{}` once whitespace is trimmed.
    let was_empty = leading.ends_with('{');
    let needs_comma = !was_empty && !leading.ends_with(',');

    let mut out = String::with_capacity(raw.len() + 256 * new_entries.len());
    out.push_str(leading);
    if needs_comma {
        out.push(',');
    }
    for (i, (stem, exp)) in new_entries.iter().enumerate() {
        out.push('\n');
        write_file_expectations(&mut out, stem, exp);
        if i + 1 < new_entries.len() {
            out.push(',');
        }
    }
    out.push('\n');
    out.push_str(trailing);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn write_file_expectations(out: &mut String, stem: &str, exp: &FileExpectations) {
    writeln!(out, "  \"{stem}\": {{").unwrap();
    writeln!(
        out,
        "    \"fai-t\": {},",
        render_route_expectations(&exp.fai_triangle)
    )
    .unwrap();
    writeln!(
        out,
        "    \"free-t\": {},",
        render_route_expectations(&exp.free_triangle)
    )
    .unwrap();
    writeln!(out, "    \"fd\": {}", render_route_expectations(&exp.fd)).unwrap();
    write!(out, "  }}").unwrap();
}

fn render_route_expectations(exp: &RouteExpectations) -> String {
    let mut s = format!("{{ \"time\": {}", exp.time);
    if let Some(distance) = exp.distance {
        write!(&mut s, ", \"distance\": {distance}").unwrap();
    }
    if let Some(score) = exp.score {
        write!(&mut s, ", \"score\": {:.2}", score).unwrap();
    }
    s.push_str(" }");
    s
}
