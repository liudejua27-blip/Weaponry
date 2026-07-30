PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Validated unified author source retained between the accounted Author call
-- and the one permitted visual Patch call. Reference/candidate image bytes,
-- credentials, prompts and raw Provider responses are never persisted here.
CREATE TABLE IF NOT EXISTS e005_visual_review_checkpoints (
  authorization_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  task_payload_sha256 TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('awaiting_visual_review', 'completed', 'reconciliation_required')),
  author_source_json TEXT NOT NULL,
  author_source_sha256 TEXT NOT NULL,
  author_reservation_id TEXT NOT NULL,
  author_budget_evidence_json TEXT NOT NULL,
  author_budget_evidence_sha256 TEXT NOT NULL,
  author_provider_usage_json TEXT NOT NULL,
  author_provider_usage_sha256 TEXT NOT NULL,
  visual_reservation_id TEXT,
  visual_budget_evidence_sha256 TEXT,
  visual_review_evidence_sha256 TEXT,
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  PRIMARY KEY(authorization_id, task_id),
  FOREIGN KEY(authorization_id, task_id, task_payload_sha256)
    REFERENCES e005_provider_authorized_tasks(authorization_id, task_id, task_payload_sha256),
  FOREIGN KEY(author_reservation_id)
    REFERENCES e005_provider_call_reservations(reservation_id),
  FOREIGN KEY(visual_reservation_id)
    REFERENCES e005_provider_call_reservations(reservation_id),
  CHECK (updated_at_unix_ms >= created_at_unix_ms),
  CHECK (
    (state = 'awaiting_visual_review'
      AND visual_reservation_id IS NULL
      AND visual_budget_evidence_sha256 IS NULL
      AND visual_review_evidence_sha256 IS NULL)
    OR (state = 'completed'
      AND visual_reservation_id IS NOT NULL
      AND visual_budget_evidence_sha256 IS NOT NULL
      AND visual_review_evidence_sha256 IS NOT NULL)
    OR state = 'reconciliation_required'
  )
);

CREATE INDEX IF NOT EXISTS idx_e005_visual_review_checkpoint_recovery
  ON e005_visual_review_checkpoints(state, authorization_id, task_id);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0047', 'e005_visual_review_checkpoint', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0047', 'e005_visual_review_checkpoint');

COMMIT;
