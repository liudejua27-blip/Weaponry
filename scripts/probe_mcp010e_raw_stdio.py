#!/usr/bin/env python3
"""Run the MCP010E offline material, UV and PBR path over raw MCP stdio.

This is a source-built structural gate. It proves that the checked-in
first-party AssetPack is discovered through MCP, that a V2 geometry candidate
can bind an AppearanceProgram@2 to that pack, and that the resulting GLB and
fixed render set contain embedded texture-backed material evidence. It does
not claim visual likeness, human approval or complete 360-degree coverage.
"""

from __future__ import annotations

import base64
import copy
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    MCP_PROTOCOL_VERSION,
    GateFailure,
    McpClient,
    build_identity,
    shutdown_runtime,
    v2_program_draft,
    wait_for_ready,
)


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def png_dimensions(data: bytes) -> tuple[int, int]:
    require(data.startswith(b"\x89PNG\r\n\x1a\n") and len(data) >= 24, "--compare currently requires a PNG reference")
    width = int.from_bytes(data[16:20], "big")
    height = int.from_bytes(data[20:24], "big")
    require(1 <= width <= 8192 and 1 <= height <= 8192, "reference dimensions exceed Runtime bounds")
    return width, height


def robot_reference_annotations() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Return observed annotations for the supplied three-quarter robot image.

    Coordinates are normalized to the complete square source image.  They are
    deliberately limited to visible silhouette/assembly anchors; hidden rear
    geometry, internal mechanisms, feet and cropped lower-body areas are not
    represented as observed evidence.  The Runtime owns the resulting metrics.
    """
    landmarks = [
        {"landmark_id": "crown", "x": 0.507, "y": 0.026, "visibility": "observed", "confidence": 0.96},
        {"landmark_id": "visor-front-tip", "x": 0.383, "y": 0.171, "visibility": "observed", "confidence": 0.92},
        {"landmark_id": "visor-lower-front", "x": 0.397, "y": 0.244, "visibility": "observed", "confidence": 0.90},
        {"landmark_id": "neck-base", "x": 0.502, "y": 0.324, "visibility": "observed", "confidence": 0.86},
        {"landmark_id": "left-shoulder-outer", "x": 0.337, "y": 0.349, "visibility": "observed", "confidence": 0.88},
        {"landmark_id": "right-shoulder-outer", "x": 0.766, "y": 0.271, "visibility": "observed", "confidence": 0.88},
        {"landmark_id": "chest-center", "x": 0.509, "y": 0.397, "visibility": "observed", "confidence": 0.90},
        {"landmark_id": "chest-lower", "x": 0.500, "y": 0.535, "visibility": "observed", "confidence": 0.84},
        {"landmark_id": "left-elbow", "x": 0.333, "y": 0.535, "visibility": "observed", "confidence": 0.80},
        {"landmark_id": "right-elbow", "x": 0.768, "y": 0.492, "visibility": "observed", "confidence": 0.80},
        {"landmark_id": "pelvis-center", "x": 0.508, "y": 0.641, "visibility": "observed", "confidence": 0.82},
        {"landmark_id": "left-knee", "x": 0.365, "y": 0.785, "visibility": "observed", "confidence": 0.78},
        {"landmark_id": "right-knee", "x": 0.672, "y": 0.753, "visibility": "observed", "confidence": 0.78},
        {"landmark_id": "left-hand", "x": 0.292, "y": 0.823, "visibility": "observed", "confidence": 0.82},
        {"landmark_id": "right-hand", "x": 0.793, "y": 0.793, "visibility": "observed", "confidence": 0.82},
    ]
    regions = [
        {"region_id": "head-visor", "x": 0.360, "y": 0.018, "width": 0.275, "height": 0.292, "visibility": "observed", "confidence": 0.92},
        {"region_id": "neck-mechanism", "x": 0.405, "y": 0.260, "width": 0.205, "height": 0.145, "visibility": "observed", "confidence": 0.82},
        {"region_id": "chest-armor", "x": 0.292, "y": 0.285, "width": 0.435, "height": 0.285, "visibility": "observed", "confidence": 0.90},
        {"region_id": "left-shoulder-arm", "x": 0.245, "y": 0.280, "width": 0.205, "height": 0.400, "visibility": "observed", "confidence": 0.84},
        {"region_id": "right-shoulder-arm", "x": 0.610, "y": 0.215, "width": 0.235, "height": 0.405, "visibility": "observed", "confidence": 0.84},
        {"region_id": "pelvis-core", "x": 0.315, "y": 0.530, "width": 0.390, "height": 0.185, "visibility": "observed", "confidence": 0.80},
        {"region_id": "left-thigh-knee", "x": 0.245, "y": 0.640, "width": 0.245, "height": 0.335, "visibility": "observed", "confidence": 0.76},
        {"region_id": "right-thigh-knee", "x": 0.485, "y": 0.625, "width": 0.270, "height": 0.350, "visibility": "observed", "confidence": 0.76},
    ]
    return landmarks, regions


def parse_args() -> Any:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument(
        "--detail",
        action="store_true",
        help="Use the MCP010D robot-specific profile/panel/vent/joint/sweep detail fixture instead of primitive-only geometry.",
    )
    parser.add_argument(
        "--geometry-variant",
        choices=("baseline", "surface-linework", "chest-profile", "chest-shell-curved", "curved-tapered", "chest-wedge", "chest-wedge-mild", "chest-top-cap", "chest-top-cap-mild", "visor-profile", "chest-wedge-visor", "chest-width-mild", "helmet-visor", "head-wedge", "head-turn-mild", "shoulder-contour-mild", "shoulder-contour-tiny", "tapered-lower", "tapered-shells", "asymmetric-stance", "asymmetric-armor", "asymmetric-linework", "three-quarter", "pose-yaw-mild", "long-limbs", "sleek-armor", "visible-thigh", "thigh-width-mild", "shin-width-mild"),
        default="surface-linework",
        help="Select a bounded visual experiment for the detail fixture; surface-linework is the current source default and baseline remains available for historical comparison.",
    )
    parser.add_argument(
        "--material-variant",
        choices=("default-zones", "surface-zones", "armor-shell-zones"),
        default="armor-shell-zones",
        help="Choose the typed material zoning recipe; armor-shell-zones keeps visible outer arm shells white while retaining dark mechanical gaps.",
    )
    parser.add_argument(
        "--reference",
        type=Path,
        help="Optional user-authorized PNG/JPEG. Without it, use the 1x1 isolated fixture.",
    )
    parser.add_argument(
        "--compare",
        action="store_true",
        help="Run candidate-bound MCP010C comparison and quality readback; requires --reference.",
    )
    parser.add_argument(
        "--render-dir",
        type=Path,
        help="Optional local directory for saving returned PNG AOVs during visual inspection; never persisted in a receipt.",
    )
    parser.add_argument(
        "--fit-plan-input-dir",
        type=Path,
        help="Optional temporary directory for the exact comparison/view/catalog JSON inputs consumed by build_mcp010f_fit_plan.py; no image bytes are written.",
    )
    parser.add_argument(
        "--camera-position",
        nargs=3,
        type=float,
        metavar=("X", "Y", "Z"),
        help="Optional explicit camera position for comparison experiments; omit to use Runtime framing calibration.",
    )
    parser.add_argument(
        "--camera-target",
        nargs=3,
        type=float,
        default=[0.0, 1.5, 0.0],
        metavar=("X", "Y", "Z"),
        help="Target point for --camera-position.",
    )
    parser.add_argument(
        "--viewer-executable",
        type=Path,
        help="Optional packaged Viewer executable. While the isolated Runtime is alive, run its read-only CLI projection and include a sanitized structural receipt.",
    )
    parser.add_argument(
        "--receipt-task-id",
        choices=("FGC-MCP010E", "FGC-MCP010F"),
        default="FGC-MCP010E",
        help="Receipt task identifier. Use F when this probe is exercising only the packaged Viewer projection.",
    )
    return parser.parse_args()


def explicit_camera(position: list[float], target: list[float]) -> dict[str, Any]:
    camera: dict[str, Any] = {
        "schema_version": "CameraCalibration@1",
        "camera_hash": "",
        "projection": "perspective",
        "transform": {"position_m": position, "target_m": target, "up": [0.0, 1.0, 0.0]},
        "fov_y_degrees": 42.0,
        "near_m": 0.05,
        "far_m": 20.0,
        "resolution": {"width": 512, "height": 512},
        "coordinate_system": "right-handed-y-up-meter",
        "renderer_revision": "forgecad-renderer-2",
        "canonical_sha256": "",
    }
    camera["camera_hash"] = canonical_hash(camera)
    camera["canonical_sha256"] = canonical_hash(camera)
    return camera


def appearance_program(
    project_id: str,
    geometry_sha256: str,
    pack_sha256: str,
    part_ids: list[str] | None = None,
    material_variant: str = "armor-shell-zones",
) -> dict[str, Any]:
    if part_ids is None:
        material_zones = [
            {
                "zone_id": "zone-white-shell",
                "part_ids": ["shell"],
                "material_id": "white-dielectric-clearcoat",
                "texture_set_id": "plastic-surface",
            },
            {
                "zone_id": "zone-black-mechanical",
                "part_ids": ["joint"],
                "material_id": "dark-painted-metal",
                "texture_set_id": "metal-surface",
            },
            {
                "zone_id": "zone-emissive-amber",
                "part_ids": ["sensor"],
                "material_id": "warm-orange-emissive",
                "texture_set_id": None,
            },
        ]
    else:
        expected_detail = {
            "arrayed-part": "zone-white-shell",
            "panel-vent": "zone-black-mechanical",
            "joint-part": "zone-black-mechanical",
            "profile-part": "zone-white-shell",
            "loft-part": "zone-white-shell",
            "revolve-part": "zone-black-mechanical",
            "sweep-part": "zone-emissive-amber",
        }
        if set(part_ids) == set(expected_detail):
            zone_for_part = expected_detail
        else:
            zone_for_part = {
                part_id: detail_material_zone(part_id, material_variant)
                for part_id in part_ids
            }
            require(all(zone_for_part.values()), "robot detail fixture returned an invalid Part set")
        material_zone_specs = [
            ("zone-white-shell", "white-dielectric-clearcoat", "plastic-surface"),
            ("zone-dark-painted", "dark-painted-metal", "metal-surface"),
            ("zone-black-anodized", "black-anodized-metal", "metal-surface"),
            ("zone-brushed-steel", "brushed-steel", "metal-surface"),
            ("zone-engineering-plastic", "engineering-plastic", "plastic-surface"),
            ("zone-joint-rubber", "joint-rubber", "plastic-surface"),
            ("zone-micro-scratch", "micro-scratch-coat", "metal-surface"),
            ("zone-emissive-amber", "warm-orange-emissive", None),
        ] if material_variant in ("surface-zones", "armor-shell-zones") and set(part_ids) != set(expected_detail) else [
            ("zone-white-shell", "white-dielectric-clearcoat", "plastic-surface"),
            ("zone-black-mechanical", "dark-painted-metal", "metal-surface"),
            ("zone-emissive-amber", "warm-orange-emissive", None),
        ]
        material_zones = [
            {
                "zone_id": zone_id,
                "part_ids": [part_id for part_id in part_ids if zone_for_part[part_id] == zone_id],
                "material_id": material_id,
                "texture_set_id": texture_set_id,
            }
            for zone_id, material_id, texture_set_id in material_zone_specs
            if any(zone_for_part[part_id] == zone_id for part_id in part_ids)
        ]
    value: dict[str, Any] = {
        "schema_version": "AppearanceProgram@2",
        "project_id": project_id,
        "geometry_program_sha256": geometry_sha256,
        "material_pack_id": "forgecad-hard-surface-robot",
        "material_pack_manifest_sha256": pack_sha256,
        "material_zones": material_zones,
        "canonical_sha256": "",
    }
    value["canonical_sha256"] = canonical_hash(
        {key: item for key, item in value.items() if key != "canonical_sha256"}
    )
    return value


def detail_material_zone(part_id: str, material_variant: str) -> str:
    """Return the explicit surface recipe for a semantic robot Part."""
    # Keep outer armor white in the armor-shell recipe. The reference image
    # has dark mechanical gaps and inner structures, but its visible upper and
    # forearm shells remain predominantly white; this variant tests that cue
    # without changing geometry, camera, or comparison code.
    if material_variant == "armor-shell-zones":
        mapping = {
            "visor": "zone-black-anodized",
            "chest-vent": "zone-black-anodized",
            "chest-core": "zone-black-anodized",
            "core-ribs": "zone-black-anodized",
            "neck": "zone-brushed-steel",
            "shoulder-pair": "zone-brushed-steel",
            "elbow-pair": "zone-brushed-steel",
            "hip-pair": "zone-brushed-steel",
            "knee-pair": "zone-brushed-steel",
            "hand-pair": "zone-engineering-plastic",
            "cable-pair": "zone-joint-rubber",
            "chest-ridge": "zone-micro-scratch",
            "amber-sensor": "zone-emissive-amber",
            "visor-edge": "zone-emissive-amber",
        }
        if part_id in mapping:
            return mapping[part_id]
        # Asymmetric contour experiments split mirrored Parts into stable
        # left/right IDs. Preserve the same semantic material recipe instead
        # of silently falling back to white for those derived IDs.
        if part_id in {"shoulder-left", "shoulder-right", "elbow-left", "elbow-right", "hip-left", "hip-right", "knee-left", "knee-right", "neck-left", "neck-right"}:
            return "zone-brushed-steel"
        if part_id in {"hand-left", "hand-right"}:
            return "zone-engineering-plastic"
        if part_id in {"cable-left", "cable-right"}:
            return "zone-joint-rubber"
        if any(token in part_id for token in ("vent", "core", "visor")):
            return "zone-black-anodized"
        if any(token in part_id for token in ("ridge", "micro-scratch")):
            return "zone-micro-scratch"
        if any(token in part_id for token in ("sensor", "emissive", "light")):
            return "zone-emissive-amber"
        return "zone-white-shell"
    mapping = {
        "visor": "zone-black-anodized",
        "chest-vent": "zone-black-anodized",
        "chest-core": "zone-black-anodized",
        "core-ribs": "zone-black-anodized",
        "neck": "zone-brushed-steel",
        "shoulder-pair": "zone-brushed-steel",
        "elbow-pair": "zone-brushed-steel",
        "hip-pair": "zone-brushed-steel",
        "knee-pair": "zone-brushed-steel",
        "upper-arm-pair": "zone-dark-painted",
        "forearm-pair": "zone-dark-painted",
        "shoulder-trim-pair": "zone-dark-painted",
        "forearm-rail-pair": "zone-dark-painted",
        "hip-flank-pair": "zone-dark-painted",
        "hand-pair": "zone-engineering-plastic",
        "cable-pair": "zone-joint-rubber",
        "chest-ridge": "zone-micro-scratch",
        "amber-sensor": "zone-emissive-amber",
        "visor-edge": "zone-emissive-amber",
    }
    return mapping.get(part_id, "zone-white-shell")


def robot_detail_program_draft(
    project_id: str,
    catalog_hash: str,
    variant: str = "baseline",
    material_variant: str = "armor-shell-zones",
) -> dict[str, Any]:
    """A bounded robot-specific D program for the uploaded three-quarter image.

    This remains authored typed geometry, not an image-to-mesh shortcut.  The
    visible torso/head/limb masses are observed; rear and cropped lower-body
    structure remain inferred/unknown in the companion evidence.
    """
    nodes: list[dict[str, Any]] = []
    outputs: list[dict[str, Any]] = []

    def add(
        node_id: str,
        operator_id: str,
        inputs: list[str],
        parameters: dict[str, Any],
        *,
        part_id: str | None = None,
        material_zone_id: str = "zone-white-shell",
    ) -> None:
        nodes.append({"node_id": node_id, "operator_id": operator_id, "inputs": inputs, "parameters": parameters})
        if part_id is not None:
            outputs.append({"part_id": part_id, "input_node_ids": [node_id], "material_zone_id": material_zone_id, "solid": True})

    def primitive_box(node_id: str, size: list[float], position: list[float], rotation: list[float], *, part_id: str | None = None, zone: str = "zone-white-shell") -> None:
        add(node_id, "forgecad.geometry.primitive@2", [], {"shape": "box", "size_m": size, "position_m": position, "rotation_rad": rotation}, part_id=part_id, material_zone_id=zone)

    def primitive_ellipsoid(node_id: str, radii: list[float], position: list[float], rotation: list[float], *, part_id: str | None = None, zone: str = "zone-white-shell") -> None:
        add(node_id, "forgecad.geometry.primitive@2", [], {"shape": "ellipsoid", "radii_m": radii, "longitude_segments": 24, "latitude_segments": 16, "position_m": position, "rotation_rad": rotation}, part_id=part_id, material_zone_id=zone)

    def primitive_cylinder(node_id: str, radius: float, height: float, position: list[float], rotation: list[float], *, part_id: str | None = None, zone: str = "zone-black-mechanical") -> None:
        add(node_id, "forgecad.geometry.primitive@2", [], {"shape": "cylinder", "radius_m": radius, "height_m": height, "radial_segments": 24, "position_m": position, "rotation_rad": rotation}, part_id=part_id, material_zone_id=zone)

    def transform(node_id: str, source: str, translation: list[float], rotation: list[float], scale: list[float]) -> None:
        add(node_id, "forgecad.geometry.transform@2", [source], {"shape": "transform", "translation_m": translation, "rotation_rad": rotation, "scale": scale})

    def mirror(node_id: str, source: str) -> None:
        add(node_id, "forgecad.geometry.mirror@1", [source], {"shape": "mirror", "axis": "x", "offset_m": 0.0})

    def mirrored_box(node_id: str, size: list[float], left_position: list[float], rotation: list[float], part_id: str, zone: str = "zone-white-shell") -> None:
        source = f"{node_id}-left"
        shaped = f"{node_id}-shaped"
        # Use the bounded panel macro for armor shells instead of a raw box:
        # it keeps the same typed Part lineage while adding deterministic
        # chamfered perimeter edges that survive the fixed renderer.
        add(source, "forgecad.geometry.panel@1", [], {"shape": "panel", "size_m": size, "thickness_m": size[2], "bevel_m": min(size[0], size[1]) * 0.18, "position_m": left_position, "rotation_rad": rotation})
        transform(shaped, source, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        mirrored = f"{node_id}-pair"
        mirror(mirrored, shaped)
        outputs.append({"part_id": part_id, "input_node_ids": [mirrored], "material_zone_id": zone, "solid": True})

    def mirrored_joint(node_id: str, left_position: list[float], part_id: str) -> None:
        source = f"{node_id}-left"
        primitive_cylinder(source, 0.20, 0.24, left_position, [1.5708, 0.0, 0.0])
        mirrored = f"{node_id}-pair"
        mirror(mirrored, source)
        outputs.append({"part_id": part_id, "input_node_ids": [mirrored], "material_zone_id": "zone-black-mechanical", "solid": True})

    head_profiles = [
        {"height_m": -0.34, "points": [[-0.28, -0.40], [0.28, -0.40], [0.40, -0.22], [0.36, 0.28], [0.20, 0.43], [-0.20, 0.43], [-0.36, 0.28], [-0.40, -0.22]]},
        {"height_m": 0.18, "points": [[-0.31, -0.36], [0.31, -0.36], [0.43, -0.18], [0.38, 0.30], [0.20, 0.48], [-0.20, 0.48], [-0.38, 0.30], [-0.43, -0.18]]},
        {"height_m": 0.48, "points": [[-0.22, -0.25], [0.22, -0.25], [0.30, -0.10], [0.28, 0.22], [0.14, 0.35], [-0.14, 0.35], [-0.28, 0.22], [-0.30, -0.10]]},
    ]
    visor_parameters = {"shape": "panel", "size_m": [0.62, 0.24, 0.16], "thickness_m": 0.16, "bevel_m": 0.06, "position_m": [0.0, 2.90, 0.50], "rotation_rad": [0.0, -0.30, 0.0]}
    if variant == "helmet-visor":
        # The experiment adds a tapered crown and a wider visor band.  It is
        # deliberately opt-in so the baseline receipt remains reproducible.
        helmet_profile = [
            [-0.24, -0.46], [0.24, -0.46], [0.37, -0.38], [0.45, -0.22],
            [0.43, 0.18], [0.34, 0.36], [0.18, 0.46], [-0.18, 0.46],
            [-0.34, 0.36], [-0.43, 0.18], [-0.45, -0.22], [-0.37, -0.38],
        ]
        head_profiles = [
            {"height_m": -0.38, "points": helmet_profile},
            {"height_m": 0.12, "points": [[x * 1.06, y * 0.98] for x, y in helmet_profile]},
            {"height_m": 0.50, "points": [[x * 0.82, y * 0.78] for x, y in helmet_profile]},
        ]
        visor_parameters = {"shape": "panel", "size_m": [0.78, 0.30, 0.20], "thickness_m": 0.20, "bevel_m": 0.08, "position_m": [0.0, 2.89, 0.52], "rotation_rad": [0.0, -0.28, 0.0]}

    if variant == "head-wedge":
        # Reference-only head experiment: keep the observed crown/visor
        # anchors fixed while giving the shell a deeper rear and a tapered
        # forward brow.  The change is intentionally limited to the head and
        # visor Parts so the comparison can reject it without contaminating
        # the baseline or inventing unseen rear geometry.
        head_profiles = [
            {"height_m": -0.36, "points": [[-0.28, -0.42], [0.28, -0.42], [0.40, -0.18], [0.37, 0.26], [0.18, 0.55], [-0.18, 0.55], [-0.35, 0.30], [-0.42, -0.16]]},
            {"height_m": 0.12, "points": [[-0.31, -0.38], [0.31, -0.38], [0.43, -0.14], [0.39, 0.24], [0.16, 0.49], [-0.16, 0.49], [-0.37, 0.26], [-0.45, -0.14]]},
            {"height_m": 0.50, "points": [[-0.22, -0.28], [0.22, -0.28], [0.31, -0.10], [0.28, 0.20], [0.12, 0.39], [-0.12, 0.39], [-0.27, 0.21], [-0.32, -0.10]]},
        ]
        visor_parameters = {"shape": "panel", "size_m": [0.68, 0.25, 0.17], "thickness_m": 0.17, "bevel_m": 0.06, "position_m": [0.0, 2.89, 0.53], "rotation_rad": [0.0, -0.40, 0.0]}

    add(
        "head-shell",
        "forgecad.geometry.profile-loft@1",
        [],
        {
            "shape": "profile-loft",
            "profiles": head_profiles,
            "position_m": [0.0, 2.92, 0.0],
            "rotation_rad": [0.0, 0.24, 0.0]
        },
        part_id="head-shell",
    )
    add("visor", "forgecad.geometry.panel@1", [], visor_parameters, part_id="visor", material_zone_id="zone-black-mechanical")
    if variant in ("visor-profile", "chest-wedge-visor"):
        visor_node = next(node for node in nodes if node.get("node_id") == "visor")
        visor_node["operator_id"] = "forgecad.geometry.profile-extrude@1"
        visor_node["parameters"] = {
            "shape": "profile-extrude",
            "profile": [[-0.40, -0.10], [-0.18, -0.17], [0.30, -0.14], [0.38, -0.01], [0.25, 0.12], [-0.20, 0.14], [-0.40, 0.07]],
            "depth_m": 0.20,
            "position_m": [0.0, 2.89, 0.52],
            "rotation_rad": [0.0, -0.30, 0.0],
        }
    if variant == "head-turn-mild":
        # Single-Part pose experiment: rotate only the authored head shell
        # toward the visible three-quarter reference. The visor, body,
        # camera, and material recipe remain unchanged so comparison can
        # attribute any metric delta to the head shell alone.
        head_node = next(node for node in nodes if node.get("node_id") == "head-shell")
        head_node["parameters"]["rotation_rad"] = [0.0, 0.12, 0.0]

    add("chest-panel", "forgecad.geometry.panel@1", [], {"shape": "panel", "size_m": [1.66, 1.12, 0.68], "thickness_m": 0.18, "bevel_m": 0.12, "position_m": [0.0, 1.98, 0.04], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="chest-shell")
    if variant in ("chest-wedge", "chest-wedge-mild", "chest-wedge-visor"):
        chest_node = next(node for node in nodes if node.get("node_id") == "chest-panel")
        chest_node["operator_id"] = "forgecad.geometry.profile-extrude@1"
        chest_node["parameters"] = {
            "shape": "profile-extrude",
            "profile": (
                [[-0.83, -0.56], [0.83, -0.56], [0.78, 0.22], [0.48, 0.50], [0.0, 0.60], [-0.48, 0.50], [-0.78, 0.22]]
                if variant == "chest-wedge"
                else [[-0.83, -0.56], [0.83, -0.56], [0.82, 0.20], [0.72, 0.34], [0.42, 0.50], [0.0, 0.55], [-0.42, 0.50], [-0.72, 0.34], [-0.82, 0.20]]
            ),
            "depth_m": 0.68,
            "position_m": [0.0, 1.98, 0.04],
            "rotation_rad": [0.0, 0.0, 0.0],
        }
    if variant in ("chest-top-cap", "chest-top-cap-mild"):
        mild_cap = variant == "chest-top-cap-mild"
        add(
            "chest-top-cap",
            "forgecad.geometry.profile-extrude@1",
            [],
            {
                "shape": "profile-extrude",
                "profile": (
                    [[-0.78, -0.10], [0.78, -0.10], [0.66, 0.08], [0.34, 0.20], [-0.34, 0.20], [-0.66, 0.08]]
                    if not mild_cap
                    else [[-0.68, -0.08], [0.68, -0.08], [0.59, 0.06], [0.30, 0.16], [-0.30, 0.16], [-0.59, 0.06]]
                ),
                "depth_m": 0.72 if not mild_cap else 0.56,
                "position_m": [0.0, 2.43 if not mild_cap else 2.38, 0.04],
                "rotation_rad": [0.0, 0.0, 0.0],
            },
            part_id="chest-top-cap",
            material_zone_id="zone-white-shell",
        )
    if variant == "chest-width-mild":
        # One-Part envelope experiment: the reference chest/shoulder mass is
        # visibly broader than the retained front-facing blockout. Widen only
        # the chest shell; keep camera, head, shoulders, internals and material
        # bindings fixed so a comparison can reject the hypothesis cleanly.
        chest_node = next(node for node in nodes if node.get("node_id") == "chest-panel")
        chest_parameters = chest_node.get("parameters")
        if not isinstance(chest_parameters, dict):
            raise ValueError("chest width experiment parameters missing")
        chest_parameters["size_m"] = [1.78, 1.12, 0.68]
    # Keep the vent assembly as a separate semantic Part so the dark cavity
    # material is not flattened into the white shell by one aggregate zone.
    add("chest-vent", "forgecad.geometry.vent-array@1", [], {"shape": "vent-array", "width_m": 1.20, "height_m": 0.54, "depth_m": 0.18, "slot_count": 5, "slot_width_m": 0.11, "slot_spacing_m": 0.13, "position_m": [0.0, 2.00, 0.40], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="chest-vent", material_zone_id="zone-black-mechanical")
    add("chest-core", "forgecad.geometry.revolve@1", [], {"shape": "revolve", "profile": [[0.18, -0.28], [0.34, -0.10], [0.32, 0.16], [0.18, 0.28]], "radial_segments": 24, "position_m": [0.0, 1.76, 0.28], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="chest-core", material_zone_id="zone-black-mechanical")
    add("neck-ring", "forgecad.geometry.joint-stack@1", [], {"shape": "joint-stack", "radius_m": 0.23, "depth_m": 0.16, "ring_count": 3, "ring_spacing_m": 0.14, "radial_segments": 16, "position_m": [0.0, 2.45, 0.0], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="neck", material_zone_id="zone-black-mechanical")
    add("pelvis-shell", "forgecad.geometry.profile-loft@1", [], {"shape": "profile-loft", "profiles": [{"height_m": 0.0, "points": [[-0.48, -0.24], [0.48, -0.24], [0.48, 0.24], [-0.48, 0.24]]}, {"height_m": 0.40, "points": [[-0.36, -0.20], [0.36, -0.20], [0.36, 0.20], [-0.36, 0.20]]}], "position_m": [0.0, 1.20, 0.0], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="pelvis")

    mirrored_joint("shoulder", [-0.98, 2.08, 0.28], "shoulder-pair")
    mirrored_box("shoulder-armor", [0.52, 0.46, 0.58], [-0.90, 2.00, 0.34], [0.0, 0.0, -0.10], "shoulder-armor-pair")
    mirrored_box("upper-arm", [0.38, 0.78, 0.40], [-1.10, 1.64, 0.30], [0.0, 0.0, -0.15], "upper-arm-pair")
    mirrored_joint("elbow", [-1.16, 1.20, 0.34], "elbow-pair")
    mirrored_box("forearm", [0.34, 0.82, 0.36], [-1.18, 0.83, 0.36], [0.0, 0.0, -0.10], "forearm-pair")
    primitive_ellipsoid("hand-left", [0.24, 0.32, 0.22], [-1.18, 0.36, 0.40], [0.0, 0.0, 0.0])
    mirror("hand-pair", "hand-left")
    outputs.append({"part_id": "hand-pair", "input_node_ids": ["hand-pair"], "material_zone_id": "zone-black-mechanical", "solid": True})
    mirrored_box("thigh", [0.64, 1.02, 0.60], [-0.48, 0.62, 0.24], [0.0, 0.0, 0.06], "thigh-pair")
    mirrored_joint("hip", [-0.48, 1.10, 0.24], "hip-pair")
    mirrored_joint("knee", [-0.46, 0.10, 0.30], "knee-pair")
    mirrored_box("shin", [0.48, 0.80, 0.54], [-0.48, -0.34, 0.28], [0.0, 0.0, 0.0], "shin-pair")

    add("left-cable", "forgecad.geometry.tube-sweep@1", [], {"shape": "tube-sweep", "path": [[-0.50, 2.15, 0.22], [-0.62, 1.86, 0.34], [-0.58, 1.55, 0.26]], "radius_m": 0.045, "radial_segments": 12, "cap_ends": True, "position_m": [0.0, 0.0, 0.0], "rotation_rad": [0.0, 0.0, 0.0]})
    mirror("cable-pair", "left-cable")
    outputs.append({"part_id": "cable-pair", "input_node_ids": ["cable-pair"], "material_zone_id": "zone-black-mechanical", "solid": True})
    primitive_box("core-rib-base", [0.32, 0.08, 0.18], [0.0, 1.48, 0.34], [0.0, 0.0, 0.0])
    add("core-ribs", "forgecad.geometry.array@1", ["core-rib-base"], {"shape": "array", "count": 3, "offset_m": [0.0, -0.16, 0.0]}, part_id="core-ribs", material_zone_id="zone-black-mechanical")
    add("amber-sensor", "forgecad.geometry.revolve@1", [], {"shape": "revolve", "profile": [[0.06, -0.08], [0.12, 0.0], [0.06, 0.08]], "radial_segments": 20, "position_m": [0.0, 1.40, 0.48], "rotation_rad": [0.0, 0.0, 0.0]}, part_id="amber-sensor", material_zone_id="zone-emissive-amber")

    if variant in ("surface-linework", "chest-shell-curved", "chest-top-cap", "chest-top-cap-mild", "chest-wedge", "chest-wedge-visor", "tapered-shells", "asymmetric-linework", "pose-yaw-mild", "head-turn-mild", "shoulder-contour-mild", "shoulder-contour-tiny", "thigh-width-mild", "shin-width-mild"):
        # Surface-language experiment: add only thin, traceable layers that
        # explain the reference's panel breaks and light channels.  These
        # parts do not change the outer body envelope; they are deliberately
        # separate semantic Parts so material-ID and review can isolate them.
        add(
            "visor-edge",
            "forgecad.geometry.panel@1",
            [],
            {
                "shape": "panel",
                "size_m": [0.54, 0.07, 0.12],
                "thickness_m": 0.08,
                "bevel_m": 0.02,
                "position_m": [0.0, 2.77, 0.54],
                "rotation_rad": [0.0, -0.30, 0.0],
            },
            part_id="visor-edge",
            material_zone_id="zone-emissive-amber",
        )
        add(
            "chest-ridge",
            "forgecad.geometry.panel@1",
            [],
            {
                "shape": "panel",
                "size_m": [0.16, 0.78, 0.13],
                "thickness_m": 0.10,
                "bevel_m": 0.025,
                "position_m": [0.0, 1.98, 0.40],
                "rotation_rad": [0.0, 0.0, 0.0],
            },
            part_id="chest-ridge",
        )
        mirrored_box(
            "shoulder-trim",
            [0.16, 0.34, 0.16],
            [-0.90, 2.00, 0.64],
            [0.0, 0.0, -0.10],
            "shoulder-trim-pair",
            "zone-black-mechanical",
        )
        mirrored_box(
            "forearm-rail",
            [0.12, 0.58, 0.12],
            [-1.18, 0.83, 0.56],
            [0.0, 0.0, -0.10],
            "forearm-rail-pair",
            "zone-black-mechanical",
        )
        mirrored_box(
            "hip-flank",
            [0.18, 0.38, 0.20],
            [-0.50, 1.18, 0.38],
            [0.0, 0.0, 0.04],
            "hip-flank-pair",
            "zone-black-mechanical",
        )
        mirrored_box(
            "knee-cap",
            [0.30, 0.24, 0.18],
            [-0.46, 0.10, 0.58],
            [0.0, 0.0, 0.06],
            "knee-cap-pair",
        )

    if variant == "chest-profile":
        # One-Part silhouette experiment: replace only the broad rectangular
        # chest shell with a three-section tapered loft.  The vent/core/lineage
        # remain unchanged so a comparison can attribute any metric delta to
        # the chest envelope rather than to a stacked edit.
        for node in nodes:
            if node.get("node_id") == "chest-panel":
                node["operator_id"] = "forgecad.geometry.profile-loft@1"
                node["parameters"] = {
                    "shape": "profile-loft",
                    "profiles": [
                        {
                            "height_m": -0.56,
                            "points": [[-0.58, -0.24], [0.58, -0.24], [0.70, 0.02], [0.58, 0.27], [-0.58, 0.27], [-0.70, 0.02]],
                        },
                        {
                            "height_m": 0.0,
                            "points": [[-0.75, -0.28], [0.75, -0.28], [0.86, 0.04], [0.70, 0.32], [-0.70, 0.32], [-0.86, 0.04]],
                        },
                        {
                            "height_m": 0.56,
                            "points": [[-0.70, -0.26], [0.70, -0.26], [0.80, 0.03], [0.64, 0.29], [-0.64, 0.29], [-0.80, 0.03]],
                        },
                    ],
                    "position_m": [0.0, 1.98, 0.04],
                    "rotation_rad": [0.0, 0.0, 0.0],
                }
                break

    if variant in ("chest-shell-curved", "curved-tapered"):
        # One-Part form experiment: keep the observed chest envelope and its
        # vents/ridges, but give the shell a shallow front-to-back loft. The
        # previous chest-profile experiment used the depth axis as a large
        # height and regressed the camera fit; this keeps depth at +/-0.34 m.
        for node in nodes:
            if node.get("node_id") == "chest-panel":
                node["operator_id"] = "forgecad.geometry.profile-loft@1"
                node["parameters"] = {
                    "shape": "profile-loft",
                    "profiles": [
                        {"height_m": -0.34, "points": [[-0.72, -0.50], [0.72, -0.50], [0.80, -0.32], [0.74, 0.20], [0.54, 0.48], [-0.54, 0.48], [-0.74, 0.20], [-0.80, -0.32]]},
                        {"height_m": 0.0, "points": [[-0.80, -0.54], [0.80, -0.54], [0.88, -0.30], [0.82, 0.24], [0.60, 0.56], [-0.60, 0.56], [-0.82, 0.24], [-0.88, -0.30]]},
                        {"height_m": 0.34, "points": [[-0.74, -0.48], [0.74, -0.48], [0.82, -0.27], [0.76, 0.20], [0.52, 0.50], [-0.52, 0.50], [-0.76, 0.20], [-0.82, -0.27]]},
                    ],
                    "position_m": [0.0, 1.98, 0.04],
                    "rotation_rad": [0.0, 0.0, 0.0],
                }
                break

    if variant == "long-limbs":
        # One proportion hypothesis: the reference has a smaller helmet and
        # longer visible limbs than the blockout. Keep the edit bounded and
        # explicit so its comparison can be rejected independently.
        def node_parameters(node_id: str) -> dict[str, Any]:
            for node in nodes:
                if node.get("node_id") == node_id:
                    parameters = node.get("parameters")
                    if isinstance(parameters, dict):
                        return parameters
            raise ValueError(f"missing experiment node: {node_id}")

        head = node_parameters("head-shell")
        for profile in head["profiles"]:
            profile["points"] = [[x * 0.86, y * 0.86] for x, y in profile["points"]]
        head["position_m"] = [0.0, 3.18, 0.0]
        visor = node_parameters("visor")
        visor["size_m"] = [0.54, 0.21, 0.14]
        visor["thickness_m"] = 0.14
        visor["position_m"] = [0.0, 3.16, 0.44]
        node_parameters("chest-panel")["size_m"] = [1.52, 1.06, 0.62]
        node_parameters("chest-panel")["position_m"] = [0.0, 2.18, 0.04]
        node_parameters("chest-vent")["position_m"] = [0.0, 2.18, 0.40]
        node_parameters("chest-core")["position_m"] = [0.0, 1.93, 0.28]
        node_parameters("neck-ring")["position_m"] = [0.0, 2.68, 0.0]
        node_parameters("pelvis-shell")["position_m"] = [0.0, 1.32, 0.0]
        for node_id in ("shoulder-left", "shoulder-armor-left"):
            node_parameters(node_id)["position_m"][1] += 0.12
        node_parameters("upper-arm-left")["size_m"][1] = 0.92
        node_parameters("upper-arm-left")["position_m"][1] = 1.67
        node_parameters("elbow-left")["position_m"][1] = 1.06
        node_parameters("forearm-left")["size_m"][1] = 0.98
        node_parameters("forearm-left")["position_m"][1] = 0.28
        node_parameters("hand-left")["position_m"][1] = -0.30
        node_parameters("thigh-left")["size_m"][1] = 1.18
        node_parameters("thigh-left")["position_m"][1] = 0.66
        node_parameters("hip-left")["position_m"][1] = 1.16
        node_parameters("knee-left")["position_m"][1] = -0.04
        node_parameters("shin-left")["size_m"][1] = 1.06
        node_parameters("shin-left")["position_m"][1] = -0.70

    if variant == "sleek-armor":
        # A second bounded proportion hypothesis based on the real comparison
        # sheet: the reference has a taller, narrower visor/head, a tapered
        # chest shell, long narrow hanging arms and less dominant lower limbs.
        # It remains an experiment: no image bytes or inferred rear geometry
        # enter the typed program, and the baseline receipt is untouched.
        def node_parameters(node_id: str) -> dict[str, Any]:
            for node in nodes:
                if node.get("node_id") == node_id:
                    parameters = node.get("parameters")
                    if isinstance(parameters, dict):
                        return parameters
            raise ValueError(f"missing experiment node: {node_id}")

        head = node_parameters("head-shell")
        profile_list = head.get("profiles")
        if isinstance(profile_list, list):
            for profile in profile_list:
                if isinstance(profile, dict) and isinstance(profile.get("points"), list):
                    profile["points"] = [[x * 0.86, y * 0.86] for x, y in profile["points"]]
                    if isinstance(profile.get("height_m"), (int, float)):
                        profile["height_m"] = float(profile["height_m"]) * 1.16
        head["position_m"] = [-0.04, 2.90, 0.02]
        head["rotation_rad"] = [0.0, 0.34, 0.0]

        visor = node_parameters("visor")
        visor["size_m"] = [0.70, 0.22, 0.13]
        visor["thickness_m"] = 0.13
        visor["bevel_m"] = 0.055
        visor["position_m"] = [-0.08, 2.92, 0.49]
        visor["rotation_rad"] = [0.0, -0.38, 0.0]

        chest = node_parameters("chest-panel")
        chest["size_m"] = [1.46, 1.22, 0.50]
        chest["thickness_m"] = 0.15
        chest["bevel_m"] = 0.15
        chest["position_m"] = [0.0, 1.96, 0.06]
        chest["rotation_rad"] = [0.04, 0.0, 0.0]

        vent = node_parameters("chest-vent")
        vent["width_m"] = 1.04
        vent["height_m"] = 0.46
        vent["slot_width_m"] = 0.085
        vent["slot_spacing_m"] = 0.115
        vent["position_m"] = [0.0, 1.99, 0.34]
        node_parameters("chest-core")["position_m"] = [0.0, 1.75, 0.24]
        node_parameters("neck-ring")["position_m"] = [-0.04, 2.43, 0.01]

        # Keep shoulders present but reduce the rectangular mass and lengthen
        # the hanging arm silhouette.  The source remains mirrored so the
        # experiment cannot introduce left/right parameter drift.
        shoulder_armor = node_parameters("shoulder-armor-left")
        shoulder_armor["size_m"] = [0.48, 0.52, 0.52]
        shoulder_armor["thickness_m"] = 0.42
        shoulder_armor["bevel_m"] = 0.11
        shoulder_armor["position_m"] = [-0.84, 2.02, 0.30]
        upper_arm = node_parameters("upper-arm-left")
        upper_arm["size_m"] = [0.30, 0.92, 0.34]
        upper_arm["thickness_m"] = 0.28
        upper_arm["bevel_m"] = 0.06
        upper_arm["position_m"] = [-1.03, 1.58, 0.29]
        upper_arm["rotation_rad"] = [0.0, 0.0, -0.20]
        node_parameters("shoulder-left")["position_m"] = [-0.92, 2.08, 0.27]
        node_parameters("elbow-left")["position_m"] = [-1.08, 1.10, 0.32]
        forearm = node_parameters("forearm-left")
        forearm["size_m"] = [0.27, 0.92, 0.30]
        forearm["thickness_m"] = 0.24
        forearm["bevel_m"] = 0.05
        forearm["position_m"] = [-1.12, 0.62, 0.35]
        forearm["rotation_rad"] = [0.0, 0.0, -0.08]
        node_parameters("hand-left")["radii_m"] = [0.18, 0.25, 0.17]
        node_parameters("hand-left")["position_m"] = [-1.13, 0.16, 0.38]

        pelvis = node_parameters("pelvis-shell")
        pelvis["position_m"] = [0.0, 1.18, 0.0]
        node_parameters("hip-left")["position_m"] = [-0.43, 1.06, 0.22]
        thigh = node_parameters("thigh-left")
        thigh["size_m"] = [0.52, 0.98, 0.48]
        thigh["thickness_m"] = 0.40
        thigh["bevel_m"] = 0.10
        thigh["position_m"] = [-0.44, 0.58, 0.22]
        node_parameters("knee-left")["position_m"] = [-0.43, 0.04, 0.27]
        shin = node_parameters("shin-left")
        shin["size_m"] = [0.38, 0.86, 0.42]
        shin["thickness_m"] = 0.34
        shin["bevel_m"] = 0.075
        shin["position_m"] = [-0.43, -0.40, 0.25]

    if variant == "visible-thigh":
        # The supplied image ends at the upper-thigh/knee crop. This variant
        # tests whether a broader, tapered visible thigh reads closer to that
        # frame without changing the reference mask or pretending that hidden
        # shins are observed.
        def node_parameters(node_id: str) -> dict[str, Any]:
            for node in nodes:
                if node.get("node_id") == node_id:
                    parameters = node.get("parameters")
                    if isinstance(parameters, dict):
                        return parameters
            raise ValueError(f"missing experiment node: {node_id}")

        thigh = node_parameters("thigh-left")
        thigh["size_m"] = [0.78, 1.30, 0.62]
        thigh["thickness_m"] = 0.52
        thigh["bevel_m"] = 0.13
        thigh["position_m"] = [-0.50, 0.34, 0.22]
        node_parameters("hip-left")["position_m"] = [-0.50, 1.00, 0.22]
        node_parameters("knee-left")["position_m"] = [-0.50, -0.34, 0.25]
        shin = node_parameters("shin-left")
        shin["size_m"] = [0.58, 0.90, 0.54]
        shin["thickness_m"] = 0.45
        shin["bevel_m"] = 0.10
        shin["position_m"] = [-0.50, -1.16, 0.27]

    if variant == "thigh-width-mild":
        # One-Part contour correction selected from boundary_error_get:
        # narrow only the mirrored thigh shell source.  Keep its height,
        # depth, position, joints, camera and all other Parts unchanged so a
        # subsequent comparison can attribute any metric delta to thigh-pair.
        thigh = next(node for node in nodes if node.get("node_id") == "thigh-left")
        thigh_parameters = thigh.get("parameters")
        if not isinstance(thigh_parameters, dict) or thigh_parameters.get("shape") != "panel":
            raise ValueError("thigh width correction source is not a panel")
        size = thigh_parameters.get("size_m")
        if not isinstance(size, list) or len(size) != 3:
            raise ValueError("thigh width correction size is missing")
        size[0] = 0.52

    if variant == "shin-width-mild":
        # Second single-Part contour experiment, selected after the thigh
        # proposal regressed the global boundary gate. Narrow only the
        # mirrored shin shell source; preserve its height, depth, position,
        # joints, camera and every other semantic Part.
        shin = next(node for node in nodes if node.get("node_id") == "shin-left")
        shin_parameters = shin.get("parameters")
        if not isinstance(shin_parameters, dict) or shin_parameters.get("shape") != "panel":
            raise ValueError("shin width correction source is not a panel")
        size = shin_parameters.get("size_m")
        if not isinstance(size, list) or len(size) != 3:
            raise ValueError("shin width correction size is missing")
        size[0] = 0.42

    if variant == "tapered-lower":
        # Replace only the two lower-body box shells with tapered profile
        # lofts.  The reference crop shows layered, narrowing armor rather
        # than two detached rectangular feet; keep the semantic Parts and
        # mirror lineage unchanged while testing that visual hypothesis.
        def set_profile_loft(node_id: str, profiles: list[dict[str, Any]], position: list[float], rotation: list[float]) -> None:
            for node in nodes:
                if node.get("node_id") == node_id:
                    node["operator_id"] = "forgecad.geometry.profile-loft@1"
                    node["parameters"] = {
                        "shape": "profile-loft",
                        "profiles": profiles,
                        "position_m": position,
                        "rotation_rad": rotation,
                    }
                    return
            raise ValueError(f"missing lower-body node: {node_id}")

        set_profile_loft(
            "thigh-left",
            [
                {"height_m": -0.52, "points": [[-0.28, -0.27], [0.28, -0.27], [0.34, 0.18], [-0.24, 0.26]]},
                {"height_m": 0.52, "points": [[-0.20, -0.22], [0.20, -0.22], [0.25, 0.17], [-0.18, 0.22]]},
            ],
            [-0.48, 0.62, 0.24],
            [0.0, 0.0, 0.06],
        )
        set_profile_loft(
            "shin-left",
            [
                {"height_m": -0.42, "points": [[-0.20, -0.23], [0.20, -0.23], [0.26, 0.16], [-0.18, 0.20]]},
                {"height_m": 0.42, "points": [[-0.15, -0.18], [0.15, -0.18], [0.20, 0.14], [-0.13, 0.18]]},
            ],
            [-0.48, -0.34, 0.28],
            [0.0, 0.0, 0.0],
        )

    if variant in ("tapered-shells", "curved-tapered"):
        # Reference-driven shell experiment: the supplied robot has tapered
        # armor envelopes around the hanging arms and thighs, not five
        # independent rectangular slabs.  Replace only the visible outer
        # shell sources with closed profile-extrudes; joints, cables, camera,
        # material zones and semantic Part IDs stay unchanged.  The profiles
        # are authored in the local x/y silhouette plane and extruded along z
        # so the change is deterministic and directly attributable to shell
        # contour rather than a camera or material edit.
        def set_profile_extrude(
            node_id: str,
            profile: list[list[float]],
            depth: float,
            position: list[float],
            rotation: list[float],
        ) -> None:
            for node in nodes:
                if node.get("node_id") == node_id:
                    node["operator_id"] = "forgecad.geometry.profile-extrude@1"
                    node["parameters"] = {
                        "shape": "profile-extrude",
                        "profile": profile,
                        "depth_m": depth,
                        "position_m": position,
                        "rotation_rad": rotation,
                    }
                    return
            raise ValueError(f"missing tapered-shell node: {node_id}")

        set_profile_extrude(
            "shoulder-armor-left",
            [[-0.26, -0.22], [-0.34, 0.02], [-0.24, 0.28], [0.12, 0.34], [0.30, 0.16], [0.24, -0.20]],
            0.58,
            [-0.90, 2.00, 0.34],
            [0.0, 0.0, -0.10],
        )
        set_profile_extrude(
            "upper-arm-left",
            [[-0.18, -0.46], [-0.24, 0.30], [-0.16, 0.46], [0.16, 0.42], [0.22, -0.38]],
            0.40,
            [-1.10, 1.64, 0.30],
            [0.0, 0.0, -0.15],
        )
        set_profile_extrude(
            "forearm-left",
            [[-0.16, -0.48], [-0.20, 0.30], [-0.14, 0.44], [0.15, 0.38], [0.19, -0.40]],
            0.36,
            [-1.18, 0.83, 0.36],
            [0.0, 0.0, -0.10],
        )
        set_profile_extrude(
            "thigh-left",
            [[-0.32, -0.52], [-0.36, 0.26], [-0.24, 0.52], [0.22, 0.46], [0.34, -0.42]],
            0.60,
            [-0.48, 0.62, 0.24],
            [0.0, 0.0, 0.06],
        )
        set_profile_extrude(
            "shin-left",
            [[-0.23, -0.42], [-0.27, 0.28], [-0.18, 0.42], [0.17, 0.36], [0.24, -0.36]],
            0.54,
            [-0.48, -0.34, 0.28],
            [0.0, 0.0, 0.0],
        )

    if variant in ("asymmetric-stance", "asymmetric-armor", "asymmetric-linework"):
        # The reference has a visibly higher/farther right shoulder and a
        # lower left arm.  Replace only the mirrored shoulder-to-hand shells
        # with paired, independently positioned typed Parts.  This is still a
        # bounded experiment: no hidden-side geometry is inferred, and every
        # source node remains lineaged and material-bound.
        pair_specs = [
            ("shoulder-pair", "shoulder", [-0.98, 2.08, 0.28], [0.90, 2.20, 0.40]),
            ("shoulder-armor-pair", "shoulder-armor", [-0.90, 2.00, 0.34], [0.84, 2.14, 0.43]),
            ("upper-arm-pair", "upper-arm", [-1.10, 1.64, 0.30], [0.99, 1.75, 0.40]),
            ("elbow-pair", "elbow", [-1.16, 1.20, 0.34], [1.04, 1.34, 0.43]),
            ("forearm-pair", "forearm", [-1.18, 0.83, 0.36], [1.02, 0.96, 0.45]),
            ("hand-pair", "hand", [-1.18, 0.36, 0.40], [0.96, 0.50, 0.50]),
        ]

        def node_by_id(node_id: str) -> dict[str, Any]:
            for node in nodes:
                if node.get("node_id") == node_id:
                    return node
            raise ValueError(f"missing stance node: {node_id}")

        for pair_id, base_id, left_position, right_position in pair_specs:
            source = node_by_id(f"{base_id}-left")
            source_parameters = source.get("parameters")
            if not isinstance(source_parameters, dict):
                raise ValueError(f"stance source parameters missing: {base_id}")
            original_output = next((item for item in outputs if item.get("part_id") == pair_id), None)
            if not isinstance(original_output, dict):
                raise ValueError(f"stance output missing: {pair_id}")
            zone = original_output.get("material_zone_id", "zone-white-shell")
            mirror_node_id = f"{base_id}-pair"
            shaped_node_id = f"{base_id}-shaped"
            nodes[:] = [
                node
                for node in nodes
                if node.get("node_id") not in {mirror_node_id, shaped_node_id}
            ]
            source_parameters["position_m"] = left_position
            outputs[:] = [item for item in outputs if item.get("part_id") != pair_id]
            outputs.append({"part_id": f"{base_id}-left", "input_node_ids": [f"{base_id}-left"], "material_zone_id": zone, "solid": True})
            right_node = copy.deepcopy(source)
            right_node["node_id"] = f"{base_id}-right"
            right_parameters = right_node.get("parameters")
            if not isinstance(right_parameters, dict):
                raise ValueError(f"stance right parameters missing: {base_id}")
            right_parameters["position_m"] = right_position
            rotation = right_parameters.get("rotation_rad")
            if isinstance(rotation, list) and len(rotation) == 3:
                right_parameters["rotation_rad"] = [float(rotation[0]), float(rotation[1]), -float(rotation[2])]
            nodes.append(right_node)
            outputs.append({"part_id": f"{base_id}-right", "input_node_ids": [f"{base_id}-right"], "material_zone_id": zone, "solid": True})

        if variant == "asymmetric-armor":
            # A single refinement pass over the same pose: round only the
            # visible shell panels, leaving joint positions and silhouette
            # anchors untouched.
            for node_id, bevel in {
                "shoulder-armor-left": 0.15,
                "shoulder-armor-right": 0.18,
                "upper-arm-left": 0.095,
                "upper-arm-right": 0.11,
                "forearm-left": 0.085,
                "forearm-right": 0.10,
            }.items():
                node = node_by_id(node_id)
                parameters = node.get("parameters")
                if isinstance(parameters, dict) and parameters.get("shape") == "panel":
                    parameters["bevel_m"] = bevel

    if variant in ("shoulder-contour-mild", "shoulder-contour-tiny"):
        # Single-Part contour experiment: keep joints, arms, camera and
        # material recipe unchanged, but replace only the mirrored outer
        # shoulder shell with two explicitly positioned shell Parts. The
        # small height/width delta tests the visible asymmetric shoulder
        # contour without changing the rest of the silhouette.
        source_id = "shoulder-armor-left"
        shaped_id = "shoulder-armor-shaped"
        mirror_id = "shoulder-armor-pair"
        source = next(node for node in nodes if node.get("node_id") == source_id)
        source_parameters = source.get("parameters")
        if not isinstance(source_parameters, dict):
            raise ValueError("shoulder contour source parameters missing")
        original_output = next((item for item in outputs if item.get("part_id") == mirror_id), None)
        if not isinstance(original_output, dict):
            raise ValueError("shoulder contour output missing")
        zone = original_output.get("material_zone_id", "zone-white-shell")
        source_parameters["position_m"] = [-0.90, 1.99 if variant == "shoulder-contour-tiny" else 1.98, 0.34]
        source_parameters["size_m"] = [0.52, 0.46, 0.58]
        nodes[:] = [node for node in nodes if node.get("node_id") not in {shaped_id, mirror_id}]
        outputs[:] = [item for item in outputs if item.get("part_id") != mirror_id]
        outputs.append({"part_id": "shoulder-armor-left", "input_node_ids": [source_id], "material_zone_id": zone, "solid": True})
        right_node = copy.deepcopy(source)
        right_node["node_id"] = "shoulder-armor-right"
        right_parameters = right_node.get("parameters")
        if not isinstance(right_parameters, dict):
            raise ValueError("shoulder contour right parameters missing")
        right_parameters["position_m"] = [0.89, 2.04, 0.38] if variant == "shoulder-contour-tiny" else [0.88, 2.06, 0.40]
        right_parameters["size_m"] = [0.54, 0.47, 0.60] if variant == "shoulder-contour-tiny" else [0.56, 0.48, 0.62]
        right_parameters["rotation_rad"] = [0.0, 0.0, 0.09] if variant == "shoulder-contour-tiny" else [0.0, 0.0, 0.08]
        nodes.append(right_node)
        outputs.append({"part_id": "shoulder-armor-right", "input_node_ids": ["shoulder-armor-right"], "material_zone_id": zone, "solid": True})

    if variant in ("three-quarter", "pose-yaw-mild"):
        # Apply one typed transform to each final semantic Part sink. This
        # keeps source lineage intact while giving the fixed perspective
        # camera a genuine three-quarter silhouette. It is experimental only;
        # the baseline remains the canonical evidence fixture.
        yaw = -0.10 if variant == "pose-yaw-mild" else -0.30
        posed_outputs: list[dict[str, Any]] = []
        for index, output in enumerate(outputs):
            input_node_ids = output.get("input_node_ids")
            if not isinstance(input_node_ids, list):
                posed_outputs.append(output)
                continue
            posed_ids: list[str] = []
            for source_index, source in enumerate(input_node_ids):
                pose_id = f"pose-{index}-{source_index}"
                transform(pose_id, source, [0.0, 0.0, 0.0], [0.0, yaw, 0.0], [1.0, 1.0, 1.0])
                posed_ids.append(pose_id)
            posed = dict(output)
            posed["input_node_ids"] = posed_ids
            posed_outputs.append(posed)
        outputs[:] = posed_outputs

    if material_variant in ("surface-zones", "armor-shell-zones"):
        # Keep GeometryProgram material-zone bindings in lockstep with the
        # richer offline AssetPack zoning.  The geometry hash therefore binds
        # the intended surface families before AppearancePrepare, rather than
        # allowing a later material program to reinterpret a Part silently.
        for output in outputs:
            part_id = output.get("part_id")
            if isinstance(part_id, str):
                output["material_zone_id"] = detail_material_zone(part_id, material_variant)

    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "d" * 64,
        "operator_catalog_sha256": catalog_hash,
        "units": {"length": "meter", "angle": "radian", "coordinate_system": "right-handed-y-up"},
        "budgets": {"max_nodes": 128 if variant == "pose-yaw-mild" else 64, "max_triangles": 100000, "max_glb_bytes": 64 * 1024 * 1024, "max_worker_memory_bytes": 536870912, "max_runtime_ms": 10000},
        "nodes": nodes,
        "part_outputs": outputs,
    }


def main() -> int:
    args = parse_args()
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP010E source binaries were unavailable")
    if args.viewer_executable:
        require(args.viewer_executable.is_file(), "packaged Viewer executable was unavailable")
    require(args.timeout > 0, "MCP010E timeout must be positive")
    require(not args.compare or args.reference, "--compare requires --reference")
    if args.expected_build_cohort:
        require(len(args.expected_build_cohort) == 64, "invalid expected build cohort")
        mcp_identity = build_identity(args.mcp)
        runtime_identity = build_identity(args.runtime)
        require(
            mcp_identity.get("build_cohort_sha256") == args.expected_build_cohort
            and runtime_identity.get("build_cohort_sha256") == args.expected_build_cohort,
            "MCP/Runtime build cohorts did not match",
        )

    data_root = args.data_root.resolve()
    require(not data_root.exists(), "MCP010E data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)
    ready_path = data_root / "ipc" / "ready.json"
    runtime = subprocess.Popen(
        [
            str(args.runtime),
            "serve",
            "--database",
            str(data_root / "runtime.sqlite"),
            "--cas-root",
            str(data_root / "cas"),
            "--endpoint-dir",
            str(data_root / "ipc"),
            "--ready-file",
            str(ready_path),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    result: dict[str, Any] | None = None
    try:
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "ready handoff was incomplete")
        environment = os.environ.copy()
        for key in (
            "FORGECAD_RUNTIME_COMMAND",
            "FORGECAD_RUNTIME_DATA_DIR",
            "FORGECAD_RUNTIME_READY_FILE",
            "FORGECAD_RUNTIME_STATUS_FILE",
        ):
            environment.pop(key, None)
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "forgecad-mcp010e-raw-stdio", "version": "1"},
            },
        )
        require(
            initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
            "MCP010E initialize did not negotiate 2025-06-18",
        )
        client.notify("notifications/initialized")

        listed = client.request("tools/list")
        tools = listed.get("result", {}).get("tools")
        require(isinstance(tools, list), "MCP010E tools/list did not return an array")
        tool_names = {item.get("name") for item in tools if isinstance(item, dict)}
        required_tools = {
            "material_pack_get",
            "project_create",
            "reference_import",
            "operator_catalog_get",
            "geometry_program_hash",
            "geometry_prepare",
            "appearance_prepare",
            "render_pass_get",
        }
        require(required_tools.issubset(tool_names), "MCP010E required tool set was incomplete")
        pack = client.tool("material_pack_get")
        require(
            isinstance(pack, dict)
            and pack.get("schema_version") == "MaterialPackManifest@1"
            and pack.get("pack_id") == "forgecad-hard-surface-robot"
            and isinstance(pack.get("canonical_sha256"), str)
            and len(pack["canonical_sha256"]) == 64,
            "material_pack_get did not return the trusted offline pack",
        )
        pack_hash = pack["canonical_sha256"]
        textures = pack.get("textures")
        require(isinstance(textures, list) and len(textures) >= 7, "offline pack texture manifest was incomplete")
        require(
            all(item.get("file", "").startswith("textures/") and item.get("file", "").endswith(".png") for item in textures),
            "offline pack contained a non-embedded texture path",
        )

        project = client.tool("project_create", {"name": "MCP010E offline PBR pack", "policy": {"profile": "mvp"}})
        project_id = project.get("project_id") if isinstance(project, dict) else None
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        if args.reference:
            reference_path = args.reference.expanduser().resolve()
            require(reference_path.is_file() and not reference_path.is_symlink(), "reference must be a regular file")
            reference_png = reference_path.read_bytes()
            reference_mime = "image/png" if reference_png.startswith(b"\x89PNG\r\n\x1a\n") else "image/jpeg"
            reference_declaration = "The user supplied and authorized this reference for local ForgeCAD modeling."
        else:
            reference_png = bytes.fromhex(
                "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c020000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082"
            )
            reference_mime = "image/png"
            reference_declaration = "Synthetic isolated MCP010E gate reference."
        reference_result = client.tool(
            "reference_import",
            {
                "project_id": project_id,
                "source": {"kind": "inline_content", "mime": reference_mime, "content_base64": base64.b64encode(reference_png).decode("ascii")},
                "authorization": {"user_authorized": True, "declaration": reference_declaration},
            },
        )
        reference = reference_result.get("reference") if isinstance(reference_result, dict) else None
        require(isinstance(reference, dict), "reference_import omitted evidence")
        reference_id = reference.get("reference_id")
        require(isinstance(reference_id, str) and reference_id, "reference_import omitted reference_id")

        catalog = client.tool("operator_catalog_get")
        catalog_hash = catalog.get("canonical_sha256") if isinstance(catalog, dict) else None
        require(isinstance(catalog_hash, str) and len(catalog_hash) == 64, "operator catalog hash unavailable")
        draft = robot_detail_program_draft(project_id, catalog_hash, args.geometry_variant, args.material_variant) if args.detail else v2_program_draft(project_id, catalog_hash)
        # The E path embeds seven 512px PNG textures. Keep the geometry budget
        # within the product ceiling while allowing the self-contained GLB to
        # carry those bytes.
        draft["budgets"]["max_glb_bytes"] = 64 * 1024 * 1024
        hashed = client.tool(
            "geometry_program_hash",
            {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft},
        )
        geometry_hash = hashed.get("canonical_sha256") if isinstance(hashed, dict) else None
        require(isinstance(geometry_hash, str) and len(geometry_hash) == 64, "geometry program hash unavailable")
        geometry = copy.deepcopy(draft)
        geometry["canonical_sha256"] = geometry_hash
        prepared_geometry = client.tool(
            "geometry_prepare",
            {"project_id": project_id, "request": {"typed": "geometry", "reference_id": reference_id, "geometry_program": geometry}},
        )
        geometry_artifact = prepared_geometry.get("artifact") if isinstance(prepared_geometry, dict) else None
        require(isinstance(geometry_artifact, dict) and geometry_artifact.get("hard_gate_passed") is True, "geometry prerequisite did not pass")
        if args.detail and args.geometry_variant == "surface-linework":
            linework_parts = len(geometry_artifact.get("part_ids") or [])
            linework_triangles = geometry_artifact.get("triangle_count")
            require(
                linework_parts == 26 and linework_triangles == 4704,
                f"surface-linework fixture expected 26 parts/4704 triangles, got {linework_parts}/{linework_triangles}",
            )

        appearance = appearance_program(
            project_id,
            geometry_hash,
            pack_hash,
            geometry_artifact.get("part_ids") if args.detail else None,
            args.material_variant,
        )
        appearance_response = client.request(
            "tools/call",
            {
                "name": "appearance_prepare",
                "arguments": {
                    "project_id": project_id,
                    "request": {
                        "typed": "appearance",
                        "reference_id": reference_id,
                        "geometry_program": geometry,
                        "appearance_program": appearance,
                    },
                },
            },
        )
        if isinstance(appearance_response.get("result"), dict) and appearance_response["result"].get("isError"):
            raise GateFailure("appearance_prepare rejected")
        prepared = appearance_response.get("result", {}).get("structuredContent")
        artifact = prepared.get("artifact") if isinstance(prepared, dict) else None
        render_set = prepared.get("render_set") if isinstance(prepared, dict) else None
        require(
            prepared.get("schema_version") == "AppearancePrepareResult@2"
            and isinstance(artifact, dict)
            and artifact.get("hard_gate_passed") is True,
            "appearance_prepare did not return a strict V2 artifact",
        )
        integrity = artifact.get("integrity")
        require(
            isinstance(integrity, dict)
            and integrity.get("external_uri_count") == 0
            and integrity.get("uv_non_finite_count") == 0
            and integrity.get("zero_area_uv_triangle_count") == 0
            and integrity.get("tangent_non_finite_count") == 0,
            "appearance artifact readback did not pass UV/tangent/embedded URI checks",
        )
        if args.detail and args.material_variant in ("surface-zones", "armor-shell-zones"):
            material_zone_ids = artifact.get("material_zone_ids")
            expected_zone_count = 8 if args.material_variant == "surface-zones" else (7 if args.geometry_variant in ("surface-linework", "chest-shell-curved", "chest-top-cap", "chest-top-cap-mild", "chest-wedge", "chest-wedge-visor", "tapered-shells", "asymmetric-linework", "head-turn-mild", "shoulder-contour-mild", "shoulder-contour-tiny") else 6)
            require(
                isinstance(material_zone_ids, list) and len(material_zone_ids) == expected_zone_count,
                f"{args.material_variant} fixture expected {expected_zone_count} material zones, got {material_zone_ids}",
            )
        passes = ["beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch"]
        require(
            isinstance(render_set, dict)
            and render_set.get("schema_version") == "RenderSet@2"
            and render_set.get("passes") == passes
            and isinstance(render_set.get("pass_artifacts"), dict)
            and len(render_set["pass_artifacts"]) == 9,
            "appearance RenderSet@2 did not contain the fixed nine AOVs",
        )
        render_set_hash = prepared.get("render_set_object_sha256")
        require(isinstance(render_set_hash, str) and len(render_set_hash) == 64, "render set CAS hash was missing")
        comparison_summary: dict[str, Any] = {"status": "NOT_RUN"}
        if args.compare:
            width, height = png_dimensions(reference_png)
            candidate = prepared.get("candidate") if isinstance(prepared, dict) else None
            candidate_id = candidate.get("candidate_id") if isinstance(candidate, dict) else None
            require(isinstance(candidate_id, str) and candidate_id, "appearance candidate id was missing for comparison")
            landmarks, regions = robot_reference_annotations() if args.reference else ([], [])
            view_spec: dict[str, Any] = {
                "schema_version": "ReferenceViewSpec@1",
                "reference_id": reference_id,
                "reference_sha256": reference.get("object_sha256"),
                "view_id": "three-quarter-user-reference",
                "source_view": "three-quarter",
                "image": {"width": width, "height": height, "rotation_degrees": 0, "crop": {"x": 0, "y": 0, "width": 1, "height": 1}},
                "landmarks": landmarks,
                "regions": regions,
                "canonical_sha256": "",
            }
            view_spec["canonical_sha256"] = canonical_hash(view_spec)
            comparison = client.tool(
                "reference_compare_prepare",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "reference_id": reference_id,
                    "view_spec": view_spec,
                    **(
                        {"camera": explicit_camera(args.camera_position, args.camera_target)}
                        if args.camera_position
                        else {}
                    ),
                },
            )
            comparison_render_set = comparison.get("render_set") if isinstance(comparison, dict) else None
            require(
                isinstance(comparison_render_set, dict)
                and comparison_render_set.get("schema_version") == "RenderSet@2"
                and comparison_render_set.get("passes") == passes,
                "reference_compare_prepare did not return the fixed RenderSet@2",
            )
            comparison_hash = comparison.get("comparison_report_object_sha256")
            comparison_render_set_hash = comparison.get("render_set_object_sha256")
            require(
                isinstance(comparison_hash, str)
                and len(comparison_hash) == 64
                and isinstance(comparison_render_set_hash, str)
                and len(comparison_render_set_hash) == 64,
                "reference comparison hashes were missing",
            )
            quality_raw = client.request(
                "tools/call",
                {"name": "quality_get", "arguments": {"candidate_id": candidate_id, "reference_id": reference_id}},
            )
            quality_result = quality_raw.get("result") if isinstance(quality_raw, dict) else None
            require(isinstance(quality_result, dict) and not quality_result.get("isError"), "quality_get returned an error")
            quality_response = quality_result.get("structuredContent")
            quality_report = (
                quality_response.get("quality_report") or quality_response
                if isinstance(quality_response, dict)
                else quality_response
            )
            require(
                isinstance(quality_report, dict)
                and quality_report.get("render_set_hash") == comparison_render_set_hash
                and quality_report.get("comparison_report_hash") == comparison_hash,
                f"quality_get binding mismatch (schema={quality_report.get('schema_version') if isinstance(quality_report, dict) else None}, render={quality_report.get('render_set_hash') if isinstance(quality_report, dict) else None}, compare={quality_report.get('comparison_report_hash') if isinstance(quality_report, dict) else None}, reference_compare={json.dumps(quality_report.get('reference_compare'), sort_keys=True)[:450] if isinstance(quality_report, dict) else None}, expected_render={comparison_render_set_hash}, expected_compare={comparison_hash})",
            )
            render_set_hash = comparison_render_set_hash
            comparison_summary = {
                "status": "PASS_TRANSPORT_WITH_METRICS",
                "render_set_hash": comparison_render_set_hash,
                "comparison_report_hash": comparison_hash,
                "metrics": comparison.get("comparison_report", {}).get("metrics") if isinstance(comparison.get("comparison_report"), dict) else None,
                "quality_visual_status": quality_report.get("visual_status"),
                "quality_hard_gate_passed": quality_report.get("hard_gate_passed"),
            }
            if args.fit_plan_input_dir:
                fit_plan_root = args.fit_plan_input_dir.expanduser().resolve()
                fit_plan_root.mkdir(mode=0o700, parents=True, exist_ok=True)
                comparison_report = comparison.get("comparison_report")
                require(isinstance(comparison_report, dict), "comparison report was unavailable for fit-plan input")
                (fit_plan_root / "comparison.json").write_text(
                    json.dumps(comparison_report, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                (fit_plan_root / "view-spec.json").write_text(
                    json.dumps(view_spec, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                (fit_plan_root / "operator-catalog.json").write_text(
                    json.dumps(catalog, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
        image_response = client.request(
            "tools/call", {"name": "render_pass_get", "arguments": {"render_set_hash": render_set_hash, "pass": "beauty"}}
        )
        content = image_response.get("result", {}).get("content") if isinstance(image_response.get("result"), dict) else None
        image = next((item for item in content or [] if isinstance(item, dict) and item.get("type") == "image"), None)
        require(isinstance(image, dict) and image.get("mimeType") == "image/png" and image.get("data"), "render_pass_get did not return an image block")
        if args.render_dir:
            render_dir = args.render_dir.expanduser().resolve()
            render_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
            (render_dir / "beauty.png").write_bytes(base64.b64decode(image["data"], validate=True))
            if args.compare:
                for pass_name in ("silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch"):
                    pass_response = client.request(
                        "tools/call",
                        {"name": "render_pass_get", "arguments": {"render_set_hash": render_set_hash, "pass": pass_name}},
                    )
                    pass_content = pass_response.get("result", {}).get("content") if isinstance(pass_response.get("result"), dict) else None
                    pass_image = next((item for item in pass_content or [] if isinstance(item, dict) and item.get("type") == "image"), None)
                    require(isinstance(pass_image, dict) and pass_image.get("mimeType") == "image/png" and pass_image.get("data"), f"render_pass_get did not return {pass_name}")
                    (render_dir / f"{pass_name}.png").write_bytes(base64.b64decode(pass_image["data"], validate=True))
        packaged_viewer: dict[str, Any] | None = None
        if args.viewer_executable:
            viewer_environment = os.environ.copy()
            viewer_environment["FORGECAD_RUNTIME_DATA_DIR"] = str(args.data_root.expanduser().resolve())
            identity = subprocess.run(
                [str(args.viewer_executable), "--build-identity"],
                check=False,
                capture_output=True,
                text=True,
                timeout=args.timeout,
                env=viewer_environment,
            )
            require(identity.returncode == 0, "packaged Viewer build identity command failed")
            identity_value = json.loads(identity.stdout)
            require(
                isinstance(identity_value, dict)
                and identity_value.get("schema_version") == "ForgeCADDevBuildIdentity@1"
                and identity_value.get("component") == "forgecad-viewer"
                and isinstance(identity_value.get("build_cohort_sha256"), str)
                and len(identity_value["build_cohort_sha256"]) == 64,
                "packaged Viewer build identity was incomplete",
            )
            projection = subprocess.run(
                [str(args.viewer_executable), "--viewer-read-model"],
                check=False,
                capture_output=True,
                text=True,
                timeout=args.timeout,
                env=viewer_environment,
            )
            require(projection.returncode == 0, "packaged Viewer read-model command failed")
            projection_value = json.loads(projection.stdout)
            require(
                isinstance(projection_value, dict)
                and projection_value.get("schema_version") == "ForgeCADViewerReadModel@1"
                and projection_value.get("status") == "Ready"
                and projection_value.get("retryable") is False
                and isinstance(projection_value.get("projects"), list)
                and len(projection_value["projects"]) >= 1,
                "packaged Viewer did not return a Ready read model",
            )
            project_views = projection_value["projects"]
            candidate_views = [
                candidate
                for project in project_views
                for candidate in (project.get("candidates") or [])
                if isinstance(candidate, dict)
            ]
            artifact_views = [candidate.get("artifact") for candidate in candidate_views if candidate.get("artifact")]
            quality_views = [candidate.get("quality") for candidate in candidate_views if candidate.get("quality")]
            reference_views = [candidate.get("reference") for candidate in candidate_views if candidate.get("reference")]
            packaged_viewer = {
                "status": "PASS_STRUCTURAL",
                "build_cohort_sha256": identity_value["build_cohort_sha256"],
                "schema_version": projection_value["schema_version"],
                "project_count": len(project_views),
                "candidate_count": len(candidate_views),
                "artifact_view_count": len(artifact_views),
                "quality_view_count": len(quality_views),
                "reference_view_count": len(reference_views),
                "candidate_artifact_lineage_present": any(
                    isinstance(artifact, dict)
                    and isinstance(artifact.get("part_ids"), list)
                    and isinstance(artifact.get("material_zone_ids"), list)
                    for artifact in artifact_views
                ),
                "candidate_reference_quality_projection_present": any(
                    isinstance(candidate.get("quality"), dict)
                    and isinstance(candidate.get("reference"), dict)
                    for candidate in candidate_views
                ),
                "ui_e2e": "NOT_RUN",
            }

        result = {
            "status": "PASS",
            "geometry_fixture": "mcp010d-detail" if args.detail else "primitive-v2",
            "geometry_variant": args.geometry_variant,
            "reference_mode": "user-authorized-inline" if args.reference else "synthetic-isolated",
            "reference_sha256": reference.get("object_sha256"),
            "reference_size_bytes": len(reference_png),
            "geometry_program_sha256": geometry_hash,
            "geometry_artifact_sha256": geometry_artifact.get("object_sha256"),
            "geometry_part_count": len(geometry_artifact.get("part_ids") or []),
            "geometry_triangle_count": geometry_artifact.get("triangle_count"),
            "tool_count": "21 read + 16 write",
            "pack_id": pack["pack_id"],
            "pack_manifest_sha256": pack_hash,
            "texture_manifest_count": len(textures),
            "appearance_program": "AppearanceProgram@2",
            "artifact_readback": "ArtifactReadback@2 hard_gate_passed",
            "appearance_artifact_sha256": artifact.get("object_sha256"),
            "appearance_part_count": len(artifact.get("part_ids") or []),
            "appearance_triangle_count": artifact.get("triangle_count"),
            "material_zone_count": len(artifact.get("material_zone_ids") or []),
            "material_zone_ids": artifact.get("material_zone_ids"),
            "embedded_texture_uri_count": integrity["external_uri_count"],
            "uv_atlas": "512px deterministic triangle-chart grid",
            "aov_passes": passes,
            "mcp_image_block": "PASS",
            "reference_compare": comparison_summary,
            "visual_likeness": comparison_summary.get("quality_visual_status", "NOT_RUN"),
            "human_review": "NOT_RUN",
            "packaged_viewer": packaged_viewer,
            "persistent_user_data_touched": False,
        }
    finally:
        cleanup_error: BaseException | None = None
        if client is not None:
            try:
                client.close()
            except BaseException as error:
                cleanup_error = error
        if ready is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException as error:
                if cleanup_error is None:
                    cleanup_error = error
        elif runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error

    receipt = {
        "schema_version": "ForgeCADMCP010FViewerReadModelProbe@1" if args.receipt_task_id == "FGC-MCP010F" else "ForgeCADMCP010ERawStdioProbe@1",
        "task_id": args.receipt_task_id,
        "protocol_version": MCP_PROTOCOL_VERSION,
        "persistent_user_data_touched": False,
        "runtime_cleanup": "PASS",
        **(result or {}),
    }
    if args.evidence:
        resolved = args.evidence if args.evidence.is_absolute() else Path(__file__).resolve().parents[1] / args.evidence
        evidence_root = Path(__file__).resolve().parents[1] / "docs" / "evidence"
        resolved.resolve().relative_to(evidence_root.resolve())
        resolved.parent.mkdir(parents=True, exist_ok=True)
        resolved.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(
            json.dumps(
                {
                    "schema_version": "ForgeCADMCP010ERawStdioProbe@1",
                    "task_id": "FGC-MCP010E",
                    "status": "FAIL",
                    "reason": str(error)[:2000],
                    "persistent_user_data_touched": False,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1)
