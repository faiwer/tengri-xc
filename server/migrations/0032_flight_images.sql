-- Rendered link-preview JPEG per flight (`GET /tracks/{id}/og.jpg`). Drawing
-- one costs a track decode, a scoring readback and a satellite tile fetch, so
-- the first request renders and stores it here and later ones serve the bytes.
-- Nothing references this table: a row is a cache entry, deletable on its own,
-- and re-scoring a flight drops it so the next request redraws.

CREATE TABLE flight_images (
    flight_id  text        PRIMARY KEY REFERENCES flights(id) ON DELETE CASCADE,
    bytes      bytea       NOT NULL,
    -- xxh3-64 of `bytes`, served as the HTTP ETag. Its own column so a
    -- revalidation answers from the PK index without detoasting the blob.
    etag       text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
