#!/usr/bin/env python3
"""Exercise real Codex CLI authoring of a hash-bound GeometryProgram@2.

The probe intentionally separates reference admission from V2 authoring.  It
uses the user-authorized image only for the first Codex turn, then proves that
Codex can obtain the live catalog digest, ask ForgeCAD for a canonical draft
hash, compile that exact V2 program, and read the candidate-bound artifact.
It is a structural host gate, not a visual-quality or image-similarity claim.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

from probe_mcp007_codex_cli import (
    config_override,
    event_items,
    mcp_calls,
    run_turn,
    structured_result,
    unrelated_side_effects,
)


SETUP_SEQUENCE = ("project_create", "reference_import", "reference_get")
AUTHORING_SEQUENCE = (
    "capabilities_get",
    "operator_catalog_get",
    "geometry_program_hash",
    "geometry_prepare",
    "job_get",
    "candidate_get",
    "artifact_readback_get",
    "quality_get",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--reference", required=True, help="user-authorized PNG/JPEG path")
    parser.add_argument("--runtime-command", required=True)
    parser.add_argument("--mcp-command", required=True)
    parser.add_argument("--codex-command", default="codex")
    parser.add_argument(
        "--evidence",
        type=Path,
        help="Optional JSON receipt path beneath docs/evidence.",
    )
    parser.add_argument("--timeout", type=float, default=360.0)
    parser.add_argument("--debug", action="store_true", help="print only redacted local turn counters to stderr")
    return parser.parse_args()


def write_receipt(path: Path | None, receipt: dict[str, Any]) -> None:
    if path is None:
        return
    root = Path(__file__).resolve().parents[1]
    resolved = path if path.is_absolute() else root / path
    evidence_root = (root / "docs" / "evidence").resolve()
    try:
        resolved.resolve().relative_to(evidence_root)
    except ValueError as error:
        raise SystemExit("Codex CLI probe evidence must stay under docs/evidence") from error
    if resolved.suffix != ".json":
        raise SystemExit("Codex CLI probe evidence must be JSON")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build_cohort(command: str, expected_component: str) -> str:
    """Read a public, non-secret cohort identity before starting Codex."""
    completed = subprocess.run(
        [command, "--build-identity"],
        text=True,
        capture_output=True,
        timeout=20,
        check=True,
    )
    identity = json.loads(completed.stdout)
    cohort = identity.get("build_cohort_sha256") if isinstance(identity, dict) else None
    if (
        not isinstance(cohort, str)
        or len(cohort) != 64
        or identity.get("component") != expected_component
    ):
        raise ValueError(f"{expected_component} did not expose a valid build identity")
    return cohort


def blocked(reason: str, source_sha256: str, size: int) -> dict[str, Any]:
    return {
        "schema_version": "ForgeCADMCP010BCodexCliProbe@1",
        "task_id": "FGC-MCP010B",
        "status": "BLOCKED",
        "reason": reason,
        "scope": "real Codex CLI V2 structural host gate",
        "source_sha256": source_sha256,
        "source_size_bytes": size,
        "reference_path_recorded": False,
        "image_bytes_recorded": False,
        "visual_similarity": "NOT_RUN",
        "human_review": "NOT_RUN",
        "assetpack_or_pbr_v2": "NOT_RUN",
    }


def v2_draft_template(project_id: str) -> dict[str, Any]:
    """A small visual robot blockout with no caller-computed canonical hash."""
    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "e" * 64,
        "operator_catalog_sha256": "<copy-exact-live-catalog-hash>",
        "units": {
            "length": "meter",
            "angle": "radian",
            "coordinate_system": "right-handed-y-up",
        },
        "budgets": {
            "max_nodes": 13,
            "max_triangles": 20000,
            "max_glb_bytes": 1048576,
            "max_worker_memory_bytes": 536870912,
            "max_runtime_ms": 10000,
        },
        "nodes": [
            {
                "node_id": "head-shell",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "ellipsoid",
                    "radii_m": [0.42, 0.5, 0.4],
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [0.0, 2.95, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "visor",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.62, 0.22, 0.12],
                    "position_m": [0.0, 2.94, -0.38],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "neck",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "cylinder",
                    "radius_m": 0.22,
                    "height_m": 0.34,
                    "radial_segments": 16,
                    "position_m": [0.0, 2.5, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "chest-shell",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [1.25, 0.95, 0.5],
                    "position_m": [0.0, 2.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "chest-core",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "cylinder",
                    "radius_m": 0.28,
                    "height_m": 0.54,
                    "radial_segments": 16,
                    "position_m": [0.0, 1.96, -0.24],
                    "rotation_rad": [1.5707963, 0.0, 0.0],
                },
            },
            {
                "node_id": "chest-panel",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.42, 0.18, 0.08],
                    "position_m": [0.0, 2.08, -0.3],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "shoulder-left",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "sphere",
                    "radius_m": 0.3,
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [-0.84, 2.18, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "shoulder-right",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "sphere",
                    "radius_m": 0.3,
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [0.84, 2.18, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "arm-left",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.3, 1.1, 0.32],
                    "position_m": [-0.98, 1.48, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "arm-right",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.3, 1.1, 0.32],
                    "position_m": [0.98, 1.48, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "pelvis",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.92, 0.48, 0.46],
                    "position_m": [0.0, 1.18, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "thigh-left",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.34, 0.96, 0.38],
                    "position_m": [-0.34, 0.45, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
            {
                "node_id": "thigh-right",
                "operator_id": "forgecad.geometry.primitive@2",
                "inputs": [],
                "parameters": {
                    "shape": "box",
                    "size_m": [0.34, 0.96, 0.38],
                    "position_m": [0.34, 0.45, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
            },
        ],
        "part_outputs": [
            {"part_id": "head-shell", "input_node_ids": ["head-shell"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "visor", "input_node_ids": ["visor"], "material_zone_id": "zone-black-mechanical", "solid": True},
            {"part_id": "neck", "input_node_ids": ["neck"], "material_zone_id": "zone-black-mechanical", "solid": True},
            {"part_id": "chest-shell", "input_node_ids": ["chest-shell", "chest-panel"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "chest-core", "input_node_ids": ["chest-core"], "material_zone_id": "zone-black-mechanical", "solid": True},
            {"part_id": "shoulder-left", "input_node_ids": ["shoulder-left"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "shoulder-right", "input_node_ids": ["shoulder-right"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "arm-left", "input_node_ids": ["arm-left"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "arm-right", "input_node_ids": ["arm-right"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "pelvis", "input_node_ids": ["pelvis"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "thigh-left", "input_node_ids": ["thigh-left"], "material_zone_id": "zone-white-shell", "solid": True},
            {"part_id": "thigh-right", "input_node_ids": ["thigh-right"], "material_zone_id": "zone-white-shell", "solid": True},
        ],
    }


def source_bindings_for_part(readback: Any, part_id: str) -> list[str]:
    if not isinstance(readback, dict):
        return []
    bindings = readback.get("part_bindings")
    if not isinstance(bindings, list):
        return []
    return [
        binding.get("source_node_id")
        for binding in bindings
        if isinstance(binding, dict)
        and binding.get("part_id") == part_id
        and isinstance(binding.get("source_node_id"), str)
    ]


def setup_prompt(reference_path: str) -> str:
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, other MCP servers, or arbitrary code.

This is the reference-admission half of a real V2 structural host gate. Call exactly three tools in order:
1) project_create with name="MCP010B Codex V2 robot structural probe" and policy={{"profile":"mvp"}}.
2) reference_import with the returned project_id, source={{"kind":"codex_local_file","path":{json.dumps(reference_path, ensure_ascii=False)}}}, authorization={{"user_authorized":true,"declaration":"The user supplied and authorized this reference for local ForgeCAD MVP evaluation."}}.
3) reference_get with reference_id copied exactly from reference_import. Verify the returned reference.reference_id matches the imported id and do not request or expose image bytes.

Stop after these three calls. Return only the project_id and reference_id. Do not claim image similarity, high quality, PBR, 360 degree coverage, or human approval.
"""


