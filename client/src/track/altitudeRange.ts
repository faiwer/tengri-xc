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
 * Reads GPS altitude. Baro can also be stored on the track but is *pressure*
 * altitude — computed against the ISA standard 1013.25 hPa rather than the
 * day's actual QNH. On a typical non-standard-pressure day the baro reading
 * drifts ~50–150 m from the true MSL altitude, and the offset grows with
 * altitude (thinner air → larger error per hPa), so it's not a constant
 * subtractable bias either. GPS reports geometric altitude directly, with ~5–15
 * m per-fix noise but no systematic offset.
 *
 * For the same reason, XContest and Leonardo display GPS altitude in their `Max
 * alt` / `Min alt` panels. Baro is still the right source for vario (which is a
 * *difference*, where the bias cancels) and for altitude *gain* (also a
 * difference) — but absolute extremes are GPS.
 *
 * No smoothing — averaging would shave metres off any sharp peak the pilot
 * actually flew. The few-metre positive bias on the max from GPS noise is
 * dwarfed by the ~100 m bias baro would impose.
 *
 * Returns `{ minAlt: 0, maxAlt: 0 }` for an empty range. The unit is whole
 * metres; sub-metre precision is meaningless at airframe scale.
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
