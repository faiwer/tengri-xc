import { z } from 'zod';
import {
  Collection,
  Enum,
  Struct,
  Variant,
  i32,
  i8,
  String,
  u16,
  u32,
  type Value,
} from 'bincode-ts';
import { keyByField } from '../utils/keyBy';
import { DecimalDegreeIo, E5CoordinateIo } from '../utils/geo/coordinates';

// --- JSON metadata (zod) -----------------------------------------------------
//
// Wire JSON is snake_case (Rust convention). `apiGet` camelizes the body at
// the boundary, so schemas here describe the post-conversion shape and
// consumers only ever see camelCase.

/** Decimal degrees on WGS-84. Sent by the server for both takeoff and landing. */
const PointIo = z.object({
  lat: DecimalDegreeIo,
  lon: DecimalDegreeIo,
});

const TrackPointIo = z.object({
  time: z.number().int(),
  lat: E5CoordinateIo,
  lon: E5CoordinateIo,
  geoAlt: z.number().int(),
  pressureAlt: z.number().int().nullable(),
  tas: z.number().int().nullable(),
});

const RouteFixIo = z.tuple([E5CoordinateIo, E5CoordinateIo]);

const RouteWaypointIo = z.discriminatedUnion('type', [
  z.object({
    type: z.literal('point'),
    fix: TrackPointIo,
  }),
  z.object({
    type: z.literal('cylinder'),
    center: RouteFixIo,
    mode: z.enum(['enter', 'exit']).nullable(),
    radius: z.number().int(),
    tangents: z.array(RouteFixIo),
    trackFix: TrackPointIo,
  }),
  z.object({
    type: z.literal('line'),
    trackFix: TrackPointIo,
    projection: z.tuple([RouteFixIo, RouteFixIo]),
    tangent: RouteFixIo,
  }),
]);

const RouteClosureIo = z.object({
  start: RouteWaypointIo,
  end: RouteWaypointIo,
  distance: z.number().int(),
});

const RouteIo = z.object({
  flightId: z.string(),
  routeType: z.enum(['free_distance', 'fai_triangle', 'free_triangle', 'task']),
  subType: z.enum(['none', 'olc_closed', 'olc_open', 'fai_cylinders']),
  turnpoints: z.array(RouteWaypointIo),
  legDistances: z.array(z.number().int()),
  distance: z.number().int(),
  score: z.number(),
  factor: z.number(),
  optimal: z.boolean(),
  closure: RouteClosureIo.nullable(),
});

export type Route = z.infer<typeof RouteIo>;
export type PointWaypoint = Extract<
  Route['turnpoints'][number],
  { type: 'point' }
>;

export const TrackMetadataIo = z
  .object({
    id: z.string(),
    pilot: z.object({
      name: z.string(),
      /** ISO 3166-1 alpha-2 country code, or `null` if unknown. */
      country: z.string().nullable(),
    }),
    glider: z.object({
      brandId: z.string(),
      brandName: z.string(),
      modelId: z.string(),
      modelName: z.string(),
    }),
    /** Unix epoch seconds (UTC). Convert with `new Date(value * 1000)`. */
    takeoffAt: z.number().int(),
    /** Unix epoch seconds (UTC). */
    landingAt: z.number().int(),
    /** IANA timezone names at the takeoff/landing fixes. */
    takeoffTimezone: z.string(),
    landingTimezone: z.string(),
    takeoff: PointIo,
    landing: PointIo,
    /** Wire-track size as a fraction of the gzipped source (0..1ish). */
    compressionRatio: z.number(),
    routes: z.array(RouteIo),
  })
  .transform(withMetadataOffsets);

export type TrackMetadata = z.infer<typeof TrackMetadataIo>;

/** One row of `GET /tracks`. Mirrors the server's `routes::tracks_list::Item`. */
export const TrackListItemIo = z.object({
  pilot: z.object({
    id: z.number().int(),
    name: z.string(),
    /** ISO 3166-1 alpha-2 country code, or `null` if unknown. */
    country: z.string().nullable(),
  }),
  track: z
    .object({
      id: z.string(),
      /** Unix epoch seconds (UTC). */
      takeoffAt: z.number().int(),
      /** Whole seconds, from `flights.duration`. */
      duration: z.number().int(),
      /** IANA timezone names at the takeoff/landing fixes. */
      takeoffTimezone: z.string(),
      landingTimezone: z.string(),
      takeoff: PointIo,
      landing: PointIo,
    })
    .transform(withListTrackOffsets),
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

// Mirrors `server/src/flight/metadata.rs` — bump the field set in lockstep with
// `tengri::VERSION`. The four `_lat`/`_lon` are E5 micro-degrees (deg × 10⁵),
// matching `TrackPoint`'s coordinate units.
const MetadataIo = Struct({
  takeoff_timezone: String,
  landing_timezone: String,
  takeoff_lat: i32,
  takeoff_lon: i32,
  landing_lat: i32,
  landing_lon: i32,
});

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

function withMetadataOffsets<
  T extends {
    takeoffAt: number;
    landingAt: number;
    takeoffTimezone: string;
    landingTimezone: string;
  },
>(value: T): T & { takeoffOffset: number; landingOffset: number } {
  return {
    ...value,
    takeoffOffset: offsetSecondsAt(value.takeoffAt, value.takeoffTimezone),
    landingOffset: offsetSecondsAt(value.landingAt, value.landingTimezone),
  };
}

function withListTrackOffsets<
  T extends {
    takeoffAt: number;
    duration: number;
    takeoffTimezone: string;
    landingTimezone: string;
  },
>(value: T): T & { takeoffOffset: number; landingOffset: number } {
  return {
    ...value,
    takeoffOffset: offsetSecondsAt(value.takeoffAt, value.takeoffTimezone),
    landingOffset: offsetSecondsAt(
      value.takeoffAt + value.duration,
      value.landingTimezone,
    ),
  };
}

function offsetSecondsAt(epochSeconds: number, timeZone: string): number {
  const date = new Date(epochSeconds * 1000);
  const parts = keyByField(
    offsetFormatter(timeZone).formatToParts(date),
    'type',
  );
  const asUtc = Date.UTC(
    Number(parts.year.value),
    Number(parts.month.value) - 1,
    Number(parts.day.value),
    Number(parts.hour.value),
    Number(parts.minute.value),
    Number(parts.second.value),
  );
  return Math.round((asUtc - date.getTime()) / 1000);
}

const offsetFormatterCache = new Map<string, Intl.DateTimeFormat>();

function offsetFormatter(timeZone: string): Intl.DateTimeFormat {
  const cached = offsetFormatterCache.get(timeZone);
  if (cached) {
    return cached;
  }

  const formatter = new Intl.DateTimeFormat('en-US', {
    timeZone,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23',
  });
  offsetFormatterCache.set(timeZone, formatter);
  return formatter;
}
