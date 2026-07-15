from __future__ import annotations

import json
import os
import socket
import threading
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, List, Literal, Optional, Protocol

from pydantic import Field, ValidationError

from .concept_models import StrictApiModel
from .conversation import ProviderConversation
from .domain_packs import DomainPackManifest
from .provider_gateway import ProviderConnectionState, ProviderExecutionTrace
from .visual_intent import (
    VisualIntentMapping,
    build_visual_intent_mapping,
    visual_intent_description,
)


class MechanicalPlannerError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        recoverable: bool = True,
        network_call_made: bool = False,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable
        self.network_call_made = network_call_made


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
    last_execution_trace: Optional[ProviderExecutionTrace]

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
        cancel_event: Optional[threading.Event] = None,
        trace_observer: Optional[Callable[[ProviderExecutionTrace], None]] = None,
    ) -> MechanicalConceptPlan:
        ...

    def connection_state(self) -> ProviderConnectionState:
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
        self.last_execution_trace: Optional[ProviderExecutionTrace] = None

    def connection_state(self) -> ProviderConnectionState:
        return ProviderConnectionState(
            status="unconfigured",
            provider_id=self.provider_id,
            configured=False,
            metadata_status="not_checked",
            secret_status="not_checked",
            supervisor_status="not_checked",
            capability_status="offline",
            message="当前使用本机离线规划；不会发起 DeepSeek 网络请求。",
        )

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
        cancel_event: Optional[threading.Event] = None,
        trace_observer: Optional[Callable[[ProviderExecutionTrace], None]] = None,
    ) -> MechanicalConceptPlan:
        del conversation
        if cancel_event is not None and cancel_event.is_set():
            raise MechanicalPlannerError("PROVIDER_CANCELLED", "Provider request was cancelled.", recoverable=False)
        trace = ProviderExecutionTrace.new(
            phase="completed",
            provider_id=self.provider_id,
            network_call_made=False,
            message="本机离线规划已完成；未调用外部 Provider。",
        )
        self.last_execution_trace = trace
        _notify_trace(trace_observer, trace)
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
        self.last_execution_trace: Optional[ProviderExecutionTrace] = None

    def connection_state(self) -> ProviderConnectionState:
        configured = bool(self.config.api_key and self.config.model and self.config.base_url)
        return ProviderConnectionState(
            status="ready" if configured else "unconfigured",
            provider_id=self.provider_id,
            configured=configured,
            metadata_status="valid" if self.config.model and self.config.base_url else "missing",
            secret_status="available" if self.config.api_key else "missing",
            supervisor_status="not_checked",
            capability_status="ready" if configured else "unavailable",
            failure_code=None if configured else "PROVIDER_UNCONFIGURED",
            message="Provider capability is ready for an explicit request." if configured else "Provider metadata or secret is missing.",
        )

    def plan_complete_concept(
        self,
        *,
        brief: str,
        pack: DomainPackManifest,
        project_id: Optional[str],
        conversation: Optional[ProviderConversation] = None,
        cancel_event: Optional[threading.Event] = None,
        trace_observer: Optional[Callable[[ProviderExecutionTrace], None]] = None,
    ) -> MechanicalConceptPlan:
        trace_id = f"ptrace_{uuid.uuid4().hex}"
        started_at = time.monotonic()
        self.last_call_telemetry = None
        self.last_execution_trace = None
        if not self.config.api_key:
            self._fail_trace(
                trace_id,
                "PROVIDER_UNCONFIGURED",
                "Provider API key is not configured.",
                started_at,
                False,
                trace_observer,
            )
            raise MechanicalPlannerError("PROVIDER_UNCONFIGURED", "Provider API key is not configured.", recoverable=False)
        if not self.config.model:
            self._fail_trace(
                trace_id,
                "PROVIDER_UNCONFIGURED",
                "Provider model is not configured.",
                started_at,
                False,
                trace_observer,
            )
            raise MechanicalPlannerError("PROVIDER_UNCONFIGURED", "Provider model is not configured.", recoverable=False)
        if cancel_event is not None and cancel_event.is_set():
            self._cancel_trace(trace_id, started_at, False, trace_observer)
            raise MechanicalPlannerError("PROVIDER_CANCELLED", "Provider request was cancelled.", recoverable=False)
        preflight = ProviderExecutionTrace.new(
            trace_id=trace_id,
            phase="preflight",
            provider_id=self.provider_id,
            network_call_made=False,
            message="Provider metadata and in-memory secret passed preflight.",
        )
        self.last_execution_trace = preflight
        _notify_trace(trace_observer, preflight)
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
                        "versioned_json_output_example": _provider_output_example(pack),
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
            "stream": True,
            "stream_options": {"include_usage": True},
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
        request.add_header("Accept", "text/event-stream, application/json")
        request.add_header("Authorization", f"Bearer {self.config.api_key}")
        request_started = ProviderExecutionTrace.new(
            trace_id=trace_id,
            phase="request_started",
            provider_id=self.provider_id,
            network_call_made=True,
            message="Provider network request started.",
        )
        self.last_execution_trace = request_started
        _notify_trace(trace_observer, request_started)
        try:
            with urllib.request.urlopen(request, timeout=self.config.timeout_seconds) as response:
                response_payload = _read_provider_response(
                    response,
                    cancel_event=cancel_event,
                    on_streaming=lambda: self._streaming_trace(trace_id, started_at, trace_observer),
                )
        except urllib.error.HTTPError as exc:
            self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=_elapsed_ms(started_at))
            code, message, recoverable = _http_error(exc.code)
            self._fail_trace(trace_id, code, message, started_at, True, trace_observer)
            raise MechanicalPlannerError(code, message, recoverable=recoverable, network_call_made=True) from exc
        except MechanicalPlannerError as exc:
            if exc.code == "PROVIDER_CANCELLED":
                self._cancel_trace(trace_id, started_at, True, trace_observer)
            else:
                self._fail_trace(trace_id, exc.code, str(exc), started_at, True, trace_observer)
            raise
        except (socket.timeout, TimeoutError) as exc:
            self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=_elapsed_ms(started_at))
            code = "PROVIDER_TIMEOUT"
            message = "Provider request timed out; its outcome is unknown."
            self._fail_trace(trace_id, code, message, started_at, True, trace_observer)
            raise MechanicalPlannerError(code, message, network_call_made=True) from exc
        except (urllib.error.URLError, OSError) as exc:
            self.last_call_telemetry = MechanicalPlannerTelemetry(latency_ms=_elapsed_ms(started_at))
            code = "PROVIDER_NETWORK_ERROR"
            message = "Provider network request failed."
            self._fail_trace(trace_id, code, message, started_at, True, trace_observer)
            raise MechanicalPlannerError(code, message, network_call_made=True) from exc
        self.last_call_telemetry = _provider_telemetry(response_payload, _elapsed_ms(started_at))
        validating = _trace_from_telemetry(
            trace_id=trace_id,
            phase="validating",
            provider_id=self.provider_id,
            telemetry=self.last_call_telemetry,
            network_call_made=True,
            message="Provider stream completed; validating structured JSON output.",
        )
        self.last_execution_trace = validating
        _notify_trace(trace_observer, validating)
        try:
            message = response_payload["choices"][0]["message"]
            if message.get("tool_calls"):
                raise MechanicalPlannerError(
                    "PROVIDER_TOOL_CALLS_UNSUPPORTED",
                    "Provider returned tool calls, which are not enabled for this local Alpha.",
                    recoverable=False,
                    network_call_made=True,
                )
            content = message.get("content")
            if not isinstance(content, str) or not content.strip():
                raise MechanicalPlannerError(
                    "PROVIDER_EMPTY_CONTENT",
                    "Provider returned empty JSON content.",
                    recoverable=False,
                    network_call_made=True,
                )
            try:
                decoded = json.loads(content)
            except json.JSONDecodeError as exc:
                raise MechanicalPlannerError(
                    "PROVIDER_INVALID_JSON",
                    "Provider returned invalid JSON content.",
                    recoverable=False,
                    network_call_made=True,
                ) from exc
            try:
                result = MechanicalConceptPlan.model_validate(decoded)
            except ValidationError as exc:
                raise MechanicalPlannerError(
                    "PROVIDER_SCHEMA_MISMATCH",
                    "Provider JSON did not match MechanicalConceptPlan@1.",
                    recoverable=False,
                    network_call_made=True,
                ) from exc
        except MechanicalPlannerError as exc:
            self._fail_trace(trace_id, exc.code, str(exc), started_at, True, trace_observer)
            raise
        except (KeyError, IndexError, TypeError) as exc:
            code = "PROVIDER_SCHEMA_MISMATCH"
            message = "Provider response envelope did not contain a completion message."
            self._fail_trace(trace_id, code, message, started_at, True, trace_observer)
            raise MechanicalPlannerError(code, message, recoverable=False, network_call_made=True) from exc
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
        normalized = result.model_copy(
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
        completed = _trace_from_telemetry(
            trace_id=trace_id,
            phase="completed",
            provider_id=self.provider_id,
            telemetry=self.last_call_telemetry,
            network_call_made=True,
            message="Provider output passed MechanicalConceptPlan@1 validation.",
        )
        self.last_execution_trace = completed
        _notify_trace(trace_observer, completed)
        return normalized

    def _streaming_trace(
        self,
        trace_id: str,
        started_at: float,
        observer: Optional[Callable[[ProviderExecutionTrace], None]],
    ) -> None:
        if self.last_execution_trace is not None and self.last_execution_trace.phase == "streaming":
            return
        trace = ProviderExecutionTrace.new(
            trace_id=trace_id,
            phase="streaming",
            provider_id=self.provider_id,
            network_call_made=True,
            latency_ms=_elapsed_ms(started_at),
            message="Provider response stream started.",
        )
        self.last_execution_trace = trace
        _notify_trace(observer, trace)

    def _fail_trace(
        self,
        trace_id: str,
        code: str,
        message: str,
        started_at: float,
        network_call_made: bool,
        observer: Optional[Callable[[ProviderExecutionTrace], None]],
    ) -> None:
        trace = _trace_from_telemetry(
            trace_id=trace_id,
            phase="failed",
            provider_id=self.provider_id,
            telemetry=self.last_call_telemetry,
            network_call_made=network_call_made,
            message=message,
            error_code=code,
            fallback_latency_ms=_elapsed_ms(started_at),
        )
        self.last_execution_trace = trace
        _notify_trace(observer, trace)

    def _cancel_trace(
        self,
        trace_id: str,
        started_at: float,
        network_call_made: bool,
        observer: Optional[Callable[[ProviderExecutionTrace], None]],
    ) -> None:
        trace = ProviderExecutionTrace.new(
            trace_id=trace_id,
            phase="cancelled",
            provider_id=self.provider_id,
            network_call_made=network_call_made,
            latency_ms=_elapsed_ms(started_at),
            error_code="PROVIDER_CANCELLED",
            message="Provider request was cancelled; saved assets were not changed.",
        )
        self.last_execution_trace = trace
        _notify_trace(observer, trace)


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


def planner_connection_state(planner: MechanicalConceptPlanner) -> ProviderConnectionState:
    connection_state = getattr(planner, "connection_state", None)
    if callable(connection_state):
        return connection_state()
    provider_id = str(getattr(planner, "provider_id", "unknown"))
    return ProviderConnectionState(
        status="degraded",
        provider_id=provider_id,
        configured=False,
        metadata_status="not_checked",
        secret_status="not_checked",
        supervisor_status="not_checked",
        capability_status="unavailable",
        failure_code="PROVIDER_CAPABILITY_UNAVAILABLE",
        message="Planner does not expose ProviderConnectionState@1.",
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


def _provider_output_example(pack: DomainPackManifest) -> Dict[str, Any]:
    roles = _roles_for_pack(pack.domain)
    return {
        "schema_version": "MechanicalConceptPlan@1",
        "plan_id": "plan_example_contract_v1",
        "domain_pack_id": pack.pack_id,
        "brief": "JSON example only",
        "generation_stage": "blockout",
        "spec": {"non_functional_only": True},
        "directions": [
            {
                "direction_id": f"direction_{index}",
                "title": f"完整外观方向 {index}",
                "summary": "描述完整对象轮廓、主要分件和非功能视觉语言。",
                "silhouette": silhouette,
                "primary_part_roles": roles[: max(2, min(6, len(roles)))],
                "material_direction": "描述视觉材质与配色，不提供工程材料配方。",
            }
            for index, silhouette in enumerate(("compact", "balanced", "industrial"), start=1)
        ],
        "provider_id": "provider_contract_example",
        "model": None,
        "shape_program_ready": False,
    }


def _read_provider_response(
    response: Any,
    *,
    cancel_event: Optional[threading.Event],
    on_streaming: Callable[[], None],
) -> Dict[str, Any]:
    content_type = str(response.headers.get("Content-Type", "")).casefold()
    if "text/event-stream" not in content_type:
        if cancel_event is not None and cancel_event.is_set():
            raise MechanicalPlannerError(
                "PROVIDER_CANCELLED",
                "Provider request was cancelled.",
                recoverable=False,
                network_call_made=True,
            )
        raw = response.read().decode("utf-8")
        if cancel_event is not None and cancel_event.is_set():
            raise MechanicalPlannerError(
                "PROVIDER_CANCELLED",
                "Provider request was cancelled.",
                recoverable=False,
                network_call_made=True,
            )
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise MechanicalPlannerError(
                "PROVIDER_INVALID_JSON",
                "Provider returned an invalid JSON response envelope.",
                recoverable=False,
                network_call_made=True,
            ) from exc
        if not isinstance(value, dict):
            raise MechanicalPlannerError(
                "PROVIDER_SCHEMA_MISMATCH",
                "Provider response envelope must be a JSON object.",
                recoverable=False,
                network_call_made=True,
            )
        return value

    content_chunks: List[str] = []
    usage: Optional[Dict[str, Any]] = None
    tool_calls: List[Any] = []
    stream_started = False
    for raw_line in response:
        if cancel_event is not None and cancel_event.is_set():
            raise MechanicalPlannerError(
                "PROVIDER_CANCELLED",
                "Provider request was cancelled.",
                recoverable=False,
                network_call_made=True,
            )
        line = raw_line.decode("utf-8").strip()
        if not line or line.startswith(":") or not line.startswith("data:"):
            continue
        data = line[5:].strip()
        if data == "[DONE]":
            break
        try:
            chunk = json.loads(data)
        except json.JSONDecodeError as exc:
            raise MechanicalPlannerError(
                "PROVIDER_INVALID_JSON",
                "Provider stream contained invalid JSON.",
                recoverable=False,
                network_call_made=True,
            ) from exc
        if not isinstance(chunk, dict):
            continue
        if not stream_started:
            stream_started = True
            on_streaming()
        chunk_usage = chunk.get("usage")
        if isinstance(chunk_usage, dict):
            usage = chunk_usage
        choices = chunk.get("choices")
        if not isinstance(choices, list):
            continue
        for choice in choices:
            if not isinstance(choice, dict):
                continue
            delta = choice.get("delta")
            if not isinstance(delta, dict):
                continue
            content = delta.get("content")
            if isinstance(content, str):
                content_chunks.append(content)
            delta_calls = delta.get("tool_calls")
            if isinstance(delta_calls, list):
                tool_calls.extend(delta_calls)
            # reasoning_content is intentionally ignored and never persisted.
    return {
        "choices": [{"message": {"content": "".join(content_chunks), "tool_calls": tool_calls}}],
        "usage": usage or {},
    }


def _http_error(status_code: int) -> tuple[str, str, bool]:
    mapping = {
        400: ("DEEPSEEK_INVALID_REQUEST", "DeepSeek rejected the request format.", False),
        401: ("DEEPSEEK_AUTH_FAILED", "DeepSeek rejected the API key.", False),
        403: ("DEEPSEEK_AUTH_FAILED", "DeepSeek rejected the API key or access policy.", False),
        402: ("DEEPSEEK_BALANCE_EXHAUSTED", "DeepSeek account balance is insufficient.", False),
        422: ("DEEPSEEK_INVALID_PARAMETERS", "DeepSeek rejected one or more request parameters.", False),
        429: ("DEEPSEEK_RATE_LIMITED", "DeepSeek rate limited the request.", True),
        500: ("DEEPSEEK_SERVER_ERROR", "DeepSeek reported an internal server error.", True),
        503: ("DEEPSEEK_SERVER_BUSY", "DeepSeek is temporarily busy.", True),
    }
    return mapping.get(
        status_code,
        ("PROVIDER_HTTP_ERROR", f"Provider returned HTTP {status_code}.", status_code >= 500),
    )


def _trace_from_telemetry(
    *,
    trace_id: str,
    phase: Literal["validating", "completed", "failed"],
    provider_id: str,
    telemetry: Optional[MechanicalPlannerTelemetry],
    network_call_made: bool,
    message: str,
    error_code: Optional[str] = None,
    fallback_latency_ms: int = 0,
) -> ProviderExecutionTrace:
    return ProviderExecutionTrace.new(
        trace_id=trace_id,
        phase=phase,
        provider_id=provider_id,
        network_call_made=network_call_made,
        latency_ms=telemetry.latency_ms if telemetry is not None else fallback_latency_ms,
        input_tokens=telemetry.input_tokens if telemetry is not None else None,
        output_tokens=telemetry.output_tokens if telemetry is not None else None,
        total_tokens=telemetry.total_tokens if telemetry is not None else None,
        prompt_cache_hit_tokens=telemetry.prompt_cache_hit_tokens if telemetry is not None else None,
        prompt_cache_miss_tokens=telemetry.prompt_cache_miss_tokens if telemetry is not None else None,
        error_code=error_code,
        message=message,
    )


def _notify_trace(
    observer: Optional[Callable[[ProviderExecutionTrace], None]],
    trace: ProviderExecutionTrace,
) -> None:
    if observer is None:
        return
    try:
        observer(trace)
    except Exception:  # noqa: BLE001 - telemetry must never change Provider outcome.
        return


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
