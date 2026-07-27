#!/usr/bin/env python3
"""Compile and verify the first C111A robotic-arm golden surface asset.

This is an engineering and visual-development gate.  It proves that one
independent reviewed Recipe lineage compiles to deterministic preview and
production GLBs with the required visible-layer vocabulary.  It deliberately
does not claim target-image similarity or replace the M108B human 4/5 gate.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
from typing import Any, Mapping

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryBoundaryError,
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
RUST_WRAPPER = ROOT / "script" / "with_rust_toolchain.sh"
ROOT_RECIPE_ID = "recipe_c111_arm_golden_surface"
REPORT_SCHEMA = "C111GoldenSurfaceAssetGate@1"
DETAIL_INVENTORY = (
    ROOT
    / "packages"
    / "concept-spec"
    / "fixtures"
    / "c111-golden-surface-robotic-arm-visual-detail-inventory.json"
)
DETAIL_INVENTORY_SCHEMA = "C111GoldenSurfaceVisualDetailInventory@1"
REQUIRED_LAYER_OPERATIONS = {
    "silhouette": ("_base_shell", "_yoke_left", "_yoke_right", "_plinth_guard_array"),
    "joints": (
        "_joint_outer_ring",
        "_joint_inner_bearing",
        "_joint_signal_core",
        "_joint_upper_guard",
        "_joint_lower_guard",
    ),
    "links": (
        "_link_armor_plate",
        "_link_upper_tension_rod",
        "_link_tip_collar",
        "_link_frame_rail_upper",
        "_link_frame_cross_brace",
    ),
    "cables": ("_cable_a", "_cable_b", "_cable_clamp_bridge"),
    "end_effector": (
        "_gripper_wrist_collar",
        "_gripper_palm_armor_a",
        "_gripper_knuckle_a",
        "_gripper_knuckle_c",
        "_gripper_finger_tip_a",
        "_gripper_finger_c_panel",
        "_gripper_finger_tip_c",
    ),
    "surface": ("_link_signal_strip", "_trim_panel", "_plinth_signal_array"),
}
REQUIRED_SURFACE_PROGRAM_IDS = {
    "adorn_c111_base_flowline",
    "adorn_c111_joint_microgrid",
    "adorn_c111_link_groove",
    "adorn_c111_gripper_chevron",
    "adorn_c111_gripper_microgrid",
}
PV002_FIXTURE_SCHEMA = "C111ForgeVisualProgramFixture@1"


def _mapping(value: object, code: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise AssertionError(code)
    return value


def _load_detail_inventory() -> Mapping[str, Any]:
    payload = json.loads(DETAIL_INVENTORY.read_text(encoding="utf-8"))
    inventory = _mapping(payload, "C111_DETAIL_INVENTORY_INVALID")
    if (
        inventory.get("schema_version") != DETAIL_INVENTORY_SCHEMA
        or inventory.get("scope") != "c111b_development_fixture_only"
        or inventory.get("formal_eligible") is not False
        or inventory.get("root_recipe_id") != ROOT_RECIPE_ID
    ):
        raise AssertionError("C111_DETAIL_INVENTORY_HEADER_INVALID")
    return inventory


def _assert_detail_inventory(
    inventory: Mapping[str, Any],
    payload: Mapping[str, Any],
    candidate: Mapping[str, Any],
    production: Mapping[str, Any],
) -> Mapping[str, Any]:
    if inventory.get("registry_id") != payload.get("registry_id"):
        raise AssertionError("C111_DETAIL_INVENTORY_REGISTRY_INVALID")
    compiled = _mapping(
        inventory.get("compiled_evidence"),
        "C111_DETAIL_INVENTORY_COMPILED_EVIDENCE_INVALID",
    )
    if (
        compiled.get("shape_program_sha256") != payload.get("shape_program_sha256")
        or compiled.get("production_glb_sha256") != production.get("glb_sha256")
        or compiled.get("production_triangles") != production.get("triangle_count")
        or compiled.get("production_primitives") != production.get("primitive_count")
    ):
        raise AssertionError(
            "C111_DETAIL_INVENTORY_LINEAGE_INVALID:"
            + json.dumps(
                {
                    "expected": {
                        "shape_program_sha256": compiled.get("shape_program_sha256"),
                        "production_glb_sha256": compiled.get("production_glb_sha256"),
                        "production_triangles": compiled.get("production_triangles"),
                        "production_primitives": compiled.get("production_primitives"),
                    },
                    "actual": {
                        "shape_program_sha256": payload.get("shape_program_sha256"),
                        "production_glb_sha256": production.get("glb_sha256"),
                        "production_triangles": production.get("triangle_count"),
                        "production_primitives": production.get("primitive_count"),
                    },
                },
                sort_keys=True,
            )
        )
    fixed_views = compiled.get("fixed_views")
    if not isinstance(fixed_views, list):
        raise AssertionError("C111_DETAIL_INVENTORY_FIXED_VIEWS_INVALID")
    view_ids = {
        str(view.get("view_id"))
        for view in fixed_views
        if isinstance(view, Mapping)
        and re.fullmatch(r"[0-9a-f]{64}", str(view.get("sha256", "")))
    }
    required_views = {
        "iso",
        "front",
        "back",
        "left",
        "right",
        "top",
        "gripper_iso",
        "gripper_front",
    }
    if view_ids != required_views:
        raise AssertionError("C111_DETAIL_INVENTORY_FIXED_VIEWS_INCOMPLETE")

    graph = _mapping(
        candidate.get("expanded_assembly_graph"),
        "C111_DETAIL_INVENTORY_GRAPH_INVALID",
    )
    program = _mapping(
        candidate.get("expanded_shape_program"),
        "C111_DETAIL_INVENTORY_PROGRAM_INVALID",
    )
    recipe_instances = candidate.get("component_recipe_instances")
    if not isinstance(recipe_instances, list):
        raise AssertionError("C111_DETAIL_INVENTORY_RECIPE_INSTANCES_INVALID")
    recipe_ids = {
        str(recipe_ref.get("recipe_id"))
        for instance in recipe_instances
        if isinstance(instance, Mapping)
        for recipe_ref in [instance.get("recipe")]
        if isinstance(recipe_ref, Mapping)
    }
    operation_ids = {
        str(operation.get("operation_id"))
        for operation in program.get("operations", [])
        if isinstance(operation, Mapping)
    }
    material_zone_ids = {
        str(zone_id)
        for part in graph.get("parts", [])
        if isinstance(part, Mapping)
        for zone_id in part.get("material_zone_ids", [])
        if isinstance(zone_id, str)
    }
    adornment_program_ids = {
        str(adornment.get("program_id"))
        for adornment in payload.get("surface_adornment_programs", [])
        if isinstance(adornment, Mapping)
    }
    adornment_program_zones = {
        str(adornment.get("target_zone_id"))
        for adornment in payload.get("surface_adornment_programs", [])
        if isinstance(adornment, Mapping)
    }
    items = inventory.get("items")
    if not isinstance(items, list):
        raise AssertionError("C111_DETAIL_INVENTORY_ITEMS_INVALID")
    required_counts = _mapping(
        inventory.get("required_counts"),
        "C111_DETAIL_INVENTORY_REQUIRED_COUNTS_INVALID",
    )
    actual_counts = {
        band: sum(
            1
            for item in items
            if isinstance(item, Mapping) and item.get("scale_band") == band
        )
        for band in ("macro", "meso", "micro")
    }
    if actual_counts != {
        band: int(required_counts.get(band, -1))
        for band in ("macro", "meso", "micro")
    }:
        raise AssertionError("C111_DETAIL_INVENTORY_SCALE_COUNTS_INVALID")

    detail_ids: set[str] = set()
    critical_unresolved: set[str] = set()
    status_counts = {
        "planned": 0,
        "lowered": 0,
        "readback_verified": 0,
        "unresolved": 0,
    }
    for item in items:
        detail = _mapping(item, "C111_DETAIL_INVENTORY_ITEM_INVALID")
        detail_id = str(detail.get("detail_id", ""))
        if not detail_id or detail_id in detail_ids:
            raise AssertionError("C111_DETAIL_INVENTORY_DETAIL_ID_INVALID")
        detail_ids.add(detail_id)
        status = str(detail.get("status", ""))
        if status not in status_counts:
            raise AssertionError(f"C111_DETAIL_INVENTORY_STATUS_INVALID:{detail_id}")
        status_counts[status] += 1
        evidence = detail.get("evidence")
        if (
            not isinstance(evidence, list)
            or not evidence
            or any(
                evidence_id != "production_readback" and evidence_id not in view_ids
                for evidence_id in evidence
            )
        ):
            raise AssertionError(f"C111_DETAIL_INVENTORY_EVIDENCE_INVALID:{detail_id}")
        mappings = detail.get("maps_to")
        if not isinstance(mappings, list):
            raise AssertionError(f"C111_DETAIL_INVENTORY_MAPPING_INVALID:{detail_id}")
        if detail.get("importance") == "critical" and not mappings:
            raise AssertionError(
                f"C111_DETAIL_INVENTORY_CRITICAL_MAPPING_MISSING:{detail_id}"
            )
        for raw_mapping in mappings:
            mapping = _mapping(
                raw_mapping,
                f"C111_DETAIL_INVENTORY_MAPPING_INVALID:{detail_id}",
            )
            known_keys = {
                "recipe_id",
                "shape_operation_suffix",
                "material_zone_id",
                "adornment_program_id",
                "surface_program_zone_id",
            }
            mapping_keys = set(mapping)
            if len(mapping_keys) != 1 or not mapping_keys <= known_keys:
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_MAPPING_KIND_INVALID:{detail_id}"
                )
            if "recipe_id" in mapping and mapping["recipe_id"] not in recipe_ids:
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_RECIPE_MISSING:{detail_id}"
                )
            if "shape_operation_suffix" in mapping and not any(
                operation_id.endswith(str(mapping["shape_operation_suffix"]))
                for operation_id in operation_ids
            ):
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_OPERATION_MISSING:{detail_id}"
                )
            if (
                "material_zone_id" in mapping
                and mapping["material_zone_id"] not in material_zone_ids
            ):
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_ZONE_MISSING:{detail_id}"
                )
            if (
                "adornment_program_id" in mapping
                and mapping["adornment_program_id"] not in adornment_program_ids
            ):
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_ADORNMENT_MISSING:{detail_id}"
                )
            if (
                "surface_program_zone_id" in mapping
                and mapping["surface_program_zone_id"] not in adornment_program_zones
            ):
                raise AssertionError(
                    f"C111_DETAIL_INVENTORY_SURFACE_ZONE_MISSING:{detail_id}"
                )
        if detail.get("importance") == "critical" and status == "unresolved":
            critical_unresolved.add(detail_id)

    unresolved = _mapping(
        inventory.get("unresolved_summary"),
        "C111_DETAIL_INVENTORY_UNRESOLVED_SUMMARY_INVALID",
    )
    declared_critical_unresolved = set(unresolved.get("critical_detail_ids", []))
    if (
        critical_unresolved != declared_critical_unresolved
        or unresolved.get("blocks_single_result_display") is not bool(critical_unresolved)
        or unresolved.get("human_benchmark_evidence") is not False
        or unresolved.get("reference_source_unavailable") is not False
    ):
        raise AssertionError("C111_DETAIL_INVENTORY_UNRESOLVED_SUMMARY_MISMATCH")
    reference = _mapping(
        inventory.get("reference_evidence"),
        "C111_DETAIL_INVENTORY_REFERENCE_INVALID",
    )
    if (
        reference.get("status") != "digest_verified"
        or not re.fullmatch(r"[0-9a-f]{64}", str(reference.get("sha256", "")))
        or reference.get("repository_storage") != "external_user_authorized"
    ):
        raise AssertionError("C111_DETAIL_INVENTORY_REFERENCE_STATE_INVALID")
    return {
        "inventory_id": inventory["inventory_id"],
        "inventory_sha256": hashlib.sha256(
            DETAIL_INVENTORY.read_bytes()
        ).hexdigest(),
        "item_count": len(items),
        "scale_counts": actual_counts,
        "status_counts": status_counts,
        "critical_unresolved_detail_ids": sorted(critical_unresolved),
        "blocks_single_result_display": bool(critical_unresolved),
        "reference_source_unavailable": False,
    }


def _rust_dump() -> Mapping[str, Any]:
    result = subprocess.run(
        [
            str(RUST_WRAPPER),
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(MANIFEST),
            "-p",
            "forgecad-core",
            "--bin",
            "c111_golden_surface_recipe_dump",
            "--offline",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=120,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(f"C111_RUST_DUMP_FAILED:{result.stderr[-2000:]}")
    payload = json.loads(result.stdout)
    if payload.get("schema_version") != "C111GoldenSurfaceRecipeExpansion@1":
        raise AssertionError("C111_RUST_DUMP_SCHEMA_INVALID")
    return payload


def _compile(
    executor: RestrictedGeometryExecutor,
    payload: Mapping[str, Any],
    profile_id: str,
) -> tuple[bytes, Mapping[str, Any], str]:
    fixture = _mapping(
        payload.get("forge_visual_program_fixture"),
        "PV002_FORGE_VISUAL_FIXTURE_INVALID",
    )
    lowering = _mapping(fixture.get("lowering"), "PV002_LOWERING_INVALID")
    request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"exec_c111a_{profile_id}",
            "idempotency_key": f"idem_c111a_{profile_id}",
            "cancellation_id": f"cancel_c111a_{profile_id}",
            "cancellation_token": f"token_c111a_{profile_id}",
            "action": "compile_readback",
            "timeout_ms": 120_000,
            "artifact_profile_id": profile_id,
            "shape_program": lowering["shape_program"],
            "shape_program_canonical_json": payload["shape_program_canonical_json"],
            "shape_program_sha256": payload["shape_program_sha256"],
            "surface_adornment_programs": fixture["surface_adornment_programs"],
            "surface_layer_input": payload["surface_layer_input"],
        }
    )
    try:
        result = executor.execute(request)
    except RestrictedGeometryBoundaryError as exc:
        raise AssertionError(
            f"C111_{profile_id.upper()}_COMPILE_REJECTED:{exc.code}"
        ) from None
    if result.glb_base64 is None or result.readback is None:
        raise AssertionError(f"C111_{profile_id.upper()}_RESULT_INVALID")
    glb = base64.b64decode(result.glb_base64, validate=True)
    if hashlib.sha256(glb).hexdigest() != result.glb_sha256:
        raise AssertionError(f"C111_{profile_id.upper()}_GLB_HASH_INVALID")
    if result.artifact_handle is None:
        raise AssertionError(f"C111_{profile_id.upper()}_ARTIFACT_HANDLE_INVALID")
    return glb, result.readback, result.artifact_handle


def _assert_production_fixed_views(
    executor: RestrictedGeometryExecutor,
    *,
    artifact_handle: str,
    shape_program_sha256: str,
    inventory: Mapping[str, Any],
    verify_expected: bool = True,
) -> Mapping[str, str]:
    compiled = _mapping(
        inventory.get("compiled_evidence"),
        "C111_DETAIL_INVENTORY_COMPILED_EVIDENCE_INVALID",
    )
    fixed_views = compiled.get("fixed_views")
    if not isinstance(fixed_views, list):
        raise AssertionError("C111_DETAIL_INVENTORY_FIXED_VIEWS_INVALID")
    expected = {
        str(view.get("view_id")): str(view.get("sha256"))
        for view in fixed_views
        if isinstance(view, Mapping)
    }
    request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": "exec_c111a_production_fixed_views",
            "idempotency_key": "idem_c111a_production_fixed_views",
            "cancellation_id": "cancel_c111a_production_fixed_views",
            "cancellation_token": "token_c111a_production_fixed_views",
            "action": "render",
            "timeout_ms": 120_000,
            "artifact_handle": artifact_handle,
            "shape_program_sha256": shape_program_sha256,
            "render": {
                "width": 640,
                "height": 640,
                "view_profile": "convergence_eight",
                "exploded_parts": [],
            },
        }
    )
    try:
        rendered = executor.execute(request)
    except RestrictedGeometryBoundaryError as exc:
        raise AssertionError(f"C111_PRODUCTION_FIXED_VIEWS_REJECTED:{exc.code}") from None
    actual = rendered.render_view_sha256
    if actual is None:
        raise AssertionError("C111_PRODUCTION_FIXED_VIEWS_RESULT_INVALID")
    if verify_expected and actual != expected:
        raise AssertionError(
            "C111_PRODUCTION_FIXED_VIEWS_HASH_DRIFT:"
            + json.dumps(
                {
                    "actual": actual,
                    "expected": expected,
                },
                sort_keys=True,
            )
        )
    return actual


def _assert_forge_visual_program_fixture(
    payload: Mapping[str, Any],
    candidate: Mapping[str, Any],
    inventory: Mapping[str, Any],
) -> Mapping[str, Any]:
    fixture = _mapping(
        payload.get("forge_visual_program_fixture"),
        "PV002_FORGE_VISUAL_FIXTURE_INVALID",
    )
    program = _mapping(fixture.get("program"), "PV002_FORGE_VISUAL_PROGRAM_INVALID")
    lowering = _mapping(fixture.get("lowering"), "PV002_LOWERING_INVALID")
    if (
        fixture.get("schema_version") != PV002_FIXTURE_SCHEMA
        or fixture.get("registry_id") != payload.get("registry_id")
        or fixture.get("registry_sha256") != payload.get("registry_sha256")
        or fixture.get("inventory_id") != inventory.get("inventory_id")
        or not re.fullmatch(
            r"[0-9a-f]{64}", str(fixture.get("inventory_semantic_sha256", ""))
        )
    ):
        raise AssertionError("PV002_FORGE_VISUAL_FIXTURE_IDENTITY_INVALID")
    if (
        program.get("schema_version") != "ForgeVisualProgram@1"
        or program.get("stage")
        != ("draft" if inventory.get("unresolved_summary", {}).get("critical_detail_ids") else "sealed")
        or program.get("visual_only") is not True
        or program.get("geometry_graph") != candidate.get("expanded_shape_program")
        or program.get("assembly_graph") != candidate.get("expanded_assembly_graph")
        or lowering.get("shape_program") != candidate.get("expanded_shape_program")
        or lowering.get("assembly_graph") != candidate.get("expanded_assembly_graph")
        or lowering.get("shape_program", {}).get("schema_version") != "ShapeProgram@1"
        or lowering.get("source_program_sha256") == payload.get("shape_program_sha256")
        or not re.fullmatch(
            r"[0-9a-f]{64}", str(lowering.get("source_program_sha256", ""))
        )
    ):
        raise AssertionError("PV002_FORGE_VISUAL_PROGRAM_LINEAGE_INVALID")
    parts = program.get("parts")
    materials = program.get("material_graph")
    surfaces = program.get("surface_graph")
    details = program.get("detail_inventory")
    if (
        not isinstance(parts, list)
        or len(parts) != 10
        or not isinstance(materials, list)
        or len(materials) != 48
        or not isinstance(surfaces, list)
        or len(surfaces) != 6
        or not isinstance(details, list)
        or len(details) != 27
    ):
        raise AssertionError("PV002_FORGE_VISUAL_PROGRAM_COUNTS_INVALID")
    surface_layer_input = _mapping(
        payload.get("surface_layer_input"),
        "PV002_SURFACE_LAYER_INPUT_INVALID",
    )
    surface_layer_lowering = _mapping(
        surface_layer_input.get("lowering"),
        "PV002_SURFACE_LAYER_LOWERING_INVALID",
    )
    lowered_program_ids = {
        str(item.get("program_id"))
        for item in surface_layer_lowering.get("adornments", [])
        if isinstance(item, Mapping)
    }
    if {
        str(surface.get("surface_program_id"))
        for surface in surfaces
        if isinstance(surface, Mapping)
    } != REQUIRED_SURFACE_PROGRAM_IDS | lowered_program_ids:
        raise AssertionError("PV002_FORGE_VISUAL_SURFACE_LINEAGE_INVALID")
    bound = [
        detail
        for detail in details
        if isinstance(detail, Mapping) and detail.get("status") == "bound"
    ]
    if not bound or any(not detail.get("bindings") for detail in bound):
        raise AssertionError("PV002_FORGE_VISUAL_BOUND_DETAIL_INVALID")
    critical_unresolved = sorted(
        str(detail.get("detail_id"))
        for detail in details
        if isinstance(detail, Mapping)
        and detail.get("critical") is True
        and detail.get("status") == "unresolved"
    )
    expected_blockers = sorted(
        inventory.get("unresolved_summary", {}).get("critical_detail_ids", [])
    )
    expected_sealed_status = (
        "blocked_critical_details"
        if expected_blockers
        else "sealed_critical_details_complete"
    )
    expected_sealed_error = "FORGE_VISUAL_PROGRAM_INVALID" if expected_blockers else ""
    if (
        critical_unresolved != expected_blockers
        or fixture.get("critical_unresolved_detail_ids") != expected_blockers
        or fixture.get("sealed_status") != expected_sealed_status
        or fixture.get("sealed_error_code") != expected_sealed_error
    ):
        raise AssertionError("PV002_FALSE_SEAL_BARRIER_INVALID")
    if fixture.get("fixed_views") != inventory.get("compiled_evidence", {}).get(
        "fixed_views"
    ):
        raise AssertionError("PV002_FIXED_VIEW_LINEAGE_INVALID")
    return fixture


def _assert_candidate(payload: Mapping[str, Any]) -> Mapping[str, Any]:
    candidate = _mapping(payload.get("candidate"), "C111_CANDIDATE_INVALID")
    recipe = _mapping(candidate.get("recipe"), "C111_RECIPE_INVALID")
    if recipe.get("recipe_id") != ROOT_RECIPE_ID:
        raise AssertionError("C111_ROOT_RECIPE_INVALID")
    graph = _mapping(candidate.get("expanded_assembly_graph"), "C111_GRAPH_INVALID")
    program = _mapping(candidate.get("expanded_shape_program"), "C111_PROGRAM_INVALID")
    parts = graph.get("parts")
    connections = graph.get("connections")
    operations = program.get("operations")
    outputs = program.get("outputs")
    if not isinstance(parts, list) or len(parts) != 10:
        raise AssertionError("C111_PART_COUNT_INVALID")
    if not isinstance(connections, list) or len(connections) != 9:
        raise AssertionError("C111_CONNECTION_COUNT_INVALID")
    if not isinstance(operations, list) or not 120 <= len(operations) <= 220:
        raise AssertionError("C111_OPERATION_BUDGET_INVALID")
    if not isinstance(outputs, list) or not 80 <= len(outputs) <= 110:
        raise AssertionError("C111_OUTPUT_BUDGET_INVALID")
    operation_ids = {
        str(item.get("operation_id"))
        for item in operations
        if isinstance(item, Mapping)
    }
    missing_layers = {
        layer: suffixes
        for layer, suffixes in REQUIRED_LAYER_OPERATIONS.items()
        if any(not any(operation_id.endswith(suffix) for operation_id in operation_ids) for suffix in suffixes)
    }
    if missing_layers:
        raise AssertionError(f"C111_VISIBLE_LAYER_VOCABULARY_MISSING:{sorted(missing_layers)}")
    zones = {
        zone
        for part in parts
        if isinstance(part, Mapping)
        for zone in part.get("material_zone_ids", [])
        if isinstance(zone, str)
    }
    if len(zones) < 8:
        raise AssertionError("C111_MATERIAL_ZONE_SPECTRUM_INVALID")
    return candidate


def _assert_readback(
    readback: Mapping[str, Any],
    *,
    profile_id: str,
    shape_program_sha256: str,
    minimum_triangles: int,
) -> None:
    if (
        readback.get("schema_version") != "GeometryCompileReadback@2"
        or readback.get("shape_program_sha256") != shape_program_sha256
        or readback.get("readback_status") != "passed"
    ):
        raise AssertionError(f"C111_{profile_id.upper()}_LINEAGE_INVALID")
    profile = _mapping(readback.get("artifact_profile"), "C111_PROFILE_MISSING")
    if profile.get("artifact_profile_id") != profile_id:
        raise AssertionError(f"C111_{profile_id.upper()}_PROFILE_INVALID")
    triangles = int(readback.get("triangle_count", 0))
    primitives = int(readback.get("primitive_count", 0))
    if triangles < minimum_triangles or primitives < 150:
        raise AssertionError(
            f"C111_{profile_id.upper()}_GEOMETRY_DETAIL_INVALID:{triangles}:{primitives}"
        )
    if any(int(readback.get(field, 0)) != primitives for field in (
        "uv0_primitive_count",
        "normal_primitive_count",
        "tangent_primitive_count",
    )):
        raise AssertionError(f"C111_{profile_id.upper()}_VERTEX_CHANNEL_INVALID")
    surfaces = readback.get("surface_provenance")
    zones = readback.get("material_zone_faces")
    if not isinstance(surfaces, list) or len(surfaces) != primitives:
        raise AssertionError(f"C111_{profile_id.upper()}_SURFACE_PROVENANCE_INVALID")
    if not isinstance(zones, list) or len(zones) != primitives:
        raise AssertionError(f"C111_{profile_id.upper()}_ZONE_PROVENANCE_INVALID")
    texture_sets = readback.get("visual_texture_sets")
    if not isinstance(texture_sets, list) or len(texture_sets) < 6:
        raise AssertionError(f"C111_{profile_id.upper()}_PBR_SPECTRUM_INVALID")
    expected_extent = 128 if profile_id == "interactive_preview" else 1024
    extents = {
        (int(item["width"]), int(item["height"]))
        for texture_set in texture_sets
        if isinstance(texture_set, Mapping)
        for item in texture_set.get("maps", [])
        if isinstance(item, Mapping)
    }
    if extents != {(expected_extent, expected_extent)}:
        raise AssertionError(f"C111_{profile_id.upper()}_TEXTURE_EXTENT_INVALID:{sorted(extents)}")
    dynamic_program_ids = {
        str(texture_set["surface_adornment"]["program_id"])
        for texture_set in texture_sets
        if isinstance(texture_set, Mapping)
        and isinstance(texture_set.get("surface_adornment"), Mapping)
    }
    if dynamic_program_ids != REQUIRED_SURFACE_PROGRAM_IDS:
        raise AssertionError(
            f"C111_{profile_id.upper()}_SURFACE_PROGRAM_LINEAGE_INVALID:{sorted(dynamic_program_ids)}"
        )
    retained = [
        texture_set
        for texture_set in texture_sets
        if isinstance(texture_set, Mapping)
        and isinstance(texture_set.get("surface_layer_lowering"), Mapping)
    ]
    if len(retained) != 1:
        raise AssertionError(f"C111_{profile_id.upper()}_RETAINED_SURFACE_MISSING")
    retained_maps = retained[0].get("maps")
    if not isinstance(retained_maps, list) or {
        item.get("texture_role")
        for item in retained_maps
        if isinstance(item, Mapping)
    } != {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}:
        raise AssertionError(f"C111_{profile_id.upper()}_RETAINED_PBR_INCOMPLETE")


def _artifact_directory(name: str) -> Path:
    if not re.fullmatch(r"[a-z0-9][a-z0-9_-]{0,63}", name):
        raise AssertionError("C111_ARTIFACT_DIRECTORY_INVALID")
    return ROOT / "output" / name


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-dir")
    parser.add_argument("--print-current-evidence", action="store_true")
    args = parser.parse_args()
    payload = _rust_dump()
    candidate = _assert_candidate(payload)
    detail_inventory = _load_detail_inventory()
    forge_visual_fixture = _assert_forge_visual_program_fixture(
        payload,
        candidate,
        detail_inventory,
    )
    executor = RestrictedGeometryExecutor(environment={})
    preview_glb, preview, _ = _compile(executor, payload, "interactive_preview")
    production_glb, production, production_artifact_handle = _compile(
        executor,
        payload,
        "production_concept",
    )
    shape_hash = str(payload["shape_program_sha256"])
    _assert_readback(
        preview,
        profile_id="interactive_preview",
        shape_program_sha256=shape_hash,
        minimum_triangles=20_000,
    )
    _assert_readback(
        production,
        profile_id="production_concept",
        shape_program_sha256=shape_hash,
        minimum_triangles=100_000,
    )
    if int(production["triangle_count"]) <= int(preview["triangle_count"]):
        raise AssertionError("C111_LOD_DETAIL_ORDER_INVALID")
    if args.print_current_evidence:
        fixed_views = _assert_production_fixed_views(
            executor,
            artifact_handle=production_artifact_handle,
            shape_program_sha256=shape_hash,
            inventory=detail_inventory,
            verify_expected=False,
        )
        print(
            json.dumps(
                {
                    "shape_program_sha256": production["shape_program_sha256"],
                    "production_glb_sha256": production["glb_sha256"],
                    "production_triangles": production["triangle_count"],
                    "production_primitives": production["primitive_count"],
                    "fixed_views": fixed_views,
                },
                ensure_ascii=False,
                sort_keys=True,
            )
        )
        return 0
    expected_production = _mapping(
        forge_visual_fixture.get("expected_production"),
        "PV002_EXPECTED_PRODUCTION_INVALID",
    )
    if any(
        expected_production.get(expected) != production.get(actual)
        for expected, actual in (
            ("shape_program_sha256", "shape_program_sha256"),
            ("glb_sha256", "glb_sha256"),
            ("triangle_count", "triangle_count"),
            ("primitive_count", "primitive_count"),
        )
    ):
        raise AssertionError("PV002_PRODUCTION_READBACK_LINEAGE_INVALID")
    detail_inventory_summary = _assert_detail_inventory(
        detail_inventory,
        payload,
        candidate,
        production,
    )
    _assert_production_fixed_views(
        executor,
        artifact_handle=production_artifact_handle,
        shape_program_sha256=shape_hash,
        inventory=detail_inventory,
    )
    destination = None
    if args.artifact_dir:
        destination = _artifact_directory(args.artifact_dir)
        if destination.exists():
            raise AssertionError("C111_ARTIFACT_DIRECTORY_EXISTS")
        destination.mkdir(parents=True)
        try:
            (destination / "robotic-arm-golden-surface-preview.glb").write_bytes(preview_glb)
            (destination / "robotic-arm-golden-surface-production.glb").write_bytes(production_glb)
            summary = {
                "schema_version": REPORT_SCHEMA,
                "status": "pass",
                "formal_eligible": False,
                "human_benchmark_evidence": False,
                "provider_calls": 0,
                "root_recipe_id": ROOT_RECIPE_ID,
                "registry_id": payload["registry_id"],
                "registry_sha256": payload["registry_sha256"],
                "shape_program_sha256": shape_hash,
                "forge_visual_program_sha256": forge_visual_fixture["lowering"][
                    "source_program_sha256"
                ],
                "forge_visual_program_stage": forge_visual_fixture["program"]["stage"],
                "sealed_status": forge_visual_fixture["sealed_status"],
                "parts": len(candidate["expanded_assembly_graph"]["parts"]),
                "connections": len(candidate["expanded_assembly_graph"]["connections"]),
                "operations": len(candidate["expanded_shape_program"]["operations"]),
                "outputs": len(candidate["expanded_shape_program"]["outputs"]),
                "surface_adornment_manifest": payload["surface_adornment_manifest"],
                "surface_adornment_program_ids": sorted(REQUIRED_SURFACE_PROGRAM_IDS),
                "visual_detail_inventory": detail_inventory_summary,
                "preview": {
                    "file": "robotic-arm-golden-surface-preview.glb",
                    "glb_sha256": preview["glb_sha256"],
                    "triangle_count": preview["triangle_count"],
                    "primitive_count": preview["primitive_count"],
                },
                "production": {
                    "file": "robotic-arm-golden-surface-production.glb",
                    "glb_sha256": production["glb_sha256"],
                    "triangle_count": production["triangle_count"],
                    "primitive_count": production["primitive_count"],
                },
            }
            (destination / "readback-summary.json").write_text(
                json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
        except BaseException:
            shutil.rmtree(destination, ignore_errors=True)
            raise
    print(
        json.dumps(
            {
                "schema_version": REPORT_SCHEMA,
                "status": "pass",
                "formal_eligible": False,
                "human_benchmark_evidence": False,
                "provider_calls": 0,
                "root_recipe_id": ROOT_RECIPE_ID,
                "registry_id": payload["registry_id"],
                "shape_program_sha256": shape_hash,
                "forge_visual_program_sha256": forge_visual_fixture["lowering"][
                    "source_program_sha256"
                ],
                "forge_visual_program_stage": forge_visual_fixture["program"]["stage"],
                "sealed_status": forge_visual_fixture["sealed_status"],
                "parts": 10,
                "connections": 9,
                "operations": len(candidate["expanded_shape_program"]["operations"]),
                "outputs": len(candidate["expanded_shape_program"]["outputs"]),
                "surface_adornment_program_ids": sorted(REQUIRED_SURFACE_PROGRAM_IDS),
                "visual_detail_inventory": detail_inventory_summary,
                "preview_triangles": preview["triangle_count"],
                "production_triangles": production["triangle_count"],
                "preview_primitives": preview["primitive_count"],
                "production_primitives": production["primitive_count"],
                "artifact_directory": (
                    str(destination.relative_to(ROOT)) if destination is not None else None
                ),
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
