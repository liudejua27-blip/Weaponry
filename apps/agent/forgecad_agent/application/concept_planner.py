from __future__ import annotations

import hashlib
import json
import os
import re
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, Optional, Protocol, Sequence, Union

from pydantic import BaseModel, ConfigDict, Field, model_validator

from forgecad_agent.application.concept_models import ConceptPlannerProvenance
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec


class ConceptPlannerError(RuntimeError):
    def __init__(self, code: str, message: str, recoverable: bool = True) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class ConceptPlannerProvider(Protocol):
    provider_id: str
    provider_type: Literal["deterministic", "openai_compatible"]
    model_name: Optional[str]
    last_call_metrics: Optional["PlannerCallMetrics"]

    def interpret_brief(
        self,
        *,
        source_text: str,
        current_spec: WeaponConceptSpec,
        module_catalog: Sequence[dict[str, str]],
    ) -> WeaponConceptSpec:
        ...

    def plan_variants(
        self,
        *,
        source_text: str,
        interpreted_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
    ) -> list["ConceptVariantPlan"]:
        ...

    def plan_change_set(
        self,
        *,
        instruction: str,
        current_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
        selected_node_id: Optional[str],
        selected_module_id: Optional[str],
    ) -> "ConceptChangePlan":
        ...


class _StrictPlannerModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


@dataclass(frozen=True)
class PlannerCallMetrics:
    latency_ms: int
    input_tokens: Optional[int] = None
    output_tokens: Optional[int] = None
    total_tokens: Optional[int] = None


class ConceptBriefPatch(_StrictPlannerModel):
    keywords: list[str] = Field(min_length=1, max_length=12)
    palette: list[str] = Field(min_length=1, max_length=8)
    detail_density: float = Field(ge=0, le=1)
    overall_length_mm: float = Field(gt=0, le=1000)
    body_height_mm: float = Field(gt=0, le=1000)
    grip_angle_deg: float = Field(ge=-45, le=45)
    symmetry: Literal["symmetric", "mostly_symmetric", "asymmetric"]


class ConceptVariantNodeTransform(_StrictPlannerModel):
    """A bounded visual adjustment for an existing editable ModuleGraph node.

    Variants deliberately adjust existing display modules only. They do not add
    functional components or create manufacturing geometry, while still making
    the A/B/C silhouette meaningfully different in the CAD viewport.
    """

    node_id: str = Field(min_length=1)
    position: Optional[list[float]] = Field(default=None, min_length=3, max_length=3)
    rotation: Optional[list[float]] = Field(default=None, min_length=3, max_length=3)
    scale: Optional[list[float]] = Field(default=None, min_length=3, max_length=3)
    mirror_axis: Optional[Literal["none", "x", "y", "z"]] = None

    @model_validator(mode="after")
    def validate_transform(self) -> "ConceptVariantNodeTransform":
        if (
            self.position is None
            and self.rotation is None
            and self.scale is None
            and self.mirror_axis is None
        ):
            raise ValueError("variant node transform requires at least one adjustment")
        if self.position is not None and any(abs(value) > 500 for value in self.position):
            raise ValueError("variant positions must stay inside the display workspace")
        if self.rotation is not None and any(abs(value) > 3.15 for value in self.rotation):
            raise ValueError("variant rotations must stay inside [-3.15, 3.15]")
        if self.scale is not None and any(value < 0.9 or value > 1.1 for value in self.scale):
            raise ValueError("variant scale values must stay inside [0.9, 1.1]")
        return self


class ConceptVariantPlan(_StrictPlannerModel):
    rank: int = Field(ge=1, le=3)
    name: str = Field(min_length=1, max_length=120)
    summary: str = Field(min_length=1, max_length=500)
    target_node_id: str = Field(min_length=1)
    scale: list[float] = Field(min_length=3, max_length=3)
    node_transforms: list[ConceptVariantNodeTransform] = Field(default_factory=list, max_length=8)
    recommended_module_ids: list[str] = Field(min_length=1, max_length=12)
    rationale: list[str] = Field(min_length=1, max_length=12)

    @model_validator(mode="after")
    def validate_scale(self) -> "ConceptVariantPlan":
        if any(value < 0.9 or value > 1.1 for value in self.scale):
            raise ValueError("variant scale values must stay inside [0.9, 1.1]")
        if len(set(self.recommended_module_ids)) != len(self.recommended_module_ids):
            raise ValueError("recommended module ids must be unique")
        if len({item.node_id for item in self.node_transforms}) != len(self.node_transforms):
            raise ValueError("variant node transforms must not target a node twice")
        return self


class ConceptVariantPlanBatch(_StrictPlannerModel):
    variants: list[ConceptVariantPlan] = Field(min_length=3, max_length=3)

    @model_validator(mode="after")
    def validate_ranks(self) -> "ConceptVariantPlanBatch":
        if sorted(item.rank for item in self.variants) != [1, 2, 3]:
            raise ValueError("variant ranks must be exactly 1, 2, 3")
        return self


