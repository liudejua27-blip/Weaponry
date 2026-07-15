"""Contracts for safe, pre-plan domain classification.

This module intentionally contains no inference algorithm. D001 freezes the result
shape before D002 replaces the legacy weapon-defaulting behavior.
"""

from __future__ import annotations

import json
from functools import lru_cache
from typing import Dict, List, Literal, Optional, Tuple

from pydantic import Field, model_validator

from .concept_models import StrictApiModel
from .domain_packs import DomainPackId
from forgecad_agent.runtime_paths import runtime_resource_root


DomainInferenceStatus = Literal["recognized", "ambiguous", "unsupported"]


class DomainInferenceFixtureError(RuntimeError):
    """Raised only when the bundled lexical fixture has been corrupted."""


class DomainInferenceResult(StrictApiModel):
    """Classification result that must be resolved before planning can begin.

    It is deliberately not a project, plan, asset, or persisted event. Only a
    ``recognized`` result can identify one Domain Pack. ``ambiguous`` and
    ``unsupported`` results are write barriers for the later service/UI tasks.
    """

    schema_version: Literal["DomainInferenceResult@1"] = "DomainInferenceResult@1"
    status: DomainInferenceStatus
    domain_pack_id: Optional[DomainPackId] = None
    candidate_domain_pack_ids: List[DomainPackId] = Field(default_factory=list, max_length=4)
    matched_terms: List[str] = Field(default_factory=list, max_length=12)

    @model_validator(mode="after")
    def validate_status_shape(self) -> "DomainInferenceResult":
        candidates = self.candidate_domain_pack_ids
        if len(candidates) != len(set(candidates)):
            raise ValueError("candidate_domain_pack_ids must be unique")
        if len(self.matched_terms) != len(set(self.matched_terms)):
            raise ValueError("matched_terms must be unique")
        if any(not term.strip() or len(term) > 80 for term in self.matched_terms):
            raise ValueError("matched_terms must contain non-empty terms up to 80 characters")

        if self.status == "recognized":
            if self.domain_pack_id is None or candidates != [self.domain_pack_id] or not self.matched_terms:
                raise ValueError("recognized requires one matching candidate, domain_pack_id, and matched_terms")
        elif self.status == "ambiguous":
            if self.domain_pack_id is not None or not 2 <= len(candidates) <= 4 or not self.matched_terms:
                raise ValueError("ambiguous requires two to four candidates, no domain_pack_id, and matched_terms")
        elif self.domain_pack_id is not None or candidates or self.matched_terms:
            raise ValueError("unsupported must not carry a pack candidate or matched term")
        return self


class DomainInferenceService:
    """Deterministic, non-persisting classifier for the first four domain packs."""

    def __init__(self, terms_by_pack: Dict[DomainPackId, Tuple[str, ...]]) -> None:
        self.terms_by_pack = terms_by_pack

    def infer(self, message: str) -> DomainInferenceResult:
        normalized = message.casefold()
        candidates: List[DomainPackId] = []
        matched_terms: List[str] = []
        for pack_id, terms in self.terms_by_pack.items():
            matched = [term for term in terms if term.casefold() in normalized]
            if matched:
                candidates.append(pack_id)
                matched_terms.extend(matched)
        if len(candidates) == 1:
            return DomainInferenceResult(
                status="recognized",
                domain_pack_id=candidates[0],
                candidate_domain_pack_ids=candidates,
                matched_terms=matched_terms,
            )
        if len(candidates) > 1:
            return DomainInferenceResult(
                status="ambiguous",
                domain_pack_id=None,
                candidate_domain_pack_ids=candidates,
                matched_terms=matched_terms,
            )
        return DomainInferenceResult(
            status="unsupported",
            domain_pack_id=None,
            candidate_domain_pack_ids=[],
            matched_terms=[],
        )


def infer_domain(message: str) -> DomainInferenceResult:
    """Return a tri-state classification without calling a Provider or writing data."""

    return _default_service().infer(message)


@lru_cache(maxsize=1)
def _default_service() -> DomainInferenceService:
    fixture_path = runtime_resource_root() / "packages" / "concept-spec" / "fixtures" / "domain-inference-keywords.json"
    try:
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise DomainInferenceFixtureError("Domain inference keyword fixture cannot be loaded") from exc
    if fixture.get("schema_version") != "DomainInferenceKeywordFixture@1" or not isinstance(fixture.get("packs"), list):
        raise DomainInferenceFixtureError("Domain inference keyword fixture has an unsupported shape")

    expected: set[DomainPackId] = {
        "pack_future_weapon_prop",
        "pack_vehicle_concept",
        "pack_aircraft_concept",
        "pack_robotic_arm_concept",
    }
    terms_by_pack: Dict[DomainPackId, Tuple[str, ...]] = {}
    for entry in fixture["packs"]:
        if not isinstance(entry, dict):
            raise DomainInferenceFixtureError("Domain inference keyword fixture contains a malformed pack entry")
        pack_id = entry.get("domain_pack_id")
        keyword_groups = (entry.get("keywords"), entry.get("synonyms"))
        if pack_id not in expected or pack_id in terms_by_pack or any(not isinstance(group, list) for group in keyword_groups):
            raise DomainInferenceFixtureError("Domain inference keyword fixture contains invalid pack terms")
        terms = tuple(term.strip() for group in keyword_groups for term in group if isinstance(term, str) and term.strip())
        if not terms or len(terms) != len(set(terms)):
            raise DomainInferenceFixtureError("Domain inference keyword fixture terms must be non-empty and unique")
        terms_by_pack[pack_id] = terms  # type: ignore[index]
    if set(terms_by_pack) != expected:
        raise DomainInferenceFixtureError("Domain inference keyword fixture must cover exactly the four first-party packs")
    return DomainInferenceService(terms_by_pack)
