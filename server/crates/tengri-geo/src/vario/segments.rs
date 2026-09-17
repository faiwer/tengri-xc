//! Merging per-fix buckets into runs worth colouring.

/// A run of fixes sharing one bucket. `end_idx` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarioSegment {
    pub bucket: i8,
    pub start_idx: usize,
    pub end_idx: usize,
}

/// A run this short between two runs of the same bucket is a disruption, not
/// a change of air.
const REABSORB_INTERIOR_SECONDS: u32 = 8;
/// Below this a run is a blip and belongs to whichever side dominates.
const MIN_SEGMENT_SECONDS: u32 = 15;

/// Group `buckets[from_idx..to_idx]` into runs: coalesce equal neighbours,
/// reabsorb brief interior excursions, then dissolve what is still too short
/// into its longer neighbour.
///
/// `times` are Unix epoch seconds, so the duration thresholds hold whatever
/// the sample rate is.
pub fn build_vario_segments(
    buckets: &[i8],
    times: &[u32],
    from_idx: usize,
    to_idx: usize,
) -> Vec<VarioSegment> {
    let to_idx = to_idx.min(buckets.len().min(times.len()));
    if to_idx <= from_idx {
        return Vec::new();
    }

    let segments = coalesce(buckets, from_idx, to_idx);
    let segments = reabsorb_interior_runs(segments, times);
    dissolve_short_runs(segments, times)
}

fn coalesce(buckets: &[i8], from_idx: usize, to_idx: usize) -> Vec<VarioSegment> {
    let mut segments = Vec::new();
    let mut start_idx = from_idx;
    let mut bucket = buckets[from_idx];

    for (idx, &next) in buckets.iter().enumerate().take(to_idx).skip(from_idx + 1) {
        if next != bucket {
            segments.push(VarioSegment {
                bucket,
                start_idx,
                end_idx: idx,
            });
            start_idx = idx;
            bucket = next;
        }
    }
    segments.push(VarioSegment {
        bucket,
        start_idx,
        end_idx: to_idx,
    });
    segments
}

fn reabsorb_interior_runs(segments: Vec<VarioSegment>, times: &[u32]) -> Vec<VarioSegment> {
    if segments.len() < 3 {
        return segments;
    }

    let mut out = vec![segments[0]];
    let mut idx = 1;
    while idx < segments.len() - 1 {
        let previous = *out.last().unwrap();
        let current = segments[idx];
        let next = segments[idx + 1];
        let brief = duration_s(current, times) <= REABSORB_INTERIOR_SECONDS;

        if previous.bucket == next.bucket && current.bucket != previous.bucket && brief {
            *out.last_mut().unwrap() = VarioSegment {
                bucket: previous.bucket,
                start_idx: previous.start_idx,
                end_idx: next.end_idx,
            };
            idx += 1;
        } else {
            out.push(current);
        }
        idx += 1;
    }

    let last = *segments.last().unwrap();
    if out.last().unwrap().end_idx != last.end_idx {
        out.push(last);
    }
    out
}

/// Worst case is quadratic, over a segment count in the hundreds.
fn dissolve_short_runs(segments: Vec<VarioSegment>, times: &[u32]) -> Vec<VarioSegment> {
    let mut out = segments;

    let mut made_progress = true;
    while made_progress {
        made_progress = false;
        for idx in 0..out.len() {
            if duration_s(out[idx], times) >= MIN_SEGMENT_SECONDS {
                continue;
            }
            let Some(neighbour) = pick_longer_neighbour(&out, idx, times) else {
                break;
            };

            let merged = VarioSegment {
                bucket: out[neighbour].bucket,
                start_idx: out[idx].start_idx.min(out[neighbour].start_idx),
                end_idx: out[idx].end_idx.max(out[neighbour].end_idx),
            };
            let low = idx.min(neighbour);
            let high = idx.max(neighbour);
            out.splice(low..=high, [merged]);
            merge_with_same_bucket_neighbours(&mut out, low);
            made_progress = true;
            break;
        }
    }

    out
}

fn pick_longer_neighbour(segments: &[VarioSegment], idx: usize, times: &[u32]) -> Option<usize> {
    let left = idx.checked_sub(1);
    let right = (idx + 1 < segments.len()).then_some(idx + 1);

    match (left, right) {
        (None, None) => None,
        (None, Some(right)) => Some(right),
        (Some(left), None) => Some(left),
        (Some(left), Some(right)) => {
            if duration_s(segments[right], times) > duration_s(segments[left], times) {
                Some(right)
            } else {
                Some(left)
            }
        }
    }
}

fn merge_with_same_bucket_neighbours(segments: &mut Vec<VarioSegment>, idx: usize) {
    if idx + 1 < segments.len() && segments[idx].bucket == segments[idx + 1].bucket {
        segments[idx].end_idx = segments[idx + 1].end_idx;
        segments.remove(idx + 1);
    }
    if idx > 0 && segments[idx - 1].bucket == segments[idx].bucket {
        segments[idx - 1].end_idx = segments[idx].end_idx;
        segments.remove(idx);
    }
}

fn duration_s(segment: VarioSegment, times: &[u32]) -> u32 {
    times[segment.end_idx - 1].saturating_sub(times[segment.start_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 Hz timestamps.
    fn seconds(count: usize) -> Vec<u32> {
        (0..count as u32).collect()
    }

    /// `[(bucket, length)]` flattened into a per-fix bucket column.
    fn buckets(runs: &[(i8, usize)]) -> Vec<i8> {
        runs.iter()
            .flat_map(|&(bucket, length)| std::iter::repeat_n(bucket, length))
            .collect()
    }

    #[test]
    fn equal_neighbours_coalesce_into_one_run() {
        let buckets = buckets(&[(2, 60), (-1, 60)]);

        let segments = build_vario_segments(&buckets, &seconds(120), 0, 120);

        assert_eq!(
            segments,
            vec![
                VarioSegment {
                    bucket: 2,
                    start_idx: 0,
                    end_idx: 60
                },
                VarioSegment {
                    bucket: -1,
                    start_idx: 60,
                    end_idx: 120
                },
            ]
        );
    }

    #[test]
    fn a_brief_excursion_between_equal_runs_is_reabsorbed() {
        let buckets = buckets(&[(2, 60), (-3, 5), (2, 60)]);

        let segments = build_vario_segments(&buckets, &seconds(125), 0, 125);

        assert_eq!(
            segments,
            vec![VarioSegment {
                bucket: 2,
                start_idx: 0,
                end_idx: 125
            }]
        );
    }

    #[test]
    fn a_short_run_joins_its_longer_neighbour() {
        let buckets = buckets(&[(1, 20), (4, 10), (-2, 90)]);

        let segments = build_vario_segments(&buckets, &seconds(120), 0, 120);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].bucket, -2);
        // The 10 s run of `4` went to the 90 s side, not the 20 s one.
        assert_eq!(segments[1].start_idx, 20);
    }

    #[test]
    fn an_empty_range_has_no_segments() {
        let buckets = buckets(&[(2, 60)]);

        assert!(build_vario_segments(&buckets, &seconds(60), 30, 30).is_empty());
    }

    #[test]
    fn segments_stay_inside_the_requested_range() {
        let buckets = buckets(&[(-1, 60), (2, 60), (-1, 60)]);

        let segments = build_vario_segments(&buckets, &seconds(180), 120, 180);

        assert_eq!(
            segments,
            vec![VarioSegment {
                bucket: -1,
                start_idx: 120,
                end_idx: 180
            }]
        );
    }
}
