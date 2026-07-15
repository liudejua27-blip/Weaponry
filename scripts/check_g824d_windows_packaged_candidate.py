#!/usr/bin/env python3
"""Validate a Windows x64 G824D report produced by the packaged runner."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))
    assert report["schema_version"] == "ForgeCADCSGWindowsPackagedEvidence@1"
    assert report["machine"]["machine"].lower() in {"amd64", "x86_64"}
    assert report["candidate"]["name"] == "manifold_python"
    assert report["candidate"]["health"]["passed"] is True
    assert report["candidate"]["provider_calls"] == 0
    provenance = report["packaged_provenance"]
    assert provenance["deterministic_provenance"] is True
    assert provenance["property_channels_verified"] is True
    assert provenance["simplify_provenance_verified"] is True
    assert len(provenance["fixtures"]) == 6
    assert provenance["fixtures"]["near_degenerate_subtract"]["status"] == "rejected"
    for fixture_id, fixture in provenance["fixtures"].items():
        if fixture_id != "near_degenerate_subtract":
            assert fixture["status"] == "passed"
            assert fixture["forgecad_readback_verified"] is True
            assert fixture["glb_provenance_verified"] is True
    assert len(report["lifecycle_cases"]) == 3
    assert all(
        case["process_reaped"]
        and case["database_unchanged"]
        and case["object_store_unchanged"]
        and not case["authoritative_paths_passed_to_child"]
        and not case["partial_glb_emitted"]
        and case["staging_cleanup_verified"]
        for case in report["lifecycle_cases"]
    )
    assert report["promotion_transaction"]["injected_failure_rollback_verified"] is True
    assert report["promotion_transaction"]["version_head_snapshot_atomic_success_verified"] is True
    assert report["decision"]["status"] == "windows_packaged_runtime_passed"
    assert report["decision"]["production_dependency_added"] is False
    print("G824D Windows packaged candidate passed: provenance, lifecycle, and promotion evidence are complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
