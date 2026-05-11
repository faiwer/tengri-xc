-- Rename `flights.landed_at` → `flights.landing_at` so the schema uses one
-- consistent verb form (the noun "landing" — to match `takeoff_at`) for the
-- two flight events. Phase 0 of the timezone-offsets work; the new
-- `landing_offset` / `landing_point` columns added in 0007 then sit next to
-- a column whose name uses the same form.
--
-- The `duration_s` generated column references this column via
-- `EXTRACT(EPOCH FROM (landed_at - takeoff_at))`. Postgres rewrites
-- generated-column expressions to track column renames, so the expression
-- becomes `(landing_at - takeoff_at)` automatically — no DROP/ADD needed.

ALTER TABLE flights RENAME COLUMN landed_at TO landing_at;
