#!/usr/bin/env python3
"""Fail closed if Geometry and Render Worker source ownership drifts."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
geometry_source = (ROOT / "apps/geometry-worker/src/lib.rs").read_text(encoding="utf-8")
render_manifest = (ROOT / "apps/render-worker/Cargo.toml").read_text(encoding="utf-8")
render_source = (ROOT / "apps/render-worker/src/main.rs").read_text(encoding="utf-8")
render_core = (ROOT / "apps/render-core/src/lib.rs").read_text(encoding="utf-8")
runtime_source = (ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs").read_text(encoding="utf-8")
runtime_render_adapter = (ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/render_worker.rs").read_text(encoding="utf-8")
runtime_geometry_adapter = (ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/geometry_worker.rs").read_text(encoding="utf-8")

assert "forgecad-geometry-worker" not in render_manifest
assert "forgecad_geometry_worker" not in render_source
assert "forgecad-render-core" in render_manifest
for symbol in (
    "pub fn render_fixed_glb",
    "pub fn render_perspective_glb",
    "pub fn render_perspective_glb_fit_at_resolution",
):
    assert symbol not in geometry_source, symbol
    assert symbol in render_core, symbol
assert "fn render_worker_result" in render_source
assert '"glb_base64"' in render_source
assert '"geometry_program"' not in render_source.split("fn render_worker_result", 1)[1].split("fn require_closed_payload", 1)[0]
assert '"render_glb" =>' not in geometry_source
assert '"render_glb_fit_batch" =>' not in geometry_source
assert "mod render_worker;" in runtime_source
for symbol in (
    "geometry_worker::render_fixed",
    "geometry_worker::render_glb",
    "geometry_worker::render_glb_fit_batch",
    "geometry_worker::render_glb_fit_batch_at_resolution",
):
    assert symbol not in runtime_source, symbol
assert "render_worker::render_glb_fit_batch_at_resolution" in runtime_source
assert 'const RENDER_WORKER_BINARY: &str = "forgecad-render-worker";' in runtime_render_adapter
assert "fn execute_render_worker" in runtime_render_adapter
assert "pub(crate) fn execute_sibling_worker" in runtime_geometry_adapter
assert "RENDER_WORKER_BINARY" not in runtime_geometry_adapter
assert "execute_render_worker" not in runtime_geometry_adapter
assert "geometry_worker::compile_geometry(" not in runtime_render_adapter
print("Render Worker source ownership boundary PASS")
