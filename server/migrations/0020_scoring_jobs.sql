-- Route scoring is CPU-heavy, so an upload persists the flight synchronously
-- (part 1) and defers scoring (part 2) to a global queue drained by a bounded
-- worker pool. This table is the durable record of that queue: one job per
-- flight, surviving restarts (boot re-enqueues anything left `queued`/`running`).

CREATE TYPE scoring_job_state AS ENUM ('queued', 'running', 'done', 'failed');

CREATE TABLE scoring_jobs (
    flight_id   text              PRIMARY KEY REFERENCES flights(id) ON DELETE CASCADE,
    state       scoring_job_state NOT NULL DEFAULT 'queued',
    attempts    smallint          NOT NULL DEFAULT 0,
    error       text              NULL,
    created_at  timestamptz       NOT NULL DEFAULT now(),
    started_at  timestamptz       NULL,
    finished_at timestamptz       NULL
);

-- FIFO drain and cheap "jobs ahead of me" position counting both key off
-- (state = 'queued', created_at).
CREATE INDEX scoring_jobs_queued_idx ON scoring_jobs (created_at) WHERE state = 'queued';