class ConceptChangeOperationPlan(_StrictPlannerModel):
    op: Literal["replace_module", "set_mirror", "set_style", "set_parameter"]
    node_id: Optional[str]
    module_id: Optional[str]
    path: Optional[str]
    value: Optional[Union[float, str, list[str]]]
    mirror_axis: Optional[Literal["none", "x", "y", "z"]]
    rationale: str = Field(min_length=1, max_length=300)

    @model_validator(mode="after")
    def validate_payload(self) -> "ConceptChangeOperationPlan":
        if self.op == "replace_module" and (not self.node_id or not self.module_id):
            raise ValueError("replace_module requires node_id and module_id")
        if self.op == "set_mirror" and (not self.node_id or self.mirror_axis is None):
            raise ValueError("set_mirror requires node_id and mirror_axis")
        if self.op in {"set_style", "set_parameter"} and (
            not self.path or self.value is None
        ):
            raise ValueError(f"{self.op} requires path and value")
        return self


class ConceptChangePlan(_StrictPlannerModel):
    summary: str = Field(min_length=1, max_length=500)
    operations: list[ConceptChangeOperationPlan] = Field(min_length=1, max_length=8)
    rationale: list[str] = Field(min_length=1, max_length=12)


def _variant_transforms(
    editable_by_id: dict[str, Any],
    requested: dict[str, dict[str, Any]],
) -> list[ConceptVariantNodeTransform]:
    """Keep deterministic templates portable across smaller registered packs."""

    return [
        ConceptVariantNodeTransform(node_id=node_id, **payload)
        for node_id, payload in requested.items()
        if node_id in editable_by_id
    ]


