#!/usr/bin/env python3
"""Run the explicitly authorised ForgeCAD general-mechanical Provider evaluation.

Without the three live flags this command is a no-network contract dry-run.
It never writes an Agent project, Snapshot, asset, quality report, export, or
ordinary Agent thread/turn.  Real invocation is intentionally a manual external
operation; CI must use the E001 no-call commands instead.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Sequence

from check_agent_provider_evaluation_contract import (
    DEFAULT_CONTRACT,
    DEFAULT_TRUTH_SET,
    ContractError,
    expanded_normal_cases,
    load_json,
    validate,
)
from forgecad_agent.application.mechanical_planner import (
    MechanicalPlannerConfig,
    OpenAICompatibleMechanicalPlanner,
    mechanical_planner_from_env,
)
from forgecad_agent.application.provider_evaluation import (
    ProviderEvaluationAuthorization,
    ProviderEvaluationError,
    run_authorized_evaluation,
)


KEYCHAIN_SERVICE = "ForgeCAD Agent Provider"
KEYCHAIN_ACCOUNT = "default"


def _macos_keychain_planner_config(
    *,
    platform_name: str | None = None,
    metadata_path: Path | None = None,
    command_runner=subprocess.run,
) -> MechanicalPlannerConfig:
    """Read the same local Alpha configuration as the Tauri shell.

    This is deliberately an opt-in path used only by the explicitly authorised
    evaluator.  The Keychain secret remains in memory and never enters the
    environment, the run ledger, or the redacted evaluation report.
    """

    if (platform_name or sys.platform) != "darwin":
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "macOS Keychain Provider configuration is unavailable on this target.")
    path = metadata_path or (Path.home() / "Library" / "Application Support" / "ForgeCAD" / "provider.json")
    try:
        metadata = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "macOS Provider configuration is unavailable.") from exc
    base_url = metadata.get("base_url") if isinstance(metadata, dict) else None
    model = metadata.get("model") if isinstance(metadata, dict) else None
    configured = metadata.get("configured") if isinstance(metadata, dict) else False
    if not configured or not isinstance(base_url, str) or not base_url.startswith(("https://", "http://")) or not isinstance(model, str) or not (0 < len(model.strip()) <= 160):
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "macOS Provider configuration is incomplete.")
    try:
        result = command_runner(
            ["/usr/bin/security", "find-generic-password", "-a", KEYCHAIN_ACCOUNT, "-s", KEYCHAIN_SERVICE, "-w"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "macOS Provider secret is unavailable.") from exc
    api_key = (result.stdout or "").strip() if result.returncode == 0 else ""
    if not api_key or len(api_key) > 4096:
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "macOS Provider secret is unavailable.")
    return MechanicalPlannerConfig(base_url=base_url.rstrip("/"), model=model.strip(), api_key=api_key)


def _configured_evaluation_planner(contract: dict, *, provider_config_source: str = "environment") -> OpenAICompatibleMechanicalPlanner:
    if provider_config_source == "macos-keychain":
        config = _macos_keychain_planner_config()
    elif provider_config_source == "environment":
        planner = mechanical_planner_from_env()
        if not isinstance(planner, OpenAICompatibleMechanicalPlanner) or not planner.config.api_key:
            raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "A configured OpenAI-compatible local Provider is required.")
        config = planner.config
    else:
        raise ProviderEvaluationError("E002_PROVIDER_UNCONFIGURED", "Unknown Provider configuration source.")
    budget = contract["budget"]
    return OpenAICompatibleMechanicalPlanner(
        MechanicalPlannerConfig(
            base_url=config.base_url,
            model=config.model,
            api_key=config.api_key,
            timeout_seconds=min(config.timeout_seconds, float(budget["max_request_timeout_seconds"])),
            response_mode=config.response_mode,
            max_output_tokens=min(config.max_output_tokens, budget["max_output_tokens_per_request"]),
        )
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run the explicitly authorised ForgeCAD Provider evaluation.")
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--truth-set", type=Path, default=DEFAULT_TRUTH_SET)
    parser.add_argument("--confirm-live-provider", action="store_true")
    parser.add_argument("--confirmed-budget-cny", type=float)
    parser.add_argument("--evaluation-run-id")
    parser.add_argument("--operator-name")
    parser.add_argument("--approval-timestamp")
    parser.add_argument("--provider-connection-preflight", action="store_true")
    parser.add_argument(
        "--provider-config-source",
        choices=("environment", "macos-keychain"),
        default="environment",
        help="explicit Provider configuration source; macos-keychain reads the existing ForgeCAD desktop configuration without exporting its secret",
    )
    return parser


def claim_evaluation_run_id(*, evaluation_run_id: str, fixture_sha256: str, ledger_dir: Path | None = None) -> None:
    """Atomically reserve one live run identifier without storing its plaintext.

    This ledger is separate from projects and reports.  A reservation survives a
    crash deliberately: a possibly-billed run must never be automatically
    repeated with the same identifier.
    """

    configured_dir = os.environ.get("FORGECAD_EVALUATION_LEDGER_DIR")
    if ledger_dir is not None:
        directory = ledger_dir
    elif configured_dir:
        directory = Path(configured_dir)
        if not directory.is_absolute():
            raise ProviderEvaluationError("E002_RUN_LEDGER_UNAVAILABLE", "evaluation ledger directory must be absolute.")
    else:
        directory = Path.home() / ".forgecad" / "provider-evaluation-runs"
    try:
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        os.chmod(directory, 0o700)
        marker = directory / f"{hashlib.sha256(evaluation_run_id.encode('utf-8')).hexdigest()}.json"
        descriptor = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump({"fixture_sha256": fixture_sha256}, handle, ensure_ascii=False, sort_keys=True)
    except FileExistsError as exc:
        raise ProviderEvaluationError("E002_RUN_ID_ALREADY_USED", "evaluation_run_id was already reserved; use a new run identifier.") from exc
    except OSError as exc:
        raise ProviderEvaluationError("E002_RUN_LEDGER_UNAVAILABLE", "Unable to reserve evaluation_run_id before a Provider call.") from exc


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        contract = load_json(args.contract)
        truth_set = load_json(args.truth_set)
        dry_report = validate(contract, truth_set)
        live_values = (args.confirm_live_provider, args.confirmed_budget_cny, args.evaluation_run_id)
        if not any(live_values):
            print(json.dumps(dry_report, ensure_ascii=False, indent=2))
            return 0
        if not args.confirm_live_provider or args.confirmed_budget_cny is None or not args.evaluation_run_id or not args.operator_name or not args.approval_timestamp or not args.provider_connection_preflight:
            raise ProviderEvaluationError("E002_AUTHORIZATION_REQUIRED", "All live flags and the human authorization record are required; no Provider call was made.")
        authorization = ProviderEvaluationAuthorization(
            confirm_live_provider=args.confirm_live_provider,
            confirmed_budget_cny=args.confirmed_budget_cny,
            evaluation_run_id=args.evaluation_run_id,
            operator_name=args.operator_name,
            approval_timestamp=args.approval_timestamp,
            provider_connection_preflight=args.provider_connection_preflight,
        )
        planner = _configured_evaluation_planner(contract, provider_config_source=args.provider_config_source)
        claim_evaluation_run_id(
            evaluation_run_id=authorization.evaluation_run_id,
            fixture_sha256=dry_report["fixture_sha256"],
        )
        report = run_authorized_evaluation(
            contract=contract,
            normal_cases=expanded_normal_cases(truth_set),
            safety_cases=truth_set["clarification_or_rejection_cases"],
            fixture_sha256=dry_report["fixture_sha256"],
            planner=planner,
            authorization=authorization,
            execution_mode="external",
        )
    except (ContractError, ProviderEvaluationError) as exc:
        print(json.dumps({"ok": False, "error": getattr(exc, "code", str(exc)), "network_calls_made": 0}, ensure_ascii=False, indent=2))
        return 2
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
