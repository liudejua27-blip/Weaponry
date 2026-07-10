#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from typing import Any, Optional

from evaluate_r4_planner_truth_set import (
    DEFAULT_TRUTH_SET,
    EvaluationSetupError,
    _load_truth_set,
    _provider,
    evaluate,
)
from forgecad_agent.application.concept_planner import (
    ConceptChangePlan,
    ConceptVariantPlan,
    DeterministicConceptPlanner,
    PlannerCallMetrics,
)
from forgecad_agent.domain.concepts.models import WeaponConceptSpec


class _SyntheticTelemetryPlanner:
    provider_id = "synthetic_telemetry_planner"
    provider_type = "openai_compatible"
    model_name: Optional[str] = "synthetic-eval-model"

    def __init__(self) -> None:
        self.delegate = DeterministicConceptPlanner()
        self.last_call_metrics: Optional[PlannerCallMetrics] = None

    def interpret_brief(self, **kwargs: Any) -> WeaponConceptSpec:
        result = self.delegate.interpret_brief(**kwargs)
        self._record()
        return result

    def plan_variants(self, **kwargs: Any) -> list[ConceptVariantPlan]:
        result = self.delegate.plan_variants(**kwargs)
        self._record()
        return result

    def plan_change_set(self, **kwargs: Any) -> ConceptChangePlan:
        try:
            return self.delegate.plan_change_set(**kwargs)
        finally:
            self._record()

    def _record(self) -> None:
        self.last_call_metrics = PlannerCallMetrics(
            latency_ms=12,
            input_tokens=10,
            output_tokens=5,
            total_tokens=15,
        )


def main() -> int:
    truth_set = _load_truth_set(DEFAULT_TRUTH_SET)
    report = evaluate(
        truth_set,
        provider=_SyntheticTelemetryPlanner(),  # type: ignore[arg-type]
        generator="configured_provider",
        live_provider_run=True,
        max_cases=None,
        truth_set_path=DEFAULT_TRUTH_SET,
    )
    _assert(report["real_provider_evidence_eligible"] is True, "eligible branch failed")
    telemetry = report["telemetry"]
    _assert(telemetry["provider_call_records"] == 80, "provider call count mismatch")
    _assert(telemetry["calls_with_provider_latency"] == 80, "latency coverage mismatch")
    _assert(telemetry["calls_with_token_usage"] == 80, "token coverage mismatch")
    _assert(telemetry["input_tokens"] == 800, "input token total mismatch")
    _assert(telemetry["output_tokens"] == 400, "output token total mismatch")
    _assert(telemetry["total_tokens"] == 1200, "total token count mismatch")

    confirmation_required = False
    try:
        _provider(
            argparse.Namespace(
                provider="configured_provider",
                confirm_live_provider=False,
            )
        )
    except EvaluationSetupError as exc:
        confirmation_required = exc.code == "EVAL_LIVE_CONFIRMATION_REQUIRED"
    _assert(confirmation_required, "live evaluation did not require explicit confirmation")
    print(
        json.dumps(
            {
                "ok": True,
                "synthetic_only": True,
                "truth_set_cases": 80,
                "eligible_branch_verified": True,
                "live_confirmation_guard_verified": True,
                "telemetry_calls": telemetry["provider_call_records"],
                "total_tokens": telemetry["total_tokens"],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
