import type { Track } from './types';
import { haversineM } from '../utils/geo/haversine';

const MS_TO_KMH = 3.6;

/**
 * Half-width of the centred displacement window in seconds. ±30 s
 * covers at least one full thermal turn (free-flight circle period
 * lands at 20–30 s for hang gliders and paragliders), so the wing's
 * contribution to ground motion averages to zero around each turn
 * and what the formula reports is the wind drift component — actual
 * ground speed.
 *
 * Wider windows would cancel slower-circling sailplanes too but lag
 * thermal entry/exit transitions more visibly in the chart. ±30 s is
 * the sweet spot for the free-flight tracks this app targets.
 */
const WINDOW_HALF_SECONDS = 30;

/**
 * Per-fix ground speed in km/h, computed as straight-line displacement
 * across a centred ±30 s window divided by the elapsed time of that
 * window: `haversine(p[L], p[R]) / (t[R] − t[L])`.
 *
 * The formula deliberately uses *displacement* (chord between the two
 * window endpoints), not *path length* (sum of per-leg chords). The
 * difference matters in turning flight: a closed thermal circle has a
 * path length of ~50 km/h × 30 s ≈ 415 m but a displacement of only
 * the wind drift over those 30 s. Reporting displacement gives the
 * pilot's actual cross-country speed; reporting path length gives
 * airspeed, which is what {@link computePathSpeed} is for.
 *
 * Window edges are clamped to track bounds, so the first/last ~30 s of
 * fixes use an asymmetric, narrower window. After downstream
 * mean-bucketing into ~1500 samples those edge buckets each cover
 * several seconds anyway, so the asymmetry is invisible in the chart.
 *
 * Output is km/h to match the TAS channel and pilot intuition.
 */
export const computeGroundSpeed = (track: Track): Float32Array => {
  const { lat, lng, t: times } = track;
  const fixCount = times.length;
  const speed = new Float32Array(fixCount);
  if (fixCount < 2) {
    return speed;
  }

  // Two-pointer sweep: L and R chase the centred window edges as i
  // walks forward. Both indices only ever advance, so the whole pass
  // is O(n) regardless of sampling rate.
  let lo = 0;
  let hi = 0;
  for (let i = 0; i < fixCount; i++) {
    const tCenter = times[i]!;
    const tLo = tCenter - WINDOW_HALF_SECONDS;
    const tHi = tCenter + WINDOW_HALF_SECONDS;

    while (lo < i && times[lo]! < tLo) {
      lo++;
    }
    while (hi + 1 < fixCount && times[hi + 1]! <= tHi) {
      hi++;
    }

    const dt = times[hi]! - times[lo]!;
    if (dt <= 0) {
      // Single-fix window (edge case at fixCount=1 was handled above,
      // but a degenerate burst of duplicate timestamps could land us
      // here) — copy the previous sample to keep the array aligned.
      speed[i] = i > 0 ? speed[i - 1]! : 0;
      continue;
    }
    const distM = haversineM(lat[lo]!, lng[lo]!, lat[hi]!, lng[hi]!);
    speed[i] = (distM / dt) * MS_TO_KMH;
  }

  return speed;
};
