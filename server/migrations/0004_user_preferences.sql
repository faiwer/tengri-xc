-- Per-user display preferences. One row per user, always — unlike
-- `user_profiles` (which is genuinely optional and lives on a `LEFT
-- JOIN`), preferences always have a value (defaults if untouched).
-- The trigger + backfill below guarantee that invariant so readers
-- can `JOIN` (not `LEFT JOIN`) and never special-case a missing row.
--
-- Storage shape: typed columns with CHECK constraints rather than
-- a single JSONB blob. Each pref is a small enum we want the DB to
-- reject bad values for (catches typos at insert time), and adding
-- a new pref is `ALTER TABLE … ADD COLUMN … DEFAULT 'system' NOT
-- NULL` (one line, online — Postgres ≥11 doesn't rewrite the table).
--
-- The literal `'system'` is a sentinel meaning "follow the user's
-- locale". The client resolves it to a concrete unit at format time
-- using `Intl`; the server stores it as-is. That keeps the wire
-- contract symmetric with what the user picked in the settings UI
-- ("System") instead of leaking a guessed-from-IP guess into the
-- saved row.

CREATE TABLE user_preferences (
    user_id      int         PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- '12-hour' / '24-hour' / locale default.
    time_format  text        NOT NULL DEFAULT 'system'
                 CHECK (time_format IN ('system', 'h12', 'h24')),
    -- Day-month-year / month-day-year / locale default. ISO not
    -- offered: it reads as engineer-shaped, not pilot-shaped.
    date_format  text        NOT NULL DEFAULT 'system'
                 CHECK (date_format IN ('system', 'dmy', 'mdy')),
    -- Drives both altitude (m vs ft) and XC distance (km vs mi).
    -- Combining them avoids the 8-way combinatorial explosion you'd
    -- get from a per-quantity setting; the rare hybrid pilot uses
    -- the unit-specific overrides below.
    units        text        NOT NULL DEFAULT 'system'
                 CHECK (units IN ('system', 'metric', 'imperial')),
    -- Independent of `units` because metric pilots flying with an
    -- imperial-import vario instrument do exist (and vice versa).
    -- m/s vs ft/min — `ft/s` would be too coarse a resolution for
    -- typical climb rates (1 m/s ≈ 200 ft/min ≈ 3 ft/s).
    vario_unit   text        NOT NULL DEFAULT 'system'
                 CHECK (vario_unit IN ('system', 'mps', 'fpm')),
    -- Ground speed unit. No knots: knots are a wind/aviation thing,
    -- and we don't surface wind data yet. When wind ships, it gets
    -- its own column with knots in the option set.
    speed_unit   text        NOT NULL DEFAULT 'system'
                 CHECK (speed_unit IN ('system', 'kmh', 'mph')),
    -- Calendar pickers only. 'sat' (Middle East) intentionally
    -- omitted — add it the day a user from there asks.
    week_start   text        NOT NULL DEFAULT 'system'
                 CHECK (week_start IN ('system', 'mon', 'sun')),
    updated_at   timestamptz NOT NULL DEFAULT now()
);


-- Eager creation: every new `users` row gets a sibling preferences
-- row with all-defaults. Means readers can `JOIN user_preferences`
-- (never `LEFT JOIN`) and the SELECT list doesn't need `COALESCE`
-- defaults.
CREATE FUNCTION user_preferences_create_for_new_user() RETURNS trigger AS $$
BEGIN
    INSERT INTO user_preferences (user_id) VALUES (NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER user_preferences_create_for_new_user_trg
    AFTER INSERT ON users
    FOR EACH ROW
    EXECUTE FUNCTION user_preferences_create_for_new_user();


-- Backfill: every existing user gets their default row. `ON CONFLICT
-- DO NOTHING` makes this re-runnable in dev where the trigger and
-- the backfill might race during a migration replay.
INSERT INTO user_preferences (user_id)
    SELECT id FROM users
    ON CONFLICT (user_id) DO NOTHING;


-- Same `updated_at` autoset shape as `users` and `user_profiles`.
-- See those migrations for the rationale on `IS DISTINCT FROM`.
CREATE FUNCTION user_preferences_touch_updated_at() RETURNS trigger AS $$
BEGIN
    IF row(NEW.*) IS DISTINCT FROM row(OLD.*) THEN
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER user_preferences_touch_updated_at_trg
    BEFORE UPDATE ON user_preferences
    FOR EACH ROW
    EXECUTE FUNCTION user_preferences_touch_updated_at();
