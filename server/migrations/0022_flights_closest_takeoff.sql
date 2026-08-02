-- Nearest known takeoff site per flight, plus the takeoff's country.
-- All nullable: filled by a backfill / scoring pass that runs a KNN of
-- flights.takeoff_point against sites (ORDER BY point <-> takeoff_point
-- LIMIT 1), not at insert time. A flight with no takeoff_point, or one
-- ingested before the pass runs, leaves these NULL.

ALTER TABLE flights
    -- ISO 3166-1 alpha-2 (uppercase), same shape as user_profiles.country.
    ADD COLUMN takeoff_country          char(2) NULL,
    -- FK to the nearest sites row. ON DELETE SET NULL so removing a site
    -- doesn't strand the flight; the pass can re-resolve it later.
    ADD COLUMN closest_takeoff_id       int     NULL REFERENCES sites(id) ON DELETE SET NULL,
    -- Distance from takeoff_point to that site, whole metres (matches the
    -- integer-metre convention of main_distance).
    ADD COLUMN closest_takeoff_distance integer NULL;
