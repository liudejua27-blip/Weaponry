from __future__ import annotations

import hashlib
import base64
import inspect
import json
import sqlite3
import threading
import uuid
from datetime import datetime, timezone
from typing import Any, Mapping, Optional

from forgecad_agent.application.agent_models import (
    AgentApproval,
    AgentApprovalResolution,
    AgentEvent,
    AgentItem,
    AgentThreadDetail,
    AgentThreadListResponse,
    AgentThreadSummary,
    AgentTurn,
    AgentProviderCheckResponse,
    CreateAgentApprovalRequest,
    CreateAgentThreadRequest,
    ResolveAgentApprovalRequest,
    StartAgentTurnRequest,
    BuildAgentBlockoutRequest,
    BuildAgentBlockoutResponse,
    RenderAgentBlockoutConceptPreviewRequest,
    AgentBlockoutConceptPreview,
    SegmentAgentBlockoutRequest,
    SegmentAgentBlockoutResponse,
    BlockoutPartCandidate,
)
from forgecad_agent.application.domain_inference import infer_domain
from forgecad_agent.application.domain_packs import DomainPackId, domain_pack_by_id, list_domain_packs
from forgecad_agent.application.concept_scope import ConceptScopeDecision, decide_concept_scope
from forgecad_agent.application.conversation import (
    PROMPT_CONTRACT_VERSION,
    compile_provider_conversation,
    make_deterministic_memory_summary,
)
from forgecad_agent.application.mechanical_planner import (
    MechanicalConceptPlan,
    MechanicalPlannerError,
    MechanicalConceptPlanner,
    mechanical_planner_from_env,
    planner_connection_state,
)
from forgecad_agent.application.provider_gateway import ProviderConnectionState, ProviderExecutionTrace
from forgecad_agent.application.agent_action_loop import AgentActionToolEvent, ProductToolRegistryManifest
from forgecad_agent.application.geometry_worker import build_blockout, resolve_blockout_variant, segment_blockout
from forgecad_agent.application.agent_rendering import AgentRenderError, render_agent_views
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


class AgentKernelError(RuntimeError):
    def __init__(self, code: str, message: str, *, status_code: int = 400, details: Optional[Mapping[str, Any]] = None) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code
        self.details = dict(details or {})


class AgentKernelIdempotencyConflict(RuntimeError):
    pass