class DeterministicConceptPlanner:
    provider_id = "deterministic_concept_rules"
    provider_type: Literal["deterministic"] = "deterministic"
    model_name: Optional[str] = None
    last_call_metrics: Optional[PlannerCallMetrics] = None

    def interpret_brief(
        self,
        *,
        source_text: str,
        current_spec: WeaponConceptSpec,
        module_catalog: Sequence[dict[str, str]],
    ) -> WeaponConceptSpec:
        del module_catalog
        payload = current_spec.model_dump(mode="json")
        normalized = source_text.lower()
        proportions = payload["proportions"]
        if any(token in normalized for token in ("紧凑", "短小", "compact")):
            proportions["overall_length_mm"] = round(
                max(1.0, proportions["overall_length_mm"] * 0.9), 6
            )
            proportions["body_height_mm"] = round(
                max(1.0, proportions["body_height_mm"] * 0.95), 6
            )
        elif any(token in normalized for token in ("延展", "修长", "long profile")):
            proportions["overall_length_mm"] = round(
                min(1000.0, proportions["overall_length_mm"] * 1.08), 6
            )
        if any(token in normalized for token in ("精密", "细致", "高细节", "丰富细节")):
            payload["style"]["detail_density"] = max(
                0.82, payload["style"]["detail_density"]
            )
        elif any(token in normalized for token in ("简洁", "低细节", "极简")):
            payload["style"]["detail_density"] = min(
                0.45, payload["style"]["detail_density"]
            )

        explicit_values = (
            (
                "overall_length_mm",
                _extract_number(
                    normalized,
                    (
                        r"(?:整体)?长度[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm|毫米)",
                        r"overall\s+length[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm)?",
                    ),
                ),
                0.0,
                1000.0,
            ),
            (
                "body_height_mm",
                _extract_number(
                    normalized,
                    (
                        r"(?:主体|机身|本体)(?:高度|高)[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm|毫米)",
                        r"body\s+height[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm)?",
                    ),
                ),
                0.0,
                1000.0,
            ),
            (
                "grip_angle_deg",
                _extract_number(
                    normalized,
                    (
                        r"(?:握把|握持)(?:角度|角)[^0-9-]{0,12}(-?\d+(?:\.\d+)?)\s*(?:°|度)?",
                        r"grip\s+angle[^0-9-]{0,12}(-?\d+(?:\.\d+)?)\s*(?:degrees?)?",
                    ),
                ),
                -45.0,
                45.0,
            ),
        )
        for key, value, minimum, maximum in explicit_values:
            if value is None:
                continue
            within_range = (
                minimum < value <= maximum
                if minimum == 0
                else minimum <= value <= maximum
            )
            if within_range:
                proportions[key] = value
        explicit_density = _extract_number(
            normalized,
            (
                r"(?:细节密度|细节)[^0-9]{0,12}(\d+(?:\.\d+)?)\s*%",
                r"detail\s+density[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:%|percent)",
            ),
        )
        if explicit_density is not None and 0 <= explicit_density <= 100:
            payload["style"]["detail_density"] = round(explicit_density / 100.0, 6)
        if any(token in normalized for token in ("非对称", "asymmetric")):
            payload["constraints"]["symmetry"] = "asymmetric"
        elif any(token in normalized for token in ("严格对称", "左右对称", "symmetric")):
            payload["constraints"]["symmetry"] = "symmetric"

        known_keywords = (
            "寒地",
            "工业",
            "紧凑",
            "模块化",
            "未来",
            "硬表面",
            "轻量",
            "装甲",
            "展示",
            "精密",
        )
        payload["style"]["keywords"] = _unique_limited(
            payload["style"]["keywords"]
            + [keyword for keyword in known_keywords if keyword in source_text],
            12,
        )
        color_tokens = (
            (("石墨", "graphite"), "graphite"),
            (("枪灰", "gunmetal"), "gunmetal"),
            (("红", "red"), "signal_red"),
            (("蓝", "blue"), "signal_blue"),
            (("白", "white"), "arctic_white"),
        )
        additions = [
            color
            for tokens, color in color_tokens
            if any(token in normalized for token in tokens)
        ]
        payload["style"]["palette"] = _unique_limited(
            payload["style"]["palette"] + additions, 8
        )
        return WeaponConceptSpec.model_validate(payload)

    def plan_variants(
        self,
        *,
        source_text: str,
        interpreted_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
    ) -> list[ConceptVariantPlan]:
        del source_text, interpreted_spec
        editable = [
            node
            for node in base_graph.nodes
            if node.node_id != base_graph.root_node_id and not node.locked
        ]
        if not editable:
            raise ConceptPlannerError(
                "PLANNER_NO_EDITABLE_NODE",
                "Current ModuleGraph has no editable non-root module.",
                False,
            )
        editable_by_id = {node.node_id: node for node in editable}
        compact_transforms = _variant_transforms(
            editable_by_id,
            {
                "node_front": {"position": [-44.0, 0.0, 0.0], "scale": [0.9, 0.9, 0.9]},
                "node_rear": {"position": [44.0, 0.0, 0.0], "scale": [0.9, 0.92, 0.92]},
                "node_top": {"position": [0.0, 21.0, 0.0], "scale": [0.9, 0.9, 0.9]},
                "node_grip": {"position": [10.0, -22.0, 0.0], "scale": [0.9, 0.92, 0.92]},
            },
        )
        armored_transforms = _variant_transforms(
            editable_by_id,
            {
                "node_front": {"position": [-52.0, 0.0, 0.0], "scale": [1.02, 1.08, 1.08]},
                "node_armor": {"position": [0.0, 0.0, -23.0], "scale": [1.1, 1.1, 1.1]},
                "node_side": {"position": [0.0, 0.0, 24.0], "scale": [1.1, 1.04, 1.1]},
                "node_top": {"position": [-4.0, 28.0, 0.0], "scale": [1.05, 1.1, 1.1]},
            },
        )
        showcase_transforms = _variant_transforms(
            editable_by_id,
            {
                "node_front": {"position": [-62.0, 0.0, 0.0], "scale": [1.1, 1.04, 1.04]},
                "node_rear": {"position": [57.0, 0.0, 0.0], "scale": [1.1, 1.04, 1.06]},
                "node_grip": {"position": [17.0, -28.0, 0.0], "rotation": [0.0, 0.0, -0.1], "scale": [1.04, 1.1, 1.06]},
                "node_storage": {"position": [35.0, -28.0, 0.0], "scale": [1.08, 1.1, 1.08]},
            },
        )
        choices = (
            ("A · 紧凑轮廓", "缩短前后轮廓并压低顶部附件，形成紧凑精密的展示姿态。", [0.9, 0.9, 0.9], compact_transforms),
            ("B · 装甲均衡", "强化正面装甲、侧向体块与顶部体量，形成厚实均衡的未来展示轮廓。", [1.02, 1.08, 1.08], armored_transforms),
            ("C · 延展展示", "延展前后体块并拉开下部模块，形成更有张力的长轴展示轮廓。", [1.1, 1.04, 1.04], showcase_transforms),
        )
        plans: list[ConceptVariantPlan] = []
        for index, (name, summary, scale, transforms) in enumerate(choices):
            target = editable[min(index, len(editable) - 1)]
            category = next(
                (
                    item["category"]
                    for item in module_catalog
                    if item["module_id"] == target.module_id
                ),
                "",
            )
            recommended = [
                item["module_id"]
                for item in module_catalog
                if item["category"] == category
            ][:3]
            plans.append(
                ConceptVariantPlan(
                    rank=index + 1,
                    name=name,
                    summary=summary,
                    target_node_id=target.node_id,
                    scale=scale,
                    node_transforms=transforms,
                    recommended_module_ids=recommended,
                    rationale=[
                        f"只修改未锁定节点 {target.node_id}",
                        "模块建议来自当前项目 Profile 对应的注册表",
                        "保持 Connector Graph 与 root 节点不变",
                    ],
                )
            )
        return plans

    def plan_change_set(
        self,
        *,
        instruction: str,
        current_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
        selected_node_id: Optional[str],
        selected_module_id: Optional[str],
    ) -> ConceptChangePlan:
        normalized = instruction.lower()
        operations: list[ConceptChangeOperationPlan] = []
        current_spec_payload = current_spec.model_dump(mode="json")
        graph_nodes = {node.node_id: node for node in base_graph.nodes}
        catalog_by_id = {item["module_id"]: item for item in module_catalog}
        current_categories = {
            node.node_id: catalog_by_id.get(node.module_id, {}).get("category", "")
            for node in base_graph.nodes
        }

        target_node_id = selected_node_id if selected_node_id in graph_nodes else None
        requested_module_id = next(
            (
                item["module_id"]
                for item in module_catalog
                if item["module_id"].lower() in normalized
            ),
            None,
        )
        replacement_requested = any(
            token in normalized
            for token in ("替换", "换成", "换为", "候选模块", "replace")
        )
        if replacement_requested:
            replacement_module_id = requested_module_id or selected_module_id
            if (
                target_node_id
                and replacement_module_id
                and graph_nodes[target_node_id].module_id != replacement_module_id
            ):
                operations.append(
                    ConceptChangeOperationPlan(
                        op="replace_module",
                        node_id=target_node_id,
                        module_id=replacement_module_id,
                        path=None,
                        value=None,
                        mirror_axis=None,
                        rationale="使用当前选中节点与注册表候选模块形成可审计替换。",
                    )
                )
            else:
                category = _mentioned_category(normalized)
                category_node = next(
                    (
                        node
                        for node in base_graph.nodes
                        if current_categories.get(node.node_id) == category
                        and node.node_id != base_graph.root_node_id
                        and not node.locked
                    ),
                    None,
                )
                alternative = next(
                    (
                        item["module_id"]
                        for item in module_catalog
                        if item["category"] == category
                        and category_node is not None
                        and item["module_id"] != category_node.module_id
                    ),
                    None,
                )
                if category_node is not None and alternative is not None:
                    operations.append(
                        ConceptChangeOperationPlan(
                            op="replace_module",
                            node_id=category_node.node_id,
                            module_id=alternative,
                            path=None,
                            value=None,
                            mirror_axis=None,
                            rationale="按指令中的模块类别选择同类别注册替代件。",
                        )
                    )

        explicit_parameters = (
            (
                "proportions.overall_length_mm",
                _extract_number(
                    normalized,
                    (
                        r"(?:整体)?长度[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm|毫米)",
                        r"overall\s+length[^0-9]{0,12}(\d+(?:\.\d+)?)",
                    ),
                ),
                "整体长度",
            ),
            (
                "proportions.body_height_mm",
                _extract_number(
                    normalized,
                    (
                        r"(?:主体|机身|本体)(?:高度|高)[^0-9]{0,12}(\d+(?:\.\d+)?)\s*(?:mm|毫米)",
                        r"body\s+height[^0-9]{0,12}(\d+(?:\.\d+)?)",
                    ),
                ),
                "主体高度",
            ),
            (
                "proportions.grip_angle_deg",
                _extract_number(
                    normalized,
                    (
                        r"(?:握把|握持)(?:角度|角)[^0-9-]{0,12}(-?\d+(?:\.\d+)?)\s*(?:°|度)?",
                        r"grip\s+angle[^0-9-]{0,12}(-?\d+(?:\.\d+)?)",
                    ),
                ),
                "握持角",
            ),
        )
        parameter_paths = set()
        for path, value, label in explicit_parameters:
            if value is None:
                continue
            section, key = path.split(".")
            if float(current_spec_payload[section][key]) == value:
                continue
            parameter_paths.add(path)
            operations.append(
                ConceptChangeOperationPlan(
                    op="set_parameter",
                    node_id=None,
                    module_id=None,
                    path=path,
                    value=value,
                    mirror_axis=None,
                    rationale=f"将{label}设置为指令中的明确数值。",
                )
            )

        if "proportions.overall_length_mm" not in parameter_paths:
            current_length = current_spec.proportions.overall_length_mm
            if any(token in normalized for token in ("更紧凑", "缩短", "shorter")):
                next_length = max(1.0, round(current_length * 0.95, 6))
                if next_length != current_length:
                    operations.append(
                        _numeric_change(
                            "proportions.overall_length_mm",
                            next_length,
                            "按视觉指令将整体长度缩短 5%。",
                        )
                    )
            elif any(token in normalized for token in ("更修长", "延长", "longer")):
                next_length = min(1000.0, round(current_length * 1.05, 6))
                if next_length != current_length:
                    operations.append(
                        _numeric_change(
                            "proportions.overall_length_mm",
                            next_length,
                            "按视觉指令将整体长度延长 5%。",
                        )
                    )

        detail_density = _extract_number(
            normalized,
            (
                r"(?:细节密度|细节)[^0-9]{0,12}(\d+(?:\.\d+)?)\s*%",
                r"detail\s+density[^0-9]{0,12}(\d+(?:\.\d+)?)\s*%?",
            ),
        )
        if detail_density is not None:
            next_density = round(detail_density / 100.0, 6)
            if next_density != current_spec.style.detail_density:
                operations.append(
                    ConceptChangeOperationPlan(
                        op="set_style",
                        node_id=None,
                        module_id=None,
                        path="style.detail_density",
                        value=next_density,
                        mirror_axis=None,
                        rationale="将细节密度设置为指令中的百分比。",
                    )
                )
        elif any(token in normalized for token in ("增加细节", "更精密", "更细致")):
            next_density = min(1.0, round(current_spec.style.detail_density + 0.1, 6))
            if next_density != current_spec.style.detail_density:
                operations.append(
                    ConceptChangeOperationPlan(
                        op="set_style",
                        node_id=None,
                        module_id=None,
                        path="style.detail_density",
                        value=next_density,
                        mirror_axis=None,
                        rationale="按视觉指令提高细节密度 0.1。",
                    )
                )
        elif any(token in normalized for token in ("减少细节", "更简洁", "低细节")):
            next_density = max(0.0, round(current_spec.style.detail_density - 0.1, 6))
            if next_density != current_spec.style.detail_density:
                operations.append(
                    ConceptChangeOperationPlan(
                        op="set_style",
                        node_id=None,
                        module_id=None,
                        path="style.detail_density",
                        value=next_density,
                        mirror_axis=None,
                        rationale="按视觉指令降低细节密度 0.1。",
                    )
                )

        palette_additions = _recognized_palette(normalized)
        if palette_additions and any(
            token in normalized for token in ("颜色", "配色", "点缀", "palette", "color")
        ):
            next_palette = _unique_limited(
                list(current_spec.style.palette) + palette_additions, 8
            )
            if next_palette != current_spec.style.palette:
                operations.append(
                    ConceptChangeOperationPlan(
                        op="set_style",
                        node_id=None,
                        module_id=None,
                        path="style.palette",
                        value=next_palette,
                        mirror_axis=None,
                        rationale="仅使用识别出的展示配色更新概念资产调色板。",
                    )
                )

        mirror_requested = "镜像" in normalized or "mirror" in normalized
        if mirror_requested and target_node_id:
            axis: Literal["none", "x", "y", "z"] = "none"
            if not any(token in normalized for token in ("取消镜像", "清除镜像", "unmirror")):
                axis = (
                    "y"
                    if re.search(r"(?:^|\s)y(?:\s|$)|y\s*轴", normalized)
                    else "z"
                    if re.search(r"(?:^|\s)z(?:\s|$)|z\s*轴", normalized)
                    else "x"
                )
            if graph_nodes[target_node_id].mirror_axis != axis:
                operations.append(
                    ConceptChangeOperationPlan(
                        op="set_mirror",
                        node_id=target_node_id,
                        module_id=None,
                        path=None,
                        value=None,
                        mirror_axis=axis,
                        rationale=f"对选中节点设置 {axis.upper()} 镜像状态。",
                    )
                )

        if not operations:
            raise ConceptPlannerError(
                "PLANNER_NO_ACTION",
                "Instruction did not contain a supported visual parameter, style, mirror, or registry replacement change.",
                False,
            )
        return ConceptChangePlan(
            summary=f"自然语言修改：{instruction.strip()[:420]}",
            operations=operations,
            rationale=[
                "只生成 DesignChangeSet@1 支持的受限视觉操作。",
                "锁定节点、注册模块和 Connector 约束由服务端再次校验。",
                "确认前只生成 ghost preview，不覆盖当前版本。",
            ],
        )


