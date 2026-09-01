#!/usr/bin/env python3
"""Search the checked-in knife design-prior knowledge without network access.

The result is a ranked planning aid for ``KnifeDesignIntent``. It never turns a
design prior into observed reference evidence, a real-world dimension, or a
quality approval.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_KNOWLEDGE = ROOT / "references" / "crossfire-knife-knowledge.json"

ROOT_KEYS = {
    "schema_version",
    "knowledge_id",
    "route",
    "scope",
    "unit_system",
    "real_world_dimensions_permitted",
    "silhouette_grammar",
    "curve_families",
    "section_constraints",
    "attachment_ratios",
    "fps_occupancy",
    "material_readability",
    "claims",
    "non_claims",
}
CLASSIFICATIONS = {"observed", "inferred", "design-prior", "original-choice", "unknown"}
PERMITTED_USES = {"lock-visible-feature", "provisional-geometry", "rank-candidate", "author-original", "preserve-gap"}
CONFIDENCE_WEIGHT = {"high": 1.0, "medium": 0.82, "low": 0.64}

ALIASES = {
    "尼泊尔": "kukri",
    "廓尔喀": "kukri",
    "刀身": "blade body silhouette",
    "刀脊": "spine",
    "刃口": "cutting edge",
    "刃腹": "belly",
    "刀尖": "tip convergence",
    "护手": "guard",
    "龙头": "dragon guard jaw muzzle horn eye negative space",
    "刀柄": "grip",
    "握把": "grip",
    "刀首": "pommel",
    "宝石": "gem ruby accent",
    "材质": "material readability roughness metalness",
    "雕刻": "relief engraving",
    "龙纹": "dragon relief engraving scale",
    "第一人称": "fps occupancy",
    "固定视图": "fixed view camera comparison",
}


class KnowledgeError(ValueError):
    """Closed knowledge input is invalid."""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise KnowledgeError(message)


def load_knowledge(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise KnowledgeError(f"cannot load knife knowledge: {exc}") from exc
    _require(isinstance(value, dict), "knowledge root must be an object")
    _require(set(value) == ROOT_KEYS, "knowledge root fields are not closed")
    _require(value["schema_version"] == "KnifeKnowledge@1", "knowledge schema version drifted")
    _require(value["route"] == "weaponry-threejs-knife-studio", "knowledge route drifted")
    _require(value["scope"] == "nonfunctional-game-film-display", "knowledge scope drifted")
    _require(value["unit_system"] == "normalized-visual-units@1", "knowledge unit system drifted")
    _require(value["real_world_dimensions_permitted"] is False, "real-world dimensions must remain forbidden")
    claims = value["claims"]
    _require(isinstance(claims, list) and claims, "knowledge claims must be non-empty")
    claim_ids: set[str] = set()
    for index, claim in enumerate(claims):
        _require(isinstance(claim, dict), f"claims[{index}] must be an object")
        claim_id = claim.get("claim_id")
        _require(isinstance(claim_id, str) and claim_id not in claim_ids, f"claims[{index}].claim_id is invalid or duplicated")
        claim_ids.add(claim_id)
        _require(claim.get("classification") in CLASSIFICATIONS, f"claims[{index}].classification is invalid")
        _require(claim.get("permitted_use") in PERMITTED_USES, f"claims[{index}].permitted_use is invalid")
        _require(claim.get("confidence") in CONFIDENCE_WEIGHT, f"claims[{index}].confidence is invalid")
    return value


def normalize_query(query: str) -> str:
    normalized = query.lower()
    for source, replacement in ALIASES.items():
        normalized = normalized.replace(source, f" {replacement} ")
    return normalized


def tokens(text: str) -> list[str]:
    normalized = normalize_query(text)
    return [token for token in re.findall(r"[a-z0-9][a-z0-9_.-]{1,31}", normalized) if token not in {"the", "and", "for", "with", "from"}]


def _records(knowledge: dict[str, Any]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for claim in knowledge["claims"]:
        records.append(
            {
                "record_id": claim["claim_id"],
                "record_kind": "claim",
                "text": " ".join(
                    [
                        claim["claim_id"],
                        claim["statement"],
                        claim["uncertainty"],
                        claim["classification"],
                        claim["permitted_use"],
                        *claim["source_refs"],
                    ]
                ),
                "classification": claim["classification"],
                "confidence": claim["confidence"],
                "permitted_use": claim["permitted_use"],
                "payload": claim,
            }
        )
    for family in knowledge["silhouette_grammar"]["families"]:
        records.append(
            {
                "record_id": f"family-{family['family_id']}",
                "record_kind": "silhouette-family",
                "text": " ".join([family["family_id"], family["section_rhythm"], family["cue"], *family["curve_roles"]]),
                "classification": family["classification"],
                "confidence": "medium",
                "permitted_use": "rank-candidate",
                "payload": family,
            }
        )
    for curve in knowledge["curve_families"]:
        records.append(
            {
                "record_id": curve["curve_family_id"],
                "record_kind": "curve-family",
                "text": " ".join(
                    [
                        curve["curve_family_id"],
                        curve["role"],
                        curve["behavior"],
                        curve["formula"]["expression"],
                        curve["formula"].get("rationale", ""),
                    ]
                ),
                "classification": curve["classification"],
                "confidence": "medium",
                "permitted_use": "rank-candidate",
                "payload": curve,
            }
        )
    for ratio in knowledge["attachment_ratios"]:
        records.append(
            {
                "record_id": ratio["ratio_id"],
                "record_kind": "ratio-prior",
                "text": " ".join([ratio["ratio_id"], ratio["numerator"], ratio["denominator"], ratio["reason"]]),
                "classification": ratio["classification"],
                "confidence": "medium",
                "permitted_use": "rank-candidate",
                "payload": ratio,
            }
        )
    return records


def search(knowledge: dict[str, Any], query: str, limit: int) -> dict[str, Any]:
    query_terms = tokens(query)
    _require(query_terms, "query must contain at least one searchable design term")
    records = _records(knowledge)
    documents = [Counter(tokens(record["text"])) for record in records]
    document_frequency: Counter[str] = Counter()
    for document in documents:
        document_frequency.update(document.keys())

    ranked: list[tuple[float, dict[str, Any], list[str]]] = []
    query_counts = Counter(query_terms)
    for record, document in zip(records, documents):
        contributions: list[str] = []
        score = 0.0
        length_norm = 1.0 + 0.12 * max(0, sum(document.values()) - 12)
        for term, query_frequency in query_counts.items():
            term_frequency = document.get(term, 0)
            if not term_frequency:
                continue
            inverse_document_frequency = math.log((len(records) + 1) / (document_frequency[term] + 0.5)) + 1.0
            contribution = query_frequency * (1.0 + math.log(term_frequency)) * inverse_document_frequency / length_norm
            score += contribution
            contributions.append(f"{term}:{contribution:.6f}")
        if score > 0:
            score *= CONFIDENCE_WEIGHT[record["confidence"]]
            ranked.append((score, record, contributions))

    ranked.sort(key=lambda item: (-item[0], item[1]["record_id"]))
    selected = ranked[:limit]
    return {
        "schema_version": "KnifeKnowledgeSearchResult@1",
        "knowledge_id": knowledge["knowledge_id"],
        "knowledge_sha256": hashlib.sha256(canonical_bytes(knowledge)).hexdigest(),
        "query": query,
        "normalized_terms": sorted(set(query_terms)),
        "result_count": len(selected),
        "results": [
            {
                "rank": index + 1,
                "record_id": record["record_id"],
                "record_kind": record["record_kind"],
                "score": round(score, 9),
                "score_terms": contributions,
                "classification": record["classification"],
                "confidence": record["confidence"],
                "permitted_use": record["permitted_use"],
                "payload": record["payload"],
            }
            for index, (score, record, contributions) in enumerate(selected)
        ],
        "non_claims": knowledge["non_claims"],
        "decision_boundary": "ranked-design-priors-only-not-observed-truth-or-quality-approval",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("query", help="bounded knife design question or semantic scope")
    parser.add_argument("--knowledge", type=Path, default=DEFAULT_KNOWLEDGE)
    parser.add_argument("--limit", type=int, default=8, choices=range(1, 17), metavar="1..16")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = search(load_knowledge(args.knowledge), args.query, args.limit)
    except KnowledgeError as exc:
        print(f"KNIFE_KNOWLEDGE_INVALID: {exc}", file=sys.stderr)
        return 2
    encoded = json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    if args.output is not None:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    import sys

    raise SystemExit(main())
