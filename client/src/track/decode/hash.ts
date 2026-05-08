import type {
  CoordDual,
  CoordGps,
  FixDual,
  FixGps,
  TasFix,
  TimeFix,
} from '../../api/tracks.io';
import type { UnpackedBody, UnpackedTas } from './unpackBody';

const FNV_OFFSET = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

/**
 * Recompute the FNV-1a 32 hash of the just-decoded payload (everything in
 * `CompactTrack` except the `hash` field itself). MUST stay in sync with the
 * server's `server/src/flight/compact/hash.rs`. Field order, integer widths,
 * and little-endian encoding are the contract.
 *
 * Returned value is the unsigned 32-bit hash (`>>> 0` is applied internally
 * to keep arithmetic in the unsigned domain — JS bitwise ops are signed).
 */
export function computeCompactHash(
  startTime: number,
  interval: number,
  body: UnpackedBody,
  timeFixes: TimeFix[],
  tas: UnpackedTas,
): number {
  let h = FNV_OFFSET;
  h = feedU32(h, startTime);
  h = feedU16(h, interval);
  h = feedTrack(h, body);
  h = feedTimeFixes(h, timeFixes);
  h = feedTas(h, tas);
  return h >>> 0;
}

function feedByte(h: number, b: number): number {
  let x = (h ^ (b & 0xff)) >>> 0;
  // Math.imul keeps the multiplication in 32-bit; `* 0x01000193` would
  // silently lose precision past 2^53.
  x = Math.imul(x, FNV_PRIME) >>> 0;
  return x;
}

function feedU16(h: number, v: number): number {
  h = feedByte(h, v & 0xff);
  h = feedByte(h, (v >>> 8) & 0xff);
  return h;
}

function feedU32(h: number, v: number): number {
  h = feedByte(h, v & 0xff);
  h = feedByte(h, (v >>> 8) & 0xff);
  h = feedByte(h, (v >>> 16) & 0xff);
  h = feedByte(h, (v >>> 24) & 0xff);
  return h;
}

function feedI32(h: number, v: number): number {
  return feedU32(h, v >>> 0);
}

function feedTrack(h: number, body: UnpackedBody): number {
  h = feedByte(h, body.dual ? 1 : 0);
  h = feedU32(h, body.fixes.length);
  if (body.dual) {
    for (const f of body.fixes as FixDual[]) {
      h = feedFixDual(h, f);
    }
  } else {
    for (const f of body.fixes as FixGps[]) {
      h = feedFixGps(h, f);
    }
  }
  h = feedU32(h, body.coords.length);
  if (body.dual) {
    for (const c of body.coords as CoordDual[]) {
      h = feedCoordDual(h, c);
    }
  } else {
    for (const c of body.coords as CoordGps[]) {
      h = feedCoordGps(h, c);
    }
  }
  return h;
}

function feedFixGps(h: number, f: FixGps): number {
  h = feedU32(h, f.idx);
  h = feedI32(h, f.lat);
  h = feedI32(h, f.lon);
  h = feedI32(h, f.geo_alt);
  return h;
}

function feedFixDual(h: number, f: FixDual): number {
  h = feedU32(h, f.idx);
  h = feedI32(h, f.lat);
  h = feedI32(h, f.lon);
  h = feedI32(h, f.geo_alt);
  h = feedI32(h, f.pressure_alt);
  return h;
}

function feedCoordGps(h: number, c: CoordGps): number {
  h = feedByte(h, c.lat);
  h = feedByte(h, c.lon);
  h = feedByte(h, c.geo_alt);
  return h;
}

function feedCoordDual(h: number, c: CoordDual): number {
  h = feedByte(h, c.lat);
  h = feedByte(h, c.lon);
  h = feedByte(h, c.geo_alt);
  h = feedByte(h, c.pressure_alt);
  return h;
}

function feedTimeFixes(h: number, tf: TimeFix[]): number {
  h = feedU32(h, tf.length);
  for (const t of tf) {
    h = feedU32(h, t.idx);
    h = feedU32(h, t.time);
  }
  return h;
}

function feedTas(h: number, tas: UnpackedTas): number {
  if (tas.kind === 'none') {
    return feedByte(h, 0);
  }

  h = feedByte(h, 1);
  h = feedU32(h, tas.fixes.length);
  for (const f of tas.fixes) {
    h = feedTasFix(h, f);
  }

  h = feedU32(h, tas.deltas.length);
  for (const d of tas.deltas) {
    // Rust feeds `i8 as u8`; on the wire each delta occupies one byte.
    // `& 0xff` performs the same two's-complement reinterpret in JS.
    h = feedByte(h, d & 0xff);
  }

  return h;
}

function feedTasFix(h: number, f: TasFix): number {
  h = feedU32(h, f.idx);
  h = feedU16(h, f.tas);
  return h;
}
