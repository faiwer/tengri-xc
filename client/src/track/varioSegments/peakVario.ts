export interface VarioPeaks {
  /** Highest smoothed climb in m/s (positive). `0` if the range never climbs. */
  peakClimb: number;
  /** Strongest smoothed sink in m/s (negative). `0` if the range never sinks. */
  peakSink: number;
}

/**
 * Find the strongest climb and strongest sink in a smoothed vario series
 * over the half-open range `[fromIdx, toIdx)`. Operates on the same array
 * returned by `computeVario`, so the values share its smoothing semantics
 * (centred ±5 s window).
 *
 * Both fields default to `0` for an empty range — meaning "no climb / no
 * sink observed", which is the correct neutral display value.
 */
export const peakVario = (
  vario: Float32Array,
  fromIdx: number,
  toIdx: number,
): VarioPeaks => {
  let peakClimb = 0;
  let peakSink = 0;
  for (let i = fromIdx; i < toIdx; i++) {
    const v = vario[i]!;
    if (v > peakClimb) {
      peakClimb = v;
    } else if (v < peakSink) {
      peakSink = v;
    }
  }
  return { peakClimb, peakSink };
};
