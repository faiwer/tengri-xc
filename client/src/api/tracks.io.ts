import { z } from 'zod';
import {
  Collection,
  Enum,
  Struct,
  Variant,
  i32,
  i8,
  u16,
  u32,
  type Value,
} from 'bincode-ts';

// --- JSON metadata (zod) -----------------------------------------------------
//
// Wire JSON is snake_case (Rust convention). `apiGet` camelizes the body at
// the boundary, so schemas here describe the post-conversion shape and
// consumers only ever see camelCase.

/** Decimal degrees on WGS-84. Sent by the server for both takeoff and landing. */
const PointIo = z.object({
  lat: z.number(),
  lon: z.number(),
});

export const TrackMetadataIo = z.object({
  id: z.string(),
  pilot: z.object({
    name: z.string(),
    /** ISO 3166-1 alpha-2 country code, or `null` if unknown. */
    country: z.string().nullable(),
  }),
  /** Unix epoch seconds (UTC). Convert with `new Date(value * 1000)`. */
  takeoffAt: z.number().int(),
  /** Unix epoch seconds (UTC). */
  landingAt: z.number().int(),
  /**
   * UTC offsets in whole seconds, computed at ingest from the takeoff/landing
   * fix coordinates and the `tzdb` rules valid on the flight's date. Positive =
   * ahead of UTC. Use these to display flight-local wall-clock time without
   * dragging the viewer's TZ in.
   */
  takeoffOffset: z.number().int(),
  landingOffset: z.number().int(),
  takeoff: PointIo,
  landing: PointIo,
  /** Wire-track size as a fraction of the gzipped source (0..1ish). */
  compressionRatio: z.number(),
});

export type TrackMetadata = z.infer<typeof TrackMetadataIo>;

/** One row of `GET /tracks`. Mirrors the server's `routes::tracks_list::Item`. */
export const TrackListItemIo = z.object({
  pilot: z.object({
    id: z.number().int(),
    name: z.string(),
    /** ISO 3166-1 alpha-2 country code, or `null` if unknown. */
    country: z.string().nullable(),
  }),
  track: z.object({
    id: z.string(),
    /** Unix epoch seconds (UTC). */
    takeoffAt: z.number().int(),
    /** Whole seconds, from `flights.duration`. */
    duration: z.number().int(),
    /** UTC offsets — see {@link TrackMetadataIo}. */
    takeoffOffset: z.number().int(),
    landingOffset: z.number().int(),
    takeoff: PointIo,
    landing: PointIo,
  }),
});

export type TrackListItem = z.infer<typeof TrackListItemIo>;

export const TracksPageIo = z.object({
  items: z.array(TrackListItemIo),
  /** Opaque cursor for the next page; `null` on the last page. */
  nextCursor: z.string().nullable(),
});

export type TracksPage = z.infer<typeof TracksPageIo>;

// --- TengriFile binary wire format (bincode-ts) ------------------------------
//
// MUST stay in sync with `server/src/flight/tengri/format.rs` and
// `server/src/flight/compact/types.rs`. Bincode is positional, so field order
// matters; the lib iterates `Object.keys`, which preserves insertion order in
// modern JS engines, so just declare fields in the exact Rust order.
//
// Variant tags also come from the Rust declaration order: `TrackBody::Gps` is
// variant 0, `TrackBody::Dual` is variant 1.

const FixGpsIo = Struct({
  idx: u32,
  lat: i32,
  lon: i32,
  geo_alt: i32,
});

const FixDualIo = Struct({
  idx: u32,
  lat: i32,
  lon: i32,
  geo_alt: i32,
  pressure_alt: i32,
});

const CoordGpsIo = Struct({
  lat: i8,
  lon: i8,
  geo_alt: i8,
});

const CoordDualIo = Struct({
  lat: i8,
  lon: i8,
  geo_alt: i8,
  pressure_alt: i8,
});

const TimeFixIo = Struct({
  idx: u32,
  time: u32,
});

const TrackBodyIo = Enum({
  Gps: Variant(
    0,
    Struct({
      fixes: Collection(FixGpsIo),
      coords: Collection(CoordGpsIo),
    }),
  ),
  Dual: Variant(
    1,
    Struct({
      fixes: Collection(FixDualIo),
      coords: Collection(CoordDualIo),
    }),
  ),
});

const TasFixIo = Struct({
  idx: u32,
  tas: u16,
});

const TasBodyIo = Enum({
  None: Variant(0),
  Tas: Variant(
    1,
    Struct({
      fixes: Collection(TasFixIo),
      deltas: Collection(i8),
    }),
  ),
});

const CompactTrackIo = Struct({
  start_time: u32,
  interval: u16,
  track: TrackBodyIo,
  time_fixes: Collection(TimeFixIo),
  tas: TasBodyIo,
  hash: u32,
});

const MetadataIo = Struct({});

export const TengriFileIo = Struct({
  version: u16,
  metadata: MetadataIo,
  track: CompactTrackIo,
});

export type TengriFile = Value<typeof TengriFileIo>;

export type FixGps = Value<typeof FixGpsIo>;
export type FixDual = Value<typeof FixDualIo>;
export type CoordGps = Value<typeof CoordGpsIo>;
export type CoordDual = Value<typeof CoordDualIo>;
export type TimeFix = Value<typeof TimeFixIo>;
export type TasFix = Value<typeof TasFixIo>;
export type TasBody = Value<typeof TasBodyIo>;
