#!/usr/bin/env python3
"""Focused positive/negative checks for the ADR-0026 first-party contracts.

This checker intentionally uses only the Python standard library.  It validates
the small JSON Schema subset used by these contracts, checks their closed and
hash-only shape, and exercises the binding and stage-gate invariants that JSON
Schema cannot compare across separate fields or documents.
"""

from __future__ import annotations

import copy
import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "forgecad-contracts" / "schemas"
MANIFEST = ROOT / "packages" / "forgecad-contracts" / "manifest.json"

EXPECTED = {
    "semantic-scene-graph.schema.json": "SemanticSceneGraph@1",
    "model-understanding-bundle.schema.json": "ModelUnderstandingBundle@1",
    "reference-canvas.schema.json": "ReferenceCanvas@1",
    "design-spec.schema.json": "DesignSpec@1",
    "design-session.schema.json": "DesignSession@1",
    "design-stage-plan.schema.json": "DesignStagePlan@1",
    "design-checkpoint.schema.json": "DesignCheckpoint@1",
    "design-critic-report.schema.json": "DesignCriticReport@1",
    "repair-intent.schema.json": "RepairIntent@1",
    "visual-evidence-bundle.schema.json": "VisualEvidenceBundle@1",
}

QUALITY_STATUS_SCHEMAS = {
    "design-session.schema.json",
    "design-stage-plan.schema.json",
    "design-critic-report.schema.json",
    "repair-intent.schema.json",
    "visual-evidence-bundle.schema.json",
}

OBSERVATION_STATE_SCHEMAS = {
    "semantic-scene-graph.schema.json",
    "model-understanding-bundle.schema.json",
    "reference-canvas.schema.json",
    "design-spec.schema.json",
    "design-critic-report.schema.json",
    "visual-evidence-bundle.schema.json",
}

REPAIR_INTENT_RUN_SCHEMAS = {
    "repair-intent-run-request.schema.json": "RepairIntentRunRequest@1",
    "repair-intent-run-result.schema.json": "RepairIntentRunResult@1",
}

HASH = "a" * 64
REFERENCE_HASH = "b" * 64
CAMERA_HASH = "c" * 64
CANDIDATE_HASH = "d" * 64
EVIDENCE_HASH = "e" * 64
CANONICAL_HASH = "f" * 64
TIMESTAMP = "2026-08-13T00:00:00Z"


class ContractError(Exception):
    """Validation error with a JSON path."""


def fail(message: str) -> None:
    raise SystemExit(f"Agentic contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")
    require(isinstance(value, dict), f"{path.name} must contain a JSON object")
    return value


def load_schema_registry(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    registry: dict[str, dict[str, Any]] = {}
    for filename in manifest.get("schemas", []):
        schema = load_json(SCHEMA_ROOT / filename)
        schema_id = schema.get("$id")
        require(isinstance(schema_id, str), f"{filename} has no schema $id")
        registry[schema_id] = schema
        registry[f"https://forgecad.local/contracts/{filename}"] = schema
    return registry


def resolve_ref(
    root: dict[str, Any],
    reference: str,
    registry: dict[str, dict[str, Any]] | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    if reference.startswith("#"):
        target_root = root
        pointer = reference[1:]
    else:
        base, separator, pointer = reference.partition("#")
        require(registry is not None and base in registry, f"unregistered external $ref: {reference}")
        target_root = registry[base]
    node: Any = target_root
    for part in pointer.lstrip("/").split("/") if pointer else []:
        part = part.replace("~1", "/").replace("~0", "~")
        require(isinstance(node, dict) and part in node, f"unresolved $ref: {reference}")
        node = node[part]
    require(isinstance(node, dict), f"$ref does not resolve to a schema object: {reference}")
    return node, target_root


def type_matches(value: Any, expected: str) -> bool:
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    return False


def json_equal(left: Any, right: Any) -> bool:
    return json.dumps(left, sort_keys=True, separators=(",", ":")) == json.dumps(
        right, sort_keys=True, separators=(",", ":")
    )


def validate(
    schema: dict[str, Any],
    value: Any,
    root: dict[str, Any],
    path: str = "$",
    registry: dict[str, dict[str, Any]] | None = None,
) -> None:
    """Validate the subset of draft 2020-12 used by the new contracts."""
    if "$ref" in schema:
        target, target_root = resolve_ref(root, schema["$ref"], registry)
        validate(target, value, target_root, path, registry)
        return

    if "allOf" in schema:
        for index, child in enumerate(schema["allOf"]):
            validate(child, value, root, f"{path}.allOf[{index}]", registry)
    if "anyOf" in schema:
        errors: list[str] = []
        for child in schema["anyOf"]:
            try:
                validate(child, value, root, path, registry)
            except ContractError as exc:
                errors.append(str(exc))
            else:
                break
        else:
            raise ContractError(f"{path} failed anyOf: {errors[-1] if errors else 'no branch'}")
    if "oneOf" in schema:
        successes = 0
        for child in schema["oneOf"]:
            try:
                validate(child, value, root, path, registry)
            except ContractError:
                continue
            successes += 1
        if successes != 1:
            raise ContractError(f"{path} matched {successes} oneOf branches")
    if "not" in schema:
        try:
            validate(schema["not"], value, root, path, registry)
        except ContractError:
            pass
        else:
            raise ContractError(f"{path} matched a forbidden schema")

    if "if" in schema:
        try:
            validate(schema["if"], value, root, path, registry)
        except ContractError:
            branch = schema.get("else")
        else:
            branch = schema.get("then")
        if branch is not None:
            validate(branch, value, root, path, registry)

    if "const" in schema:
        if not json_equal(value, schema["const"]):
            raise ContractError(f"{path} is not constant {schema['const']!r}")
    if "enum" in schema and not any(json_equal(value, option) for option in schema["enum"]):
        raise ContractError(f"{path} is outside enum")

    if "type" in schema:
        expected = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(type_matches(value, item) for item in expected):
            raise ContractError(f"{path} has type {type(value).__name__}, expected {expected}")

    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0) or len(value) > schema.get("maxLength", 2**31):
            raise ContractError(f"{path} has invalid string length")
        if "pattern" in schema and re.search(schema["pattern"], value) is None:
            raise ContractError(f"{path} does not match {schema['pattern']!r}")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            raise ContractError(f"{path} is below minimum")
        if "maximum" in schema and value > schema["maximum"]:
            raise ContractError(f"{path} is above maximum")
        if "exclusiveMinimum" in schema:
            bound = schema["exclusiveMinimum"]
            if isinstance(bound, bool) and bound and value <= 0:
                raise ContractError(f"{path} is not above zero")
            if isinstance(bound, (int, float)) and value <= bound:
                raise ContractError(f"{path} is not above exclusive minimum")
        if "exclusiveMaximum" in schema:
            bound = schema["exclusiveMaximum"]
            if isinstance(bound, bool) and bound and value >= 0:
                raise ContractError(f"{path} is not below zero")
            if isinstance(bound, (int, float)) and value >= bound:
                raise ContractError(f"{path} is not below exclusive maximum")

    if isinstance(value, dict):
        required = schema.get("required", [])
        missing = [key for key in required if key not in value]
        if missing:
            raise ContractError(f"{path} is missing required fields {missing}")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            unknown = sorted(set(value) - set(properties))
            if unknown:
                raise ContractError(f"{path} has unknown fields {unknown}")
        for key, child in properties.items():
            if key in value:
                validate(child, value[key], root, f"{path}.{key}", registry)

    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0) or len(value) > schema.get("maxItems", 2**31):
            raise ContractError(f"{path} has invalid item count")
        if schema.get("uniqueItems"):
            encoded = [json.dumps(item, sort_keys=True, separators=(",", ":")) for item in value]
            if len(set(encoded)) != len(encoded):
                raise ContractError(f"{path} contains duplicate items")
        if "items" in schema:
            for index, child in enumerate(value):
                validate(schema["items"], child, root, f"{path}[{index}]", registry)
        if "contains" in schema:
            if not any(
                is_valid(schema["contains"], item, registry)
                for item in value
            ):
                raise ContractError(f"{path} does not contain a matching item")


