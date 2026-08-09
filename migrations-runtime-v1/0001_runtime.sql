CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_meta (key, value) VALUES ('runtime_schema_version', '1');

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
    source_version_id TEXT,
    prepared_object_id TEXT,
    prepared_object_sha256 TEXT,
    state TEXT NOT NULL CHECK (state IN ('prepared', 'compiling', 'evaluating', 'reviewable', 'confirmed', 'rejected', 'failed', 'expired')),
    request_sha256 TEXT NOT NULL,
    manifest_hash TEXT,
    quality_report_id TEXT,
    quality_hard_gate_passed INTEGER NOT NULL DEFAULT 0 CHECK (quality_hard_gate_passed IN (0, 1)),
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

CREATE TABLE IF NOT EXISTS reference_evidence (
    reference_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
    mime TEXT NOT NULL CHECK (mime IN ('image/png', 'image/jpeg')),
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    frame_count INTEGER NOT NULL CHECK (frame_count = 1),
    import_mode TEXT NOT NULL CHECK (import_mode IN ('inline_content', 'codex_local_file')),
    authorization_json TEXT NOT NULL,
    derived_object_sha256 TEXT REFERENCES objects(sha256),
    canonical_sha256 TEXT NOT NULL,
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
    tool TEXT NOT NULL CHECK (tool IN ('candidate_confirm', 'candidate_reject', 'restore_confirm', 'export_confirm')),
    base_version_id TEXT,
    prepared_object_id TEXT NOT NULL,
    prepared_object_sha256 TEXT NOT NULL,
    quality_report_id TEXT,
    summary_sha256 TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected', 'expired')),
    expires_at TEXT NOT NULL,
    session_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS export_manifests (
    export_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    version_id TEXT NOT NULL REFERENCES design_asset_versions(version_id),
    format TEXT NOT NULL,
    profile TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL REFERENCES objects(sha256),
    artifact_hashes_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('prepared', 'confirmed', 'rejected', 'failed')),
    approval_receipt_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS write_idempotency (
    idempotency_key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    tool TEXT NOT NULL,
    request_sha256 TEXT NOT NULL,
    response_json TEXT NOT NULL,
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
CREATE INDEX IF NOT EXISTS reference_evidence_project_idx ON reference_evidence(project_id, created_at DESC, reference_id ASC);
CREATE INDEX IF NOT EXISTS audit_project_idx ON audit_events(project_id, created_at ASC, audit_id ASC);
CREATE INDEX IF NOT EXISTS idempotency_project_idx ON write_idempotency(project_id, created_at ASC, idempotency_key ASC);
CREATE INDEX IF NOT EXISTS export_manifests_project_idx ON export_manifests(project_id, created_at DESC, export_id ASC);
