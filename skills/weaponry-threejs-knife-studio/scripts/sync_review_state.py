#!/usr/bin/env python3
"""Sync the Weaponry sidecar review ledger into img2threejs local state."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from copy import deepcopy
from pathlib import Path
from typing import Any


class SyncError(ValueError):
    pass


def load(path: Path) -> tuple[dict[str, Any], str]:
    payload = path.read_bytes()
    value = json.loads(payload)
    if not isinstance(value, dict):
        raise SyncError(f"{path} must contain an object")
    return value, hashlib.sha256(payload).hexdigest()


def save(path: Path, value: dict[str, Any]) -> None:
    payload = json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    args = parser.parse_args()
    try:
        state, _ = load(args.state)
        ledger, ledger_file_sha = load(args.ledger)
        if ledger.get("schema_version") != "WeaponryThreeJsKnifeReviewLedger@1":
            raise SyncError("review ledger schema differs")
        reviews = ledger.get("reviews")
        if not isinstance(reviews, list) or not reviews:
            raise SyncError("review ledger must contain reviews")
        if state.get("currentPass") != "blockout":
            raise SyncError("state is not in the blockout pass")
        pass_steps = [entry for entry in state.get("checklist", []) if entry.get("scope") == "pass"]
        if any(entry.get("status") != "done" for entry in pass_steps):
            raise SyncError("all current pass steps must be done before sync")
        actions = [review for review in reviews if review.get("action") in {"refine-spec", "refine-code"}]
        if len(actions) != 1 or actions[0].get("pass_id") != "blockout":
            raise SyncError("this atom requires exactly one blockout refinement review")
        if ledger.get("program_sha256") is None:
            raise SyncError("ledger program binding is missing")

        already = any(
            item.get("weaponry_review_id") == actions[0].get("review_id")
            for item in state.get("passHistory", [])
            if isinstance(item, dict)
        )
        if already:
            raise SyncError("review is already synchronized")
        state.setdefault("passHistory", []).append(
            {
                "passId": "blockout",
                "iteration": "refine",
                "weaponry_review_id": actions[0]["review_id"],
                "weaponry_review_ledger_sha256": ledger_file_sha,
                "checklist": deepcopy(pass_steps),
            }
        )
        for entry in pass_steps:
            entry["status"] = "pending"
            entry["evidence"] = []
            entry["reason"] = ""
        state["iterationAction"] = actions[0]["action"]
        state["reviewCursor"] = len(reviews)
        state["loops"]["perPass"] = {"blockout": 1}
        state["loops"]["total"] = 1
        state.setdefault("artifacts", {})["weaponryReviewLedger"] = str(args.ledger.resolve())
        state["artifacts"]["weaponryReviewLedgerSha256"] = ledger_file_sha
        state["artifacts"]["workflowAdapter"] = "weaponry-sidecar-review-ledger@1"
        state["status"] = "active"
        state["currentStep"] = "build-current-pass"
        state["stopReason"] = ""
        save(args.state, state)
        print(json.dumps({"status": "SYNCED", "action": state["iterationAction"], "loop": 1}))
        return 0
    except (OSError, json.JSONDecodeError, KeyError, SyncError) as error:
        print(f"WEAPONRY_THREEJS_REVIEW_SYNC_INVALID: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
