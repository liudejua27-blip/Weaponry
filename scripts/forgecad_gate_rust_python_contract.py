#!/usr/bin/env python3
"""FGC Rust<->Python contract gate.

This gate is intentionally an operational contract adapter, not a second
source of product truth.  It imports the Python catalog/schema, reads the
Rust code-owned catalog projections, runs the existing boundary tests and
Rust golden tests, and checks one real restricted-geometry GLB without
printing its JSON or binary payload.

The command always emits one bounded ``ForgeCADRustPythonContractGateReport@1``
JSON object.  Detailed subprocess output is retained only in memory and is
never copied into the report; this keeps prompts, bodies, GLB bytes and
secrets out of the gate surface.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
from typing import Any, Iterable, Mapping


ROOT = Path(__file__).resolve().parents[1]
AGENT_ROOT = ROOT / "apps" / "agent"
TAURI_ROOT = ROOT / "apps" / "desktop" / "src-tauri"
RUST_CATALOG_SOURCE = ROOT / "apps" / "desktop" / "src-tauri" / "src" / "rust_product_catalog.rs"
RUST_REPOSITORY_SOURCE = ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-core" / "src" / "repository.rs"
RUST_TOOLCHAIN = ROOT / "script" / "with_rust_toolchain.sh"

REPORT_SCHEMA = "ForgeCADRustPythonContractGateReport@1"
SELF_TEST_SCHEMA = "ForgeCADRustPythonContractSelfTest@1"
GATE_ID = "FGC-GATE-RUST-PYTHON-CONTRACT"
PHASE = "gate.rust_python_contract"
SUBSYSTEM = "rust_python_boundary"
EXPECTED_SHAPE_PROGRAM_SHA256 = "e90ed38d0814a2c53f177fab3bd3673b3b5cb0565d2fc40a06741acb4b28d156"


class GateContractFailure(RuntimeError):
    """Stable, value-free failure used by both the gate and its self-tests."""

    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code


SHAPE_PROGRAM_FIXTURE: dict[str, Any] = {
    "schema_version": "ShapeProgram@1",
    "program_id": "shape_gate_contract",
    "units": "millimeter",
    "seed": 41,
    "triangle_budget": 1000,
    "parameters": [],
    "operations": [
        {
            "operation_id": "op_body",
            "op": "box",
            "inputs": [],
            "args": {
                "position": [0.0, 0.0, 0.0],
                "size": [100.0, 40.0, 20.0],
                "part_role": "body_shell",
                "zone_id": "zone_body_shell",
                "material_id": "mat_graphite",
            },
        }
    ],
    "outputs": [
        {
            "output_id": "output_body",
            "operation_id": "op_body",
            "kind": "mesh",
            "part_role": "body_shell",
        }
    ],
    "non_functional_only": True,
}


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_json(value: Any) -> str:
    return _sha256_bytes(_canonical_json(value).encode("utf-8"))


def _check(check_id: str, result: str, error_code: str | None = None) -> dict[str, Any]:
    return {
        "check_id": check_id,
        "result": result,
        "stable_error_code": error_code,
    }


def _contract(
    contract_id: str,
    version: str,
    producer: str,
    consumer: str,
    fixture: Any,
    checks: list[dict[str, Any]],
) -> dict[str, Any]:
    failed = [item for item in checks if item["result"] != "passed"]
    return {
        "contract_id": contract_id,
        "version": version,
        "producer": producer,
        "consumer": consumer,
        "fixture_hash": _sha256_json(fixture),
        "result": "failed" if failed else "passed",
        "stable_error_code": failed[0]["stable_error_code"] if failed else None,
        "checks": checks,
    }


def _failed_check(check_id: str, code: str) -> dict[str, Any]:
    return _check(check_id, "failed", code)


def _read_source(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        raise GateContractFailure("RUST_CATALOG_SOURCE_UNAVAILABLE") from exc


def _rust_reviewed_material_ids() -> tuple[set[str], set[str]]:
    catalog_source = _read_source(RUST_CATALOG_SOURCE)
    catalog_ids = set(re.findall(r"preset\(\s*\"(mat_[a-z0-9_\-]+)\"", catalog_source))

    repository_source = _read_source(RUST_REPOSITORY_SOURCE)
    start = repository_source.find("fn material_allowed_domains")
    end = repository_source.find("fn insert_sorted_unique", start)
    if start < 0 or end < 0:
        raise GateContractFailure("RUST_MATERIAL_ROUTING_SOURCE_UNAVAILABLE")
    routing_block = repository_source[start:end]
    accepted_ids = set(re.findall(r'"(mat_[a-z0-9_\-]+)"', routing_block))
    return catalog_ids, accepted_ids


def _python_material_catalog() -> tuple[set[str], dict[str, str], set[str]]:
    sys.path.insert(0, str(AGENT_ROOT))
    try:
        from forgecad_agent.application.material_catalog import list_material_presets
        from forgecad_agent.application.visual_texture_sets import (
            _AUTHORED_TO_TEXTURE_MATERIAL,
            builtin_material_properties,
            builtin_visual_material_count,
        )

        presets = list_material_presets()
        public_ids = {preset.material_id for preset in presets}
        aliases = dict(_AUTHORED_TO_TEXTURE_MATERIAL)
        texture_ids = {
            str(builtin_material_properties(index)["material_id"])
            for index in range(builtin_visual_material_count())
        }
        return public_ids, aliases, texture_ids
    except (ImportError, AttributeError, ValueError) as exc:
        raise GateContractFailure("PYTHON_MATERIAL_CATALOG_UNAVAILABLE") from exc


def validate_material_catalog_contract(*, inject_drift: bool = False) -> dict[str, Any]:
    rust_catalog_ids, rust_routing_ids = _rust_reviewed_material_ids()
    python_ids, aliases, texture_ids = _python_material_catalog()
    if inject_drift:
        python_ids = set(python_ids)
        python_ids.add("mat_injected_drift")

    checks: list[dict[str, Any]] = []
    if python_ids != rust_catalog_ids or rust_catalog_ids != rust_routing_ids:
        checks.append(_failed_check("catalog_id_set", "MATERIAL_CATALOG_DRIFT"))
    else:
        checks.append(_check("catalog_id_set", "passed"))

    if "mat_graphite" not in python_ids or "mat_graphite" not in rust_catalog_ids:
        checks.append(_failed_check("mat_graphite_anchor", "MATERIAL_GRAPHITE_MISSING"))
    else:
        checks.append(_check("mat_graphite_anchor", "passed"))

    missing_aliases = python_ids - set(aliases)
    invalid_targets = set(aliases.values()) - texture_ids
    if missing_aliases or invalid_targets:
        checks.append(_failed_check("alias_rules", "MATERIAL_ALIAS_RULE_DRIFT"))
    else:
        checks.append(_check("alias_rules", "passed"))

    fixture = {
        "rust_catalog_ids": sorted(rust_catalog_ids),
        "rust_routing_ids": sorted(rust_routing_ids),
        "python_catalog_ids": sorted(python_ids),
        "python_aliases": sorted(aliases.items()),
    }
    return _contract(
        "material_catalog",
        "MaterialCatalogRustPythonContract@1",
        "rust_product_catalog.rs + forgecad_agent.material_catalog",
        "Rust core material routing + Python geometry texture binding",
        fixture,
        checks,
    )


def _number_class(value: Any) -> str:
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "signed_integer" if value < 0 else "unsigned_integer"
    if isinstance(value, float):
        return "float"
    return "non_number"


def _number_classes(value: Any, path: str = "$") -> dict[str, str]:
    if isinstance(value, Mapping):
        result: dict[str, str] = {}
        for key, child in value.items():
            result.update(_number_classes(child, f"{path}/{key}"))
        return result
    if isinstance(value, list):
        result = {}
        for index, child in enumerate(value):
            result.update(_number_classes(child, f"{path}/{index}"))
        return result
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return {path: _number_class(value)}
    return {}


def validate_shape_program_contract(*, inject_hash_drift: bool = False) -> dict[str, Any]:
    sys.path.insert(0, str(AGENT_ROOT))
    try:
        from forgecad_agent.application.restricted_geometry_executor import (
            _canonical_json_sha256,
        )
        from forgecad_agent.application.shape_program import validate_shape_program
    except ImportError as exc:
        raise GateContractFailure("PYTHON_SHAPEPROGRAM_CONTRACT_UNAVAILABLE") from exc

    validated = validate_shape_program(SHAPE_PROGRAM_FIXTURE)
    persisted_text = _canonical_json(validated)
    reparsed = json.loads(persisted_text)
    if reparsed != validated:
        raise GateContractFailure("SHAPE_PROGRAM_PERSISTENCE_ROUNDTRIP_DRIFT")

    actual_hash = _canonical_json_sha256(reparsed)
    expected_hash = EXPECTED_SHAPE_PROGRAM_SHA256
    if inject_hash_drift:
        expected_hash = "0" * 64
    checks = [
        _check("json_text_serialize_parse", "passed"),
        _check("number_categories", "passed" if sorted(_number_classes(reparsed).items()) == sorted(_number_classes(validated).items()) else "failed", None if sorted(_number_classes(reparsed).items()) == sorted(_number_classes(validated).items()) else "SHAPE_PROGRAM_NUMBER_CATEGORY_DRIFT"),
        _check("semantic_hash_fixture", "passed" if actual_hash == expected_hash else "failed", None if actual_hash == expected_hash else "SHAPE_PROGRAM_SEMANTIC_HASH_DRIFT"),
    ]
    fixture = {
        "shape_program": reparsed,
        "persisted_text_sha256": _sha256_bytes(persisted_text.encode("utf-8")),
        "semantic_hash": actual_hash,
        "number_categories": _number_classes(reparsed),
    }
    return _contract(
        "shape_program_persistence",
        "ShapeProgramPersistenceRustPythonContract@1",
        "Rust forgecad-core canonical/normalization + Python restricted executor",
        "Rust repository persistence, Python ShapeProgram validator/worker",
        fixture,
        checks,
    )


def validate_restricted_geometry_contract() -> dict[str, Any]:
    sys.path.insert(0, str(AGENT_ROOT))
    try:
        from forgecad_agent.api.restricted_geometry_routes import RestrictedGeometryCapabilityOwnership
        from forgecad_agent.application.restricted_geometry_executor import (
            _ALLOWED_EXECUTOR_ENVIRONMENT_NAMES,
            _FORBIDDEN_CONTEXT_KEYS,
            RestrictedGeometryExecutionRequest,
            validate_restricted_geometry_environment,
            validate_restricted_geometry_payload,
        )
    except ImportError as exc:
        raise GateContractFailure("PYTHON_RESTRICTED_SCHEMA_UNAVAILABLE") from exc

    schema = RestrictedGeometryExecutionRequest.model_json_schema()
    ownership = RestrictedGeometryCapabilityOwnership().model_dump(mode="json")
    required_forbidden = {"database_path", "provider_key", "object_store_path", "file_path", "url"}
    if not required_forbidden.issubset(_FORBIDDEN_CONTEXT_KEYS):
        raise GateContractFailure("RESTRICTED_FORBIDDEN_PERMISSION_DRIFT")
    if _ALLOWED_EXECUTOR_ENVIRONMENT_NAMES != {
        "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN",
        "FORGECAD_RUNTIME_RESOURCE_ROOT",
    }:
        raise GateContractFailure("RESTRICTED_ENVIRONMENT_ALLOWLIST_DRIFT")
    if any(ownership.get(field) is not False for field in (
        "database_access", "object_store_access", "provider_access", "thread_session_access", "snapshot_write", "persistent_artifacts"
    )):
        raise GateContractFailure("RESTRICTED_OWNERSHIP_PERMISSION_DRIFT")

    safe_payload = {"schema_version": "safe", "values": [1, 2.0]}
    validate_restricted_geometry_payload(safe_payload)
    for forbidden in ("database_path", "provider_key", "object_store_path", "file_path", "url"):
        try:
            validate_restricted_geometry_payload({forbidden: "blocked"})
        except Exception:
            continue
        raise GateContractFailure("RESTRICTED_PERMISSION_NOT_REJECTED")
    try:
        validate_restricted_geometry_environment({"FORGECAD_AGENT_API_KEY": "blocked"})
    except Exception:
        pass
    else:
        raise GateContractFailure("RESTRICTED_PROVIDER_ENVIRONMENT_NOT_REJECTED")

    checks = [
        _check("input_schema_extra_forbid", "passed" if schema.get("additionalProperties") is False else "failed", None if schema.get("additionalProperties") is False else "RESTRICTED_INPUT_SCHEMA_DRIFT"),
        _check("output_ownership_permissions", "passed"),
        _check("database_provider_path_rejection", "passed"),
    ]
    fixture = {
        "protocol_version": "forgecad.restricted-geometry/1",
        "input_schema_sha256": _sha256_json(schema),
        "ownership_schema_sha256": _sha256_json(ownership),
        "forbidden_context_keys": sorted(_FORBIDDEN_CONTEXT_KEYS),
        "allowed_environment_names": sorted(_ALLOWED_EXECUTOR_ENVIRONMENT_NAMES),
    }
    return _contract(
        "restricted_geometry_boundary",
        "RestrictedGeometryRustPythonContract@1",
        "Python restricted_geometry_executor models and route ownership",
        "Rust core capability-gated geometry port",
        fixture,
        checks,
    )


def _parse_glb(glb: bytes) -> tuple[dict[str, Any], bytes]:
    if len(glb) < 20 or glb[:4] != b"glTF" or struct.unpack_from("<I", glb, 4)[0] != 2:
        raise GateContractFailure("GLB_CONTAINER_INVALID")
    declared_length = struct.unpack_from("<I", glb, 8)[0]
    if declared_length != len(glb):
        raise GateContractFailure("GLB_CONTAINER_LENGTH_INVALID")
    offset = 12
    document: dict[str, Any] | None = None
    binary = b""
    while offset < len(glb):
        if offset + 8 > len(glb):
            raise GateContractFailure("GLB_CHUNK_INVALID")
        chunk_length, chunk_type = struct.unpack_from("<II", glb, offset)
        chunk = glb[offset + 8 : offset + 8 + chunk_length]
        if len(chunk) != chunk_length:
            raise GateContractFailure("GLB_CHUNK_INVALID")
        if chunk_type == 0x4E4F534A:
            try:
                parsed = json.loads(chunk.decode("utf-8").rstrip(" \t\r\n\0"))
            except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                raise GateContractFailure("GLB_JSON_INVALID") from exc
            if not isinstance(parsed, dict):
                raise GateContractFailure("GLB_JSON_INVALID")
            document = parsed
        elif chunk_type == 0x004E4942:
            binary = chunk
        offset += 8 + chunk_length
    if document is None or not binary:
        raise GateContractFailure("GLB_REQUIRED_CHUNKS_MISSING")
    return document, binary


def _accessor_values(document: Mapping[str, Any], binary: bytes, accessor_index: int) -> list[float | int]:
    accessors = document.get("accessors")
    views = document.get("bufferViews")
    if not isinstance(accessors, list) or not isinstance(views, list):
        raise GateContractFailure("GLB_ACCESSOR_TABLE_INVALID")
    if not isinstance(accessor_index, int) or accessor_index < 0 or accessor_index >= len(accessors):
        raise GateContractFailure("GLB_ACCESSOR_REFERENCE_INVALID")
    accessor = accessors[accessor_index]
    view_index = accessor.get("bufferView")
    if not isinstance(view_index, int) or view_index < 0 or view_index >= len(views):
        raise GateContractFailure("GLB_ACCESSOR_VIEW_INVALID")
    view = views[view_index]
    count = accessor.get("count")
    component_type = accessor.get("componentType")
    accessor_type = accessor.get("type")
    component_count = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}.get(accessor_type)
    if not isinstance(count, int) or count < 0 or component_count is None:
        raise GateContractFailure("GLB_ACCESSOR_SCHEMA_INVALID")
    if component_type not in (5123, 5125, 5126):
        raise GateContractFailure("GLB_ACCESSOR_COMPONENT_INVALID")
    component_format, component_size = {5123: ("<H", 2), 5125: ("<I", 4), 5126: ("<f", 4)}[component_type]
    element_size = component_size * component_count
    stride = view.get("byteStride") or element_size
    start = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
    values: list[float | int] = []
    for index in range(count):
        position = start + index * int(stride)
        element: list[float | int] = []
        for component_index in range(component_count):
            component_position = position + component_index * component_size
            raw = binary[component_position : component_position + component_size]
            if len(raw) != component_size:
                raise GateContractFailure("GLB_ACCESSOR_DATA_TRUNCATED")
            element.append(struct.unpack(component_format, raw)[0])
        values.append(element[0] if component_count == 1 else tuple(element))
    return values


def verify_source_face_provenance(document: Mapping[str, Any], binary: bytes) -> dict[str, int]:
    meshes = document.get("meshes")
    if not isinstance(meshes, list) or not meshes:
        raise GateContractFailure("GLB_MESH_TABLE_INVALID")
    triangle_count = 0
    provenance_triangle_count = 0
    primitive_count = 0
    for mesh in meshes:
        primitives = mesh.get("primitives") if isinstance(mesh, dict) else None
        if not isinstance(primitives, list):
            raise GateContractFailure("GLB_PRIMITIVE_TABLE_INVALID")
        for primitive in primitives:
            primitive_count += 1
            attributes = primitive.get("attributes") if isinstance(primitive, dict) else None
            if not isinstance(attributes, dict):
                raise GateContractFailure("GLB_ATTRIBUTES_MISSING")
            source_accessor = attributes.get("_FORGECAD_SOURCE_FACE_ID")
            position_accessor = attributes.get("POSITION")
            if not isinstance(source_accessor, int):
                raise GateContractFailure("GLB_SOURCE_FACE_ACCESSOR_MISSING")
            if not isinstance(position_accessor, int):
                raise GateContractFailure("GLB_POSITION_ACCESSOR_MISSING")
            source_ids = _accessor_values(document, binary, source_accessor)
            positions = _accessor_values(document, binary, position_accessor)
            if len(source_ids) != len(positions):
                raise GateContractFailure("GLB_SOURCE_FACE_VERTEX_COUNT_MISMATCH")
            index_accessor = primitive.get("indices")
            indices = list(range(len(positions))) if index_accessor is None else _accessor_values(document, binary, index_accessor)
            if len(indices) == 0 or len(indices) % 3:
                raise GateContractFailure("GLB_TRIANGLE_INDEX_COUNT_INVALID")
            triangle_count += len(indices) // 3
            for triangle_offset in range(0, len(indices), 3):
                triangle = indices[triangle_offset : triangle_offset + 3]
                try:
                    ids = [source_ids[int(index)] for index in triangle]
                except (IndexError, TypeError, ValueError) as exc:
                    raise GateContractFailure("GLB_SOURCE_FACE_INDEX_INVALID") from exc
                if any(not isinstance(value, (int, float)) or not float(value).is_integer() or value < 0 for value in ids):
                    raise GateContractFailure("GLB_SOURCE_FACE_VALUE_INVALID")
                if ids[0] != ids[1] or ids[1] != ids[2]:
                    raise GateContractFailure("GLB_SOURCE_FACE_TRIANGLE_MISMATCH")
                provenance_triangle_count += 1
    if provenance_triangle_count != triangle_count:
        raise GateContractFailure("GLB_SOURCE_FACE_TRIANGLE_COUNT_MISMATCH")
    return {
        "primitive_count": primitive_count,
        "triangle_count": triangle_count,
        "provenance_triangle_count": provenance_triangle_count,
    }


def validate_glb_provenance_contract() -> dict[str, Any]:
    sys.path.insert(0, str(AGENT_ROOT))
    try:
        from forgecad_agent.application.restricted_geometry_executor import (
            RestrictedGeometryExecutionRequest,
            RestrictedGeometryExecutor,
        )
    except ImportError as exc:
        raise GateContractFailure("PYTHON_GEOMETRY_EXECUTOR_UNAVAILABLE") from exc

    request = RestrictedGeometryExecutionRequest.model_validate({
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": "exec_gate_glb_provenance",
        "idempotency_key": "idem_gate_glb_provenance",
        "cancellation_id": "cancel_gate_glb_provenance",
        "cancellation_token": "cancel_token_gate_glb_provenance",
        "action": "compile_readback",
        "timeout_ms": 30000,
        "artifact_profile_id": "interactive_preview",
        "shape_program": SHAPE_PROGRAM_FIXTURE,
    })
    result = RestrictedGeometryExecutor(environment={}).execute(request)
    if not result.glb_base64:
        raise GateContractFailure("GLB_RESULT_BYTES_MISSING")
    try:
        glb = base64.b64decode(result.glb_base64, validate=True)
    except (ValueError, binascii.Error) as exc:  # type: ignore[name-defined]
        raise GateContractFailure("GLB_RESULT_BASE64_INVALID") from exc
    document, binary = _parse_glb(glb)
    observation = verify_source_face_provenance(document, binary)
    if observation["triangle_count"] != observation["provenance_triangle_count"]:
        raise GateContractFailure("GLB_SOURCE_FACE_TRIANGLE_COUNT_MISMATCH")
    checks = [
        _check("source_face_accessor_present", "passed"),
        _check("per_triangle_provenance_count", "passed"),
        _check("glb_sha256_and_size", "passed" if _sha256_bytes(glb) == result.glb_sha256 and len(glb) == result.glb_byte_size else "failed", None if _sha256_bytes(glb) == result.glb_sha256 and len(glb) == result.glb_byte_size else "GLB_HASH_OR_SIZE_DRIFT"),
    ]
    fixture = {
        "shape_program_sha256": result.shape_program_sha256,
        "glb_sha256": result.glb_sha256,
        "glb_byte_size": result.glb_byte_size,
        "observation": observation,
    }
    return _contract(
        "glb_surface_provenance",
        "ForgeCADGlbSurfaceProvenanceRustPythonContract@1",
        "Python restricted geometry GLB compiler",
        "Rust forgecad-core GLB readback",
        fixture,
        checks,
    )


def _run_command(command_id: str, argv: list[str], *, cwd: Path) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            argv,
            cwd=cwd,
            env=dict(os.environ),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    except OSError:
        return {"command_id": command_id, "result": "failed", "stable_error_code": "GATE_COMMAND_UNAVAILABLE"}
    return {
        "command_id": command_id,
        "result": "passed" if completed.returncode == 0 else "failed",
        "stable_error_code": None if completed.returncode == 0 else f"{command_id.upper()}_FAILED",
        "exit_code": completed.returncode,
        "observed_pass_count": _observed_pass_count(completed.stdout + completed.stderr),
    }


def _observed_pass_count(output: str) -> int | None:
    match = re.search(r"(\d+) passed", output)
    return int(match.group(1)) if match else None


def validate_test_orchestration_contract() -> dict[str, Any]:
    python = ROOT / ".venv" / "bin" / "python"
    python_test = _run_command(
        "python_boundary_21",
        [str(python), "-m", "pytest", "-q", "apps/agent/tests/test_k003_restricted_geometry_executor.py"],
        cwd=ROOT,
    )
    if python_test["result"] == "passed" and python_test.get("observed_pass_count") != 21:
        python_test = {**python_test, "result": "failed", "stable_error_code": "PYTHON_BOUNDARY_TEST_COUNT_DRIFT"}

    rust_commands = [
        (
            "rust_shape_program_golden",
            "shape_program::tests::",
        ),
        (
            "rust_glb_topology_golden",
            "artifact_readback::tests::",
        ),
        (
            "rust_material_catalog_golden",
            "rust_product_catalog::tests::visual_catalogs_are_complete_unique_and_non_engineering",
        ),
    ]
    rust_results = [
        _run_command(
            command_id,
            [
                str(RUST_TOOLCHAIN),
                "cargo",
                "test",
                "--manifest-path",
                "apps/desktop/src-tauri/Cargo.toml",
                "--offline",
                "-p",
                "forgecad-core" if command_id != "rust_material_catalog_golden" else "wushen-forge-desktop",
                "--lib" if command_id != "rust_material_catalog_golden" else "--bin",
                *([] if command_id != "rust_material_catalog_golden" else ["wushen-forge-desktop"]),
                filter_name,
            ],
            cwd=ROOT,
        )
        for command_id, filter_name in rust_commands
    ]
    checks = [
        _check("python_boundary_tests_21", python_test["result"], python_test.get("stable_error_code")),
        *[_check(item["command_id"], item["result"], item.get("stable_error_code")) for item in rust_results],
    ]
    fixture = {
        "python_test_module": "apps/agent/tests/test_k003_restricted_geometry_executor.py",
        "python_expected_count": 21,
        "rust_golden_filters": [item[1] for item in rust_commands],
    }
    return _contract(
        "boundary_test_orchestration",
        "RustPythonBoundaryTestOrchestration@1",
        "existing Python boundary tests + forgecad-core/Rust catalog golden tests",
        "FGC Rust<->Python contract gate",
        fixture,
        checks,
    )


def run_fault_injection_self_tests() -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    try:
        result = validate_material_catalog_contract(inject_drift=True)
        checks.append(_check("material_drift", "passed" if result["stable_error_code"] == "MATERIAL_CATALOG_DRIFT" else "failed", None if result["stable_error_code"] == "MATERIAL_CATALOG_DRIFT" else "SELF_TEST_MATERIAL_DRIFT_NOT_DETECTED"))
    except GateContractFailure as exc:
        checks.append(_check("material_drift", "passed" if exc.code == "MATERIAL_CATALOG_DRIFT" else "failed", None if exc.code == "MATERIAL_CATALOG_DRIFT" else "SELF_TEST_MATERIAL_DRIFT_UNEXPECTED"))

    result = validate_shape_program_contract(inject_hash_drift=True)
    checks.append(_check(
        "hash_drift",
        "passed" if result["stable_error_code"] == "SHAPE_PROGRAM_SEMANTIC_HASH_DRIFT" else "failed",
        None if result["stable_error_code"] == "SHAPE_PROGRAM_SEMANTIC_HASH_DRIFT" else "SELF_TEST_HASH_DRIFT_NOT_DETECTED",
    ))

    try:
        sys.path.insert(0, str(AGENT_ROOT))
        from forgecad_agent.application.restricted_geometry_executor import RestrictedGeometryExecutor, RestrictedGeometryExecutionRequest
        # A structurally valid fixture is not needed here: the verifier must
        # reject a missing accessor before it could inspect triangle values.
        del RestrictedGeometryExecutor, RestrictedGeometryExecutionRequest
        verify_source_face_provenance({"meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}], "accessors": [], "bufferViews": []}, b"")
    except GateContractFailure as exc:
        checks.append(_check("accessor_missing", "passed" if exc.code == "GLB_SOURCE_FACE_ACCESSOR_MISSING" else "failed", None if exc.code == "GLB_SOURCE_FACE_ACCESSOR_MISSING" else "SELF_TEST_ACCESSOR_MISSING_UNEXPECTED"))
    else:
        checks.append(_check("accessor_missing", "failed", "SELF_TEST_ACCESSOR_MISSING_NOT_DETECTED"))

    return {
        "schema_version": SELF_TEST_SCHEMA,
        "phase": PHASE,
        "subsystem": SUBSYSTEM,
        "result": "passed" if all(item["result"] == "passed" for item in checks) else "failed",
        "stable_error_code": next((item["stable_error_code"] for item in checks if item["result"] != "passed"), None),
        "exit_code": 0 if all(item["result"] == "passed" for item in checks) else 1,
        "checks": checks,
    }


def run_gate() -> dict[str, Any]:
    contracts: list[dict[str, Any]] = []
    for function in (
        validate_material_catalog_contract,
        validate_shape_program_contract,
        validate_restricted_geometry_contract,
        validate_glb_provenance_contract,
        validate_test_orchestration_contract,
    ):
        try:
            contracts.append(function())
        except GateContractFailure as exc:
            contracts.append(_contract(
                function.__name__.removeprefix("validate_").removesuffix("_contract"),
                "RustPythonContract@1",
                "contract producer unavailable",
                "FGC gate",
                {"function": function.__name__},
                [_failed_check("execution", exc.code)],
            ))
        except Exception:
            contracts.append(_contract(
                function.__name__.removeprefix("validate_").removesuffix("_contract"),
                "RustPythonContract@1",
                "contract producer unavailable",
                "FGC gate",
                {"function": function.__name__},
                [_failed_check("execution", "GATE_INTERNAL_ERROR")],
            ))
    failed = [item for item in contracts if item["result"] != "passed"]
    return {
        "schema_version": REPORT_SCHEMA,
        "gate_id": GATE_ID,
        "phase": PHASE,
        "subsystem": SUBSYSTEM,
        "result": "failed" if failed else "passed",
        "stable_error_code": failed[0]["stable_error_code"] if failed else None,
        "exit_code": 1 if failed else 0,
        "contracts": contracts,
        "summary": {
            "contract_count": len(contracts),
            "passed_count": len(contracts) - len(failed),
            "failed_count": len(failed),
        },
    }


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="run only local fault-injection self-tests")
    args = parser.parse_args(list(argv) if argv is not None else None)
    report = run_fault_injection_self_tests() if args.self_test else run_gate()
    print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return int(report["exit_code"])


if __name__ == "__main__":
    raise SystemExit(main())
