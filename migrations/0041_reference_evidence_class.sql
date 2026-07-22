PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- R007B made coverage class part of the immutable evidence contract after
-- 0039 had already shipped. Keep 0039 immutable and upgrade existing
-- libraries forward. Historical image evidence has no code-owned basis for
-- claiming a contact sheet, so it is conservatively classified as one image;
-- GLB evidence remains strict readback evidence.
DROP TRIGGER IF EXISTS reference_evidence_immutable;

ALTER TABLE reference_evidence
  ADD COLUMN reference_class TEXT
  CHECK (reference_class IN ('single_image', 'multi_view_contact_sheet', 'glb_readback'));

UPDATE reference_evidence
SET reference_class = CASE kind
  WHEN 'image' THEN 'single_image'
  WHEN 'glb' THEN 'glb_readback'
END
WHERE reference_class IS NULL;

CREATE TRIGGER IF NOT EXISTS reference_evidence_class_required
BEFORE INSERT ON reference_evidence
WHEN NEW.reference_class IS NULL
BEGIN
  SELECT RAISE(ABORT, 'Reference evidence class is required');
END;

CREATE TRIGGER IF NOT EXISTS reference_evidence_immutable
BEFORE UPDATE ON reference_evidence
BEGIN
  SELECT RAISE(ABORT, 'Reference evidence is immutable');
END;

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0041', 'reference_evidence_class', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0041', 'reference_evidence_class');

COMMIT;