@dataclass(frozen=True)
class OpenAICompatibleConceptPlannerConfig:
    base_url: str
    model: str
    api_key: Optional[str]
    timeout_seconds: float = 60.0
    response_mode: Literal["auto", "json_schema", "json_object"] = "auto"
    max_output_tokens: int = 4096


class OpenAICompatibleConceptPlanner:
    provider_id = "openai_compatible_concept_planner"
    provider_type: Literal["openai_compatible"] = "openai_compatible"

    def __init__(self, config: OpenAICompatibleConceptPlannerConfig) -> None:
        self.config = config
        self.model_name = config.model or None
        self.last_call_metrics: Optional[PlannerCallMetrics] = None

    def interpret_brief(
        self,
        *,
        source_text: str,
        current_spec: WeaponConceptSpec,
        module_catalog: Sequence[dict[str, str]],
    ) -> WeaponConceptSpec:
        patch = ConceptBriefPatch.model_validate(
            self._chat(
                schema_name="forgecad_concept_brief_patch",
                schema=ConceptBriefPatch.model_json_schema(),
                system=(
                    "You are ForgeCAD Brief Interpreter for fictional future weapon concepts, "
                    "game assets, film props, and non-functional display models. Return only the "
                    "requested JSON. Do not produce real weapon engineering, mechanisms, machining, "
                    "assembly, performance, ammunition, or manufacturing instructions."
                ),
                user=_canonical_json(
                    {
                        "task": "Interpret visual style and bounded asset proportions only.",
                        "brief": source_text,
                        "current_spec": current_spec.model_dump(mode="json"),
                        "registered_modules": list(module_catalog),
                    }
                ),
            )
        )
        payload = current_spec.model_dump(mode="json")
        payload["style"] = {
            "keywords": patch.keywords,
            "palette": patch.palette,
            "detail_density": patch.detail_density,
        }
        payload["proportions"] = {
            "overall_length_mm": patch.overall_length_mm,
            "body_height_mm": patch.body_height_mm,
            "grip_angle_deg": patch.grip_angle_deg,
        }
        payload["constraints"]["symmetry"] = patch.symmetry
        return WeaponConceptSpec.model_validate(payload)

    def plan_variants(
        self,
        *,
        source_text: str,
        interpreted_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
    ) -> list[ConceptVariantPlan]:
        batch = ConceptVariantPlanBatch.model_validate(
            self._chat(
                schema_name="forgecad_concept_variant_plans",
                schema=ConceptVariantPlanBatch.model_json_schema(),
                system=(
                    "You are ForgeCAD Module Planner. Return exactly three visually distinct, "
                    "non-functional concept asset plans. Reference only supplied node and module IDs. "
                    "Use scale only inside 0.9 to 1.1. Never provide functional weapon or manufacturing instructions."
                ),
                user=_canonical_json(
                    {
                        "brief": source_text,
                        "interpreted_spec": interpreted_spec.model_dump(mode="json"),
                        "base_graph": base_graph.model_dump(mode="json"),
                        "registered_modules": list(module_catalog),
                    }
                ),
            )
        )
        return sorted(batch.variants, key=lambda item: item.rank)

    def plan_change_set(
        self,
        *,
        instruction: str,
        current_spec: WeaponConceptSpec,
        base_graph: ModuleGraph,
        module_catalog: Sequence[dict[str, str]],
        selected_node_id: Optional[str],
        selected_module_id: Optional[str],
    ) -> ConceptChangePlan:
        return ConceptChangePlan.model_validate(
            self._chat(
                schema_name="forgecad_concept_change_plan",
                schema=ConceptChangePlan.model_json_schema(),
                system=(
                    "You are ForgeCAD Change Planner for fictional future weapon concepts, game "
                    "assets, film props, and non-functional display models. Convert the request "
                    "into a small, explainable visual DesignChangeSet plan. You may only use "
                    "replace_module, set_mirror, set_style, or set_parameter. Reference only "
                    "supplied node and module IDs. Never change a locked/root node. Never provide "
                    "functional weapon engineering, mechanisms, ammunition, performance, machining, "
                    "assembly, or manufacturing instructions. Return only the requested JSON."
                ),
                user=_canonical_json(
                    {
                        "instruction": instruction,
                        "current_spec": current_spec.model_dump(mode="json"),
                        "base_graph": base_graph.model_dump(mode="json"),
                        "registered_modules": list(module_catalog),
                        "selected_node_id": selected_node_id,
                        "selected_module_id": selected_module_id,
                        "allowed_style_paths": [
                            "style.keywords",
                            "style.palette",
                            "style.detail_density",
                        ],
                        "allowed_parameter_paths": [
                            "proportions.overall_length_mm",
                            "proportions.body_height_mm",
                            "proportions.grip_angle_deg",
                        ],
                    }
                ),
            )
        )

    def _chat(
        self,
        *,
        schema_name: str,
        schema: dict[str, Any],
        system: str,
        user: str,
    ) -> dict[str, Any]:
        self.last_call_metrics = None
        if not self.config.api_key:
            raise ConceptPlannerError(
                "PLANNER_UNCONFIGURED", "Concept Planner API key is not configured."
            )
        if not self.config.model:
            raise ConceptPlannerError(
                "PLANNER_UNCONFIGURED", "Concept Planner model is not configured."
            )
        use_json_object = self._uses_json_object_mode()
        system_message = system
        if use_json_object:
            # DeepSeek V4's OpenAI-compatible endpoint supports JSON mode, not
            # OpenAI's json_schema response format. Place the complete schema
            # in the prompt so the model still has a precise contract.
            system_message = (
                f"{system}\n\nReturn exactly one JSON object and no Markdown. "
                f"The JSON object must validate against this JSON Schema:\n"
                f"{_canonical_json(schema)}"
            )
        payload = {
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system_message},
                {"role": "user", "content": user},
            ],
            "response_format": (
                {"type": "json_object"}
                if use_json_object
                else {
                    "type": "json_schema",
                    "json_schema": {"name": schema_name, "strict": True, "schema": schema},
                }
            ),
            "max_tokens": max(256, min(self.config.max_output_tokens, 16_384)),
        }
        if self._uses_deepseek_v4():
            payload["thinking"] = {"type": "enabled"}
            payload["reasoning_effort"] = "high"
        else:
            payload["temperature"] = 0.35
        request = urllib.request.Request(
            self.config.base_url.rstrip("/") + "/chat/completions",
            data=_canonical_json(payload).encode("utf-8"),
            method="POST",
        )
        request.add_header("Content-Type", "application/json")
        request.add_header("Authorization", f"Bearer {self.config.api_key}")
        started_at = time.perf_counter()
        try:
            with urllib.request.urlopen(
                request, timeout=self.config.timeout_seconds
            ) as response:
                data = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            self.last_call_metrics = PlannerCallMetrics(
                latency_ms=_elapsed_milliseconds(started_at)
            )
            if exc.code in {401, 403}:
                raise ConceptPlannerError(
                    "PLANNER_AUTH_FAILED", "Concept Planner rejected the API key.", False
                ) from exc
            if exc.code == 429:
                raise ConceptPlannerError(
                    "PLANNER_RATE_LIMITED", "Concept Planner rate limited the request."
                ) from exc
            raise ConceptPlannerError(
                "PLANNER_HTTP_ERROR", f"Concept Planner HTTP error {exc.code}."
            ) from exc
        except Exception as exc:  # noqa: BLE001 - provider failures cross this boundary.
            self.last_call_metrics = PlannerCallMetrics(
                latency_ms=_elapsed_milliseconds(started_at)
            )
            raise ConceptPlannerError(
                "PLANNER_TIMEOUT", "Concept Planner request failed."
            ) from exc
        self.last_call_metrics = _planner_call_metrics(
            data, latency_ms=_elapsed_milliseconds(started_at)
        )
        try:
            content = data["choices"][0]["message"]["content"]
            return json.loads(content)
        except Exception as exc:  # noqa: BLE001 - external response shape is untrusted.
            raise ConceptPlannerError(
                "PLANNER_BAD_OUTPUT", "Concept Planner did not return valid JSON."
            ) from exc

    def _uses_deepseek_v4(self) -> bool:
        return "api.deepseek.com" in self.config.base_url.casefold()

    def _uses_json_object_mode(self) -> bool:
        if self.config.response_mode == "json_object":
            return True
        if self.config.response_mode == "json_schema":
            return False
        return self._uses_deepseek_v4()


