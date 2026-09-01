#!/usr/bin/env python3
"""Propose one immutable blade-only successor using deterministic contour metrics."""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import evaluate_metrics as metrics
import search_candidates as search


def score(receipt: dict[str, object]) -> float:
    values = receipt["metrics"]
    assert isinstance(values, dict)
    return (
        4.0 * float(values["silhouette_iou"])
        + 2.0 * float(values["boundary_f1"])
        - 2.0 * float(values["symmetric_chamfer"])
        - float(values["p95_contour_distance"])
        - float(values["landmark_error"])
    )


def build_receipt(program: dict[str, object], reference: dict[str, object], seed: int, candidate_count: int) -> dict[str, object]:
    ledger = search.build_smoke_ledger(program)
    ledger["candidate_budget"] = candidate_count
    ledger["canonical_sha256"] = ""
    ledger["canonical_sha256"] = search.canonical_sha256(ledger)
    generated = search.generate_candidates(program, ledger, seed, candidate_count)
    baseline_metrics = metrics.evaluate_program(program, reference, grid_size=256)
    evaluated = []
    for candidate in generated["candidates"]:
        candidate_metrics = metrics.evaluate_program(candidate["program"], reference, grid_size=256)
        evaluated.append(
            {
                "candidate_id": candidate["candidate_id"],
                "program_sha256": candidate["program_sha256"],
                "parameter_delta": candidate["parameter_delta"],
                "geometry_hard_gate_pass": candidate["hard_gate_pass"],
                "metric_receipt_sha256": candidate_metrics["canonical_sha256"],
                "metrics": {
                    key: candidate_metrics["metrics"][key]
                    for key in (
                        "silhouette_iou",
                        "boundary_f1",
                        "symmetric_chamfer",
                        "p95_contour_distance",
                        "landmark_error",
                    )
                },
                "score": round(score(candidate_metrics), 12),
                "program": candidate["program"],
            }
        )
    eligible = [candidate for candidate in evaluated if candidate["geometry_hard_gate_pass"]]
    selected = max(eligible, key=lambda candidate: (candidate["score"], candidate["candidate_id"])) if eligible else None
    baseline_score = round(score(baseline_metrics), 12)
    improved = selected is not None and selected["score"] > baseline_score + 1e-9

    parent_hash = search.validate_ledger(ledger)
    successor = None
    if improved and selected is not None:
        successor = copy.deepcopy(ledger)
        successor["revision"] += 1
        successor["parent_ledger_sha256"] = parent_hash
        successor["program_sha256"] = selected["program_sha256"]
        successor["evidence_sha256"] = list(dict.fromkeys(successor["evidence_sha256"] + [selected["metric_receipt_sha256"]]))
        successor["hypothesis"] = "A bounded blade-only curve and section delta improves the authorized front contour metrics while assembly stays frozen."
        successor["canonical_sha256"] = ""
        successor["canonical_sha256"] = search.canonical_sha256(successor)
        search.validate_ledger(successor)

    receipt: dict[str, object] = {
        "schema_version": "KnifeReferenceRefinementReceipt@1",
        "route": "weaponry-threejs-knife-studio@0.1.0",
        "seed": seed,
        "candidate_count": candidate_count,
        "parent_program_sha256": search.validate_program(program),
        "parent_ledger_sha256": parent_hash,
        "reference_sha256": metrics.validate_reference(reference),
        "allowed_scope": ["blade-body", "cutting-edge"],
        "frozen_parts": ["guard", "grip", "pommel"],
        "baseline": {
            "metric_receipt_sha256": baseline_metrics["canonical_sha256"],
            "score": baseline_score,
            "metrics": {key: baseline_metrics["metrics"][key] for key in ("silhouette_iou", "boundary_f1", "symmetric_chamfer", "p95_contour_distance", "landmark_error")},
        },
        "candidates": evaluated,
        "selected_candidate_id": selected["candidate_id"] if improved and selected else None,
        "selected_program": selected["program"] if improved and selected else None,
        "successor_ledger": successor,
        "decision": "SUCCESSOR_PROPOSED_NOT_APPROVED" if improved else "PARENT_RETAINED_NO_IMPROVEMENT",
        "render_status": "NOT_RUN",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "canonical_sha256": "",
    }
    receipt["canonical_sha256"] = search.canonical_sha256(receipt)
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=20260831)
    parser.add_argument("--candidate-count", type=int, default=32, choices=range(1, 33))
    args = parser.parse_args()
    program = search.load_json(args.program)
    reference = metrics.load_json(args.reference)
    receipt = build_receipt(program, reference, args.seed, args.candidate_count)
    args.output.write_text(json.dumps(receipt, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: receipt[key] for key in ("schema_version", "decision", "selected_candidate_id", "canonical_sha256")}, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
