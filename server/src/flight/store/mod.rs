//! Postgres writers/readers for the flight bundle, split by table.
//!
//! Every ingest path writes the same shape — `flights` + `flight_sources` +
//! `flight_tracks`, plus `routes` once scored. Only the source of the
//! `flight_id` (NanoID for interactive uploads, `LEO-<n>` for the Leonardo
//! importer) and the conflict policy (none vs. `ON CONFLICT (id) DO NOTHING`)
//! differ; those decisions stay with the caller. This module owns the column
//! lists, the SQL strings, and the FK-violation translation, which is what
//! diverges painfully when the schema moves.
//!
//! Conventions shared across the submodules:
//! - Functions take an open `&mut Transaction` so the caller picks the boundary.
//! - `bigint` timestamps are unix seconds; the SQL wraps them in
//!   `to_timestamp(...)` so callers don't have to.
//! - `*_lat` / `*_lon` are E5 micro-degrees (matching `TrackPoint`); the SQL
//!   converts to decimal degrees and wraps in `ST_SetSRID(ST_MakePoint(...),
//!   4326)::geography` at the bind site.
//!
//! ## Glossary
//!
//! - `flights` — the parent `flights` row: [`FlightRow`], [`insert_flight`]
//!   (non-conflicting), [`insert_flight_idempotent`] (`DO NOTHING`), and the
//!   [`InsertFlightError`] FK translation.
//! - `sites` — takeoff↔site linking: `find_nearest_site` at ingest and
//!   [`reindex_takeoff_sites`] for the admin bulk pass, both bounded by
//!   [`TAKEOFF_SITE_RADIUS_M`].
//! - `meta` — editable glider/launch metadata: [`model_exists`] visibility
//!   check and [`update_flight_meta`] ([`FlightMetaUpdate`]).
//! - `sources` — the gzipped original upload: [`insert_source`],
//!   [`fetch_source`] ([`StoredSource`]), [`fetch_source_track`].
//! - `tracks` — the compact binary track the client decodes: [`insert_track`].
//! - `routes` — scoring persistence/readback: [`upsert_scored_routes`],
//!   [`upsert_scored_route`], [`fetch_scored_routes`], plus the
//!   `route_type` / `route_sub_type` enum mapping.

mod flights;
mod meta;
mod routes;
mod sites;
mod sources;
mod tracks;

pub use flights::{FlightRow, InsertFlightError, insert_flight, insert_flight_idempotent};
pub use meta::{FlightMetaUpdate, model_exists, update_flight_meta};
pub use routes::{fetch_scored_routes, upsert_scored_route, upsert_scored_routes};
pub use sites::{TAKEOFF_SITE_RADIUS_M, reindex_takeoff_sites};
pub use sources::{StoredSource, fetch_source, fetch_source_track, insert_source};
pub use tracks::insert_track;
