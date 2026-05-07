import { VALUE, VARIANT } from 'bincode-ts';
import type {
  CoordDual,
  CoordGps,
  FixDual,
  FixGps,
  TengriFile,
  TimeFix,
} from '../../api/tracks.io';

/**
 * Peel `bincode-ts`' `VARIANT`/`VALUE` symbols off the wire-format `TrackBody`
 * and return a plain shape the rest of the decoder can pattern-match on.
 */
export function unpackBody(file: TengriFile): Unpacked {
  const { start_time: startTime, interval, track, time_fixes } = file.track;
  const dual = track[VARIANT] === 'Dual';
  const value = track[VALUE];
  return {
    startTime,
    interval,
    body: {
      dual,
      fixes: value.fixes,
      coords: value.coords,
    },
    timeFixes: time_fixes,
  };
}

export interface Unpacked {
  startTime: number;
  interval: number;
  body: UnpackedBody;
  timeFixes: TimeFix[];
}

export interface UnpackedBody {
  dual: boolean;
  fixes: (FixGps | FixDual)[];
  coords: (CoordGps | CoordDual)[];
}
