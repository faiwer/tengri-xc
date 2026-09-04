import type { TrackWindow } from '../toPaths';
import type { Track } from '../types';
import { VARIO_WINDOW_HALF_SECONDS } from './vario';

/**
 * Coarsest average fix spacing that still yields a usable vario series.
 * `computeVario` only admits a neighbouring fix within half a window, so
 * spacing beyond that leaves every fix alone in its own window and the
 * whole series reads as a flat zero — a cliff, not a gradual decay.
 */
export const MAX_VARIO_FIX_INTERVAL_SECONDS = VARIO_WINDOW_HALF_SECONDS;

/** Mean seconds between fixes over the flight window; 0 when it holds one fix. */
export const averageFixInterval = (
  track: Track,
  window: TrackWindow,
): number => {
  const intervals = window.landingIdx - window.takeoffIdx;
  if (intervals <= 0) {
    return 0;
  }

  return (
    (track.t[window.landingIdx]! - track.t[window.takeoffIdx]!) / intervals
  );
};
