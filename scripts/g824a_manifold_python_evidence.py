#!/usr/bin/env python3
"""Isolated Manifold Python provenance/cancellation evidence adapter."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any

import manifold3d as manifold
import numpy as np


TAGS = {
    "base": {"source_id": "source_base", "material_id": "material_shell", "zone_id": "zone_shell", "codes": (101.0, 1001.0, 10001.0)},
    "tool": {"source_id": "source_tool", "material_id": "material_cut", "zone_id": "zone_cut", "codes": (202.0, 2002.0, 20002.0)},
}


FIXTURES = {
    "vehicle_window_subtract": ((1800, 800, 600), (520, 500, 700), (250, 0, 0), "subtract"),
    "aircraft_canopy_subtract": ((1600, 650, 500), (700, 420, 520), (120, 80, 0), "subtract"),
    "appliance_vent_subtract": ((900, 700, 1100), (500, 800, 260), (0, 0, 350), "subtract"),
    "robot_arm_housing_union": ((700, 700, 800), (480, 480, 900), (260, 0, 0), "union"),
    "coplanar_subtract": ((1000, 700, 500), (420, 520, 620), (290, 0, 0), "subtract"),
    "near_degenerate_subtract": ((1000, 700, 500), (420, 520, 620), (289.999999, 0, 0), "subtract"),
}


def _tagged_cube(size: tuple[float, float, float], offset: tuple[float, float, float], tag: str) -> manifold.Manifold:
    value = manifold.Manifold.cube(size, True)
    if offset != (0, 0, 0):
        value = value.translate(offset)
    codes = TAGS[tag]["codes"]
    return value.set_properties(3, lambda _position, _old: list(codes))


def _source_original_id(value: manifold.Manifold) -> int:
    mesh = value.to_mesh()
    if len(mesh.run_original_id) != 1:
        raise AssertionError("tagged input must have exactly one original run")
    return int(mesh.run_original_id[0])


def _fixture_payload(fixture: tuple[Any, ...]) -> dict[str, Any]:
    size, tool_size, offset, operation = fixture
    base = _tagged_cube(size, (0, 0, 0), "base")
    tool = _tagged_cube(tool_size, offset, "tool")
    source_by_original = {
        _source_original_id(base): TAGS["base"],
        _source_original_id(tool): TAGS["tool"],
    }
    raw = base + tool if operation == "union" else base - tool
    optimized = raw.simplify(1e-7)
    mesh = optimized.to_mesh()
    if mesh.vert_properties.shape[1] != 6:
        raise AssertionError("candidate lost the three provenance property channels")
    if len(mesh.run_index) != len(mesh.run_original_id) + 1:
        raise AssertionError("candidate run metadata is incomplete")
    triangles: list[dict[str, Any]] = []
    for run_index, original_id in enumerate(mesh.run_original_id):
        original_id = int(original_id)
        source = source_by_original.get(original_id)
        if source is None:
            raise AssertionError(f"unknown source original id: {original_id}")
        first = int(mesh.run_index[run_index]) // 3
        end = int(mesh.run_index[run_index + 1]) // 3
        backside = bool(mesh.backside(run_index))
        for triangle_index in range(first, end):
            vertex_indices = [int(value) for value in mesh.tri_verts[triangle_index]]
            properties = mesh.vert_properties[vertex_indices, 3:6]
            expected = np.asarray(source["codes"], dtype=properties.dtype)
            if not np.allclose(properties, expected, rtol=0, atol=1e-5):
                raise AssertionError("candidate mixed source/material/zone property channels")
            face_id = int(mesh.face_id[triangle_index])
            triangles.append({
                "vertices_mm": [[float(value) for value in mesh.vert_properties[index, :3]] for index in vertex_indices],
                "source_id": source["source_id"],
                "material_id": source["material_id"],
                "zone_id": source["zone_id"],
                "source_face_id": face_id,
                "backside": backside,
                "surface_role": "boolean_cut" if backside else f"source_face_{face_id}",
            })
    if not triangles:
        raise AssertionError("candidate produced no triangles")
    provenance_payload = json.dumps(
        [{key: value for key, value in triangle.items() if key != "vertices_mm"} for triangle in triangles],
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return {
        "operation": operation,
        "triangle_count": len(triangles),
        "provenance_sha256": hashlib.sha256(provenance_payload).hexdigest(),
        "source_ids": sorted({triangle["source_id"] for triangle in triangles}),
        "material_ids": sorted({triangle["material_id"] for triangle in triangles}),
        "zone_ids": sorted({triangle["zone_id"] for triangle in triangles}),
        "has_backside_cut_surface": any(triangle["backside"] for triangle in triangles),
        "optimized": True,
        "triangles": triangles,
    }


def _busy_probe(marker: Path, output: Path) -> int:
    marker.parent.mkdir(parents=True, exist_ok=True)
    marker.write_text(json.dumps({"pid": os.getpid(), "state": "kernel_loop_started"}), encoding="utf-8")
    counter = 0
    while True:
        base = manifold.Manifold.sphere(120, 96)
        tool = manifold.Manifold.cube((180, 180, 180), True).translate((counter % 17, 0, 0))
        result = base - tool
        _ = result.num_tri()
        counter += 1
    output.write_text("unreachable", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--busy-probe", action="store_true")
    parser.add_argument("--marker", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.busy_probe:
        if args.marker is None or args.output is None:
            parser.error("--busy-probe requires --marker and --output")
        return _busy_probe(args.marker, args.output)
    first = {fixture_id: _fixture_payload(fixture) for fixture_id, fixture in FIXTURES.items()}
    second = {fixture_id: _fixture_payload(fixture) for fixture_id, fixture in FIXTURES.items()}
    deterministic = all(first[key]["provenance_sha256"] == second[key]["provenance_sha256"] for key in first)
    print(json.dumps({
        "adapter": "manifold_python",
        "fixtures": first,
        "deterministic_provenance": deterministic,
        "property_channels_verified": True,
        "simplify_provenance_verified": True,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
