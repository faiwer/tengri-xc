import type { Track } from '../types';

export const VARIO_WINDOW_HALF_SECONDS = 5;

/**
 * Per-fix vertical velocity in m/s, computed over a centred ±5 s window.
 *
 * Uses barometric altitude when present (much smoother than GPS — ~±0.1 m
 * vs ±2 m), falls back to GPS altitude otherwise. The window absorbs
 * second-to-second jitter so downstream classification can use simple
 * thresholds without seeing every breath of turbulence.
 *
 * Near the array boundaries the window is one-sided; the result is still a
 * valid local slope but is computed over fewer samples.
 */
export const computeVario = (track: Track): Float32Array => {
  const alt = track.baroAlt ?? track.alt;
  const times = track.t;
  const fixCount = times.length;
  const vario = new Float32Array(fixCount);

  let leftIdx = 0;
  let rightIdx = 0;

  for (let i = 0; i < fixCount; i++) {
    const tLeft = times[i]! - VARIO_WINDOW_HALF_SECONDS;
    const tRight = times[i]! + VARIO_WINDOW_HALF_SECONDS;

    while (leftIdx < fixCount - 1 && times[leftIdx]! < tLeft) {
      leftIdx++;
    }
    while (rightIdx < fixCount - 1 && times[rightIdx + 1]! <= tRight) {
      rightIdx++;
    }

    const dt = times[rightIdx]! - times[leftIdx]!;
    if (dt <= 0) {
      vario[i] = 0;
      continue;
    }
    const dAltDm = alt[rightIdx]! - alt[leftIdx]!;
    vario[i] = dAltDm / 10 / dt;
  }

  return vario;
};
