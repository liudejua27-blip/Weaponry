from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Literal, Optional, Protocol

from pydantic import Field

from .concept_models import StrictApiModel
from .conversation import ProviderConversation
from .domain_packs import DomainPackManifest
from .visual_intent import (
    VisualIntentMapping,
    build_visual_intent_mapping,
    visual_intent_description,
)


class MechanicalPlannerError(RuntimeError):
    def __init__(self, code: str, message: str, *, recoverable: bool = True) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


@dataclass(frozen=True)
class MechanicalPlannerTelemetry:
    """Redaction-safe usage facts from the most recent Provider call.

    The record deliberately excludes response text, model identifiers, base URLs
    and headers.  It is optional because some OpenAI-compatible providers do not
    return usage data.
    """

    latency_ms: int
    input_tokens: Optional[int] = None
    output_tokens: Optional[int] = None
    total_tokens: Optional[int] = None
    prompt_cache_hit_tokens: Optional[int] = None
    prompt_cache_miss_tokens: Optional[int] = None


class ConceptDirection(StrictApiModel):
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    title: str = Field(min_length=1, max_length=80)
    summary: str = Field(min_length=1, max_length=500)
    silhouette: Literal["compact", "balanced", "extended", "organic", "industrial"]
    primary_part_roles: List[str] = Field(min_length=2, max_length=16)
    material_direction: str = Field(min_length=1, max_length=160)


class MechanicalConceptPlan(StrictApiModel):
    schema_version: Literal["MechanicalConceptPlan@1"] = "MechanicalConceptPlan@1"
    plan_id: str = Field(pattern=r"^plan_[a-z0-9_\-]+$")
    domain_pack_id: str = Field(pattern=r"^pack_[a-z0-9_\-]+$")
    brief: str = Field(min_length=1, max_length=2000)
    generation_stage: Literal["blockout"] = "blockout"
    spec: Dict[str, Any]
    directions: List[ConceptDirection] = Field(min_length=3, max_length=3)
    provider_id: str = Field(min_length=1, max_length=120)
    model: Optional[str] = Field(default=None, max_length=160)
    shape_program_ready: bool = False


class MechanicalConceptPlanner(Protocol):
    provider_id: str
    model_name: Optional[str]
    last_call_telemetry: Optional[MechanicalPlannerTelemetry]

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
    ) -> MechanicalConceptPlan:
        ...


@dataclass(frozen=True)
class MechanicalPlannerConfig:
    base_url: str
    model: str
    api_key: Optional[str]
    timeout_seconds: float = 60.0
    response_mode: Literal["auto", "json_schema", "json_object"] = "auto"
    max_output_tokens: int = 4096


class DeterministicMechanicalPlanner:
    provider_id = "deterministic_mechanical_planner"
    model_name = None

    def __init__(self) -> None:
        self.last_call_telemetry: Optional[MechanicalPlannerTelemetry] = None

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
    ) -> MechanicalConceptPlan:
        del conversation
        roles = _roles_for_pack(pack.domain)
        direction_ids = [f"direction_{index}" for index in range(1, 4)]
        visual_intent = build_visual_intent_mapping(
            brief=brief,
            domain_pack_id=pack.pack_id,
            direction_ids=direction_ids,
        )
        directions = _directions_for_pack(pack.domain, roles, visual_intent)
        return MechanicalConceptPlan(
            plan_id=_new_plan_id(),
            domain_pack_id=pack.pack_id,
            brief=brief,
            spec=_draft_spec(brief=brief, pack=pack, roles=roles, project_id=project_id, visual_intent=visual_intent),
            directions=directions,
            provider_id=self.provider_id,
            model=None,
            shape_program_ready=False,
        )


