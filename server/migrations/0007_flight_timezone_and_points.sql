-- Per-flight timezone offsets and start/end geography points.
--
-- 1. Enable PostGIS so we can store the takeoff/landing fix coordinates as
--    `geography(Point, 4326)` (WGS-84, spherical maths). The column type
--    makes future "flights within X km of (lat, lon)" / "flights launched
--    inside this polygon" / "near a known site" queries first-class and
--    GiST-indexable; doing the same with a pair of `numeric` lat/lon
--    columns would require expression indices and bespoke geometry maths.
-- 2. Drop the `_s` suffix on `duration`. The unit (whole seconds) is
--    documented next to the column; the suffix added noise and didn't
--    survive the schema's other timestamp/integer pairs.
-- 3. Add the four new columns nullable on this first cut. The Rust
--    backfill (`tengri_server::flight::backfill::run`, also invoked at
--    server startup right after `sqlx::migrate!`) re-parses each flight
--    from `flight_sources` and populates them in one transaction per
--    flight. A follow-up migration (`0008_*.sql`) flips them to
--    `NOT NULL` once the first deploy's backfill is verified clean.
-- 4. GiST on each point so the future spatial filters land on an index.
--    BRIN is the wrong choice here — flights are not stored in geographic
--    order, so the per-page summary tuples don't compress to a useful
--    bounding box.

CREATE EXTENSION IF NOT EXISTS postgis;

ALTER TABLE flights RENAME COLUMN duration_s TO duration;

ALTER TABLE flights
    ADD COLUMN takeoff_offset integer,
    ADD COLUMN landing_offset integer,
    ADD COLUMN takeoff_point  geography(Point, 4326),
    ADD COLUMN landing_point  geography(Point, 4326);

CREATE INDEX flights_takeoff_point_gix ON flights USING gist (takeoff_point);
CREATE INDEX flights_landing_point_gix ON flights USING gist (landing_point);
