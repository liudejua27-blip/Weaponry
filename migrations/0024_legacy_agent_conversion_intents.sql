PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- An explicit, auditable user hand-off is required before a newly committed
-- Agent asset may replace a legacy read-only active design.  The legacy
-- Project, ConceptVersion and ModuleGraph remain untouched.
CREATE TABLE legacy_agent_conversion_intents (
  project_id TEXT PRIMARY KEY REFERENCES projects(project_id) ON DELETE CASCADE,
  legacy_version_id TEXT NOT NULL REFERENCES project_versions(version_id) ON DELETE RESTRICT,
  legacy_module_graph_id TEXT NOT NULL REFERENCES module_graphs(graph_id) ON DELETE RESTRICT,
  snapshot_revision INTEGER NOT NULL CHECK (snapshot_revision >= 1),
  requested_at TEXT NOT NULL
);

INSERT INTO schema_migrations(version, name)
VALUES ('0024', 'legacy_agent_conversion_intents');

COMMIT;
