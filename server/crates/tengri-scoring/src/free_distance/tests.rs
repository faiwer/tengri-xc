use super::constants::FREE_DISTANCE_MULTIPLIER;
use super::solver::{find_best_free_distance_dp, squeeze_route};
use super::track::DedupeTrack;
use super::types::{FreeDistanceScore, route_point};
use super::*;
use crate::ScoringTrack;
use crate::{Route, RouteSubType, RouteType, ScoringOutcome};
use tengri_geo::{METERS_PER_KM, PointE5 as Point};

fn point(_time: u32, lat: i32, lon: i32) -> Point {
    Point { lat, lon }
}

fn track(points: Vec<Point>) -> ScoringTrack {
    ScoringTrack { points }
}

fn five_point_track() -> ScoringTrack {
    track(vec![
        point(0, 0, 0),
        point(1, 0, 100_000),
        point(2, 100_000, 100_000),
        point(3, 100_000, 0),
        point(4, 200_000, 0),
    ])
}

fn answer(outcome: ScoringOutcome<Route>) -> Route {
    match outcome {
        ScoringOutcome::Answer(route) => route,
        other => panic!("expected free distance answer, got {other:?}"),
    }
}

#[test]
fn fewer_than_five_fixes_has_no_answer() {
    assert!(matches!(
        evaluate_free_distance(&track(vec![
            point(0, 0, 0),
            point(1, 0, 100_000),
            point(2, 100_000, 100_000),
            point(3, 100_000, 0),
        ])),
        ScoringOutcome::NoAnswer,
    ));
}

#[test]
fn route_metadata_score_and_rounding_match_free_distance_rules() {
    let source = five_point_track();
    let route = route_result(FreeDistanceScore {
        turnpoints: source
            .points
            .iter()
            .enumerate()
            .map(|(idx, point)| route_point(idx, point))
            .collect(),
    });
    let raw_distance = route.leg_distances.iter().copied().sum::<u32>();

    assert_eq!(route.route_type, RouteType::FreeDistance);
    assert_eq!(route.sub_type, RouteSubType::None);
    assert_eq!(route.closure, None);
    assert_eq!(route.factor, FREE_DISTANCE_MULTIPLIER);
    assert!(route.optimal);
    assert_eq!(route.turnpoints.len(), 5);
    assert_eq!(route.leg_distances.len(), 4);
    assert_eq!(route.distance, round_final_distance_m(raw_distance));
    assert_eq!(
        route.score,
        (route.distance as f64 / METERS_PER_KM) * FREE_DISTANCE_MULTIPLIER,
    );
}

#[test]
fn public_scorer_returns_the_only_possible_five_point_route() {
    let route = answer(evaluate_free_distance(&five_point_track()));
    let indexes = route
        .turnpoints
        .iter()
        .map(|point| point.idx)
        .collect::<Vec<_>>();

    assert_eq!(indexes, vec![0, 1, 2, 3, 4]);
}

#[test]
fn scoring_track_dedupes_consecutive_positions_and_remaps_indexes() {
    let source = track(vec![
        point(0, 0, 0),
        point(1, 0, 0),
        point(2, 0, 100_000),
        point(3, 0, 100_000),
        point(4, 100_000, 100_000),
    ]);
    let scoring_track = DedupeTrack::new(&source);

    assert_eq!(
        scoring_track
            .track()
            .points
            .iter()
            .map(|point| point.lon)
            .collect::<Vec<_>>(),
        vec![0, 100_000, 100_000],
    );

    let remapped = scoring_track.remap_score(FreeDistanceScore {
        turnpoints: scoring_track
            .track()
            .points
            .iter()
            .enumerate()
            .map(|(idx, point)| route_point(idx, point))
            .collect(),
    });

    assert_eq!(
        remapped
            .turnpoints
            .iter()
            .map(|point| point.idx)
            .collect::<Vec<_>>(),
        vec![0, 2, 4],
    );
}

#[test]
fn squeeze_route_keeps_ordered_windows_around_route_points() {
    assert_eq!(
        squeeze_route(20, &[0, 5, 10, 15, 19], 10.0),
        vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19
        ],
    );
    assert_eq!(squeeze_route(10, &[5], 0.1), vec![4, 5, 6]);
}

#[test]
fn dp_finds_best_ordered_five_point_route_inside_seed_set() {
    let points = [
        point(0, 0, 0),
        point(1, 0, 1_000),
        point(2, 0, 100_000),
        point(3, 100_000, 100_000),
        point(4, 100_000, 0),
        point(5, 200_000, 0),
    ]
    .iter()
    .map(Point::from_e5_coords)
    .collect::<Vec<_>>();

    assert_eq!(
        find_best_free_distance_dp(&points, &[0, 1, 2, 3, 4, 5]),
        Some([0, 2, 3, 4, 5]),
    );
}

#[test]
fn dp_rejects_too_few_seed_points() {
    let points = five_point_track()
        .points
        .iter()
        .map(Point::from_e5_coords)
        .collect::<Vec<_>>();

    assert_eq!(find_best_free_distance_dp(&points, &[0, 1, 2, 3]), None);
}
