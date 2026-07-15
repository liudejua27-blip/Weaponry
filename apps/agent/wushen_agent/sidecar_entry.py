"""Frozen entrypoint for the local ForgeCAD Agent sidecar.

The build process embeds SQLite migrations beside this entrypoint.  It accepts
only the supervisor's bounded ``agent serve`` command and never configures a
Provider itself; credentials remain an explicit desktop runtime concern.
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
    if not (0 < args.port < 65536):
        parser.error("--port must be between 1 and 65535")

    os.environ.setdefault("WUSHEN_MIGRATIONS_DIR", str(_migrations_dir()))
    os.environ.setdefault("FORGECAD_RUNTIME_RESOURCE_ROOT", str(_resource_root()))
    from wushen_agent.main import app
    import uvicorn

    uvicorn.run(app, host=args.host, port=args.port, log_level="warning")
    return 0


def _migrations_dir() -> Path:
    bundle_root = getattr(sys, "_MEIPASS", None)
    if bundle_root:
        return Path(bundle_root) / "migrations"
    return Path(__file__).resolve().parents[3] / "migrations"


def _resource_root() -> Path:
    bundle_root = getattr(sys, "_MEIPASS", None)
    if bundle_root:
        return Path(bundle_root)
    return Path(__file__).resolve().parents[3]


if __name__ == "__main__":
    raise SystemExit(main())
