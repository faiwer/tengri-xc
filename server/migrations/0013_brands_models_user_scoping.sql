-- Prep step for dropping the `gliders` table (0014): fold per-pilot custom
-- wings into the existing `brands` / `glider_models` tables (the latter is
-- renamed to `models` in 0014) by adding a nullable `user_id` column. `user_id
-- IS NULL` is the curated canonical catalog; `user_id IS NOT NULL` is a
-- pilot-private custom row that dedupes by per-pilot slug prefix on `id`
-- (`42:my-secret-wing`). See `docs/gliders.md` for the full flow.
--
-- The `'unknown'` enum value this migration's CHECKs reference is added in
-- 0012; splitting `ADD VALUE` off into its own migration is what lets us use
-- the value here without tripping Postgres' "unsafe use of new value"
-- in-transaction guard.

-- 1. User scoping. NULL = canonical (curated by us); NOT NULL = pilot custom.
--    ON DELETE CASCADE so closing an account cleans up the pilot's customs.

ALTER TABLE brands
    ADD COLUMN user_id int NULL REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE glider_models
    ADD COLUMN user_id int NULL REFERENCES users(id) ON DELETE CASCADE;


-- 2. Switch name-uniqueness from "globally unique" to "globally unique among
--    canonical rows". Customs dedupe at the PK because `id` is
--    `<user_id>:<slugify(name)>` — two cosmetic variants of the same raw text
--    from the same pilot collide naturally on `(brand_id, kind, id)`.

ALTER TABLE brands DROP CONSTRAINT brands_name_key;

CREATE UNIQUE INDEX brands_canonical_name_uniq
    ON brands (name)
    WHERE user_id IS NULL;

ALTER TABLE glider_models DROP CONSTRAINT glider_models_brand_id_kind_name_key;

CREATE UNIQUE INDEX glider_models_canonical_name_uniq
    ON glider_models (brand_id, kind, name)
    WHERE user_id IS NULL;


-- 3. Expand `class_matches_kind` to allow `'unknown'` universally, and add a
--    guard so curated rows can never drift into `'unknown'` — the canonical
--    catalog stays opinionated.

ALTER TABLE glider_models
    DROP CONSTRAINT glider_models_class_matches_kind,
    ADD CONSTRAINT glider_models_class_matches_kind CHECK (
        class = 'unknown' OR
        (kind = 'pg' AND class IN ('en_a','en_b','en_c','en_d','ccc')) OR
        (kind = 'hg' AND class IN ('single_surface','kingpost','topless','rigid')) OR
        (kind = 'sp' AND class IN ('thirteen_point_five_metre','standard','fifteen_metre',
                                    'eighteen_metre','twenty_metre_two_seater','open','club',
                                    'microlift','ultralight'))
    );

ALTER TABLE glider_models
    ADD CONSTRAINT glider_models_unknown_implies_custom CHECK (
        class <> 'unknown' OR user_id IS NOT NULL
    );


-- 4. Canonical-vs-custom slug-shape invariant. Canonical `id`s are plain slugs
--    (`aeros`, `mantra-m7`); custom `id`s are user-prefixed
--    (`42:my-secret-wing`). `slugify()` only emits `[a-z0-9-]`, so the `:`
--    prefix is unambiguous in either direction.

ALTER TABLE brands
    ADD CONSTRAINT brands_slug_shape CHECK (
        (user_id IS NULL     AND id NOT LIKE '%:%') OR
        (user_id IS NOT NULL AND id     LIKE '%:%')
    );

ALTER TABLE glider_models
    ADD CONSTRAINT glider_models_slug_shape CHECK (
        (user_id IS NULL     AND id NOT LIKE '%:%') OR
        (user_id IS NOT NULL AND id     LIKE '%:%')
    );