def is_valid(
    schema: dict[str, Any],
    value: Any,
    registry: dict[str, dict[str, Any]] | None = None,
) -> bool:
    try:
        validate(schema, value, schema, registry=registry)
    except ContractError:
        return False
    return True


def walk_schema(node: Any, path: str = "$") -> list[tuple[str, dict[str, Any]]]:
    objects: list[tuple[str, dict[str, Any]]] = []
    if not isinstance(node, dict):
        return objects
    if node.get("type") == "object":
        objects.append((path, node))
    for key, child in node.items():
        if key in {"properties", "$defs", "definitions"} and isinstance(child, dict):
            for name, value in child.items():
                objects.extend(walk_schema(value, f"{path}.{key}.{name}"))
        elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for index, value in enumerate(child):
                    objects.extend(walk_schema(value, f"{path}.{key}[{index}]"))
            else:
                objects.extend(walk_schema(child, f"{path}.{key}"))
    return objects


def walk_property_names(node: Any) -> list[str]:
    names: list[str] = []
    if not isinstance(node, dict):
        return names
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for child in properties.values():
            names.extend(walk_property_names(child))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def schema_ref_name(schema: dict[str, Any]) -> str | None:
    reference = schema.get("$ref")
    if isinstance(reference, str) and reference.startswith("#/$defs/"):
        return reference.rsplit("/", 1)[-1]
    return None


