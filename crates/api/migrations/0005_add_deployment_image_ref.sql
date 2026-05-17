ALTER TABLE deployments
    ADD COLUMN image_ref TEXT;

CREATE INDEX idx_deployments_image_ref ON deployments(image_ref) WHERE image_ref IS NOT NULL;
