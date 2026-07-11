#!/usr/bin/env python3
"""Prove R4 provider readiness preflight stays local and redacts configuration values."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check_r4_provider_readiness.py"
CONFIG_NAMES = (
    "FORGECAD_CONCEPT_PLANNER_PROVIDER",
    "FORGECAD_CONCEPT_PLANNER_BASE_URL",
    "FORGECAD_CONCEPT_PLANNER_MODEL",
    "FORGECAD_CONCEPT_PLANNER_API_KEY",
    "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
    "FORGECAD_CONCEPT_PLANNER_TIMEOUT_SECONDS",
    "WUSHEN_LLM_PROVIDER",
    "WUSHEN_LLM_BASE_URL",
    "WUSHEN_LLM_MODEL",
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
    "WUSHEN_OPENAI_BASE_URL",
    "WUSHEN_OPENAI_MODEL",
)


def main() -> int:
    clean_env = os.environ.copy()
    for name in CONFIG_NAMES:
        clean_env.pop(name, None)

    unconfigured = _run(clean_env)
    _assert(unconfigured.returncode == 0, "diagnostic preflight should be inspectable")
    report = _json(unconfigured)
    _assert(report["network_calls_made"] == 0, "preflight reported a network call")
    _assert(
        report["ready_for_live_evaluation"] is False, "clean env unexpectedly ready"
    )
    _assert(
        report["provider"]["type"] == "deterministic", "clean env provider mismatch"
    )

    ready_env = clean_env | {
        "FORGECAD_CONCEPT_PLANNER_PROVIDER": "openai_compatible",
        "FORGECAD_CONCEPT_PLANNER_BASE_URL": "https://redacted.invalid/v1",
        "FORGECAD_CONCEPT_PLANNER_MODEL": "preflight-smoke-model",
        "FORGECAD_CONCEPT_PLANNER_API_KEY": "preflight-smoke-secret",
    }
    ready = _run(ready_env, "--require-ready")
    _assert(ready.returncode == 0, "configured preflight did not pass")
    ready_report = _json(ready)
    _assert(ready_report["ready_for_live_evaluation"] is True, "ready state mismatch")
    _assert(ready_report["network_calls_made"] == 0, "ready preflight sent a request")
    _assert("preflight-smoke-secret" not in ready.stdout, "secret leaked")
    _assert("redacted.invalid" not in ready.stdout, "endpoint leaked")
    _assert(str(ROOT) not in ready.stdout, "absolute path leaked")

    invalid_endpoint_env = ready_env | {
        "FORGECAD_CONCEPT_PLANNER_BASE_URL": "file:///not/reported",
    }
    invalid_endpoint = _run(invalid_endpoint_env, "--require-ready")
    _assert(invalid_endpoint.returncode == 2, "invalid endpoint passed require-ready")
    invalid_endpoint_report = _json(invalid_endpoint)
    _assert(
        "endpoint" in invalid_endpoint_report["missing"],
        "invalid endpoint not reported",
    )
    _assert("file:///not/reported" not in invalid_endpoint.stdout, "endpoint leaked")

    invalid_timeout_env = ready_env | {
        "FORGECAD_CONCEPT_PLANNER_TIMEOUT_SECONDS": "not-a-number",
    }
    invalid_timeout = _run(invalid_timeout_env, "--require-ready")
    _assert(invalid_timeout.returncode == 2, "invalid timeout passed require-ready")
    invalid_timeout_report = _json(invalid_timeout)
    _assert(
        invalid_timeout_report["status"] == "invalid_configuration",
        "invalid timeout did not return safe configuration diagnostic",
    )
    _assert(
        "timeout" in invalid_timeout_report["missing"], "invalid timeout not reported"
    )

    legacy_credential_env = ready_env | {
        "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE": "/not/reported/and/not/read",
        "WUSHEN_LLM_API_KEY": "legacy-preflight-smoke-secret",
    }
    legacy_credential_env.pop("FORGECAD_CONCEPT_PLANNER_API_KEY")
    legacy = _run(legacy_credential_env, "--require-ready")
    _assert(legacy.returncode == 0, "legacy credential fallback did not pass")
    _assert(
        _json(legacy)["provider"]["credential_source"] == "legacy_env",
        "credential source does not match provider fallback order",
    )

    missing_credential_env = ready_env.copy()
    missing_credential_env.pop("FORGECAD_CONCEPT_PLANNER_API_KEY")
    missing = _run(missing_credential_env, "--require-ready")
    _assert(missing.returncode == 2, "require-ready did not reject missing credential")
    _assert(
        "credential" in _json(missing)["missing"], "missing credential not reported"
    )
    print(
        json.dumps(
            {"ok": True, "network_calls_made": 0, "redaction_verified": True},
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _run(env: dict[str, str], *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=ROOT,
        env=env | {"PYTHONPATH": "apps/agent"},
        text=True,
        capture_output=True,
        check=False,
    )


def _json(result: subprocess.CompletedProcess[str]) -> dict[str, object]:
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise AssertionError(result.stderr or result.stdout) from exc


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
