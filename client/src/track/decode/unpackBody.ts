import { VALUE, VARIANT } from 'bincode-ts';
import type {
  CoordDual,
  CoordGps,
  FixDual,
  FixGps,
  TasFix,
  TengriFile,
  TimeFix,
} from '../../api/tracks.io';

/**
 * Peel `bincode-ts`' `VARIANT`/`VALUE` symbols off the wire-format enums
 * (`TrackBody`, `TasBody`) and return a plain shape the rest of the decoder
 * can pattern-match on.
 */
export function unpackBody(file: TengriFile): Unpacked {
  const {
    start_time: startTime,
    interval,
    track,
    time_fixes,
    tas,
  } = file.track;

  const dual = track[VARIANT] === 'Dual';
  const trackValue = track[VALUE];

  const tasUnpacked: UnpackedTas =
    tas[VARIANT] === 'Tas'
      ? { kind: 'tas', fixes: tas[VALUE].fixes, deltas: tas[VALUE].deltas }
      : { kind: 'none' };

  return {
    startTime,
    interval,
    body: {
      dual,
      fixes: trackValue.fixes,
      coords: trackValue.coords,
    },
    timeFixes: time_fixes,
    tas: tasUnpacked,
  };
}

export interface Unpacked {
  startTime: number;
  interval: number;
  body: UnpackedBody;
  timeFixes: TimeFix[];
  tas: UnpackedTas;
}

export interface UnpackedBody {
  dual: boolean;
  fixes: (FixGps | FixDual)[];
  coords: (CoordGps | CoordDual)[];
}

/**
 * Plain-shaped TAS channel. `kind: 'none'` mirrors `TasBody::None` (the
 * source had no TAS column); `kind: 'tas'` carries the sparse fix overrides
 * and per-non-fix-index `i8` deltas in km/h, which the decoder merges into
 * a `Uint16Array` aligned with the position arrays.
 */
export type UnpackedTas =
  | { kind: 'none' }
  | { kind: 'tas'; fixes: TasFix[]; deltas: number[] };
