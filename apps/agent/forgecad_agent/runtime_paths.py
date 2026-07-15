"""Resolve read-only ForgeCAD runtime resources in source and frozen builds."""

from __future__ import annotations

import os
import sys
from pathlib import Path


def runtime_resource_root() -> Path:
    """Return the bundled resource root without exposing it through an API."""
    bundled = os.environ.get("FORGECAD_RUNTIME_RESOURCE_ROOT")
    if bundled:
        return Path(bundled).expanduser().resolve()
    frozen_root = getattr(sys, "_MEIPASS", None)
    if frozen_root:
        return Path(frozen_root).resolve()
    return Path(__file__).resolve().parents[3]
