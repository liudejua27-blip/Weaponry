PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS module_asset_catalog_metadata (
  module_id TEXT PRIMARY KEY REFERENCES module_assets(module_id) ON DELETE CASCADE,
  display_name TEXT NOT NULL,
  description TEXT NOT NULL,
  tags_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags_json)),
  catalog_path TEXT NOT NULL,
  origin_claim TEXT NOT NULL CHECK (origin_claim IN ('self_declared_original', 'third_party', 'unknown')),
  creator_name TEXT NOT NULL,
  review_status TEXT NOT NULL CHECK (review_status IN ('draft', 'pending_review', 'approved', 'restricted')),
  reviewer_name TEXT,
  reviewed_at TEXT,
  review_note TEXT,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_module_asset_catalog_review
  ON module_asset_catalog_metadata(review_status, catalog_path);

-- Existing bundled modules are authored reference assets awaiting the independently
-- scheduled review. Manifest geometry remains unchanged.
INSERT OR IGNORE INTO module_asset_catalog_metadata (
  module_id, display_name, description, tags_json, catalog_path,
  origin_claim, creator_name, review_status, reviewer_name, reviewed_at,
  review_note, updated_at
)
SELECT
  module_id,
  replace(module_id, 'module_', ''),
  '资产信息待补充。',
  '[]',
  category,
  'self_declared_original',
  'ForgeCAD Author',
  'pending_review',
  NULL,
  NULL,
  '已声明为本人原创，等待独立审阅。',
  updated_at
FROM module_assets;

-- Record this migration exactly once. Earlier revisions omitted the ledger
-- entry, causing harmless but unnecessary replay on every startup.
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0017', 'module_asset_catalog_metadata');

COMMIT;