class AgentKernelService:
    """Durable Agent session kernel with a swappable mechanical planner."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        planner: Optional[MechanicalConceptPlanner] = None,
    ) -> None:
        self.connection_factory = connection_factory
        self.planner = planner or mechanical_planner_from_env()
        self._provider_cancellations: dict[str, threading.Event] = {}
        self._provider_cancellations_lock = threading.Lock()
        self._provider_check_cancellations: dict[str, threading.Event] = {}

    def create_thread(
        self,
        request: CreateAgentThreadRequest,
        idempotency_key: str,
    ) -> AgentThreadDetail:
        scope = "POST /api/v1/agent/threads"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different thread request."
                    )
                return AgentThreadDetail.model_validate_json(existing.response_json)
            if request.project_id is not None:
                project = unit.concept_projects.get_active(request.project_id)
                if project is None:
                    raise AgentKernelError("PROJECT_NOT_FOUND", "Agent project was not found.", status_code=404)
            now = _utc_now()
            thread_id = _new_id("thr")
            unit.agent_kernel.add_thread(
                thread_id=thread_id,
                project_id=request.project_id,
                title=request.title,
                provider_id=request.provider_id,
                created_at=now,
            )
            response = self._thread_detail(unit, thread_id)
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def list_threads(self) -> AgentThreadListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            items = [AgentThreadSummary(**dict(row)) for row in unit.agent_kernel.list_threads()]
        return AgentThreadListResponse(items=items, next_cursor=None)

    def build_blockout(
        self,
        request: BuildAgentBlockoutRequest,
        idempotency_key: str,
    ) -> BuildAgentBlockoutResponse:
        scope = "POST /api/v1/agent/blockouts"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different blockout request."
                    )
                return BuildAgentBlockoutResponse.model_validate_json(existing.response_json)
            try:
                variant_id = resolve_blockout_variant(
                    request.plan,
                    request.direction_id,
                    request.variant_id,
                    request.variation_index,
                )
                result = build_blockout(
                    request.plan,
                    request.direction_id,
                    variant_id,
                    request.presentation_profile,
                )
            except ValueError as exc:
                raise AgentKernelError("BLOCKOUT_INVALID", str(exc), status_code=400) from exc
            response = BuildAgentBlockoutResponse(
                artifact_id=_new_id("artifact"),
                plan_id=request.plan.plan_id,
                direction_id=result.direction_id,
                variant_id=result.variant_id,
                variation_index=request.variation_index,
                presentation_profile=result.presentation_profile,
                domain_pack_id=request.plan.domain_pack_id,
                triangle_count=result.triangle_count,
                bounds_mm=result.bounds_mm,
                topology_hash=result.topology_hash,
                assembly_graph=result.assembly_graph,
                shape_program=result.shape_program,
                glb_base64=base64.b64encode(result.glb_bytes).decode("ascii"),
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=_utc_now(),
            )
            return response

    def render_blockout_concept_preview(
        self,
        request: RenderAgentBlockoutConceptPreviewRequest,
    ) -> AgentBlockoutConceptPreview:
        """Render one local, disposable direction image without opening a UnitOfWork.

        Unlike ``build_blockout`` and ``segment_blockout``, this path does not
        create an idempotency row or candidate.  It reconstructs the same
        bounded geometry from the supplied plan and releases only a 320x240
        transparent PNG plus a context fingerprint for the UI's stale-result
        guard.
        """
        try:
            variant_id = resolve_blockout_variant(
                request.plan,
                request.direction_id,
                request.variant_id,
                request.variation_index,
            )
            blockout = build_blockout(
                request.plan,
                request.direction_id,
                variant_id,
                request.presentation_profile,
            )
            rendered = render_agent_views(blockout.glb_bytes, width=320, height=240)
            payload = rendered.views["iso"]
        except (AgentRenderError, ValueError, KeyError) as exc:
            raise AgentKernelError("BLOCKOUT_CONCEPT_PREVIEW_INVALID", str(exc), status_code=400) from exc
        context = {
            "schema_version": "AgentBlockoutConceptPreview@1",
            "plan_id": request.plan.plan_id,
            "direction_id": blockout.direction_id,
            "variant_id": blockout.variant_id,
            "variation_index": request.variation_index,
            "presentation_profile": request.presentation_profile,
            "domain_pack_id": request.plan.domain_pack_id,
            "topology_hash": blockout.topology_hash,
            "renderer_id": "forgecad-agent-software-raster@1",
            "width": rendered.width,
            "height": rendered.height,
        }
        return AgentBlockoutConceptPreview(
            plan_id=request.plan.plan_id,
            direction_id=blockout.direction_id,
            variant_id=blockout.variant_id,
            variation_index=request.variation_index,
            domain_pack_id=request.plan.domain_pack_id,
            topology_hash=blockout.topology_hash,
            render_context_sha256=_hash_json(context),
            width=rendered.width,
            height=rendered.height,
            png_base64=base64.b64encode(payload).decode("ascii"),
            sha256=hashlib.sha256(payload).hexdigest(),
            byte_size=len(payload),
        )

    def segment_blockout(
        self,
        request: SegmentAgentBlockoutRequest,
        idempotency_key: str,
    ) -> SegmentAgentBlockoutResponse:
        scope = "POST /api/v1/agent/blockouts:segment"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different segmentation request."
                    )
                return SegmentAgentBlockoutResponse.model_validate_json(existing.response_json)
            try:
                variant_id = resolve_blockout_variant(
                    request.plan,
                    request.direction_id,
                    request.variant_id,
                    request.variation_index,
                )
                parts = segment_blockout(
                    request.plan,
                    request.direction_id,
                    variant_id,
                    request.presentation_profile,
                )
                result = build_blockout(
                    request.plan,
                    request.direction_id,
                    variant_id,
                    request.presentation_profile,
                )
            except ValueError as exc:
                raise AgentKernelError("SEGMENTATION_INVALID", str(exc), status_code=400) from exc
            response = SegmentAgentBlockoutResponse(
                artifact_id=request.artifact_id or _new_id("artifact"),
                plan_id=request.plan.plan_id,
                direction_id=request.direction_id,
                variant_id=result.variant_id,
                variation_index=request.variation_index,
                presentation_profile=result.presentation_profile,
                domain_pack_id=request.plan.domain_pack_id,
                parts=[BlockoutPartCandidate.model_validate(part) for part in parts],
                assembly_graph=result.assembly_graph,
            )
            project_id = request.plan.spec.get("project_id") if isinstance(request.plan.spec, dict) else None
            if project_id == "prj_unbound_agent_session" or not isinstance(project_id, str):
                project_id = None
            elif unit.concept_projects.get_active(project_id) is None:
                project_id = None
            unit.agent_assets.add_candidate(
                artifact_id=response.artifact_id,
                project_id=project_id,
                plan_id=response.plan_id,
                direction_id=response.direction_id,
                domain_pack_id=response.domain_pack_id,
                candidate_json=_canonical_json(response.model_dump(mode="json")),
                shape_program_json=_canonical_json(result.shape_program),
                assembly_graph_json=_canonical_json(result.assembly_graph),
                material_bindings_json="{}",
                glb_base64=base64.b64encode(result.glb_bytes).decode("ascii"),
                created_at=_utc_now(),
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=_utc_now(),
            )
            return response

    def get_thread(self, thread_id: str) -> AgentThreadDetail:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            return self._thread_detail(unit, thread_id)

    def provider_connection_state(self) -> ProviderConnectionState:
        return planner_connection_state(self.planner)

    def product_tool_registry_manifest(self) -> ProductToolRegistryManifest:
        from forgecad_agent.application.product_tool_registry import forgecad_product_tool_registry

        return forgecad_product_tool_registry().public_manifest()

    def check_provider(self, check_id: Optional[str] = None) -> AgentProviderCheckResponse:
        provider_id = str(getattr(self.planner, "provider_id", "unknown"))
        model = getattr(self.planner, "model_name", None)
        connection = planner_connection_state(self.planner)
        if provider_id == "deterministic_mechanical_planner":
            return AgentProviderCheckResponse(
                status="not_configured",
                provider_id=provider_id,
                model=model,
                message="当前使用本机离线规划，没有发起大模型请求。",
                network_call_made=False,
                connection=connection,
            )
        traces: list[ProviderExecutionTrace] = []
        cancel_event = threading.Event()
        if check_id:
            with self._provider_cancellations_lock:
                self._provider_check_cancellations[check_id] = cancel_event
        try:
            _plan_complete_concept(
                self.planner,
                brief="ForgeCAD Provider connectivity check. Return a valid complete concept plan; do not provide engineering or manufacturing instructions.",
                pack=domain_pack_by_id("pack_future_weapon_prop"),
                project_id=None,
                conversation=None,
                cancel_event=cancel_event,
                trace_observer=traces.append,
                action_loop_enabled=False,
            )
        except MechanicalPlannerError as exc:
            trace = getattr(self.planner, "last_execution_trace", None)
            network_call_made = bool(trace.network_call_made) if isinstance(trace, ProviderExecutionTrace) else exc.network_call_made
            return AgentProviderCheckResponse(
                status="not_configured" if exc.code == "PROVIDER_UNCONFIGURED" else "cancelled" if exc.code == "PROVIDER_CANCELLED" else "failed",
                provider_id=provider_id,
                model=model,
                message=str(exc),
                network_call_made=network_call_made,
                connection=connection.model_copy(
                    update={
                        "status": "unconfigured" if exc.code == "PROVIDER_UNCONFIGURED" else "failed",
                        "network_call_made": network_call_made,
                        "failure_code": exc.code,
                        "message": str(exc),
                    }
                ),
                execution_trace=traces,
            )
        finally:
            if check_id:
                with self._provider_cancellations_lock:
                    self._provider_check_cancellations.pop(check_id, None)
        return AgentProviderCheckResponse(
            status="ready",
            provider_id=provider_id,
            model=model,
            message="Provider 已返回符合合同的结构化设计计划。",
            network_call_made=True,
            connection=connection.model_copy(update={"network_call_made": True}),
            execution_trace=traces,
        )

    def cancel_provider_check(self, check_id: str) -> bool:
        with self._provider_cancellations_lock:
            cancel_event = self._provider_check_cancellations.get(check_id)
            if cancel_event is None:
                return False
            cancel_event.set()
            return True

    def start_turn(
        self,
        thread_id: str,
        request: StartAgentTurnRequest,
        idempotency_key: str,
    ) -> AgentTurn:
        scope = f"POST /api/v1/agent/threads/{thread_id}/turns"
        request_hash = _hash_json(request.model_dump(mode="json"))
        provider_context = None
        domain_pack_id: Optional[str] = None
        project_id: Optional[str] = None
        turn_id: Optional[str] = None
        budget_provider_id: Optional[str] = None
        budget_day_utc: Optional[str] = None
        budget_reservation_micros = 0
        cancel_event: Optional[threading.Event] = None
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            thread = unit.agent_kernel.get_thread(thread_id)
            if thread is None:
                raise AgentKernelError("THREAD_NOT_FOUND", "Agent thread was not found.", status_code=404)
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different turn request."
                    )
                return AgentTurn.model_validate_json(existing.response_json)
            if thread["status"] == "archived":
                raise AgentKernelError("THREAD_ARCHIVED", "Archived threads cannot receive turns.")
            if unit.agent_kernel.has_in_flight_turn(thread_id):
                raise AgentKernelError(
                    "THREAD_TURN_IN_PROGRESS",
                    "当前设计请求仍在处理中，请等待完成后再继续。",
                    status_code=409,
                )
            bound_domain_pack_id = _bound_domain_pack_id(unit.agent_kernel.list_items(thread_id))
            inference = infer_domain(request.message)
            if (
                bound_domain_pack_id is not None
                and inference.status == "recognized"
                and inference.domain_pack_id != bound_domain_pack_id
            ):
                raise AgentKernelError(
                    "THREAD_DOMAIN_CHANGE_REQUIRES_NEW_THREAD",
                    "当前会话已绑定一个设计类别。请新建设计会话后再切换到另一类对象。",
                    status_code=409,
                )
            scope_decision = decide_concept_scope(
                request.message,
                inference,
                selected_domain_pack_id=request.clarification_domain_pack_id or bound_domain_pack_id,
            )
            if scope_decision.status == "unsupported":
                return self._record_scope_stop(
                    unit,
                    thread_id=thread_id,
                    request=request,
                    idempotency_key=idempotency_key,
                    scope_decision=scope_decision,
                    scope=scope,
                    request_hash=request_hash,
                )
            if scope_decision.status == "clarification_required":
                return self._record_domain_clarification(
                    unit,
                    thread_id=thread_id,
                    request=request,
                    idempotency_key=idempotency_key,
                    inference=inference,
                    scope_decision=scope_decision,
                    scope=scope,
                    request_hash=request_hash,
                )
            if scope_decision.domain_pack_id is None:
                raise AgentKernelError("CONCEPT_SCOPE_INVALID", "Concept scope decision did not bind a domain pack.", status_code=500)
            now = _utc_now()
            turn_id = _new_id("turn")
            project_id = str(thread["project_id"]) if thread["project_id"] else None
            prior_items = [self._item(item).model_dump(mode="json") for item in unit.agent_kernel.list_items(thread_id)]
            existing_summary = unit.agent_kernel.latest_memory_summary(thread_id)
            memory_summary = dict(existing_summary) if existing_summary is not None else None
            compacted = make_deterministic_memory_summary(prior_items)
            if compacted is not None and (
                memory_summary is None or int(compacted["up_to_sequence"]) > int(memory_summary["up_to_sequence"])
            ):
                unit.agent_kernel.add_memory_summary(
                    summary_id=_new_id("threadmem"),
                    thread_id=thread_id,
                    up_to_sequence=int(compacted["up_to_sequence"]),
                    summary_text=str(compacted["summary_text"]),
                    domain_pack_id=scope_decision.domain_pack_id,
                    snapshot_fingerprint=None,
                    prompt_contract_version=PROMPT_CONTRACT_VERSION,
                    created_at=now,
                )
                memory_summary = {
                    **compacted,
                    "prompt_contract_version": PROMPT_CONTRACT_VERSION,
                }
            snapshot = unit.active_designs.get_snapshot(project_id) if project_id else None
            snapshot_payload = snapshot.model_dump(mode="json") if snapshot is not None else None
            provider_context = compile_provider_conversation(
                prior_items=prior_items,
                current_request=request.message,
                memory_summary=memory_summary,
                snapshot=snapshot_payload,
            )
            domain_pack_id = scope_decision.domain_pack_id
            budget_provider_id = _deepseek_budget_provider_id(self.planner)
            if budget_provider_id is not None:
                budget_day_utc = now[:10]
                budget_reservation_micros = _maximum_deepseek_reservation_micros(provider_context)
                if not unit.agent_kernel.reserve_daily_budget(
                    day_utc=budget_day_utc,
                    provider_id=budget_provider_id,
                    budget_micros=_DAILY_DEEPSEEK_BUDGET_MICROS,
                    reservation_micros=budget_reservation_micros,
                    updated_at=now,
                ):
                    raise AgentKernelError(
                        "PROVIDER_DAILY_BUDGET_EXCEEDED",
                        "今日智能设计额度暂时已到上限；你仍可继续查看、编辑和导出当前设计。",
                        status_code=429,
                    )
            cancel_event = threading.Event()
            with self._provider_cancellations_lock:
                self._provider_cancellations[turn_id] = cancel_event
            unit.agent_kernel.add_turn(
                turn_id=turn_id,
                thread_id=thread_id,
                request_text=request.message,
                status="running",
                created_at=now,
                context_hash=provider_context.context_hash,
                prompt_contract_version=provider_context.prompt_contract_version,
                provider_request_fingerprint=_hash_json(
                    {
                        "context_hash": provider_context.context_hash,
                        "domain_pack_id": domain_pack_id,
                        "provider": getattr(self.planner, "provider_id", "unknown"),
                    }
                ),
            )
            self._add_item(
                unit,
                thread_id=thread_id,
                turn_id=turn_id,
                item_type="user_message",
                status="completed",
                payload={"text": request.message},
                created_at=now,
            )
            unit.agent_kernel.update_thread(
                thread_id=thread_id,
                status="active",
                summary="正在深化设计",
                last_turn_id=turn_id,
                updated_at=now,
            )

        assert turn_id is not None and provider_context is not None and domain_pack_id is not None and cancel_event is not None
        try:
            # Never hold a SQLite transaction while a remote Provider is running.
            plan = _plan_complete_concept(
                self.planner,
                brief=request.message,
                pack=domain_pack_by_id(domain_pack_id),
                project_id=project_id,
                conversation=provider_context,
                cancel_event=cancel_event,
                trace_observer=lambda trace: self._record_provider_execution_trace(
                    thread_id=thread_id,
                    turn_id=turn_id,
                    trace=trace,
                ),
                action_observer=lambda event: self._record_action_tool_event(
                    thread_id=thread_id,
                    turn_id=turn_id,
                    event=event,
                ),
            )
        except MechanicalPlannerError as exc:
            now = _utc_now()
            with SQLiteUnitOfWork(self.connection_factory) as unit:
                current_turn = unit.agent_kernel.get_turn(turn_id)
                actual_cost_micros = _actual_deepseek_cost_micros(getattr(self.planner, "last_call_telemetry", None))
                if budget_provider_id is not None and budget_day_utc is not None:
                    unit.agent_kernel.settle_daily_budget(
                        day_utc=budget_day_utc,
                        provider_id=budget_provider_id,
                        reservation_micros=budget_reservation_micros,
                        actual_micros=actual_cost_micros,
                        updated_at=now,
                    )
                if current_turn is not None and current_turn["status"] == "cancelled":
                    unit.agent_kernel.update_turn(
                        turn_id=turn_id,
                        status="cancelled",
                        updated_at=now,
                        error_code="PROVIDER_CANCELLED",
                        error_message="Provider request was cancelled; saved assets were not changed.",
                        usage=_usage_from_telemetry(
                            self.planner,
                            provider_context,
                            budget_reservation_micros=budget_reservation_micros,
                            estimated_cost_micros=actual_cost_micros,
                        ),
                    )
                    return self._turn(unit, turn_id)
                unit.agent_kernel.update_turn(
                    turn_id=turn_id,
                    status="failed",
                    updated_at=now,
                    error_code="PROVIDER_OUTCOME_UNKNOWN" if exc.code == "PROVIDER_TIMEOUT" else exc.code,
                    error_message="本次模型请求超时且结果未知；不会自动重试或切换离线成功。" if exc.code == "PROVIDER_TIMEOUT" else str(exc),
                    usage=_usage_from_telemetry(
                        self.planner,
                        provider_context,
                        budget_reservation_micros=budget_reservation_micros,
                        estimated_cost_micros=actual_cost_micros,
                    ),
                )
                unit.agent_kernel.update_thread(
                    thread_id=thread_id,
                    status="error",
                    summary="本次智能设计没有完成，可重新发起。",
                    last_turn_id=turn_id,
                    updated_at=now,
                )
            raise AgentKernelError(
                exc.code,
                str(exc),
                status_code=_provider_error_status(exc.code, exc.recoverable),
                details={
                    "network_call_made": exc.network_call_made,
                    "saved_assets_safe": True,
                    "fallback_used": False,
                },
            ) from exc
        finally:
            with self._provider_cancellations_lock:
                self._provider_cancellations.pop(turn_id, None)

        now = _utc_now()
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            actual_cost_micros = _actual_deepseek_cost_micros(getattr(self.planner, "last_call_telemetry", None))
            if budget_provider_id is not None and budget_day_utc is not None:
                unit.agent_kernel.settle_daily_budget(
                    day_utc=budget_day_utc,
                    provider_id=budget_provider_id,
                    reservation_micros=budget_reservation_micros,
                    actual_micros=actual_cost_micros,
                    updated_at=now,
                )
            current_turn = unit.agent_kernel.get_turn(turn_id)
            if current_turn is not None and current_turn["status"] == "cancelled":
                unit.agent_kernel.update_turn(
                    turn_id=turn_id,
                    status="cancelled",
                    updated_at=now,
                    error_code="PROVIDER_CANCELLED",
                    error_message="Provider completion arrived after cancellation and was discarded.",
                    usage=_usage_from_telemetry(
                        self.planner,
                        provider_context,
                        provider=plan.provider_id,
                        budget_reservation_micros=budget_reservation_micros,
                        estimated_cost_micros=actual_cost_micros,
                    ),
                )
                return self._turn(unit, turn_id)
            for item_type, payload in self._plan_items(plan, scope_decision):
                self._add_item(
                    unit,
                    thread_id=thread_id,
                    turn_id=turn_id,
                    item_type=item_type,
                    status="completed",
                    payload=payload,
                    created_at=now,
                )
            unit.agent_kernel.update_turn(
                turn_id=turn_id,
                status="completed",
                updated_at=now,
                usage=_usage_from_telemetry(
                    self.planner,
                    provider_context,
                    provider=plan.provider_id,
                    budget_reservation_micros=budget_reservation_micros,
                    estimated_cost_micros=actual_cost_micros,
                ),
            )
            unit.agent_kernel.update_thread(
                thread_id=thread_id,
                status="idle",
                summary=request.message[:240],
                last_turn_id=turn_id,
                updated_at=now,
            )
            response = self._turn(unit, turn_id)
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def _record_domain_clarification(
        self,
        unit: SQLiteUnitOfWork,
        *,
        thread_id: str,
        request: StartAgentTurnRequest,
        idempotency_key: str,
        inference: Any,
        scope_decision: ConceptScopeDecision,
        scope: str,
        request_hash: str,
    ) -> AgentTurn:
        """Persist only a user-facing clarification, never a plan or asset.

        D002 remains the pure classifier/write barrier. D003 adds this durable
        conversation item so the zero-basics UI can ask one plain-language
        question and retry with the user's choice. No Plan, Blockout, Version,
        Snapshot or asset row is created in this branch.
        """
        now = _utc_now()
        turn_id = _new_id("turn")
        unit.agent_kernel.add_turn(
            turn_id=turn_id,
            thread_id=thread_id,
            request_text=request.message,
            status="waiting_for_clarification",
            created_at=now,
        )
        self._add_item(
            unit,
            thread_id=thread_id,
            turn_id=turn_id,
            item_type="user_message",
            status="completed",
            payload={"text": request.message},
            created_at=now,
        )
        options = _domain_clarification_options(inference)
        if inference.status == "ambiguous":
            question = "你想先设计汽车、飞机、机械臂，还是未来概念道具？"
        else:
            question = "我还不能判断对象类别。你想先设计汽车、飞机、机械臂，还是未来概念道具？"
        payload = {
            "kind": "domain",
            "status": inference.status,
            "question": question,
            "options": options,
            "domain_inference": inference.model_dump(mode="json"),
            "scope_decision": scope_decision.model_dump(mode="json"),
        }
        self._add_item(
            unit,
            thread_id=thread_id,
            turn_id=turn_id,
            item_type="clarification",
            status="completed",
            payload=payload,
            created_at=now,
        )
        unit.agent_kernel.update_thread(
            thread_id=thread_id,
            status="active",
            summary=question,
            last_turn_id=turn_id,
            updated_at=now,
        )
        response = self._turn(unit, turn_id)
        unit.idempotency.add(
            scope=scope,
            key=idempotency_key,
            request_hash=request_hash,
            response_json=_canonical_json(response.model_dump(mode="json")),
            created_at=now,
        )
        return response

    def _record_scope_stop(
        self,
        unit: SQLiteUnitOfWork,
        *,
        thread_id: str,
        request: StartAgentTurnRequest,
        idempotency_key: str,
        scope_decision: ConceptScopeDecision,
        scope: str,
        request_hash: str,
    ) -> AgentTurn:
        """Persist one readable local stop without touching the planner or assets."""

        now = _utc_now()
        turn_id = _new_id("turn")
        unit.agent_kernel.add_turn(
            turn_id=turn_id,
            thread_id=thread_id,
            request_text=request.message,
            status="completed",
            created_at=now,
        )
        self._add_item(
            unit,
            thread_id=thread_id,
            turn_id=turn_id,
            item_type="user_message",
            status="completed",
            payload={"text": request.message},
            created_at=now,
        )
        self._add_item(
            unit,
            thread_id=thread_id,
            turn_id=turn_id,
            item_type="clarification",
            status="completed",
            payload={
                "kind": "scope",
                "status": "unsupported",
                "question": scope_decision.user_message,
                "options": [],
                "scope_decision": scope_decision.model_dump(mode="json"),
            },
            created_at=now,
        )
        self._add_item(
            unit,
            thread_id=thread_id,
            turn_id=turn_id,
            item_type="assistant_message",
            status="completed",
            payload={"text": "当前请求没有发送给模型，也没有创建 3D 模型、版本或导出。请改为仅描述外观概念。"},
            created_at=now,
        )
        unit.agent_kernel.update_turn(
            turn_id=turn_id,
            status="completed",
            updated_at=now,
            usage={"provider_called": False, "scope_status": "unsupported"},
        )
        unit.agent_kernel.update_thread(
            thread_id=thread_id,
            status="idle",
            summary=scope_decision.user_message,
            last_turn_id=turn_id,
            updated_at=now,
        )
        response = self._turn(unit, turn_id)
        unit.idempotency.add(
            scope=scope,
            key=idempotency_key,
            request_hash=request_hash,
            response_json=_canonical_json(response.model_dump(mode="json")),
            created_at=now,
        )
        return response

    def cancel_turn(self, turn_id: str, idempotency_key: str) -> AgentTurn:
        with self._provider_cancellations_lock:
            cancel_event = self._provider_cancellations.get(turn_id)
            if cancel_event is not None:
                cancel_event.set()
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            scope = f"POST /api/v1/agent/turns/{turn_id}/cancel"
            request_hash = _hash_json({"turn_id": turn_id, "action": "cancel"})
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different cancel request."
                    )
                return AgentTurn.model_validate_json(existing.response_json)
            turn = unit.agent_kernel.get_turn(turn_id)
            if turn is None:
                raise AgentKernelError("TURN_NOT_FOUND", "Agent turn was not found.", status_code=404)
            if turn["status"] in {"completed", "failed", "cancelled"}:
                response = self._turn(unit, turn_id)
                unit.idempotency.add(
                    scope=scope,
                    key=idempotency_key,
                    request_hash=request_hash,
                    response_json=_canonical_json(response.model_dump(mode="json")),
                    created_at=_utc_now(),
                )
                return response
            now = _utc_now()
            unit.agent_kernel.update_turn(turn_id=turn_id, status="cancelled", updated_at=now)
            unit.agent_kernel.update_thread(
                thread_id=str(turn["thread_id"]),
                status="idle",
                summary="本次请求已取消",
                last_turn_id=turn_id,
                updated_at=now,
            )
            self._add_item(
                unit,
                thread_id=str(turn["thread_id"]),
                turn_id=turn_id,
                item_type="assistant_message",
                status="cancelled",
                payload={"text": "本次请求已取消，当前项目没有新增永久修改。"},
                created_at=now,
            )
            response = self._turn(unit, turn_id)
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def _record_provider_execution_trace(
        self,
        *,
        thread_id: str,
        turn_id: str,
        trace: ProviderExecutionTrace,
    ) -> None:
        """Persist only the sanitized lifecycle contract as an Agent Item."""
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            turn = unit.agent_kernel.get_turn(turn_id)
            if turn is None or turn["status"] == "cancelled" and trace.phase != "cancelled":
                return
            status = {
                "failed": "failed",
                "cancelled": "cancelled",
                "completed": "completed",
            }.get(trace.phase, "completed")
            self._add_item(
                unit,
                thread_id=thread_id,
                turn_id=turn_id,
                item_type="tool_result" if trace.phase in {"completed", "failed", "cancelled"} else "tool_call",
                status=status,
                payload={
                    "tool": "provider_gateway",
                    "message": trace.message,
                    "provider_execution_trace": trace.model_dump(mode="json"),
                },
                created_at=_utc_now(),
            )

    def create_approval(
        self,
        thread_id: str,
        request: CreateAgentApprovalRequest,
        idempotency_key: str,
    ) -> AgentApproval:
        scope = f"POST /api/v1/agent/threads/{thread_id}/approvals"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            thread = unit.agent_kernel.get_thread(thread_id)
            if thread is None:
                raise AgentKernelError("THREAD_NOT_FOUND", "Agent thread was not found.", status_code=404)
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different approval request."
                    )
                return AgentApproval.model_validate_json(existing.response_json)
            turn = unit.agent_kernel.get_turn(request.turn_id)
            if turn is None or turn["thread_id"] != thread_id:
                raise AgentKernelError("TURN_NOT_FOUND", "Approval turn was not found in this thread.", status_code=404)
            now = _utc_now()
            item_id = self._add_item(
                unit,
                thread_id=thread_id,
                turn_id=request.turn_id,
                item_type="approval_request",
                status="pending",
                payload={"action": request.action, "payload": request.payload},
                created_at=now,
            )
            approval_id = _new_id("approval")
            unit.agent_kernel.add_approval(
                approval_id=approval_id,
                thread_id=thread_id,
                turn_id=request.turn_id,
                item_id=item_id,
                action=request.action,
                payload=request.payload,
                created_at=now,
            )
            unit.agent_kernel.update_turn(
                turn_id=request.turn_id,
                status="waiting_for_approval",
                updated_at=now,
            )
            unit.agent_kernel.update_thread(
                thread_id=thread_id,
                status="active",
                summary="等待用户确认",
                last_turn_id=request.turn_id,
                updated_at=now,
            )
            approval = self._approval(unit.agent_kernel.get_approval(approval_id))
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(approval.model_dump(mode="json")),
                created_at=now,
            )
            return approval

    def resolve_approval(
        self,
        approval_id: str,
        request: ResolveAgentApprovalRequest,
        idempotency_key: str,
    ) -> AgentApprovalResolution:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            scope = f"POST /api/v1/agent/approvals/{approval_id}/resolve"
            request_hash = _hash_json(request.model_dump(mode="json"))
            existing = unit.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise AgentKernelIdempotencyConflict(
                        "Idempotency-Key was reused with a different approval resolution."
                    )
                return AgentApprovalResolution.model_validate_json(existing.response_json)
            approval_row = unit.agent_kernel.get_approval(approval_id)
            if approval_row is None:
                raise AgentKernelError("APPROVAL_NOT_FOUND", "Agent approval was not found.", status_code=404)
            if approval_row["status"] != "pending":
                raise AgentKernelError("APPROVAL_ALREADY_RESOLVED", "Agent approval was already resolved.")
            now = _utc_now()
            unit.agent_kernel.resolve_approval(
                approval_id,
                status=request.decision,
                resolved_at=now,
            )
            unit.agent_kernel.update_item_status(
                str(approval_row["item_id"]),
                status="completed" if request.decision == "approved" else "cancelled",
            )
            turn_status = "completed" if request.decision == "approved" else "cancelled"
            unit.agent_kernel.update_turn(
                turn_id=str(approval_row["turn_id"]),
                status=turn_status,
                updated_at=now,
                error_code=None if request.decision == "approved" else "USER_REJECTED",
                error_message=None if request.decision == "approved" else request.note or "用户拒绝了这次修改。",
            )
            unit.agent_kernel.update_thread(
                thread_id=str(approval_row["thread_id"]),
                status="idle",
                summary="用户已确认" if request.decision == "approved" else "用户已取消修改",
                last_turn_id=str(approval_row["turn_id"]),
                updated_at=now,
            )
            approval = self._approval(unit.agent_kernel.get_approval(approval_id))
            response = AgentApprovalResolution(
                approval=approval,
                turn=self._turn(unit, str(approval_row["turn_id"])),
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def events(self, thread_id: str, *, after: int = 0) -> list[AgentEvent]:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            if unit.agent_kernel.get_thread(thread_id) is None:
                raise AgentKernelError("THREAD_NOT_FOUND", "Agent thread was not found.", status_code=404)
            events: list[AgentEvent] = []
            for row in unit.agent_kernel.list_items(thread_id, after=after):
                events.append(
                    AgentEvent(
                        sequence=int(row["sequence"]),
                        thread_id=str(row["thread_id"]),
                        turn_id=str(row["turn_id"]),
                        item=self._item(row),
                    )
                )
            return events

    def _plan_items(self, plan: MechanicalConceptPlan, scope_decision: ConceptScopeDecision) -> list[tuple[str, Mapping[str, Any]]]:
        plan_payload = plan.model_dump(mode="json")
        return [
            (
                "plan",
                {
                    "stage": "complete_concept_plan",
                    "domain_pack_id": plan.domain_pack_id,
                    "plan_id": plan.plan_id,
                    "message": "已记录创意并生成三个完整外观方向，确认后再进入 ShapeProgram。",
                    "directions": plan_payload["directions"],
                    "spec": plan_payload["spec"],
                    "scope_decision": scope_decision.model_dump(mode="json"),
                    "provider": plan.provider_id,
                },
            ),
            (
                "assistant_message",
                {
                    "text": "我已生成三个完整外观方向。请选择一个方向，确认后再生成可预览的 3D blockout。",
                    "next_action": "select_concept_direction",
                    "input_hash": hashlib.sha256(plan.brief.encode("utf-8")).hexdigest(),
                    "provider": plan.provider_id,
                    "plan_id": plan.plan_id,
                },
            ),
        ]

    def _record_action_tool_event(
        self,
        *,
        thread_id: str,
        turn_id: str,
        event: AgentActionToolEvent,
    ) -> None:
        """Persist only public tool lifecycle facts, never hidden reasoning."""

        normalized = event.model_copy(update={"parent_turn_id": turn_id})
        payload = normalized.model_dump(mode="json", exclude_none=True)
        # Defense in depth for future Provider adapters: these fields are never
        # part of AgentActionToolEvent@1 and must not appear in durable items.
        payload.pop("reasoning_content", None)
        payload.pop("raw_reasoning", None)
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            current = unit.agent_kernel.get_turn(turn_id)
            if current is None or str(current["thread_id"]) != thread_id:
                return
            self._add_item(
                unit,
                thread_id=thread_id,
                turn_id=turn_id,
                item_type=normalized.event_kind,
                status=normalized.status,
                payload=payload,
                created_at=_utc_now(),
            )

    def _add_item(
        self,
        unit: SQLiteUnitOfWork,
        *,
        thread_id: str,
        turn_id: str,
        item_type: str,
        status: str,
        payload: Mapping[str, Any],
        created_at: str,
    ) -> str:
        item_id = _new_id("item")
        unit.agent_kernel.add_item(
            item_id=item_id,
            thread_id=thread_id,
            turn_id=turn_id,
            sequence=unit.agent_kernel.next_sequence(thread_id),
            item_type=item_type,
            status=status,
            payload=payload,
            created_at=created_at,
        )
        return item_id

    def _thread_detail(self, unit: SQLiteUnitOfWork, thread_id: str) -> AgentThreadDetail:
        row = unit.agent_kernel.get_thread(thread_id)
        if row is None:
            raise AgentKernelError("THREAD_NOT_FOUND", "Agent thread was not found.", status_code=404)
        summary = AgentThreadSummary(**dict(row))
        return AgentThreadDetail(
            **summary.model_dump(),
            turns=[self._turn(unit, str(turn["turn_id"])) for turn in unit.agent_kernel.list_turns(thread_id)],
        )

    def _turn(self, unit: SQLiteUnitOfWork, turn_id: str) -> AgentTurn:
        row = unit.agent_kernel.get_turn(turn_id)
        if row is None:
            raise AgentKernelError("TURN_NOT_FOUND", "Agent turn was not found.", status_code=404)
        items = [self._item(item) for item in unit.agent_kernel.list_items(str(row["thread_id"]), after=0) if item["turn_id"] == turn_id]
        approvals = [self._approval(item) for item in unit.agent_kernel.list_approvals(str(row["thread_id"])) if item["turn_id"] == turn_id]
        return AgentTurn(
            turn_id=str(row["turn_id"]),
            thread_id=str(row["thread_id"]),
            request_text=str(row["request_text"]),
            status=str(row["status"]),
            error_code=row["error_code"],
            error_message=row["error_message"],
            usage=json.loads(str(row["usage_json"])),
            created_at=str(row["created_at"]),
            updated_at=str(row["updated_at"]),
            items=items,
            approvals=approvals,
        )

    @staticmethod
    def _item(row: sqlite3.Row) -> AgentItem:
        return AgentItem(
            item_id=str(row["item_id"]),
            thread_id=str(row["thread_id"]),
            turn_id=str(row["turn_id"]),
            sequence=int(row["sequence"]),
            item_type=str(row["item_type"]),
            status=str(row["status"]),
            payload=json.loads(str(row["payload_json"])),
            created_at=str(row["created_at"]),
        )

    @staticmethod
    def _approval(row: Optional[sqlite3.Row]) -> AgentApproval:
        if row is None:
            raise AgentKernelError("APPROVAL_NOT_FOUND", "Agent approval was not found.", status_code=404)
        return AgentApproval(
            approval_id=str(row["approval_id"]),
            thread_id=str(row["thread_id"]),
            turn_id=str(row["turn_id"]),
            item_id=str(row["item_id"]),
            action=str(row["action"]),
            status=str(row["status"]),
            payload=json.loads(str(row["payload_json"])),
            created_at=str(row["created_at"]),
            resolved_at=row["resolved_at"],
        )


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _plan_complete_concept(
    planner: MechanicalConceptPlanner,
    *,
    brief: str,
    pack: Any,
    project_id: Optional[str],
    conversation: Any,
    cancel_event: Optional[threading.Event] = None,
    trace_observer: Optional[Any] = None,
    action_observer: Optional[Any] = None,
    action_loop_enabled: bool = True,
) -> MechanicalConceptPlan:
    """Pass compiled context to current Providers without breaking test adapters.

    The public planner protocol now accepts ``conversation``.  Older local
    deterministic adapters are deliberately still usable in smoke tests and
    extensions; they do not make an HTTP request and therefore cannot lose
    conversation context by receiving the legacy call shape.
    """

    try:
        parameters = inspect.signature(planner.plan_complete_concept).parameters.values()
        parameter_list = list(parameters)
        accepts_kwargs = any(parameter.kind is inspect.Parameter.VAR_KEYWORD for parameter in parameter_list)
        accepted_names = {parameter.name for parameter in parameter_list}
    except (TypeError, ValueError):
        accepts_kwargs = True
        accepted_names = set()
    kwargs: dict[str, Any] = {"brief": brief, "pack": pack, "project_id": project_id}
    if accepts_kwargs or "conversation" in accepted_names:
        kwargs["conversation"] = conversation
    if accepts_kwargs or "cancel_event" in accepted_names:
        kwargs["cancel_event"] = cancel_event
    if accepts_kwargs or "trace_observer" in accepted_names:
        kwargs["trace_observer"] = trace_observer
    if accepts_kwargs or "action_observer" in accepted_names:
        kwargs["action_observer"] = action_observer
    if accepts_kwargs or "action_loop_enabled" in accepted_names:
        kwargs["action_loop_enabled"] = action_loop_enabled
    return planner.plan_complete_concept(**kwargs)


def _usage_from_telemetry(
    planner: MechanicalConceptPlanner,
    context: Any,
    *,
    provider: Optional[str] = None,
    budget_reservation_micros: int = 0,
    estimated_cost_micros: Optional[int] = None,
) -> dict[str, Any]:
    """Persist only redaction-safe Provider accounting fields."""

    telemetry = getattr(planner, "last_call_telemetry", None)
    trace = getattr(planner, "last_execution_trace", None)
    values: dict[str, Any] = {
        "provider": provider or getattr(planner, "provider_id", "unknown"),
        "routing_mode": getattr(context, "routing_mode", "concept_planning"),
        "context_hash": getattr(context, "context_hash", None),
        "prompt_contract_version": getattr(context, "prompt_contract_version", PROMPT_CONTRACT_VERSION),
        "budget_reservation_cny": round(budget_reservation_micros / 1_000_000, 6) if budget_reservation_micros else None,
        "estimated_cost_cny": round(estimated_cost_micros / 1_000_000, 6) if estimated_cost_micros is not None else None,
        "network_call_made": trace.network_call_made if isinstance(trace, ProviderExecutionTrace) else False,
        "provider_phase": trace.phase if isinstance(trace, ProviderExecutionTrace) else "unavailable",
        "provider_error_code": trace.error_code if isinstance(trace, ProviderExecutionTrace) else None,
        "provider_attempt": trace.attempt if isinstance(trace, ProviderExecutionTrace) else None,
        "fallback_used": False,
    }
    if telemetry is None:
        values["usage_status"] = "unavailable"
        return {key: value for key, value in values.items() if value is not None}
    values.update(
        {
            "latency_ms": telemetry.latency_ms,
            "prompt_tokens": telemetry.input_tokens,
            "completion_tokens": telemetry.output_tokens,
            "total_tokens": telemetry.total_tokens,
            "prompt_cache_hit_tokens": telemetry.prompt_cache_hit_tokens,
            "prompt_cache_miss_tokens": telemetry.prompt_cache_miss_tokens,
            "usage_status": "reported" if telemetry.total_tokens is not None else "unavailable",
        }
    )
    return {key: value for key, value in values.items() if value is not None}


_DAILY_DEEPSEEK_BUDGET_MICROS = 20_000_000


def _deepseek_budget_provider_id(planner: MechanicalConceptPlanner) -> Optional[str]:
    config = getattr(planner, "config", None)
    base_url = str(getattr(config, "base_url", "")).casefold()
    return "deepseek-v4" if "api.deepseek.com" in base_url else None


def _maximum_deepseek_reservation_micros(context: Any) -> int:
    # CNY per million tokens: cache miss 3, output 6.  A004 permits at most 12
    # sequential sub-requests, so reserve that pessimistic upper bound and then
    # settle against the Provider-reported aggregate use.
    prompt_limit = 32_000
    output_limit = int(getattr(context, "max_output_tokens", 1800))
    return int((prompt_limit * 3 + output_limit * 6) * 12)


def _actual_deepseek_cost_micros(telemetry: Any) -> Optional[int]:
    if telemetry is None:
        return None
    hit = getattr(telemetry, "prompt_cache_hit_tokens", None)
    miss = getattr(telemetry, "prompt_cache_miss_tokens", None)
    output = getattr(telemetry, "output_tokens", None)
    if not all(isinstance(value, int) and value >= 0 for value in (hit, miss, output)):
        return None
    # Current price table is versioned in operations docs.  Values here are
    # micros of CNY: 0.025/3/6 CNY per million tokens.
    return int(hit * 0.025 + miss * 3 + output * 6)


def _provider_error_status(code: str, recoverable: bool) -> int:
    return {
        "PROVIDER_UNCONFIGURED": 503,
        "DEEPSEEK_INVALID_REQUEST": 400,
        "DEEPSEEK_AUTH_FAILED": 401,
        "DEEPSEEK_BALANCE_EXHAUSTED": 402,
        "DEEPSEEK_INVALID_PARAMETERS": 422,
        "DEEPSEEK_RATE_LIMITED": 429,
        "DEEPSEEK_SERVER_ERROR": 502,
        "DEEPSEEK_SERVER_BUSY": 503,
        "PROVIDER_NETWORK_ERROR": 502,
        "PROVIDER_TIMEOUT": 504,
        "PROVIDER_EMPTY_CONTENT": 502,
        "PROVIDER_INVALID_JSON": 502,
        "PROVIDER_SCHEMA_MISMATCH": 502,
        "PROVIDER_TOOL_CALLS_UNSUPPORTED": 502,
        "PROVIDER_CANCELLED": 409,
    }.get(code, 502 if recoverable else 400)


def _bound_domain_pack_id(items: list[sqlite3.Row]) -> Optional[DomainPackId]:
    """Read an already persisted Plan; never infer a default weapon domain."""

    for row in reversed(items):
        if row["item_type"] != "plan":
            continue
        try:
            pack_id = json.loads(str(row["payload_json"])).get("domain_pack_id")
            if pack_id in {pack.pack_id for pack in list_domain_packs()}:
                return pack_id
        except (TypeError, ValueError, json.JSONDecodeError):
            continue
    return None


_DOMAIN_OPTION_PROMPTS: dict[DomainPackId, str] = {
    "pack_future_weapon_prop": "我想设计一个非功能性的未来武器概念道具，用于游戏、影视或展示。",
    "pack_vehicle_concept": "我想设计一辆汽车或未来地面载具，先做完整外观概念。",
    "pack_aircraft_concept": "我想设计一架飞机或未来航空器，先做完整外观概念。",
    "pack_robotic_arm_concept": "我想设计一台机械臂或机器人机构，先做完整外观概念。",
}


def _domain_clarification_options(inference: Any) -> list[dict[str, str]]:
    # D003 exposes one stable, zero-basics choice surface for both ambiguous
    # and unsupported inference. Candidate IDs remain diagnostic evidence in
    # domain_inference; they must not silently narrow the user's safe choices.
    _ = inference
    candidate_ids = [pack.pack_id for pack in list_domain_packs()]
    options: list[dict[str, str]] = []
    for pack_id in candidate_ids:
        pack = domain_pack_by_id(pack_id)
        options.append(
            {
                "domain_pack_id": pack.pack_id,
                "label": pack.display_name,
                "prompt": _DOMAIN_OPTION_PROMPTS[pack.pack_id],
            }
        )
    return options
