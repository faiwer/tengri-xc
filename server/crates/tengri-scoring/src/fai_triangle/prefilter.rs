use crate::{ScoringOutcome, ScoringTrack};
use tengri_geo::{simplify_track_for_scoring_with_chord_cap, track_aspect_ratio};

pub(super) const MIN_FREE_DISTANCE_M: u32 = 5_000;
pub(super) const MAX_ASPECT_RATIO: f64 = 8.0;
pub(super) const PROBE_RDP_TOLERANCE_M: f64 = 250.0;
pub(super) const CHORD_RDP_CAP_M: f64 = 500.0;
pub(super) const MIN_COARSE_DISTANCE_M: u32 = 10_000;
pub(super) const MIN_COARSE_TO_FREE_DISTANCE_RATIO: f64 = 0.25;

/// Why the lazy evaluator skipped the full FAI triangle search.
#[derive(Debug, Clone, Copy)]
pub enum FaiTriangleLazySkipReason {
    FreeDistanceTooSmall,
    AspectRatioTooHigh,
    /// The simplified-track probe found no FAI triangle at all.
    CoarseFaiNoAnswer,
    CoarseFaiTooSmall,
    CoarseFaiTooSmallVsFreeDistance,
}

/// Diagnostic data collected by {@link evaluate_fai_triangle_lazy}.
#[derive(Default)]
pub struct FaiTriangleLazyAudit {
    pub aspect_ratio: Option<f64>,
    pub simplified_points: Option<usize>,
    pub coarse_distance_m: Option<u32>,
    pub coarse_to_free_distance_ratio: Option<f64>,
    /// `Some` if evaluation was skipped.
    pub skip_reason: Option<FaiTriangleLazySkipReason>,
}

/// Returns `true` if the track is a plausible FAI triangle candidate and the
/// full search should proceed. Returns `false` (and sets `audit.skip_reason`)
/// if the track passes one of the early-exit filters.
pub(super) fn is_valuable(
    track: &ScoringTrack,
    free_distance_m: u32,
    audit: Option<&mut FaiTriangleLazyAudit>,
) -> bool {
    if free_distance_m < MIN_FREE_DISTANCE_M {
        // Too small triangles:
        // - Makes no sense to score
        // - Improprotionally expensive to score
        if let Some(a) = audit {
            a.skip_reason = Some(FaiTriangleLazySkipReason::FreeDistanceTooSmall);
        }
        return false;
    }

    let aspect_ratio = track_aspect_ratio(&track.points);
    if aspect_ratio.is_some_and(|r| r >= MAX_ASPECT_RATIO) {
        // Very elongated tracks are almost never real FAI candidates: the good
        // route is a long line, while any triangle is a small side-effect. They
        // are also the cases where exact triangle search can explode.
        if let Some(a) = audit {
            a.aspect_ratio = aspect_ratio;
            a.skip_reason = Some(FaiTriangleLazySkipReason::AspectRatioTooHigh);
        }
        return false;
    }

    let simplified = simplified_track(track, PROBE_RDP_TOLERANCE_M, CHORD_RDP_CAP_M);
    let coarse_fai_distance_m = match super::probe_fai_triangle(&simplified) {
        ScoringOutcome::Answer(route) => route.distance,
        _ => {
            if let Some(a) = audit {
                a.aspect_ratio = aspect_ratio;
                a.simplified_points = Some(simplified.points.len());
                a.coarse_distance_m = Some(0);
                a.skip_reason = Some(FaiTriangleLazySkipReason::CoarseFaiNoAnswer);
            }
            // We could not find a FAI triangle in the simplified track with a
            // significantly bigger closure. I.e., the FAI triangle doesn't
            // exist.
            return false;
        }
    };

    let coarse_to_free_distance_ratio = (free_distance_m > 0)
        .then(|| f64::from(coarse_fai_distance_m) / f64::from(free_distance_m));

    // The simplified FAI pass is a cheap "is there anything worth pursuing?"
    // probe. It can understate the exact triangle, but if the coarse candidate
    // is still tiny, exact scoring buys precision for a result we do not care
    // to compete on.
    let skip_reason = if coarse_fai_distance_m < MIN_COARSE_DISTANCE_M {
        Some(FaiTriangleLazySkipReason::CoarseFaiTooSmall)
    } else if coarse_to_free_distance_ratio
        .is_some_and(|ratio| ratio < MIN_COARSE_TO_FREE_DISTANCE_RATIO)
    {
        // Even a non-tiny triangle is not useful if it is a small fraction of
        // the flight's free distance. These are usually long flights with a
        // token triangle, where exact search can be expensive and the outcome
        // is not valuable enough to audit.
        Some(FaiTriangleLazySkipReason::CoarseFaiTooSmallVsFreeDistance)
    } else {
        None
    };

    let valuable = skip_reason.is_none();

    if let Some(a) = audit {
        a.aspect_ratio = aspect_ratio;
        a.simplified_points = Some(simplified.points.len());
        a.coarse_distance_m = Some(coarse_fai_distance_m);
        a.coarse_to_free_distance_ratio = coarse_to_free_distance_ratio;
        a.skip_reason = skip_reason;
    }

    valuable
}

fn simplified_track(track: &ScoringTrack, tolerance_m: f64, chord_cap_m: f64) -> ScoringTrack {
    let indexes =
        simplify_track_for_scoring_with_chord_cap(&track.points, tolerance_m, chord_cap_m);
    track.select_at(indexes)
}
