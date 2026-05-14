-- Drop the NOT NULL on `flights.glider_id` so ingest paths that don't carry
-- glider metadata (currently `tengri add`) can insert with `glider_id IS NULL`
-- instead of pointing every such flight at a shared "blank kind='other'"
-- catch-all wing. Consumers that join through `glider_id` (`/me/gliders`
-- counts) already use `= g.id`, which doesn't match NULL, so unassigned flights
-- correctly don't count against any wing's flight total.
--
-- The FK and its index stay. A future "make glider_id NOT NULL again" migration
-- is on deck (and would also fold `brand_text` / `model_text` into a per-user
-- `gliders` row keyed by `user_id IS NOT NULL`); this migration is the
-- intermediate step that lets the `tengri add` path stop forging a shared
-- placeholder wing in the meantime.

ALTER TABLE flights
    ALTER COLUMN glider_id DROP NOT NULL;
