import type {
  CoordDual,
  CoordGps,
  FixDual,
  FixGps,
  TasFix,
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

  if (c.tas) {
    stepTas(i, s, c.tas);
  }

  s.i++;
}

/**
 * Advance the TAS merge-walk for index `i`. Mirrors `apply_tas` in
 * `server/src/flight/compact/decode.rs`: at i=0 the initial fix has already
 * stamped `s.sTas`; for i > 0, either the next fix override matches `i` and
 * replaces the running state outright, or we add the next i8 delta. The
 * combined cursor invariant `tasFixes.length + tasDeltas.length === length`
 * guarantees we never underflow either array.
 */
function stepTas(i: number, s: DecodeState, t: TasCtx): void {
  if (i === 0) {
    t.out[0] = s.sTas;
    return;
  }

  const nextFix = s.tasFixCur < t.fixes.length ? t.fixes[s.tasFixCur]! : null;
  if (nextFix && nextFix.idx === i) {
    s.sTas = nextFix.tas;
    s.tasFixCur++;
  } else {
    if (s.tasDeltaCur >= t.deltas.length) {
      throw new TrackDecodeError(
        `compact track is malformed: index ${i} out of TAS delta range`,
      );
    }

    const next = s.sTas + t.deltas[s.tasDeltaCur]!;
    // Defensive clamp — encoded streams that respect the i8 delta + override
    // contract never exceed u16 range.
    s.sTas = Math.max(0, Math.min(0xffff, next));
    s.tasDeltaCur++;
  }

  t.out[i] = s.sTas;
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

  /** `null` when the source had no TAS column. */
  tas: TasCtx | null;
}

export interface TasCtx {
  fixes: TasFix[];
  deltas: number[];
  out: Uint16Array;
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

  tasFixCur: number;
  tasDeltaCur: number;
  sTas: number;
}