class OpenAICompatibleMechanicalPlanner:
    provider_id = "openai_compatible_mechanical_planner"

    def __init__(self, config: MechanicalPlannerConfig) -> None:
        self.config = config
        self.model_name = config.model or None
        self.last_call_telemetry: Optional[MechanicalPlannerTelemetry] = None

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
    ) -> MechanicalConceptPlan:
        if not self.config.api_key:
            raise MechanicalPlannerError("PLANNER_UNCONFIGURED", "Mechanical Planner API key is not configured.", recoverable=False)
        if not self.config.model:
            raise MechanicalPlannerError("PLANNER_UNCONFIGURED", "Mechanical Planner model is not configured.", recoverable=False)
        schema = MechanicalConceptPlan.model_json_schema()
        stable_messages = [
            {
                "role": "system",
                "content": (
                    "You are ForgeCAD's general mechanical concept planner. Return exactly one JSON object "
                    "matching the supplied schema and exactly three complete exterior directions. The first "
                    "stage is a visual, non-functional blockout for a future prop, vehicle, aircraft, or "
                    "robotic arm. Describe whole-object silhouette, primary part roles, and visual materials. "
                    "Do not provide real-world weapon engineering, manufacturing, dimensions for fabrication, "
                    "performance, flight safety, load, torque, control code, or assembly instructions."
                ),
            },
            {
                "role": "system",
                "content": _canonical_json(
                    {
                        "schema_version": "ForgeCADProviderStaticContract@1",
                        "domain_pack": pack.model_dump(mode="json"),
                        "output_schema": schema,
                    }
                ),
            },
        ]
        dynamic_messages = list(conversation.messages) if conversation is not None else [
            {
                "role": "user",
                "content": _canonical_json(
                    {
                        "schema_version": "ForgeCADTurnRequest@1",
                        "project_id": project_id,
                        "request": brief,
                    }
                ),
            }
        ]
        payload = {
            "model": self.config.model,
            "messages": [*stable_messages, *dynamic_messages],
            "response_format": _response_format(self.config, schema),
            "max_tokens": max(512, min(
                conversation.max_output_tokens if conversation is not None else self.config.max_output_tokens,
                self.config.max_output_tokens,
                16_384,
            )),
        }
        if "api.deepseek.com" in self.config.base_url.casefold():
            thinking_enabled = conversation.thinking_enabled if conversation is not None else True
            payload["thinking"] = {"type": "enabled" if thinking_enabled else "disabled"}
            if thinking_enabled:
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
        started_at = time.monotonic()
        try:
            with urllib.request.urlopen(request, timeout=self.config.timeout_seconds) as response:
                response_payload = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=_elapsed_ms(started_at))
            if exc.code in {401, 403}:
                raise MechanicalPlannerError("PLANNER_AUTH_FAILED", "Mechanical Planner rejected the API key.", recoverable=False) from exc
            if exc.code == 429:
                raise MechanicalPlannerError("PLANNER_RATE_LIMITED", "Mechanical Planner rate limited the request.") from exc
            raise MechanicalPlannerError("PLANNER_HTTP_ERROR", f"Mechanical Planner HTTP error {exc.code}.") from exc
        except Exception as exc:  # noqa: BLE001 - provider failures cross a stable boundary.
            self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=_elapsed_ms(started_at))
            raise MechanicalPlannerError("PLANNER_TIMEOUT", "Mechanical Planner request failed.") from exc
        self.last_call_telemetry = _provider_telemetry(response_payload, _elapsed_ms(started_at))
        try:
            message = response_payload["choices"][0]["message"]
            if message.get("tool_calls"):
                raise MechanicalPlannerError(
                    "PLANNER_TOOL_CALLS_UNSUPPORTED",
                    "Mechanical Planner returned tool calls, which are not enabled for this local Alpha.",
                    recoverable=False,
                )
            content = message["content"]
            result = MechanicalConceptPlan.model_validate(json.loads(content))
        except Exception as exc:  # noqa: BLE001 - external output is untrusted.
            raise MechanicalPlannerError("PLANNER_BAD_OUTPUT", "Mechanical Planner returned invalid structured output.", recoverable=False) from exc
        visual_intent = build_visual_intent_mapping(
            brief=brief,
            domain_pack_id=pack.pack_id,
            direction_ids=[direction.direction_id for direction in result.directions],
        )
        intent_by_direction = {item.direction_id: item for item in visual_intent.directions}
        normalized_directions = [
            direction.model_copy(
                update={
                    "silhouette": intent_by_direction[direction.direction_id].silhouette,
                    "material_direction": visual_intent_description(intent_by_direction[direction.direction_id]),
                }
            )
            for direction in result.directions
        ]
        return result.model_copy(
            update={
                "domain_pack_id": pack.pack_id,
                "brief": brief,
                "spec": _draft_spec(
                    brief=brief,
                    pack=pack,
                    roles=_roles_for_pack(pack.domain),
                    project_id=project_id,
                    visual_intent=visual_intent,
                ),
                "directions": normalized_directions,
                "provider_id": self.provider_id,
                "model": self.model_name,
                "shape_program_ready": False,
            }
        )


