#!/usr/bin/env python3
"""Validate durable Agentic Runtime records captured by the isolated probe."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from check_agentic_contracts import SCHEMA_ROOT, is_valid, load_json


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RECEIPT = ROOT / "docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json"


def fail(message: str) -> None:
    raise SystemExit(f"Agentic Runtime receipt check failed: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


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
    records = receipt.get("durable_records")
    require(isinstance(records, dict), "receipt omitted durable_records")

    targets = {
        "session": "design-session.schema.json",
        "checkpoint": "design-checkpoint.schema.json",
        "repair_intent": "repair-intent.schema.json",
    }
    for record_name, schema_name in targets.items():
        value = records.get(record_name)
        require(isinstance(value, dict), f"receipt omitted {record_name}")
        schema = load_json(SCHEMA_ROOT / schema_name)
        require(is_valid(schema, value), f"Runtime producer output failed {schema_name}")

    session = records["session"]
    checkpoint = records["checkpoint"]
    intent = records["repair_intent"]
    require(session["session_id"] == receipt.get("session_id"), "session id drifted from probe binding")
    require(checkpoint["checkpoint_id"] == receipt.get("checkpoint_id"), "checkpoint id drifted from probe binding")
    require(session["current_checkpoint_id"] == checkpoint["checkpoint_id"], "session checkpoint pointer is not durable")
    require(intent["candidate_id"] == session["candidate_id"], "repair intent crossed candidate binding")
    require(intent["runtime_write"] is False, "repair intent is not CAS-only")
    print("Agentic Runtime receipt OK: durable session/checkpoint/repair records conform and remain bound")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
