#!/usr/bin/env python3
"""Fail-closed R007B reference-surface gate for the C106 robotic-arm path.

This gate deliberately composes existing Rust-owned contracts instead of
inventing an image-to-mesh path in Python. The two PNGs are 1×1 byte/CAS
contract fixtures only: their declared notes describe the intended visible
evidence, but no pixel content is analyzed. The third fixture is a strict
readback GLB. Their declared plans target three different reviewed C106 roots.
A reference is never copied into ShapeProgram: the production gate supplies a
separately compiled GLB and this gate proves the two CAS identities differ.

It is engineering evidence only.  It produces neither a visual score nor an
M108B human-review claim.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Mapping
from uuid import uuid4


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
RUST = ROOT / "script" / "with_rust_toolchain.sh"
FIXTURES = ROOT / "packages" / "concept-spec" / "fixtures" / "r007b-robotic-arm-reference-fixtures.json"
SCHEMA_VERSION = "R007BReferenceSurfaceGate@1"
ROOT_IDS = {
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
}
ANALYSIS_SOURCE = "declared_user_notes_and_strict_glb_readback"
FORBIDDEN_CLAIM_KEYS = {
    "visual_analyzer",
    "vision_model",
    "provider",
    "provider_model",
    "pixel_analysis",
    "image_analysis",
}
FORBIDDEN_CLAIM_TEXT = ("visual analyzer", "vision model", "pixel analysis", "provider")
WORKBENCH_STABILITY_RUNS = 3
EVIDENCE_BUNDLE_SCHEMA = "R007BEvidenceBundle@1"
EVIDENCE_ARTIFACT_ROOT = Path(
    os.environ.get(
        "FORGECAD_R007B_ARTIFACT_DIR",
        str(ROOT / "output" / "r007b-reference-surface-gate"),
    )
).resolve()
SHA256_LENGTH = 64


class GateFailure(RuntimeError):
    pass


def fail(code: str) -> None:
    raise GateFailure(code)


def last_json(output: str, schema_version: str) -> dict[str, Any]:
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == schema_version:
            return value
    fail(f"R007B_GATE_REPORT_MISSING:{schema_version}")


def run(
    name: str,
    command: list[str],
    *,
    environment: Mapping[str, str] | None = None,
) -> tuple[dict[str, Any], str]:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=1_200,
            check=False,
            env=dict(environment) if environment is not None else None,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise GateFailure(f"R007B_GATE_{name}_UNAVAILABLE") from exc
    if completed.returncode:
        detail = (completed.stdout + "\n" + completed.stderr).strip()[-4_000:]
        raise GateFailure(f"R007B_GATE_{name}_FAILED:{detail}")
    return {"id": name, "status": "pass", "elapsed_ms": round((time.monotonic() - started) * 1_000)}, completed.stdout


def stability_run_id(attempt: int) -> str:
    return f"r007b-stability-{attempt}-{uuid4().hex[:12]}"


def sha256_json(value: Any) -> str:
    """Hash one redacted JSON value without depending on pretty-printing."""

    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def is_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == SHA256_LENGTH and all(char in "0123456789abcdef" for char in value)


def safe_artifact_name(value: Any, *, suffix: str) -> bool:
    return (
        isinstance(value, str)
        and value.endswith(suffix)
        and value == Path(value).name
        and ".." not in value
        and len(value) <= 180
    )


def load_evidence_bundle(attempt_dir: Path, bundle_file: Any) -> tuple[dict[str, Any], str]:
    """Read one child bundle while refusing artifact-root escapes and raw text."""

    if not safe_artifact_name(bundle_file, suffix=".json"):
        fail("R007B_GATE_EVIDENCE_BUNDLE_PATH_INVALID")
    target = (attempt_dir / bundle_file).resolve()
    if target.parent != attempt_dir.resolve() or not target.is_file():
        fail("R007B_GATE_EVIDENCE_BUNDLE_MISSING")
    try:
        raw = target.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise GateFailure("R007B_GATE_EVIDENCE_BUNDLE_INVALID") from exc
    if not isinstance(value, dict):
        fail("R007B_GATE_EVIDENCE_BUNDLE_INVALID")
    return value, hashlib.sha256(raw).hexdigest()


def reject_sensitive_evidence_bundle_value(value: Any) -> None:
    """Evidence is deliberately provenance hashes/facts, never user source text."""

    forbidden_keys = {
        "source_statement",
        "license_statement",
        "license",
        "request_body",
        "absolute_path",
        "project_id",
        "asset_version_id",
        "evidence_id",
        "content_base64",
        "source_url",
        "file_path",
    }
    if isinstance(value, Mapping):
        for key, child in value.items():
            if key in forbidden_keys:
                fail("R007B_GATE_EVIDENCE_BUNDLE_NOT_REDACTED")
            reject_sensitive_evidence_bundle_value(child)
    elif isinstance(value, list):
        for child in value:
            reject_sensitive_evidence_bundle_value(child)
    elif isinstance(value, str):
        if str(ROOT) in value or "Authorized only for ForgeCAD" in value:
            fail("R007B_GATE_EVIDENCE_BUNDLE_NOT_REDACTED")


def validate_evidence_bundle(bundle: Mapping[str, Any], *, run_id: str, screenshot_expected: bool) -> dict[str, Any]:
    """Require bounded, persistent engineering evidence for one browser run.

    This validation intentionally knows no source pixels, GLB content, provider,
    visual score, or M108B outcome.  It confirms only that the real rendered
    workbench persisted the exact sealed design-surface provenance it read back.
    """

    reject_sensitive_evidence_bundle_value(bundle)
    if (
        bundle.get("schema_version") != EVIDENCE_BUNDLE_SCHEMA
        or bundle.get("status") != "pass"
        or bundle.get("task") != "FGC-R007B"
        or bundle.get("run_id") != run_id
        or not is_sha256(bundle.get("smoke_source_sha256"))
        or not is_sha256(bundle.get("fixture_source_sha256"))
        or bundle.get("reference_vision_capability") is not False
        or bundle.get("visual_fidelity_validated") is not False
        or bundle.get("formal_eligible") is not False
    ):
        fail("R007B_GATE_EVIDENCE_BUNDLE_INVALID")
    screenshot_path = bundle.get("screenshot_path")
    if screenshot_expected and not safe_artifact_name(screenshot_path, suffix=".png"):
        fail("R007B_GATE_EVIDENCE_SCREENSHOT_INVALID")
    if not screenshot_expected and screenshot_path is not None:
        fail("R007B_GATE_EVIDENCE_SCREENSHOT_INVALID")
    lineage = bundle.get("lineage_bindings")
    required_lineage_hashes = {
        "project_identity_sha256",
        "base_asset_identity_sha256",
        "evidence_identity_sha256",
        "plan_identity_sha256",
        "analysis_identity_sha256",
        "change_set_identity_sha256",
        "result_asset_identity_sha256",
    }
    if (
        not isinstance(lineage, Mapping)
        or set(lineage) != required_lineage_hashes
        or not all(is_sha256(value) for value in lineage.values())
    ):
        fail("R007B_GATE_EVIDENCE_LINEAGE_BINDING_INVALID")
    production_glb = bundle.get("production_glb")
    if not isinstance(production_glb, Mapping) or set(production_glb) != {"base", "result"}:
        fail("R007B_GATE_EVIDENCE_PRODUCTION_GLB_INVALID")
    for production in production_glb.values():
        readback = production.get("readback") if isinstance(production, Mapping) else None
        if (
            not isinstance(production, Mapping)
            or not is_sha256(production.get("glb_sha256"))
            or not isinstance(production.get("byte_size"), int)
            or production["byte_size"] < 64
            or production.get("artifact_profile_id") != "production_concept"
            or not is_sha256(production.get("artifact_profile_sha256"))
            or not is_sha256(production.get("shape_program_sha256"))
            or not isinstance(readback, Mapping)
            or not isinstance(readback.get("triangle_count"), int)
            or readback["triangle_count"] < 1
            or not isinstance(readback.get("mesh_count"), int)
            or readback["mesh_count"] < 1
            or not isinstance(readback.get("primitive_count"), int)
            or readback["primitive_count"] < 1
            or not isinstance(readback.get("material_count"), int)
            or readback["material_count"] < 1
            or not isinstance(readback.get("pbr_material_count"), int)
            or readback["pbr_material_count"] < 1
        ):
            fail("R007B_GATE_EVIDENCE_PRODUCTION_GLB_INVALID")
    recipe_readback = bundle.get("c106_recipe_readback")
    if not isinstance(recipe_readback, Mapping) or set(recipe_readback) != {"base", "result"}:
        fail("R007B_GATE_EVIDENCE_C106_RECIPE_INVALID")
    for asset_recipe in recipe_readback.values():
        roots = asset_recipe.get("root_recipe_ids") if isinstance(asset_recipe, Mapping) else None
        recipe_hashes = asset_recipe.get("recipe_hashes") if isinstance(asset_recipe, Mapping) else None
        if (
            not isinstance(asset_recipe, Mapping)
            or asset_recipe.get("domain_pack_id") != "pack_robotic_arm_concept"
            or asset_recipe.get("component_recipe_instance_count") != 10
            or not isinstance(roots, list)
            or len(roots) != 1
            or roots[0] not in ROOT_IDS
            or not isinstance(recipe_hashes, list)
            or len(recipe_hashes) != 10
            or not all(is_sha256(value) for value in recipe_hashes)
        ):
            fail("R007B_GATE_EVIDENCE_C106_RECIPE_INVALID")
    renderer = bundle.get("single_renderer")
    rust_only = bundle.get("rust_only_counts")
    if (
        not isinstance(renderer, Mapping)
        or renderer.get("canvas_count") != 1
        or renderer.get("stable_reference_result_swap") is not True
        or not isinstance(renderer.get("renderer_generation"), int)
        or renderer["renderer_generation"] < 1
        or renderer.get("active_webgl_contexts") != 1
        or renderer.get("render_source") != "glb_pbr"
        or renderer.get("artifact_kind") != "compiled_agent_production_pbr"
        or renderer.get("base_and_result_generation_equal") is not True
        or not isinstance(rust_only, Mapping)
        or not isinstance(rust_only.get("rust_product_request_count"), int)
        or rust_only["rust_product_request_count"] < 1
        or rust_only.get("python_product_route_attempts") != 0
    ):
        fail("R007B_GATE_EVIDENCE_RUNTIME_PROOF_INVALID")
    paired = bundle.get("paired_screenshots")
    if screenshot_expected:
        if not isinstance(paired, Mapping) or set(paired) != {"reference", "result"}:
            fail("R007B_GATE_EVIDENCE_SCREENSHOT_PAIR_INVALID")
        captures = []
        for phase in ("reference", "result"):
            capture = paired.get(phase)
            if (
                not isinstance(capture, Mapping)
                or capture.get("phase") != phase
                or not safe_artifact_name(capture.get("filename"), suffix=".png")
                or not is_sha256(capture.get("sha256"))
                or not isinstance(capture.get("byte_size"), int)
                or capture["byte_size"] < 64
                or capture.get("renderer_generation") != renderer["renderer_generation"]
                or capture.get("active_webgl_contexts") != 1
            ):
                fail("R007B_GATE_EVIDENCE_SCREENSHOT_PAIR_INVALID")
            captures.append(capture)
        if captures[0]["filename"] == captures[1]["filename"] or captures[0]["sha256"] == captures[1]["sha256"]:
            fail("R007B_GATE_EVIDENCE_SCREENSHOT_PAIR_INVALID")
        if screenshot_path != paired["result"]["filename"]:
            fail("R007B_GATE_EVIDENCE_SCREENSHOT_PAIR_INVALID")
    elif paired is not None:
        fail("R007B_GATE_EVIDENCE_SCREENSHOT_PAIR_INVALID")
    flows = bundle.get("flows")
    if not isinstance(flows, list) or len(flows) != 2:
        fail("R007B_GATE_EVIDENCE_FLOW_COUNT_INVALID")
    by_outcome = {flow.get("outcome"): flow for flow in flows if isinstance(flow, Mapping)}
    if set(by_outcome) != {"rejected", "confirmed"}:
        fail("R007B_GATE_EVIDENCE_OUTCOME_INVALID")
    source_hashes: set[str] = set()
    for outcome, flow in by_outcome.items():
        for key in ("source_hash", "analysis_hash", "plan_hash", "change_set_hash"):
            if not is_sha256(flow.get(key)):
                fail("R007B_GATE_EVIDENCE_HASH_INVALID")
        source_hashes.add(flow["source_hash"])
        if flow.get("fidelity_ceiling") != "single_image_visible_surface_only":
            fail("R007B_GATE_EVIDENCE_CEILING_INVALID")
        for key in ("retained", "intentionally_changed", "unresolved", "sealed_operations", "surface_adornment_readback"):
            if not isinstance(flow.get(key), list):
                fail("R007B_GATE_EVIDENCE_FACTS_INVALID")
        sealed = flow["sealed_operations"]
        if not sealed or not any(
            item.get("op") == "apply_surface_adornment"
            and is_sha256(item.get("operation_sha256"))
            and isinstance(item.get("program_id"), str)
            for item in sealed
            if isinstance(item, Mapping)
        ):
            fail("R007B_GATE_EVIDENCE_SURFACE_OPERATION_MISSING")
        if outcome == "rejected":
            if flow.get("result_glb_hash") is not None or flow["surface_adornment_readback"]:
                fail("R007B_GATE_EVIDENCE_REJECT_INVALID")
        else:
            if not is_sha256(flow.get("result_glb_hash")) or flow["result_glb_hash"] == flow["source_hash"]:
                fail("R007B_GATE_EVIDENCE_RESULT_HASH_INVALID")
            readback_programs = flow["surface_adornment_readback"]
            if not readback_programs or not all(
                isinstance(item, Mapping)
                and isinstance(item.get("program_id"), str)
                and is_sha256(item.get("program_sha256"))
                and isinstance(item.get("target_part_id"), str)
                and isinstance(item.get("target_zone_id"), str)
                for item in readback_programs
            ):
                fail("R007B_GATE_EVIDENCE_SURFACE_READBACK_INVALID")
            sealed_program_ids = {
                item.get("program_id") for item in sealed
                if isinstance(item, Mapping) and item.get("op") == "apply_surface_adornment"
            }
            if not sealed_program_ids.issubset({item["program_id"] for item in readback_programs}):
                fail("R007B_GATE_EVIDENCE_SURFACE_READBACK_INVALID")
    if len(source_hashes) != 1 or next(iter(source_hashes)) != bundle["fixture_source_sha256"]:
        fail("R007B_GATE_EVIDENCE_SOURCE_HASH_INVALID")
    if by_outcome["confirmed"]["result_glb_hash"] != production_glb["result"]["glb_sha256"]:
        fail("R007B_GATE_EVIDENCE_RESULT_HASH_INVALID")
    return {
        "bundle_sha256": sha256_json(bundle),
        "screenshot_path": screenshot_path,
        "reference_screenshot_path": paired["reference"]["filename"] if isinstance(paired, Mapping) else None,
        "result_screenshot_path": paired["result"]["filename"] if isinstance(paired, Mapping) else None,
        "source_hash": bundle["fixture_source_sha256"],
        "result_glb_hash": by_outcome["confirmed"]["result_glb_hash"],
        "rust_product_request_count": rust_only["rust_product_request_count"],
    }


def compact_run_failure(error: GateFailure) -> tuple[str, str]:
    """Keep one useful, non-path failure reason per isolated browser run."""

    raw = str(error)
    code = raw.split(":", 1)[0]
    for marker in (
        "Recipe replacement refuses a subtree referenced by external ShapeProgram operations.",
        "REFERENCE_REBUILD_C106_BASE_REQUIRED",
    ):
        if marker in raw:
            return code, marker
    for line in raw.splitlines():
        stripped = line.strip()
        if stripped.startswith("Error: "):
            return code, stripped[:360]
    return code, "R007B rendered workbench child process failed without a stable diagnostic line."


def run_rendered_workbench_stability() -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Run the real workbench three times without reusing process state.

    The browser smoke itself owns a fresh mkdtemp root, Rust SQLite/CAS root,
    Python compatibility root and three loopback ports per invocation. This
    wrapper deliberately starts three separate Node processes, each with a
    unique project-shell request token. It reports every attempt so a later
    green cannot conceal an earlier intermittent failure.
    """

    attempts: list[dict[str, Any]] = []
    seen_project_ids: set[str] = set()
    seen_bundle_hashes: set[str] = set()
    failures = 0
    EVIDENCE_ARTIFACT_ROOT.mkdir(parents=True, exist_ok=True)
    for attempt in range(1, WORKBENCH_STABILITY_RUNS + 1):
        run_id = stability_run_id(attempt)
        attempt_artifact_dir = (EVIDENCE_ARTIFACT_ROOT / run_id).resolve()
        if attempt_artifact_dir.parent != EVIDENCE_ARTIFACT_ROOT or attempt_artifact_dir.exists():
            fail("R007B_GATE_EVIDENCE_ARTIFACT_DIR_INVALID")
        attempt_artifact_dir.mkdir(parents=True)
        started = time.monotonic()
        try:
            facet, output = run(
                f"rendered_workbench_exact_lineage_run_{attempt}",
                ["npm", "run", "desktop:r007b-reference-workbench-playwright"],
                environment={
                    **os.environ,
                    "FORGECAD_R007B_RUN_ID": run_id,
                    "FORGECAD_R007B_ARTIFACT_DIR": str(attempt_artifact_dir),
                },
            )
            rendered = last_json(output, "R007BReferenceWorkbenchPlaywright@1")
            required_rendered_assertions = {
                "single_image_file_input_evidence",
                "explicit_a005_v2_enable",
                "rust_driver_only_product_state",
                "isolated_ephemeral_run",
                "real_c106_production_glb_base_and_result",
                "strict_c106_base_complete_lineage",
                "preview_zero_version",
                "same_workbench_reference_result_pair",
                "paired_reference_result_screenshots",
                "exact_evidence_plan_changeset_result_lineage",
                "reference_effect_changes_sealed_design_surface",
                "reject_zero_result",
                "confirm_one_result",
                "stable_renderer_generation",
                "single_webgl_context",
                "no_similarity_or_visual_score",
            }
            isolation = rendered.get("isolation")
            evidence_summary = rendered.get("evidence_bundle")
            project_id = isolation.get("project_id") if isinstance(isolation, Mapping) else None
            if (
                rendered.get("status") != "pass"
                or rendered.get("visual_fidelity_validated") is not False
                or rendered.get("formal_eligible") is not False
                or not required_rendered_assertions.issubset(set(rendered.get("assertions", [])))
                or not isinstance(isolation, Mapping)
                or isolation.get("run_id") != run_id
                or isolation.get("ephemeral_roots_unique") is not True
                or isolation.get("distinct_loopback_ports") is not True
                or isolation.get("python_compatibility_shell_only") is not True
                or isolation.get("rust_product_state_only") is not True
                or isolation.get("python_product_route_attempts") != 0
                or not isinstance(isolation.get("rust_product_request_count"), int)
                or isolation["rust_product_request_count"] < 1
                or not isinstance(project_id, str)
                or not project_id
                or project_id in seen_project_ids
            ):
                fail("R007B_GATE_RENDERED_WORKBENCH_INVALID")
            if (
                not isinstance(evidence_summary, Mapping)
                or evidence_summary.get("retained") is not True
                or evidence_summary.get("artifact_dir_configured") is not True
                or not safe_artifact_name(evidence_summary.get("bundle_file"), suffix=".json")
                or not safe_artifact_name(evidence_summary.get("screenshot_path"), suffix=".png")
                or not safe_artifact_name(evidence_summary.get("reference_screenshot_path"), suffix=".png")
                or not safe_artifact_name(evidence_summary.get("result_screenshot_path"), suffix=".png")
                or evidence_summary.get("screenshot_path") != evidence_summary.get("result_screenshot_path")
            ):
                fail("R007B_GATE_EVIDENCE_BUNDLE_NOT_RETAINED")
            bundle, persisted_bundle_sha256 = load_evidence_bundle(attempt_artifact_dir, evidence_summary["bundle_file"])
            evidence = validate_evidence_bundle(bundle, run_id=run_id, screenshot_expected=True)
            if persisted_bundle_sha256 in seen_bundle_hashes:
                fail("R007B_GATE_EVIDENCE_BUNDLE_IDENTITY_INVALID")
            for screenshot_name in (evidence["reference_screenshot_path"], evidence["result_screenshot_path"]):
                screenshot = (attempt_artifact_dir / screenshot_name).resolve()
                if screenshot.parent != attempt_artifact_dir or not screenshot.is_file() or screenshot.stat().st_size < 64:
                    fail("R007B_GATE_EVIDENCE_SCREENSHOT_MISSING")
            seen_project_ids.add(project_id)
            seen_bundle_hashes.add(persisted_bundle_sha256)
            attempts.append({
                "id": facet["id"],
                "status": "pass",
                "elapsed_ms": facet["elapsed_ms"],
                "run_id": run_id,
                "project_id": project_id,
                "rust_product_request_count": isolation["rust_product_request_count"],
                "evidence_bundle_file": evidence_summary["bundle_file"],
                "evidence_bundle_sha256": persisted_bundle_sha256,
                "source_hash": evidence["source_hash"],
                "result_glb_hash": evidence["result_glb_hash"],
                "screenshot_path": evidence["screenshot_path"],
                "reference_screenshot_path": evidence["reference_screenshot_path"],
                "result_screenshot_path": evidence["result_screenshot_path"],
            })
        except GateFailure as exc:
            failures += 1
            error_code, detail = compact_run_failure(exc)
            attempts.append({
                "id": f"rendered_workbench_exact_lineage_run_{attempt}",
                "status": "fail",
                "elapsed_ms": round((time.monotonic() - started) * 1_000),
                "run_id": run_id,
                "error": error_code,
                "detail": detail,
            })
        except Exception as exc:  # pragma: no cover - defensive Gate reporting
            failures += 1
            attempts.append({
                "id": f"rendered_workbench_exact_lineage_run_{attempt}",
                "status": "fail",
                "elapsed_ms": round((time.monotonic() - started) * 1_000),
                "run_id": run_id,
                "error": f"R007B_GATE_RENDERED_WORKBENCH_EXCEPTION:{type(exc).__name__}",
            })
    if failures or len(seen_project_ids) != WORKBENCH_STABILITY_RUNS or len(seen_bundle_hashes) != WORKBENCH_STABILITY_RUNS:
        return {
            "id": "rendered_workbench_exact_lineage_stability",
            "status": "fail",
            "run_count": WORKBENCH_STABILITY_RUNS,
            "isolated_project_count": len(seen_project_ids),
            "retained_evidence_bundle_count": len(seen_bundle_hashes),
            "error": "R007B_GATE_RENDERED_WORKBENCH_STABILITY_INVALID",
        }, attempts
    return {
        "id": "rendered_workbench_exact_lineage_stability",
        "status": "pass",
        "run_count": WORKBENCH_STABILITY_RUNS,
        "isolated_project_count": len(seen_project_ids),
        "retained_evidence_bundle_count": len(seen_bundle_hashes),
    }, attempts