def mechanical_planner_from_env() -> MechanicalConceptPlanner:
    selected = os.environ.get(
        "FORGECAD_AGENT_PROVIDER",
        os.environ.get("FORGECAD_CONCEPT_PLANNER_PROVIDER", "deterministic_rules"),
    ).strip().lower()
    if selected != "openai_compatible":
        return DeterministicMechanicalPlanner()
    return OpenAICompatibleMechanicalPlanner(
        MechanicalPlannerConfig(
            base_url=os.environ.get(
                "FORGECAD_AGENT_BASE_URL",
                os.environ.get("FORGECAD_CONCEPT_PLANNER_BASE_URL", "https://api.openai.com/v1"),
            ),
            model=os.environ.get(
                "FORGECAD_AGENT_MODEL",
                os.environ.get("FORGECAD_CONCEPT_PLANNER_MODEL", ""),
            ),
            api_key=_read_secret("FORGECAD_AGENT_API_KEY", "FORGECAD_AGENT_API_KEY_FILE")
            or _read_secret("FORGECAD_CONCEPT_PLANNER_API_KEY", "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE")
            or _read_secret("WUSHEN_LLM_API_KEY", "WUSHEN_LLM_API_KEY_FILE"),
            timeout_seconds=_float_env("FORGECAD_AGENT_TIMEOUT_SECONDS", 60.0),
            response_mode=_response_mode_env(),
            max_output_tokens=_int_env("FORGECAD_AGENT_MAX_TOKENS", 4096),
        )
    )


def _roles_for_pack(domain: str) -> List[str]:
    return {
        "future_weapon_prop": ["primary_body", "secondary_body", "mobility", "trim", "transparent"],
        "vehicle_concept": ["body_shell", "cabin", "wheel_or_track", "lighting", "trim_panel"],
        "aircraft_concept": ["fuselage", "cockpit_canopy", "main_wing", "tail_surface", "nacelle"],
        "robotic_arm_concept": ["base", "shoulder_joint", "upper_link", "elbow_joint", "forearm_link", "end_effector"],
    }.get(domain, ["primary_body", "secondary_body", "trim"])


def _directions_for_pack(domain: str, roles: List[str], visual_intent: VisualIntentMapping) -> List[ConceptDirection]:
    names = {
        "future_weapon_prop": [("紧凑轮廓", "压低主体与附件，形成易理解的完整展示轮廓。", "compact"), ("延展陈列", "拉长主轴并强调前后层次，形成展陈比例。", "extended"), ("模块均衡", "平衡主体、护罩和装饰模块，形成工业化外观。", "industrial")],
        "vehicle_concept": [("探索舱体", "抬高座舱并强调轮拱与防护层，形成冰原探索姿态。", "industrial"), ("城市流线", "压低车身并拉顺前后体块，形成紧凑城市轮廓。", "balanced"), ("重载平台", "放大车身和下部支撑，形成稳定的运输外观。", "extended")],
        "aircraft_concept": [("垂直起降", "集中机身、旋翼舱和尾部稳定面，形成完整飞行器轮廓。", "balanced"), ("高速单座", "收窄机身并延展翼面，形成轻快的高速概念。", "extended"), ("宽体运输", "放大中央舱体和机翼根部，形成厚实运输外观。", "industrial")],
        "robotic_arm_concept": [("精密桌面", "缩短各段连杆并突出关节护罩，形成清晰的桌面设备外观。", "compact"), ("长臂维护", "延展上臂与前臂并拉开运动链，形成维护机构轮廓。", "extended"), ("双工具服务", "强调腕部和末端工具分件，形成可继续替换的服务机构。", "industrial")],
    }.get(domain, [("紧凑轮廓", "建立清楚的完整机械外观。", "compact"), ("均衡结构", "平衡主体与附件比例。", "balanced"), ("延展展示", "拉开主轴和层次。", "extended")])
    return [
        ConceptDirection(
            direction_id=f"direction_{index + 1}",
            title=title,
            summary=f"{summary} 这版采用{visual_intent_description(visual_intent.directions[index])}的完整外观方向。",
            silhouette=visual_intent.directions[index].silhouette,
            primary_part_roles=roles[: min(len(roles), 6)],
            material_direction=visual_intent_description(visual_intent.directions[index]),
        )
        for index, (title, summary, _silhouette) in enumerate(names)
    ]


