from __future__ import annotations

import hashlib
import json
import os
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, Optional, Protocol, Sequence

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


class _StrictPlannerModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class ConceptBriefPatch(_StrictPlannerModel):
    keywords: list[str] = Field(min_length=1, max_length=12)
    palette: list[str] = Field(min_length=1, max_length=8)
    detail_density: float = Field(ge=0, le=1)
    overall_length_mm: float = Field(gt=0, le=1000)
    body_height_mm: float = Field(gt=0, le=1000)
    grip_angle_deg: float = Field(ge=-45, le=45)
    symmetry: Literal["symmetric", "mostly_symmetric", "asymmetric"]


class ConceptVariantPlan(_StrictPlannerModel):
    rank: int = Field(ge=1, le=3)
    name: str = Field(min_length=1, max_length=120)
    summary: str = Field(min_length=1, max_length=500)
    target_node_id: str = Field(min_length=1)
    scale: list[float] = Field(min_length=3, max_length=3)
    recommended_module_ids: list[str] = Field(min_length=1, max_length=12)
    rationale: list[str] = Field(min_length=1, max_length=12)

    @model_validator(mode="after")
    def validate_scale(self) -> "ConceptVariantPlan":
        if any(value < 0.85 or value > 1.15 for value in self.scale):
            raise ValueError("variant scale values must stay inside [0.85, 1.15]")
        if len(set(self.recommended_module_ids)) != len(self.recommended_module_ids):
            raise ValueError("recommended module ids must be unique")
        return self


class ConceptVariantPlanBatch(_StrictPlannerModel):
    variants: list[ConceptVariantPlan] = Field(min_length=3, max_length=3)

    @model_validator(mode="after")
    def validate_ranks(self) -> "ConceptVariantPlanBatch":
        if sorted(item.rank for item in self.variants) != [1, 2, 3]:
            raise ValueError("variant ranks must be exactly 1, 2, 3")
        return self


class DeterministicConceptPlanner:
    provider_id = "deterministic_concept_rules"
    provider_type: Literal["deterministic"] = "deterministic"
    model_name: Optional[str] = None

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
        choices = (
            ("A · 紧凑轮廓", "压缩主要可编辑模块，强调紧凑轮廓。", [0.9, 0.96, 0.96]),
            ("B · 低矮均衡", "降低次要轴向比例，形成与基准不同的低矮方案。", [1.0, 0.94, 1.0]),
            ("C · 延展展示", "延展主要可编辑模块，强调展示张力。", [1.1, 1.04, 1.04]),
        )
        plans: list[ConceptVariantPlan] = []
        for index, (name, summary, scale) in enumerate(choices):
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
                    recommended_module_ids=recommended,
                    rationale=[
                        f"只修改未锁定节点 {target.node_id}",
                        "模块建议来自当前项目 Profile 对应的注册表",
                        "保持 Connector Graph 与 root 节点不变",
                    ],
                )
            )
        return plans


@dataclass(frozen=True)
class OpenAICompatibleConceptPlannerConfig:
    base_url: str
    model: str
    api_key: Optional[str]
    timeout_seconds: float = 60.0


class OpenAICompatibleConceptPlanner:
    provider_id = "openai_compatible_concept_planner"
    provider_type: Literal["openai_compatible"] = "openai_compatible"

    def __init__(self, config: OpenAICompatibleConceptPlannerConfig) -> None:
        self.config = config
        self.model_name = config.model or None

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
                    "Use scale only inside 0.85 to 1.15. Never provide functional weapon or manufacturing instructions."
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

    def _chat(
        self,
        *,
        schema_name: str,
        schema: dict[str, Any],
        system: str,
        user: str,
    ) -> dict[str, Any]:
        if not self.config.api_key:
            raise ConceptPlannerError(
                "PLANNER_UNCONFIGURED", "Concept Planner API key is not configured."
            )
        if not self.config.model:
            raise ConceptPlannerError(
                "PLANNER_UNCONFIGURED", "Concept Planner model is not configured."
            )
        payload = {
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": schema_name, "strict": True, "schema": schema},
            },
            "temperature": 0.35,
        }
        request = urllib.request.Request(
            self.config.base_url.rstrip("/") + "/chat/completions",
            data=_canonical_json(payload).encode("utf-8"),
            method="POST",
        )
        request.add_header("Content-Type", "application/json")
        request.add_header("Authorization", f"Bearer {self.config.api_key}")
        try:
            with urllib.request.urlopen(
                request, timeout=self.config.timeout_seconds
            ) as response:
                data = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
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
            raise ConceptPlannerError(
                "PLANNER_TIMEOUT", "Concept Planner request failed."
            ) from exc
        try:
            content = data["choices"][0]["message"]["content"]
            return json.loads(content)
        except Exception as exc:  # noqa: BLE001 - external response shape is untrusted.
            raise ConceptPlannerError(
                "PLANNER_BAD_OUTPUT", "Concept Planner did not return valid JSON."
            ) from exc


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
            )
        )
    return DeterministicConceptPlanner()


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
    )


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
