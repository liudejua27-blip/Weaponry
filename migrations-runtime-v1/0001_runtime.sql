CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_meta (key, value) VALUES ('runtime_schema_version', '1');

CREATE TABLE IF NOT EXISTS writer_lease (
    lease_id INTEGER PRIMARY KEY CHECK (lease_id = 1),
    owner TEXT NOT NULL,
    lease_token_hash TEXT NOT NULL,
    acquired_at INTEGER NOT NULL,
    heartbeat_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    project_id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    policy_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    active_snapshot_revision INTEGER NOT NULL DEFAULT 0 CHECK (active_snapshot_revision >= 0),
    head_snapshot_id TEXT,
    canonical_sha256 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS snapshots (
    snapshot_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    parent_snapshot_id TEXT,
    candidate_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    status TEXT NOT NULL CHECK (status IN ('draft', 'preview', 'confirmed', 'reverted')),
    manifest_hash TEXT NOT NULL,
    canonical_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS candidates (
    candidate_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    base_version_id TEXT,
    state TEXT NOT NULL CHECK (state IN ('prepared', 'compiling', 'evaluating', 'reviewable', 'confirmed', 'rejected', 'failed', 'expired')),
    request_sha256 TEXT NOT NULL,
    manifest_hash TEXT,
    canonical_sha256 TEXT NOT NULL,
    error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS design_asset_versions (
    version_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    parent_version_id TEXT,
    candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
    manifest_hash TEXT NOT NULL,
    canonical_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_jobs (
    job_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    kind TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'waiting_for_input', 'succeeded', 'failed', 'cancelled')),
    progress INTEGER NOT NULL CHECK (progress BETWEEN 0 AND 100),
    request_sha256 TEXT NOT NULL,
    checkpoint_sha256 TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runtime_job_events (
    job_id TEXT NOT NULL REFERENCES runtime_jobs(job_id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (job_id, sequence)
);

CREATE TABLE IF NOT EXISTS runtime_job_checkpoints (
    job_id TEXT NOT NULL REFERENCES runtime_jobs(job_id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    checkpoint_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (job_id, sequence)
);

CREATE TABLE IF NOT EXISTS objects (
    sha256 TEXT PRIMARY KEY,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    mime TEXT NOT NULL,
    kind TEXT NOT NULL,
    reachability TEXT NOT NULL CHECK (reachability IN ('temporary', 'reachable', 'quarantined')),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artifact_manifests (
    manifest_hash TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(project_id),
    object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
    manifest_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approval_receipts (
    approval_receipt_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    prepared_object_id TEXT NOT NULL,
    prepared_object_sha256 TEXT NOT NULL,
    quality_report_id TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected', 'expired')),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_events (
    audit_id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(project_id),
    kind TEXT NOT NULL,
    object_id TEXT,
    request_sha256 TEXT,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS snapshots_project_idx ON snapshots(project_id, revision DESC, snapshot_id ASC);
CREATE INDEX IF NOT EXISTS candidates_project_idx ON candidates(project_id, updated_at DESC, candidate_id ASC);
CREATE INDEX IF NOT EXISTS versions_project_idx ON design_asset_versions(project_id, created_at DESC, version_id ASC);
CREATE INDEX IF NOT EXISTS jobs_project_idx ON runtime_jobs(project_id, updated_at DESC, job_id ASC);
CREATE INDEX IF NOT EXISTS job_events_cursor_idx ON runtime_job_events(job_id, sequence ASC);
CREATE INDEX IF NOT EXISTS objects_reachability_idx ON objects(reachability, created_at ASC);
CREATE INDEX IF NOT EXISTS audit_project_idx ON audit_events(project_id, created_at ASC, audit_id ASC);
