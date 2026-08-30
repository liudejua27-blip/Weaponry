#!/usr/bin/env python3
"""Materialize the D1 weapon AppearanceProgram@3 review candidate and AOVs.

The Runtime remains the sole writer.  This helper only constructs closed typed
requests from already persisted project/candidate/CAS identities, and never
confirms, versions, exports, or submits a human review.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

from probe_mcp010c_codex_cli import canonical_hash
from probe_production_weapon_form_art_repair_execution_d1 import (
    close_client,
    open_client,
    read_cas_json,
    require,
)

PROJECT_ID = "project-0d236b8acdde4f1187b3a46a7d5e4f0f"
REFERENCE_ID = "reference-c0ea57e80e1d4c37b7e65353d08b6e74"
REFERENCE_SHA256 = "1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109"
SOURCE_CANDIDATE_ID = "candidate-50b6981546f74bca9c2ca774ac5c1b00"
SOURCE_CANDIDATE_STATE = "b70f01f6f60fffe7ce42c92f33b95d8687a70ee3b97d291ccd3a18b70255055c"
SOURCE_ARTIFACT_SHA256 = "3caa4a87ae078635dc50b4512a39732fe919df822985a1c60fe5415757bd7596"
GEOMETRY_PROGRAM_SHA256 = "8e78cc571a9a3ddf05f9f0c96f644bf30bd5a40401c8d889be71a40c62694732"
REFERENCE_CANVAS_SHA256 = "7662da20653dd147d1674bd4b8a67cd39c797e767d41ae775317af760756b7cd"
CAMERA_OBJECT_SHA256 = "7a3fc4d53c262141cfe99bd99a033c3d7c344127c84863882c97ff294db356c0"
RETOPOLOGY_CAGE_BUNDLE_SHA256 = "c73271da1b43cfa33f7848bcf463613d29871bfb4244c5d8c934d8b07fbdcd58"
LOW_ARTIFACT_SHA256 = "28bba1541e30a7bb4109f737eb548cfa854fa295c5a5079263c1e8f93736596e"
CAGE_ARTIFACT_SHA256 = "338cabf1b3753bdcf1eb3c54b6d35909896c8e6eb13e50dfd5c416e1e216a1c3"
PACK_ID = "forgecad-fictional-energy-weapon-2k"


def canonical_object_hash(value: dict[str, Any]) -> str:
    preimage = dict(value)
    preimage.pop("canonical_sha256", None)
    return canonical_hash(preimage)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=360.0)
    return parser.parse_args()


def exact_rear_three_quarter(data_root: Path) -> tuple[dict[str, Any], str]:
    canvas = read_cas_json(data_root, REFERENCE_CANVAS_SHA256)
    for view in canvas.get("views", []):
        if isinstance(view, dict) and view.get("kind") == "rear-three-quarter":
            spec = view.get("view_spec")
            require(isinstance(spec, dict), "rear-three-quarter ViewSpec is unavailable")
            require(
                spec.get("canonical_sha256")
                == "59d8f7f8866f2777206e62e2027630adcc76dd98f31d4db5a655a9789aacb8f5",
                "rear-three-quarter ViewSpec hash differs",
            )
            target = view.get("target_sha256")
            require(isinstance(target, str), "rear-three-quarter target is unavailable")
            return spec, target
    raise RuntimeError("rear-three-quarter reference view is unavailable")


def appearance_program(
    geometry: dict[str, Any], geometry_program_sha256: str, manifest_sha256: str
) -> dict[str, Any]:
    zone_parts: dict[str, list[str]] = {}
    for output in geometry.get("part_outputs", []):
        require(isinstance(output, dict), "GeometryProgram Part output is invalid")
        part_id = output.get("part_id")
        zone_id = output.get("material_zone_id")
        require(isinstance(part_id, str) and isinstance(zone_id, str), "Part/MaterialZone binding is invalid")
        zone_parts.setdefault(zone_id, []).append(part_id)
    expected = {
        "zone-white-shell",
        "zone-black-mechanical",
        "zone-gold-accent",
        "zone-amber-emissive",
    }
    require(set(zone_parts) == expected, f"unexpected MaterialZone inventory: {sorted(zone_parts)}")
    recipes = {
        "zone-white-shell": ("energy-white-clearcoat", "weapon-plastic-surface"),
        "zone-black-mechanical": ("energy-black-anodized", "weapon-metal-surface"),
        "zone-gold-accent": ("energy-brushed-gold", "weapon-metal-surface"),
        "zone-amber-emissive": ("energy-cyan-emissive", None),
    }
    material_zones = [
        {
            "zone_id": zone_id,
            "part_ids": sorted(zone_parts[zone_id]),
            "material_id": recipes[zone_id][0],
            "texture_set_id": recipes[zone_id][1],
        }
        for zone_id in sorted(zone_parts)
    ]
    all_parts = sorted(part for parts in zone_parts.values() for part in parts)
    all_zones = sorted(zone_parts)
    stack: dict[str, Any] = {
        "schema_version": "MaterialLayerStack@1",
        "stack_id": "d1-hero-surface-layer-stack-v1",
        "material_pack_id": PACK_ID,
        "material_pack_manifest_sha256": manifest_sha256,
        "uv_source": "TEXCOORD_0",
        "layers": [
            {
                "layer_id": "fictional-safety-markings",
                "order": 0,
                "kind": "decal",
                "recipe_id": "forgecad-first-party-fictional-safety-markings@1",
                "blend_policy": "precompose-baseColor-no-custom-shader",
                "targets": {
                    "part_ids": sorted(zone_parts["zone-white-shell"] + zone_parts["zone-gold-accent"]),
                    "material_zone_ids": ["zone-white-shell", "zone-gold-accent"],
                },
                "opacity": 0.78,
            },
            {
                "layer_id": "bounded-edge-wear",
                "order": 1,
                "kind": "wear",
                "recipe_id": "forgecad-first-party-geometry-edge-ao-wear@1",
                "blend_policy": "precompose-baseColor-metallicRoughness-no-custom-shader",
                "targets": {"part_ids": all_parts, "material_zone_ids": all_zones},
                "edge_width_texels": 10,
                "strength": 0.42,
            },
            {
                "layer_id": "texture-backed-clearcoat",
                "order": 2,
                "kind": "clearcoat",
                "recipe_id": "forgecad-first-party-zone-clearcoat-mask@1",
                "blend_policy": "KHR_materials_clearcoat",
                "targets": {
                    "part_ids": sorted(zone_parts["zone-white-shell"]),
                    "material_zone_ids": ["zone-white-shell"],
                },
                "factor": 0.86,
                "roughness": 0.11,
            },
        ],
        "budget": {
            "resolution": 2048,
            "padding_texels": 8,
            "max_output_textures": 8,
            "max_output_bytes": 67108864,
            "max_runtime_ms": 120000,
        },
        "canonical_sha256": "",
    }
    stack["canonical_sha256"] = canonical_object_hash(stack)
    program: dict[str, Any] = {
        "schema_version": "AppearanceProgram@3",
        "project_id": PROJECT_ID,
        "geometry_program_sha256": geometry_program_sha256,
        "material_pack_id": PACK_ID,
        "material_pack_manifest_sha256": manifest_sha256,
        "material_zones": material_zones,
        "material_layer_stack": stack,
        "material_layer_stack_sha256": stack["canonical_sha256"],
        "canonical_sha256": "",
    }
    program["canonical_sha256"] = canonical_object_hash(program)
    return program


def main() -> int:
    args = parse_args()
    repository = Path(__file__).resolve().parents[1]
    output = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    output.resolve().relative_to((repository / "docs" / "evidence").resolve())
    geometry = read_cas_json(args.data_root, GEOMETRY_PROGRAM_SHA256)
    # Six embedded 2K layer textures require the product's already supported
    # 64 MiB appearance budget.  This changes only the declared resource
    # envelope; geometry nodes, Parts, MaterialZones and topology stay exact.
    geometry["budgets"]["max_glb_bytes"] = 64 * 1024 * 1024
    appearance_geometry_program_sha256 = canonical_object_hash(geometry)
    geometry["canonical_sha256"] = appearance_geometry_program_sha256
    view_spec, target_sha256 = exact_rear_three_quarter(args.data_root)
    camera = read_cas_json(args.data_root, CAMERA_OBJECT_SHA256)

    with tempfile.TemporaryDirectory(prefix="forgecad-04bh-", dir="/tmp") as temporary:
        runtime, ready_path, ready, client = open_client(
            args.mcp,
            args.runtime,
            args.data_root,
            Path(temporary) / "ipc",
            args.timeout,
        )
        try:
            capabilities = client.tool("capabilities_get")
            runtime_status = client.tool("runtime_status")
            catalog = client.tool("operator_catalog_get")
            skills = client.tool("skill_list")
            uv_pbr = client.tool("skill_get", {"skill_id": "uv-pbr", "version": "0.2.0"})
            pack = client.tool("material_pack_get", {"pack_id": PACK_ID})
            require(runtime_status.get("state") == "Ready", "Runtime is not Ready")
            require(capabilities.get("build_cohort_sha256") == args.expected_build_cohort, "build cohort differs")
            require(catalog.get("canonical_sha256") == capabilities.get("operator_catalog_sha256"), "operator catalog differs")
            require(isinstance(skills.get("skills"), list), "Skill inventory is unavailable")
            require(uv_pbr.get("skill", {}).get("execution_availability") == "active", "uv-pbr Skill is not active")
            require(pack.get("pack_id") == PACK_ID, "2K fictional weapon pack is unavailable")
            manifest_sha256 = pack.get("canonical_sha256")
            require(isinstance(manifest_sha256, str), "2K material manifest hash is unavailable")

            source = client.tool("candidate_get", {"candidate_id": SOURCE_CANDIDATE_ID})
            require(source.get("canonical_sha256") == SOURCE_CANDIDATE_STATE, "source candidate state differs")
            program = appearance_program(
                geometry, appearance_geometry_program_sha256, manifest_sha256
            )
            prepared = client.tool(
                "appearance_prepare",
                {
                    "project_id": PROJECT_ID,
                    "request": {
                        "typed": "appearance",
                        "reference_id": REFERENCE_ID,
                        "geometry_program": geometry,
                        "appearance_program": program,
                    },
                },
            )
            candidate = prepared.get("candidate")
            artifact = prepared.get("artifact")
            require(isinstance(candidate, dict) and isinstance(artifact, dict), "Appearance prepare omitted candidate/artifact")
            candidate_id = candidate.get("candidate_id")
            artifact_id = artifact.get("artifact_id")
            require(isinstance(candidate_id, str) and isinstance(artifact_id, str), "Appearance identities are unavailable")
            readback = client.tool("artifact_readback_get", {"artifact_id": artifact_id, "candidate_id": candidate_id})
            require(readback.get("hard_gate_passed") is True, "Appearance GLB readback failed")
            comparison = client.tool(
                "reference_compare_prepare",
                {
                    "project_id": PROJECT_ID,
                    "candidate_id": candidate_id,
                    "reference_id": REFERENCE_ID,
                    "view_spec": view_spec,
                    "camera": camera,
                    "target_sha256": target_sha256,
                },
            )
            render_set = comparison.get("render_set")
            require(isinstance(render_set, dict), "approved-camera render set is unavailable")
            pass_artifacts = render_set.get("pass_artifacts")
            require(isinstance(pass_artifacts, dict), "Render pass inventory is unavailable")
            passes = {
                name: value.get("sha256") if isinstance(value, dict) else value
                for name, value in pass_artifacts.items()
            }
            require(isinstance(passes.get("beauty"), str), "Beauty pass is unavailable")
            verified_passes = {}
            for pass_name in ("beauty", "normal", "material-id", "wireframe", "uv-stretch"):
                item = client.tool(
                    "render_pass_get",
                    {
                        "pass": pass_name,
                        "render_set_hash": comparison["render_set_object_sha256"],
                    },
                )
                verified_passes[pass_name] = item.get("object_sha256") or passes.get(pass_name)
            quality = client.tool("quality_get", {"candidate_id": candidate_id, "reference_id": REFERENCE_ID})

            evidence = {
                "schema_version": "ForgeCADWeaponAppearanceV3HeroRenderEvidence@1",
                "task_id": "FPS-PRODUCTION-04BH-APPEARANCE-V3",
                "recorded_at": "2026-08-29",
                "scope": "fictional game and film visual asset only",
                "source": {
                    "project_id": PROJECT_ID,
                    "candidate_id": SOURCE_CANDIDATE_ID,
                    "candidate_state_sha256": SOURCE_CANDIDATE_STATE,
                    "high_artifact_sha256": SOURCE_ARTIFACT_SHA256,
                    "source_geometry_program_sha256": GEOMETRY_PROGRAM_SHA256,
                    "appearance_geometry_program_sha256": appearance_geometry_program_sha256,
                    "geometry_change": "RESOURCE_BUDGET_ONLY_8_MIB_TO_64_MIB",
                    "retopology_cage_bundle_sha256": RETOPOLOGY_CAGE_BUNDLE_SHA256,
                    "low_artifact_sha256": LOW_ARTIFACT_SHA256,
                    "cage_artifact_sha256": CAGE_ARTIFACT_SHA256,
                },
                "discovery": {
                    "build_cohort_sha256": capabilities.get("build_cohort_sha256"),
                    "runtime_state": runtime_status.get("state"),
                    "operator_catalog_sha256": catalog.get("canonical_sha256"),
                    "uv_pbr_manifest_sha256": uv_pbr.get("skill", {}).get("canonical_sha256"),
                    "material_pack_id": PACK_ID,
                    "material_pack_manifest_sha256": manifest_sha256,
                    "material_pack_license_spdx": pack.get("license_spdx"),
                },
                "appearance": {
                    "appearance_program": program,
                    "appearance_program_sha256": program["canonical_sha256"],
                    "material_layer_stack_sha256": program["material_layer_stack_sha256"],
                    "texture_build_receipt_sha256": prepared.get("texture_build_receipt_sha256"),
                    "candidate_surface_bake_receipt_sha256": prepared.get("candidate_surface_bake_receipt_sha256"),
                },
                "reviewable_candidate": {
                    "candidate_id": candidate_id,
                    "candidate_state_sha256": candidate.get("canonical_sha256"),
                    "artifact_sha256": artifact.get("object_sha256") or artifact_id,
                    "artifact_readback_canonical_sha256": readback.get("canonical_sha256"),
                    "artifact_readback_object_sha256": readback.get("object_sha256"),
                    "triangle_count": readback.get("triangle_count"),
                    "part_ids": readback.get("part_ids"),
                    "material_zone_ids": readback.get("material_zone_ids"),
                    "candidate_confirmed": False,
                    "version_created": False,
                    "exported": False,
                },
                "camera_and_reference": {
                    "reference_id": REFERENCE_ID,
                    "reference_sha256": REFERENCE_SHA256,
                    "view_spec_canonical_sha256": view_spec["canonical_sha256"],
                    "target_sha256": target_sha256,
                    "camera_hash": camera["camera_hash"],
                    "camera_canonical_sha256": camera["canonical_sha256"],
                    "projection": camera["projection"],
                    "subject_screen_order": "stock-left-muzzle-right",
                    "upright": True,
                },
                "render": {
                    "render_set_object_sha256": comparison.get("render_set_object_sha256"),
                    "render_set_canonical_sha256": render_set.get("canonical_sha256"),
                    "render_worker_build_cohort_sha256": render_set.get("render_worker_build_cohort_sha256"),
                    "render_worker_binding_status": render_set.get("render_worker_binding_status"),
                    "passes": passes,
                    "verified_passes": verified_passes,
                    "beauty_object_sha256": passes["beauty"],
                },
                "comparison": {
                    "comparison_report_object_sha256": comparison.get("comparison_report_object_sha256"),
                    "quality_report_object_sha256": comparison.get("quality_report_object_sha256"),
                    "status": comparison.get("comparison_report", {}).get("status"),
                    "metrics": comparison.get("comparison_report", {}).get("metrics"),
                    "quality": quality,
                },
                "truth": {
                    "appearance_program_v3_layer_stack": "CREATED_REVIEWABLE",
                    "texture_build_2k": "PASS_RUNTIME_DURABLE",
                    "candidate_surface_bake": "PASS_RUNTIME_DURABLE_SELF_SURFACE_NOT_HIGH_TO_LOW_CAGE",
                    "formal_hero_uv_durable": "NOT_CREATED",
                    "formal_high_low_geometric_bake": "NOT_RUN",
                    "human_review": "AWAITING_USER_DECISION",
                    "commercial_engine_validation": "NOT_RUN",
                    "commercial_quality": "NOT_PROVEN",
                    "promotion_allowed": False,
                },
            }
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(evidence, ensure_ascii=False, indent=2))
        finally:
            close_client(runtime, ready_path, ready, client)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
