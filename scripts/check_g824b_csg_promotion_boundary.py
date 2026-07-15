#!/usr/bin/env python3
"""Validate committed G824B authoritative promotion-boundary evidence."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
report = json.loads((ROOT / "evaluations/csg-g824b/report.json").read_text(encoding="utf-8"))
assert report["schema_version"] == "ForgeCADCSGPromotionBoundaryEvidence@1"
assert report["decision"]["selected"] is None
assert report["decision"]["status"] == "production_lifecycle_evidence_passed_candidate_not_selected"
assert len(report["decision"]["remaining_blockers"]) == 3
assert report["authoritative_state"]["real_sqlite_migrations_applied"] is True
assert report["authoritative_state"]["real_content_addressed_store_used"] is True
assert report["promotion_transaction"]["injected_failure_rollback_verified"] is True
assert report["promotion_transaction"]["version_head_snapshot_atomic_success_verified"] is True
for candidate in report["candidates"]:
    assert candidate["candidate"] in {"python", "wasm"}
    assert candidate["all_interrupt_windows_zero_promotion"] is True
    assert [case["window"] for case in candidate["cases"]] == [
        "kernel_running", "kernel_running", "candidate_ready_before_promotion",
    ]
    assert [case["error_code"] for case in candidate["cases"]] == [
        "CSG_CANCELLED", "CSG_TIMEOUT", "CSG_CANCELLED",
    ]
    for case in candidate["cases"]:
        assert case["database_unchanged"] is True
        assert case["object_store_unchanged"] is True
        assert case["authoritative_paths_passed_to_child"] is False
        assert case["process_reaped"] is True
print("G824B CSG promotion boundary passed: historical interruption windows still preserve authoritative state")