def _draft_spec(
    *,
    brief: str,
    pack: DomainPackManifest,
    roles: List[str],
    project_id: Optional[str],
    visual_intent: VisualIntentMapping,
) -> Dict[str, Any]:
    primary_intent = visual_intent.directions[0]
    return {
        "schema_version": "MechanicalConceptSpec@1",
        "concept_id": f"asset_plan_{uuid.uuid4().hex[:12]}",
        "project_id": project_id or "prj_unbound_agent_session",
        "domain_pack_id": pack.pack_id,
        "brief": brief,
        "design_language": {
            "keywords": [pack.domain, "完整外观", "非功能展示"],
            "silhouette": primary_intent.silhouette,
            "detail_density": primary_intent.detail_density,
            "color_direction": visual_intent_description(primary_intent),
        },
        "visual_intent_mapping": visual_intent.model_dump(mode="json"),
        "envelope": {"min_mm": [0, 0, 0], "max_mm": [2400, 1800, 1800]},
        "pose": {"position": [0, 0, 0], "rotation": [0, 0, 0]},
        "full_look": {
            "completeness": "full_exterior",
            "generation_stage": "blockout",
            "primary_part_roles": roles[: min(len(roles), 8)],
            "preview_views": ["perspective", "front", "side", "top"],
        },
        "material_intents": [
            {"zone_role": "primary_shell", "material_preset_id": pack.material_preset_ids[0]},
        ],
        "non_functional_only": True,
    }


def _response_format(config: MechanicalPlannerConfig, schema: Dict[str, Any]) -> Dict[str, Any]:
    use_json_object = config.response_mode == "json_object" or (
        config.response_mode == "auto" and "api.deepseek.com" in config.base_url.casefold()
    )
    if use_json_object:
        return {"type": "json_object"}
    return {"type": "json_schema", "json_schema": {"name": "forgecad_mechanical_concept_plan", "strict": True, "schema": schema}}


def _read_secret(value_name: str, file_name: str) -> Optional[str]:
    value = os.environ.get(value_name)
    if value:
        return value
    path = os.environ.get(file_name)
    if not path:
        return None
    try:
        return Path(path).read_text(encoding="utf-8").strip() or None
    except OSError:
        return None


def _float_env(name: str, fallback: float) -> float:
    try:
        return max(1.0, min(float(os.environ.get(name, str(fallback))), 300.0))
    except ValueError:
        return fallback


def _int_env(name: str, fallback: int) -> int:
    try:
        return max(512, min(int(os.environ.get(name, str(fallback))), 16_384))
    except ValueError:
        return fallback


def _elapsed_ms(started_at: float) -> int:
    return max(0, int((time.monotonic() - started_at) * 1000))


def _provider_telemetry(response_payload: Any, latency_ms: int) -> MechanicalPlannerTelemetry:
    usage = response_payload.get("usage") if isinstance(response_payload, dict) else None
    if not isinstance(usage, dict):
        return MechanicalPlannerTelemetry(latency_ms=latency_ms)

    def token_value(name: str) -> Optional[int]:
        value = usage.get(name)
        return value if isinstance(value, int) and value >= 0 else None

    return MechanicalPlannerTelemetry(
        latency_ms=latency_ms,
        input_tokens=token_value("prompt_tokens"),
        output_tokens=token_value("completion_tokens"),
        total_tokens=token_value("total_tokens"),
        prompt_cache_hit_tokens=token_value("prompt_cache_hit_tokens"),
        prompt_cache_miss_tokens=token_value("prompt_cache_miss_tokens"),
    )


def _response_mode_env() -> Literal["auto", "json_schema", "json_object"]:
    value = os.environ.get("FORGECAD_AGENT_RESPONSE_MODE", "auto").strip().lower()
    return value if value in {"auto", "json_schema", "json_object"} else "auto"  # type: ignore[return-value]


def _new_plan_id() -> str:
    return f"plan_{uuid.uuid4().hex}"


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
