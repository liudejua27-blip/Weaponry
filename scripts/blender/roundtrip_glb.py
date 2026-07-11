from __future__ import annotations

import argparse
import sys
from pathlib import Path

import bpy


def main() -> int:
    args = _arguments_after_separator()
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args(args)

    source = options.input.expanduser().resolve()
    output = options.output.expanduser().resolve()
    if source == output:
        raise RuntimeError("round-trip output must not overwrite the source GLB")
    output.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    bpy.ops.import_scene.gltf(filepath=str(source))
    bpy.ops.export_scene.gltf(
        filepath=str(output),
        export_format="GLB",
        use_selection=False,
        export_apply=True,
        export_yup=True,
    )
    if not output.is_file():
        raise RuntimeError("Blender did not create the round-trip GLB")
    return 0


def _arguments_after_separator() -> list[str]:
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


if __name__ == "__main__":
    raise SystemExit(main())
