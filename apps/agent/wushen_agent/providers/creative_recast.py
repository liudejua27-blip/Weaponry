from __future__ import annotations

import hashlib
from typing import Iterable

from ..models import CreativeInterpretationCandidate, CreativeInterpretationRequest, CreativeStructureGraph


AFFORDANCE_SETS = [
    ("defense", "area_control"),
    ("projectile", "attack"),
    ("mobility", "transform"),
    ("summon", "seal"),
    ("reflect", "recover"),
]


def stable_seed_for_text(*parts: str) -> int:
    digest = hashlib.sha256("\n".join(part.strip() for part in parts).encode("utf-8")).hexdigest()
    return int(digest[:8], 16)


def build_mock_interpretation_candidates(
    request: CreativeInterpretationRequest,
    *,
    stable_seed: int,
    interpretation_id: str,
) -> list[CreativeInterpretationCandidate]:
    source = _compact(request.source_object) or _infer_source_object(request.raw_description)
    description = _compact(request.raw_description) or source
    rotations = _rotated_affordance_sets(stable_seed)
    names = [
        ("环阵守域形", "把对象外缘重诠释为护体边界、领域墙和可展开阵面。"),
        ("脉冲炮座形", "把对象的长轴、口袋、边缘或开口重诠释为能量汇聚与发射路径。"),
        ("折叠机巧形", "把对象的折痕、连接和悬挂关系重诠释为位移、变形和回收机构。"),
    ]
    candidates: list[CreativeInterpretationCandidate] = []
    for index in range(3):
        rank = index + 1
        affordances = list(rotations[index])
        name_suffix, intent = names[index]
        candidate_seed = stable_seed + rank * 101
        candidate_id = f"cand_{hashlib.sha1(f'{interpretation_id}:{rank}:{candidate_seed}'.encode('utf-8')).hexdigest()[:10]}"
        anchor_points = _anchor_points(source, rank)
        protected_regions = _protected_regions(source, rank)
        skill_anchor_points = [anchor_points[0], f"{source}_skill_node_{rank}"]
        candidates.append(
            CreativeInterpretationCandidate(
                candidate_id=candidate_id,
                rank=rank,
                name=f"{source}{name_suffix}",
                summary=f"{description} -> {intent}",
                recast_summary=f"{source}不被预分类为武器，而是重诠释为{intent}",
                combat_affordances=affordances,  # type: ignore[arg-type]
                confidence=round(0.84 - index * 0.06 + (stable_seed % 7) * 0.005, 3),
                anchor_points=anchor_points,
                protected_regions=protected_regions,
                skill_anchor_points=skill_anchor_points,
                risk_tags=_risk_tags(rank),
                structure_graph=CreativeStructureGraph(
                    skeleton=[f"{source}_outer_contour", f"{source}_load_axis_{rank}", f"{source}_secondary_ring"],
                    interaction_path=_interaction_path(source, rank),
                    attack_sources=[f"{source}_energy_mouth_{rank}", f"{source}_rune_edge_{rank}"],
                    movable_nodes=[f"{source}_fold_joint_{rank}", f"{source}_floating_guard_{rank}"],
                    energy_flow=[f"{source}_core", f"{source}_rune_channel_{rank}", f"{source}_release_arc_{rank}"],
                ),
                candidate_seed=candidate_seed,
            )
        )
    return candidates


def _rotated_affordance_sets(seed: int) -> list[tuple[str, str]]:
    offset = seed % len(AFFORDANCE_SETS)
    rotated = AFFORDANCE_SETS[offset:] + AFFORDANCE_SETS[:offset]
    return rotated[:3]


def _compact(value: str) -> str:
    compact = "".join(value.strip().split())
    return compact[:18]


def _infer_source_object(raw_description: str) -> str:
    for token in ["裤", "棍", "椅", "镜", "伞", "门", "戒指", "树枝", "花盆", "钥匙", "铃", "风车", "花环"]:
        if token in raw_description:
            return token
    compact = _compact(raw_description)
    return compact[:8] if compact else "未知物件"


def _anchor_points(source: str, rank: int) -> list[str]:
    if rank == 1:
        return [f"{source}_waist_or_grip_anchor", f"{source}_outer_ring_anchor"]
    if rank == 2:
        return [f"{source}_barrel_axis_anchor", f"{source}_palm_socket_anchor"]
    return [f"{source}_hinge_anchor", f"{source}_back_socket_anchor"]


def _protected_regions(source: str, rank: int) -> list[str]:
    if rank == 1:
        return [f"{source}_core_silhouette", f"{source}_defense_shell"]
    if rank == 2:
        return [f"{source}_emission_mouth", f"{source}_rune_channel"]
    return [f"{source}_fold_joint", f"{source}_transform_outline"]


def _interaction_path(source: str, rank: int) -> list[str]:
    if rank == 1:
        return [f"wear_or_place_{source}", "expand_field", "hold_boundary"]
    if rank == 2:
        return [f"grip_or_align_{source}", "charge_core", "release_projectile"]
    return [f"attach_{source}", "fold_transform", "recover_to_idle"]


def _risk_tags(rank: int) -> list[str]:
    tags = [
        ["pivot_sensitive", "silhouette_must_stay_readable"],
        ["emission_direction_sensitive", "projectile_mouth_must_not_be_blueprint"],
        ["symmetry_break", "joint_density_limit"],
    ]
    return tags[rank - 1]
