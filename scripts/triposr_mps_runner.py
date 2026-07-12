#!/usr/bin/env python3
"""Run upstream TripoSR with a macOS MPS-aware device selection.

The upstream ``run.py`` intentionally falls back to CPU whenever CUDA is
unavailable. That makes a stock TripoSR checkout unnecessarily slow on Apple
silicon even when PyTorch MPS is available. This project-owned adapter imports
the upstream package without modifying it, keeps its preprocessing/export
contract, and records the actual selected device beside each generated GLB.

This is an image-to-3D *candidate* generator. Its output must still pass the
existing ModuleAsset/quality gates before it can be assembled or promoted.
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from pathlib import Path
from typing import Iterable


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", nargs="+", help="Input concept image(s).")
    parser.add_argument(
        "--triposr-repo",
        required=True,
        help="Absolute path to an unmodified upstream TripoSR checkout.",
    )
    parser.add_argument(
        "--device",
        default="auto",
        choices=("auto", "mps", "cpu", "cuda:0"),
        help="auto prefers MPS on Apple silicon, then CUDA, then CPU.",
    )
    parser.add_argument(
        "--pretrained-model-name-or-path",
        default="stabilityai/TripoSR",
        help="Hugging Face model ID or an already-downloaded local model path.",
    )
    parser.add_argument("--chunk-size", type=int, default=4096)
    parser.add_argument("--mc-resolution", type=int, default=128)
    parser.add_argument("--no-remove-bg", action="store_true")
    parser.add_argument("--foreground-ratio", type=float, default=0.85)
    parser.add_argument("--output-dir", default="output")
    parser.add_argument("--model-save-format", choices=("obj", "glb"), default="glb")
    return parser.parse_args()


def select_device(requested: str, torch: object) -> str:
    cuda_available = bool(torch.cuda.is_available())
    mps_available = bool(torch.backends.mps.is_available())
    if requested == "auto":
        return "mps" if mps_available else "cuda:0" if cuda_available else "cpu"
    if requested == "mps":
        if not mps_available:
            raise RuntimeError("MPS was requested but PyTorch MPS is unavailable.")
        return "mps"
    if requested == "cuda:0":
        if not cuda_available:
            raise RuntimeError("CUDA was requested but no CUDA device is available.")
        return "cuda:0"
    return "cpu"


def import_upstream(repo: Path) -> tuple[object, object, object, object, object, object]:
    if not (repo / "tsr" / "system.py").is_file():
        raise RuntimeError(f"TripoSR checkout is missing tsr/system.py: {repo}")
    sys.path.insert(0, str(repo))
    import numpy as np  # type: ignore[import-not-found]
    import rembg  # type: ignore[import-not-found]
    import torch  # type: ignore[import-not-found]
    from PIL import Image  # type: ignore[import-not-found]
    from tsr.system import TSR  # type: ignore[import-not-found]
    from tsr.utils import remove_background, resize_foreground, scale_tensor  # type: ignore[import-not-found]

    return np, rembg, torch, Image, TSR, (remove_background, resize_foreground, scale_tensor)


def prepare_images(
    paths: Iterable[str],
    *,
    output_dir: Path,
    remove_background: bool,
    foreground_ratio: float,
    np: object,
    rembg: object,
    Image: object,
    remove_background_fn: object,
    resize_foreground_fn: object,
) -> list[object]:
    session = None if not remove_background else rembg.new_session()
    images: list[object] = []
    for index, image_path in enumerate(paths):
        if not remove_background:
            images.append(Image.open(image_path).convert("RGB"))
            continue
        image = remove_background_fn(Image.open(image_path), session)
        image = resize_foreground_fn(image, foreground_ratio)
        rgb = np.array(image).astype(np.float32) / 255.0
        rgb = rgb[:, :, :3] * rgb[:, :, 3:4] + (1 - rgb[:, :, 3:4]) * 0.5
        prepared = Image.fromarray((rgb * 255.0).astype(np.uint8))
        image_output = output_dir / str(index)
        image_output.mkdir(parents=True, exist_ok=True)
        prepared.save(image_output / "input.png")
        images.append(prepared)
    return images


def extract_mesh_with_cpu_marching_cubes(
    model: object,
    scene_codes: object,
    *,
    resolution: int,
    torch: object,
    scale_tensor_fn: object,
) -> list[object]:
    """Keep neural inference on MPS while running torchmcubes on its CPU ABI.

    The official TripoSR ``MarchingCubeHelper`` accepts CUDA/CPU tensors only.
    The neural query and vertex-colour lookup still execute on the selected
    MPS device; only the marching-cubes volume crosses to CPU.
    """
    import trimesh  # type: ignore[import-not-found]

    model.set_marching_cubes_resolution(resolution)
    helper = model.isosurface_helper
    meshes: list[object] = []
    for scene_code in scene_codes:
        with torch.no_grad():
            density = model.renderer.query_triplane(
                model.decoder,
                scale_tensor_fn(
                    helper.grid_vertices.to(scene_codes.device),
                    helper.points_range,
                    (-model.renderer.cfg.radius, model.renderer.cfg.radius),
                ),
                scene_code,
            )["density_act"]
        vertices, faces = helper(-(density.detach().cpu() - 25.0))
        vertices = scale_tensor_fn(
            vertices,
            helper.points_range,
            (-model.renderer.cfg.radius, model.renderer.cfg.radius),
        )
        with torch.no_grad():
            colors = model.renderer.query_triplane(
                model.decoder,
                vertices.to(scene_codes.device),
                scene_code,
            )["color"]
        meshes.append(
            trimesh.Trimesh(
                vertices=vertices.cpu().numpy(),
                faces=faces.cpu().numpy(),
                vertex_colors=colors.cpu().numpy(),
            ),
        )
    return meshes


def main() -> int:
    args = parse_args()
    repo = Path(args.triposr_repo).expanduser().resolve()
    output_dir = Path(args.output_dir).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    np, rembg, torch, Image, TSR, utilities = import_upstream(repo)
    remove_background_fn, resize_foreground_fn, scale_tensor_fn = utilities
    device = select_device(args.device, torch)

    logging.basicConfig(format="%(asctime)s - %(levelname)s - %(message)s", level=logging.INFO)
    started_at = time.monotonic()
    logging.info("Loading upstream TripoSR on %s", device)
    model = TSR.from_pretrained(
        args.pretrained_model_name_or_path,
        config_name="config.yaml",
        weight_name="model.ckpt",
    )
    model.renderer.set_chunk_size(args.chunk_size)
    model.to(device)

    images = prepare_images(
        args.image,
        output_dir=output_dir,
        remove_background=not args.no_remove_bg,
        foreground_ratio=args.foreground_ratio,
        np=np,
        rembg=rembg,
        Image=Image,
        remove_background_fn=remove_background_fn,
        resize_foreground_fn=resize_foreground_fn,
    )
    output_paths: list[str] = []
    for index, image in enumerate(images):
        logging.info("Generating TripoSR candidate %s/%s", index + 1, len(images))
        with torch.no_grad():
            scene_codes = model([image], device=device)
        meshes = extract_mesh_with_cpu_marching_cubes(
            model,
            scene_codes,
            resolution=args.mc_resolution,
            torch=torch,
            scale_tensor_fn=scale_tensor_fn,
        )
        image_output = output_dir / str(index)
        image_output.mkdir(parents=True, exist_ok=True)
        mesh_path = image_output / f"mesh.{args.model_save_format}"
        meshes[0].export(mesh_path)
        output_paths.append(str(mesh_path))

    report = {
        "schema_version": "ForgeCADTripoSRRun@1",
        "upstream_repo": str(repo),
        "requested_device": args.device,
        "selected_device": device,
        "mc_resolution": args.mc_resolution,
        "chunk_size": args.chunk_size,
        "background_removed": not args.no_remove_bg,
        "outputs": output_paths,
        "elapsed_seconds": round(time.monotonic() - started_at, 3),
    }
    (output_dir / "triposr-run.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    logging.info("Wrote %s generated mesh(es) in %.2fs", len(output_paths), report["elapsed_seconds"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
