"""Frozen entrypoint for the Rust-owned ForgeCAD geometry sidecar.

Production exposes only the loopback RestrictedGeometryExecutor.  SQLite
migrations and the historical product core are not selectable by this binary.
"""

from __future__ import annotations

import argparse
import multiprocessing
import os
import sys
from pathlib import Path


def main() -> int:
    # Required for the bounded G825 CSG child process in a PyInstaller sidecar.
    multiprocessing.freeze_support()
    parser = argparse.ArgumentParser(prog="wushen-agent")
    agent = parser.add_subparsers(dest="command", required=True)
    serve = agent.add_parser("agent").add_subparsers(dest="agent_command", required=True)
    run = serve.add_parser("serve")
    run.add_argument("--host", default="127.0.0.1")
    run.add_argument("--port", type=int, default=8000)
    args = parser.parse_args()

    if args.command != "agent" or args.agent_command != "serve":
        parser.error("only 'agent serve' is supported")
    if args.host not in {"127.0.0.1", "::1", "localhost"}:
        parser.error("--host must be loopback")
    if not (0 < args.port < 65536):
        parser.error("--port must be between 1 and 65535")

    # Never trust a caller-supplied resource/library location.  Packaged
    # geometry can see only this code-derived audited bundle root.
    os.environ["FORGECAD_RUNTIME_RESOURCE_ROOT"] = str(_resource_root())
    # K003 production startup never hands the Python facet a library or
    # migration root.  Geometry may resolve only the audited read-only bundle
    # above; Rust owns SQLite, WAL, migrations and the object store.  Legacy
    # test switches are stripped too: the formal binary never parses them.
    for name in (
        "FORGECAD_K001_PACKAGED_PROBE",
        "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE",
        "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_MIGRATIONS_DIR",
        "DATABASE_URL",
        "FORGECAD_DATABASE_PATH",
        "FORGECAD_SQLITE_PATH",
        "FORGECAD_OBJECT_STORE_ROOT",
        "FORGECAD_LIBRARY_ROOT",
        "FORGECAD_AGENT_PROVIDER",
        "FORGECAD_AGENT_BASE_URL",
        "FORGECAD_AGENT_MODEL",
        "FORGECAD_AGENT_API_KEY",
        "FORGECAD_AGENT_API_KEY_FILE",
        "FORGECAD_CONCEPT_PLANNER_PROVIDER",
        "FORGECAD_CONCEPT_PLANNER_BASE_URL",
        "FORGECAD_CONCEPT_PLANNER_MODEL",
        "FORGECAD_CONCEPT_PLANNER_API_KEY",
        "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
        "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN",
        "DEEPSEEK_API_KEY",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "ANTHROPIC_API_KEY",
        "DASHSCOPE_API_KEY",
        "WUSHEN_3D_HTTP_API_KEY",
        "WUSHEN_LLM_PROVIDER",
        "WUSHEN_LLM_BASE_URL",
        "WUSHEN_LLM_MODEL",
        "WUSHEN_LLM_API_KEY",
        "WUSHEN_LLM_API_KEY_FILE",
        "WUSHEN_OPENAI_BASE_URL",
        "WUSHEN_OPENAI_MODEL",
        "WUSHEN_OPENAI_API_KEY",
        "WUSHEN_OPENAI_API_KEY_FILE",
        "FORGECAD_ACTIVE_DESIGN_SNAPSHOT_WRITE_TOKEN",
    ):
        os.environ.pop(name, None)
    from wushen_agent.main import app
    import uvicorn

    uvicorn.run(app, host=args.host, port=args.port, log_level="warning")
    return 0


def _resource_root() -> Path:
    bundle_root = getattr(sys, "_MEIPASS", None)
    if bundle_root:
        return Path(bundle_root)
    return Path(__file__).resolve().parents[3]


if __name__ == "__main__":
    raise SystemExit(main())