def concept_planner_from_env() -> ConceptPlannerProvider:
    selected = os.environ.get(
        "FORGECAD_CONCEPT_PLANNER_PROVIDER",
        os.environ.get("WUSHEN_LLM_PROVIDER", "deterministic_rules"),
    ).strip().lower()
    if selected == "openai_compatible":
        return OpenAICompatibleConceptPlanner(
            OpenAICompatibleConceptPlannerConfig(
                base_url=os.environ.get(
                    "FORGECAD_CONCEPT_PLANNER_BASE_URL",
                    os.environ.get(
                        "WUSHEN_LLM_BASE_URL",
                        os.environ.get("WUSHEN_OPENAI_BASE_URL", "https://api.openai.com/v1"),
                    ),
                ),
                model=os.environ.get(
                    "FORGECAD_CONCEPT_PLANNER_MODEL",
                    os.environ.get("WUSHEN_LLM_MODEL", os.environ.get("WUSHEN_OPENAI_MODEL", "")),
                ),
                api_key=_read_secret(
                    "FORGECAD_CONCEPT_PLANNER_API_KEY",
                    "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
                    fallback_value="WUSHEN_LLM_API_KEY",
                    fallback_file="WUSHEN_LLM_API_KEY_FILE",
                ),
                timeout_seconds=float(
                    os.environ.get("FORGECAD_CONCEPT_PLANNER_TIMEOUT_SECONDS", "60")
                ),
                response_mode=_planner_response_mode_from_env(),
                max_output_tokens=_planner_max_output_tokens_from_env(),
            )
        )
    return DeterministicConceptPlanner()


