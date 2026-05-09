-- Auth columns on `users`. The first migration only carried `name`
-- (display) and `id`; this one teaches the table how to log somebody
-- in. Designed so that:
--
-- * Imports can land — `login` / `email` / `password_hash` are all
--   nullable. CLI-created users (`tengri add` runs in `ensure_user_exists`)
--   never set them; the leonardo importer fills all three.
-- * OAuth-only accounts are first-class — `password_hash` stays
--   NULL, login flow refuses password verification, OAuth provider
--   linkage lives in a separate table when we add it.
-- * Login names are stored as the user typed them (so `Stepanov`
--   stays `Stepanov` everywhere we display it) but compared
--   case-insensitively. Login queries use `WHERE LOWER(login) =
--   LOWER($1)`; uniqueness is enforced via the `users_login_key`
--   functional unique index on `LOWER(login)`. This matches the
--   contract Leonardo XC has on `leonardo_users.username`
--   (`utf8mb3_general_ci`), so a pilot whose Leonardo login is
--   `Faiwer` can log in as `faiwer` / `FAIWER` like they're used
--   to. Email is treated as fully case-insensitive: lowercased
--   on write, then compared as-is — there's no display value
--   worth preserving for emails.
-- * Permissions ride a single `int` bitfield. The names live in the
--   `tengri_server::user::Permissions` Rust type. The COMMENT below
--   mirrors that for the benefit of anyone reading the DB directly.

-- Where the row originated. Imports tag themselves; CLI / future
-- HTTP signups use the default. We deliberately don't put OAuth
-- providers here: a single user can have many OAuth links over a
-- lifetime (forgot-Google, switched-to-Apple) and lumping that into
-- this column would force a destructive UPDATE on every link change.
-- OAuth lives in a future `user_oauth_links(user_id, provider, ...)`
-- table.
CREATE TYPE user_source AS ENUM ('internal', 'leo');


ALTER TABLE users
    ADD COLUMN login             text,
    ADD COLUMN email             text,
    ADD COLUMN password_hash     text,
    ADD COLUMN source            user_source NOT NULL DEFAULT 'internal',
    -- Bitfield. Bit 0 set = the account can log in at all (clear
    -- this to soft-disable). Higher bits unlock manage-* powers.
    -- See `tengri_server::user::Permissions` for the full layout.
    ADD COLUMN permissions       int         NOT NULL DEFAULT 1,
    ADD COLUMN email_verified_at timestamptz,
    ADD COLUMN last_login_at     timestamptz,
    ADD COLUMN created_at        timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at        timestamptz NOT NULL DEFAULT now();

COMMENT ON COLUMN users.permissions IS
    'Bitfield: bit 0 = can_authorize (login), bit 1 = manage_tracks, bit 2 = manage_users, bit 3 = manage_settings';


-- Partial unique indexes: nullable columns + uniqueness need this
-- shape (a plain UNIQUE(...) would forbid more than one NULL row in
-- some Postgres setups, and is misleading either way — the contract
-- we want is "non-null values are unique"). The predicate makes the
-- index tiny when most rows are still legacy / OAuth-only.
--
-- `users_login_key` is a *functional* unique index on `LOWER(login)`:
-- the on-disk value keeps its casing, the constraint folds. Login
-- lookups must match the index expression to use it — i.e. write
-- `WHERE LOWER(login) = LOWER($1)`, not `WHERE login = $1`.
CREATE UNIQUE INDEX users_login_key ON users (LOWER(login)) WHERE login IS NOT NULL;
CREATE UNIQUE INDEX users_email_key ON users (email)         WHERE email IS NOT NULL;


-- `updated_at` housekeeping. Trivial trigger, keeps app code from
-- having to remember it. The `IS DISTINCT FROM` (rather than `<>`)
-- handles NULL-on-both-sides correctly so a touch with no real
-- change is a no-op.
CREATE FUNCTION users_touch_updated_at() RETURNS trigger AS $$
BEGIN
    IF row(NEW.*) IS DISTINCT FROM row(OLD.*) THEN
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_touch_updated_at_trg
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION users_touch_updated_at();
