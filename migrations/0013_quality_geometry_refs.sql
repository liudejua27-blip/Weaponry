PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS quality_finding_geometry_refs (
  finding_id TEXT NOT NULL REFERENCES quality_findings(finding_id) ON DELETE CASCADE,
  ref_index INTEGER NOT NULL CHECK (ref_index >= 0),
  node_id TEXT NOT NULL,
  geometry_ref_json TEXT NOT NULL CHECK (json_valid(geometry_ref_json)),
  PRIMARY KEY (finding_id, ref_index)
);

CREATE INDEX IF NOT EXISTS idx_quality_geometry_refs_node
  ON quality_finding_geometry_refs(node_id, finding_id);

INSERT INTO schema_migrations(version, name)
VALUES ('0013', 'quality_geometry_refs');

COMMIT;