def _planner_response_mode_from_env() -> Literal["auto", "json_schema", "json_object"]:
    value = os.environ.get("FORGECAD_CONCEPT_PLANNER_RESPONSE_MODE", "auto").strip().lower()
    return value if value in {"auto", "json_schema", "json_object"} else "auto"


def _planner_max_output_tokens_from_env() -> int:
    try:
        return max(256, min(int(os.environ.get("FORGECAD_CONCEPT_PLANNER_MAX_TOKENS", "4096")), 16_384))
    except ValueError:
        return 4096


def planner_provenance(
    provider: ConceptPlannerProvider,
    *,
    input_payload: Any,
    output_payload: Any,
    registry_module_ids: Sequence[str],
    attempted_provider: Optional[ConceptPlannerProvider] = None,
    fallback_used: bool = False,
    warnings: Sequence[str] = (),
) -> ConceptPlannerProvenance:
    metrics = getattr(provider, "last_call_metrics", None)
    return ConceptPlannerProvenance(
        generator=(
            "openai_compatible"
            if provider.provider_type == "openai_compatible"
            else "deterministic_rules"
        ),
        provider_id=provider.provider_id,
        provider_type=provider.provider_type,
        model=provider.model_name,
        attempted_provider_id=(
            attempted_provider.provider_id if attempted_provider is not None else None
        ),
        attempted_provider_type=(
            attempted_provider.provider_type if attempted_provider is not None else None
        ),
        attempted_model=(
            attempted_provider.model_name if attempted_provider is not None else None
        ),
        fallback_used=fallback_used,
        input_sha256=_sha256_json(input_payload),
        output_sha256=_sha256_json(output_payload),
        registry_module_ids=list(registry_module_ids),
        warnings=list(warnings),
        latency_ms=metrics.latency_ms if metrics is not None else None,
        input_tokens=metrics.input_tokens if metrics is not None else None,
        output_tokens=metrics.output_tokens if metrics is not None else None,
        total_tokens=metrics.total_tokens if metrics is not None else None,
    )


