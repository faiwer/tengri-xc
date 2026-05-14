-- Add a new `'unknown'` value to the `glider_class` enum. Used by the Leonardo
-- importer for per-pilot custom rows it can't classify (HG-flex
-- single_surface-vs-kingpost ambiguity; SP, where Leo records `cat=8` with no
-- class signal). Curated rows never use it — see the CHECK in migration 0013
-- that forbids `class='unknown'` on canonical rows.
--
-- This is the entire migration on purpose: Postgres lifts the "ALTER TYPE … ADD
-- VALUE inside a transaction" restriction in 12, but the new value still can't
-- be *referenced* until the transaction commits. Splitting the ADD VALUE off
-- into its own migration lets 0013 (`CHECK (class <> 'unknown' …)`) use the
-- value safely.

ALTER TYPE glider_class ADD VALUE 'unknown';
