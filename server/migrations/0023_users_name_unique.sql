-- Case-insensitive uniqueness on display names. `users.name` already has a
-- case-sensitive UNIQUE (the `NOT NULL UNIQUE` in 0001, constraint
-- `users_name_key`); this adds a functional unique index on `LOWER(name)` so
-- "Bob" and "bob" collide too — same shape as `users_login_key`. `name` is
-- `NOT NULL`, so no partial predicate is needed. Uniqueness lookups must match
-- the index expression (`WHERE LOWER(name) = LOWER($1)`) to use it.
CREATE UNIQUE INDEX users_name_lower_key ON users (LOWER(name));
