#!/usr/bin/env python3
"""Fail closed when Forge Studio regains a non-approved AI provider route."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

REMOVED_RUNTIME_FILES = (
    "apps/desktop/src-tauri/src/neural_3d_provider_adapter.rs",
    "apps/desktop/src-tauri/src/visual_provider_adapters.rs",
    "apps/desktop/src-tauri/src/visual_provider_acceptance_probe.rs",
    "apps/desktop/src/shared/tauri/meshSeedGeneration.ts",
    "scripts/run_visual_provider_acceptance.py",
    "scripts/smoke_visual_provider_acceptance.py",
)

SCANNED_RUNTIME_FILES = (
    "apps/desktop/src-tauri/src/main.rs",
    "apps/desktop/src-tauri/src/provider_credentials.rs",
    "apps/desktop/src-tauri/src/vision_evidence_adapter.rs",
    "apps/desktop/src-tauri/crates/forgecad-app-server/src/lib.rs",
    "apps/desktop/src/features/cad-workbench/VisionEvidencePanel.tsx",
    "apps/desktop/src/features/cad-workbench/ReferenceEvidenceDrawer.tsx",
    "apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx",
    "apps/desktop/src/features/cad-workbench/agentBlockoutDisplayState.ts",
    "apps/agent/wushen_agent/main.py",
    "package.json",
)

FORBIDDEN_RUNTIME_TOKENS = (
    "queue." + "fal.run",
    "/" + "fal-ai/",
    "Fal" + "Credential",
    "Fal" + "Hunyuan",
    "fal" + "_api_key",
    "fal" + "_flux",
    "generate_" + "visual_asset",
    "save_" + "visual_provider_config",
    "neural_" + "visual_candidate_pbr",
    "mesh" + "SeedGeneration",
)


def main() -> int:
    failures: list[str] = []

    for relative in REMOVED_RUNTIME_FILES:
        if (ROOT / relative).exists():
            failures.append(f"removed provider runtime file exists: {relative}")

    for relative in SCANNED_RUNTIME_FILES:
        path = ROOT / relative
        if not path.is_file():
            failures.append(f"provider policy input is missing: {relative}")
            continue
        source = path.read_text(encoding="utf-8")
        lowered = source.lower()
        for token in FORBIDDEN_RUNTIME_TOKENS:
            if token.lower() in lowered:
                failures.append(f"forbidden provider route token {token!r} found in {relative}")

    text_policy = (ROOT / "apps/desktop/src-tauri/src/provider_credentials.rs").read_text(
        encoding="utf-8"
    )
    vision_policy = (ROOT / "apps/desktop/src-tauri/src/vision_evidence_adapter.rs").read_text(
        encoding="utf-8"
    )
    if 'ALLOWED_PROVIDER_HOST: &str = "api.deepseek.com"' not in text_policy:
        failures.append("DeepSeek official endpoint allowlist is missing")
    if 'ALLOWED_PROVIDER_MODEL_PREFIX: &str = "deepseek-"' not in text_policy:
        failures.append("DeepSeek model-family allowlist is missing")
    if 'ALLOWED_VISION_HOST_SUFFIX: &str = ".aliyuncs.com"' not in vision_policy:
        failures.append("Qwen official endpoint allowlist is missing")
    if 'ALLOWED_VISION_MODEL_PREFIX: &str = "qwen"' not in vision_policy:
        failures.append("Qwen model-family allowlist is missing")

    desktop_main = (ROOT / "apps/desktop/src-tauri/src/main.rs").read_text(encoding="utf-8")
    for token in ("direct_visual_brief", "VisualBriefDirector", "ConceptImageBackend"):
        if token in desktop_main:
            failures.append(f"legacy concept-image route remains registered in desktop runtime: {token}")
    app_server_lib = (ROOT / "apps/desktop/src-tauri/crates/forgecad-app-server/src/lib.rs").read_text(
        encoding="utf-8"
    )
    for module_name in ("concept_image_provider", "visual_brief_director"):
        if f"#[cfg(test)]\nmod {module_name};" not in app_server_lib:
            failures.append(f"legacy concept-image module is not test-only: {module_name}")

    python_entrypoint = (ROOT / "apps/agent/wushen_agent/main.py").read_text(
        encoding="utf-8"
    )
    if '"provider_access": False' not in python_entrypoint:
        failures.append("restricted geometry sidecar no-provider assertion is missing")
    if "return create_restricted_geometry_app(environment=restricted_environment)" not in python_entrypoint:
        failures.append("production Python entrypoint is not sealed to restricted geometry")

    if failures:
        for failure in failures:
            print(f"AI_PROVIDER_POLICY_FAIL {failure}")
        return 1

    print(
        "AI_PROVIDER_POLICY_PASS text_authoring=deepseek vision_understanding=qwen "
        "remote_mesh_generation=absent"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
