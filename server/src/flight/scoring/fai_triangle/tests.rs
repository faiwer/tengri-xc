use super::constants::{FAI_TRIANGLE_CLOSED_MULTIPLIER, FAI_TRIANGLE_OPEN_MULTIPLIER};
use super::*;
use crate::flight::scoring::{Route, RouteSubType, ScoringOutcome};
use crate::flight::types::TrackPoint;
use crate::geo::METERS_PER_KM;

fn point(time: u32, lat: i32, lon: i32) -> TrackPoint {
    TrackPoint {
        time,
        lat,
        lon,
        geo_alt: 0,
        pressure_alt: None,
        tas: None,
    }
}

fn triangle_track(return_lon: i32) -> Track {
    Track {
        start_time: 0,
        points: vec![
            point(0, 0, 0),
            point(1, 0, 90_000),
            point(2, 77_942, 45_000),
            point(3, 0, return_lon),
        ],
    }
}

fn answer(outcome: ScoringOutcome<Route>) -> Route {
    match outcome {
        ScoringOutcome::Answer(route) => route,
        other => panic!("expected FAI triangle answer, got {other:?}"),
    }
}

#[test]
fn strict_closure_rejects_borderline_relaxed_accepts() {
    let track = triangle_track(58_000);

    assert!(matches!(
        evaluate_fai_triangle(&track, Some(FaiTriangleClass::Open)),
        ScoringOutcome::NoAnswer,
    ));
    let relaxed = answer(probe_fai_triangle(&track));

    assert!(relaxed.distance > 0);
}

#[test]
fn relaxed_matches_strict_on_clean_closure_flight() {
    let track = triangle_track(27_000);

    let strict = answer(evaluate_fai_triangle(&track, Some(FaiTriangleClass::Open)));
    let relaxed = answer(probe_fai_triangle(&track));

    assert_eq!(
        strict.distance, relaxed.distance,
        "relaxed should match strict when the optimum's closure is well below both thresholds",
    );
}

#[test]
fn route_distance_and_score_subtract_closure() {
    let route = answer(evaluate_fai_triangle(
        &triangle_track(27_000),
        Some(FaiTriangleClass::Open),
    ));
    let raw_perimeter = route.leg_distances.iter().copied().sum::<u32>();
    let closure_distance = route
        .closure
        .as_ref()
        .expect("FAI triangle should carry closure details")
        .distance;

    assert_eq!(route.distance, raw_perimeter - closure_distance);
    assert_eq!(
        route.score,
        ((route.distance as f64 / METERS_PER_KM) * FAI_TRIANGLE_OPEN_MULTIPLIER * 100.0).round()
            / 100.0,
    );
}

#[test]
fn explicit_closed_class_uses_closed_subtype_and_multiplier() {
    let route = answer(evaluate_fai_triangle(
        &triangle_track(5_000),
        Some(FaiTriangleClass::Closed),
    ));

    assert_eq!(route.sub_type, RouteSubType::OlcClosed);
    assert_eq!(route.factor, FAI_TRIANGLE_CLOSED_MULTIPLIER);
    assert_eq!(
        route.score,
        (route.distance as f64 / METERS_PER_KM) * FAI_TRIANGLE_CLOSED_MULTIPLIER,
    );
}

#[test]
fn explicit_open_class_uses_open_subtype_and_multiplier() {
    let route = answer(evaluate_fai_triangle(
        &triangle_track(27_000),
        Some(FaiTriangleClass::Open),
    ));

    assert_eq!(route.sub_type, RouteSubType::OlcOpen);
    assert_eq!(route.factor, FAI_TRIANGLE_OPEN_MULTIPLIER);
}

#[test]
fn shape_rule_rejects_thin_triangle() {
    // Three nearly collinear points: the two shorter legs are each ~25% of the
    // perimeter, below the 28% minimum-side floor.
    let track = Track {
        start_time: 0,
        points: vec![
            point(0, 0, 0),
            point(1, 1_000, 50_000), // 0.01°N, 0.5°E — barely off the axis
            point(2, 0, 100_000),
            point(3, 0, 0), // return to origin for a zero closure gap
        ],
    };
    assert!(matches!(
        evaluate_fai_triangle(&track, Some(FaiTriangleClass::Open)),
        ScoringOutcome::NoAnswer,
    ));
}

#[test]
fn straight_track_has_no_triangle() {
    // All fixes on the equator — every candidate triangle is degenerate (one
    // leg equals the sum of the other two), so the 28% shape rule kills all
    // of them.
    let track = Track {
        start_time: 0,
        points: vec![
            point(0, 0, 0),
            point(1, 0, 33_333),
            point(2, 0, 66_666),
            point(3, 0, 33_333),
            point(4, 0, 0),
        ],
    };
    assert!(matches!(
        evaluate_fai_triangle(&track, Some(FaiTriangleClass::Open)),
        ScoringOutcome::NoAnswer,
    ));
}

#[test]
fn min_scoring_side_filters_small_triangle() {
    // The track forms a valid FAI triangle (~80–100 km sides), but an
    // artificially high floor of 1000 km should suppress it entirely.
    let track = triangle_track(0);
    assert!(matches!(
        evaluate_fai_triangle_with_min_side(&track, Some(FaiTriangleClass::Open), 1_000.0),
        ScoringOutcome::NoAnswer,
    ));
}
