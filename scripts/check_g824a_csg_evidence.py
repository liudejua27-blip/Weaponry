#!/usr/bin/env python3
"""Validate committed G824A evidence after the G825 production decision."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
report = json.loads((ROOT / "evaluations/csg-g824a/report.json").read_text(encoding="utf-8"))
assert report["schema_version"] == "ForgeCADCSGAdoptionEvidence@1"
assert report["decision"]["selected"] is None
assert report["decision"]["status"] == "evidence_advanced_candidate_not_selected"
assert report["cross_binding_valid_glb_hashes_match"] is True
assert report["stable_error_codes"] == ["CSG_CANCELLED", "CSG_TIMEOUT", "CSG_DEGENERATE_OUTPUT"]
assert len(report["decision"]["remaining_blockers"]) == 3
for candidate in report["candidates"]:
    assert candidate["deterministic_provenance"] is True
    assert candidate["property_channels_verified"] is True
    assert candidate["simplify_provenance_verified"] is True
    assert candidate["valid_fixture_glb_provenance_verified"] is True
    assert candidate["near_degenerate_stable_rejection_verified"] is True
    assert candidate["cancellation_process_boundary_verified"] is True
    assert candidate["production_operation_lifecycle_verified"] is False
    assert len(candidate["fixtures"]) == 6
    for case in candidate["cancellation_cases"]:
        assert case["partial_glb_emitted"] is False
        assert case["isolated_snapshot_version_cache_unchanged"] is True
        assert case["process_reaped"] is True
package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
assert "manifold-3d" not in package.get("dependencies", {})
assert "manifold-3d" not in package.get("devDependencies", {})
pyproject = (ROOT / "apps/agent/pyproject.toml").read_text(encoding="utf-8")
assert '"manifold3d==3.5.2"' in pyproject and '"numpy==2.4.6"' in pyproject
print("G824A CSG evidence passed: historical provenance/readback and isolated cancellation remain valid for G825")