def reject_visual_or_provider_claims(value: Any) -> None:
    """Fail closed if a fixture starts claiming pixel/vision/provider analysis."""

    if isinstance(value, Mapping):
        for key, child in value.items():
            if key in FORBIDDEN_CLAIM_KEYS:
                fail("R007B_GATE_VISUAL_OR_PROVIDER_CLAIM_FORBIDDEN")
            if key == "reference_vision_capability" and child is not False:
                fail("R007B_GATE_REFERENCE_VISION_CLAIM_FORBIDDEN")
            if key == "visual_fidelity_validated" and child is not False:
                fail("R007B_GATE_VISUAL_FIDELITY_CLAIM_FORBIDDEN")
            reject_visual_or_provider_claims(child)
    elif isinstance(value, list):
        for child in value:
            reject_visual_or_provider_claims(child)
    elif isinstance(value, str):
        if any(claim in value.lower() for claim in FORBIDDEN_CLAIM_TEXT):
            fail("R007B_GATE_VISUAL_OR_PROVIDER_CLAIM_FORBIDDEN")


def validate_fixture_set_value(fixture_set: Any) -> list[dict[str, Any]]:
    reject_visual_or_provider_claims(fixture_set)
    if (
        not isinstance(fixture_set, dict)
        or fixture_set.get("schema_version") != "R007BReferenceFixtureSet@1"
        or fixture_set.get("domain_pack_id") != "pack_robotic_arm_concept"
        or fixture_set.get("license_boundary") != "user_authorized_fixture_only_no_network_no_mesh_copy"
        or fixture_set.get("analysis_source") != ANALYSIS_SOURCE
        or fixture_set.get("reference_vision_capability") is not False
        or fixture_set.get("visual_fidelity_validated") is not False
        or fixture_set.get("formal_eligible") is not False
    ):
        fail("R007B_GATE_FIXTURE_SET_INVALID")
    fixtures = fixture_set.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 3:
        fail("R007B_GATE_FIXTURE_COUNT_INVALID")
    classes = {item.get("reference_class") for item in fixtures if isinstance(item, dict)}
    if classes != {"single_image", "multi_view_contact_sheet", "strict_glb_readback"}:
        fail("R007B_GATE_FIXTURE_CLASS_INVALID")
    roots: set[str] = set()
    details: list[dict[str, Any]] = []
    for item in fixtures:
        if not isinstance(item, dict):
            fail("R007B_GATE_FIXTURE_ITEM_INVALID")
        root_id = item.get("declared_target_root_recipe_id")
        roles = item.get("declared_binding_roles")
        ceiling = item.get("declared_fidelity_ceiling")
        if not isinstance(root_id, str) or root_id not in ROOT_IDS or root_id in roots:
            fail("R007B_GATE_FIXTURE_PLAN_NOT_DISTINCT")
        if not isinstance(roles, list) or not roles or len(roles) != len(set(roles)):
            fail("R007B_GATE_FIXTURE_BINDING_INVALID")
        if ceiling not in {
            "single_image_visible_surface_only",
            "multi_view_image_visible_surface_only",
            "strict_glb_readback_visible_bounds_only",
        }:
            fail("R007B_GATE_FIXTURE_CEILING_INVALID")
        if item.get("kind") == "image":
            encoded = item.get("content_base64")
            if not isinstance(encoded, str) or not encoded.startswith("iVBOR"):
                fail("R007B_GATE_IMAGE_FIXTURE_INVALID")
            if item.get("reference_class") == "single_image":
                if ceiling != "single_image_visible_surface_only" or len(item.get("declared_missing_views", [])) < 3:
                    fail("R007B_GATE_SINGLE_IMAGE_MISSING_VIEW_INVALID")
            elif item.get("reference_class") == "multi_view_contact_sheet":
                if ceiling != "multi_view_image_visible_surface_only" or item.get("declared_missing_views") != []:
                    fail("R007B_GATE_CONTACT_SHEET_CEILING_INVALID")
        elif item.get("kind") == "glb":
            if item.get("declared_source") != "compile_c106_root_through_restricted_executor":
                fail("R007B_GATE_GLB_FIXTURE_INVALID")
            if ceiling != "strict_glb_readback_visible_bounds_only":
                fail("R007B_GATE_GLB_CEILING_INVALID")
        else:
            fail("R007B_GATE_FIXTURE_KIND_INVALID")
        roots.add(root_id)
        details.append(
            {
                "fixture_id": item.get("fixture_id"),
                "reference_class": item.get("reference_class"),
                "analysis_source": ANALYSIS_SOURCE,
                "declared_target_root_recipe_id": root_id,
                "declared_fidelity_ceiling": ceiling,
                "declared_binding_roles": roles,
                "reference_vision_capability": False,
                "visual_fidelity_validated": False,
            }
        )
    if roots != ROOT_IDS:
        fail("R007B_GATE_FIXTURE_PLAN_NOT_DISTINCT")
    return details


