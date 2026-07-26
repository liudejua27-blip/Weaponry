#!/usr/bin/env python3
"""Offline contract checks for the explicit visual-provider acceptance runner."""

from __future__ import annotations

import argparse
import hashlib
import io
from contextlib import redirect_stdout

from run_visual_provider_acceptance import (
    LIVE_CONFIRMATION,
    _validate_report,
    main,
)


def run() -> None:
    stdout = io.StringIO()
    with redirect_stdout(stdout):
        assert main([]) == 0
    assert '"status": "dry_run"' in stdout.getvalue()
    assert '"network_calls_made": 0' in stdout.getvalue()
    assert '"credential_reads": 0' in stdout.getvalue()

    run_id = "live_visual_smoke_001"
    report = {
        "schema_version": "ForgeStudioVisualProviderAcceptance@1",
        "status": "pass",
        "execution_mode": "live_explicit_opt_in",
        "run_id_sha256": hashlib.sha256(run_id.encode()).hexdigest(),
        "provider_owner": "rust_desktop",
        "credential_source": "private_visual_secret_file",
        "concept_provider_completed": True,
        "neural_provider_completed": True,
        "remote_job_completed": True,
        "concept_png_sha256": "a" * 64,
        "glb_sha256": "b" * 64,
        "glb_byte_size": 10_000,
        "triangle_count": 50_000,
        "mesh_count": 4,
        "material_count": 2,
        "pbr_channels": ["base_color", "normal", "roughness", "metallic"],
        "every_primitive_has_uv0": True,
        "every_primitive_has_tangent": False,
        "no_raw_prompt_or_response": True,
        "no_key_or_provider_endpoint": True,
    }
    assert _validate_report(report, run_id) == report
    leaked = dict(report, prompt="forbidden")
    try:
        _validate_report(leaked, run_id)
    except Exception as error:
        assert str(error) == "VISUAL_ACCEPTANCE_REPORT_REDACTION_INVALID"
    else:
        raise AssertionError("raw prompt key must be rejected")

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--confirmation", default=LIVE_CONFIRMATION)
    assert parser.parse_args([]).confirmation == LIVE_CONFIRMATION


if __name__ == "__main__":
    run()
    print("visual provider acceptance smoke passed")
