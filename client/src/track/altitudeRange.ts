import type { Track } from './types';

export interface AltitudeRange {
  /** Lowest altitude observed in the range, in metres. */
  minAlt: number;
  /** Highest altitude observed in the range, in metres. */
  maxAlt: number;
}

/**
 * Min and max altitude (MSL) over `[fromIdx, toIdx)`, in metres.
 *
 * Reads GPS altitude rather than baro. Both are stored on the track, but
 * they answer different questions: baro is smoother (~±0.3 m sample noise
 * vs ~±5–15 m for GPS) but shifts en bloc when the day's QNH differs from
 * the standard atmosphere — easily ±50 m. GPS is noisier per-fix but
 * unbiased on average, so its extremes match what a pilot would call
 * their absolute altitude AMSL. For a "highest point reached" panel, the
 * unbiased reading is the better truth.
 *
 * No smoothing — averaging would shave metres off any genuinely sharp
 * crossing the pilot actually flew. The few-metre positive bias on the
 * max from GPS noise is dwarfed by the QNH bias baro would impose.
 *
 * Returns `{ minAlt: 0, maxAlt: 0 }` for an empty range. The unit is
 * whole metres; sub-metre precision is meaningless at airframe scale.
 */
export const altitudeRange = (
  track: Track,
  fromIdx: number,
  toIdx: number,
): AltitudeRange => {
  if (fromIdx >= toIdx) {
    return { minAlt: 0, maxAlt: 0 };
  }

  const altDm = track.alt;
  let minDm = altDm[fromIdx]!;
  let maxDm = minDm;
  for (let i = fromIdx + 1; i < toIdx; i++) {
    const v = altDm[i]!;
    if (v < minDm) {
      minDm = v;
    } else if (v > maxDm) {
      maxDm = v;
    }
  }

  return { minAlt: Math.round(minDm / 10), maxAlt: Math.round(maxDm / 10) };
};
