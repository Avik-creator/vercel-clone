-- Enable UUID generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Users
CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    password_hash   TEXT,
    github_id       BIGINT UNIQUE,
    github_login    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_github_id ON users(github_id);

-- Projects
CREATE TABLE projects (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id                UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    slug                    TEXT NOT NULL,
    github_repo             TEXT,
    github_installation_id  BIGINT,
    framework               TEXT,
    build_command           TEXT NOT NULL DEFAULT 'npm run build',
    output_dir              TEXT NOT NULL DEFAULT 'dist',
    root_dir                TEXT,
    env_vars                JSONB NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, slug)
);

CREATE INDEX idx_projects_owner ON projects(owner_id);
CREATE INDEX idx_projects_github_repo ON projects(github_repo);

-- Deployment state enum
CREATE TYPE deployment_state AS ENUM (
    'queued', 'building', 'uploading', 'ready', 'error', 'cancelled'
);

-- Deployments
CREATE TABLE deployments (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    commit_sha          TEXT NOT NULL,
    commit_message      TEXT,
    branch              TEXT NOT NULL,
    state               deployment_state NOT NULL DEFAULT 'queued',
    url                 TEXT,
    is_production       BOOLEAN NOT NULL DEFAULT FALSE,
    build_log           TEXT,
    build_started_at    TIMESTAMPTZ,
    build_finished_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_deployments_project ON deployments(project_id);
CREATE INDEX idx_deployments_state   ON deployments(state);
CREATE INDEX idx_deployments_prod    ON deployments(project_id, is_production) WHERE is_production = TRUE;

-- API keys
CREATE TABLE api_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL,
    key_prefix      TEXT NOT NULL,   -- first 20 chars for efficient lookup
    last_used_at    TIMESTAMPTZ,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_keys_user   ON api_keys(user_id);
CREATE INDEX idx_api_keys_prefix ON api_keys(key_prefix);

-- Auto-update updated_at trigger
CREATE OR REPLACE FUNCTION touch_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_updated_at    BEFORE UPDATE ON users    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE TRIGGER projects_updated_at BEFORE UPDATE ON projects FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE TRIGGER deploys_updated_at  BEFORE UPDATE ON deployments FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
