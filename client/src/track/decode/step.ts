import type {
  CoordDual,
  CoordGps,
  FixDual,
  FixGps,
  TimeFix,
} from '../../api/tracks.io';
import { TrackDecodeError } from '../types';

/**
 * Advance one index: pick fix-or-coord, apply time anchor, write the row.
 * Mutates `s.i++`. Throws `TrackDecodeError` on a malformed track.
 *
 * Mirrors the Rust decoder loop body in `server/src/flight/compact/decode.rs`.
 */
export function decodeStep(s: DecodeState, c: DecodeCtx): void {
  const { i } = s;

  if (i > 0) {
    const nextFix = s.fixCur < c.fixes.length ? c.fixes[s.fixCur]! : null;
    if (nextFix && nextFix.idx === i) {
      s.sLat = nextFix.lat;
      s.sLng = nextFix.lon;
      s.sAlt = nextFix.geo_alt;
      if (c.dual) {
        s.sBaroAlt = (nextFix as FixDual).pressure_alt;
      }
      s.fixCur++;
    } else {
      if (s.coordCur >= c.coords.length) {
        throw new TrackDecodeError(
          `compact track is malformed: index ${i} out of coord range (length=${c.length})`,
        );
      }
      const co = c.coords[s.coordCur]!;
      s.sLat += co.lat;
      s.sLng += co.lon;
      s.sAlt += co.geo_alt;
      if (c.dual) {
        s.sBaroAlt += (co as CoordDual).pressure_alt;
      }
      s.coordCur++;
    }
  }

  if (s.timeCur < c.timeFixes.length && c.timeFixes[s.timeCur]!.idx === i) {
    s.anchorIdx = i;
    s.anchorTime = c.timeFixes[s.timeCur]!.time;
    s.timeCur++;
  }
  c.t[i] = s.anchorTime + (i - s.anchorIdx) * c.interval;

  c.lat[i] = s.sLat;
  c.lng[i] = s.sLng;
  c.alt[i] = s.sAlt;
  if (c.baroAlt) {
    c.baroAlt[i] = s.sBaroAlt;
  }

  s.i++;
}

export interface DecodeCtx {
  fixes: (FixGps | FixDual)[];
  coords: (CoordGps | CoordDual)[];
  timeFixes: TimeFix[];
  interval: number;
  dual: boolean;
  length: number;

  t: Uint32Array;
  lat: Int32Array;
  lng: Int32Array;
  alt: Int32Array;
  baroAlt: Int32Array | null;
}

export interface DecodeState {
  i: number;
  fixCur: number;
  coordCur: number;
  timeCur: number;

  anchorIdx: number;
  anchorTime: number;

  sLat: number;
  sLng: number;
  sAlt: number;
  sBaroAlt: number;
}
