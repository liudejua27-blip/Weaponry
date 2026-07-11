"""Read edited ForgeCAD .blend sources and export a validated three-module pack."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector


AUTHORING_SCHEMA = "ForgeCADBlenderAuthoring@1"
MATERIAL_NAMES = {"MAT_primary", "MAT_secondary", "MAT_accent"}
BLENDER_TO_GLTF = Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0)))
MODULE_SETS = {
    "starter": (
        "module_core_shell_01",
        "module_front_shell_01",
        "module_front_shell_02",
    ),
    "full_candidate": (
        "module_core_shell_01",
        "module_front_shell_01",
        "module_front_shell_02",
        "module_rear_shell_01",
        "module_grip_shell_01",
        "module_top_accessory_01",
        "module_side_accessory_01",
        "module_lower_structure_01",
        "module_storage_visual_01",
        "module_armor_panel_01",
    ),
}
REQUIRED_MODULE_IDS = MODULE_SETS["starter"]
LICENSE_TEXT = (
    "SPDX-License-Identifier: LicenseRef-ForgeCAD-Authoring-Starter\n"
    "Editable non-functional concept/game/film-prop asset exported by ForgeCAD.\n"
    "Not final art until human review; not manufacturing documentation.\n"
)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument(
        "--module-set", choices=("starter", "full_candidate"), default="starter"
    )
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args(_script_args())
    source_root = args.source_root.expanduser().resolve()
    output_root = args.output_root.expanduser().resolve()
    _prepare_output(output_root, args.force)
    required_module_ids = MODULE_SETS[args.module_set]
    _require_sources(source_root, required_module_ids)

    (output_root / "LICENSES").mkdir(parents=True, exist_ok=True)
    (output_root / "LICENSES" / "PACK.txt").write_text(LICENSE_TEXT, encoding="utf-8")
    entries = []
    for module_id in required_module_ids:
        source_path = source_root / f"{module_id}.blend"
        bpy.ops.wm.open_mainfile(filepath=str(source_path), load_ui=False)
        entries.append(_export_open_source(output_root, source_path, module_id))
    _write_json(output_root / "pack.json", _pack_manifest(entries))


def _pack_manifest(entries):
    return {
        "schema_version": "ModulePackManifest@1",
        "pack_id": "pack_weapon_concept_v1",
        "profile_id": "profile_weapon_concept_v1",
        "name": "Weapon Concept v1 edited Blender export",
        "version": "0.1.0",
        "description": (
            "Re-exported edited Blender sources for non-functional concept/game/film-prop "
            "review; requires human approval before promotion."
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


def _prepare_output(output_root: Path, force: bool) -> None:
    repository_root = Path(__file__).resolve().parents[2]
    committed_pack_root = repository_root / "assets" / "module-packs"
    if output_root.is_relative_to(committed_pack_root):
        raise RuntimeError("edited export cannot target committed assets/module-packs")
    if output_root.exists() and any(output_root.iterdir()):
        if not force:
            raise RuntimeError(
                "edited export output is not empty; use --force deliberately"
            )
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)


def _require_sources(source_root: Path, required_module_ids: tuple[str, ...]) -> None:
    expected = {f"{module_id}.blend" for module_id in required_module_ids}
    actual = (
        {path.name for path in source_root.glob("*.blend")}
        if source_root.is_dir()
        else set()
    )
    if actual != expected:
        raise RuntimeError(
            f"source root must contain exactly {sorted(expected)}, found {sorted(actual)}"
        )


def _export_open_source(output_root: Path, source_path: Path, expected_module_id: str):
    scene = bpy.context.scene
    metadata = _metadata(scene)
    module_id = metadata["module_id"]
    if module_id != expected_module_id or source_path.stem != module_id:
        raise RuntimeError(
            f"source/module identity mismatch: file={source_path.stem}, metadata={module_id}"
        )
    mesh_pattern = re.compile(rf"^GEO_{re.escape(module_id)}_LOD0(?:_[0-9]{{2}})?$")
    data_pattern = re.compile(rf"^MESH_{re.escape(module_id)}_LOD0(?:_[0-9]{{2}})?$")
    mesh_objects = sorted(
        (
            obj
            for obj in scene.objects
            if obj.type == "MESH" and mesh_pattern.fullmatch(obj.name)
        ),
        key=lambda obj: obj.name,
    )
    if not mesh_objects:
        raise RuntimeError(f"{module_id}: no canonical LOD0 mesh objects")
    all_mesh_names = {obj.name for obj in scene.objects if obj.type == "MESH"}
    if all_mesh_names != {obj.name for obj in mesh_objects}:
        raise RuntimeError(
            f"{module_id}: non-canonical mesh objects: {sorted(all_mesh_names)}"
        )

    used_materials = set()
    for obj in mesh_objects:
        if not data_pattern.fullmatch(obj.data.name):
            raise RuntimeError(f"{module_id}: invalid mesh data name: {obj.data.name}")
        if obj.modifiers:
            raise RuntimeError(
                f"{module_id}: apply modifiers before export: {obj.name}"
            )
        if not _identity_transform(obj):
            raise RuntimeError(
                f"{module_id}: apply location/rotation/scale: {obj.name}"
            )
        if "UV0" not in obj.data.uv_layers:
            raise RuntimeError(f"{module_id}: UV0 is missing: {obj.name}")
        used_materials.update(
            slot.material.name
            for slot in obj.material_slots
            if slot.material is not None
        )
    if used_materials != MATERIAL_NAMES:
        raise RuntimeError(
            f"{module_id}: materials must be {sorted(MATERIAL_NAMES)}, found {sorted(used_materials)}"
        )

    connectors = _connectors(scene, metadata)
    module_root = output_root / "modules" / module_id
    module_root.mkdir(parents=True, exist_ok=True)
    _select_only(mesh_objects)
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
    if scene.camera is None:
        raise RuntimeError(f"{module_id}: thumbnail camera is missing")
    scene.render.resolution_x = 512
    scene.render.resolution_y = 512
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = str(module_root / "thumbnail.png")
    bpy.ops.render.render(write_still=True)

    payload = glb_path.read_bytes()
    manifest = {
        "schema_version": "ModuleAssetManifest@1",
        "module_id": module_id,
        "pack_id": metadata["pack_id"],
        "category": metadata["category"],
        "asset_id": metadata["asset_id"],
        "sha256": hashlib.sha256(payload).hexdigest(),
        "bounds_mm": _bounds_mm(mesh_objects),
        "triangle_count": sum(
            sum(max(0, len(poly.vertices) - 2) for poly in obj.data.polygons)
            for obj in mesh_objects
        ),
        "material_slots": sorted(MATERIAL_NAMES),
        "connectors": connectors,
    }
    _write_json(module_root / "module.json", manifest)
    (module_root / "LICENSE.txt").write_text(LICENSE_TEXT, encoding="utf-8")
    relative_root = f"modules/{module_id}"
    return {
        "module_id": module_id,
        "manifest_path": f"{relative_root}/module.json",
        "glb_path": f"{relative_root}/model.glb",
        "thumbnail_path": f"{relative_root}/thumbnail.png",
        "license_path": f"{relative_root}/LICENSE.txt",
        "lod": "LOD0",
    }


def _metadata(scene):
    raw = scene.get("forgecad_authoring_metadata")
    if not isinstance(raw, str):
        raise RuntimeError("scene forgecad_authoring_metadata is missing")
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise RuntimeError("scene forgecad_authoring_metadata is invalid JSON") from exc
    required = {
        "schema_version",
        "module_id",
        "category",
        "pack_id",
        "asset_id",
        "connectors",
    }
    if not isinstance(value, dict) or set(value) != required:
        raise RuntimeError("scene authoring metadata fields do not match the contract")
    if value["schema_version"] != AUTHORING_SCHEMA or not isinstance(
        value["connectors"], list
    ):
        raise RuntimeError("scene authoring metadata schema is unsupported")
    return value


def _connectors(scene, metadata):
    definitions = {item["connector_id"]: item for item in metadata["connectors"]}
    actual = {
        obj.name.removeprefix("CON_"): obj
        for obj in scene.objects
        if obj.type == "EMPTY" and obj.name.startswith("CON_connector_")
    }
    if set(actual) != set(definitions):
        raise RuntimeError(
            f"{metadata['module_id']}: connector empties differ from metadata: "
            f"expected={sorted(definitions)}, actual={sorted(actual)}"
        )
    result = []
    for connector_id in sorted(definitions):
        definition = definitions[connector_id]
        obj = actual[connector_id]
        if any(abs(value - 1) > 1e-6 for value in obj.scale):
            raise RuntimeError(
                f"{metadata['module_id']}: apply Connector scale: {obj.name}"
            )
        result.append(
            {
                "connector_id": connector_id,
                "slot": definition["slot"],
                "connector_type": definition["connector_type"],
                "transform": {
                    "position": _blender_position_m_to_business_mm(obj.location),
                    "rotation": _blender_rotation_to_business_euler(obj.rotation_euler),
                    "scale": [1, 1, 1],
                },
                "scale_range": definition["scale_range"],
                "exclusive": definition["exclusive"],
            }
        )
    return result


def _identity_transform(obj) -> bool:
    return (
        all(abs(value) <= 1e-7 for value in obj.location)
        and all(abs(value) <= 1e-7 for value in obj.rotation_euler)
        and all(abs(value - 1) <= 1e-7 for value in obj.scale)
    )


def _select_only(objects) -> None:
    for obj in bpy.context.selected_objects:
        obj.select_set(False)
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]


def _bounds_mm(objects):
    points = [
        obj.matrix_world @ Vector(corner) for obj in objects for corner in obj.bound_box
    ]
    minimum = [min(point[axis] for point in points) for axis in range(3)]
    maximum = [max(point[axis] for point in points) for axis in range(3)]
    blender_extent = [maximum[axis] - minimum[axis] for axis in range(3)]
    return [
        round(blender_extent[0] * 1000, 4),
        round(blender_extent[2] * 1000, 4),
        round(blender_extent[1] * 1000, 4),
    ]


def _blender_position_m_to_business_mm(value):
    converted = BLENDER_TO_GLTF @ value
    # Blender stores object transforms as float32. Four decimal places in millimeters
    # removes representation noise (for example 50.000001) without hiding author edits.
    return [round(component * 1000, 4) for component in converted]


def _blender_rotation_to_business_euler(value):
    blender_matrix = value.to_matrix()
    converted = BLENDER_TO_GLTF @ blender_matrix @ BLENDER_TO_GLTF.transposed()
    return [round(float(component), 9) for component in converted.to_euler("XYZ")]


def _write_json(path, value) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def _script_args():
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, TypeError) as exc:
        print(
            json.dumps(
                {"ok": False, "status": "edited_export_failed", "message": str(exc)}
            )
        )
        raise SystemExit(1) from exc
