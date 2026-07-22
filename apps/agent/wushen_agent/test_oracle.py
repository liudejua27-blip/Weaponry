"""Explicit source-tree factory for historical product-core tests.

This module is not referenced by the frozen sidecar or the desktop launcher.
Legacy compatibility tests opt in by naming this factory directly; production
``wushen_agent.main:create_app`` always creates the restricted geometry facet.
"""

from __future__ import annotations

from fastapi import FastAPI

from .main import create_test_only_legacy_product_core_app


def create_app() -> FastAPI:
    """Create the historical full app for a test process only."""

    return create_test_only_legacy_product_core_app()
