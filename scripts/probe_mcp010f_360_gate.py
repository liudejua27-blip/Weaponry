#!/usr/bin/env python3
"""Check 360° coverage gate for MCP010F reference inventory.

For one-image front/partial-back/left/right references this should remain
`BLOCKED_REFERENCE_COVERAGE` until sufficient orthographic coverage exists.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


REQUIRED_VIEWS = ("front", "back", "left", "right", "rear_three_quarter")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--inventory",
        type=Path,
        required=True,
        help="Reference-detail inventory JSON used by MCP010F correction planning.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Output evidence JSON path.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    inventory_path = args.inventory.expanduser()
    if not inventory_path.is_file():
        raise SystemExit("reference inventory is missing")
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    if not isinstance(inventory, dict):
        raise SystemExit("reference inventory must be a JSON object")

    correction = inventory.get("correction_state") if isinstance(inventory.get("correction_state"), dict) else {}
    quality_contract = inventory.get("quality_contract") if isinstance(inventory.get("quality_contract"), dict) else {}
    coverage = (
        inventory.get("reference", {})
        .get("coverage", {})
        if isinstance(inventory.get("reference"), dict)
        else {}
    )

    coverage_missing = [view for view in REQUIRED_VIEWS if not _view_visible(coverage.get(view))]
    blocked = bool(coverage_missing) or len(coverage) < len(REQUIRED_VIEWS)

    observed = {
        "front": coverage.get("front"),
        "back": coverage.get("back"),
        "left": coverage.get("left"),
        "right": coverage.get("right"),
        "rear_three_quarter": coverage.get("rear_three_quarter") or coverage.get("feet"),
        "feet": coverage.get("feet"),
    }

    gate_value = "BLOCKED_REFERENCE_COVERAGE" if blocked else "PASS"
    status = "PASS_BLOCKED" if blocked else "PASS"

    receipt = {
        "schema_version": "ForgeCADMCP010FHQ360Probe@1",
        "task_id": "FGC-MCP010F",
        "status": status,
        "full_360_reference": gate_value,
        "required_views": list(REQUIRED_VIEWS),
        "observed_coverage": observed,
        "missing_coverage_views": coverage_missing,
        "quality_contract_hq_360": quality_contract.get("hq_360_status"),
        "correction_state_hq_360": correction.get("full_360"),
        "persistent_user_data_touched": bool(inventory.get("persistent_user_data_touched", False)),
        "limitations": [
            "One-image inputs still block full 360 gate until all required orthographic views are available.",
            "This probe does not alter quality score, only confirms coverage status.",
        ],
    }

    output = args.output.expanduser().resolve()
    if not str(output).startswith(str((Path(__file__).resolve().parents[1] / "docs/evidence").resolve())):
        raise SystemExit("--output must be under docs/evidence")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0 if blocked else 1


def _view_visible(status: object) -> bool:
    if not isinstance(status, str):
        return False
    lowered = status.lower()
    return "unknown" not in lowered and "blocked" not in lowered and status not in {"", "unavailable"}


if __name__ == "__main__":
    raise SystemExit(main())