def authoring_prompt(project_id: str, reference_id: str) -> str:
    draft = json.dumps(v2_draft_template(project_id), ensure_ascii=False, separators=(",", ":"))
    return f"""Use only the ForgeCAD MCP server. Do not use shell, filesystem, browser, images, other MCP servers, arbitrary code, or any local hash implementation.

A valid project and its authorized reference already exist. Do not analyse the image or discuss the design in this turn. This is a machine-checked authoring run: prose without the eight MCP calls below is a failed run. Start with tool call 1 now, complete each returned-ID/hash substitution exactly, then stop.

Execute exactly these eight tools in order:
1) capabilities_get. Save its exact operator_catalog_sha256.
2) operator_catalog_get. Verify its canonical_sha256 exactly matches step 1 and it lists active forgecad.geometry.primitive@2. Save that same canonical_sha256.
3) geometry_program_hash. Its arguments must be {{"schema_version":"GeometryProgramHashRequest@1","geometry_program_draft":<the draft below>}}. Replace only the draft's string "<copy-exact-live-catalog-hash>" with the exact catalog hash from steps 1–2. Do not add canonical_sha256 before this call.
4) geometry_prepare with project_id={json.dumps(project_id)} and request={{"typed":"geometry","reference_id":{json.dumps(reference_id)},"geometry_program":<the same draft plus canonical_sha256 copied exactly from the geometry_program_hash result>}}. The reference_id binding is required; do not omit it.
5) job_get with job_id copied exactly from step 4's job result. Verify status is succeeded and progress is 100.
6) candidate_get with candidate_id copied exactly from step 4's candidate result. Verify the candidate is reviewable, quality_hard_gate_passed is true, and project_id is {json.dumps(project_id)}.
7) artifact_readback_get with artifact_id and candidate_id copied exactly from step 4's artifact result.
8) quality_get with candidate_id copied from step 4 and reference_id={json.dumps(reference_id)}. Verify the returned reference_compare.reference_id is exactly the imported reference_id. This is a limited read-only quality check, not visual acceptance.

Draft template (all fields must remain exactly present except the live catalog placeholder, then the returned canonical hash added only in step 3). `part_outputs[].input_node_ids` are ordered, non-empty semantic Part sinks; do not replace them with a singular source field or reorder the chest-shell inputs:
{draft}

Do not call candidate_confirm, appearance_prepare, export, or any extra ForgeCAD tool. Return only the hash receipt, artifact id, candidate id, job/candidate readback, triangle count, validator status and reference-binding result after all eight calls have completed. This proves a typed structural path only; do not claim visual similarity, PBR texture quality, human approval, full 360 degree coverage, or a finished high-quality model.
"""


