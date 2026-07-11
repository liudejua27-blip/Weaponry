#!/usr/bin/env python3
"""Regression smoke for independent ModuleAsset catalog metadata and review state."""
from __future__ import annotations

import json
import tempfile
from pathlib import Path

from smoke_r2_concept_projects import (
    _assert,
    _free_port,
    _json_request,
    _json_request_allow_error,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r2_module_registry import _manifest, _minimal_glb, _register


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r3_catalog_") as temporary_directory:
        root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(root, port)
        try:
            _wait_for_health(base_url, process)
            payload = _minimal_glb("catalog-core")
            manifest = _manifest(
                module_id="module_catalog_core",
                asset_id="asset_catalog_core",
                category="core_shell",
                payload=payload,
                connectors=[],
            )
            registered = _register(
                base_url,
                manifest,
                payload,
                "packs/catalog/core.glb",
                "r3-catalog-register",
            )
            defaults = registered["catalog_metadata"]
            _assert(defaults["origin_claim"] == "self_declared_original", "default origin claim mismatch")
            _assert(defaults["review_status"] == "pending_review", "default review status mismatch")

            invalid_status, _ = _json_request_allow_error(
                base_url,
                "/api/v1/module-assets/module_catalog_core/catalog-metadata",
                method="PUT",
                body={
                    "client_request_id": "r3-catalog-invalid-review",
                    "display_name": "Catalog Core",
                    "description": "Original visual shell.",
                    "tags": ["core", "original"],
                    "catalog_path": "shell/core",
                    "origin_claim": "self_declared_original",
                    "creator_name": "Asset Author",
                    "review_status": "approved",
                    "reviewer_name": "Asset Author",
                    "reviewed_at": "2026-07-11T09:00:00Z",
                },
                idempotency_key="r3-catalog-invalid-review",
            )
            _assert(invalid_status == 422, "self-review approval must be rejected")

            updated = _json_request(
                base_url,
                "/api/v1/module-assets/module_catalog_core/catalog-metadata",
                method="PUT",
                body={
                    "client_request_id": "r3-catalog-approved-review",
                    "display_name": "Catalog Core",
                    "description": "Original visual shell for the concept workbench.",
                    "tags": ["core", "original", "industrial"],
                    "catalog_path": "shell/core",
                    "origin_claim": "self_declared_original",
                    "creator_name": "Asset Author",
                    "review_status": "approved",
                    "reviewer_name": "Independent Reviewer",
                    "reviewed_at": "2026-07-11T09:15:00Z",
                    "review_note": "Visual provenance and mesh handoff reviewed.",
                },
                idempotency_key="r3-catalog-approved-review",
            )
            metadata = updated["catalog_metadata"]
            _assert(metadata["review_status"] == "approved", "approved status was not persisted")
            _assert(metadata["reviewer_name"] == "Independent Reviewer", "reviewer was not persisted")

            filtered = _json_request(
                base_url,
                "/api/v1/module-assets?review_status=approved&tag=industrial&query=visual&catalog_path=shell",
                method="GET",
            )
            _assert(len(filtered["items"]) == 1, "catalog metadata filters did not compose")
            _assert(filtered["items"][0]["catalog_metadata"]["display_name"] == "Catalog Core", "catalog list metadata mismatch")
            print(json.dumps({"ok": True, "module_id": manifest["module_id"], "review_status": "approved"}, ensure_ascii=False))
        finally:
            _stop_agent(process)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
