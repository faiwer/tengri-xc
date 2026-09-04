-- Turn the OAuth `enabled` boolean into a three-state visibility. The switch
-- couldn't express "configured, but only offered to admins", which is what an
-- operator wants while a provider is still being trialled: usable by anyone
-- holding MANAGE_USERS, hidden from ordinary visitors.
--
--   disabled — hidden from everyone (credentials kept, no login/link offered).
--   admins   — offered only to callers with MANAGE_USERS.
--   public   — offered to everyone (the old `enabled = TRUE`).
--
-- Existing rows carry over: an enabled provider becomes `public`, everything
-- else defaults to `disabled` (matching the old `enabled = FALSE` default).

CREATE TYPE oauth_visibility AS ENUM ('disabled', 'admins', 'public');

ALTER TABLE oauth_provider_settings
    ADD COLUMN visibility oauth_visibility NOT NULL DEFAULT 'disabled';

UPDATE oauth_provider_settings SET visibility = 'public' WHERE enabled = TRUE;

ALTER TABLE oauth_provider_settings DROP COLUMN enabled;
