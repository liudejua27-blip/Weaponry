"""Render a real GLB into a deterministic visual-QA preview.

This script is intentionally only a renderer: it does not alter the source GLB,
and the PNG is never treated as geometry or a substitute for the workbench view.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import bpy
from mathutils import Vector


def main() -> None:
    args = _args()
    source = args.input.resolve()
    output = args.output.resolve()
    if not source.is_file() or source.suffix.lower() != ".glb":
        raise ValueError("--input must be an existing .glb file")
    if output.suffix.lower() != ".png":
        raise ValueError("--output must end with .png")

    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 1280
    scene.render.resolution_y = 768
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.film_transparent = False
    if scene.world is None:
        scene.world = bpy.data.worlds.new("ForgeCAD_QA_World")
    scene.world.color = (0.008, 0.015, 0.025)
    scene.world.use_nodes = True
    background = scene.world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (0.004, 0.01, 0.022, 1)
    background.inputs["Strength"].default_value = 0.12
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.view_settings.exposure = -1.0
    bpy.ops.import_scene.gltf(filepath=str(source))
    meshes = [item for item in scene.objects if item.type == "MESH"]
    if not meshes:
        raise ValueError("input GLB contains no mesh objects")
    minimum, maximum = _bounds(meshes)
    center = (minimum + maximum) * 0.5
    extent = max(maximum - minimum)

    bpy.ops.mesh.primitive_plane_add(size=extent * 8, location=(center.x, center.y, minimum.z - extent * 0.12))
    floor = bpy.context.object
    floor.data.materials.append(_material("QA_Floor", (0.01, 0.025, 0.045, 1), 0.15, 0.72))
    bpy.ops.object.camera_add(location=center + Vector((extent * 1.35, -extent * 2.1, extent * 0.95)))
    camera = bpy.context.object
    camera.data.lens = 56
    camera.rotation_euler = _look_at(camera.location, center)
    scene.camera = camera
    for location, energy, size, color in (
        (center + Vector((extent * 1.0, -extent * 1.5, extent * 1.7)), 110, extent * 1.8, (0.62, 0.78, 1.0)),
        (center + Vector((-extent * 1.4, -extent * 0.2, extent * 0.8)), 55, extent * 1.2, (0.25, 0.48, 1.0)),
        (center + Vector((extent * 0.25, extent * 1.4, extent * 0.55)), 32, extent, (1.0, 0.22, 0.14)),
    ):
        bpy.ops.object.light_add(type="AREA", location=location)
        light = bpy.context.object
        light.data.energy = energy
        light.data.shape = "DISK"
        light.data.size = size
        light.data.color = color
        light.rotation_euler = _look_at(light.location, center)
    output.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(output)
    bpy.ops.render.render(write_still=True)


def _material(name: str, color: tuple[float, float, float, float], metallic: float, roughness: float):
    material = bpy.data.materials.new(name)
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    principled.inputs["Base Color"].default_value = color
    principled.inputs["Metallic"].default_value = metallic
    principled.inputs["Roughness"].default_value = roughness
    return material


def _bounds(objects):
    points = [item.matrix_world @ Vector(corner) for item in objects for corner in item.bound_box]
    return (
        Vector(min(point[axis] for point in points) for axis in range(3)),
        Vector(max(point[axis] for point in points) for axis in range(3)),
    )


def _look_at(origin: Vector, target: Vector):
    return (target - origin).to_track_quat("-Z", "Y").to_euler()


def _args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else [])


if __name__ == "__main__":
    main()
