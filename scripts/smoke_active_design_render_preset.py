#!/usr/bin/env python3
"""R001 smoke for Snapshot-owned camera/light presets and deterministic writes."""

from __future__ import annotations

import asyncio
import hashlib
import json
import os
import tempfile
from pathlib import Path

from smoke_active_design_api import (
    _error_code,
    _request,
    _seed_agent_head,
    _seed_legacy_current,
    _seed_project,
)


ROOT = Path(__file__).resolve().parents[1]


def _canonical(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-r001-render-preset-") as raw:
        root = Path(raw)
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE"] = "1"
        os.environ["FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE"] = "1"
        os.environ["FORGECAD_K001_PACKAGED_PROBE"] = "1"

        from forgecad_agent.application.agent_models import ActiveDesignSnapshot
        from forgecad_agent.infrastructure.db import SQLiteConnectionFactory
        from wushen_agent.main import create_test_only_legacy_product_core_app

        app = create_test_only_legacy_product_core_app()
        factory = SQLiteConnectionFactory(root / "library" / "library.db")
        _seed_project(factory, "prj_r001_agent")
        _seed_project(factory, "prj_r001_legacy")
        _seed_agent_head(
            factory,
            "prj_r001_agent",
            "assetver_r001_agent",
            "mg_r001_agent",
            "part_r001_body",
        )
        _seed_legacy_current(factory, "prj_r001_legacy")

        status, headers, initial = asyncio.run(
            _request(app, "GET", "/api/v1/projects/prj_r001_agent/active-design")
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-1"'
        assert initial["render_preset"]["camera_view"] == "iso"
        assert initial["render_preset"]["light_preset"] == "cad_neutral"
        fingerprint = hashlib.sha256(
            _canonical(initial["render_preset"]).encode("utf-8")
        ).hexdigest()
        assert (
            fingerprint
            == hashlib.sha256(
                _canonical(initial["render_preset"]).encode("utf-8")
            ).hexdigest()
        )

        change = {
            "client_request_id": "r001-preset-1",
            "snapshot_revision": 1,
            "camera_view": "front",
            "light_preset": "soft_studio",
        }
        change_headers = {"Idempotency-Key": "r001-preset-key"}
        status, headers, updated = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_r001_agent/active-design:render-preset",
                payload=change,
                headers=change_headers,
            )
        )
        assert status == 200 and headers["etag"] == 'W/"active-design-2"'
        assert updated["revision"] == 2
        assert updated["render_preset"]["camera_view"] == "front"
        assert updated["render_preset"]["light_preset"] == "soft_studio"

        status, headers, replay = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_r001_agent/active-design:render-preset",
                payload=change,
                headers=change_headers,
            )
        )
        assert (
            status == 200
            and headers["etag"] == 'W/"active-design-2"'
            and replay == updated
        )

        conflict = {**change, "camera_view": "top"}
        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_r001_agent/active-design:render-preset",
                payload=conflict,
                headers=change_headers,
            )
        )
        assert status == 409 and _error_code(error) == "IDEMPOTENCY_CONFLICT"

        stale = {
            **change,
            "client_request_id": "r001-preset-stale",
            "snapshot_revision": 1,
            "camera_view": "right",
        }
        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_r001_agent/active-design:render-preset",
                payload=stale,
                headers={"Idempotency-Key": "r001-preset-stale-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_STALE"

        status, _, error = asyncio.run(
            _request(
                app,
                "POST",
                "/api/v1/projects/prj_r001_legacy/active-design:render-preset",
                payload={
                    "client_request_id": "r001-legacy",
                    "snapshot_revision": 1,
                    "camera_view": "front",
                    "light_preset": "soft_studio",
                },
                headers={"Idempotency-Key": "r001-legacy-key"},
            )
        )
        assert status == 409 and _error_code(error) == "ACTIVE_DESIGN_LEGACY_READ_ONLY"

        mismatched = dict(updated)
        mismatched["render_preset"] = {
            **mismatched["render_preset"],
            "asset_version_id": "assetver_other",
        }
        try:
            ActiveDesignSnapshot.model_validate(mismatched)
        except ValueError:
            pass
        else:
            raise AssertionError("cross-asset render preset was accepted")

    print("R001 ActiveDesignRenderPreset Snapshot/CAS/idempotency/legacy smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
