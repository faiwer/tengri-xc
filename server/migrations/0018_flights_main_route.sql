-- Add a surrogate key to routes so flights can reference a single row.
ALTER TABLE routes
    ADD COLUMN id bigint GENERATED ALWAYS AS IDENTITY UNIQUE;

-- Denormalize the three hot scalars from the chosen route into flights so
-- list queries (sorted / filtered by distance or score) don't need a JOIN.
-- main_route_id is the FK for detail views (turnpoints, closure, leg distances).
ALTER TABLE flights
    ADD COLUMN main_route_id   bigint         NULL REFERENCES routes(id) ON DELETE SET NULL,
    ADD COLUMN main_route_type route_type     NULL,
    ADD COLUMN main_score      numeric(7, 2)  NULL,
    ADD COLUMN main_distance   integer        NULL;

-- Per-user list sorted by score or distance (the primary list-view queries).
CREATE INDEX flights_user_score_desc_idx    ON flights (user_id, main_score    DESC);
CREATE INDEX flights_user_score_asc_idx     ON flights (user_id, main_score    ASC);
CREATE INDEX flights_user_distance_desc_idx ON flights (user_id, main_distance DESC);
CREATE INDEX flights_user_distance_asc_idx  ON flights (user_id, main_distance ASC);

-- Global leaderboards.
CREATE INDEX flights_score_desc_idx ON flights (main_score DESC);
CREATE INDEX flights_score_asc_idx  ON flights (main_score ASC);

-- "Show only FAI triangles" filter + sort.
CREATE INDEX flights_type_score_desc_idx ON flights (main_route_type, main_score DESC);
CREATE INDEX flights_type_score_asc_idx  ON flights (main_route_type, main_score ASC);
