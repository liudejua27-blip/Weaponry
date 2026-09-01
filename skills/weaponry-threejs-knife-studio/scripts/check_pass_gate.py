#!/usr/bin/env python3
"""Close the Weaponry Three.js blockout pass without faking ObjectSculptSpec.

The upstream img2threejs pass checker owns ``ObjectSculptSpec.sculptPipeline``.
Weaponry deliberately owns a different canonical document,
``KnifeSceneProgram@1``.  This adapter consumes only checked-in, hash-bound
Weaponry evidence and emits a conservative pass decision.  It never modifies
the program, creates a candidate, or upgrades visual/human/commercial status.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Any


RESULT_SCHEMA = "WeaponryThreeJsKnifePassGateResult@1"
PASS_ID = "blockout"
SHA256_LENGTH = 64


class GateError(ValueError):
    pass


def read_json(path: Path, label: str) -> tuple[dict[str, Any], str]:
    payload = path.read_bytes()
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as error:
        raise GateError(f"{label} is not JSON: {error}") from error
    if not isinstance(value, dict):
        raise GateError(f"{label} must be an object")
    return value, hashlib.sha256(payload).hexdigest()


def require(value: bool, message: str) -> None:
    if not value:
        raise GateError(message)


def is_sha256(value: object) -> bool:
    return (
        isinstance(value, str)
        and len(value) == SHA256_LENGTH
        and all(character in "0123456789abcdef" for character in value)
    )


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def write_atomic(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except Exception:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def evaluate(
    program: dict[str, Any],
    program_file_sha256: str,
    comparison: dict[str, Any],
    comparison_file_sha256: str,
    multi_angle: dict[str, Any],
    multi_angle_file_sha256: str,
) -> dict[str, Any]:
    require(program.get("schema_version") == "KnifeSceneProgram@1", "program schema differs")
    program_sha256 = program.get("canonical_sha256")
    require(is_sha256(program_sha256), "program canonical_sha256 is invalid")

    require(
        comparison.get("schema_version") == "WeaponryThreeJsKnifeComparisonLiveEvidence@1",
        "comparison evidence schema differs",
    )
    require(comparison.get("task_id") == "WPN-THREE-COMPARE-006", "comparison task differs")
    require(
        comparison.get("source", {}).get("program_sha256") == program_sha256,
        "comparison is not bound to the exact program",
    )
    require(
        comparison.get("comparison", {}).get("parent_retained") is True,
        "comparison must retain the parent",
    )

    require(
        multi_angle.get("schema_version") == "WeaponryThreeJsKnifeMultiViewReviewEvidence@1",
        "multi-angle evidence schema differs",
    )
    require(multi_angle.get("task_id") == "WPN-THREE-MULTIVIEW-007", "multi-angle task differs")
    require(
        multi_angle.get("source", {}).get("program_sha256") == program_sha256,
        "multi-angle evidence is not bound to the exact program",
    )
    require(
        multi_angle.get("deterministic_multi_angle_gate", {}).get("degenerate") is False,
        "multi-angle volume gate is degenerate or missing",
    )
    require(
        multi_angle.get("truth_boundary", {}).get("geometry_modified") is False,
        "review evidence must not modify geometry",
    )

    blockers: list[dict[str, str]] = []
    if comparison.get("comparison", {}).get("comparison_status") != "APPROVED":
        blockers.append(
            {
                "code": "FRONT_REFERENCE_NOT_APPROVED",
                "detail": "The durable FRONT blade-only comparison is measured but not approved.",
            }
        )
    if multi_angle.get("agent_visual_review", {}).get("decision") == "refine-spec":
        blockers.append(
            {
                "code": "REFERENCE_VIEW_MAPPING_REQUIRED",
                "detail": "Reference view labels are not acceptance-equivalent to fixed camera axes.",
            }
        )
    if comparison.get("truth_boundary", {}).get("visual_status") != "PASS":
        blockers.append(
            {
                "code": "CALIBRATED_VISUAL_GATE_NOT_RUN",
                "detail": "Deterministic metrics do not replace calibrated visual or human review.",
            }
        )

    result: dict[str, Any] = {
        "schema_version": RESULT_SCHEMA,
        "route": "weaponry-threejs-knife-studio@0.1.0",
        "pass_id": PASS_ID,
        "program_sha256": program_sha256,
        "program_file_sha256": program_file_sha256,
        "comparison_evidence_sha256": comparison_file_sha256,
        "multi_angle_evidence_sha256": multi_angle_file_sha256,
        "upstream_img2threejs_checker": {
            "status": "INCOMPATIBLE_CANONICAL_SPEC_SHAPE",
            "reason": "ObjectSculptSpec.sculptPipeline is not part of KnifeSceneProgram@1",
        },
        "geometry_hard_gate": "PASS_NON_DEGENERATE_VOLUME",
        "reference_likeness_gate": "FAIL_MEASURED_NOT_APPROVED",
        "reference_view_mapping_gate": "FAIL_MAPPING_REQUIRED",
        "decision": "refine-spec",
        "pass_gate_status": "FAIL",
        "blockers": blockers,
        "required_successor": {
            "schema_version": "WeaponryThreeJsReferenceViewMapping@1",
            "must_separate": ["reference_view_kind", "render_view_id", "camera_axis", "handedness_transform"],
            "must_preserve": ["program_sha256", "preview_worker_cohort_sha256", "reference hashes"],
        },
        "parent_retained": True,
        "candidate_created": False,
        "version_created": False,
        "export_performed": False,
        "visual_status": "NOT_RUN_CALIBRATED_GATE",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "canonical_sha256": "",
    }
    result["canonical_sha256"] = hashlib.sha256(canonical_bytes(result)).hexdigest()
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--comparison", type=Path, required=True)
    parser.add_argument("--multi-angle", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    arguments = parser.parse_args()

    try:
        program, program_file_sha256 = read_json(arguments.program, "program")
        comparison, comparison_file_sha256 = read_json(arguments.comparison, "comparison")
        multi_angle, multi_angle_file_sha256 = read_json(arguments.multi_angle, "multi-angle evidence")
        result = evaluate(
            program,
            program_file_sha256,
            comparison,
            comparison_file_sha256,
            multi_angle,
            multi_angle_file_sha256,
        )
        write_atomic(arguments.out, result)
        print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))
        return 1 if result["pass_gate_status"] == "FAIL" else 0
    except (OSError, GateError) as error:
        print(f"WEAPONRY_THREEJS_PASS_GATE_INVALID: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
