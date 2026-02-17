CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE memories (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT NOT NULL,
    embedding vector(1536),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(agent_id, key)
);

CREATE INDEX idx_memories_agent_id ON memories(agent_id);
CREATE INDEX idx_memories_category ON memories(category);

CREATE TABLE cron_jobs (
    id TEXT PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    expression TEXT NOT NULL,
    command TEXT NOT NULL,
    schedule JSONB,
    job_type TEXT NOT NULL DEFAULT 'shell',
    prompt TEXT,
    name TEXT,
    session_target TEXT NOT NULL DEFAULT 'isolated',
    model TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    delivery JSONB,
    delete_after_run BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    next_run TIMESTAMPTZ NOT NULL,
    last_run TIMESTAMPTZ,
    last_status TEXT,
    last_output TEXT
);

CREATE INDEX idx_cron_jobs_agent_id ON cron_jobs(agent_id);
CREATE INDEX idx_cron_jobs_next_run ON cron_jobs(next_run);

CREATE TABLE cron_runs (
    id SERIAL PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES cron_jobs(id) ON DELETE CASCADE,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    output TEXT,
    duration_ms BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cron_runs_job_id ON cron_runs(job_id);
