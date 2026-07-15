#!/usr/bin/env python3
"""Freeze DomainInferenceResult@1 and its lexical fixture before service work."""

from __future__ import annotations

import json
import warnings
from pathlib import Path
from typing import Any, Dict

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, ValidationError  # noqa: E402
from pydantic import ValidationError as PydanticValidationError  # noqa: E402

from forgecad_agent.application.domain_inference import DomainInferenceResult  # noqa: E402
from forgecad_agent.application.domain_packs import list_domain_packs  # noqa: E402


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "packages" / "concept-spec" / "schemas" / "domain-inference-result.schema.json"
FIXTURE_PATH = ROOT / "packages" / "concept-spec" / "fixtures" / "domain-inference-keywords.json"


def _load(path: Path) -> Dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _expect_invalid(validator: Draft202012Validator, value: Dict[str, Any]) -> None:
    try:
        validator.validate(value)
    except ValidationError:
        return
    raise AssertionError(f"schema accepted invalid inference result: {value}")


def _expect_pydantic_invalid(value: Dict[str, Any]) -> None:
    try:
        DomainInferenceResult.model_validate(value)
    except PydanticValidationError:
        return
    raise AssertionError(f"Pydantic accepted invalid inference result: {value}")


def main() -> int:
    schema = _load(SCHEMA_PATH)
    fixture = _load(FIXTURE_PATH)
    validator = Draft202012Validator(schema)
    expected_pack_ids = {pack.pack_id for pack in list_domain_packs()}

    assert fixture["schema_version"] == "DomainInferenceKeywordFixture@1"
    assert {entry["domain_pack_id"] for entry in fixture["packs"]} == expected_pack_ids
    assert len(fixture["packs"]) == len(expected_pack_ids)
    for entry in fixture["packs"]:
        terms = [*entry["keywords"], *entry["synonyms"]]
        assert terms and len(terms) == len(set(terms)), f"terms must be unique for {entry['domain_pack_id']}"
        assert all(isinstance(term, str) and term.strip() for term in terms)

    recognized = {
        "schema_version": "DomainInferenceResult@1",
        "status": "recognized",
        "domain_pack_id": "pack_vehicle_concept",
        "candidate_domain_pack_ids": ["pack_vehicle_concept"],
        "matched_terms": ["汽车", "rover"],
    }
    ambiguous = {
        "schema_version": "DomainInferenceResult@1",
        "status": "ambiguous",
        "domain_pack_id": None,
        "candidate_domain_pack_ids": ["pack_vehicle_concept", "pack_aircraft_concept"],
        "matched_terms": ["无人机", "载具"],
    }
    unsupported = {
        "schema_version": "DomainInferenceResult@1",
        "status": "unsupported",
        "domain_pack_id": None,
        "candidate_domain_pack_ids": [],
        "matched_terms": [],
    }
    for value in (recognized, ambiguous, unsupported):
        validator.validate(value)
        DomainInferenceResult.model_validate(value)

    mismatched_recognized = {**recognized, "candidate_domain_pack_ids": ["pack_aircraft_concept"]}
    single_candidate_ambiguous = {**ambiguous, "candidate_domain_pack_ids": ["pack_vehicle_concept"]}
    leaking_unsupported = {**unsupported, "matched_terms": ["模型"]}
    for value in (mismatched_recognized, single_candidate_ambiguous, leaking_unsupported):
        _expect_invalid(validator, value)
        _expect_pydantic_invalid(value)

    print("D001 domain inference contract smoke passed: strict result states and four-pack lexical fixture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
