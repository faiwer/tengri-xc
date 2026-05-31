-- `scored_ms` records how long the scorer spent on this route type.
ALTER TABLE routes ADD COLUMN scored_ms integer NOT NULL;
