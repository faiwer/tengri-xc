-- Profile fields, separated from `users` because they live on a
-- different lifecycle: `users` rows change rarely (login flips,
-- password resets, soft-bans), profile fields change whenever a
-- pilot updates their bio. Splitting also means the auth-only
-- callers (login, /me, JWT issue) can read a narrow `users` row
-- without dragging text columns they don't need.
--
-- Why no FK from `users` here: a freshly-created internal user
-- (e.g. via `tengri add` or future signup) gets a `users` row but
-- no `user_profiles` row until they fill the form. `LEFT JOIN
-- user_profiles` makes the absence visible.
--
-- Future columns this table is likely to grow: bio, photo_url,
-- home_takeoff_id, instagram_handle, club_id, total_xc_hours.

-- Self-described gender, optional. `diverse` is for non-binary /
-- prefer-not-to-conform-to-MF; we'd rather have one extra branch
-- than force people to lie. Leonardo's source only provides M/F/
-- empty so the importer never produces `diverse` — it's there for
-- internal signups.
CREATE TYPE user_sex AS ENUM ('male', 'female', 'diverse');


CREATE TABLE user_profiles (
    user_id    int         PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- FAI/CIVL pilot id. Stored *not unique* on purpose: a unique
    -- constraint would turn this column into a land-grab — the
    -- first impostor to claim someone else's CIVL ID would lock
    -- the real owner out, with no recourse short of a DB edit.
    -- Verification is a separate concern (a future
    -- `civl_verified_at` plus an OAuth-y flow against CIVL); until
    -- then we treat the value as user-asserted, not authoritative.
    civl_id    int,
    -- ISO-3166 alpha-2 country code (uppercase). The pilot's
    -- self-declared residence. Nullable — Leonardo always fills
    -- it but our future signup flow shouldn't force it.
    country    char(2),
    sex        user_sex,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Filter / group by country ("flights from KZ this season",
-- "leaderboard for FR"). Common enough to deserve a real index;
-- the cardinality (one row per user, dozens of distinct
-- countries) makes it cheap.
CREATE INDEX user_profiles_country_idx ON user_profiles (country);

-- Same updated_at autoset trigger as users (0002). We define our
-- own function rather than reusing `users_touch_updated_at`
-- because that one's table-private (its name says so); refactor
-- to a single shared function once we have a third user.
CREATE FUNCTION user_profiles_touch_updated_at() RETURNS trigger AS $$
BEGIN
    IF row(NEW.*) IS DISTINCT FROM row(OLD.*) THEN
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER user_profiles_touch_updated_at_trg
    BEFORE UPDATE ON user_profiles
    FOR EACH ROW
    EXECUTE FUNCTION user_profiles_touch_updated_at();