def validate_fixture_set() -> list[dict[str, Any]]:
    try:
        fixture_set = json.loads(FIXTURES.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise GateFailure("R007B_GATE_FIXTURE_SET_INVALID") from exc
    return validate_fixture_set_value(fixture_set)


def validate_r007b_contract(output: str) -> None:
    # Exact test count prevents a renamed/no-match Cargo filter from turning
    # into a false green.  The fourth test includes source-hash tampering and
    # the contact-sheet distinct-root fixture.
    if "test result: ok. 4 passed" not in output:
        fail("R007B_GATE_CONTRACT_TEST_COUNT_INVALID")


def validate_c106_production(output: str) -> dict[str, Any]:
    report = last_json(output, "C106RoboticArmProductionGate@1")
    measurement = report.get("provider_measurement")
    roots = report.get("roots")
    if (
        report.get("status") != "pass"
        or report.get("formal_eligible") is not False
        or report.get("measured_provider_calls") != 0
        or not isinstance(measurement, dict)
        or measurement.get("schema_version") != "C106ProviderCallMeasurement@1"
        or measurement.get("measurement_source") != "execution_node_trace"
        or measurement.get("measured_provider_calls") != 0
        or not isinstance(roots, list)
        or len(roots) != 3
    ):
        fail("R007B_GATE_C106_PRODUCTION_INVALID")
    hashes: dict[str, str] = {}
    for root in roots:
        if not isinstance(root, dict):
            fail("R007B_GATE_C106_PRODUCTION_INVALID")
        recipe_id = root.get("recipe_id")
        glb_sha256 = root.get("glb_sha256")
        if recipe_id not in ROOT_IDS or not isinstance(glb_sha256, str) or len(glb_sha256) != 64:
            fail("R007B_GATE_C106_PRODUCTION_INVALID")
        hashes[recipe_id] = glb_sha256
    if set(hashes) != ROOT_IDS or len(set(hashes.values())) != 3:
        fail("R007B_GATE_C106_GLB_IDENTITY_INVALID")
    # The strict-readback GLB is the desktop root.  The frozen R007B analysis
    # chooses the gallery root as its new candidate, so its content hash must
    # prove that the source asset was not copied or edited in place.
    if hashes["recipe_c106_arm_desktop_assistant"] == hashes["recipe_c106_arm_gallery_industrial"]:
        fail("R007B_GATE_REFERENCE_RESULT_HASH_NOT_DISTINCT")
    return {
        "reference_glb_recipe_id": "recipe_c106_arm_desktop_assistant",
        "reference_glb_sha256": hashes["recipe_c106_arm_desktop_assistant"],
        "result_glb_recipe_id": "recipe_c106_arm_gallery_industrial",
        "result_glb_sha256": hashes["recipe_c106_arm_gallery_industrial"],
    }


def validate_lifecycle(output: str) -> Mapping[str, Any]:
    report = last_json(output, "C106RoboticArmLifecycleGate@1")
    measurement = report.get("provider_measurement")
    negative = report.get("negative_evidence")
    if (
        report.get("status") != "pass"
        or report.get("production_gate") != "skipped_by_aggregate"
        or report.get("measured_provider_calls") != 0
        or not isinstance(measurement, dict)
        or measurement.get("schema_version") != "C106LifecycleMeasuredEvidence@1"
        or measurement.get("measurement_source") != "FakeDeepSeekClient.records"
        or measurement.get("measured_provider_calls") != 0
        or measurement.get("restart_readback") is not True
        or measurement.get("transient_cancel_reject_zero_persistent_writes") is not True
        or not isinstance(negative, list)
        or "in_flight_cancel_discards_late_geometry_and_never_promotes_state" not in negative
    ):
        fail("R007B_GATE_LIFECYCLE_INVALID")
    return measurement


def self_test() -> int:
    details = validate_fixture_set()
    if len(details) != 3:
        fail("R007B_GATE_SELF_TEST_INVALID")
    run_ids = [stability_run_id(index) for index in range(1, WORKBENCH_STABILITY_RUNS + 1)]
    if WORKBENCH_STABILITY_RUNS < 3 or len(run_ids) != len(set(run_ids)):
        fail("R007B_GATE_SELF_TEST_STABILITY_ISOLATION_INVALID")
    fixture_set = json.loads(FIXTURES.read_text(encoding="utf-8"))
    tampered_vision = json.loads(json.dumps(fixture_set))
    tampered_vision["reference_vision_capability"] = True
    try:
        validate_fixture_set_value(tampered_vision)
    except GateFailure as exc:
        if str(exc) != "R007B_GATE_REFERENCE_VISION_CLAIM_FORBIDDEN":
            raise
    else:
        fail("R007B_GATE_SELF_TEST_VISION_TAMPER_ACCEPTED")
    tampered_provider = json.loads(json.dumps(fixture_set))
    tampered_provider["fixtures"][0]["provider"] = "forbidden"
    try:
        validate_fixture_set_value(tampered_provider)
    except GateFailure as exc:
        if str(exc) != "R007B_GATE_VISUAL_OR_PROVIDER_CLAIM_FORBIDDEN":
            raise
    else:
        fail("R007B_GATE_SELF_TEST_PROVIDER_TAMPER_ACCEPTED")
    digest = lambda label: hashlib.sha256(label.encode("utf-8")).hexdigest()
    surface_program_id = "adorn_r007b_self_test"
    sealed_operation = {
        "op": "apply_surface_adornment",
        "operation_sha256": digest("sealed-operation"),
        "program_id": surface_program_id,
    }
    base_flow = {
        "source_hash": digest("source"),
        "analysis_hash": digest("analysis"),
        "plan_hash": digest("plan"),
        "change_set_hash": digest("change-set"),
        "fidelity_ceiling": "single_image_visible_surface_only",
        "retained": ["silhouette"],
        "intentionally_changed": ["surface_adornment_normalization"],
        "unresolved": ["hidden_structure"],
        "sealed_operations": [sealed_operation],
    }
    self_test_bundle = {
        "schema_version": EVIDENCE_BUNDLE_SCHEMA,
        "status": "pass",
        "task": "FGC-R007B",
        "run_id": "r007b-self-test",
        "smoke_source_sha256": digest("smoke"),
        "fixture_source_sha256": base_flow["source_hash"],
        "lineage_bindings": {
            "project_identity_sha256": digest("project-identity"),
            "base_asset_identity_sha256": digest("base-asset-identity"),
            "evidence_identity_sha256": digest("evidence-identity"),
            "plan_identity_sha256": digest("plan-identity"),
            "analysis_identity_sha256": digest("analysis-identity"),
            "change_set_identity_sha256": digest("change-set-identity"),
            "result_asset_identity_sha256": digest("result-asset-identity"),
        },
        "flows": [
            {**base_flow, "outcome": "rejected", "result_glb_hash": None, "surface_adornment_readback": []},
            {
                **base_flow,
                "outcome": "confirmed",
                "result_glb_hash": digest("result-glb"),
                "surface_adornment_readback": [{
                    "program_id": surface_program_id,
                    "program_sha256": digest("program"),
                    "target_part_id": "part_r007b_self_test",
                    "target_zone_id": "zone_arm_surface_trim",
                }],
            },
        ],
        "production_glb": {
            "base": {
                "glb_sha256": digest("base-glb"),
                "byte_size": 4096,
                "artifact_profile_id": "production_concept",
                "artifact_profile_sha256": digest("profile"),
                "shape_program_sha256": digest("base-shape"),
                "readback": {"triangle_count": 12000, "mesh_count": 10, "primitive_count": 10, "material_count": 5, "pbr_material_count": 5},
            },
            "result": {
                "glb_sha256": digest("result-glb"),
                "byte_size": 4352,
                "artifact_profile_id": "production_concept",
                "artifact_profile_sha256": digest("profile"),
                "shape_program_sha256": digest("result-shape"),
                "readback": {"triangle_count": 12000, "mesh_count": 10, "primitive_count": 10, "material_count": 5, "pbr_material_count": 5},
            },
        },
        "c106_recipe_readback": {
            "base": {
                "domain_pack_id": "pack_robotic_arm_concept",
                "component_recipe_instance_count": 10,
                "root_recipe_ids": ["recipe_c106_arm_desktop_assistant"],
                "recipe_hashes": [digest(f"base-recipe-{index}") for index in range(10)],
            },
            "result": {
                "domain_pack_id": "pack_robotic_arm_concept",
                "component_recipe_instance_count": 10,
                "root_recipe_ids": ["recipe_c106_arm_desktop_assistant"],
                "recipe_hashes": [digest(f"result-recipe-{index}") for index in range(10)],
            },
        },
        "single_renderer": {
            "canvas_count": 1,
            "stable_reference_result_swap": True,
            "renderer_generation": 1,
            "active_webgl_contexts": 1,
            "render_source": "glb_pbr",
            "artifact_kind": "compiled_agent_production_pbr",
            "base_and_result_generation_equal": True,
        },
        "rust_only_counts": {"rust_product_request_count": 1, "python_product_route_attempts": 0},
        "paired_screenshots": None,
        "screenshot_path": None,
        "reference_vision_capability": False,
        "visual_fidelity_validated": False,
        "formal_eligible": False,
    }
    validate_evidence_bundle(self_test_bundle, run_id="r007b-self-test", screenshot_expected=False)
    tampered_bundle = json.loads(json.dumps(self_test_bundle))
    tampered_bundle["flows"][1]["surface_adornment_readback"] = []
    try:
        validate_evidence_bundle(tampered_bundle, run_id="r007b-self-test", screenshot_expected=False)
    except GateFailure as exc:
        if str(exc) != "R007B_GATE_EVIDENCE_SURFACE_READBACK_INVALID":
            raise
    else:
        fail("R007B_GATE_SELF_TEST_SURFACE_READBACK_TAMPER_ACCEPTED")
    sensitive_bundle = json.loads(json.dumps(self_test_bundle))
    sensitive_bundle["source_statement"] = "must not persist"
    try:
        validate_evidence_bundle(sensitive_bundle, run_id="r007b-self-test", screenshot_expected=False)
    except GateFailure as exc:
        if str(exc) != "R007B_GATE_EVIDENCE_BUNDLE_NOT_REDACTED":
            raise
    else:
        fail("R007B_GATE_SELF_TEST_SENSITIVE_EVIDENCE_ACCEPTED")
    print(json.dumps({
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "mode": "self_test",
        "analysis_source": ANALYSIS_SOURCE,
        "reference_vision_capability": False,
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "measured_provider_calls": 0,
        "negative_evidence": [
            "reference_vision_capability_true_rejected",
            "provider_claim_rejected",
            "stability_gate_requires_three_unique_child_run_ids",
            "surface_adornment_readback_tamper_rejected",
            "sensitive_evidence_bundle_rejected",
        ],
        "workbench_stability_runs": WORKBENCH_STABILITY_RUNS,
    }, sort_keys=True))
    return 0


def main(*, self_test_only: bool = False, workbench_stability_only: bool = False) -> int:
    if self_test_only:
        return self_test()
    if workbench_stability_only:
        facet, attempts = run_rendered_workbench_stability()
        print(json.dumps({
            "schema_version": SCHEMA_VERSION,
            "status": facet["status"],
            "formal_eligible": False,
            "workbench_stability": {**facet, "attempts": attempts},
        }, ensure_ascii=False, sort_keys=True))
        return 0 if facet["status"] == "pass" else 1
    details = validate_fixture_set()
    facets: list[dict[str, Any]] = []
    facet, output = run(
        "r007b_reference_surface_contract",
        [
            str(RUST), "cargo", "test", "--manifest-path", str(MANIFEST), "-p", "forgecad-core",
            "--test", "r007b_reference_surface_analysis", "--offline",
        ],
    )
    validate_r007b_contract(output)
    facets.append(facet)
    facet, output = run("r007a_lifecycle_and_restart", ["npm", "run", "agent:r007-gate"])
    facets.append(facet)
    facet, output = run("c106_real_glb_production", ["npm", "run", "agent:c106-robotic-arm-production-gate"])
    glb_identity = validate_c106_production(output)
    facets.append(facet)
    facet, output = run(
        "cancel_late_restart_and_provider_measurement",
        ["npm", "run", "agent:c106-robotic-arm-lifecycle-gate", "--", "--skip-production"],
    )
    lifecycle_measurement = validate_lifecycle(output)
    facets.append(facet)
    facet, _ = run("a005_surface_slot_boundary", ["npm", "run", "agent:a005-surface-adornment-pbr-smoke"])
    facets.append(facet)
    facet, _ = run("contract_types", ["npm", "run", "contracts:types:check"])
    facets.append(facet)
    facet, output = run("runtime_api_projection", ["npm", "run", "desktop:r007b-reference-api-projection-smoke"])
    projection = last_json(output, "R007BReferenceApiProjectionSmoke@1")
    if projection.get("status") != "pass" or "distinct_result_glb_sha256" not in projection.get("assertions", []):
        fail("R007B_GATE_RUNTIME_API_PROJECTION_INVALID")
    facets.append(facet)

    facet, stability_attempts = run_rendered_workbench_stability()
    facet["attempts"] = stability_attempts
    facets.append(facet)
    if facet["status"] != "pass":
        print(json.dumps({
            "schema_version": SCHEMA_VERSION,
            "status": "fail",
            "formal_eligible": False,
            "error": facet["error"],
            "facets": facets,
        }, ensure_ascii=False, sort_keys=True))
        return 1
    print(json.dumps({
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "analysis_source": ANALYSIS_SOURCE,
        "reference_vision_capability": False,
        "visual_fidelity_validated": False,
        "formal_eligible": False,
        "human_benchmark_evidence": False,
        "measured_provider_calls": lifecycle_measurement["measured_provider_calls"],
        "provider_measurement": lifecycle_measurement,
        "fixture_analyses": details,
        "reference_and_result_glb": glb_identity,
        "negative_evidence": [
            "wrong_zone_cross_domain_wrong_recipe_wrong_source_hash_and_legacy_a005_v1_rejected",
            "single_image_visible_surface_ceiling_and_missing_views_retained",
            "strict_glb_readback_denies_visible_part_projection",
            "r007a_preview_reject_restart_and_c106_late_cancel_do_not_promote_state",
        ],
        "facets": facets,
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run the fail-closed R007B reference-surface gate.")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--workbench-stability-only", action="store_true")
    arguments = parser.parse_args()
    try:
        raise SystemExit(main(
            self_test_only=arguments.self_test,
            workbench_stability_only=arguments.workbench_stability_only,
        ))
    except GateFailure as exc:
        print(json.dumps({"schema_version": SCHEMA_VERSION, "status": "fail", "formal_eligible": False, "error": str(exc)}, ensure_ascii=False, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from exc
