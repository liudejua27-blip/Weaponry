#!/usr/bin/env python3
"""MCP002 contract smoke: every checked-in JSON contract must be valid and versioned."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"


def main() -> int:
    required = [
        CONTRACT_ROOT / "manifest.json",
        SCHEMA_ROOT / "audit-event.schema.json",
        SCHEMA_ROOT / "candidate.schema.json",
        SCHEMA_ROOT / "cas-object.schema.json",
        SCHEMA_ROOT / "design-asset-version.schema.json",
        SCHEMA_ROOT / "job-event.schema.json",
        SCHEMA_ROOT / "project.schema.json",
        SCHEMA_ROOT / "runtime-capabilities.schema.json",
        SCHEMA_ROOT / "runtime-tool.schema.json",
        SCHEMA_ROOT / "runtime-project.schema.json",
        SCHEMA_ROOT / "runtime-snapshot.schema.json",
        SCHEMA_ROOT / "runtime-job.schema.json",
        SCHEMA_ROOT / "runtime-error.schema.json",
        SCHEMA_ROOT / "runtime-resource.schema.json",
        SCHEMA_ROOT / "runtime-selection.schema.json",
        SCHEMA_ROOT / "snapshot.schema.json",
        ROOT / "migrations-runtime-v1" / "0001_runtime.sql",
    ]
    missing = [str(path.relative_to(ROOT)) for path in required if not path.exists()]
    if missing:
        raise SystemExit(f"missing MCP002 contract files: {missing}")

    for path in sorted(SCHEMA_ROOT.glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
            raise SystemExit(f"schema draft missing: {path}")
        if not str(document.get("$id", "")).startswith("https://forgecad.local/contracts/"):
            raise SystemExit(f"schema id missing: {path}")

    manifest = json.loads((CONTRACT_ROOT / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("contract_set") != "forgecad-runtime-contracts@1":
        raise SystemExit("unexpected contract set")
    if manifest.get("model_calls") is not False:
        raise SystemExit("MCP002 contracts must declare model_calls=false")
    actual_schemas = sorted(path.name for path in SCHEMA_ROOT.glob("*.json"))
    declared_schemas = sorted(manifest.get("schemas", []))
    if actual_schemas != declared_schemas:
        raise SystemExit("contract manifest schema list does not match checked-in schemas")
    print(f"ForgeCAD contracts OK: {len(actual_schemas)} schemas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
