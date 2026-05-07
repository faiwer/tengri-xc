import type { Track } from './types';

/**
 * Find the index of the first track fix at-or-after `epochSeconds`.
 *
 * The `track.t` series is monotonically non-decreasing (1 Hz IGC, possibly
 * with the odd duplicate timestamp), so binary search is valid. Returns
 * `track.t.length` if every fix is strictly before the target.
 */
export const findIndexAt = (track: Track, epochSeconds: number): number => {
  const times = track.t;
  let lo = 0;
  let hi = times.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (times[mid]! < epochSeconds) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo;
};
