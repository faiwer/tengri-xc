-- `email` now only ever holds a proven address: self-service changes go to
-- `pending_email` and are promoted by the confirmation link, admins set the
-- address explicitly, and OAuth providers vouch for the one they hand over.
-- That makes `email IS NOT NULL` the verified check, and the timestamp
-- redundant.

ALTER TABLE users DROP COLUMN email_verified_at;
