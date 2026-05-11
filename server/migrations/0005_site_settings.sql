-- Site-wide singleton settings. One row, ever. The first deploy gets a row with
-- defaults from this migration; subsequent edits go through the admin form
-- (`PATCH /admin/site`). There's deliberately no per-tenant scoping — this is a
-- single-tenant instance.
--
-- Storage shape: typed columns rather than a JSONB blob. Each field is
-- something the operator UI exposes by name, and adding a new field is one
-- `ALTER TABLE … ADD COLUMN … DEFAULT … NOT NULL` (or nullable for optional
-- fields). Keeps the SELECT typed end-to-end without a runtime schema layer.
--
-- The `id boolean PRIMARY KEY DEFAULT TRUE CHECK (id)` trick makes a second
-- insert impossible: only `TRUE` satisfies the CHECK, the PK forbids two rows
-- with the same `TRUE`. Cheaper than a separate `is_current` sentinel column
-- and reads obviously in psql.

CREATE TABLE site_settings (
    id           boolean      PRIMARY KEY DEFAULT TRUE CHECK (id),
    site_name    text         NOT NULL DEFAULT 'Tengri XC',
    can_register boolean      NOT NULL DEFAULT TRUE,
    -- Long-form markdown rendered at `/terms` and `/privacy`. NULL
    -- means "not published yet" — `GET /site/{kind}` 404s and the
    -- public footer omits the link.
    tos_md       text,
    privacy_md   text,
    updated_at   timestamptz  NOT NULL DEFAULT now()
);

INSERT INTO site_settings (id) VALUES (TRUE);


-- `updated_at` housekeeping. Same shape as `users` / `user_profiles`
-- / `user_preferences` — single-table trigger function, `IS DISTINCT
-- FROM` so a touch with no real change is a no-op.
CREATE FUNCTION site_settings_touch_updated_at() RETURNS trigger AS $$
BEGIN
    IF row(NEW.*) IS DISTINCT FROM row(OLD.*) THEN
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER site_settings_touch_updated_at_trg
    BEFORE UPDATE ON site_settings
    FOR EACH ROW
    EXECUTE FUNCTION site_settings_touch_updated_at();
