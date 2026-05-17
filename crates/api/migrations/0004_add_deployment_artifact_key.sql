ALTER TABLE deployments
    ADD COLUMN artifact_key TEXT;

CREATE INDEX idx_deployments_artifact_key ON deployments(artifact_key) WHERE artifact_key IS NOT NULL;
