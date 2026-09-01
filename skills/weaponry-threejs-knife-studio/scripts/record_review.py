#!/usr/bin/env python3
"""Record one hash-bound Weaponry review without mutating KnifeSceneProgram."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Any


class ReviewError(ValueError):
    pass


def load_object(path: Path, label: str) -> tuple[dict[str, Any], str]:
    payload = path.read_bytes()
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as error:
        raise ReviewError(f"{label} is not valid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ReviewError(f"{label} must be an object")
    return value, hashlib.sha256(payload).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReviewError(message)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def write_new(path: Path, value: dict[str, Any]) -> None:
    if path.exists():
        raise ReviewError(f"refusing to overwrite existing review ledger: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
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
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--pass-gate", type=Path, required=True)
    parser.add_argument("--comparison-evidence", type=Path, required=True)
    parser.add_argument("--multi-angle-evidence", type=Path, required=True)
    parser.add_argument("--comparison-image", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    try:
        program, program_file_sha = load_object(args.program, "program")
        gate, gate_file_sha = load_object(args.pass_gate, "pass gate")
        comparison, comparison_file_sha = load_object(args.comparison_evidence, "comparison evidence")
        multi_angle, multi_angle_file_sha = load_object(args.multi_angle_evidence, "multi-angle evidence")
        image_sha = hashlib.sha256(args.comparison_image.read_bytes()).hexdigest()
        program_sha = program.get("canonical_sha256")
        require(program.get("schema_version") == "KnifeSceneProgram@1", "program schema differs")
        require(gate.get("program_sha256") == program_sha, "pass gate program binding differs")
        require(gate.get("pass_gate_status") == "FAIL", "only a truthful failed gate may enter this review")
        require(gate.get("decision") == "refine-spec", "pass gate must require refine-spec")
        require(gate.get("comparison_evidence_sha256") == comparison_file_sha, "comparison hash differs")
        require(gate.get("multi_angle_evidence_sha256") == multi_angle_file_sha, "multi-angle hash differs")
        require(comparison.get("source", {}).get("program_sha256") == program_sha, "comparison program differs")
        require(multi_angle.get("source", {}).get("program_sha256") == program_sha, "multi-angle program differs")

        identity_preimage = {
            "program_sha256": program_sha,
            "pass_id": "blockout",
            "action": "refine-spec",
            "pass_gate_sha256": gate_file_sha,
            "comparison_image_sha256": image_sha,
        }
        review_id = "weaponry-review-" + hashlib.sha256(canonical_bytes(identity_preimage)).hexdigest()[:40]
        result: dict[str, Any] = {
            "schema_version": "WeaponryThreeJsKnifeReviewLedger@1",
            "asset_id": program.get("asset_id"),
            "program_sha256": program_sha,
            "program_file_sha256": program_file_sha,
            "reviews": [
                {
                    "review_id": review_id,
                    "sequence": 1,
                    "pass_id": "blockout",
                    "action": "refine-spec",
                    "correction_scope": "reference-view-camera-contract",
                    "estimated_fidelity": 0.32,
                    "score_kind": "agent-estimate-not-calibrated",
                    "layer_scores": {
                        "silhouette": 0.45,
                        "proportions": 0.35,
                        "structure": 0.30,
                        "material": 0.25,
                        "lighting": 0.30,
                    },
                    "strengths": [
                        "The fixed Worker produced a real non-degenerate volume in broadside and edge-profile views.",
                        "The asset remains recognizable as a curved fantasy kukri blockout.",
                    ],
                    "failures": [
                        "The blade is too leaf-shaped, wide and blunt relative to the supplied reference.",
                        "The guard, grip and relief are primitive and remain outside commercial quality.",
                        "Reference LEFT/RIGHT labels and fixed camera LEFT/RIGHT axes are not semantically equivalent.",
                    ],
                    "single_next_action": "Freeze an explicit reference-view to render-camera mapping before any new likeness score or geometry correction.",
                    "evidence": {
                        "pass_gate_sha256": gate_file_sha,
                        "comparison_evidence_sha256": comparison_file_sha,
                        "multi_angle_evidence_sha256": multi_angle_file_sha,
                        "comparison_image_path": str(args.comparison_image),
                        "comparison_image_sha256": image_sha,
                    },
                    "visual_status": "NOT_APPROVED",
                    "human_status": "NOT_RUN",
                    "engine_status": "NOT_RUN",
                    "commercial_status": "NOT_RUN",
                    "parent_retained": True,
                    "geometry_modified": False,
                    "candidate_created": False,
                    "version_created": False,
                    "export_performed": False,
                }
            ],
            "canonical_sha256": "",
        }
        result["canonical_sha256"] = hashlib.sha256(canonical_bytes(result)).hexdigest()
        write_new(args.out, result)
        print(json.dumps({"review_id": review_id, "canonical_sha256": result["canonical_sha256"]}))
        return 0
    except (OSError, ReviewError) as error:
        print(f"WEAPONRY_THREEJS_REVIEW_INVALID: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
