"""Bounded concept-scope policy for the normal ForgeCAD Agent path.

This is a narrow product-scope decision, not a claim of complete content
moderation.  It runs locally after domain inference and before a planner can
receive a user brief.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Optional, Sequence

from pydantic import Field, model_validator

from .concept_models import StrictApiModel
from .domain_inference import DomainInferenceResult
from .domain_packs import DomainPackId, list_domain_packs


ConceptScopeStatus = Literal["allowed", "clarification_required", "unsupported"]
ConceptScopeReason = Literal[
    "allowed_non_functional_concept",
    "domain_ambiguous",
    "domain_unknown",
    "real_weapon_or_manufacturing",
    "engineering_safety_or_control",
]


class ConceptScopeDecision(StrictApiModel):
    schema_version: Literal["ConceptScopeDecision@1"] = "ConceptScopeDecision@1"
    policy_version: Literal["ForgeCADConceptScopePolicy@1"] = "ForgeCADConceptScopePolicy@1"
    status: ConceptScopeStatus
    reason_code: ConceptScopeReason
    domain_pack_id: Optional[DomainPackId] = None
    candidate_domain_pack_ids: list[DomainPackId] = Field(default_factory=list, max_length=4)
    matched_policy_rule_ids: list[str] = Field(default_factory=list, max_length=4)
    user_message: str = Field(min_length=1, max_length=240)

    @model_validator(mode="after")
    def validate_shape(self) -> "ConceptScopeDecision":
        if len(self.candidate_domain_pack_ids) != len(set(self.candidate_domain_pack_ids)):
            raise ValueError("candidate_domain_pack_ids must be unique")
        if len(self.matched_policy_rule_ids) != len(set(self.matched_policy_rule_ids)):
            raise ValueError("matched_policy_rule_ids must be unique")
        if any(not rule.startswith("scope_") for rule in self.matched_policy_rule_ids):
            raise ValueError("matched_policy_rule_ids must use scope_ ids")
        if self.status == "allowed":
            if self.reason_code != "allowed_non_functional_concept" or self.domain_pack_id is None or self.candidate_domain_pack_ids != [self.domain_pack_id] or self.matched_policy_rule_ids:
                raise ValueError("allowed requires one domain pack and no matching policy rule")
        elif self.status == "clarification_required":
            if self.reason_code not in {"domain_ambiguous", "domain_unknown"} or self.domain_pack_id is not None or not 2 <= len(self.candidate_domain_pack_ids) <= 4 or self.matched_policy_rule_ids:
                raise ValueError("clarification_required requires two to four packs and no policy match")
        elif self.reason_code not in {"real_weapon_or_manufacturing", "engineering_safety_or_control"} or self.domain_pack_id is not None or self.candidate_domain_pack_ids or not self.matched_policy_rule_ids:
            raise ValueError("unsupported requires a matched policy rule and no domain selection")
        return self


@dataclass(frozen=True)
class _ScopeRule:
    rule_id: str
    reason_code: Literal["real_weapon_or_manufacturing", "engineering_safety_or_control"]
    phrases: tuple[str, ...]


# Deliberately limited, reviewed phrases. This policy must not be described as
# an exhaustive moderator; the safe product boundary is also enforced by tools,
# ShapeProgram restrictions, confirmation, and the Provider system instruction.
_SCOPE_RULES: tuple[_ScopeRule, ...] = (
    _ScopeRule(
        "scope_real_weapon_operation",
        "real_weapon_or_manufacturing",
        ("现实枪械", "真实枪械", "可实际发射", "发射机构", "武器机构", "枪械加工", "弹药"),
    ),
    _ScopeRule(
        "scope_manufacturing_specification",
        "real_weapon_or_manufacturing",
        ("加工尺寸", "制造尺寸", "制造步骤", "加工步骤", "加工图", "加工图纸", "材料配方", "材料牌号", "生产图", "可生产"),
    ),
    _ScopeRule(
        "scope_engineering_safety_control",
        "engineering_safety_or_control",
        ("起飞载荷", "适航", "碰撞安全", "制动设计", "扭矩计算", "控制程序", "飞行控制", "安全认证", "结构强度", "认证结论"),
    ),
)


def decide_concept_scope(
    message: str,
    inference: DomainInferenceResult,
    *,
    selected_domain_pack_id: Optional[DomainPackId] = None,
) -> ConceptScopeDecision:
    """Return a local decision before a Provider or planner is touched."""

    matched_rules = _matched_scope_rules(message)
    if matched_rules:
        reason = (
            "engineering_safety_or_control"
            if any(rule.reason_code == "engineering_safety_or_control" for rule in matched_rules)
            else "real_weapon_or_manufacturing"
        )
        return ConceptScopeDecision(
            status="unsupported",
            reason_code=reason,
            matched_policy_rule_ids=[rule.rule_id for rule in matched_rules],
            user_message="这个请求涉及现实制造、安全、控制或性能内容。我只能帮助制作非功能性的外观概念、游戏资产或展示道具。",
        )
    if selected_domain_pack_id is not None:
        return _allowed(selected_domain_pack_id)
    if inference.status == "recognized" and inference.domain_pack_id is not None:
        return _allowed(inference.domain_pack_id)
    if inference.status == "ambiguous":
        return ConceptScopeDecision(
            status="clarification_required",
            reason_code="domain_ambiguous",
            candidate_domain_pack_ids=inference.candidate_domain_pack_ids,
            user_message="这段创意同时接近多个方向。请先选择想设计的对象类别。",
        )
    return ConceptScopeDecision(
        status="clarification_required",
        reason_code="domain_unknown",
        candidate_domain_pack_ids=[pack.pack_id for pack in list_domain_packs()],
        user_message="我还不能判断对象类别。请先选择汽车、飞机、机械臂，或未来概念道具。",
    )


def _allowed(pack_id: DomainPackId) -> ConceptScopeDecision:
    return ConceptScopeDecision(
        status="allowed",
        reason_code="allowed_non_functional_concept",
        domain_pack_id=pack_id,
        candidate_domain_pack_ids=[pack_id],
        user_message="已确认这是非功能性的机械概念外观，可以开始规划。",
    )


def _matched_scope_rules(message: str) -> Sequence[_ScopeRule]:
    normalized = message.casefold()
    return tuple(rule for rule in _SCOPE_RULES if any(phrase.casefold() in normalized for phrase in rule.phrases))
