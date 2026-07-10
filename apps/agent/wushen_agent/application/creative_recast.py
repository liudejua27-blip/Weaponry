from __future__ import annotations

import hashlib
import json
import sqlite3
import uuid

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork

from ..models import (
    CreativeGraphResponse,
    CreativeInterpretationCandidate,
    CreativeInterpretationRequest,
    CreativeInterpretationResponse,
    CreativeRecastConfirmRequest,
    CreativeRecastConfirmResponse,
    CreativeWeaponGraphPayload,
    SkillCard,
    SkillGraphPayload,
    utc_now,
)
from ..providers.creative_recast import build_mock_interpretation_candidates, stable_seed_for_text


class CreativeRecastError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class CreativeRecastIdempotencyConflict(RuntimeError):
    pass


class LegacyCreativeRecastService:
    """Frozen legacy Creative Recast use cases behind the AssetStore facade."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def create_interpretation(
        self,
        weapon_id: str,
        request: CreativeInterpretationRequest,
        idempotency_key: str,
    ) -> CreativeInterpretationResponse:
        scope = f"POST /api/weapons/{weapon_id}/interpretation"
        request_hash = _hash_json(request.model_dump())

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            connection = unit_of_work.require_connection()
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise CreativeRecastIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return CreativeInterpretationResponse(**json.loads(existing.response_json))

            if not _has_active_weapon(connection, weapon_id):
                raise CreativeRecastError("WEAPON_NOT_FOUND", "Weapon not found.")

            now = utc_now()
            interpretation_id = _new_id("interp")
            stable_seed = stable_seed_for_text(
                weapon_id,
                request.source_object,
                request.raw_description,
            )
            candidates = build_mock_interpretation_candidates(
                request,
                stable_seed=stable_seed,
                interpretation_id=interpretation_id,
            )
            valid_candidates = candidates[:3]
            affordance_axes = {tuple(candidate.combat_affordances) for candidate in valid_candidates}
            status = "ready"
            failure_code = None
            failure_reason = None
            if len(valid_candidates) < 2 or len(affordance_axes) < 2:
                status = "failed"
                failure_code = "PROVIDER_BAD_OUTPUT"
                failure_reason = "Interpretation did not produce 2 distinct candidate affordance directions."

            candidates_payload = [candidate.model_dump() for candidate in valid_candidates]
            snapshot_hash = _hash_json(candidates_payload)
            response = CreativeInterpretationResponse(
                interpretation_id=interpretation_id,
                weapon_id=weapon_id,
                source_object=request.source_object,
                raw_description=request.raw_description,
                status=status,  # type: ignore[arg-type]
                needs_confirm=status != "failed",
                candidate_count=len(valid_candidates),
                candidates=valid_candidates,
                stable_seed=stable_seed,
                resample_attempted=False,
                preserved_candidate_id=(
                    valid_candidates[0].candidate_id if valid_candidates else None
                ),
                candidate_snapshot_hash=snapshot_hash,
                failure_code=failure_code,
                failure_reason=failure_reason,
                created_at=now,
            )
            connection.execute(
                """
                INSERT INTO structure_interpretations (
                  interpretation_id, weapon_id, source_object, raw_description, status,
                  candidate_count, candidates_json, request_hash, stable_seed,
                  resample_attempted, preserved_candidate_id, candidate_snapshot_hash,
                  failure_code, failure_reason, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?)
                """,
                (
                    interpretation_id,
                    weapon_id,
                    request.source_object,
                    request.raw_description,
                    status,
                    len(valid_candidates),
                    _canonical_json(candidates_payload),
                    request_hash,
                    stable_seed,
                    response.preserved_candidate_id,
                    snapshot_hash,
                    failure_code,
                    failure_reason,
                    now,
                    now,
                ),
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump()),
                created_at=now,
            )
            return response

    def confirm_recast(
        self,
        weapon_id: str,
        request: CreativeRecastConfirmRequest,
        idempotency_key: str,
    ) -> CreativeRecastConfirmResponse:
        scope = f"POST /api/weapons/{weapon_id}/recast/confirm"
        request_hash = _hash_json(request.model_dump())

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            connection = unit_of_work.require_connection()
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise CreativeRecastIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return CreativeRecastConfirmResponse(**json.loads(existing.response_json))

            interpretation = connection.execute(
                """
                SELECT interpretation_id, weapon_id, source_object, raw_description, status,
                       candidates_json, created_at
                FROM structure_interpretations
                WHERE interpretation_id = ? AND weapon_id = ?
                """,
                (request.interpretation_id, weapon_id),
            ).fetchone()
            if interpretation is None:
                raise CreativeRecastError(
                    "INVALID_INTERPRETATION_ID",
                    "Interpretation does not belong to this weapon.",
                )
            if interpretation["status"] == "failed":
                raise CreativeRecastError(
                    "PROVIDER_BAD_OUTPUT",
                    "Failed interpretation cannot be confirmed.",
                    recoverable=True,
                )

            candidates = [
                CreativeInterpretationCandidate(**item)
                for item in json.loads(interpretation["candidates_json"] or "[]")
            ]
            selected = next(
                (
                    candidate
                    for candidate in candidates
                    if candidate.candidate_id == request.selected_candidate_id
                    and candidate.rank == request.selected_candidate_rank
                ),
                None,
            )
            if selected is None:
                raise CreativeRecastError(
                    "INVALID_INTERPRETATION_CANDIDATE",
                    "Selected candidate was not found in this interpretation.",
                )

            now = utc_now()
            creative_graph_id = _new_id("cg")
            skill_graph_id = _new_id("sg")
            creative_graph = CreativeWeaponGraphPayload(
                creative_graph_id=creative_graph_id,
                weapon_id=weapon_id,
                source_interpretation_id=request.interpretation_id,
                selected_candidate_id=selected.candidate_id,
                selected_candidate_rank=selected.rank,
                source_object=str(interpretation["source_object"]),
                recast_summary=request.recast_choice_text or selected.recast_summary,
                combat_affordances=selected.combat_affordances,
                structure_graph=selected.structure_graph,
                anchor_points=selected.anchor_points,
                protected_regions=selected.protected_regions,
                skill_anchor_points=selected.skill_anchor_points,
                unity_handoff={
                    "socket": selected.anchor_points[0],
                    "scale_policy": "normalized_game_asset_scale",
                    "axis_hint": "+Y long axis, +Z forward for Unity preview",
                },
                created_at=now,
            )
            skill_graph = SkillGraphPayload(
                skill_graph_id=skill_graph_id,
                weapon_id=weapon_id,
                origin_graph_id=creative_graph_id,
                source_interpretation_id=request.interpretation_id,
                skills=_skill_cards_for_candidate(selected),
                created_at=now,
            )
            connection.execute(
                """
                INSERT INTO creative_weapon_graphs (
                  creative_graph_id, weapon_id, origin_interpretation_id,
                  selected_candidate_id, selected_candidate_rank, graph_json,
                  graph_parent_id, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, NULL, ?)
                """,
                (
                    creative_graph_id,
                    weapon_id,
                    request.interpretation_id,
                    selected.candidate_id,
                    selected.rank,
                    _canonical_json(creative_graph.model_dump()),
                    now,
                ),
            )
            connection.execute(
                """
                INSERT INTO skill_graphs (
                  skill_graph_id, weapon_id, origin_graph_id, origin_interpretation_id,
                  skill_graph_json, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                (
                    skill_graph_id,
                    weapon_id,
                    creative_graph_id,
                    request.interpretation_id,
                    _canonical_json(skill_graph.model_dump()),
                    now,
                ),
            )
            connection.execute(
                """
                UPDATE structure_interpretations
                SET status = 'confirmed', confirmed_candidate_id = ?,
                    confirmed_at = ?, updated_at = ?
                WHERE interpretation_id = ?
                """,
                (selected.candidate_id, now, now, request.interpretation_id),
            )
            connection.execute(
                """
                UPDATE weapon_versions
                SET structure_interpretation_id = ?, creative_graph_id = ?, skill_graph_id = ?
                WHERE weapon_id = ?
                  AND version_id = (SELECT current_version_id FROM weapons WHERE weapon_id = ?)
                """,
                (
                    request.interpretation_id,
                    creative_graph_id,
                    skill_graph_id,
                    weapon_id,
                    weapon_id,
                ),
            )
            response = CreativeRecastConfirmResponse(
                weapon_id=weapon_id,
                interpretation_id=request.interpretation_id,
                selected_candidate_id=selected.candidate_id,
                selected_candidate_rank=selected.rank,
                creative_graph_id=creative_graph_id,
                skill_graph_id=skill_graph_id,
                creative_graph=creative_graph,
                skill_graph=skill_graph,
                created_at=now,
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump()),
                created_at=now,
            )
            return response

    def get_creative_graph(self, weapon_id: str) -> CreativeGraphResponse:
        with self.connection_factory.connect() as connection:
            graph = connection.execute(
                """
                SELECT creative_graph_id, origin_interpretation_id, graph_json
                FROM creative_weapon_graphs
                WHERE weapon_id = ?
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (weapon_id,),
            ).fetchone()
            if graph is None:
                raise CreativeRecastError(
                    "INTERPRETATION_NOT_CONFIRMED",
                    "No confirmed CreativeWeaponGraph exists for this weapon.",
                )
            skill = connection.execute(
                """
                SELECT skill_graph_id, skill_graph_json
                FROM skill_graphs
                WHERE origin_graph_id = ?
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (graph["creative_graph_id"],),
            ).fetchone()
        return CreativeGraphResponse(
            weapon_id=weapon_id,
            creative_graph_id=graph["creative_graph_id"],
            skill_graph_id=skill["skill_graph_id"] if skill else None,
            interpretation_id=graph["origin_interpretation_id"],
            creative_graph=CreativeWeaponGraphPayload(**json.loads(graph["graph_json"])),
            skill_graph=(
                SkillGraphPayload(**json.loads(skill["skill_graph_json"])) if skill else None
            ),
        )


