CREATE TABLE build_log_lines (
    id              BIGSERIAL PRIMARY KEY,
    deployment_id   UUID NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
    line            TEXT NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_build_log_lines_deployment ON build_log_lines(deployment_id, id);
