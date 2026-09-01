#!/usr/bin/env python3
"""Run the closed same-input img2threejs/Weaponry browser benchmark.

The pinned img2threejs generator and the Weaponry compatibility importer read
the same ObjectSculptSpec bytes.  Both scenes use one normalization contract,
one fixed eight-view rig, one live WebGLRenderer, and the same required AOV
capture path.  Temporary generated code, browser PNG bytes, and the static
bundle are discarded; only a bounded comparison receipt is persisted.

The receipt deliberately separates structural evidence, pairwise pixel
measurements, and superiority claims.  Without an authorized reference image
and threshold contract, the pixel classification is NOT_PROVEN even when the
browser capture succeeds.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from typing import Any

from run_img2threejs_baseline import (
    BenchmarkBlocked,
    EXPECTED_TREE,
    REVISION,
    extract_pinned_source,
    parse_json_output,
    run_checked,
    sha256_bytes,
    sha256_file,
    verify_pinned_source,
)
from run_img2threejs_aov_baseline import (
    BrowserBlocked,
    discover_playwright_cli,
    parse_playwright_value,
    run_playwright,
    start_static_server,
)
from run_img2threejs_browser_baseline import load_json, validate_closed_inputs


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PACKAGE_ROOT.parents[1]
BENCHMARK_ROOT = PACKAGE_ROOT / "benchmark"
DEFAULT_SPEC = BENCHMARK_ROOT / "dragonfang-like-objects-sculpt-spec.json"
DEFAULT_CONTRACT = BENCHMARK_ROOT / "upstream-render-normalization.contract.json"
DEFAULT_BASELINE_RECEIPT = BENCHMARK_ROOT / "img2threejs-baseline.receipt.json"
DEFAULT_TEMPLATE = BENCHMARK_ROOT / "same-input-capture-entry.template.ts"
DEFAULT_ADAPTER = BENCHMARK_ROOT / "img2threejs-compiled-scene-adapter.ts"
DEFAULT_RECEIPT = BENCHMARK_ROOT / "same-input-benchmark.receipt.json"
DEFAULT_NODE_MODULES = REPOSITORY_ROOT / "node_modules"
FIXED_RIG_FINGERPRINT = "3fa0202473e3352b"
REQUIRED_AOV_IDS = ["beauty", "silhouette", "depth", "normal", "part-id", "material-id", "wireframe"]
EXPECTED_VIEW_IDS = ["FRONT", "BACK", "TOP", "BOTTOM", "LEFT", "RIGHT", "REAR_THREE_QUARTER", "FPS_HOLD"]
CAPTURE_SOURCE_FILES = [
    "knife-browser-capture.ts",
    "knife-view-evaluation.ts",
    "knife-scene-compiler.ts",
    "knife-scene-program.ts",
    "knife-assembly-compiler.ts",
    "knife-material.ts",
    "knife-surface-field.ts",
    "knife-attachment-loft.ts",
    "knife-relief-curve.ts",
    "img2threejs-source-envelope.ts",
    "img2threejs-compatibility-compiler.ts",
    "img2threejs-object-sculpt-adapter.ts",
]
SHA256_PATTERN = "0123456789abcdef"
STABLE_ID_PATTERN = re.compile(r"^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$")
CLASSIFICATIONS = ("STRUCTURAL_PARITY", "METRICALLY_SUPERIOR", "NOT_PROVEN")


class SameInputBlocked(BenchmarkBlocked):
    """A local benchmark blocker, never a product-quality result."""


def load_contract_json(path: Path, label: str) -> tuple[dict[str, Any], bytes]:
    return load_json(path.expanduser().resolve(), label)


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False)


def canonical_sha256(value: dict[str, Any]) -> str:
    payload = dict(value)
    payload["canonical_sha256"] = ""
    return sha256_bytes(canonical_json(payload).encode("utf-8"))


def bounded_sha(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in SHA256_PATTERN for character in value):
        raise SameInputBlocked(f"{label} is not a SHA-256")
    return value


def bounded_fingerprint(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) < 16 or len(value) > 128 or any(character not in SHA256_PATTERN for character in value):
        raise SameInputBlocked(f"{label} is not a bounded hexadecimal fingerprint")
    return value


def stable_id_list(value: Any, label: str, allowed: set[str] | None = None) -> list[str]:
    if not isinstance(value, list):
        raise SameInputBlocked(f"{label} is not a stable ID list")
    result: list[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str) or STABLE_ID_PATTERN.fullmatch(item) is None:
            raise SameInputBlocked(f"{label}[{index}] is not a bounded stable ID")
        if allowed is not None and item not in allowed:
            raise SameInputBlocked(f"{label}[{index}] is not present in the closed source ID set")
        result.append(item)
    if len(set(result)) != len(result):
        raise SameInputBlocked(f"{label} contains duplicate IDs")
    return result


def validate_import_receipt(
    receipt: dict[str, Any],
    spec: dict[str, Any],
    source_info: dict[str, Any],
) -> dict[str, Any]:
    """Validate the full importer envelope without turning missing IDs into a pass."""

    expected_components = spec.get("componentTree")
    expected_materials = spec.get("materials")
    if not isinstance(expected_components, list) or not isinstance(expected_materials, list):
        raise SameInputBlocked("ObjectSculptSpec source arrays are unavailable to the importer receipt validator")
    expected_component_ids = [component.get("id") for component in expected_components if isinstance(component, dict)]
    expected_material_ids = [material.get("id") for material in expected_materials if isinstance(material, dict)]
    if len(expected_component_ids) != len(expected_components) or len(expected_material_ids) != len(expected_materials):
        raise SameInputBlocked("ObjectSculptSpec source arrays contain malformed entries")
    if any(not isinstance(identifier, str) for identifier in expected_component_ids + expected_material_ids):
        raise SameInputBlocked("ObjectSculptSpec source IDs are not strings")
    component_set = set(expected_component_ids)
    material_set = set(expected_material_ids)
    component_materials = {
        component["id"]: component.get("material")
        for component in expected_components
        if isinstance(component, dict) and isinstance(component.get("id"), str)
    }
    component_roles = {
        component["id"]: component.get("role")
        for component in expected_components
        if isinstance(component, dict) and isinstance(component.get("id"), str)
    }
    component_primitives = {
        component["id"]: component.get("primitive")
        for component in expected_components
        if isinstance(component, dict) and isinstance(component.get("id"), str)
    }

    if receipt.get("schema_version") != "Img2ThreeJsKnifeImportReceipt@1":
        raise SameInputBlocked("compatibility import receipt schema drifted")
    if receipt.get("upstream_revision") != REVISION:
        raise SameInputBlocked("compatibility import receipt is not bound to the pinned upstream revision")
    if receipt.get("source_schema_version") != spec.get("schemaVersion") or receipt.get("source_target_name") != spec.get("targetName"):
        raise SameInputBlocked("compatibility import receipt target/schema binding drifted")
    source_blade_ids = [
        component["id"]
        for component in expected_components
        if isinstance(component, dict)
        and component.get("primitive") == "ground-blade"
        and (component.get("role") == "blade" or component.get("id") == "blade")
    ]
    if receipt.get("source_blade_component_id") not in source_blade_ids:
        raise SameInputBlocked("compatibility import receipt is not bound to the fixture ground-blade component")
    source_identity = receipt.get("source_identity")
    if not isinstance(source_identity, dict):
        raise SameInputBlocked("compatibility import receipt has no source identity")
    for field in ("revision", "tree", "generator_sha256", "validator_sha256"):
        if field not in source_identity:
            raise SameInputBlocked(f"compatibility import source identity is missing {field}")
    if source_identity.get("revision") != REVISION or source_identity.get("tree") != EXPECTED_TREE:
        raise SameInputBlocked("compatibility import source identity does not match the pinned commit/tree")
    if source_identity.get("revision") != source_info.get("revision") or source_identity.get("tree") != source_info.get("tree"):
        raise SameInputBlocked("compatibility import source identity disagrees with the pinned checkout proof")
    for field in ("generator_sha256", "validator_sha256"):
        bounded_sha(source_identity.get(field), f"compatibility import source identity {field}")
        if source_identity.get(field) != source_info.get(field):
            raise SameInputBlocked(f"compatibility import source identity {field} disagrees with pinned extract")
    station_count = receipt.get("imported_station_count")
    if not isinstance(station_count, int) or isinstance(station_count, bool) or station_count < 2:
        raise SameInputBlocked("compatibility import station count is not a bounded positive integer")
    if receipt.get("execution_performed") is not False or receipt.get("network_used") is not False or receipt.get("quality_status") != "NOT_RUN":
        raise SameInputBlocked("compatibility import receipt crossed its closed boundary")

    imported_components = stable_id_list(receipt.get("imported_component_ids"), "imported_component_ids", component_set)
    mapped_components = stable_id_list(receipt.get("mapped_component_ids"), "mapped_component_ids", component_set)
    preserved_components = stable_id_list(receipt.get("preserved_component_ids"), "preserved_component_ids", component_set)
    unsupported_components = stable_id_list(receipt.get("unsupported_component_ids"), "unsupported_component_ids", component_set)
    ignored_components = stable_id_list(receipt.get("ignored_component_ids"), "ignored_component_ids", component_set)
    imported_materials = stable_id_list(receipt.get("imported_material_ids"), "imported_material_ids", material_set)
    mapped_materials = stable_id_list(receipt.get("mapped_material_ids"), "mapped_material_ids", material_set)
    preserved_materials = stable_id_list(receipt.get("preserved_material_ids"), "preserved_material_ids", material_set)
    unsupported_materials = stable_id_list(receipt.get("unsupported_material_ids"), "unsupported_material_ids", material_set)
    if set(mapped_components) - set(imported_components) or set(preserved_components) - set(imported_components) or set(unsupported_components) - set(imported_components) or set(ignored_components) - set(imported_components):
        raise SameInputBlocked("component importer receipt contains an ID outside imported_component_ids")
    if set(mapped_materials) - set(imported_materials) or set(preserved_materials) - set(imported_materials) or set(unsupported_materials) - set(imported_materials):
        raise SameInputBlocked("material importer receipt contains an ID outside imported_material_ids")
    if set(ignored_components) != set(unsupported_components):
        raise SameInputBlocked("ignored_component_ids must remain the unsupported-component alias")

    component_mappings = receipt.get("component_mappings")
    if not isinstance(component_mappings, list) or len(component_mappings) != len(imported_components):
        raise SameInputBlocked("component_mappings is not a one-to-one mapping for imported components")
    mapped_component_mapping_ids: set[str] = set()
    component_mapping_exact = True
    for index, mapping in enumerate(component_mappings):
        if not isinstance(mapping, dict):
            raise SameInputBlocked(f"component_mappings[{index}] is not an object")
        component_id = mapping.get("source_component_id")
        if not isinstance(component_id, str) or component_id not in component_set or component_id in mapped_component_mapping_ids:
            raise SameInputBlocked(f"component_mappings[{index}] has an unknown or duplicate source component ID")
        mapped_component_mapping_ids.add(component_id)
        source_order = mapping.get("source_order")
        if not isinstance(source_order, int) or isinstance(source_order, bool) or source_order < 0 or source_order >= len(expected_components) or expected_component_ids[source_order] != component_id:
            raise SameInputBlocked(f"component_mappings[{index}] source_order is not the closed source order")
        if mapping.get("source_role") != component_roles[component_id] or mapping.get("source_primitive") != component_primitives[component_id] or mapping.get("source_material_id") != component_materials[component_id]:
            raise SameInputBlocked(f"component_mappings[{index}] source semantics drifted from ObjectSculptSpec")
        target_part_ids = stable_id_list(mapping.get("target_part_ids"), f"component_mappings[{index}].target_part_ids")
        target_material = mapping.get("target_material_zone_id")
        if target_material is not None and (not isinstance(target_material, str) or STABLE_ID_PATTERN.fullmatch(target_material) is None):
            raise SameInputBlocked(f"component_mappings[{index}].target_material_zone_id is invalid")
        status = mapping.get("status")
        projection = mapping.get("projection")
        if status not in ("MAPPED", "UNSUPPORTED") or projection not in ("exact", "lossy", "unsupported"):
            raise SameInputBlocked(f"component_mappings[{index}] status/projection is outside the closed vocabulary")
        if status == "MAPPED":
            if not target_part_ids or target_material is None or projection == "unsupported":
                raise SameInputBlocked(f"component_mappings[{index}] is marked MAPPED without a target")
            if projection != "exact":
                component_mapping_exact = False
        else:
            if target_part_ids or target_material is not None or projection != "unsupported":
                raise SameInputBlocked(f"component_mappings[{index}] is marked UNSUPPORTED with a target")
            component_mapping_exact = False
        if isinstance(mapping.get("reason"), str) and len(mapping["reason"]) > 240:
            raise SameInputBlocked(f"component_mappings[{index}].reason is unbounded")
        if "reason" in mapping and mapping.get("reason") is not None and not isinstance(mapping.get("reason"), str):
            raise SameInputBlocked(f"component_mappings[{index}].reason is not text")
    if mapped_component_mapping_ids != set(imported_components):
        raise SameInputBlocked("component_mappings do not cover exactly imported_component_ids")

    material_mappings = receipt.get("material_mappings")
    if not isinstance(material_mappings, list) or len(material_mappings) != len(imported_materials):
        raise SameInputBlocked("material_mappings is not a one-to-one mapping for imported materials")
    mapped_material_mapping_ids: set[str] = set()
    material_mapping_exact = True
    for index, mapping in enumerate(material_mappings):
        if not isinstance(mapping, dict):
            raise SameInputBlocked(f"material_mappings[{index}] is not an object")
        material_id = mapping.get("source_material_id")
        if not isinstance(material_id, str) or material_id not in material_set or material_id in mapped_material_mapping_ids:
            raise SameInputBlocked(f"material_mappings[{index}] has an unknown or duplicate source material ID")
        mapped_material_mapping_ids.add(material_id)
        source_order = mapping.get("source_order")
        if not isinstance(source_order, int) or isinstance(source_order, bool) or source_order < 0 or source_order >= len(expected_materials) or expected_material_ids[source_order] != material_id:
            raise SameInputBlocked(f"material_mappings[{index}] source_order is not the closed source order")
        target_material = mapping.get("target_material_zone_id")
        if target_material is not None and (not isinstance(target_material, str) or STABLE_ID_PATTERN.fullmatch(target_material) is None):
            raise SameInputBlocked(f"material_mappings[{index}].target_material_zone_id is invalid")
        status = mapping.get("status")
        projection = mapping.get("projection")
        if status not in ("MAPPED", "UNSUPPORTED") or projection not in ("exact", "lossy", "unsupported"):
            raise SameInputBlocked(f"material_mappings[{index}] status/projection is outside the closed vocabulary")
        if status == "MAPPED":
            if target_material != material_id or projection == "unsupported":
                raise SameInputBlocked(f"material_mappings[{index}] is marked MAPPED without an exact target zone")
            if projection != "exact":
                material_mapping_exact = False
        else:
            if target_material is not None or projection != "unsupported":
                raise SameInputBlocked(f"material_mappings[{index}] is marked UNSUPPORTED with a target")
            material_mapping_exact = False
        if isinstance(mapping.get("reason"), str) and len(mapping["reason"]) > 240:
            raise SameInputBlocked(f"material_mappings[{index}].reason is unbounded")
        if "reason" in mapping and mapping.get("reason") is not None and not isinstance(mapping.get("reason"), str):
            raise SameInputBlocked(f"material_mappings[{index}].reason is not text")
    if mapped_material_mapping_ids != set(imported_materials):
        raise SameInputBlocked("material_mappings do not cover exactly imported_material_ids")

    full_status = receipt.get("full_assembly_status")
    if full_status not in ("COMPILED", "BLOCKED_UNSUPPORTED_COMPONENTS"):
        raise SameInputBlocked("full_assembly_status is outside the closed vocabulary")
    blocked_by = receipt.get("full_assembly_blocked_by")
    if not isinstance(blocked_by, list) or len(set(blocked_by)) != len(blocked_by) or any(
        not isinstance(item, str) or re.fullmatch(r"(?:component|material):[a-zA-Z][a-zA-Z0-9_.-]{0,63}", item) is None
        for item in blocked_by
    ):
        raise SameInputBlocked("full_assembly_blocked_by is not a stable bounded list")
    if full_status == "COMPILED" and blocked_by:
        raise SameInputBlocked("COMPILED import contains full_assembly_blocked_by entries")
    component_parity = (
        set(imported_components) == component_set
        and set(mapped_components) == component_set
        and set(preserved_components) == component_set
        and not unsupported_components
        and not ignored_components
        and component_mapping_exact
    )
    material_parity = (
        set(imported_materials) == material_set
        and set(mapped_materials) == material_set
        and set(preserved_materials) == material_set
        and not unsupported_materials
        and material_mapping_exact
    )
    return {
        "imported_component_ids": imported_components,
        "mapped_component_ids": mapped_components,
        "preserved_component_ids": preserved_components,
        "unsupported_component_ids": unsupported_components,
        "ignored_component_ids": ignored_components,
        "component_mappings": component_mappings,
        "imported_material_ids": imported_materials,
        "mapped_material_ids": mapped_materials,
        "preserved_material_ids": preserved_materials,
        "unsupported_material_ids": unsupported_materials,
        "material_mappings": material_mappings,
        "full_assembly_status": full_status,
        "full_assembly_blocked_by": blocked_by,
        "component_id_parity": component_parity,
        "material_id_parity": material_parity,
        "full_parity": component_parity and material_parity and full_status == "COMPILED" and not blocked_by,
    }


def load_and_validate_inputs(
    spec_path: Path,
    contract_path: Path,
    baseline_receipt_path: Path,
) -> tuple[dict[str, Any], bytes, dict[str, Any], bytes, dict[str, Any], bytes]:
    spec, spec_bytes = load_contract_json(spec_path, "closed ObjectSculptSpec fixture")
    contract, contract_bytes = load_contract_json(contract_path, "normalization contract")
    baseline, baseline_bytes = load_contract_json(baseline_receipt_path, "pinned baseline receipt")
    validate_closed_inputs(contract, baseline, spec, spec_path)

    source = contract.get("source")
    baseline_source = baseline.get("source")
    if not isinstance(source, dict) or not isinstance(baseline_source, dict):
        raise SameInputBlocked("same-input contract has no source binding")
    if source.get("revision") != REVISION or source.get("tree") != EXPECTED_TREE:
        raise SameInputBlocked("normalization contract is not bound to the pinned upstream revision/tree")
    if baseline_source.get("revision") != REVISION or baseline_source.get("tree") != EXPECTED_TREE:
        raise SameInputBlocked("baseline receipt is not bound to the pinned upstream revision/tree")
    spec_sha = sha256_bytes(spec_bytes)
    contract_factory_sha = bounded_sha(source.get("factory_sha256"), "normalization contract factory_sha256")
    baseline_factory_sha = bounded_sha(baseline.get("generation", {}).get("factory_sha256"), "baseline factory_sha256")
    if contract_factory_sha != baseline_factory_sha:
        raise SameInputBlocked("normalization contract and pinned structural receipt disagree on generated factory hash")
    if source.get("fixture_spec_sha256") != spec_sha:
        raise SameInputBlocked("normalization contract fixture hash does not match the exact fixture bytes")
    rig = contract.get("fixed_view_rig")
    if not isinstance(rig, dict):
        raise SameInputBlocked("normalization contract has no fixed_view_rig")
    if rig.get("schema_version") != "KnifeFixedEightViewRig@1" or rig.get("rig_id") != "knife-fixed-eight-view@1":
        raise SameInputBlocked("normalization contract fixed rig schema is not closed")
    if [view.get("view_id") for view in rig.get("views", [])] != EXPECTED_VIEW_IDS:
        raise SameInputBlocked("normalization contract fixed view order is not closed")
    if rig.get("frame_width") != 256 or rig.get("frame_height") != 256 or rig.get("margin") != 0.08:
        raise SameInputBlocked("normalization contract fixed frame or margin drifted")
    aov_contract = contract.get("aov_contract")
    if not isinstance(aov_contract, dict) or aov_contract.get("required") != REQUIRED_AOV_IDS:
        raise SameInputBlocked("normalization contract required AOV order is not closed")
    if contract.get("scope", {}).get("network_allowed") is not False:
        raise SameInputBlocked("same-input benchmark contract allows network access")
    if contract.get("scope", {}).get("quality_claim") != "NOT_COMPUTED":
        raise SameInputBlocked("same-input benchmark contract crosses the quality boundary")
    components = spec.get("componentTree")
    materials = spec.get("materials")
    if not isinstance(components, list) or not components or not isinstance(materials, list) or not materials:
        raise SameInputBlocked("same-input ObjectSculptSpec has no closed component/material arrays")
    component_ids = [component.get("id") for component in components if isinstance(component, dict)]
    material_ids = [material.get("id") for material in materials if isinstance(material, dict)]
    if len(component_ids) != len(components) or len(material_ids) != len(materials):
        raise SameInputBlocked("same-input ObjectSculptSpec contains malformed component/material entries")
    if any(not isinstance(identifier, str) or not identifier for identifier in component_ids) or len(set(component_ids)) != len(component_ids):
        raise SameInputBlocked("ObjectSculptSpec component IDs are not stable and unique")
    if any(not isinstance(identifier, str) or not identifier for identifier in material_ids) or len(set(material_ids)) != len(material_ids):
        raise SameInputBlocked("ObjectSculptSpec material IDs are not stable and unique")
    return spec, spec_bytes, contract, contract_bytes, baseline, baseline_bytes


def validate_capture_result(
    payload: dict[str, Any],
    spec: dict[str, Any],
    spec_sha: str,
    contract_sha: str,
    factory_sha: str,
    source_info: dict[str, Any],
) -> None:
    if payload.get("schema_version") != "WeaponryThreeJsSameInputCapture@1":
        raise SameInputBlocked("browser page returned an unsupported same-input capture schema")
    if payload.get("status") != "PASS_SAME_INPUT_BROWSER_CAPTURE":
        raise SameInputBlocked(f"browser page reported {payload.get('status')}: {payload.get('error', 'unknown error')}")
    if payload.get("quality_status") != "NOT_RUN" or payload.get("visual_superiority") != "NOT_PROVEN" or payload.get("network_used") is not False:
        raise SameInputBlocked("browser capture crossed the quality or network boundary")

    input_payload = payload.get("input")
    if not isinstance(input_payload, dict):
        raise SameInputBlocked("browser capture has no input binding")
    if input_payload.get("source_spec_sha256") != spec_sha:
        raise SameInputBlocked("browser capture source spec hash does not match the exact fixture bytes")
    if input_payload.get("schema_version") != spec.get("schemaVersion") or input_payload.get("target_name") != spec.get("targetName"):
        raise SameInputBlocked("browser capture target/schema does not match the ObjectSculptSpec")
    expected_component_ids = [component["id"] for component in spec["componentTree"]]
    expected_material_ids = [material["id"] for material in spec["materials"]]
    if input_payload.get("component_ids") != expected_component_ids or input_payload.get("material_ids") != expected_material_ids:
        raise SameInputBlocked("browser capture component/material ID set does not match the ObjectSculptSpec")
    import_receipt = input_payload.get("compatibility_import_receipt")
    if not isinstance(import_receipt, dict):
        raise SameInputBlocked("browser capture has no compatibility import receipt")
    import_evidence = validate_import_receipt(import_receipt, spec, source_info)

    normalization = payload.get("normalization")
    if not isinstance(normalization, dict) or normalization.get("contract_sha256") != contract_sha or normalization.get("same_contract") is not True:
        raise SameInputBlocked("browser capture normalization contract binding is not exact")
    if normalization.get("contract_id") != "weaponry-threejs-upstream-normalization@1" or normalization.get("target_max_extent") != 2.2:
        raise SameInputBlocked("browser capture normalization contract identity drifted")
    for label in ("baseline", "compatibility_import"):
        summary = normalization.get(label)
        validate_normalization_summary(summary, label)

    rig = payload.get("rig")
    if not isinstance(rig, dict) or rig.get("rig_id") != "knife-fixed-eight-view@1" or rig.get("rig_fingerprint") != FIXED_RIG_FINGERPRINT:
        raise SameInputBlocked("browser capture rig is not the fixed eight-view rig")
    if rig.get("view_ids") != EXPECTED_VIEW_IDS or rig.get("frame_width") != 256 or rig.get("frame_height") != 256 or rig.get("margin") != 0.08:
        raise SameInputBlocked("browser capture rig dimensions/order are not closed")
    if rig.get("same_rig") is not True or rig.get("camera_bindings_equal") is not True:
        raise SameInputBlocked("browser capture did not bind both outputs to the same camera matrices")
    camera_bindings = rig.get("camera_bindings")
    if not isinstance(camera_bindings, list) or len(camera_bindings) != 8:
        raise SameInputBlocked("browser capture has an incomplete camera binding receipt")
    for index, camera in enumerate(camera_bindings):
        if not isinstance(camera, dict) or camera.get("view_id") != EXPECTED_VIEW_IDS[index]:
            raise SameInputBlocked("browser capture camera binding order is not closed")
        if len(camera.get("matrix_world", [])) != 16 or len(camera.get("matrix_world_inverse", [])) != 16 or len(camera.get("projection_matrix", [])) != 16:
            raise SameInputBlocked(f"camera {EXPECTED_VIEW_IDS[index]} does not carry three 4x4 matrices")
        bounded_fingerprint(camera.get("camera_fingerprint"), f"camera {EXPECTED_VIEW_IDS[index]} fingerprint")
        for matrix_name in ("matrix_world", "matrix_world_inverse", "projection_matrix"):
            matrix = camera.get(matrix_name)
            if any(not isinstance(value, (int, float)) or not is_finite(value) for value in matrix):
                raise SameInputBlocked(f"camera {EXPECTED_VIEW_IDS[index]} {matrix_name} contains a non-finite value")

    cohort = payload.get("renderer_cohort")
    if not isinstance(cohort, dict) or cohort.get("renderer") != "browser-webgl@1" or cohort.get("capture_mode") != "browser-canvas-to-png@1" or cohort.get("same_renderer_instance") is not True or cohort.get("aov_ids") != REQUIRED_AOV_IDS or cohort.get("capture_count") != 2 or cohort.get("external_network_used") is not False:
        raise SameInputBlocked("browser capture renderer/AOV cohort is not closed")

    captures = payload.get("captures")
    if not isinstance(captures, list) or len(captures) != 2:
        raise SameInputBlocked("browser capture did not return exactly two capture summaries")
    names = [capture.get("capture_name") for capture in captures if isinstance(capture, dict)]
    if names != ["pinned-img2threejs-baseline", "weaponry-compatibility-import"]:
        raise SameInputBlocked("browser capture cohort names are not closed")
    for capture in captures:
        validate_capture_summary(capture)

    structure = payload.get("structure")
    if not isinstance(structure, dict):
        raise SameInputBlocked("browser capture has no structural comparison")
    if structure.get("same_input_spec") is not True or structure.get("comparable_capture_cohort") is not True:
        raise SameInputBlocked("structural comparison is not bound to the same input/cohort")
    if structure.get("classification") not in ("STRUCTURAL_PARITY", "NOT_PROVEN"):
        raise SameInputBlocked("structural comparison emitted an unsupported classification")
    if structure.get("baseline_component_count") != len(expected_component_ids) or structure.get("baseline_renderable_part_count") != len(expected_component_ids):
        raise SameInputBlocked("baseline structural count is not bound to the fixture")
    if structure.get("compatibility_imported_component_count") != len(import_evidence["imported_component_ids"]):
        raise SameInputBlocked("compatibility import structural count disagrees with imported_component_ids")
    for field in ("baseline_part_ids", "compatibility_part_ids", "common_part_ids", "missing_from_compatibility_import"):
        values = structure.get(field)
        stable_id_list(values, f"structural comparison {field}")
    if structure.get("source_component_ids") != expected_component_ids or structure.get("source_material_ids") != expected_material_ids:
        raise SameInputBlocked("structural comparison source ID order is not bound to the fixture")
    for field in (
        "imported_component_ids",
        "mapped_component_ids",
        "preserved_component_ids",
        "unsupported_component_ids",
        "ignored_component_ids",
        "imported_material_ids",
        "mapped_material_ids",
        "preserved_material_ids",
        "unsupported_material_ids",
    ):
        expected = import_evidence[field]
        actual = stable_id_list(structure.get(field), f"structural comparison {field}")
        if actual != expected:
            raise SameInputBlocked(f"structural comparison {field} disagrees with the importer receipt")
    if structure.get("component_mappings") != import_evidence["component_mappings"] or structure.get("material_mappings") != import_evidence["material_mappings"]:
        raise SameInputBlocked("structural comparison mappings disagree with the importer receipt")
    if structure.get("full_assembly_status") != import_evidence["full_assembly_status"] or structure.get("full_assembly_blocked_by") != import_evidence["full_assembly_blocked_by"]:
        raise SameInputBlocked("structural comparison full-assembly status disagrees with the importer receipt")
    if structure.get("component_id_parity") is not import_evidence["component_id_parity"] or structure.get("material_id_parity") is not import_evidence["material_id_parity"]:
        raise SameInputBlocked("structural comparison parity flags disagree with the importer receipt")
    expected_missing = [identifier for identifier in expected_component_ids if identifier not in set(import_evidence["preserved_component_ids"])]
    if structure.get("missing_from_compatibility_import") != expected_missing:
        raise SameInputBlocked("structural comparison missing-component list is not source-order stable")
    expected_preserved = import_evidence["full_parity"]
    if structure.get("all_input_components_preserved_by_compatibility_import") is not expected_preserved:
        raise SameInputBlocked("structural comparison preservation flag is not derived from full importer parity")
    if structure.get("classification") == "STRUCTURAL_PARITY" and not expected_preserved:
        raise SameInputBlocked("structural parity was emitted without exact 7/7 component and 4/4 material parity")
    if structure.get("stable_part_ids") is not True or structure.get("stable_material_ids") is not True:
        raise SameInputBlocked("structural comparison did not prove stable IDs")

    pixel = payload.get("pixel_metrics")
    if not isinstance(pixel, dict) or pixel.get("reference_available") is not False or pixel.get("classification") not in ("METRICALLY_SUPERIOR", "NOT_PROVEN"):
        raise SameInputBlocked("pixel comparison did not use the closed reference-aware classification")
    if pixel.get("classification") == "METRICALLY_SUPERIOR" and pixel.get("reference_available") is not True:
        raise SameInputBlocked("metric superiority was emitted without an authorized reference")
    pairwise = pixel.get("pairwise")
    if not isinstance(pairwise, dict) or set(pairwise) != set(REQUIRED_AOV_IDS):
        raise SameInputBlocked("pixel comparison is missing a required AOV metric")
    for aov_id in REQUIRED_AOV_IDS:
        metric = pairwise[aov_id]
        if not isinstance(metric, dict) or metric.get("pair_count") != 8:
            raise SameInputBlocked(f"pixel metric {aov_id} does not cover all eight views")
        for field in ("mean_absolute_rgba_error", "mean_exact_rgba_fraction", "mean_silhouette_iou", "mean_part_id_exact_fraction", "mean_material_id_exact_fraction"):
            value = metric.get(field)
            if not isinstance(value, (int, float)) or not is_finite(value) or value < 0 or value > 1:
                raise SameInputBlocked(f"pixel metric {aov_id}/{field} is non-finite or outside [0,1]")


def validate_normalization_summary(summary: Any, label: str) -> None:
    if not isinstance(summary, dict):
        raise SameInputBlocked(f"normalization summary {label} is missing")
    for field in ("source_center", "source_size"):
        value = summary.get(field)
        if not isinstance(value, list) or len(value) != 3 or any(not isinstance(item, (int, float)) or not is_finite(item) for item in value):
            raise SameInputBlocked(f"normalization summary {label}/{field} is invalid")
    for bounds_field in ("source_bounds", "normalized_bounds"):
        bounds = summary.get(bounds_field)
        if not isinstance(bounds, dict):
            raise SameInputBlocked(f"normalization summary {label}/{bounds_field} is missing")
        for field in ("min", "max", "center", "size"):
            value = bounds.get(field)
            if not isinstance(value, list) or len(value) != 3 or any(not isinstance(item, (int, float)) or not is_finite(item) for item in value):
                raise SameInputBlocked(f"normalization summary {label}/{bounds_field}/{field} is invalid")
        if not isinstance(bounds.get("max_extent"), (int, float)) or not is_finite(bounds["max_extent"]):
            raise SameInputBlocked(f"normalization summary {label}/{bounds_field}/max_extent is invalid")
    for field in ("uniform_scale", "max_extent_error", "center_error"):
        value = summary.get(field)
        if not isinstance(value, (int, float)) or not is_finite(value) or value < 0:
            raise SameInputBlocked(f"normalization summary {label}/{field} is invalid")
    if summary["max_extent_error"] > 1e-6 or summary["center_error"] > 1e-6:
        raise SameInputBlocked(f"normalization summary {label} did not reach the fixed target center/extent")


def validate_capture_summary(capture: Any) -> None:
    if not isinstance(capture, dict):
        raise SameInputBlocked("capture summary is not an object")
    if capture.get("render_status") != "RENDERED" or capture.get("quality_status") != "RENDERED_NOT_APPROVED" or capture.get("renderer_invoked") is not True:
        raise SameInputBlocked(f"capture {capture.get('capture_name')} did not remain render-only")
    if capture.get("view_count") != 8 or capture.get("aov_count") != 56 or capture.get("png_count") != 56:
        raise SameInputBlocked(f"capture {capture.get('capture_name')} is not complete 8x7")
    bounded_sha(capture.get("manifest_sha256"), f"capture {capture.get('capture_name')} manifest")
    bounded_sha(capture.get("receipt_sha256"), f"capture {capture.get('capture_name')} receipt")
    bounded_fingerprint(capture.get("program_fingerprint"), f"capture {capture.get('capture_name')} program")
    bounded_fingerprint(capture.get("scene_fingerprint"), f"capture {capture.get('capture_name')} scene")
    if not isinstance(capture.get("part_ids"), list) or len(set(capture["part_ids"])) != len(capture["part_ids"]):
        raise SameInputBlocked(f"capture {capture.get('capture_name')} part IDs are not unique")
    if not isinstance(capture.get("material_zone_ids"), list) or len(set(capture["material_zone_ids"])) != len(capture["material_zone_ids"]):
        raise SameInputBlocked(f"capture {capture.get('capture_name')} material IDs are not unique")
    validate_normalization_summary(capture.get("normalized_scene"), f"capture {capture.get('capture_name')}")


def is_finite(value: int | float) -> bool:
    return value == value and value not in (float("inf"), float("-inf"))


def choose_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def copy_capture_sources(app: Path) -> dict[str, str]:
    source_dir = app / "weaponry-source"
    source_dir.mkdir(parents=True)
    hashes: dict[str, str] = {}
    for name in CAPTURE_SOURCE_FILES:
        source_file = PACKAGE_ROOT / "src" / name
        if not source_file.is_file():
            raise SameInputBlocked(f"required current Weaponry capture source is missing: {name}")
        shutil.copyfile(source_file, source_dir / name)
        hashes[f"src/{name}"] = sha256_file(source_file)
    return hashes


def build_browser_app(
    app: Path,
    generated: Path,
    template_path: Path,
    adapter_path: Path,
    spec_path: Path,
    contract_path: Path,
    factory_sha: str,
    spec_sha: str,
    contract_sha: str,
    node_modules: Path,
) -> dict[str, str]:
    (app / "generated").mkdir(parents=True)
    shutil.copyfile(generated, app / "generated" / "DragonfangLikeBaseline.ts")
    template = template_path.read_text(encoding="utf-8")
    for marker, value in (
        ("__FACTORY_SHA256__", factory_sha),
        ("__SPEC_SHA256__", spec_sha),
        ("__CONTRACT_SHA256__", contract_sha),
    ):
        template = template.replace(marker, value)
    if "__FACTORY_SHA256__" in template or "__SPEC_SHA256__" in template or "__CONTRACT_SHA256__" in template:
        raise SameInputBlocked("same-input entry template still contains an unbound hash marker")
    (app / "same-input-entry.ts").write_text(template, encoding="utf-8")
    shutil.copyfile(adapter_path, app / "img2threejs-compiled-scene-adapter.ts")
    shutil.copyfile(spec_path, app / spec_path.name)
    shutil.copyfile(contract_path, app / contract_path.name)
    source_hashes = copy_capture_sources(app)
    (app / "index.html").write_text(
        "<!doctype html>\n"
        "<html><head><meta charset=\"utf-8\"><title>Weaponry same-input benchmark</title></head>\n"
        "<body><div id=\"app\"></div><script type=\"module\">\n"
        "import { runSameInputCapture } from '/same-input-entry.ts';\n"
        "try {\n"
        "  const canvas = document.createElement('canvas');\n"
        "  document.getElementById('app').appendChild(canvas);\n"
        "  const renderer = new (await import('three')).WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });\n"
        "  renderer.setPixelRatio(1);\n"
        "  renderer.setSize(256, 256, false);\n"
        "  globalThis.__WPN_SAME_INPUT_RESULT__ = await runSameInputCapture(renderer);\n"
        "  document.documentElement.dataset.weaponrySameInputStatus = 'PASS_SAME_INPUT_BROWSER_CAPTURE';\n"
        "  renderer.dispose();\n"
        "} catch (error) {\n"
        "  globalThis.__WPN_SAME_INPUT_ERROR__ = String(error instanceof Error ? error.message : error);\n"
        "  document.documentElement.dataset.weaponrySameInputStatus = 'BLOCKED_SAME_INPUT_BROWSER';\n"
        "}\n"
        "</script></body></html>\n",
        encoding="utf-8",
    )
    resolved_modules = node_modules.resolve()
    if not (resolved_modules / "three" / "package.json").is_file():
        raise SameInputBlocked(f"existing Three.js dependency is unavailable at {resolved_modules}")
    if not (resolved_modules / ".bin" / "vite").is_file():
        raise SameInputBlocked(f"existing Vite CLI is unavailable at {resolved_modules / '.bin' / 'vite'}")
    (app / "node_modules").symlink_to(resolved_modules, target_is_directory=True)
    return source_hashes


def run_same_input_benchmark(
    source_path: Path,
    spec_path: Path,
    contract_path: Path,
    baseline_receipt_path: Path,
    template_path: Path,
    adapter_path: Path,
    node_modules: Path,
    playwright_cli: Path,
) -> dict[str, Any]:
    source_info = verify_pinned_source(source_path)
    spec, spec_bytes, contract, contract_bytes, baseline, _baseline_bytes = load_and_validate_inputs(
        spec_path, contract_path, baseline_receipt_path
    )
    spec_sha = sha256_bytes(spec_bytes)
    contract_sha = sha256_bytes(contract_bytes)
    factory_sha = bounded_sha(contract["source"]["factory_sha256"], "contract factory hash")
    if not shutil.which("npx"):
        raise SameInputBlocked("npx is unavailable; Playwright CLI prerequisite is not satisfied")
    template_bytes = template_path.read_bytes()
    if b"runSameInputCapture" not in template_bytes or b"__FACTORY_SHA256__" not in template_bytes:
        raise SameInputBlocked("same-input capture template is missing the closed entry or hash marker")
    adapter_bytes = adapter_path.read_bytes()
    vite_binary = node_modules.expanduser().resolve() / ".bin" / "vite"
    baseline_generation = baseline.get("generation", {})

    with tempfile.TemporaryDirectory(prefix="weaponry-three-same-input-") as temporary:
        isolated = Path(temporary)
        pinned = isolated / "source"
        pinned.mkdir()
        extract_pinned_source(source_path, pinned)
        isolated_spec = isolated / "input" / spec_path.name
        isolated_spec.parent.mkdir()
        isolated_spec.write_bytes(spec_bytes)
        validation = run_checked(
            [sys.executable, str(pinned / "forge/stage2_spec/validate_sculpt_spec.py"), str(isolated_spec), "--json"],
            cwd=isolated,
            label="pinned ObjectSculptSpec validator",
        )
        validation_payload = parse_json_output(validation.stdout, "pinned validator")
        if validation_payload.get("ok") is not True:
            raise SameInputBlocked("pinned validator rejected the same-input fixture")
        generated = isolated / "output" / "DragonfangLikeBaseline.ts"
        generation = run_checked(
            [
                sys.executable,
                str(pinned / "forge/stage3_build/generate_threejs_factory.py"),
                str(isolated_spec),
                "--out",
                str(generated),
                "--allow-nonstrict",
            ],
            cwd=isolated,
            label="pinned img2threejs generator",
        )
        if "non-production test-fixture" not in generation.stderr:
            raise SameInputBlocked("pinned generator did not report its bounded fixture-only mode")
        generated_sha = sha256_file(generated)
        if generated_sha != factory_sha or generated.stat().st_size != baseline_generation.get("factory_bytes"):
            raise SameInputBlocked("same-input generated factory does not match the frozen structural baseline")

        app = isolated / "app"
        source_hashes = build_browser_app(
            app,
            generated,
            template_path,
            adapter_path,
            spec_path,
            contract_path,
            generated_sha,
            spec_sha,
            contract_sha,
            node_modules,
        )
        dist = app / "dist"
        vite_version = run_checked([str(vite_binary), "--version"], cwd=app, label="Vite availability probe").stdout.strip()
        run_checked([str(vite_binary), "build", "--outDir", str(dist)], cwd=app, label="same-input offline browser bundle build")
        bundle = bundle_inventory(dist)

        server: subprocess.Popen[bytes] | None = None
        session = f"wpn-three-same-input-{os.getpid()}"
        capture_payload: dict[str, Any] | None = None
        try:
            server, port = start_static_server(dist)
            base_url = f"http://127.0.0.1:{port}/index.html"
            run_playwright(playwright_cli, session, "open", base_url, cwd=app, label="Playwright same-input browser open")
            run_playwright(
                playwright_cli,
                session,
                "run-code",
                "async (page) => { await page.waitForFunction(() => globalThis.__WPN_SAME_INPUT_RESULT__ || globalThis.__WPN_SAME_INPUT_ERROR__, { timeout: 60000 }); }",
                cwd=app,
                label="Playwright same-input capture wait",
            )
            value = run_playwright(
                playwright_cli,
                session,
                "eval",
                "() => globalThis.__WPN_SAME_INPUT_RESULT__ || { status: 'BLOCKED_SAME_INPUT_BROWSER', error: globalThis.__WPN_SAME_INPUT_ERROR__ || 'missing same-input result' }",
                cwd=app,
                label="Playwright same-input result readback",
            )
            parsed = parse_playwright_value(value.stdout, "Playwright same-input result")
            if not isinstance(parsed, dict):
                raise SameInputBlocked("Playwright same-input result is not a JSON object")
            capture_payload = parsed
            validate_capture_result(capture_payload, spec, spec_sha, contract_sha, generated_sha, source_info)
        finally:
            try:
                run_playwright(playwright_cli, session, "close", cwd=app, label="Playwright same-input browser close")
            except BrowserBlocked:
                pass
            if server is not None:
                server.terminate()
                try:
                    server.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    server.kill()
        if capture_payload is None:
            raise SameInputBlocked("browser capture produced no payload")

        rig = capture_payload["rig"]
        cohort_input = {
            "spec_sha256": spec_sha,
            "contract_sha256": contract_sha,
            "factory_sha256": generated_sha,
            "rig_id": rig["rig_id"],
            "rig_fingerprint": rig["rig_fingerprint"],
            "frame_width": rig["frame_width"],
            "frame_height": rig["frame_height"],
            "margin": rig["margin"],
            "renderer": capture_payload["renderer_cohort"]["renderer"],
            "capture_mode": capture_payload["renderer_cohort"]["capture_mode"],
            "aov_ids": capture_payload["renderer_cohort"]["aov_ids"],
            "view_ids": rig["view_ids"],
        }
        cohort_sha = sha256_bytes(canonical_json(cohort_input).encode("utf-8"))
        return {
            "source": source_info,
            "input": {
                "spec_path": str(spec_path.relative_to(PACKAGE_ROOT)),
                "spec_sha256": spec_sha,
                "schema_version": spec.get("schemaVersion"),
                "target_name": spec.get("targetName"),
                "component_ids": [component["id"] for component in spec["componentTree"]],
                "material_ids": [material["id"] for material in spec["materials"]],
                "component_count": len(spec["componentTree"]),
                "material_count": len(spec["materials"]),
                "browser_bound_spec_sha256": capture_payload["input"]["source_spec_sha256"],
                "compatibility_import_receipt": capture_payload["input"]["compatibility_import_receipt"],
            },
            "normalization": {
                "contract_path": str(contract_path.relative_to(PACKAGE_ROOT)),
                "contract_sha256": contract_sha,
                "contract_id": contract.get("contract_id"),
                "contract_schema_version": contract.get("schema_version"),
                "same_contract": capture_payload["normalization"]["same_contract"],
                "target_max_extent": capture_payload["normalization"]["target_max_extent"],
                "formula": capture_payload["normalization"]["formula"],
                "baseline": capture_payload["normalization"]["baseline"],
                "compatibility_import": capture_payload["normalization"]["compatibility_import"],
            },
            "rig": rig,
            "cohort": {
                "id": f"weaponry-threejs-same-input@1:{cohort_sha[:32]}",
                "canonical_sha256": cohort_sha,
                **cohort_input,
                "same_renderer_instance": capture_payload["renderer_cohort"]["same_renderer_instance"],
            },
            "renderer_cohort": capture_payload["renderer_cohort"],
            "captures": capture_payload["captures"],
            "structure": capture_payload["structure"],
            "pixel_metrics": capture_payload["pixel_metrics"],
            "capture_source": {
                "route": "existing-captureKnifeAovs@1",
                "entry_template": str(template_path.relative_to(PACKAGE_ROOT)),
                "adapter": str(adapter_path.relative_to(PACKAGE_ROOT)),
                "files": source_hashes,
            },
            "generation": {
                "generator": "forge/stage3_build/generate_threejs_factory.py",
                "mode": "allow-nonstrict-test-fixture",
                "factory_sha256": generated_sha,
                "factory_bytes": generated.stat().st_size,
                "factory_persisted": False,
                "baseline_receipt": str(baseline_receipt_path.relative_to(PACKAGE_ROOT)),
            },
            "browser": {
                "engine": "Playwright Chromium",
                "vite_version": vite_version,
                "static_network_policy": "local-static-bundle-only@1",
                "external_network_used": False,
                "dependencies_installed": False,
                "bundle": bundle,
                "temporary_artifacts_persisted": False,
            },
        }


def bundle_inventory(dist: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    files: list[dict[str, Any]] = []
    paths = sorted((candidate for candidate in dist.rglob("*") if candidate.is_file()), key=lambda item: item.relative_to(dist).as_posix())
    for path in paths:
        relative = path.relative_to(dist).as_posix()
        data = path.read_bytes()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(data)
        digest.update(b"\0")
        files.append({"path": relative, "bytes": len(data), "sha256": sha256_bytes(data)})
    if not files:
        raise SameInputBlocked("same-input Vite bundle is empty")
    return {"file_count": len(files), "bytes": sum(item["bytes"] for item in files), "sha256": digest.hexdigest(), "files": files}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-checkout", type=Path, required=True)
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--baseline-receipt", type=Path, default=DEFAULT_BASELINE_RECEIPT)
    parser.add_argument("--template", type=Path, default=DEFAULT_TEMPLATE)
    parser.add_argument("--adapter", type=Path, default=DEFAULT_ADAPTER)
    parser.add_argument("--node-modules", type=Path, default=DEFAULT_NODE_MODULES)
    parser.add_argument("--playwright-cli", type=Path, default=discover_playwright_cli())
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    parser.add_argument("--force", action="store_true", help="overwrite an existing same-input receipt")
    args = parser.parse_args(argv)

    receipt_path = args.receipt.expanduser().resolve()
    if BENCHMARK_ROOT not in receipt_path.parents:
        print("BLOCKED: receipt must remain inside packages/weaponry-threejs/benchmark", file=sys.stderr)
        return 2
    if receipt_path.exists() and not args.force:
        print(f"BLOCKED: receipt already exists: {receipt_path}; use --force to refresh", file=sys.stderr)
        return 2

    try:
        result = run_same_input_benchmark(
            args.source_checkout.expanduser().resolve(),
            args.spec.expanduser().resolve(),
            args.contract.expanduser().resolve(),
            args.baseline_receipt.expanduser().resolve(),
            args.template.expanduser().resolve(),
            args.adapter.expanduser().resolve(),
            args.node_modules.expanduser().resolve(),
            args.playwright_cli.expanduser().resolve(),
        )
    except (BenchmarkBlocked, BrowserBlocked, OSError, ValueError, KeyError) as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 2

    receipt: dict[str, Any] = {
        "schema_version": "WeaponryThreeJsSameInputBenchmarkReceipt@1",
        "task_id": "WPN-THREE-LOSSLESS-BENCH-001",
        "benchmark_only": True,
        "status": "PASS_SAME_INPUT_BROWSER_AOV_CAPTURE",
        "quality_status": "NOT_RUN",
        "visual_superiority": "NOT_PROVEN",
        "classification": {
            "structural": result["structure"]["classification"],
            "metric": result["pixel_metrics"]["classification"],
            "conclusion": "NOT_PROVEN",
            "allowed_values": list(CLASSIFICATIONS),
            "rules": {
                "STRUCTURAL_PARITY": "Only when all source component/material IDs are imported, mapped, and preserved with unique source_order mappings under the same input, normalization, rig, renderer, and AOV cohort; this is structural equivalence, never superiority.",
                "METRICALLY_SUPERIOR": "Only when an authorized reference pixel target and explicit thresholds are bound to the same fixed rig/AOV capture and one output wins the declared metrics.",
                "NOT_PROVEN": "Required structural preservation or authorized reference/threshold evidence is absent; measurements remain non-ranking evidence.",
            },
        },
        "network_used": False,
        "dependencies_installed": False,
        "product_runtime_execution": False,
        "runtime_store_cas_write": False,
        **result,
    }
    receipt["canonical_sha256"] = canonical_sha256(receipt)
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(receipt, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
