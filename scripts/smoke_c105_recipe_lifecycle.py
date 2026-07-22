#!/usr/bin/env python3
"""Aggregate the executable C105 Recipe lifecycle evidence.

This is intentionally a native/service gate, not a browser surrogate.  The
focused Rust test owns the four-domain compatibility HTTP lifecycle and uses a
fake restricted-geometry endpoint for the Rust product-state boundary; no
Provider request is possible.  It exports each exact sealed/confirmed active
Recipe ShapeProgram only to a caller-owned temporary evidence directory.  This
gate immediately recompiles those four programs through the real Python
RestrictedGeometryExecutor using the production profile and verifies the GLB,
readback, material zones and provenance.  The existing C105 smoke remains
responsible for schemas, the reviewed registry and Rust expansion golden.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "script" / "with_rust_toolchain.sh"
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
LIFECYCLE_EVIDENCE_FILE = "c105-recipe-lifecycle.json"
LIFECYCLE_EVIDENCE_SCHEMA = "C105RecipeLifecycleEvidence@1"
LIFECYCLE_DOMAIN_SCHEMA = "C105RecipeLifecycleDomainEvidence@1"
EXPECTED_DOMAINS = {
    "prop": "pack_future_weapon_prop",
    "vehicle": "pack_vehicle_concept",
    "aircraft": "pack_aircraft_concept",
    "arm": "pack_robotic_arm_concept",
}


class GateFailure(AssertionError):
    """One stable failure boundary for the C105 aggregation."""


def run(
    label: str,
    command: list[str],
    *,
    timeout: int = 900,
    environment: Mapping[str, str] | None = None,
) -> int:
    try:
        child_environment = os.environ.copy()
        if environment:
            child_environment.update(environment)
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
            env=child_environment,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise GateFailure(f"C105_LIFECYCLE_{label}_UNAVAILABLE: {exc}") from exc
    output = f"{completed.stdout}\n{completed.stderr}"
    if completed.returncode:
        raise GateFailure(f"C105_LIFECYCLE_{label}_FAILED:\n{output[-5000:]}")
    passed = sum(int(value) for value in re.findall(r"test result: ok\. (\d+) passed", output))
    if label.startswith("RUST_") and passed < 1:
        raise GateFailure(f"C105_LIFECYCLE_{label}_MISSING: focused Rust test ran zero tests")
    return passed


def cargo_filter(test_name: str) -> list[str]:
    return [
        str(RUST),
        "cargo",
        "test",
        "--manifest-path",
        str(MANIFEST),
        "-p",
        "wushen-forge-desktop",
        "--offline",
        test_name,
    ]


def _canonical_sha256(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    ).hexdigest()


def _fail(code: str, detail: str) -> None:
    raise GateFailure(f"{code}: {detail}")


def _load_real_geometry_evidence(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_MISSING", path.name)
    try:
        evidence = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_INVALID", str(exc))
    if not isinstance(evidence, dict) or set(evidence) != {
        "schema_version",
        "provider_calls",
        "domains",
    }:
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_ENVELOPE_INVALID", "unexpected evidence envelope")
    if evidence.get("schema_version") != LIFECYCLE_EVIDENCE_SCHEMA:
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_VERSION_INVALID", str(evidence.get("schema_version")))
    if evidence.get("provider_calls") != 0:
        _fail("C105_LIFECYCLE_PROVIDER_CALL_FORBIDDEN", str(evidence.get("provider_calls")))
    domains = evidence.get("domains")
    if not isinstance(domains, list) or len(domains) != len(EXPECTED_DOMAINS):
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_DOMAIN_COUNT_INVALID", str(type(domains).__name__))
    rows: list[dict[str, Any]] = []
    observed: dict[str, str] = {}
    expected_keys = {
        "schema_version",
        "domain_slug",
        "domain_pack_id",
        "project_id",
        "asset_version_id",
        "target_part_id",
        "recipe_candidate_id",
        "recipe_candidate_sha256",
        "recipe",
        "expected_shape_program_sha256",
        "preview_shape_program_sha256",
        "shape_program",
        "assembly_graph",
        "material_bindings",
    }
    for row in domains:
        if not isinstance(row, dict) or set(row) != expected_keys:
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_ROW_INVALID", "unexpected domain evidence fields")
        if row.get("schema_version") != LIFECYCLE_DOMAIN_SCHEMA:
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_ROW_VERSION_INVALID", str(row.get("domain_slug")))
        slug = row.get("domain_slug")
        domain_pack_id = row.get("domain_pack_id")
        if not isinstance(slug, str) or not isinstance(domain_pack_id, str):
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_DOMAIN_INVALID", repr(slug))
        if observed.setdefault(slug, domain_pack_id) != domain_pack_id:
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_DOMAIN_DUPLICATE", slug)
        if not all(
            isinstance(row.get(key), str) and row[key]
            for key in (
                "project_id",
                "asset_version_id",
                "target_part_id",
                "recipe_candidate_id",
                "recipe_candidate_sha256",
                "expected_shape_program_sha256",
                "preview_shape_program_sha256",
            )
        ):
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_ID_INVALID", slug)
        for key in (
            "recipe_candidate_sha256",
            "expected_shape_program_sha256",
            "preview_shape_program_sha256",
        ):
            if not re.fullmatch(r"[a-f0-9]{64}", str(row[key])):
                _fail("C105_LIFECYCLE_REAL_EVIDENCE_HASH_INVALID", f"{slug}/{key}")
        if not isinstance(row.get("recipe"), dict) or not isinstance(row.get("shape_program"), dict):
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_SHAPE_INVALID", slug)
        if not isinstance(row.get("assembly_graph"), dict) or not isinstance(row.get("material_bindings"), dict):
            _fail("C105_LIFECYCLE_REAL_EVIDENCE_GRAPH_INVALID", slug)
        rows.append(row)
    if observed != EXPECTED_DOMAINS:
        _fail("C105_LIFECYCLE_REAL_EVIDENCE_DOMAIN_COVERAGE_INVALID", repr(observed))
    return rows


def _assert_no_recipe_context_crosses_python_boundary(program: Mapping[str, Any], slug: str) -> None:
    forbidden = {
        "assembly_graph",
        "candidate_sha256",
        "component_recipe",
        "component_recipe_instances",
        "component_recipe_ref",
        "project_id",
        "recipe",
        "recipe_candidate",
        "recipe_candidate_id",
        "recipe_candidate_sha256",
        "snapshot_revision",
        "target_part_id",
    }

    def walk(value: Any) -> Sequence[str]:
        if isinstance(value, dict):
            keys = list(value)
            for child in value.values():
                keys.extend(walk(child))
            return keys
        if isinstance(value, list):
            keys: list[str] = []
            for child in value:
                keys.extend(walk(child))
            return keys
        return []

    leaked = sorted(set(walk(program)) & forbidden)
    if leaked:
        _fail("C105_LIFECYCLE_PYTHON_BOUNDARY_CONTEXT_LEAK", f"{slug}: {leaked}")


def _expected_zone_materials(row: Mapping[str, Any]) -> set[tuple[str, str]]:
    bindings = row["material_bindings"]
    assert isinstance(bindings, dict)
    expected: set[tuple[str, str]] = set()
    for key, value in bindings.items():
        if not isinstance(key, str) or ":" not in key:
            _fail("C105_LIFECYCLE_MATERIAL_BINDING_INVALID", str(row["domain_slug"]))
        _part_id, zone_id = key.split(":", 1)
        if not zone_id or not isinstance(value, str) or not value.startswith("mat_"):
            _fail("C105_LIFECYCLE_MATERIAL_BINDING_INVALID", str(row["domain_slug"]))
        expected.add((zone_id, value))
    if not expected:
        _fail("C105_LIFECYCLE_MATERIAL_BINDING_MISSING", str(row["domain_slug"]))
    return expected


def _compile_real_active_recipe_lifecycle(rows: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    agent_root = ROOT / "apps/agent"
    scripts_root = ROOT / "scripts"
    for path in (agent_root, scripts_root):
        if str(path) not in sys.path:
            sys.path.insert(0, str(path))
    try:
        from forgecad_agent.application.restricted_geometry_executor import (
            RestrictedGeometryExecutionRequest,
            RestrictedGeometryExecutor,
        )
        from forgecad_gate_rust_python_contract import _parse_glb, verify_source_face_provenance
    except ImportError as exc:
        _fail("C105_LIFECYCLE_RESTRICTED_GEOMETRY_UNAVAILABLE", str(exc))

    executor = RestrictedGeometryExecutor(environment={})
    reports: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        slug = str(row["domain_slug"])
        program = row["shape_program"]
        assert isinstance(program, dict)
        _assert_no_recipe_context_crosses_python_boundary(program, slug)
        expected_sha = str(row["expected_shape_program_sha256"])
        actual_sha = _canonical_sha256(program)
        if actual_sha != expected_sha or row["preview_shape_program_sha256"] != expected_sha:
            _fail("C105_LIFECYCLE_SEALED_PROGRAM_HASH_MISMATCH", slug)
        outputs = program.get("outputs")
        operations = program.get("operations")
        graph = row["assembly_graph"]
        assert isinstance(graph, dict)
        if not isinstance(outputs, list) or not isinstance(operations, list):
            _fail("C105_LIFECYCLE_SEALED_PROGRAM_INVALID", slug)
        graph_parts = graph.get("parts")
        instances = graph.get("component_recipe_instances")
        if not isinstance(graph_parts, list) or not graph_parts or not isinstance(instances, list) or not instances:
            _fail("C105_LIFECYCLE_RECIPE_PROVENANCE_MISSING", slug)
        graph_operation_ids = {
            part.get("operation_id")
            for part in graph_parts
            if isinstance(part, dict) and isinstance(part.get("operation_id"), str)
        }
        output_ids = {
            output.get("output_id")
            for output in outputs
            if isinstance(output, dict) and isinstance(output.get("output_id"), str)
        }
        if not graph_operation_ids or any(
            not isinstance(part, dict) or part.get("output_id") not in output_ids
            for part in graph_parts
        ):
            _fail("C105_LIFECYCLE_GRAPH_PROGRAM_BINDING_INVALID", slug)
        expected_materials = _expected_zone_materials(row)
        request = RestrictedGeometryExecutionRequest.model_validate(
            {
                "schema_version": "RestrictedGeometryExecutionRequest@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "execution_id": f"exec_c105_active_recipe_{index}",
                "idempotency_key": f"idem_c105_active_recipe_{index}",
                "cancellation_id": f"cancel_c105_active_recipe_{index}",
                "cancellation_token": f"cancel_token_c105_active_recipe_{index}",
                "action": "compile_readback",
                "timeout_ms": 120_000,
                "artifact_profile_id": "production_concept",
                "shape_program": program,
            }
        )
        try:
            result = executor.execute(request)
        except Exception as exc:
            _fail("C105_LIFECYCLE_REAL_PROGRAM_COMPILE_FAILED", f"{slug}: {exc}")
        if not result.glb_base64 or not isinstance(result.readback, dict):
            _fail("C105_LIFECYCLE_REAL_READBACK_MISSING", slug)
        try:
            glb = base64.b64decode(result.glb_base64, validate=True)
        except (ValueError, binascii.Error):
            _fail("C105_LIFECYCLE_REAL_GLB_INVALID_BASE64", slug)
        if hashlib.sha256(glb).hexdigest() != result.glb_sha256 or len(glb) != result.glb_byte_size:
            _fail("C105_LIFECYCLE_REAL_GLB_IDENTITY_MISMATCH", slug)
        if result.shape_program_sha256 != expected_sha:
            _fail("C105_LIFECYCLE_REAL_SHAPE_HASH_MISMATCH", slug)
        readback = result.readback
        if (
            readback.get("shape_program_sha256") != expected_sha
            or readback.get("glb_sha256") != result.glb_sha256
            or readback.get("closed_manifold") is not True
            or readback.get("surface_provenance_present") is not True
            or readback.get("artifact_profile", {}).get("artifact_profile_id") != "production_concept"
        ):
            _fail("C105_LIFECYCLE_REAL_READBACK_IDENTITY_MISMATCH", slug)
        actual_materials = {
            (str(item.get("material_zone_id")), str(item.get("material_id")))
            for item in readback.get("material_zone_faces", [])
            if isinstance(item, dict)
        }
        if actual_materials != expected_materials:
            _fail(
                "C105_LIFECYCLE_REAL_MATERIAL_ZONE_DRIFT",
                f"{slug}: {sorted(actual_materials)} != {sorted(expected_materials)}",
            )
        actual_source_operations = {
            source
            for item in readback.get("surface_provenance", [])
            if isinstance(item, dict)
            for source in item.get("source_operation_ids", [])
            if isinstance(source, str)
        }
        if not graph_operation_ids.issubset(actual_source_operations):
            _fail("C105_LIFECYCLE_REAL_PROVENANCE_DRIFT", slug)
        try:
            document, binary = _parse_glb(glb)
            provenance = verify_source_face_provenance(document, binary)
        except Exception as exc:
            _fail("C105_LIFECYCLE_REAL_GLB_PROVENANCE_INVALID", f"{slug}: {exc}")
        if provenance["triangle_count"] != result.triangle_count or result.triangle_count < 1:
            _fail("C105_LIFECYCLE_REAL_GLB_READBACK_DRIFT", slug)
        reports.append(
            {
                "domain": slug,
                "shape_program_sha256": expected_sha,
                "glb_sha256": result.glb_sha256,
                "triangles": result.triangle_count,
                "zones": len(actual_materials),
            }
        )
    return reports


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--skip-regression-gates",
        action="store_true",
        help="Only run C105 contracts and focused Rust lifecycle tests (diagnostic use).",
    )
    args = parser.parse_args()

    if not RUST.is_file() or not MANIFEST.is_file():
        raise GateFailure("C105_LIFECYCLE_RUST_TOOLCHAIN_MISSING")

    # This test locks build -> segment -> commit, active candidate zero-write,
    # ratio/material previews, sealed Recipe replacement, Q003 quality/export,
    # restart, undo/redo and repeated replacement provenance across all four
    # domains.  It deliberately performs no real Provider call.
    with tempfile.TemporaryDirectory(prefix="forgecad-c105-real-geometry-") as temporary_evidence:
        evidence_dir = Path(temporary_evidence)
        lifecycle_tests = run(
            "RUST_FOUR_DOMAIN",
            cargo_filter("rust_blockout_compat_c105_recipe_lifecycle_all_domains"),
            environment={
                "FORGECAD_C105_RECIPE_LIFECYCLE_EVIDENCE_DIR": str(evidence_dir),
            },
        )
        real_geometry_reports = _compile_real_active_recipe_lifecycle(
            _load_real_geometry_evidence(evidence_dir / LIFECYCLE_EVIDENCE_FILE)
        )
    upgrade_tests = run(
        "RUST_VERSION_UPGRADE",
        [
            str(RUST),
            "cargo",
            "test",
            "--manifest-path",
            str(MANIFEST),
            "-p",
            "forgecad-core",
            "--test",
            "component_recipe_contract",
            "--offline",
            "c105_recipe_version_upgrade_preserves_old_candidate_hash_and_rejects_stale_ref",
        ],
    )
    run(
        "C105_CONTRACT_AND_PYTHON_BOUNDARY",
        [str(ROOT / ".venv" / "bin" / "python"), str(ROOT / "scripts" / "smoke_c105_component_recipe.py")],
    )
    if not args.skip_regression_gates:
        run(
            "Q003_REGRESSION",
            [str(ROOT / ".venv" / "bin" / "python"), str(ROOT / "scripts" / "smoke_q003_compile_readback_quality.py")],
        )
        run(
            "M108A_REGRESSION",
            ["npm", "run", "agent:m108-production-concept-smoke"],
            timeout=1_200,
        )
    print(
        "C105 Recipe lifecycle gate passed: "
        f"four_domain_rust_tests={lifecycle_tests}, "
        f"real_python_production_glbs={len(real_geometry_reports)}, "
        f"version_upgrade_rust_tests={upgrade_tests}, "
        "provider_calls=0"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(1) from None
