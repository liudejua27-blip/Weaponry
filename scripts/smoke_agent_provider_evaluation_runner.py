#!/usr/bin/env python3
"""No-network regression coverage for FGC-E002's authorised evaluator."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from check_agent_provider_evaluation_contract import DEFAULT_CONTRACT, DEFAULT_TRUTH_SET, expanded_normal_cases, load_json, validate
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner, MechanicalPlannerError, MechanicalPlannerTelemetry
from forgecad_agent.application.provider_evaluation import ProviderEvaluationAuthorization, ProviderEvaluationError, run_authorized_evaluation
from run_agent_provider_evaluation import _macos_keychain_planner_config, claim_evaluation_run_id


class SyntheticPlanner:
    provider_id = "openai_compatible_mechanical_planner"
    model_name = None

    def __init__(self, *, telemetry: bool = True, failure: str | None = None, input_tokens: int = 20, output_tokens: int = 30, total_tokens: int = 50) -> None:
        self.calls = 0
        self.telemetry = telemetry
        self.failure = failure
        self.input_tokens = input_tokens
        self.output_tokens = output_tokens
        self.total_tokens = total_tokens
        self.delegate = DeterministicMechanicalPlanner()
        self.last_call_telemetry = None

    def plan_complete_concept(self, *, brief, pack, project_id):
        self.calls += 1
        if self.failure:
            raise MechanicalPlannerError(self.failure, "synthetic failure")
        plan = self.delegate.plan_complete_concept(brief=brief, pack=pack, project_id=project_id)
        self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=3, input_tokens=self.input_tokens, output_tokens=self.output_tokens, total_tokens=self.total_tokens) if self.telemetry else None
        return plan.model_copy(update={"provider_id": self.provider_id})


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _authorization(*, budget: float = 10.0) -> ProviderEvaluationAuthorization:
    return ProviderEvaluationAuthorization(
        confirm_live_provider=True,
        confirmed_budget_cny=budget,
        evaluation_run_id="eval_synthetic_20260714",
        operator_name="synthetic test operator",
        approval_timestamp="2026-07-14T00:00:00Z",
        provider_connection_preflight=True,
    )


def _run(planner: SyntheticPlanner, *, authorization: ProviderEvaluationAuthorization | None = None, cancelled=None):
    contract = load_json(DEFAULT_CONTRACT)
    truth_set = load_json(DEFAULT_TRUTH_SET)
    dry = validate(contract, truth_set)
    return run_authorized_evaluation(
        contract=contract,
        normal_cases=expanded_normal_cases(truth_set),
        safety_cases=truth_set["clarification_or_rejection_cases"],
        fixture_sha256=dry["fixture_sha256"],
        planner=planner,
        authorization=authorization or _authorization(),
        execution_mode="synthetic",
        cancelled=cancelled,
    )


def _reject(planner: SyntheticPlanner, authorization: ProviderEvaluationAuthorization, expected: str) -> None:
    try:
        _run(planner, authorization=authorization)
    except ProviderEvaluationError as exc:
        _assert(expected == exc.code, f"expected {expected}, got {exc.code}")
        _assert(planner.calls == 0, "authorization failure must occur before a Provider call")
        return
    raise AssertionError(f"expected {expected}")


def main() -> int:
    runner_source = (Path(__file__).resolve().parents[1] / "scripts" / "run_agent_provider_evaluation.py").read_text(encoding="utf-8")
    for forbidden in ("find-generic-password", "KEYCHAIN_ACCOUNT", "KEYCHAIN_SERVICE"):
        _assert(forbidden not in runner_source, f"Python evaluation runner must not contain a Keychain secret-read path: {forbidden}")

    with tempfile.TemporaryDirectory() as temporary:
        metadata_path = Path(temporary) / "provider.json"
        metadata_path.write_text(json.dumps({"base_url": "https://api.deepseek.com", "model": "deepseek-v4-pro", "configured": True}), encoding="utf-8")
        try:
            _macos_keychain_planner_config(
                platform_name="darwin",
                metadata_path=metadata_path,
            )
        except ProviderEvaluationError as exc:
            _assert(
                exc.code == "E002_RUST_NATIVE_PROVIDER_REQUIRED",
                "the retired Python Keychain bridge must reject before credential access",
            )
        else:
            raise AssertionError("the retired Python Keychain bridge must reject")

    full = SyntheticPlanner()
    report = _run(full)
    _assert(full.calls == 80, "only 80 normal cases may call a Provider")
    _assert(report["network_calls_made"] == 0 and report["synthetic_provider_calls"] == 80, "synthetic execution must never claim network calls")
    _assert(report["quality_gates_pass"] is True and report["external_evidence_eligible"] is False, "synthetic success is not real Provider evidence")
    _assert(report["summary"]["safe_stop_pass_rate"] == 1.0, "all unsafe and ambiguous cases must stop locally")
    serialized = json.dumps(report, ensure_ascii=False)
    for forbidden in ("base_url", "api_key", "model_identifier", "brief_stems", "加工尺寸"):
        _assert(forbidden not in serialized, f"redacted report leaked {forbidden}")

    _reject(SyntheticPlanner(), _authorization(budget=0), "E002_BUDGET_INVALID")
    _reject(SyntheticPlanner(), _authorization(budget=101), "E002_BUDGET_EXCEEDED")
    _reject(SyntheticPlanner(), ProviderEvaluationAuthorization(False, 10, "eval_synthetic_20260714", "operator", "2026-07-14T00:00:00Z", True), "E002_CONFIRMATION_REQUIRED")

    timed_out = SyntheticPlanner(failure="PLANNER_TIMEOUT")
    timeout_report = _run(timed_out)
    _assert(timed_out.calls == 1 and timeout_report["stopped_category"] == "timeout", "timeout must stop before another request")

    cancelled = SyntheticPlanner()
    cancelled_report = _run(cancelled, cancelled=lambda: True)
    _assert(cancelled.calls == 0 and cancelled_report["stopped_category"] == "cancelled", "cancel must stop before first Provider request")

    no_usage = SyntheticPlanner(telemetry=False)
    no_usage_report = _run(no_usage)
    _assert(no_usage.calls == 80 and no_usage_report["quality_gates_pass"] is False, "missing usage must make evidence incomplete")
    _assert(no_usage_report["summary"]["token_usage_coverage"] == 0.0, "missing usage coverage must be reported")

    output_budget = SyntheticPlanner(output_tokens=1201, total_tokens=1221)
    output_budget_report = _run(output_budget)
    _assert(output_budget.calls == 1 and output_budget_report["stopped_category"] == "budget_exceeded", "per-request output limit must stop before another request")

    empty_environment = {"PATH": os.environ.get("PATH", ""), "PYTHONPATH": "apps/agent:scripts"}
    absent_credentials = subprocess.run(
        [sys.executable, "scripts/run_agent_provider_evaluation.py", "--confirm-live-provider", "--confirmed-budget-cny", "10", "--evaluation-run-id", "eval_no_credentials", "--operator-name", "test", "--approval-timestamp", "2026-07-14T00:00:00Z", "--provider-connection-preflight"],
        cwd=Path(__file__).resolve().parents[1], env=empty_environment, capture_output=True, text=True, check=False,
    )
    _assert(absent_credentials.returncode == 2 and "E002_PROVIDER_UNCONFIGURED" in absent_credentials.stdout, "absent credentials must reject before network access")

    retired_keychain = subprocess.run(
        [sys.executable, "scripts/run_agent_provider_evaluation.py", "--confirm-live-provider", "--confirmed-budget-cny", "10", "--evaluation-run-id", "eval_no_keychain", "--operator-name", "test", "--approval-timestamp", "2026-07-14T00:00:00Z", "--provider-connection-preflight", "--provider-config-source", "macos-keychain"],
        cwd=Path(__file__).resolve().parents[1], env=empty_environment, capture_output=True, text=True, check=False,
    )
    _assert(retired_keychain.returncode == 2 and "E002_RUST_NATIVE_PROVIDER_REQUIRED" in retired_keychain.stdout, "the retired Python Keychain path must reject before network access")

    with tempfile.TemporaryDirectory() as temporary:
        ledger_dir = Path(temporary) / "ledger"
        claim_evaluation_run_id(evaluation_run_id="eval_unique_20260714", fixture_sha256="a" * 64, ledger_dir=ledger_dir)
        try:
            claim_evaluation_run_id(evaluation_run_id="eval_unique_20260714", fixture_sha256="a" * 64, ledger_dir=ledger_dir)
        except ProviderEvaluationError as exc:
            _assert(exc.code == "E002_RUN_ID_ALREADY_USED", "duplicate run ID must be rejected before a Provider call")
        else:
            raise AssertionError("duplicate run ID must fail")

    print("FGC-E002 Provider evaluation runner smoke passed (synthetic only, no network calls)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
