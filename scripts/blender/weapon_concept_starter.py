"""Blender background script for editable core/front Weapon Concept starter assets."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

import bpy
from mathutils import Vector


MATERIALS = {
    "MAT_primary": ((0.035, 0.055, 0.08, 1.0), 0.72, 0.34),
    "MAT_secondary": ((0.12, 0.16, 0.21, 1.0), 0.58, 0.42),
    "MAT_accent": ((0.62, 0.018, 0.012, 1.0), 0.42, 0.30),
}


@dataclass(frozen=True)
class Part:
    name: str
    center_mm: tuple[float, float, float]
    size_mm: tuple[float, float, float]
    material: str
    bevel_mm: float = 2.0
    profile: str = "box"


@dataclass(frozen=True)
class Connector:
    connector_id: str
    slot: str
    connector_type: str
    position_mm: tuple[float, float, float]


@dataclass(frozen=True)
class Module:
    module_id: str
    category: str
    parts: tuple[Part, ...]
    connectors: tuple[Connector, ...]


STARTER_MODULES = (
    Module(
        "module_core_shell_01",
        "core_shell",
        (
            Part("main_body", (0, 0, 0), (100, 46, 40), "MAT_primary", 4.0, "wedge"),
            Part("upper_spine", (-4, 21, 0), (76, 10, 30), "MAT_secondary", 2.5),
            Part("lower_keel", (-10, -21, 0), (48, 9, 32), "MAT_secondary", 2.0),
            Part("side_plate_a", (25, 0, 20), (28, 26, 3), "MAT_accent", 1.0),
            Part("side_plate_b", (-28, 0, -20), (20, 22, 3), "MAT_accent", 1.0),
            Part("top_visual_rail_a", (-27, 26, 0), (28, 4, 14), "MAT_secondary", 1.0),
            Part("top_visual_rail_b", (8, 26, 0), (30, 4, 14), "MAT_secondary", 1.0),
            Part(
                "left_visual_strake", (12, 2, 22), (56, 14, 2.5), "MAT_secondary", 0.8
            ),
            Part(
                "right_visual_strake",
                (-20, -2, -22),
                (44, 12, 2.5),
                "MAT_secondary",
                0.8,
            ),
            Part("signal_marker_a", (35, 0, 23.5), (10, 9, 1.5), "MAT_accent", 0.6),
            Part("signal_marker_b", (20, 0, 23.5), (7, 9, 1.5), "MAT_accent", 0.6),
            Part("lower_visual_guard", (-9, -26, 0), (35, 4, 24), "MAT_secondary", 1.0),
        ),
        (
            Connector("connector_core_front", "core.front", "shell_mount", (-50, 0, 0)),
            Connector("connector_core_rear", "core.rear", "rear_mount", (50, 0, 0)),
            Connector("connector_core_grip", "core.grip", "grip_mount", (14, -24, 0)),
            Connector("connector_core_top", "core.top", "top_mount", (0, 24, 0)),
            Connector("connector_core_side", "core.side", "side_mount", (0, 0, 20)),
            Connector(
                "connector_core_lower", "core.lower", "lower_mount", (-12, -24, 0)
            ),
            Connector(
                "connector_core_storage", "core.storage", "storage_mount", (30, -24, 0)
            ),
            Connector("connector_core_armor", "core.armor", "armor_mount", (0, 0, -20)),
        ),
    ),
    Module(
        "module_front_shell_01",
        "front_shell",
        (
            Part("main_wedge", (-31, 0, 0), (62, 32, 32), "MAT_primary", 3.0, "wedge"),
            Part("upper_visual_tube", (-37, 8, 25.5), (74, 12, 12), "MAT_secondary", 1.2, "cylinder_x"),
            Part("lower_visual_tube", (-37, -8, 25.5), (74, 12, 12), "MAT_secondary", 1.2, "cylinder_x"),
            Part("upper_side_visual_tube", (-42, 8, -26), (64, 12, 12), "MAT_secondary", 1.2, "cylinder_x"),
            Part("lower_side_visual_tube", (-42, -8, -26), (64, 12, 12), "MAT_secondary", 1.2, "cylinder_x"),
            Part("upper_rib", (-36, 16, 0), (42, 6, 27), "MAT_secondary", 1.5),
            Part("nose_cap", (-60, 5, 0), (14, 22, 28), "MAT_secondary", 2.5, "wedge"),
            Part("accent_fin", (-44, 0, 17), (20, 15, 2.5), "MAT_accent", 0.8),
            Part("top_visual_split", (-27, 21, 0), (28, 4, 20), "MAT_secondary", 0.8),
            Part("left_visual_fin", (-43, 2, 18), (24, 9, 2), "MAT_secondary", 0.6),
            Part("signal_tab", (-56, -2, 18.5), (9, 8, 1.2), "MAT_accent", 0.5),
        ),
        (Connector("connector_front_01_core", "front.core", "shell_mount", (0, 0, 0)),),
    ),
    Module(
        "module_front_shell_02",
        "front_shell",
        (
            Part("main_wedge", (-37, 0, 0), (74, 28, 38), "MAT_primary", 4.0, "wedge"),
            Part("upper_visual_tube", (-43, 7, 27), (86, 11, 11), "MAT_secondary", 1.1, "cylinder_x"),
            Part("lower_visual_tube", (-43, -7, 27), (86, 11, 11), "MAT_secondary", 1.1, "cylinder_x"),
            Part("upper_side_visual_tube", (-48, 7, -28), (74, 11, 11), "MAT_secondary", 1.1, "cylinder_x"),
            Part("lower_side_visual_tube", (-48, -7, -28), (74, 11, 11), "MAT_secondary", 1.1, "cylinder_x"),
            Part("upper_bridge", (-34, 15, 0), (40, 8, 32), "MAT_secondary", 2.0),
            Part("nose_block", (-68, 7, 0), (16, 18, 30), "MAT_secondary", 2.0, "wedge"),
            Part("lower_step", (-20, -15, 0), (32, 6, 30), "MAT_accent", 1.0),
            Part("tip_accent", (-61, 0, -20), (16, 12, 2.5), "MAT_accent", 0.8),
            Part("upper_visual_frame", (-35, 21, 0), (38, 4, 26), "MAT_secondary", 1.0),
            Part("left_armor_band", (-43, 1, 21), (34, 12, 2.5), "MAT_secondary", 0.8),
            Part("signal_marker", (-67, 0, 21.5), (8, 9, 1.5), "MAT_accent", 0.5),
        ),
        (Connector("connector_front_02_core", "front.core", "shell_mount", (0, 0, 0)),),
    ),
)


# This deliberately remains an editable visual candidate, not a release asset pack.
# The stable IDs and connector semantics match the reference Pack so an artist can
# take the same graph from the three-module authoring exercise to a full assembly.
FULL_CANDIDATE_MODULES = STARTER_MODULES + (
    Module(
        "module_rear_shell_01",
        "rear_shell",
        (
            Part("rear_body", (22, 0, 0), (42, 38, 36), "MAT_primary", 3.5, "reverse_wedge"),
            Part("rear_cap", (40, 0, 0), (12, 26, 30), "MAT_secondary", 2.0),
            Part("upper_rear_spine", (20, 20, 0), (34, 5, 24), "MAT_secondary", 1.0),
            Part("rear_side_armor", (26, 0, 19), (26, 16, 2.5), "MAT_secondary", 0.8),
            Part("rear_signal_strip", (38, 0, 19), (9, 9, 1.5), "MAT_accent", 0.5),
            Part("lower_rear_step", (17, -20, 0), (22, 5, 21), "MAT_accent", 0.8),
        ),
        (Connector("connector_rear_core", "rear.core", "rear_mount", (0, 0, 0)),),
    ),
    Module(
        "module_grip_shell_01",
        "grip_shell",
        (
            Part("grip_main", (2, -32, 0), (30, 59, 31), "MAT_primary", 4.0, "grip_taper"),
            Part("grip_backstrap", (11, -35, 0), (10, 51, 34), "MAT_secondary", 2.2),
            Part("grip_front_guard", (-12, -13, 0), (8, 20, 29), "MAT_secondary", 1.5),
            Part(
                "grip_side_inlay_a", (1, -36, 17), (20, 31, 2.4), "MAT_secondary", 0.7
            ),
            Part("grip_side_inlay_b", (-4, -51, 17), (13, 13, 2.4), "MAT_accent", 0.6),
            Part("grip_base_plate", (4, -64, 0), (28, 8, 32), "MAT_accent", 1.2),
        ),
        (Connector("connector_grip_core", "grip.core", "grip_mount", (0, 0, 0)),),
    ),
    Module(
        "module_top_accessory_01",
        "top_accessory",
        (
            Part("top_body", (0, 12, 0), (40, 18, 16), "MAT_primary", 2.6, "wedge"),
            Part("top_bridge", (0, 3, 0), (28, 8, 12), "MAT_secondary", 1.0),
            Part("top_frame_left", (-12, 18, 0), (8, 5, 18), "MAT_secondary", 0.7),
            Part("top_frame_right", (12, 18, 0), (8, 5, 18), "MAT_secondary", 0.7),
            Part("top_signal", (0, 21, 9), (12, 6, 1.6), "MAT_accent", 0.5),
        ),
        (Connector("connector_top_core", "top.core", "top_mount", (0, 0, 0)),),
    ),
    Module(
        "module_side_accessory_01",
        "side_accessory",
        (
            Part("side_body", (0, 0, 12), (34, 21, 16), "MAT_primary", 2.2),
            Part("side_root", (0, 0, 3), (24, 13, 8), "MAT_secondary", 1.0),
            Part("side_visual_frame", (0, 13, 13), (25, 3, 13), "MAT_secondary", 0.6),
            Part("side_fin", (8, -8, 18), (16, 8, 3), "MAT_secondary", 0.6),
            Part("side_marker", (-10, 0, 20), (8, 7, 1.4), "MAT_accent", 0.5),
        ),
        (Connector("connector_side_core", "side.core", "side_mount", (0, 0, 0)),),
    ),
    Module(
        "module_lower_structure_01",
        "lower_structure",
        (
            Part("lower_body", (-25, -14, 0), (42, 20, 24), "MAT_primary", 2.8),
            Part("lower_root", (-22, -3, 0), (30, 8, 17), "MAT_secondary", 1.0),
            Part("lower_keel", (-32, -23, 0), (27, 5, 20), "MAT_secondary", 0.8),
            Part(
                "lower_side_plate", (-15, -14, 13), (18, 12, 2.2), "MAT_secondary", 0.6
            ),
            Part("lower_signal_tab", (-38, -19, 13), (9, 7, 1.4), "MAT_accent", 0.5),
        ),
        (Connector("connector_lower_core", "lower.core", "lower_mount", (0, 0, 0)),),
    ),
    Module(
        "module_storage_visual_01",
        "storage_visual",
        (
            Part("storage_body", (25, -23, 0), (30, 32, 29), "MAT_primary", 3.0),
            Part("storage_root", (8, 3, 0), (16, 6, 16), "MAT_secondary", 1.2),
            Part("storage_front_band", (8, -23, 0), (6, 30, 27), "MAT_secondary", 0.8),
            Part(
                "storage_side_plate", (26, -23, 16), (19, 22, 2.4), "MAT_secondary", 0.7
            ),
            Part("storage_base", (27, -42, 0), (27, 7, 31), "MAT_accent", 1.0),
            Part("storage_marker", (33, -31, 16), (8, 7, 1.4), "MAT_accent", 0.5),
        ),
        (
            Connector(
                "connector_storage_core", "storage.core", "storage_mount", (0, 0, 0)
            ),
        ),
    ),
    Module(
        "module_armor_panel_01",
        "armor_panel",
        (
            Part("armor_base", (0, 0, -7), (50, 28, 10), "MAT_primary", 2.0),
            Part("armor_root", (0, 0, -2), (32, 16, 5), "MAT_secondary", 0.8),
            Part("armor_upper_ridge", (0, 16, -8), (34, 4, 9), "MAT_secondary", 0.6),
            Part("armor_lower_ridge", (0, -16, -8), (34, 4, 9), "MAT_secondary", 0.6),
            Part("armor_signal_tile", (13, 0, -13), (10, 8, 1.5), "MAT_accent", 0.5),
        ),
        (Connector("connector_armor_core", "armor.core", "armor_mount", (0, 0, 0)),),
    ),
)

MODULE_SETS = {
    "starter": STARTER_MODULES,
    "full_candidate": FULL_CANDIDATE_MODULES,
}


def module_ids_for_set(module_set: str) -> tuple[str, ...]:
    return tuple(module.module_id for module in MODULE_SETS[module_set])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--module-set", choices=sorted(MODULE_SETS), default="starter")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args(_script_args())
    output_root = args.output_root.expanduser().resolve()
    repository_root = Path(__file__).resolve().parents[2]
    committed_pack_root = repository_root / "assets" / "module-packs"
    if output_root.is_relative_to(committed_pack_root):
        raise RuntimeError("starter output cannot target committed assets/module-packs")
    if output_root.exists() and any(output_root.iterdir()):
        if not args.force:
            raise RuntimeError(
                "starter output is not empty; use --force for a deliberate rebuild"
            )
        shutil.rmtree(output_root)
    (output_root / "LICENSES").mkdir(parents=True, exist_ok=True)
    license_text = (
        "SPDX-License-Identifier: LicenseRef-ForgeCAD-Authoring-Starter\n"
        "Editable non-functional concept/game/film-prop starter generated by ForgeCAD.\n"
        "Not final art and not manufacturing documentation.\n"
    )
    (output_root / "LICENSES" / "PACK.txt").write_text(license_text, encoding="utf-8")
    modules = MODULE_SETS[args.module_set]
    entries = [_build_module(output_root, module, license_text) for module in modules]
    pack = {
        "schema_version": "ModulePackManifest@1",
        "pack_id": "pack_weapon_concept_v1",
        "profile_id": "profile_weapon_concept_v1",
        "name": (
            "Weapon Concept v1 Blender authoring starter"
            if args.module_set == "starter"
            else "Weapon Concept v1 Blender full visual candidate"
        ),
        "version": "0.1.0",
        "description": (
            "Editable Blender "
            + (
                "starter for three"
                if args.module_set == "starter"
                else "full visual candidate for ten"
            )
            + " non-functional concept/game/film-prop modules; requires human art review before promotion."
        ),
        "intended_uses": [
            "visual_asset",
            "game_asset",
            "film_prop",
            "non_functional_display",
        ],
        "non_functional_only": True,
        "units": "millimeter",
        "up_axis": "Y",
        "forward_axis": "-Z",
        "handedness": "right",
        "license": {
            "spdx_expression": "LicenseRef-ForgeCAD-Authoring-Starter",
            "license_path": "LICENSES/PACK.txt",
        },
        "modules": entries,
    }
    _write_json(output_root / "pack.json", pack)


def _build_module(
    output_root: Path, module: Module, license_text: str
) -> dict[str, str]:
    _reset_scene()
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    # Blender 4 exposed EEVEE Next as a separate enum; Blender 5 folds it
    # back into BLENDER_EEVEE. Keep the generated authoring pack runnable on
    # both the historical 4.x workstation and the current local Blender 5.x.
    scene.render.engine = (
        "BLENDER_EEVEE"
        if "BLENDER_EEVEE" in {item.identifier for item in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items}
        else "BLENDER_EEVEE_NEXT"
    )
    scene.render.resolution_x = 512
    scene.render.resolution_y = 512
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.film_transparent = True
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.view_settings.exposure = -1.5
    collection = bpy.data.collections.new(f"MOD_{module.module_id}")
    scene.collection.children.link(collection)
    materials = _create_materials()
    mesh_objects = []
    for part in module.parts:
        object_name = f"GEO_{module.module_id}_LOD0_{len(mesh_objects) + 1:02d}"
        mesh_objects.append(_create_part(object_name, part, materials, collection))
        for detail in _surface_detail_parts(part):
            detail_name = f"GEO_{module.module_id}_LOD0_{len(mesh_objects) + 1:02d}"
            mesh_objects.append(_create_part(detail_name, detail, materials, collection))
    for connector in module.connectors:
        _create_connector(connector, collection)
    _create_render_rig(mesh_objects)
    scene["forgecad_authoring_metadata"] = json.dumps(
        {
            "schema_version": "ForgeCADBlenderAuthoring@1",
            "module_id": module.module_id,
            "category": module.category,
            "pack_id": "pack_weapon_concept_v1",
            "asset_id": f"asset_{module.module_id.removeprefix('module_')}",
            "connectors": [
                {
                    "connector_id": connector.connector_id,
                    "slot": connector.slot,
                    "connector_type": connector.connector_type,
                    "scale_range": [0.9, 1.1],
                    "exclusive": True,
                }
                for connector in module.connectors
            ],
        },
        sort_keys=True,
    )

    module_root = output_root / "modules" / module.module_id
    source_root = output_root / "sources"
    module_root.mkdir(parents=True, exist_ok=True)
    source_root.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(source_root / f"{module.module_id}.blend"))

    for item in bpy.context.selected_objects:
        item.select_set(False)
    # Connector empties remain in the editable .blend source. Runtime Connector truth
    # lives in module.json, so GLB exports only identity-transform mesh objects.
    for item in mesh_objects:
        item.select_set(True)
    bpy.context.view_layer.objects.active = mesh_objects[0]
    glb_path = module_root / "model.glb"
    bpy.ops.export_scene.gltf(
        filepath=str(glb_path),
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_texcoords=True,
        export_normals=True,
        export_materials="EXPORT",
    )

    scene.render.filepath = str(module_root / "thumbnail.png")
    bpy.ops.render.render(write_still=True)
    bounds_mm = _bounds_mm(mesh_objects)
    triangle_count = sum(
        sum(max(0, len(poly.vertices) - 2) for poly in obj.data.polygons)
        for obj in mesh_objects
    )
    payload = glb_path.read_bytes()
    manifest = {
        "schema_version": "ModuleAssetManifest@1",
        "module_id": module.module_id,
        "pack_id": "pack_weapon_concept_v1",
        "category": module.category,
        "asset_id": f"asset_{module.module_id.removeprefix('module_')}",
        "sha256": hashlib.sha256(payload).hexdigest(),
        "bounds_mm": bounds_mm,
        "triangle_count": triangle_count,
        "material_slots": list(MATERIALS),
        "connectors": [
            {
                "connector_id": connector.connector_id,
                "slot": connector.slot,
                "connector_type": connector.connector_type,
                "transform": {
                    "position": list(connector.position_mm),
                    "rotation": [0, 0, 0],
                    "scale": [1, 1, 1],
                },
                "scale_range": [0.9, 1.1],
                "exclusive": True,
            }
            for connector in module.connectors
        ],
    }
    _write_json(module_root / "module.json", manifest)
    (module_root / "LICENSE.txt").write_text(license_text, encoding="utf-8")
    relative_root = f"modules/{module.module_id}"
    return {
        "module_id": module.module_id,
        "manifest_path": f"{relative_root}/module.json",
        "glb_path": f"{relative_root}/model.glb",
        "thumbnail_path": f"{relative_root}/thumbnail.png",
        "license_path": f"{relative_root}/LICENSE.txt",
        "lod": "LOD0",
    }


def _reset_scene() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)


def _create_materials() -> dict[str, bpy.types.Material]:
    result = {}
    for name, (color, metallic, roughness) in MATERIALS.items():
        material = bpy.data.materials.new(name)
        material.diffuse_color = color
        material.use_nodes = True
        principled = material.node_tree.nodes.get("Principled BSDF")
        principled.inputs["Base Color"].default_value = color
        principled.inputs["Metallic"].default_value = metallic
        principled.inputs["Roughness"].default_value = roughness
        result[name] = material
    return result


def _create_part(object_name, part, materials, collection):
    obj = _create_profile_mesh(object_name, part, collection)
    obj.data.name = object_name.replace("GEO_", "MESH_")
    obj.data.materials.append(materials[part.material])
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    if part.profile == "box":
        obj.location = _business_position_mm_to_blender_m(part.center_mm)
        obj.dimensions = _business_size_mm_to_blender_m(part.size_mm)
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bevel = obj.modifiers.new("Bevel", "BEVEL")
    bevel.width = part.bevel_mm / 1000
    bevel.segments = 3
    bevel.limit_method = "ANGLE"
    bpy.ops.object.modifier_apply(modifier=bevel.name)
    triangulate = obj.modifiers.new("Triangulate", "TRIANGULATE")
    bpy.ops.object.modifier_apply(modifier=triangulate.name)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.02)
    bpy.ops.object.mode_set(mode="OBJECT")
    obj.data.uv_layers.active.name = "UV0"
    obj.select_set(False)
    return obj


def _create_profile_mesh(object_name, part, collection):
    if part.profile == "box":
        bpy.ops.mesh.primitive_cube_add(size=1)
        obj = bpy.context.object
        for owner in tuple(obj.users_collection):
            owner.objects.unlink(obj)
        collection.objects.link(obj)
        obj.name = object_name
        return obj

    sx, sy, sz = part.size_mm
    x0, x1 = -sx / 2, sx / 2
    y0, y1 = -sy / 2, sy / 2
    z0, z1 = -sz / 2, sz / 2
    # The business coordinate system is X (length), Y (height), Z (depth).
    # These intentional visual tapers make the authored source read as a
    # designed hard-surface prop instead of a stack of axis-aligned boxes.
    if part.profile == "wedge":
        top_front, top_rear = y0 + sy * 0.66, y1
        vertices = [
            (x0, y0, z0), (x0, y0, z1), (x1, y0, z1), (x1, y0, z0),
            (x0, top_front, z0), (x0, top_front, z1), (x1, top_rear, z1), (x1, top_rear, z0),
        ]
    elif part.profile == "reverse_wedge":
        top_front, top_rear = y1, y0 + sy * 0.66
        vertices = [
            (x0, y0, z0), (x0, y0, z1), (x1, y0, z1), (x1, y0, z0),
            (x0, top_front, z0), (x0, top_front, z1), (x1, top_rear, z1), (x1, top_rear, z0),
        ]
    elif part.profile == "grip_taper":
        upper_x, upper_z = sx / 2, sz / 2
        lower_x, lower_z = sx * 0.72 / 2, sz * 0.82 / 2
        vertices = [
            (-lower_x, y0, -lower_z), (-lower_x, y0, lower_z),
            (lower_x, y0, lower_z), (lower_x, y0, -lower_z),
            (-upper_x, y1, -upper_z), (-upper_x, y1, upper_z),
            (upper_x, y1, upper_z), (upper_x, y1, -upper_z),
        ]
    elif part.profile == "cylinder_x":
        segments = 24
        vertices = []
        radius_y = sy / 2
        radius_z = sz / 2
        for x in (x0, x1):
            for segment in range(segments):
                angle = math.tau * segment / segments
                vertices.append((x, math.cos(angle) * radius_y, math.sin(angle) * radius_z))
        faces = []
        for segment in range(segments):
            next_segment = (segment + 1) % segments
            faces.append((segment, next_segment, segments + next_segment, segments + segment))
        faces.extend((tuple(range(segments - 1, -1, -1)), tuple(range(segments, segments * 2))))
        mesh = bpy.data.meshes.new(object_name.replace("GEO_", "MESH_"))
        mesh.from_pydata(
            [_business_position_mm_to_blender_m((x + part.center_mm[0], y + part.center_mm[1], z + part.center_mm[2])) for x, y, z in vertices],
            [],
            faces,
        )
        mesh.update()
        obj = bpy.data.objects.new(object_name, mesh)
        collection.objects.link(obj)
        return obj
    else:
        raise ValueError(f"unsupported visual profile: {part.profile}")
    faces = [
        (0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1),
        (1, 5, 6, 2), (2, 6, 7, 3), (3, 7, 4, 0),
    ]
    mesh = bpy.data.meshes.new(object_name.replace("GEO_", "MESH_"))
    mesh.from_pydata(
        [_business_position_mm_to_blender_m((x + part.center_mm[0], y + part.center_mm[1], z + part.center_mm[2])) for x, y, z in vertices],
        [],
        faces,
    )
    mesh.update()
    obj = bpy.data.objects.new(object_name, mesh)
    collection.objects.link(obj)
    return obj


def _surface_detail_parts(part):
    """Author small real meshes for vents, seams and grip ribs.

    These are geometric details exported into the GLB, not a viewport-only
    decoration. Their shallow offset keeps connector locations and the module
    silhouette stable while giving the visual candidate readable panel rhythm.
    """
    sx, sy, sz = part.size_mm
    result = []
    if sx < 22 or sy < 10 or sz < 10:
        return result
    count = max(2, min(5, int(sx // 18)))
    for side in (-1, 1):
        for detail_index in range(count):
            fraction = (detail_index + 1) / (count + 1) - 0.5
            center = (
                part.center_mm[0] + fraction * sx * 0.68,
                part.center_mm[1] + sy * 0.05,
                part.center_mm[2] + side * (sz / 2 + 0.38),
            )
            detail = Part(
                f"{part.name}_surface_rail_{side}_{detail_index}",
                center,
                (max(3.4, sx * 0.07), max(4.0, sy * 0.25), 0.75),
                "MAT_secondary" if detail_index % 3 else "MAT_accent",
                0.34,
            )
            result.append(detail)
    if part.profile == "grip_taper":
        for rib_index in range(5):
            center = (
                part.center_mm[0],
                part.center_mm[1] - sy * 0.12 - rib_index * sy * 0.12,
                part.center_mm[2] + sz / 2 + 0.5,
            )
            rib = Part(
                f"{part.name}_grip_rib_{rib_index}",
                center,
                (sx * 0.7, 1.8, 0.95),
                "MAT_secondary",
                0.28,
            )
            result.append(rib)
    return result


def _create_connector(connector, collection):
    obj = bpy.data.objects.new(f"CON_{connector.connector_id}", None)
    collection.objects.link(obj)
    obj.empty_display_type = "ARROWS"
    obj.empty_display_size = 0.008
    obj.location = _business_position_mm_to_blender_m(connector.position_mm)
    return obj


def _create_render_rig(objects) -> None:
    minimum, maximum = _bounds(objects)
    center = (minimum + maximum) * 0.5
    extent = max(maximum - minimum)
    bpy.ops.object.camera_add(
        location=center + Vector((extent * 1.8, -extent * 2.2, extent * 1.4))
    )
    camera = bpy.context.object
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = extent * 1.65
    camera.rotation_euler = _look_at(camera.location, center)
    bpy.context.scene.camera = camera
    for location, energy, size in (
        (center + Vector((extent, -extent, extent * 2)), 8, extent * 3),
        (center + Vector((-extent, -extent * 0.5, extent)), 4, extent * 2),
    ):
        bpy.ops.object.light_add(type="AREA", location=location)
        light = bpy.context.object
        light.data.energy = energy
        light.data.shape = "DISK"
        light.data.size = size
        light.rotation_euler = _look_at(light.location, center)
    world = bpy.context.scene.world
    if world is None:
        world = bpy.data.worlds.new("ForgeCAD_World")
        bpy.context.scene.world = world
    world.color = (0.012, 0.018, 0.028)


def _look_at(origin, target):
    return (target - origin).to_track_quat("-Z", "Y").to_euler()


def _bounds(objects):
    points = [
        obj.matrix_world @ Vector(corner) for obj in objects for corner in obj.bound_box
    ]
    return (
        Vector(min(point[axis] for point in points) for axis in range(3)),
        Vector(max(point[axis] for point in points) for axis in range(3)),
    )


def _bounds_mm(objects):
    minimum, maximum = _bounds(objects)
    blender_extent = maximum - minimum
    return [
        round(blender_extent[0] * 1000, 4),
        round(blender_extent[2] * 1000, 4),
        round(blender_extent[1] * 1000, 4),
    ]


def _business_position_mm_to_blender_m(values):
    x, y, z = (value / 1000 for value in values)
    return Vector((x, -z, y))


def _business_size_mm_to_blender_m(values):
    x, y, z = (value / 1000 for value in values)
    return Vector((x, z, y))


def _write_json(path, value):
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def _script_args():
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


if __name__ == "__main__":
    main()
