-- Drop the `gliders` junction table and rewire `flights` to point at `models`
-- (renamed from `glider_models`) directly via a composite FK. Custom wings now
-- live as rows in `models` with `user_id IS NOT NULL` (set up by 0013); there's
-- no separate per-pilot wing-instance row anymore.
--
-- Precondition: `flights` is empty. This migration does NOT issue a DELETE or
-- TRUNCATE — a data-destructive statement in a permanent migration is a
-- landmine for anyone re-running migrations later (see
-- `.cursor/rules/migrations.mdc` and `.cursor/rules/pre-release-db.mdc`). If
-- the precondition isn't met, the `ADD COLUMN ... NOT NULL` below fails with
-- "column contains null values" — wipe the table and retry.

-- 1. Cut `flights` loose from `gliders`. The FK and index attached to the
--    column go with it automatically.

ALTER TABLE flights DROP COLUMN glider_id;


-- 2. Tear down the sync-trigger machinery; the `class` / `is_tandem` denorms on
--    `gliders` it kept honest go away with the table.

DROP TRIGGER sync_glider_denorm ON glider_models;
DROP FUNCTION sync_glider_denorm;


-- 3. Drop `gliders`. The only inbound FK was `flights.glider_id`, already gone,
--    so nothing cascades. Its indexes go with the table.

DROP TABLE gliders;


-- 4. Rename `glider_models` to `models`. PG auto-renames the implicit PK index
--    along with the table; the partial unique (from 0013) and the
--    explicit-named CHECKs / FKs need renaming by hand for grep-ability.

ALTER TABLE glider_models RENAME TO models;

ALTER INDEX glider_models_canonical_name_uniq RENAME TO models_canonical_name_uniq;
ALTER INDEX glider_models_pkey                RENAME TO models_pkey;

ALTER TABLE models RENAME CONSTRAINT glider_models_class_matches_kind        TO models_class_matches_kind;
ALTER TABLE models RENAME CONSTRAINT glider_models_two_seater_implies_tandem TO models_two_seater_implies_tandem;
ALTER TABLE models RENAME CONSTRAINT glider_models_unknown_implies_custom    TO models_unknown_implies_custom;
ALTER TABLE models RENAME CONSTRAINT glider_models_slug_shape                TO models_slug_shape;
ALTER TABLE models RENAME CONSTRAINT glider_models_brand_id_fkey             TO models_brand_id_fkey;
ALTER TABLE models RENAME CONSTRAINT glider_models_user_id_fkey              TO models_user_id_fkey;


-- 5. Push the resolved wing onto `flights` as a composite FK to `models`. The
--    `ADD COLUMN ... NOT NULL` without a DEFAULT also doubles as the "is the
--    table empty?" precondition check above — it fails on a populated table,
--    which is the right failure mode.

ALTER TABLE flights
    ADD COLUMN brand_id text        NOT NULL,
    ADD COLUMN kind     glider_kind NOT NULL,
    ADD COLUMN model_id text        NOT NULL,
    ADD CONSTRAINT flights_model_fkey
        FOREIGN KEY (brand_id, kind, model_id)
        REFERENCES models (brand_id, kind, id)
        ON UPDATE CASCADE ON DELETE RESTRICT;

-- Powers `/me/gliders` GROUP BY (`brand_id`, `kind`, `model_id`) and
-- "find all flights on this wing" lookups.
CREATE INDEX flights_model_idx ON flights (brand_id, kind, model_id);


-- 6. Reap pilot-private brand/model rows when their last referencing flight
--    goes away. Canonical rows are immortal because `m.user_id = OLD.user_id`
--    is `NULL = i32` → never true for `user_id IS NULL` (set up by 0013). The
--    brand-cleanup pass also sweeps canonical brands harmlessly: the `b.user_id
--    = OLD.user_id` filter excludes them the same way.

CREATE FUNCTION cleanup_orphan_custom_glider_data() RETURNS trigger AS $$
BEGIN
    DELETE FROM models m
     WHERE m.user_id  = OLD.user_id
       AND m.brand_id = OLD.brand_id
       AND m.kind     = OLD.kind
       AND m.id       = OLD.model_id
       AND NOT EXISTS (
           SELECT 1 FROM flights f
            WHERE f.brand_id = m.brand_id
              AND f.kind     = m.kind
              AND f.model_id = m.id
       );

    DELETE FROM brands b
     WHERE b.user_id = OLD.user_id
       AND b.id      = OLD.brand_id
       AND NOT EXISTS (
           SELECT 1 FROM models m WHERE m.brand_id = b.id
       );

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER flights_cleanup_orphan_customs
    AFTER DELETE ON flights
    FOR EACH ROW
    EXECUTE FUNCTION cleanup_orphan_custom_glider_data();
