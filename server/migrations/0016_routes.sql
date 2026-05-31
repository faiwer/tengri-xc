-- Per-flight scored routes: free distance, FAI / free triangles, tasks. Each
-- row is one (kind, sub_type) combination scored against the flight track.
-- Turnpoint lists and OLC closure witnesses are JSONB because their shape
-- varies (real fix vs cylinder crossing vs line crossing).

CREATE TYPE route_type AS ENUM (
    'free_distance',
    'fai_triangle',
    'free_triangle',
    'task'
);

CREATE TYPE route_sub_type AS ENUM (
    'none',
    -- Triangle sub-types
    'olc_closed',   -- 5% closure gap
    'olc_open',     -- 20% closure gap
    'fai_cylinders' -- FAI-style cylinders
);

CREATE TABLE routes (
    flight_id     text           NOT NULL REFERENCES flights(id) ON DELETE CASCADE,
    type          route_type     NOT NULL,
    sub_type      route_sub_type NOT NULL,
    turnpoints    jsonb          NOT NULL,
    leg_distances integer[]      NOT NULL,
    distance      integer        NOT NULL,
    score         numeric(7, 2)  NOT NULL, -- e.g., 163.23
    factor        numeric(2, 1)  NOT NULL, -- 0.0 (excluded), 1.0 (FD), 1.4 (open OLC-FAI-T), 1.6 (closed OLC-FAI-T)
    closure       jsonb, -- for OLC-like triangles

    PRIMARY KEY (flight_id, type, sub_type),

    CONSTRAINT routes_turnpoints_array CHECK (jsonb_typeof(turnpoints) = 'array'),
    CONSTRAINT routes_closure_object CHECK (closure IS NULL OR jsonb_typeof(closure) = 'object'),
    CONSTRAINT routes_non_negative_score CHECK (score >= 0),
    CONSTRAINT routes_non_negative_factor CHECK (factor >= 0),
    CONSTRAINT routes_sub_type_matches_type CHECK (
        (type IN ('free_distance', 'task') AND sub_type = 'none') OR
        (type = 'fai_triangle' AND sub_type IN ('olc_closed', 'olc_open', 'fai_cylinders')) OR
        (type = 'free_triangle' AND sub_type IN ('olc_closed', 'olc_open'))
    ),
    CONSTRAINT routes_closure_matches_sub_type CHECK (
        (sub_type IN ('olc_open', 'olc_closed') AND closure IS NOT NULL) OR
        (sub_type NOT IN ('olc_open', 'olc_closed') AND closure IS NULL)
    )
);

CREATE INDEX routes_type_score_idx ON routes (type, score DESC);
CREATE INDEX routes_score_idx ON routes (score DESC);
