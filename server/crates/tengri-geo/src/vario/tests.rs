//! The three passes together: slope, buckets, segments. Each submodule tests
//! its own pass in isolation; these check the shape of the whole pipeline.

use super::*;

/// `[(seconds, climb_rate_mps)]` at 1 Hz, starting from 1000 m.
fn legs(legs: &[(u32, f64)]) -> (Vec<u32>, Vec<i32>) {
    let mut times = Vec::new();
    let mut alt_dm = Vec::new();
    let mut time = 0;
    let mut altitude_m: f64 = 1000.0;

    for &(seconds, rate) in legs {
        for _ in 0..seconds {
            times.push(time);
            alt_dm.push((altitude_m * 10.0).round() as i32);
            time += 1;
            altitude_m += rate;
        }
    }
    (times, alt_dm)
}

fn segments_of(times: &[u32], alt_dm: &[i32]) -> Vec<VarioSegment> {
    let buckets = classify_buckets(&vario_mps(times, alt_dm));
    build_vario_segments(&buckets, times, 0, times.len())
}

#[test]
fn climb_glide_climb_splits_into_three_runs() {
    let (times, alt_dm) = legs(&[(120, 2.0), (120, -1.0), (120, 3.0)]);

    let buckets: Vec<i8> = segments_of(&times, &alt_dm)
        .iter()
        .map(|segment| segment.bucket)
        .collect();

    assert!(buckets.contains(&2), "{buckets:?}");
    assert!(buckets.contains(&-1), "{buckets:?}");
    assert!(buckets.contains(&3), "{buckets:?}");
}

#[test]
fn a_blip_of_sink_does_not_cut_a_thermal_in_two() {
    let (times, alt_dm) = legs(&[(60, 2.0), (5, -3.0), (60, 2.0)]);

    let segments = segments_of(&times, &alt_dm);
    let longest = segments
        .iter()
        .max_by_key(|segment| segment.end_idx - segment.start_idx)
        .unwrap();

    assert_eq!(longest.bucket, 2);
}

#[test]
fn a_screaming_climb_clamps_to_the_top_bucket() {
    let (times, alt_dm) = legs(&[(60, 1.0), (60, 9.0), (60, 1.0)]);

    let segments = segments_of(&times, &alt_dm);

    assert!(segments.iter().any(|segment| segment.bucket == MAX_BUCKET));
}

#[test]
fn a_spiral_dive_clamps_to_the_bottom_bucket() {
    let (times, alt_dm) = legs(&[(60, -1.0), (60, -8.0), (60, -1.0)]);

    let segments = segments_of(&times, &alt_dm);

    assert!(segments.iter().any(|segment| segment.bucket == MIN_BUCKET));
}
