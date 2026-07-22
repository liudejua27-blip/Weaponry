#!/usr/bin/env python3
"""Export FastAPI OpenAPI and generated desktop API component types."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List

from generate_schema_types import ROOT, ts_type


OPENAPI_OUT = ROOT / "packages" / "weapon-spec" / "generated" / "openapi.json"
API_TS_OUT = ROOT / "apps" / "desktop" / "src" / "shared" / "generated" / "api-types.ts"
RUST_NATIVE_COMPONENT_RECIPE_PATH = (
    "/api/v1/agent/asset-versions/{asset_version_id}/parts/{part_id}/component-recipes:expand"
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    openapi = build_openapi()
    outputs = {
        OPENAPI_OUT: json.dumps(openapi, ensure_ascii=False, indent=2, sort_keys=True)
        + "\n",
        API_TS_OUT: render_api_types(openapi),
    }

    if args.check:
        stale = [
            path
            for path, content in outputs.items()
            if not path.exists() or path.read_text(encoding="utf-8") != content
        ]
        if stale:
            for path in stale:
                print(
                    f"stale generated artifact: {path.relative_to(ROOT)}",
                    file=sys.stderr,
                )
            return 1
        print("openapi generated artifacts ok")
        return 0

    for path, content in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        print(f"wrote {path.relative_to(ROOT)}")
    return 0


def build_openapi() -> Dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="wushen_openapi_") as tmp:
        sys.path.insert(0, str(ROOT / "apps" / "agent"))
        # Contract generation must be hermetic.  A developer may have a real
        # Library path exported in their shell, and that location can be
        # unavailable in CI/sandboxed checks.  OpenAPI does not need user data,
        # so temporarily force an isolated library and then restore the caller's
        # environment exactly.
        overrides = {
            "WUSHEN_LIBRARY_ROOT": str(Path(tmp) / "WushenForgeLibrary"),
            "WUSHEN_MIGRATIONS_DIR": str(ROOT / "migrations"),
            "WUSHEN_RECOVER_ON_STARTUP": "0",
            "FORGECAD_CONCEPT_RECOVER_ON_STARTUP": "0",
            "WUSHEN_LOCAL_WORKER_ENABLED": "0",
            "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
            "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE": "1",
            "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE": "1",
            "FORGECAD_K001_PACKAGED_PROBE": "1",
        }
        previous = {name: os.environ.get(name) for name in overrides}
        os.environ.update(overrides)
        try:
            from wushen_agent.main import (
                create_test_only_legacy_product_core_app,
            )

            openapi = create_test_only_legacy_product_core_app().openapi()
            return apply_rust_native_openapi_overlay(openapi)
        finally:
            for name, value in previous.items():
                if value is None:
                    os.environ.pop(name, None)
                else:
                    os.environ[name] = value


def apply_rust_native_openapi_overlay(openapi: Dict[str, Any]) -> Dict[str, Any]:
    """Add code-owned Rust product routes without mounting them in FastAPI.

    K003 intentionally removed Python product-state ownership.  The generated
    compatibility OpenAPI still needs to describe bounded Rust-native routes,
    so this explicit overlay is the single contract source for those paths. A
    collision fails generation: adding the route to FastAPI would silently
    recreate a second product owner.
    """

    paths = openapi.setdefault("paths", {})
    if RUST_NATIVE_COMPONENT_RECIPE_PATH in paths:
        raise RuntimeError(
            "Rust-native Component Recipe route must not be mounted by FastAPI"
        )
    schemas = openapi.setdefault("components", {}).setdefault("schemas", {})
    overlay_schemas = rust_native_component_recipe_schemas()
    collisions = sorted(set(overlay_schemas).intersection(schemas))
    if collisions:
        raise RuntimeError(
            "Rust-native OpenAPI component collision: " + ", ".join(collisions)
        )
    schemas.update(overlay_schemas)
    augment_agent_part_edit_operation(schemas)
    paths[RUST_NATIVE_COMPONENT_RECIPE_PATH] = {
        "post": {
            "tags": ["Rust-native C105"],
            "summary": "Expand a reviewed Component Recipe into a transient candidate",
            "description": (
                "Rust core derives Project, base AgentAssetVersion and Snapshot revision "
                "from the active design, validates the exact reviewed Recipe/slot refs, "
                "and returns a deterministic ComponentRecipeCandidate@1. This operation "
                "does not write SQLite, CAS, objects, Version, head, Snapshot, ChangeSet, "
                "quality, export, or a temporary GLB. It neither reads nor requires a "
                "Provider/Keychain credential. Python is not a handler or fallback."
            ),
            "operationId": "rust_native_expand_component_recipe_candidate",
            "x-forgecad-owner": "rust-core",
            "x-forgecad-zero-write": True,
            "x-forgecad-python-fallback": False,
            "parameters": [
                {
                    "name": "asset_version_id",
                    "in": "path",
                    "required": True,
                    "schema": {
                        "type": "string",
                        "pattern": "^assetver_[a-z0-9_\\-]+$",
                    },
                },
                {
                    "name": "part_id",
                    "in": "path",
                    "required": True,
                    "schema": {
                        "type": "string",
                        "pattern": "^part_[a-z0-9_\\-]+$",
                    },
                },
            ],
            "requestBody": {
                "required": True,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/ComponentRecipeActiveCandidateRequest"
                        }
                    }
                },
            },
            "responses": {
                "200": {
                    "description": "Rust-expanded, transient and still unconfirmed candidate",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ComponentRecipeCandidate"
                            }
                        }
                    },
                },
                "400": {
                    "description": "COMPONENT_RECIPE_REQUEST_INVALID or bounded contract failure",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/RustNativeProductError"
                            }
                        }
                    },
                },
                "404": {
                    "description": "Asset, Snapshot, Part or reviewed Recipe ref not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/RustNativeProductError"
                            }
                        }
                    },
                },
                "409": {
                    "description": (
                        "COMPONENT_RECIPE_CONTEXT_STALE, COMPONENT_RECIPE_CANDIDATE_STALE, "
                        "COMPONENT_RECIPE_PREVIEW_PENDING, lock, domain, registry/hash or "
                        "active-head conflict; no product write occurs"
                    ),
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/RustNativeProductError"
                            }
                        }
                    },
                },
            },
        }
    }
    return openapi


def rust_native_component_recipe_schemas() -> Dict[str, Dict[str, Any]]:
    component_recipe_ref = {
        "title": "ComponentRecipeRef",
        "type": "object",
        "additionalProperties": False,
        "required": ["schema_version", "recipe_id", "version", "recipe_sha256"],
        "properties": {
            "schema_version": {"const": "ComponentRecipeRef@1"},
            "recipe_id": {
                "type": "string",
                "pattern": "^recipe_[a-z0-9_\\-]+$",
            },
            "version": {"type": "integer", "minimum": 1, "maximum": 9999},
            "recipe_sha256": {
                "type": "string",
                "pattern": "^[a-f0-9]{64}$",
            },
        },
    }
    recipe_slot_binding = {
        "title": "RecipeSlotBinding",
        "description": (
            "Enables only the fixed reviewed child declared by this parent Recipe slot; "
            "it is not an arbitrary replacement selector."
        ),
        "type": "object",
        "additionalProperties": False,
        "required": ["slot_id", "child_recipe"],
        "properties": {
            "slot_id": {
                "type": "string",
                "pattern": "^slot_[a-z0-9_\\-]+$",
            },
            "child_recipe": {"$ref": "#/components/schemas/ComponentRecipeRef"},
        },
    }
    return {
        "ComponentRecipeRef": component_recipe_ref,
        "RecipeSlotBinding": recipe_slot_binding,
        "RecipeParameterValue": {
            "title": "RecipeParameterValue",
            "type": "object",
            "additionalProperties": False,
            "required": ["parameter_id", "value"],
            "properties": {
                "parameter_id": {
                    "type": "string",
                    "pattern": "^editparam_[a-z0-9_\\-]+$",
                },
                "value": {"type": "number", "minimum": -10000, "maximum": 10000},
            },
        },
        "RecipeMaterialZoneOverride": {
            "title": "RecipeMaterialZoneOverride",
            "type": "object",
            "additionalProperties": False,
            "required": ["zone_id", "material_preset_id"],
            "properties": {
                "zone_id": {
                    "type": "string",
                    "pattern": "^zone_[a-z0-9_\\-]+$",
                },
                "material_preset_id": {
                    "type": "string",
                    "pattern": "^mat_[a-z0-9_\\-]+$",
                },
            },
        },
        "ComponentRecipeActiveCandidateRequest": {
            "title": "ComponentRecipeActiveCandidateRequest",
            "description": (
                "Bounded zero-write request for Rust core. Free parameter and material "
                "overrides are explicitly empty in C105 v1."
            ),
            "type": "object",
            "additionalProperties": False,
            "required": [
                "schema_version",
                "recipe_request_id",
                "component_recipe_ref",
                "slot_bindings",
                "parameter_values",
                "material_zone_overrides",
            ],
            "properties": {
                "schema_version": {
                    "const": "ComponentRecipeActiveCandidateRequest@1"
                },
                "recipe_request_id": {
                    "type": "string",
                    "pattern": "^recipereq_[a-z0-9_\\-]+$",
                },
                "component_recipe_ref": {
                    "$ref": "#/components/schemas/ComponentRecipeRef"
                },
                "slot_bindings": {
                    "type": "array",
                    "maxItems": 12,
                    "uniqueItems": True,
                    "items": {"$ref": "#/components/schemas/RecipeSlotBinding"},
                },
                "parameter_values": {
                    "description": "Must be empty in C105 v1.",
                    "type": "array",
                    "maxItems": 0,
                    "items": {"$ref": "#/components/schemas/RecipeParameterValue"},
                },
                "material_zone_overrides": {
                    "description": "Must be empty in C105 v1.",
                    "type": "array",
                    "maxItems": 0,
                    "items": {
                        "$ref": "#/components/schemas/RecipeMaterialZoneOverride"
                    },
                },
            },
        },
        "ComponentRecipeInstanceProvenance": {
            "title": "ComponentRecipeInstanceProvenance",
            "type": "object",
            "additionalProperties": False,
            "required": [
                "schema_version",
                "instance_id",
                "instance_path",
                "recipe",
                "registry_sha256",
                "policy_version",
                "parent_instance_id",
                "parent_slot_id",
                "domain_pack_id",
                "source",
                "license",
                "review_state",
                "quality_status",
                "non_functional_only",
            ],
            "properties": {
                "schema_version": {"const": "ComponentRecipeInstanceProvenance@1"},
                "instance_id": {
                    "type": "string",
                    "pattern": "^recipeinst_[a-z0-9_\\-]+$",
                },
                "instance_path": {"type": "string"},
                "recipe": {"$ref": "#/components/schemas/ComponentRecipeRef"},
                "registry_sha256": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{64}$",
                },
                "policy_version": {"const": "ComponentRecipePolicy@1"},
                "parent_instance_id": {"type": ["string", "null"]},
                "parent_slot_id": {"type": ["string", "null"]},
                "domain_pack_id": {"type": "string"},
                "source": {"type": "object", "additionalProperties": True},
                "license": {"type": "object", "additionalProperties": True},
                "review_state": {"type": "object", "additionalProperties": True},
                "quality_status": {"const": "passed"},
                "non_functional_only": {"const": True},
            },
        },
        "ComponentRecipeCandidate": {
            "title": "ComponentRecipeCandidate",
            "description": (
                "ComponentRecipeCandidate@1 is transient Rust expansion evidence, not a "
                "Version, ChangeSet preview, quality result, export, or compiled GLB."
            ),
            "type": "object",
            "additionalProperties": False,
            "required": [
                "schema_version",
                "candidate_id",
                "request_id",
                "project_id",
                "context_mode",
                "base_asset_version_id",
                "snapshot_revision",
                "target_part_id",
                "recipe",
                "instance_path",
                "changeset_id",
                "expanded_shape_program",
                "expanded_assembly_graph",
                "component_recipe_instances",
                "registry_sha256",
                "candidate_sha256",
                "status",
                "quality_profile",
                "non_functional_only",
            ],
            "properties": {
                "schema_version": {"const": "ComponentRecipeCandidate@1"},
                "candidate_id": {
                    "type": "string",
                    "pattern": "^recipecandidate_[a-z0-9_\\-]+$",
                },
                "request_id": {
                    "type": "string",
                    "pattern": "^recipereq_[a-z0-9_\\-]+$",
                },
                "project_id": {"type": ["string", "null"]},
                "context_mode": {
                    "type": "string",
                    "enum": ["initial_candidate", "active_asset_edit"],
                },
                "base_asset_version_id": {"type": ["string", "null"]},
                "snapshot_revision": {"type": ["integer", "null"], "minimum": 1},
                "target_part_id": {"type": ["string", "null"]},
                "recipe": {"$ref": "#/components/schemas/ComponentRecipeRef"},
                "instance_path": {"type": "string"},
                "changeset_id": {"type": ["string", "null"]},
                "expanded_shape_program": {
                    "type": "object",
                    "additionalProperties": True,
                },
                "expanded_assembly_graph": {
                    "type": "object",
                    "additionalProperties": True,
                },
                "component_recipe_instances": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 128,
                    "items": {
                        "$ref": "#/components/schemas/ComponentRecipeInstanceProvenance"
                    },
                },
                "registry_sha256": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{64}$",
                },
                "candidate_sha256": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{64}$",
                },
                "status": {"type": "string", "enum": ["expanded", "rejected"]},
                "quality_profile": {
                    "type": "string",
                    "enum": ["interactive_preview", "production_concept"],
                },
                "non_functional_only": {"const": True},
            },
        },
        "RustNativeProductError": {
            "title": "RustNativeProductError",
            "type": "object",
            "additionalProperties": False,
            "required": ["error"],
            "properties": {
                "error": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["code", "message", "recoverable", "details"],
                    "properties": {
                        "code": {"type": "string"},
                        "message": {"type": "string"},
                        "recoverable": {"type": "boolean"},
                        "details": {"type": "object", "additionalProperties": True},
                    },
                }
            },
        },
    }


def augment_agent_part_edit_operation(schemas: Dict[str, Any]) -> None:
    """Expose the sealed C105 replace_part replay fields in generated clients."""

    operation = schemas.get("AgentPartEditOperation")
    if not isinstance(operation, dict):
        raise RuntimeError("OpenAPI is missing AgentPartEditOperation")
    properties = operation.get("properties")
    if not isinstance(properties, dict):
        raise RuntimeError("AgentPartEditOperation is missing properties")
    additions = {
        "recipe_request_id": {
            "type": "string",
            "pattern": "^recipereq_[a-z0-9_\\-]+$",
            "description": "Sealed Recipe expansion request identity for replace_part.",
        },
        "component_recipe_ref": {
            "$ref": "#/components/schemas/ComponentRecipeRef",
            "description": "Exact reviewed Recipe ID/version/hash sealed by the candidate.",
        },
        "recipe_slot_bindings": {
            "type": "array",
            "maxItems": 12,
            "uniqueItems": True,
            "items": {"$ref": "#/components/schemas/RecipeSlotBinding"},
            "description": "Exact fixed child-slot bindings sealed by expansion.",
        },
        "recipe_candidate_id": {
            "type": "string",
            "pattern": "^recipecandidate_[a-z0-9_\\-]+$",
        },
        "recipe_candidate_sha256": {
            "type": "string",
            "pattern": "^[a-f0-9]{64}$",
        },
        "recipe_snapshot_revision": {
            "type": "integer",
            "minimum": 1,
            "description": "Snapshot revision sealed by the zero-write expansion.",
        },
    }
    duplicate = sorted(set(additions).intersection(properties))
    if duplicate:
        raise RuntimeError(
            "FastAPI unexpectedly owns C105 replace_part fields: " + ", ".join(duplicate)
        )
    properties.update(additions)


def render_api_types(openapi: Dict[str, Any]) -> str:
    schemas = openapi.get("components", {}).get("schemas", {})
    lines: List[str] = [
        "/* eslint-disable */",
        "// Generated by scripts/export_openapi.py. Do not edit by hand.",
        "",
    ]
    for name in sorted(schemas):
        lines.append(f"export type {name} = {ts_type(schemas[name], required=True)}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


if __name__ == "__main__":
    sys.exit(main())
