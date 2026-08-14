#!/usr/bin/env python3
"""Validate a real Agentic projection producer/consumer receipt.

The durable receipt checker intentionally covers only persisted session,
checkpoint, and RepairIntent records.  This checker covers the separate
read-only projection envelope and its nested projection contracts so that a
Runtime producer cannot drift away from the MCP/Viewer consumer schemas.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from check_agentic_contracts import (
    ContractError,
    MANIFEST,
    SCHEMA_ROOT,
    is_valid,
    load_json,
    load_schema_registry,
    validate,
)


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RECEIPT = ROOT / "docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json"


def fail(message: str) -> None:
    raise SystemExit(f"Agentic projection receipt check failed: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def validate_record(
    record: Any,
    schema_name: str,
    registry: dict[str, dict[str, Any]],
    label: str,
) -> None:
    require(isinstance(record, dict), f"receipt omitted {label}")
    schema = load_json(SCHEMA_ROOT / schema_name)
    try:
        validate(schema, record, schema, registry=registry)
    except ContractError as error:
        fail(f"{label} failed {schema_name}: {error}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    args = parser.parse_args()
    receipt_path = args.receipt if args.receipt.is_absolute() else ROOT / args.receipt
    try:
        receipt: dict[str, Any] = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"receipt could not be loaded: {error}")

    require(receipt.get("status") == "PASS", "receipt is not a passing isolated run")
    require(receipt.get("persistent_user_data_touched") is False, "probe touched persistent user data")
    preflight = receipt.get("preflight")
    require(
        isinstance(preflight, dict)
        and preflight.get("skill_id") == "ponytail-preflight"
        and preflight.get("version") == "0.1.0"
        and preflight.get("status") == "PASS",
        "receipt did not perform the required Ponytail preflight",
    )

    records = receipt.get("projection_records")
    require(isinstance(records, dict), "receipt omitted projection_records")
    registry = load_schema_registry(load_json(MANIFEST))
    scene = records.get("scene_observe")
    stage_plan = records.get("stage_plan")
    validate_record(scene, "agentic-scene-observe-result.schema.json", registry, "scene_observe")
    validate_record(
        stage_plan,
        "design-stage-plan-projection.schema.json",
        registry,
        "stage_plan",
    )

    project_id = receipt.get("project_id")
    candidate_id = receipt.get("candidate_id")
    require(scene["project_id"] == project_id, "scene projection crossed project binding")
    require(scene["candidate_id"] == candidate_id, "scene projection crossed candidate binding")
    require(stage_plan["project_id"] == project_id, "stage plan crossed project binding")
    require(stage_plan["candidate_id"] == candidate_id, "stage plan crossed candidate binding")
    require(scene["read_only"] is True, "scene projection is not read-only")
    require(stage_plan["read_only"] is True, "stage plan projection is not read-only")
    require(stage_plan["durable_checkpoint"] is False, "projection stage plan became durable")
    require(scene["design_stage_plan"]["canonical_sha256"] == stage_plan["canonical_sha256"], "scene and stage plan hashes drifted")

    scene_graph = scene["semantic_scene_graph"]
    understanding = scene["model_understanding_bundle"]
    require(scene_graph["project_id"] == project_id, "scene graph crossed project binding")
    require(scene_graph["candidate_id"] == candidate_id, "scene graph crossed candidate binding")
    require(understanding["project_id"] == project_id, "understanding bundle crossed project binding")
    require(understanding["candidate_id"] == candidate_id, "understanding bundle crossed candidate binding")
    require(scene["quality"]["read_only"] is True, "quality projection is not read-only")
    require(scene["design_session"]["durable"] is False, "projection session is incorrectly durable")
    require(scene["design_session"]["checkpoint"]["durable"] is False, "projection checkpoint became durable")
    require(stage_plan["unlocks"]["confirm"] is False, "projection unlocked confirm without visual evidence")
    require(stage_plan["unlocks"]["export"] is False, "projection unlocked export without visual evidence")

    print("Agentic projection receipt OK: Runtime producer output conforms to nested read-only contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
