PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE design_briefs
  ADD COLUMN planner_provenance_json TEXT NOT NULL DEFAULT '{}'
  CHECK (json_valid(planner_provenance_json));

ALTER TABLE design_variants
  ADD COLUMN recommended_module_ids_json TEXT NOT NULL DEFAULT '[]'
  CHECK (json_valid(recommended_module_ids_json));

ALTER TABLE design_variants
  ADD COLUMN rationale_json TEXT NOT NULL DEFAULT '[]'
  CHECK (json_valid(rationale_json));

ALTER TABLE design_variants
  ADD COLUMN planner_provenance_json TEXT NOT NULL DEFAULT '{}'
  CHECK (json_valid(planner_provenance_json));

UPDATE design_briefs
SET planner_provenance_json = json_object(
  'generator', 'deterministic_rules',
  'provider_id', 'legacy_deterministic_template',
  'provider_type', 'deterministic',
  'model', NULL,
  'fallback_used', json('false'),
  'input_sha256', printf('%064d', 0),
  'output_sha256', printf('%064d', 0),
  'registry_module_ids', json_array(),
  'warnings', json_array('Migrated from pre-provenance deterministic template row.')
)
WHERE planner_provenance_json = '{}';

UPDATE design_variants
SET planner_provenance_json = json_object(
  'generator', 'deterministic_rules',
  'provider_id', 'legacy_deterministic_template',
  'provider_type', 'deterministic',
  'model', NULL,
  'fallback_used', json('false'),
  'input_sha256', printf('%064d', 0),
  'output_sha256', printf('%064d', 0),
  'registry_module_ids', json_array(),
  'warnings', json_array('Migrated from pre-provenance deterministic template row.')
)
WHERE planner_provenance_json = '{}';

INSERT INTO schema_migrations(version, name)
VALUES ('0014', 'concept_planner_provenance');

COMMIT;