def _planner_call_metrics(
    response_payload: Any, *, latency_ms: int
) -> PlannerCallMetrics:
    usage = response_payload.get("usage", {}) if isinstance(response_payload, dict) else {}
    input_tokens = _optional_nonnegative_int(
        usage.get("prompt_tokens", usage.get("input_tokens"))
    )
    output_tokens = _optional_nonnegative_int(
        usage.get("completion_tokens", usage.get("output_tokens"))
    )
    total_tokens = _optional_nonnegative_int(usage.get("total_tokens"))
    if total_tokens is None and input_tokens is not None and output_tokens is not None:
        total_tokens = input_tokens + output_tokens
    return PlannerCallMetrics(
        latency_ms=latency_ms,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        total_tokens=total_tokens,
    )


def _elapsed_milliseconds(started_at: float) -> int:
    return max(0, round((time.perf_counter() - started_at) * 1000))


def _optional_nonnegative_int(value: Any) -> Optional[int]:
    if isinstance(value, bool):
        return None
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        return None
    return parsed if parsed >= 0 else None


def _numeric_change(
    path: str, value: float, rationale: str
) -> ConceptChangeOperationPlan:
    return ConceptChangeOperationPlan(
        op="set_parameter",
        node_id=None,
        module_id=None,
        path=path,
        value=value,
        mirror_axis=None,
        rationale=rationale,
    )


