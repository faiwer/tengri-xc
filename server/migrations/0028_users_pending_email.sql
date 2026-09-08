-- Staging column for a self-service email change. The new address lands here
-- and `email` keeps working until the confirmation link promotes it, so a typo
-- costs the user one retry instead of the account they typed it into.
--
-- Deliberately not unique: two people may *request* the same address, and only
-- the first to prove it should get it. `users_email_key` decides that race at
-- promote time.

ALTER TABLE users ADD COLUMN pending_email text;