def _has_active_weapon(connection: sqlite3.Connection, weapon_id: str) -> bool:
    row = connection.execute(
        "SELECT 1 FROM weapons WHERE weapon_id = ? AND status = 'active'",
        (weapon_id,),
    ).fetchone()
    return row is not None


def _skill_cards_for_candidate(candidate: CreativeInterpretationCandidate) -> list[SkillCard]:
    primary = candidate.combat_affordances[0]
    secondary = candidate.combat_affordances[1] if len(candidate.combat_affordances) > 1 else primary
    anchors = candidate.skill_anchor_points or candidate.anchor_points
    anchor = anchors[0]
    secondary_anchor = anchors[1] if len(anchors) > 1 else anchor
    return [
        SkillCard(
            slot="normal",
            name=f"{candidate.name}·试锋",
            trigger="轻击",
            effect=f"沿 {anchor} 释放短促{primary}效果。",
            anchor_point=anchor,
            combat_affordances=[primary],
            cooldown_hint="short",
            cost_hint="low",
        ),
        SkillCard(
            slot="heavy",
            name=f"{candidate.name}·蓄势",
            trigger="重击蓄力",
            effect=f"把结构能量汇入 {secondary_anchor}，形成强化{secondary}。",
            anchor_point=secondary_anchor,
            combat_affordances=[secondary],
            cooldown_hint="medium",
            cost_hint="medium",
        ),
        SkillCard(
            slot="mobility_or_defense",
            name=f"{candidate.name}·护步",
            trigger="闪避或防御",
            effect="短时展开护体边界，同时保留角色持握或穿戴姿态。",
            anchor_point=anchor,
            combat_affordances=[
                "defense" if "defense" in candidate.combat_affordances else "mobility"
            ],
            cooldown_hint="medium",
            cost_hint="medium",
        ),
        SkillCard(
            slot="control",
            name=f"{candidate.name}·锁域",
            trigger="长按技能",
            effect="围绕受保护区域生成控制节点，限制敌方移动或投射路径。",
            anchor_point=secondary_anchor,
            combat_affordances=[
                "area_control" if "area_control" in candidate.combat_affordances else "seal"
            ],
            cooldown_hint="long",
            cost_hint="medium",
        ),
        SkillCard(
            slot="passive",
            name=f"{candidate.name}·灵纹回路",
            trigger="被动",
            effect="当结构节点保持完整时提升视觉能量层级和回收稳定性。",
            anchor_point=anchor,
            combat_affordances=["recover"],
            cooldown_hint="passive",
            cost_hint="none",
        ),
        SkillCard(
            slot="ultimate",
            name=f"{candidate.name}·神兵显化",
            trigger="终结技",
            effect=f"完整展开 {candidate.name} 的重诠释形态，组合{primary}与{secondary}形成大范围表现。",
            anchor_point=secondary_anchor,
            combat_affordances=list(dict.fromkeys([primary, secondary, "transform"])),  # type: ignore[list-item]
            cooldown_hint="ultimate",
            cost_hint="high",
        ),
    ]


def _canonical_json(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: object) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:12]}"
