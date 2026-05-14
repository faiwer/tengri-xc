-- Two structural fixes left over from `0009_gliders.sql`:
--
-- 1. `gliders.user_id` was missing — wings are owned by a single pilot (one
--    `gliders` row per pilot per distinct wing), so dedupe and the "delete user
--    → wipe their gliders" cascade need a real FK. Truncate `gliders` first
--    because the importer never managed to populate it (the flights step was
--    broken end-to-end on a NOT NULL `glider_id` with no binding); only the
--    test fixture's `id=1` row exists, and fixtures rebuild on every test run.
--
-- 2. `glider_class` enum overhaul for sailplanes:
--      + `thirteen_point_five_metre`  — FAI WGC class (since 2015, replaced
--                                        PW-5 World Class).
--      + `microlift`                  — FAI category (S3, since 2004): max 220
--                                        kg + ≤ 18 kg/m² wing loading. Aeriane
--                                        Swift, Archaeopteryx, Carbon Dragon.
--      + `ultralight`                 — FAI category (S3): max 220 kg, no
--                                        wing-loading cap. − `motorglider`
--      — was conceptually misplaced (it's propulsion, not class; already
--                                        covered by `propulsion='self_launch'`
--                                        / `'powered'`). Postgres can `ADD
--                                        VALUE` to an enum but not `DROP
--                                        VALUE`, so we recreate the type and
--    swap dependent columns.

TRUNCATE gliders CASCADE;

ALTER TABLE gliders
    ADD COLUMN user_id int NOT NULL REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX gliders_user_idx ON gliders (user_id);


CREATE TYPE glider_class_v2 AS ENUM (
    -- PG (unchanged)
    'en_a', 'en_b', 'en_c', 'en_d', 'ccc',
    -- HG (unchanged)
    'single_surface', 'kingpost', 'topless', 'rigid',
    -- SP (revised)
    'thirteen_point_five_metre', 'standard', 'fifteen_metre',
    'eighteen_metre', 'twenty_metre_two_seater', 'open', 'club',
    'microlift', 'ultralight'
);

-- The trigger on `glider_models.class` blocks the ALTER COLUMN TYPE below
-- ("cannot alter type of a column used in a trigger definition"); drop it and
-- recreate at the end. The CHECKs would also crash the USING-cast against the
-- new value list.
DROP TRIGGER sync_glider_denorm ON glider_models;

ALTER TABLE glider_models
    DROP CONSTRAINT glider_models_class_matches_kind,
    DROP CONSTRAINT glider_models_two_seater_implies_tandem,
    ALTER COLUMN class TYPE glider_class_v2 USING class::text::glider_class_v2;

ALTER TABLE gliders
    DROP CONSTRAINT gliders_class_matches_kind,
    DROP CONSTRAINT gliders_two_seater_implies_tandem,
    ALTER COLUMN class TYPE glider_class_v2 USING class::text::glider_class_v2;

DROP TYPE glider_class;
ALTER TYPE glider_class_v2 RENAME TO glider_class;

ALTER TABLE glider_models
    ADD CONSTRAINT glider_models_class_matches_kind CHECK (
        (kind = 'pg' AND class IN ('en_a','en_b','en_c','en_d','ccc')) OR
        (kind = 'hg' AND class IN ('single_surface','kingpost','topless','rigid')) OR
        (kind = 'sp' AND class IN ('thirteen_point_five_metre','standard','fifteen_metre',
                                    'eighteen_metre','twenty_metre_two_seater','open','club',
                                    'microlift','ultralight'))
    ),
    ADD CONSTRAINT glider_models_two_seater_implies_tandem CHECK (
        class IS DISTINCT FROM 'twenty_metre_two_seater' OR is_tandem
    );

ALTER TABLE gliders
    ADD CONSTRAINT gliders_class_matches_kind CHECK (
        class IS NULL OR
        (kind = 'pg' AND class IN ('en_a','en_b','en_c','en_d','ccc')) OR
        (kind = 'hg' AND class IN ('single_surface','kingpost','topless','rigid')) OR
        (kind = 'sp' AND class IN ('thirteen_point_five_metre','standard','fifteen_metre',
                                    'eighteen_metre','twenty_metre_two_seater','open','club',
                                    'microlift','ultralight'))
    ),
    ADD CONSTRAINT gliders_two_seater_implies_tandem CHECK (
        class IS DISTINCT FROM 'twenty_metre_two_seater' OR is_tandem IS TRUE
    );

CREATE TRIGGER sync_glider_denorm
    AFTER UPDATE OF class, is_tandem ON glider_models
    FOR EACH ROW
    EXECUTE FUNCTION sync_glider_denorm();
