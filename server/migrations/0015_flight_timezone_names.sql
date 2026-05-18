-- Store IANA timezone names instead of computed UTC offsets. Offsets depend on
-- tzdb rules baked into the running binary; names let clients and future
-- backfills apply newer rules for the same flight instant.

ALTER TABLE flights
    ADD COLUMN takeoff_timezone text NOT NULL DEFAULT 'Etc/UTC',
    ADD COLUMN landing_timezone text NOT NULL DEFAULT 'Etc/UTC';

ALTER TABLE flights
    DROP COLUMN takeoff_offset,
    DROP COLUMN landing_offset;

ALTER TABLE flights
    ALTER COLUMN takeoff_timezone DROP DEFAULT,
    ALTER COLUMN landing_timezone DROP DEFAULT;
