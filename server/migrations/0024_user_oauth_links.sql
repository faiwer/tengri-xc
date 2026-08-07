-- OAuth provider linkage, the table `0002_users_auth.sql` anticipated. One user
-- can hold many links (different providers, or even two accounts of the same
-- provider), so this is a child table rather than columns on `users`.
--
-- Identity is `(provider, provider_user_id)` — the provider's stable subject
-- id, never email. Email is a coincidence across providers (one human has
-- different emails on Google/GitHub/Microsoft) and X returns none at all, so it
-- can't be an identity key; it rides along only as a display snapshot.
--
-- No access/refresh tokens: this is login-only. We use the token once in the
-- callback to read the subject id, then discard it. Storing it would mean
-- secret-at-rest handling for no current feature.

CREATE TYPE oauth_provider AS ENUM ('google', 'facebook', 'x', 'microsoft', 'github');

CREATE TABLE user_oauth_links (
    user_id          int            NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider         oauth_provider NOT NULL,
    -- Provider's stable subject id (Google `sub`, GitHub numeric id, …).
    -- Text so every provider's id shape fits one column.
    provider_user_id text           NOT NULL,
    -- Snapshot captured at link time, display-only, never consulted for
    -- identity. Nullable: X gives no email, some providers omit a name.
    email            text,
    display_name     text,
    created_at       timestamptz    NOT NULL DEFAULT now(),

    -- One provider account maps to exactly one user. `user_id` sits
    -- outside the PK so a user can hold several links.
    PRIMARY KEY (provider, provider_user_id)
);

-- "List my connected accounts" in settings, and the `ON DELETE CASCADE`
-- cleanup when a user row goes.
CREATE INDEX user_oauth_links_user_id_idx ON user_oauth_links (user_id);
