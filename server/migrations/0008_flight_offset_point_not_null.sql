-- Lock down the four columns 0007 added as nullable. The Rust backfill
-- (`tengri_server::flight::backfill::run`, invoked from `tengri migrate` and
-- the server-startup migration path) runs immediately after `sqlx::migrate!`
-- and populates every existing row; new rows go through `flight::ingest`, which
-- always populates these fields. So by the time this migration runs there's no
-- NULL left.
--
-- Encoded as `SET NOT NULL` rather than re-issuing `ALTER ADD … NOT NULL`
-- because the columns already exist with data.

ALTER TABLE flights
    ALTER COLUMN takeoff_offset SET NOT NULL,
    ALTER COLUMN landing_offset SET NOT NULL,
    ALTER COLUMN takeoff_point  SET NOT NULL,
    ALTER COLUMN landing_point  SET NOT NULL;
