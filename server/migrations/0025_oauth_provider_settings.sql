-- Per-provider OAuth credentials, edited by admins under System → OAuth. The
-- `oauth_provider` enum is defined in `0024_user_oauth_links.sql`.
--
-- Not seeded: a row exists only once an admin configures that provider.
-- `client_id` + `client_secret` are a required pair (both NOT NULL) — a row's
-- existence therefore means "this provider is actually configured", so there
-- are no half-filled placeholder rows to reason about. The read side returns
-- only the rows that exist; the client owns the canonical list of providers and
-- fills the gaps for ones not configured yet.
--
-- `enabled` gates whether the provider is offered for login (a later phase); it
-- defaults FALSE so freshly-entered credentials don't go live until an admin
-- flips it on.

CREATE TABLE oauth_provider_settings (
    provider      oauth_provider PRIMARY KEY,
    client_id     text        NOT NULL,
    client_secret text        NOT NULL,
    enabled       boolean     NOT NULL DEFAULT FALSE,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now()
);

-- Same updated_at autoset trigger as the other settings tables (0005 /
-- 0002). Table-private function; `IS DISTINCT FROM` makes a no-op touch a
-- no-op.
CREATE FUNCTION oauth_provider_settings_touch_updated_at() RETURNS trigger AS $$
BEGIN
    IF row(NEW.*) IS DISTINCT FROM row(OLD.*) THEN
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER oauth_provider_settings_touch_updated_at_trg
    BEFORE UPDATE ON oauth_provider_settings
    FOR EACH ROW
    EXECUTE FUNCTION oauth_provider_settings_touch_updated_at();
