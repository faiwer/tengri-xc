use super::*;
use crate::flight::scoring::{Route, RouteSubType, RouteType, ScoringOutcome};
use crate::flight::types::{Track, TrackPoint};
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
        other => panic!("expected free triangle answer, got {other:?}"),
    }
}

#[test]
fn explicit_open_class_uses_free_triangle_type_subtype_and_multiplier() {
    let route = answer(evaluate_free_triangle_class(
        &triangle_track(27_000),
        OlcTriangleClass::Open,
    ));

    assert_eq!(route.route_type, RouteType::FreeTriangle);
    assert_eq!(route.sub_type, RouteSubType::OlcOpen);
    assert_eq!(route.factor, FREE_TRIANGLE_OPEN_MULTIPLIER);
}

#[test]
fn explicit_closed_class_uses_free_triangle_type_subtype_and_multiplier() {
    let route = answer(evaluate_free_triangle_class(
        &triangle_track(5_000),
        OlcTriangleClass::Closed,
    ));

    assert_eq!(route.route_type, RouteType::FreeTriangle);
    assert_eq!(route.sub_type, RouteSubType::OlcClosed);
    assert_eq!(route.factor, FREE_TRIANGLE_CLOSED_MULTIPLIER);
}

#[test]
fn route_distance_and_score_subtract_closure() {
    let route = answer(evaluate_free_triangle_class(
        &triangle_track(27_000),
        OlcTriangleClass::Open,
    ));
    let raw_perimeter = route.leg_distances.iter().copied().sum::<u32>();
    let closure_distance = route
        .closure
        .as_ref()
        .expect("free triangle should carry closure details")
        .distance;

    assert_eq!(route.distance, raw_perimeter - closure_distance);
    assert_eq!(
        route.score,
        (route.distance as f64 / METERS_PER_KM) * FREE_TRIANGLE_OPEN_MULTIPLIER,
    );
}

#[test]
fn combined_wrapper_prefers_closed_when_closed_score_is_better() {
    let route = answer(evaluate_free_triangle(&triangle_track(5_000)));

    assert_eq!(route.sub_type, RouteSubType::OlcClosed);
    assert_eq!(route.factor, FREE_TRIANGLE_CLOSED_MULTIPLIER);
}

#[test]
fn lazy_scoring_filters_free_triangles_below_free_distance_floor() {
    let track = triangle_track(27_000);
    let route = answer(evaluate_free_triangle(&track));
    let blocking_free_distance_m =
        ((f64::from(route.distance) / MIN_FREE_TO_FREE_DISTANCE_RATIO).floor() as u32) + 1;

    assert!(matches!(
        evaluate_free_triangle_lazy(&track, blocking_free_distance_m),
        ScoringOutcome::NoAnswer,
    ));
}