def _extract_number(text: str, patterns: Sequence[str]) -> Optional[float]:
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE)
        if match:
            try:
                return float(match.group(1))
            except ValueError:
                return None
    return None


def _mentioned_category(text: str) -> Optional[str]:
    categories = (
        (("前部", "前端", "front"), "front_shell"),
        (("后部", "后端", "rear"), "rear_shell"),
        (("握持", "握把", "grip"), "grip_shell"),
        (("顶部", "top"), "top_accessory"),
        (("侧部", "侧板", "side"), "side_accessory"),
        (("下部", "lower"), "lower_structure"),
        (("储存", "能源", "storage"), "storage_visual"),
        (("装甲", "armor"), "armor_panel"),
    )
    for tokens, category in categories:
        if any(token in text for token in tokens):
            return category
    return None


def _recognized_palette(text: str) -> list[str]:
    color_tokens = (
        (("石墨", "graphite"), "graphite"),
        (("枪灰", "gunmetal"), "gunmetal"),
        (("红", "red"), "signal_red"),
        (("蓝", "blue"), "signal_blue"),
        (("白", "white"), "arctic_white"),
        (("黑", "black"), "black_metal"),
    )
    return [
        color
        for tokens, color in color_tokens
        if any(token in text for token in tokens)
    ]


def _unique_limited(values: Sequence[str], limit: int) -> list[str]:
    return list(dict.fromkeys(value for value in values if value))[:limit]


def _read_secret(
    value_name: str,
    file_name: str,
    *,
    fallback_value: str,
    fallback_file: str,
) -> Optional[str]:
    value = os.environ.get(value_name) or os.environ.get(fallback_value)
    if value:
        return value
    path = os.environ.get(file_name) or os.environ.get(fallback_file)
    if not path:
        return None
    try:
        return Path(path).read_text(encoding="utf-8").strip()
    except OSError:
        return None


def _sha256_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _canonical_json(value: Any) -> str:
    if isinstance(value, BaseModel):
        value = value.model_dump(mode="json")
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