def main() -> int:
    options = parse_args()
    source = Path(options.reference)
    if not source.is_file() or source.is_symlink():
        print(json.dumps(blocked("reference is not a regular file", "", 0), separators=(",", ":")))
        return 3
    source_bytes = source.read_bytes()
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    if not options.execute:
        print(
            json.dumps(
                {**blocked("Pass --execute to run the isolated local Runtime and Codex CLI.", source_sha256, len(source_bytes)), "status": "NOT_RUN"},
                separators=(",", ":"),
            )
        )
        return 2
    if options.timeout <= 0 or not Path(options.runtime_command).is_file() or not Path(options.mcp_command).is_file():
        print(json.dumps(blocked("source-built ForgeCAD binaries were unavailable", source_sha256, len(source_bytes)), separators=(",", ":")))
        return 3
    worker_command = str(Path(options.runtime_command).with_name("forgecad-geometry-worker"))
    if not Path(worker_command).is_file():
        print(json.dumps(blocked("fixed sibling geometry Worker was unavailable", source_sha256, len(source_bytes)), separators=(",", ":")))
        return 3
    try:
        mcp_cohort = build_cohort(options.mcp_command, "forgecad-mcp")
        runtime_cohort = build_cohort(options.runtime_command, "forgecad-runtime")
        worker_cohort = build_cohort(worker_command, "forgecad-geometry-worker")
    except (OSError, subprocess.SubprocessError, ValueError, json.JSONDecodeError):
        print(json.dumps(blocked("ForgeCAD component build identity was unavailable", source_sha256, len(source_bytes)), separators=(",", ":")))
        return 3
    if len({mcp_cohort, runtime_cohort, worker_cohort}) != 1:
        print(json.dumps(blocked("ForgeCAD MCP, Runtime and Worker cohorts did not match", source_sha256, len(source_bytes)), separators=(",", ":")))
        return 3

    environment = os.environ.copy()
    for key in (
        "CODEX_MCP_PROTOCOL_VERSION",
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    environment["FORGECAD_ATTACHMENT_ROOTS"] = str(source.parent)

    with tempfile.TemporaryDirectory(dir="/tmp", prefix="fc10b-codex-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        runtime = subprocess.Popen(
            [
                options.runtime_command,
                "serve",
                "--database",
                str(root / "runtime.sqlite"),
                "--cas-root",
                str(root / "cas"),
                "--endpoint-dir",
                str(root / "ipc"),
                "--ready-file",
                str(ready),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
        )
        try:
            deadline = time.monotonic() + 30
            while not ready.exists() and time.monotonic() < deadline:
                if runtime.poll() is not None:
                    break
                time.sleep(0.05)
            if not ready.exists():
                print(json.dumps(blocked("Runtime did not publish a ready handoff", source_sha256, len(source_bytes)), separators=(",", ":")))
                return 3
            handoff = json.loads(ready.read_text(encoding="utf-8"))
            socket_path = handoff.get("socket_path")
            token = handoff.get("token")
            if not isinstance(socket_path, str) or not socket_path or not isinstance(token, str) or not token:
                print(json.dumps(blocked("Runtime handoff omitted authenticated endpoint data", source_sha256, len(source_bytes)), separators=(",", ":")))
                return 3
            environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
            environment["FORGECAD_RUNTIME_TOKEN"] = token

            first = run_turn(options, environment, setup_prompt(str(source)), str(root), str(source))
            first_items = event_items(first.stdout)
            project = structured_result(first_items, "project_create")
            project_id = project.get("project_id") if isinstance(project, dict) else None
            reference_result = structured_result(first_items, "reference_import")
            reference = reference_result.get("reference") if isinstance(reference_result, dict) else None
            reference_id = reference.get("reference_id") if isinstance(reference, dict) else None
            reference_readback_result = structured_result(first_items, "reference_get")
            reference_readback = (
                reference_readback_result.get("reference")
                if isinstance(reference_readback_result, dict)
                else None
            )
            first_calls = mcp_calls(first_items)
            first_tools = [call.get("tool") for call in first_calls if call.get("server") == "forgecad"]
            first_ok = (
                first.returncode == 0
                and first_tools == list(SETUP_SEQUENCE)
                and all(call.get("status") == "completed" for call in first_calls)
                and isinstance(project_id, str)
                and isinstance(reference_id, str)
                and isinstance(reference_readback, dict)
                and reference_readback.get("reference_id") == reference_id
                and not unrelated_side_effects(first_items)
            )
            if not first_ok:
                receipt = {
                    **blocked("Codex did not complete the exact reference-admission sequence", source_sha256, len(source_bytes)),
                    "codex_exit_code": first.returncode,
                    "expected_sequence": list(SETUP_SEQUENCE + AUTHORING_SEQUENCE),
                    "mcp_tool_calls": first_calls,
                }
                print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
                return 3

            second = run_turn(options, environment, authoring_prompt(project_id, reference_id), str(root))
            second_items = event_items(second.stdout)
            second_calls = mcp_calls(second_items)
            second_tools = [call.get("tool") for call in second_calls if call.get("server") == "forgecad"]
            capabilities = structured_result(second_items, "capabilities_get")
            catalog = structured_result(second_items, "operator_catalog_get")
            hash_result = structured_result(second_items, "geometry_program_hash")
            geometry = structured_result(second_items, "geometry_prepare")
            artifact = geometry.get("artifact") if isinstance(geometry, dict) else None
            prepared_job = geometry.get("job") if isinstance(geometry, dict) else None
            prepared_candidate = geometry.get("candidate") if isinstance(geometry, dict) else None
            job_id = prepared_job.get("job_id") if isinstance(prepared_job, dict) else None
            prepared_candidate_id = (
                prepared_candidate.get("candidate_id") if isinstance(prepared_candidate, dict) else None
            )
            job_readback = structured_result(second_items, "job_get")
            candidate_readback = structured_result(second_items, "candidate_get")
            readback = structured_result(second_items, "artifact_readback_get")
            quality = structured_result(second_items, "quality_get")
            artifact_id = artifact.get("artifact_id") if isinstance(artifact, dict) else None
            candidate_id = artifact.get("candidate_id") if isinstance(artifact, dict) else None
            capability_catalog_hash = (
                capabilities.get("operator_catalog_sha256") if isinstance(capabilities, dict) else None
            )
            catalog_hash = catalog.get("canonical_sha256") if isinstance(catalog, dict) else None
            hash_receipt_hash = (
                hash_result.get("canonical_sha256") if isinstance(hash_result, dict) else None
            )
            hash_receipt_catalog_hash = (
                hash_result.get("operator_catalog_sha256") if isinstance(hash_result, dict) else None
            )
            catalog_operators = catalog.get("operators") if isinstance(catalog, dict) else None
            artifact_chest_sources = source_bindings_for_part(artifact, "chest-shell")
            readback_chest_sources = source_bindings_for_part(readback, "chest-shell")
            artifact_integrity = artifact.get("integrity") if isinstance(artifact, dict) else None
            second_ok = (
                second.returncode == 0
                and second_tools == list(AUTHORING_SEQUENCE)
                and all(call.get("status") == "completed" for call in second_calls)
                and not unrelated_side_effects(second_items)
                and isinstance(capabilities, dict)
                and isinstance(catalog, dict)
                and catalog.get("schema_version") == "OperatorCatalog@1"
                and isinstance(capability_catalog_hash, str)
                and isinstance(catalog_hash, str)
                and capability_catalog_hash == catalog_hash
                and isinstance(catalog_operators, list)
                and any(
                    isinstance(operator, dict)
                    and operator.get("operator_id") == "forgecad.geometry.primitive@2"
                    and operator.get("status") == "active"
                    for operator in catalog_operators
                )
                and isinstance(hash_result, dict)
                and hash_result.get("schema_version") == "GeometryProgramHashResult@1"
                and isinstance(hash_receipt_hash, str)
                and isinstance(hash_receipt_catalog_hash, str)
                and hash_receipt_catalog_hash == catalog_hash
                and isinstance(artifact, dict)
                and artifact.get("schema_version") == "ArtifactReadback@2"
                and artifact.get("hard_gate_passed") is True
                and artifact.get("validator_status") == "passed"
                and artifact.get("program_sha256") == hash_receipt_hash
                and artifact.get("operator_catalog_sha256") == catalog_hash
                and artifact_chest_sources == ["chest-shell", "chest-panel"]
                and isinstance(artifact_integrity, dict)
                and artifact_integrity.get("source_coverage") == 1
                and isinstance(artifact_id, str)
                and isinstance(candidate_id, str)
                and isinstance(prepared_job, dict)
                and isinstance(job_id, str)
                and isinstance(job_readback, dict)
                and job_readback.get("job_id") == job_id
                and job_readback.get("status") == "succeeded"
                and job_readback.get("progress") == 100
                and isinstance(prepared_candidate, dict)
                and prepared_candidate_id == candidate_id
                and isinstance(candidate_readback, dict)
                and candidate_readback.get("candidate_id") == candidate_id
                and candidate_readback.get("project_id") == project_id
                and candidate_readback.get("state") == "reviewable"
                and candidate_readback.get("quality_hard_gate_passed") is True
                and isinstance(readback, dict)
                and readback.get("artifact_id") == artifact_id
                and readback.get("candidate_id") == candidate_id
                and readback.get("program_sha256") == hash_receipt_hash
                and readback.get("operator_catalog_sha256") == catalog_hash
                and readback_chest_sources == ["chest-shell", "chest-panel"]
                and isinstance(quality, dict)
                and quality.get("candidate_id") == candidate_id
                and isinstance(quality.get("reference_compare"), dict)
                and quality["reference_compare"].get("reference_id") == reference_id
            )
            all_calls = first_calls + second_calls
            if options.debug:
                import sys

                print(
                    json.dumps(
                        {
                            "first_codex_exit_code": first.returncode,
                            "first_event_count": len(first_items),
                            "second_codex_exit_code": second.returncode,
                            "second_event_count": len(second_items),
                            "raw_turn_output_redacted": True,
                        },
                        separators=(",", ":"),
                    ),
                    file=sys.stderr,
                )
            if not second_ok:
                receipt = {
                    **blocked("Codex did not complete the exact V2 canonical-hash authoring sequence", source_sha256, len(source_bytes)),
                    "codex_exit_code": second.returncode,
                    "expected_sequence": list(SETUP_SEQUENCE + AUTHORING_SEQUENCE),
                    "mcp_tool_calls": all_calls,
                }
                print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
                return 3

            receipt = {
                "schema_version": "ForgeCADMCP010BCodexCliProbe@1",
                "task_id": "FGC-MCP010B",
                "status": "PASS",
                "scope": "real Codex CLI V2 structural host gate",
                "expected_sequence": list(SETUP_SEQUENCE + AUTHORING_SEQUENCE),
                "mcp_tool_calls": all_calls,
                "mcp_build_cohort_sha256": mcp_cohort,
                "runtime_build_cohort_sha256": runtime_cohort,
                "geometry_worker_build_cohort_sha256": worker_cohort,
                "build_cohort_match": True,
                "source_sha256": source_sha256,
                "source_size_bytes": len(source_bytes),
                "reference_path_recorded": False,
                "image_bytes_recorded": False,
                "hash_receipt": {
                    "canonical_sha256": hash_result.get("canonical_sha256"),
                    "operator_catalog_sha256": hash_result.get("operator_catalog_sha256"),
                },
                "reference_binding": {
                    "reference_id_verified": True,
                    "reference_get_id_verified": True,
                    "quality_reference_compare_id_verified": True,
                    "quality_status": quality.get("status"),
                    "quality_limitation": quality.get("limitation"),
                },
                "artifact": {
                    "artifact_id": artifact_id,
                    "candidate_id": candidate_id,
                    "triangle_count": artifact.get("triangle_count"),
                    "part_count": len(artifact.get("part_ids", [])),
                    "validator_status": artifact.get("validator_status"),
                    "multi_input_part_sink": {
                        "part_id": "chest-shell",
                        "source_node_ids": artifact_chest_sources,
                    },
                },
                "job_candidate_readback": {
                    "job_id": job_id,
                    "job_status": job_readback.get("status"),
                    "job_progress": job_readback.get("progress"),
                    "candidate_state": candidate_readback.get("state"),
                    "candidate_quality_hard_gate_passed": candidate_readback.get(
                        "quality_hard_gate_passed"
                    ),
                },
                "visual_similarity": "NOT_RUN",
                "human_review": "NOT_RUN",
                "assetpack_or_pbr_v2": "NOT_RUN",
                "candidate_confirmed": False,
                "persistent_user_data_touched": False,
            }
            write_receipt(options.evidence, receipt)
            print(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")))
            return 0
        except subprocess.TimeoutExpired:
            print(json.dumps(blocked("Codex CLI timed out before the V2 host gate completed", source_sha256, len(source_bytes)), separators=(",", ":")))
            return 3
        finally:
            if runtime.poll() is None:
                runtime.terminate()
                try:
                    runtime.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    runtime.kill()
                    runtime.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
