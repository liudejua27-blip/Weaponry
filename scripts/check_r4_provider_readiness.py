#!/usr/bin/env python3
"""Inspect R4 configured-provider readiness without making any provider request."""

from __future__ import annotations

import argparse
import json
import math
import os
from typing import Any
from urllib.parse import urlparse

from forgecad_agent.application.concept_planner import (
    OpenAICompatibleConceptPlanner,
    concept_planner_from_env,
)


def main() -> int:
    args = _parse_args()
    report = provider_readiness()
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ready_for_live_evaluation"] or not args.require_ready else 2


def provider_readiness() -> dict[str, Any]:
    """Return safe local configuration facts only; this function never calls a provider."""
    try:
        provider = concept_planner_from_env()
    except (TypeError, ValueError):
        return _invalid_configuration_report("timeout")
    configured_provider = isinstance(provider, OpenAICompatibleConceptPlanner)
    model_configured = bool(provider.model_name) if configured_provider else False
    credential_configured = (
        bool(provider.config.api_key) if configured_provider else False
    )
    endpoint_valid = (
        _valid_endpoint(provider.config.base_url) if configured_provider else False
    )
    timeout_valid = (
        math.isfinite(provider.config.timeout_seconds)
        and provider.config.timeout_seconds > 0
        if configured_provider
        else False
    )
    ready = (
        configured_provider
        and model_configured
        and credential_configured
        and endpoint_valid
        and timeout_valid
    )
    missing: list[str] = []
    if not configured_provider:
        missing.append("provider")
    if configured_provider and not model_configured:
        missing.append("model")
    if configured_provider and not credential_configured:
        missing.append("credential")
    if configured_provider and not endpoint_valid:
        missing.append("endpoint")
    if configured_provider and not timeout_valid:
        missing.append("timeout")

    return {
        "schema_version": "ForgeCADPlannerProviderReadiness@1",
        "ok": ready,
        "status": "ready_for_live_evaluation" if ready else "not_ready",
        "network_calls_made": 0,
        "ready_for_live_evaluation": ready,
        "provider": {
            "type": provider.provider_type,
            "configured_provider_selected": configured_provider,
            "model_configured": model_configured,
            "credential_configured": credential_configured,
            "endpoint_syntax_valid": endpoint_valid,
            "timeout_valid": timeout_valid,
            "model_source": _first_set(
                "FORGECAD_CONCEPT_PLANNER_MODEL",
                "WUSHEN_LLM_MODEL",
                "WUSHEN_OPENAI_MODEL",
            ),
            "credential_source": _credential_source(credential_configured),
            "endpoint_source": _first_set(
                "FORGECAD_CONCEPT_PLANNER_BASE_URL",
                "WUSHEN_LLM_BASE_URL",
                "WUSHEN_OPENAI_BASE_URL",
            ),
        },
        "missing": missing,
        "next_action": (
            "Operator may explicitly run npm run agent:r4-evaluation-live; it can make 80 paid provider calls."
            if ready
            else "Configure the missing local provider settings, then rerun this no-call preflight."
        ),
        "safety": {
            "provider_request_sent": False,
            "secrets_redacted": True,
            "endpoint_redacted": True,
            "absolute_paths_redacted": True,
        },
    }


def _invalid_configuration_report(error: str) -> dict[str, Any]:
    configured_provider = _selected_provider() == "openai_compatible"
    return {
        "schema_version": "ForgeCADPlannerProviderReadiness@1",
        "ok": False,
        "status": "invalid_configuration",
        "network_calls_made": 0,
        "ready_for_live_evaluation": False,
        "provider": {
            "type": "openai_compatible" if configured_provider else "deterministic",
            "configured_provider_selected": configured_provider,
            "model_configured": False,
            "credential_configured": False,
            "endpoint_syntax_valid": False,
            "timeout_valid": False,
            "model_source": _first_set(
                "FORGECAD_CONCEPT_PLANNER_MODEL",
                "WUSHEN_LLM_MODEL",
                "WUSHEN_OPENAI_MODEL",
            ),
            "credential_source": _credential_source(False),
            "endpoint_source": _first_set(
                "FORGECAD_CONCEPT_PLANNER_BASE_URL",
                "WUSHEN_LLM_BASE_URL",
                "WUSHEN_OPENAI_BASE_URL",
            ),
        },
        "missing": [error],
        "next_action": "Correct the local provider configuration, then rerun this no-call preflight.",
        "safety": {
            "provider_request_sent": False,
            "secrets_redacted": True,
            "endpoint_redacted": True,
            "absolute_paths_redacted": True,
        },
    }


def _valid_endpoint(value: str) -> bool:
    parsed = urlparse(value)
    return parsed.scheme in {"http", "https"} and bool(parsed.netloc)


def _selected_provider() -> str:
    return (
        os.environ.get(
            "FORGECAD_CONCEPT_PLANNER_PROVIDER",
            os.environ.get("WUSHEN_LLM_PROVIDER", "deterministic_rules"),
        )
        .strip()
        .lower()
    )


def _first_set(*names: str) -> str:
    for name in names:
        if os.environ.get(name):
            return name.lower()
    return "default_or_unset"


def _credential_source(configured: bool) -> str:
    names = (
        ("FORGECAD_CONCEPT_PLANNER_API_KEY", "forgecad_env"),
        ("WUSHEN_LLM_API_KEY", "legacy_env"),
        ("FORGECAD_CONCEPT_PLANNER_API_KEY_FILE", "forgecad_secret_file"),
        ("WUSHEN_LLM_API_KEY_FILE", "legacy_secret_file"),
    )
    for name, label in names:
        if os.environ.get(name):
            return label if configured else f"{label}_unreadable_or_empty"
    return "not_configured"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check configured R4 planner settings without sending a provider request."
    )
    parser.add_argument(
        "--require-ready",
        action="store_true",
        help="exit 2 unless a configured provider, model, and credential are locally available",
    )
    return parser.parse_args()


if __name__ == "__main__":
    raise SystemExit(main())