def check_schema_shape(filename: str, schema: dict[str, Any]) -> None:
    expected_version = EXPECTED[filename]
    require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"{filename} is not draft 2020-12")
    require(schema.get("$id") == f"https://forgecad.local/contracts/{filename}", f"{filename} has the wrong $id")
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is not closed")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == expected_version, f"{filename} has the wrong schema_version")
    require("schema_version" in schema.get("required", []) and "canonical_sha256" in schema.get("required", []), f"{filename} is not version/hash bound")
    require(schema.get("$defs", {}).get("identifier", {}).get("pattern") == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$", f"{filename} identifier is not strict")
    require(schema.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$", f"{filename} SHA-256 is not strict")
    if filename in OBSERVATION_STATE_SCHEMAS:
        require('"observed"' in json.dumps(schema) and '"inferred"' in json.dumps(schema) and '"unknown"' in json.dumps(schema), f"{filename} omits observed/inferred/unknown")
    if filename in QUALITY_STATUS_SCHEMAS:
        require('"QUALITY_TARGET_NOT_MET"' in json.dumps(schema), f"{filename} does not preserve the current quality status vocabulary")
    require("safe_text" in schema.get("$defs", {}), f"{filename} has no path/secret-safe text definition")

    for path, object_schema in walk_schema(schema):
        require(object_schema.get("additionalProperties") is False, f"{filename} {path} is an open object")

    forbidden_names = {"path", "url", "uri", "raw", "raw_bytes", "bytes", "secret", "token", "password", "api_key", "prompt", "script", "shell", "environment"}
    for name in walk_property_names(schema):
        require(name.lower() not in forbidden_names, f"{filename} exposes forbidden property {name}")

    def inspect_properties(node: Any) -> None:
        if not isinstance(node, dict):
            return
        properties = node.get("properties")
        if isinstance(properties, dict):
            for name, child in properties.items():
                if name.endswith("_id"):
                    require(
                        schema_ref_name(child) in {"identifier", "nullable_identifier"} or "pattern" in child or "const" in child,
                        f"{filename}.{name} is not identifier constrained",
                    )
                if name.endswith("_sha256") or name.endswith("_hash"):
                    require(
                        schema_ref_name(child) in {"sha256", "nullable_sha256"}
                        or child.get("pattern") == "^[0-9a-f]{64}$"
                        or "const" in child,
                        f"{filename}.{name} is not SHA-256 constrained",
                    )
                inspect_properties(child)
        for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
            child = node.get(key)
            if isinstance(child, list):
                for value in child:
                    inspect_properties(value)
            elif isinstance(child, dict):
                inspect_properties(child)

    inspect_properties(schema)


def evidence(kind: str = "scene", value: str = EVIDENCE_HASH) -> dict[str, str]:
    return {"kind": kind, "sha256": value}


def state(visibility: str = "observed", confidence: float = 1.0) -> dict[str, Any]:
    return {
        "visibility": visibility,
        "confidence": 0 if visibility == "unknown" else confidence,
        "evidence_refs": [evidence()],
    }


def gate(stage: str, status: str = "fail") -> dict[str, Any]:
    return {
        "stage": stage,
        "status": status,
        "required_checks": ["reference-authorized", "reference-coverage", "visible-view"],
        "failed_checks": ["primary-silhouette"] if status != "pass" else [],
        "evidence_hashes": [EVIDENCE_HASH],
        "unlocks": ["primary-form-adjustment"] if status != "pass" else ["secondary-structure"],
        "locks": ["tertiary-detail", "uv-pbr", "export"] if status != "pass" else [],
    }


def make_fixtures() -> dict[str, dict[str, Any]]:
    part = {
        "part_id": "main-body",
        "name": "Main Body",
        "role": "main-body",
        "parent_id": None,
        "child_ids": ["chest-panel"],
        "symmetry": {
            "relation": "centered",
            "partner_id": None,
            "visibility": "observed",
            "confidence": 1,
            "evidence_refs": [evidence("scene")],
        },
        "visibility": "observed",
        "confidence": 1,
        "geometry": {
            "bbox": {
                "min": {"x": -0.5, "y": 0, "z": -0.25},
                "max": {"x": 0.5, "y": 1, "z": 0.25},
            },
            "dimensions": {"x": 1, "y": 1, "z": 0.5},
            "triangle_count": 128,
            "surface_area": 2.5,
        },
        "material_zone_ids": ["mat-shell"],
        "source_node_ids": ["node-main-body"],
        "editability": {
            "editable": True,
            "allowed_operations": ["profile", "transform"],
            "parameter_ids": ["body-width"],
            "constraints": [],
        },
        "evidence_refs": [evidence("artifact")],
    }
    common_metric = {
        "value": 0.4,
        "threshold": 0.9,
        "visibility": "observed",
        "confidence": 1,
        "evidence_sha256": EVIDENCE_HASH,
    }
    passes = ["beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch"]
    pass_artifacts = {
        name: {
            "pass": name,
            "artifact_sha256": HASH,
            "mime": "image/png",
            "width": 512,
            "height": 512,
            "channel_kind": "mask" if name == "silhouette" else "color",
        }
        for name in passes
    }
    fixtures: dict[str, dict[str, Any]] = {
        "semantic-scene-graph.schema.json": {
            "schema_version": "SemanticSceneGraph@1",
            "graph_id": "scene-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "artifact_sha256": HASH,
            "camera_hash": CAMERA_HASH,
            "render_set_sha256": EVIDENCE_HASH,
            "quality_report_sha256": CANONICAL_HASH,
            "observation": state(),
            "parts": [part],
            "material_zones": [{
                "material_zone_id": "mat-shell",
                "name": "Shell",
                "channels": ["base-color", "metallic", "roughness"],
                "surface_language": "matte painted shell",
                "asset_sha256": HASH,
                "observation": state(),
                "evidence_refs": [evidence("artifact")],
            }],
            "cameras": [{
                "camera_id": "camera-active",
                "role": "active",
                "camera_hash": CAMERA_HASH,
                "visibility": "observed",
                "confidence": 1,
                "evidence_refs": [evidence("camera", CAMERA_HASH)],
            }],
            "selection": {
                "selected_part_ids": ["main-body"],
                "selected_material_zone_ids": ["mat-shell"],
                "selected_camera_id": "camera-active",
                "isolation_mode": "part",
            },
            "claims": [{
                "claim_id": "claim-body",
                "subject_kind": "part",
                "subject_id": "main-body",
                "statement": "The main body is centered in the observed scene.",
                "visibility": "observed",
                "confidence": 1,
                "evidence_refs": [evidence("scene")],
            }],
            "canonical_sha256": CANONICAL_HASH,
        },
        "model-understanding-bundle.schema.json": {
            "schema_version": "ModelUnderstandingBundle@1",
            "bundle_id": "understanding-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "artifact_sha256": HASH,
            "camera_hash": CAMERA_HASH,
            "scene_graph_sha256": HASH,
            "render_set_sha256": EVIDENCE_HASH,
            "quality_report_sha256": CANONICAL_HASH,
            "observation": state(),
            "observations": [{
                "fact_id": "fact-body",
                "subject_kind": "part",
                "subject_id": "main-body",
                "statement": "The main body is the primary visible form.",
                "state": state(),
            }],
            "unknowns": [{
                "unknown_id": "unknown-back",
                "scope_kind": "region",
                "scope_id": "back-region",
                "question": "The back surface is not covered by the supplied reference.",
                "state": state("unknown"),
                "blocked_stages": ["final-review"],
            }],
            "decisions": [{
                "decision_id": "decision-primary",
                "stage": "primary-form",
                "statement": "Keep tertiary detail locked until the visible view passes.",
                "allowed_action_kind": "primary-form-adjustment",
                "state": state(),
            }],
            "canonical_sha256": CANONICAL_HASH,
        },
        "reference-canvas.schema.json": {
            "schema_version": "ReferenceCanvas@1",
            "canvas_id": "canvas-1",
            "project_id": "project-1",
            "reference_set_sha256": CANONICAL_HASH,
            "bindings": {
                "status": "unbound",
                "target_sha256": None,
                "camera_hash": None,
                "camera_canonical_sha256": None,
                "evidence_sha256": None,
            },
            "views": [{
                "view_id": "view-perspective",
                "reference_id": "reference-1",
                "reference_sha256": REFERENCE_HASH,
                "kind": "perspective",
                "authorization": {
                    "user_authorized": True,
                    "declaration": "The user authorized this reference for design evaluation.",
                    "evidence_refs": [evidence("reference", REFERENCE_HASH)],
                },
                "image_dimensions": {"width": 1024, "height": 1024},
                "target_sha256": None,
                "mask_sha256": None,
                "camera_claim": {
                    "visibility": "observed",
                    "camera_hash": CAMERA_HASH,
                    "camera_canonical_sha256": CANONICAL_HASH,
                    "claim": "The perspective camera is supplied by the reference evidence.",
                    "evidence_refs": [evidence("camera", CAMERA_HASH)],
                },
                "visible_regions": [{"region_id": "front-region", "label": "front shell", "state": state()}],
                "unknown_regions": [{
                    "region_id": "back-region",
                    "question": "Back geometry is not visible in this reference.",
                    "state": state("unknown"),
                }],
            }],
            "coverage": {
                "required_views": ["front", "back", "left", "right", "rear-three-quarter"],
                "supplied_views": ["perspective"],
                "missing_views": ["front", "back", "left", "right", "rear-three-quarter"],
                "coverage_status": "partial",
                "hq_360_status": "BLOCKED_REFERENCE_COVERAGE",
                "evidence_refs": [evidence("reference", REFERENCE_HASH)],
            },
            "unknowns": [{
                "unknown_id": "unknown-back",
                "scope_kind": "region",
                "scope_id": "back-region",
                "question": "Back geometry is not known from the supplied view.",
                "state": state("unknown"),
            }],
            "claims": [],
            "canonical_sha256": CANONICAL_HASH,
            "created_at": TIMESTAMP,
        },
        "design-spec.schema.json": {
            "schema_version": "DesignSpec@1",
            "spec_id": "spec-1",
            "project_id": "project-1",
            "reference_canvas_id": "canvas-1",
            "reference_canvas_sha256": CANONICAL_HASH,
            "category": "hard-surface visual asset",
            "style": "restrained industrial shell",
            "primary_forms": [{
                "form_id": "form-body",
                "name": "Main body",
                "role": "main-body",
                "description": "A centered primary housing establishes the visible silhouette.",
                "state": state(),
            }],
            "proportions": [{
                "proportion_id": "ratio-body",
                "subject_id": "main-body",
                "metric": "width-height",
                "target": 1,
                "tolerance": 0.05,
                "unit": "ratio",
                "state": state(),
            }],
            "semantic_parts": [{
                "part_id": "main-body",
                "role": "main-body",
                "parent_id": None,
                "symmetry": "centered",
                "material_zone_ids": ["mat-shell"],
                "state": state(),
            }],
            "material_language": [{
                "material_zone_id": "mat-shell",
                "surface_language": "matte painted shell",
                "color_family": "neutral graphite",
                "channels": ["base-color", "metallic", "roughness"],
                "state": state(),
            }],
            "stage_goals": [{
                "stage": "reference-canvas",
                "objective": "Record authorization, coverage, and unknown regions.",
                "allowed_action_kinds": ["reference-import", "coverage-annotation", "mark-unknown"],
                "forbidden_action_kinds": ["tertiary-detail", "uv-pbr", "export"],
                "exit_gate": gate("reference-canvas", "unknown"),
            }],
            "risks": [{
                "risk_id": "risk-coverage",
                "kind": "reference-coverage",
                "severity": "blocking",
                "description": "Missing orthographic views block a 360 review.",
                "state": state("unknown"),
            }],
            "unknowns": [{
                "unknown_id": "unknown-back",
                "question": "Which form continues around the hidden back surface?",
                "scope_kind": "region",
                "scope_id": "back-region",
                "state": state("unknown"),
                "blocked_stages": ["final-review"],
            }],
            "canonical_sha256": CANONICAL_HASH,
            "created_at": TIMESTAMP,
        },
        "design-session.schema.json": {
            "schema_version": "DesignSession@1",
            "session_id": "session-1",
            "project_id": "project-1",
            "design_spec_id": "spec-1",
            "design_spec_sha256": CANONICAL_HASH,
            "reference_canvas_id": "canvas-1",
            "reference_canvas_sha256": CANONICAL_HASH,
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "evidence_sha256": EVIDENCE_HASH,
            "observation_sha256": CANONICAL_HASH,
            "current_version_id": None,
            "current_version_sha256": None,
            "current_stage": "primary-form",
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "status": "blocked",
            "stage_gate": {
                **gate("primary-form", "fail"),
                "unlocks": ["checkpoint"],
            },
            "current_checkpoint_id": None,
            "current_checkpoint_sha256": None,
            "checkpoint_ids": [],
            "next_actions": [{
                "action_id": "action-primary",
                "stage": "primary-form",
                "action_kind": "primary-form-adjustment",
                "scope_kind": "part",
                "target_id": "main-body",
                "evidence_sha256": EVIDENCE_HASH,
                "bounded": True,
                "description": "Adjust the main body within the bounded primary-form range.",
            }],
            "rollback": {
                "relation": "none",
                "target_checkpoint_id": None,
                "target_checkpoint_sha256": None,
                "target_version_id": None,
                "target_version_sha256": None,
                "reason": None,
                "runtime_confirm_allowed": False,
            },
            "created_at": TIMESTAMP,
            "updated_at": TIMESTAMP,
            "canonical_sha256": CANONICAL_HASH,
        },
        "design-stage-plan.schema.json": {
            "schema_version": "DesignStagePlan@1",
            "plan_id": "plan-1",
            "plan_revision": 1,
            "session_id": "session-1",
            "project_id": "project-1",
            "design_spec_id": "spec-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "evidence_sha256": EVIDENCE_HASH,
            "stage": "primary-form",
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "status": "blocked",
            "gate": gate("primary-form", "fail"),
            "stage_policy": {
                "max_detail_level": "primary",
                "requires_passed_stages": ["reference-canvas"],
                "export_unlocked": False,
                "blocked_reason": "Visible-view quality remains below the strict gate.",
            },
            "allowed_actions": [{
                "action_id": "action-primary",
                "action_kind": "primary-form-adjustment",
                "stage": "primary-form",
                "scope_kind": "part",
                "target_id": "main-body",
                "bounded": True,
                "evidence_sha256": EVIDENCE_HASH,
                "description": "Adjust the main body within the bounded primary-form range.",
            }],
            "forbidden_action_kinds": ["tertiary-detail", "uv-pbr", "export"],
            "rollback_allowed": True,
            "canonical_sha256": CANONICAL_HASH,
        },
        "design-checkpoint.schema.json": {
            "schema_version": "DesignCheckpoint@1",
            "checkpoint_id": "checkpoint-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "stage": "primary-form",
            "checkpoint_type": "stage-fail",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "artifact_sha256": HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "evidence_sha256": EVIDENCE_HASH,
            "observation_sha256": CANONICAL_HASH,
            "version_id": "version-1",
            "version_sha256": HASH,
            "parent_checkpoint_id": None,
            "parent_checkpoint_sha256": None,
            "stage_gate": {
                **gate("primary-form", "fail"),
                "unlocks": ["checkpoint"],
            },
            "rollback": {
                "relation": "none",
                "target_checkpoint_id": None,
                "target_checkpoint_sha256": None,
                "target_version_id": None,
                "target_version_sha256": None,
                "reason": None,
            },
            "immutable": True,
            "runtime_write": False,
            "created_at": TIMESTAMP,
            "canonical_sha256": CANONICAL_HASH,
        },
        "design-critic-report.schema.json": {
            "schema_version": "DesignCriticReport@1",
            "report_id": "critic-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "evidence_sha256": EVIDENCE_HASH,
            "stage": "primary-form",
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "gate_status": "fail",
            "summary": "The visible contour remains below the strict primary-form gate.",
            "issues": [{
                "issue_id": "issue-silhouette",
                "scope": {"kind": "part", "part_id": "main-body"},
                "claim": "The main body contour is too narrow in the observed comparison.",
                "metric": {
                    "metric_name": "silhouette_iou",
                    "observed": 0.4,
                    "threshold": 0.9,
                    "unit": "ratio",
                    "direction": "minimum",
                    "visibility": "observed",
                    "confidence": 1,
                    "evidence_sha256": EVIDENCE_HASH,
                },
                "risk": "blocking",
                "status": "fail",
                "repair_intent_id": None,
                "repair_intent_sha256": None,
                "proposed_action": {
                    "action_kind": "bounded-repair",
                    "kit_id": "forgecad.kit.housing@1",
                    "operator_id": "forgecad.geometry.panel@1",
                    "parameter_id": "body-width",
                    "minimum": 0.8,
                    "maximum": 1.2,
                    "bounded": True,
                    "description": "Adjust the housing width within the bounded range.",
                },
            }],
            "canonical_sha256": CANONICAL_HASH,
        },
        "repair-intent.schema.json": {
            "schema_version": "RepairIntent@1",
            "intent_id": "repair-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "observation_sha256": EVIDENCE_HASH,
            "source_evidence_sha256": EVIDENCE_HASH,
            "source_critic_report_id": "critic-1",
            "source_critic_report_sha256": HASH,
            "stage": "primary-form",
            "scope": {"kind": "part", "part_id": "main-body"},
            "action": {
                "action_kind": "bounded-repair",
                "kit_id": "forgecad.kit.housing@1",
                "operator_id": "forgecad.geometry.panel@1",
                "operation": "adjust-parameter",
                "parameter_changes": [{
                    "parameter_id": "body-width",
                    "before": 1,
                    "after": 1.1,
                    "minimum": 0.8,
                    "maximum": 1.2,
                    "unit": "ratio",
                }],
                "bounded": True,
                "description": "Adjust one primary-form parameter for the main body.",
            },
            "precondition": {
                "failed_gate_id": "primary-silhouette",
                "quality_status": "QUALITY_TARGET_NOT_MET",
                "current_candidate_state_sha256": CANDIDATE_HASH,
                "evidence_sha256": EVIDENCE_HASH,
                "status": "failed",
            },
            "recompute": {
                "steps": ["compile", "readback", "render", "compare"],
                "must_rebind_reference": True,
                "must_rebind_camera": True,
                "confirm_allowed": False,
            },
            "rollback": {
                "relation": "none",
                "target_checkpoint_id": None,
                "target_checkpoint_sha256": None,
                "target_version_id": None,
                "target_version_sha256": None,
                "on_failure": "request-user",
                "reason": None,
            },
            "status": "proposed",
            "approval_required": True,
            "runtime_write": False,
            "canonical_sha256": CANONICAL_HASH,
        },
        "visual-evidence-bundle.schema.json": {
            "schema_version": "VisualEvidenceBundle@1",
            "bundle_id": "visual-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": CANDIDATE_HASH,
            "artifact_sha256": HASH,
            "reference_id": "reference-1",
            "reference_sha256": REFERENCE_HASH,
            "camera_hash": CAMERA_HASH,
            "render_set_sha256": EVIDENCE_HASH,
            "comparison_report_sha256": HASH,
            "quality_report_sha256": CANONICAL_HASH,
            "stage": "primary-form",
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "passes": passes,
            "pass_artifacts": pass_artifacts,
            "views": [{
                "view_id": "view-perspective",
                "kind": "perspective",
                "camera_hash": CAMERA_HASH,
                "render_set_sha256": EVIDENCE_HASH,
                "comparison_report_sha256": HASH,
                "visibility": "observed",
                "confidence": 1,
            }],
            "selection": {
                "part_ids": ["main-body"],
                "material_zone_ids": ["mat-shell"],
                "isolation_mode": "part",
                "selection_sha256": HASH,
            },
            "metrics": {name: copy.deepcopy(common_metric) for name in [
                "silhouette_iou", "boundary_f1_4px", "bbox_edge_error", "centroid_error", "landmark_coverage", "landmark_nme", "region_median_iou", "critical_region_min_iou"
            ]},
            "gate": {
                **gate("primary-form", "fail"),
                "unlocks": ["checkpoint"],
                "human_review_sha256": None,
                "export_restart_sha256": None,
            },
            "limitations": ["QUALITY_TARGET_NOT_MET remains the current visual result."],
            "canonical_sha256": CANONICAL_HASH,
        },
    }
    return fixtures


def make_action_run_fixture() -> dict[str, Any]:
    """Mirror the Runtime's blocked Primary Form action receipt shape."""
    return {
        "schema_version": "DesignActionRun@1",
        "run_id": "action-run-contract",
        "session_id": "session-1",
        "project_id": "project-1",
        "candidate_id": "candidate-1",
        "reference_id": "reference-1",
        "reference_sha256": REFERENCE_HASH,
        "camera_hash": CAMERA_HASH,
        "input_sha256": HASH,
        "observation_sha256": CANONICAL_HASH,
        "action": {
            "action_id": "action-primary",
            "action_kind": "bounded-repair",
            "scope_kind": "part",
            "target_id": "main-body",
            "operator_id": "forgecad.geometry.transform@2",
            "parameter_changes": [{
                "parameter_id": "body-width",
                "before": 1.0,
                "after": 1.05,
                "minimum": 0.5,
                "maximum": 1.5,
                "unit": "meter",
            }],
            "bounded": True,
            "description": "Adjust one bounded Primary Form parameter.",
        },
        "requested_stage": "primary-form",
        "status": "blocked",
        "completed_stage": "prepare",
        "stage_results": {
            "prepare": {"status": "completed", "output_sha256": HASH},
            "compile": {"status": "blocked", "error_code": "QUALITY_TARGET_NOT_MET"},
            "readback": {"status": "skipped"},
            "render": {"status": "skipped"},
            "evaluate": {"status": "skipped"},
        },
        "quality_status": "QUALITY_TARGET_NOT_MET",
        "failed_gates": ["primary-silhouette"],
        "allowed_actions": ["inspect", "retry", "bounded-repair", "checkpoint"],
        "locked_actions": ["confirm", "export", "next-stage"],
        "checkpoint_id": None,
        "checkpoint_hash": None,
        "runtime_write": False,
        "persistent_user_data_touched": False,
        "canonical_sha256": CANONICAL_HASH,
    }


def check_action_run_contract(manifest: dict[str, Any]) -> None:
    filename = "design-action-run.schema.json"
    schema = load_json(SCHEMA_ROOT / filename)
    require(schema.get("type") == "object" and schema.get("additionalProperties") is False, f"{filename} root is not closed")
    require(schema.get("properties", {}).get("schema_version", {}).get("const") == "DesignActionRun@1", f"{filename} version drifted")
    stage_status = schema.get("$defs", {}).get("stage_status", {}).get("enum", [])
    require("skipped" in stage_status, "DesignActionRun@1 omitted the Runtime skipped stage status")
    stage_result = schema.get("$defs", {}).get("stage_result", {})
    require(set(stage_result.get("required", [])) == {"status"}, "DesignActionRun@1 stage result required fields drifted")
    require("output_sha256" in stage_result.get("properties", {}), "DesignActionRun@1 omitted output_sha256")
    require("summary_sha256" in stage_result.get("properties", {}), "DesignActionRun@1 omitted summary_sha256")
    fixture = make_action_run_fixture()
    require(is_valid(schema, fixture), "Runtime-shaped blocked DesignActionRun fixture rejected")

    legacy = copy.deepcopy(fixture)
    legacy["stage_results"]["prepare"] = {"status": "completed", "hash": HASH, "reason": None}
    require(not is_valid(schema, legacy), "legacy hash/reason stage result was accepted")

    invalid_output = copy.deepcopy(fixture)
    invalid_output["stage_results"]["prepare"]["output_sha256"] = "not-a-sha256"
    require(not is_valid(schema, invalid_output), "invalid stage output hash was accepted")

    missing_observation = copy.deepcopy(fixture)
    missing_observation.pop("observation_sha256")
    require(not is_valid(schema, missing_observation), "action run without canonical observation binding was accepted")

    unpaired_checkpoint = copy.deepcopy(fixture)
    unpaired_checkpoint["stage_results"]["prepare"]["checkpoint_sha256"] = HASH
    require(not is_valid(schema, unpaired_checkpoint), "unpaired stage checkpoint hash was accepted")


def check_repair_intent_run_contract(manifest: dict[str, Any]) -> None:
    """Check the P2 CAS-bound run request/result pair and fail-closed flags."""
    registry = load_schema_registry(manifest)
    request_schema = load_json(SCHEMA_ROOT / "repair-intent-run-request.schema.json")
    result_schema = load_json(SCHEMA_ROOT / "repair-intent-run-result.schema.json")
    require(
        request_schema.get("$id") == "https://forgecad.local/contracts/repair-intent-run-request.schema.json"
        and result_schema.get("$id") == "https://forgecad.local/contracts/repair-intent-run-result.schema.json",
        "RepairIntent run schema IDs drifted",
    )
    require(
        request_schema["properties"]["approved"].get("const") is True
        and result_schema["properties"]["confirm_allowed"].get("const") is False
        and result_schema["properties"]["source_candidate_unchanged"].get("const") is True
        and result_schema["properties"]["runtime_write"].get("const") is False,
        "RepairIntent run approval/source mutation boundary drifted",
    )
    action = {
        "action_id": "repair-action-1",
        "action_kind": "bounded-repair",
        "scope_kind": "part",
        "target_id": "main-body",
        "operator_id": "forgecad.geometry.transform@2",
        "parameter_changes": [{
            "parameter_id": "body-width",
            "before": 1.0,
            "after": 1.05,
            "minimum": 0.5,
            "maximum": 1.5,
            "unit": "ratio",
        }],
        "bounded": True,
        "description": "Adjust one bounded body parameter.",
    }
    request = {
        "project_id": "project-1",
        "session_id": "session-1",
        "candidate_id": "candidate-1",
        "run_id": "run-1",
        "intent_sha256": HASH,
        "intent_object_sha256": HASH,
        "observation_sha256": EVIDENCE_HASH,
        "source_evidence_sha256": EVIDENCE_HASH,
        "reference_sha256": REFERENCE_HASH,
        "action": action,
        "proposal": {"geometry_program": {}, "view_spec": {}, "camera": {}},
        "requested_stage": "primary-form",
        "input_sha256": HASH,
        "approved": True,
        "approval_receipt_id": "approval-1",
        "approval_summary": "Approve one bounded CAS-bound RepairIntent run.",
        "approval_expires_at": "2030-01-01T00:00:00Z",
        "approval_session_id": "session-1",
        "idempotency_key": "repair-run-1",
    }
    require(is_valid(request_schema, request, registry), "positive RepairIntent run request rejected")
    unknown = copy.deepcopy(request)
    unknown["unexpected"] = True
    require(not is_valid(request_schema, unknown, registry), "RepairIntent run request accepted unknown field")
    unapproved = copy.deepcopy(request)
    unapproved["approved"] = False
    require(not is_valid(request_schema, unapproved, registry), "RepairIntent run request accepted without approval")

    result = {
        "schema_version": "RepairIntentRunResult@1",
        "project_id": "project-1",
        "session_id": "session-1",
        "candidate_id": "candidate-1",
        "run_id": "run-1",
        "intent_sha256": HASH,
        "intent_object_sha256": HASH,
        "input_sha256": HASH,
        "observation_sha256": EVIDENCE_HASH,
        "source_evidence_sha256": EVIDENCE_HASH,
        "reference_sha256": REFERENCE_HASH,
        "status": "blocked",
        "run_status": "completed",
        "quality_status": "QUALITY_TARGET_NOT_MET",
        "action_run_sha256": HASH,
        "action_run": {},
        "proposal_candidate_id": "candidate-2",
        "proposal_candidate_state_sha256": HASH,
        "prepared_object_sha256": HASH,
        "quality_report_id": HASH,
        "apply_status": "blocked",
        "next_transaction": "inspect_or_retry",
        "confirm_allowed": False,
        "source_candidate_unchanged": True,
        "active_design_state_mutated": False,
        "runtime_write": False,
        "persistent_user_data_touched": False,
        "canonical_sha256": CANONICAL_HASH,
    }
    require(is_valid(result_schema, result), "positive RepairIntent run result rejected")
    unsafe_result = copy.deepcopy(result)
    unsafe_result["confirm_allowed"] = True
    require(not is_valid(result_schema, unsafe_result), "RepairIntent run result accepted confirm_allowed=true")


def set_path(value: Any, path: tuple[Any, ...], replacement: Any) -> None:
    cursor = value
    for key in path[:-1]:
        cursor = cursor[key]
    cursor[path[-1]] = replacement


def assert_binding_fixtures(
    fixtures: dict[str, dict[str, Any]],
    schemas: dict[str, dict[str, Any]],
) -> None:
    session = fixtures["design-session.schema.json"]
    checkpoint = fixtures["design-checkpoint.schema.json"]
    plan = fixtures["design-stage-plan.schema.json"]
    require(session["candidate_id"] == checkpoint["candidate_id"] == plan["candidate_id"], "positive candidate binding drifted")
    require(session["reference_id"] == checkpoint["reference_id"] == plan["reference_id"], "positive reference binding drifted")
    require(session["reference_sha256"] == checkpoint["reference_sha256"] == plan["reference_sha256"] == REFERENCE_HASH, "positive reference hash binding drifted")
    require(session["camera_hash"] == checkpoint["camera_hash"] == plan["camera_hash"] == CAMERA_HASH, "positive camera binding drifted")
    require(session["evidence_sha256"] == checkpoint["evidence_sha256"] == plan["evidence_sha256"] == EVIDENCE_HASH, "positive evidence binding drifted")

    mismatch = copy.deepcopy(session)
    mismatch["reference_sha256"] = HASH
    require(mismatch["reference_sha256"] != fixtures["reference-canvas.schema.json"]["views"][0]["reference_sha256"], "negative reference mismatch fixture was not created")
    require(mismatch["candidate_id"] == checkpoint["candidate_id"], "negative binding fixture lost candidate identity")

    view_pair_mismatch = copy.deepcopy(fixtures["reference-canvas.schema.json"])
    view_pair_mismatch["views"][0]["target_sha256"] = HASH
    require(
        not is_valid(schemas["reference-canvas.schema.json"], view_pair_mismatch),
        "per-view target without mask was accepted",
    )

    target_without_view_spec = copy.deepcopy(fixtures["reference-canvas.schema.json"])
    target_without_view_spec["views"][0]["target_sha256"] = HASH
    target_without_view_spec["views"][0]["mask_sha256"] = HASH
    require(
        not is_valid(schemas["reference-canvas.schema.json"], target_without_view_spec),
        "per-view target/mask without ReferenceViewSpec was accepted",
    )

    unknown_camera_canonical = copy.deepcopy(fixtures["reference-canvas.schema.json"])
    unknown_camera_canonical["views"][0]["camera_claim"] = {
        "visibility": "unknown",
        "camera_hash": None,
        "camera_canonical_sha256": CANONICAL_HASH,
        "claim": "Camera is not known.",
        "evidence_refs": [evidence("reference", REFERENCE_HASH)],
    }
    require(
        not is_valid(schemas["reference-canvas.schema.json"], unknown_camera_canonical),
        "unknown camera with canonical hash was accepted",
    )


def main() -> int:
    manifest = load_json(MANIFEST)
    declared = set(manifest.get("schemas", []))
    require(set(EXPECTED) <= declared, "manifest is missing one or more agentic schemas")
    require("design-action-run.schema.json" in declared, "manifest is missing DesignActionRun@1")
    check_action_run_contract(manifest)
    require(set(REPAIR_INTENT_RUN_SCHEMAS) <= declared, "manifest is missing RepairIntent run schemas")
    check_repair_intent_run_contract(manifest)

    schemas: dict[str, dict[str, Any]] = {}
    for filename, version in EXPECTED.items():
        path = SCHEMA_ROOT / filename
        require(path.exists(), f"missing {filename}")
        schema = load_json(path)
        schemas[filename] = schema
        check_schema_shape(filename, schema)
        require(schema["properties"]["schema_version"]["const"] == version, f"{filename} version drifted")

    fixtures = make_fixtures()
    require(set(fixtures) == set(EXPECTED), "positive fixture set is incomplete")
    for filename, schema in schemas.items():
        positive = fixtures[filename]
        require(is_valid(schema, positive), f"positive fixture rejected: {filename}")

        extra = copy.deepcopy(positive)
        extra["unexpected"] = True
        require(not is_valid(schema, extra), f"top-level additional property accepted: {filename}")

        bad_hash = copy.deepcopy(positive)
        bad_hash["canonical_sha256"] = "not-a-sha256"
        require(not is_valid(schema, bad_hash), f"invalid canonical hash accepted: {filename}")

    # A complete coverage claim must name the five identity views. A
    # perspective supplement cannot stand in for rear-three-quarter; otherwise
    # an authoring payload could unlock HQ_360 with an incomplete view set.
    incomplete_hq = copy.deepcopy(fixtures["reference-canvas.schema.json"])
    incomplete_hq["coverage"] = {
        **incomplete_hq["coverage"],
        "required_views": ["front", "back", "left", "right", "perspective"],
        "supplied_views": ["front", "back", "left", "right", "perspective"],
        "missing_views": [],
        "coverage_status": "complete",
        "hq_360_status": "eligible",
    }
    require(
        not is_valid(schemas["reference-canvas.schema.json"], incomplete_hq),
        "complete ReferenceCanvas coverage accepted without rear-three-quarter",
    )

    safety_targets: dict[str, tuple[Any, ...]] = {
        "semantic-scene-graph.schema.json": ("parts", 0, "name"),
        "model-understanding-bundle.schema.json": ("observations", 0, "statement"),
        "reference-canvas.schema.json": ("views", 0, "authorization", "declaration"),
        "design-spec.schema.json": ("category",),
        "design-session.schema.json": ("next_actions", 0, "description"),
        "design-stage-plan.schema.json": ("allowed_actions", 0, "description"),
        "design-checkpoint.schema.json": ("rollback", "reason"),
        "design-critic-report.schema.json": ("summary",),
        "repair-intent.schema.json": ("action", "description"),
        "visual-evidence-bundle.schema.json": ("limitations", 0),
    }
    for filename, target in safety_targets.items():
        bad_text = copy.deepcopy(fixtures[filename])
        if filename == "design-checkpoint.schema.json":
            bad_text["rollback"].update({
                "relation": "rollback-source",
                "target_checkpoint_id": "checkpoint-2",
                "target_checkpoint_sha256": HASH,
                "reason": "/tmp/forbidden/reference.png",
            })
        else:
            set_path(bad_text, target, "password: leaked-value")
        require(not is_valid(schemas[filename], bad_text), f"path/secret text accepted: {filename}")

    raw_bytes = copy.deepcopy(fixtures["visual-evidence-bundle.schema.json"])
    raw_bytes["raw_bytes"] = "AA=="
    require(not is_valid(schemas["visual-evidence-bundle.schema.json"], raw_bytes), "raw bytes field accepted")

    assert_binding_fixtures(fixtures, schemas)
    session = copy.deepcopy(fixtures["design-session.schema.json"])
    session["rollback"]["relation"] = "requested"
    session["rollback"]["target_checkpoint_id"] = None
    session["rollback"]["target_checkpoint_sha256"] = None
    session["rollback"]["target_version_id"] = None
    session["rollback"]["target_version_sha256"] = None
    session["rollback"]["reason"] = "No rollback target"
    require(not is_valid(schemas["design-session.schema.json"], session), "rollback without a target accepted")

    stage_plan = copy.deepcopy(fixtures["design-stage-plan.schema.json"])
    stage_plan["stage_policy"]["export_unlocked"] = True
    require(not is_valid(schemas["design-stage-plan.schema.json"], stage_plan), "export unlocked while quality target is not met")

    repair = copy.deepcopy(fixtures["repair-intent.schema.json"])
    repair["recompute"]["confirm_allowed"] = True
    require(not is_valid(schemas["repair-intent.schema.json"], repair), "repair intent can confirm before recompute")

    print(f"Agentic contracts OK: {len(EXPECTED) + len(REPAIR_INTENT_RUN_SCHEMAS) + 1} schemas; positive and negative fixtures passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
