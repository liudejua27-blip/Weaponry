#!/usr/bin/env python3
"""Validate the closed Dragonfang reference-view to fixed-camera mapping."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


AXES = {
    "FRONT": "+Z broadside",
    "BACK": "-Z broadside",
    "TOP": "+Y edge-profile",
    "BOTTOM": "-Y edge-profile",
    "LEFT": "-X pommel-end",
    "RIGHT": "+X tip-end",
    "REAR_THREE_QUARTER": "orbit rear-three-quarter",
}


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def fail(message: str) -> None:
    raise ValueError(message)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mapping", type=Path, required=True)
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    args = parser.parse_args()
    try:
        mapping = json.loads(args.mapping.read_text())
        program = json.loads(args.program.read_text())
        manifest_bytes = args.manifest.read_bytes()
        manifest = json.loads(manifest_bytes)
        if mapping.get("schema_version") != "WeaponryThreeJsReferenceViewMapping@1":
            fail("mapping schema differs")
        if mapping.get("program_sha256") != program.get("canonical_sha256"):
            fail("program semantic hash differs")
        if mapping.get("reference_sha256") != hashlib.sha256(args.reference.read_bytes()).hexdigest():
            fail("reference hash differs")
        if mapping.get("aov_manifest_sha256") != hashlib.sha256(manifest_bytes).hexdigest():
            fail("AOV manifest hash differs")
        if mapping.get("preview_worker_cohort_sha256") != manifest.get("preview_worker_cohort_sha256"):
            fail("Worker cohort differs")
        entries = mapping.get("mappings")
        if not isinstance(entries, list):
            fail("mappings must be an array")
        by_kind = {entry.get("reference_view_kind"): entry for entry in entries if isinstance(entry, dict)}
        required = {"front", "back", "top", "bottom", "left", "right", "guard-bottom", "pommel", "fps-hold"}
        if set(by_kind) != required:
            fail("reference view set is not closed")
        available = {item["path"].split("-", 1)[0].upper() for item in manifest.get("files", [])}
        for kind in ("front", "back", "top", "bottom", "left", "right"):
            entry = by_kind[kind]
            render_id = entry.get("render_view_id")
            if render_id not in AXES or render_id not in available:
                fail(f"{kind} render view is unavailable")
            if entry.get("camera_axis") != AXES[render_id]:
                fail(f"{kind} camera axis differs")
            if entry.get("comparison_eligible") is not True:
                fail(f"{kind} must be comparison eligible")
        if by_kind["left"].get("render_view_id") != "TOP" or by_kind["left"].get("independent_direction") is not False:
            fail("LEFT must be an explicit TOP alias, not a -X camera")
        if by_kind["right"].get("render_view_id") != "BOTTOM" or by_kind["right"].get("independent_direction") is not False:
            fail("RIGHT must be an explicit BOTTOM alias, not a +X camera")
        for kind in ("guard-bottom", "pommel", "fps-hold"):
            entry = by_kind[kind]
            if entry.get("render_view_id") is not None or entry.get("comparison_eligible") is not False:
                fail(f"{kind} cannot enter global fixed-view comparison")
        claimed = mapping.get("canonical_sha256")
        preimage = dict(mapping)
        preimage["canonical_sha256"] = ""
        expected = hashlib.sha256(canonical_bytes(preimage)).hexdigest()
        if claimed != expected:
            fail(f"canonical hash differs: expected {expected}")
        print(json.dumps({"status": "PASS", "canonical_sha256": claimed, "mapped": len(entries)}))
        return 0
    except (OSError, json.JSONDecodeError, KeyError, ValueError) as error:
        print(f"WEAPONRY_THREEJS_REFERENCE_VIEW_MAPPING_INVALID: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
