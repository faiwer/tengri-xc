-- `sessions_valid_from` invalidates every session token minted before it: the
-- slide middleware compares it against the JWT's `iat` and clears the cookie
-- when the token predates it. Password writes stamp it today; anything else
-- that should end existing sessions can stamp it later.
--
-- `password_reset_sends` holds the last two reset-mail timestamps, `[prev,
-- last]`. The gap between them drives the escalating resend delay, and the last
-- entry is what the emailed token is bound to, which is what makes the link
-- single-use.

ALTER TABLE users
    ADD COLUMN sessions_valid_from   timestamptz,
    ADD COLUMN password_reset_sends  timestamptz[];
