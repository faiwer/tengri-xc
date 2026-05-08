import type { Track } from './types';
import { haversineM } from '../utils/geo/haversine';

const MS_TO_KMH = 3.6;

/**
 * Per-fix path speed in km/h: `haversine(p[i-1], p[i]) / Δt`. The
 * length of the polyline drawn by the GPS, sampled per second, divided
 * by Δt — i.e. *how fast the trace itself is being drawn*.
 *
 * This is **not** displacement-based ground speed. Inside a thermal
 * turn the net displacement collapses to wind drift, but path speed
 * keeps reading ~airspeed because we sum chord lengths leg-by-leg.
 * That property is exactly why path speed doubles as a synthetic
 * true-airspeed signal when averaged over a turn — the wind component
 * along each chord cancels by symmetry around the circle.
 *
 * On a straight glide path speed equals ground speed (one is the
 * other's per-leg form), so this signal is *not* a TAS estimate
 * during cruise — only inside turns, where it converges.
 *
 * `speed[0]` mirrors `speed[1]` so the array stays aligned 1:1 with
 * the source track and downstream slicing/bucketing doesn't have to
 * special-case the first sample.
 */
export const computePathSpeed = (track: Track): Float32Array => {
  const { lat, lng, t: times } = track;
  const fixCount = times.length;
  const speed = new Float32Array(fixCount);
  if (fixCount < 2) {
    return speed;
  }

  for (let i = 1; i < fixCount; i++) {
    const dt = times[i]! - times[i - 1]!;
    if (dt <= 0) {
      // Duplicate timestamps shouldn't happen in well-formed tracks,
      // but defensively reuse the previous sample so we never emit a
      // NaN or Infinity into the chart pipeline.
      speed[i] = speed[i - 1]!;
      continue;
    }
    const distM = haversineM(lat[i - 1]!, lng[i - 1]!, lat[i]!, lng[i]!);
    speed[i] = (distM / dt) * MS_TO_KMH;
  }
  speed[0] = speed[1]!;
  return speed;
};
