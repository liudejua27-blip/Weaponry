from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime, timezone
from typing import Any

from forgecad_agent.application.agent_models import (
    ActiveDesignSnapshot,
    ActiveDesignNavigation,
    ConvertLegacyActiveDesignRequest,
    LegacyActiveDesignConversionResponse,
    LegacyActiveDesignReference,
    NavigateActiveDesignRequest,
    SelectActiveDesignRequest,
    SetActiveDesignPartDisplayRequest,
    SetActiveDesignRenderPresetRequest,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.db.agent_repositories import (
    ActiveDesignSnapshotConflict,
    ActiveDesignSnapshotError,
)


class ActiveDesignApiError(RuntimeError):
    def __init__(self, code: str, message: str, *, status_code: int = 400) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code


class ActiveDesignIdempotencyConflict(RuntimeError):
    pass


class ActiveDesignService:
    """Application boundary for the server-owned active workbench design.

    Legacy projects are bootstrapped into a read-only Snapshot only.  The
    conversion endpoint deliberately returns a rebuild hand-off rather than
    synthesising an editable AgentAssetVersion from a legacy ModuleGraph.
    That preserves the original legacy records and prevents unsupported
    geometry from being represented as an editable Agent asset.
    """

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def get_snapshot(self, project_id: str) -> ActiveDesignSnapshot:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            return self._get_or_bootstrap_snapshot(unit, project_id)

    def select_part(
        self,
        project_id: str,
        request: SelectActiveDesignRequest,
        *,
        expected_revision: int,
        idempotency_key: str,
    ) -> ActiveDesignSnapshot:
        scope = f"POST /api/v1/projects/{project_id}/active-design:select"
        request_hash = _hash_json(
            {
                "project_id": project_id,
                "expected_revision": expected_revision,
                "request": request.model_dump(mode="json"),
            }
        )
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ActiveDesignIdempotencyConflict(
                        "Idempotency-Key was reused with a different active-design selection request."
                    )
                return ActiveDesignSnapshot.model_validate_json(replay.response_json)

            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if snapshot.revision != expected_revision:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后重新选择部件。",
                    status_code=409,
                )
            if not hasattr(snapshot.active_design, "asset_version_id"):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                    "当前是 legacy 只读设计；请先让 Agent 重建设计资产。",
                    status_code=409,
                )
            try:
                result = unit.active_designs.select_agent_part(
                    project_id=project_id,
                    expected_revision=expected_revision,
                    part_id=request.selected_part_id,
                    material_zone_id=request.selected_material_zone_id,
                    updated_at=_utc_now(),
                )
            except ActiveDesignSnapshotConflict as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后重新选择部件。",
                    status_code=409,
                ) from exc
            except ActiveDesignSnapshotError as exc:
                raise ActiveDesignApiError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(result.model_dump(mode="json")),
                created_at=result.updated_at,
            )
            return result

    def set_render_preset(
        self,
        project_id: str,
        request: SetActiveDesignRenderPresetRequest,
        *,
        expected_revision: int,
        idempotency_key: str,
    ) -> ActiveDesignSnapshot:
        scope = f"POST /api/v1/projects/{project_id}/active-design:render-preset"
        request_hash = _hash_json(
            {
                "project_id": project_id,
                "expected_revision": expected_revision,
                "request": request.model_dump(mode="json"),
            }
        )
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ActiveDesignIdempotencyConflict(
                        "Idempotency-Key was reused with a different render preset request."
                    )
                return ActiveDesignSnapshot.model_validate_json(replay.response_json)

            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if snapshot.revision != expected_revision:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再调整视图。",
                    status_code=409,
                )
            if isinstance(snapshot.active_design, LegacyActiveDesignReference):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                    "当前是 legacy 只读设计；请先让 Agent 重建设计资产。",
                    status_code=409,
                )
            try:
                result = unit.active_designs.set_render_preset(
                    project_id=project_id,
                    expected_revision=expected_revision,
                    camera_view=request.camera_view,
                    light_preset=request.light_preset,
                    updated_at=_utc_now(),
                )
            except ActiveDesignSnapshotConflict as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再调整视图。",
                    status_code=409,
                ) from exc
            except ActiveDesignSnapshotError as exc:
                raise ActiveDesignApiError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(result.model_dump(mode="json")),
                created_at=result.updated_at,
            )
            return result

    def set_part_display(
        self,
        project_id: str,
        request: SetActiveDesignPartDisplayRequest,
        *,
        expected_revision: int,
        idempotency_key: str,
    ) -> ActiveDesignSnapshot:
        scope = f"POST /api/v1/projects/{project_id}/active-design:part-display"
        request_hash = _hash_json(
            {
                "project_id": project_id,
                "expected_revision": expected_revision,
                "request": request.model_dump(mode="json"),
            }
        )
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ActiveDesignIdempotencyConflict(
                        "Idempotency-Key was reused with a different part display request."
                    )
                return ActiveDesignSnapshot.model_validate_json(replay.response_json)
            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if snapshot.revision != expected_revision:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再调整部件显示。",
                    status_code=409,
                )
            if isinstance(snapshot.active_design, LegacyActiveDesignReference):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                    "当前是 legacy 只读设计；请先让 Agent 重建设计资产。",
                    status_code=409,
                )
            try:
                result = unit.active_designs.set_part_display(
                    project_id=project_id,
                    expected_revision=expected_revision,
                    action=request.action,
                    part_id=request.part_id,
                    updated_at=_utc_now(),
                )
            except ActiveDesignSnapshotConflict as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再调整部件显示。",
                    status_code=409,
                ) from exc
            except ActiveDesignSnapshotError as exc:
                raise ActiveDesignApiError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(result.model_dump(mode="json")),
                created_at=result.updated_at,
            )
            return result

    def convert_legacy(
        self,
        project_id: str,
        request: ConvertLegacyActiveDesignRequest,
        *,
        expected_revision: int,
        idempotency_key: str,
    ) -> LegacyActiveDesignConversionResponse:
        scope = f"POST /api/v1/projects/{project_id}/active-design:convert-legacy"
        request_hash = _hash_json(
            {
                "project_id": project_id,
                "expected_revision": expected_revision,
                "request": request.model_dump(mode="json"),
            }
        )
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ActiveDesignIdempotencyConflict(
                        "Idempotency-Key was reused with a different legacy conversion request."
                    )
                return LegacyActiveDesignConversionResponse.model_validate_json(replay.response_json)

            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if snapshot.revision != expected_revision:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后重新发起转换。",
                    status_code=409,
                )
            if not isinstance(snapshot.active_design, LegacyActiveDesignReference):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_NOT_LEGACY",
                    "当前活动设计已经是 Agent 资产，无需转换。",
                    status_code=409,
                )
            try:
                unit.active_designs.record_legacy_conversion_intent(
                    project_id=project_id,
                    expected_revision=expected_revision,
                    legacy_version_id=snapshot.active_design.legacy_version_id,
                    module_graph_id=snapshot.active_design.module_graph_id,
                    requested_at=_utc_now(),
                )
            except ActiveDesignSnapshotConflict as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后重新发起转换。",
                    status_code=409,
                ) from exc
            except ActiveDesignSnapshotError as exc:
                raise ActiveDesignApiError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            result = LegacyActiveDesignConversionResponse(
                project_id=project_id,
                source=snapshot.active_design,
                snapshot_revision=snapshot.revision,
                message="已准备 legacy 只读设计的 Agent 重建输入；原 Concept 版本和模块图不会被修改。",
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(result.model_dump(mode="json")),
                created_at=_utc_now(),
            )
            return result

    def navigate_asset(
        self,
        project_id: str,
        request: NavigateActiveDesignRequest,
        *,
        expected_revision: int,
        idempotency_key: str,
        action: str,
    ) -> ActiveDesignSnapshot:
        """Create a new immutable version for a server-owned undo or redo.

        Historic versions are never reactivated in place.  Instead the target
        content becomes a new committed child, and a small navigation frame
        preserves the next undo/redo targets.  This keeps the head, Snapshot,
        selection, preview and quality transition transactional and auditable.
        """
        if action not in {"undo", "redo"}:
            raise ValueError(f"unsupported navigation action: {action}")
        scope = f"POST /api/v1/projects/{project_id}/active-design:{action}"
        request_hash = _hash_json(
            {
                "project_id": project_id,
                "expected_revision": expected_revision,
                "action": action,
                "request": request.model_dump(mode="json"),
            }
        )
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise ActiveDesignIdempotencyConflict(
                        "Idempotency-Key was reused with a different version navigation request."
                    )
                return ActiveDesignSnapshot.model_validate_json(replay.response_json)

            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if snapshot.revision != expected_revision:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再撤销或重做。",
                    status_code=409,
                )
            if not hasattr(snapshot.active_design, "asset_version_id"):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                    "当前是 legacy 只读设计；请先让 Agent 重建设计资产。",
                    status_code=409,
                )
            if snapshot.preview is not None:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_PREVIEW_PENDING",
                    "请先确认或取消当前修改预览，再撤销或重做。",
                    status_code=409,
                )

            current_id = snapshot.active_design.asset_version_id
            current = unit.agent_assets.get_version(current_id)
            head = unit.agent_assets.get_head(project_id)
            if (
                current is None
                or str(current["project_id"]) != project_id
                or str(current["status"]) != "committed"
                or head is None
                or str(head["asset_version_id"]) != current_id
            ):
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_HEAD_INVALID",
                    "当前可编辑资产版本不一致，请刷新后重试。",
                    status_code=409,
                )

            current_undo, current_redo = _navigation_targets(unit, current)
            target_id = current_undo if action == "undo" else current_redo
            if not target_id:
                code = "ACTIVE_DESIGN_UNDO_UNAVAILABLE" if action == "undo" else "ACTIVE_DESIGN_REDO_UNAVAILABLE"
                message = "没有可返回的上一版本。" if action == "undo" else "没有可重做的版本。"
                raise ActiveDesignApiError(code, message, status_code=409)
            target = unit.agent_assets.get_version(target_id)
            if target is None or str(target["project_id"]) != project_id:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_NAVIGATION_INVALID",
                    "版本历史不完整，无法安全切换。",
                    status_code=409,
                )
            try:
                target_graph = json.loads(str(target["assembly_graph_json"]))
            except json.JSONDecodeError as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_NAVIGATION_INVALID",
                    "目标版本缺少有效装配图，无法安全切换。",
                    status_code=409,
                ) from exc
            graph_id = target_graph.get("graph_id") if isinstance(target_graph, dict) else None
            if not isinstance(graph_id, str) or not graph_id:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_NAVIGATION_INVALID",
                    "目标版本缺少活动装配图 ID，无法安全切换。",
                    status_code=409,
                )

            now = _utc_now()
            resulting_id = _new_asset_version_id()
            target_summary = str(target["summary"])
            label = "撤销至" if action == "undo" else "重做至"
            unit.agent_assets.add_version(
                asset_version_id=resulting_id,
                project_id=project_id,
                # The semantic parent is the content being restored, not the
                # transient head being superseded.  This keeps repeated undo
                # traversal independent from monotonically increasing IDs.
                parent_asset_version_id=str(target["asset_version_id"]),
                version_no=unit.agent_assets.next_version_number(project_id),
                status="committed",
                summary=f"{label} v{int(target['version_no'])}：{target_summary}",
                stage=str(target["stage"]),
                plan_id=str(target["plan_id"]),
                direction_id=str(target["direction_id"]),
                domain_pack_id=str(target["domain_pack_id"]),
                artifact_id=str(target["artifact_id"]),
                parts_json=str(target["parts_json"]),
                shape_program_json=str(target["shape_program_json"]),
                assembly_graph_json=str(target["assembly_graph_json"]),
                material_bindings_json=str(target["material_bindings_json"]),
                created_at=now,
            )
            unit.agent_assets.supersede(current_id)
            unit.agent_assets.set_head(project_id=project_id, asset_version_id=resulting_id, updated_at=now)
            try:
                result = unit.active_designs.advance_agent_snapshot(
                    project_id=project_id,
                    expected_revision=snapshot.revision,
                    asset_version_id=resulting_id,
                    assembly_graph_id=graph_id,
                    updated_at=now,
                )
            except ActiveDesignSnapshotConflict as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_STALE",
                    "活动设计已更新，请刷新后再撤销或重做。",
                    status_code=409,
                ) from exc
            except ActiveDesignSnapshotError as exc:
                raise ActiveDesignApiError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc

            target_undo, target_redo = _navigation_targets(unit, target)
            unit.agent_assets.add_navigation_frame(
                resulting_asset_version_id=resulting_id,
                project_id=project_id,
                undo_target_asset_version_id=target_undo,
                redo_target_asset_version_id=current_id if action == "undo" else target_redo,
                action=action,
                created_at=now,
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(result.model_dump(mode="json")),
                created_at=now,
            )
            return result

    def get_navigation(self, project_id: str) -> ActiveDesignNavigation:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            snapshot = self._get_or_bootstrap_snapshot(unit, project_id)
            if not hasattr(snapshot.active_design, "asset_version_id"):
                return ActiveDesignNavigation(
                    project_id=project_id,
                    active_asset_version_id=None,
                    can_undo=False,
                    can_redo=False,
                )
            version = unit.agent_assets.get_version(snapshot.active_design.asset_version_id)
            if version is None or str(version["project_id"]) != project_id:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_HEAD_INVALID",
                    "当前可编辑资产版本不一致，请刷新后重试。",
                    status_code=409,
                )
            undo_target, redo_target = _navigation_targets(unit, version)
            return ActiveDesignNavigation(
                project_id=project_id,
                active_asset_version_id=snapshot.active_design.asset_version_id,
                can_undo=undo_target is not None and snapshot.preview is None,
                can_redo=redo_target is not None and snapshot.preview is None,
                preview_pending=snapshot.preview is not None,
            )

    def _get_or_bootstrap_snapshot(
        self,
        unit: SQLiteUnitOfWork,
        project_id: str,
    ) -> ActiveDesignSnapshot:
        existing = unit.active_designs.get_snapshot(project_id)
        if existing is not None:
            return existing

        project = unit.concept_projects.get_active(project_id)
        if project is None:
            raise ActiveDesignApiError("PROJECT_NOT_FOUND", "项目不存在或已归档。", status_code=404)

        # A valid Agent head is the newest supported workbench source.  A
        # legacy current version is only used when an Agent head does not yet
        # exist, so UI never merges two unrelated version chains.
        head = unit.agent_assets.get_head(project_id)
        if head is not None:
            version = unit.agent_assets.get_version(str(head["asset_version_id"]))
            if version is None or str(version["project_id"]) != project_id or str(version["status"]) != "committed":
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_HEAD_INVALID",
                    "Agent 资产头指向不可用版本，无法初始化工作台。",
                    status_code=409,
                )
            try:
                graph = json.loads(str(version["assembly_graph_json"]))
            except json.JSONDecodeError as exc:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_HEAD_INVALID",
                    "Agent 资产头缺少有效装配图。",
                    status_code=409,
                ) from exc
            graph_id = graph.get("graph_id") if isinstance(graph, dict) else None
            if not isinstance(graph_id, str) or not graph_id:
                raise ActiveDesignApiError(
                    "ACTIVE_DESIGN_HEAD_INVALID",
                    "Agent 资产头缺少活动装配图 ID。",
                    status_code=409,
                )
            return self._create_agent_snapshot_if_absent(
                unit,
                project_id=project_id,
                asset_version_id=str(version["asset_version_id"]),
                assembly_graph_id=graph_id,
            )

        current_version_id = project["current_version_id"]
        if current_version_id:
            version = unit.concept_projects.get_version(project_id, str(current_version_id))
            graph_id = str(version["module_graph_id"] or "") if version is not None else ""
            if version is not None and graph_id:
                return self._create_legacy_snapshot_if_absent(
                    unit,
                    project_id=project_id,
                    legacy_version_id=str(version["version_id"]),
                    module_graph_id=graph_id,
                )

        raise ActiveDesignApiError(
            "ACTIVE_DESIGN_NOT_FOUND",
            "这个项目还没有可打开的 Agent 资产或 legacy Concept 版本。",
            status_code=404,
        )

    @staticmethod
    def _create_agent_snapshot_if_absent(
        unit: SQLiteUnitOfWork,
        *,
        project_id: str,
        asset_version_id: str,
        assembly_graph_id: str,
    ) -> ActiveDesignSnapshot:
        try:
            return unit.active_designs.create_agent_snapshot(
                project_id=project_id,
                asset_version_id=asset_version_id,
                assembly_graph_id=assembly_graph_id,
                updated_at=_utc_now(),
            )
        except ActiveDesignSnapshotConflict:
            snapshot = unit.active_designs.get_snapshot(project_id)
            if snapshot is not None:
                return snapshot
            raise ActiveDesignApiError("ACTIVE_DESIGN_BOOTSTRAP_FAILED", "无法初始化活动设计。", status_code=409)
        except ActiveDesignSnapshotError as exc:
            raise ActiveDesignApiError("ACTIVE_DESIGN_HEAD_INVALID", str(exc), status_code=409) from exc

    @staticmethod
    def _create_legacy_snapshot_if_absent(
        unit: SQLiteUnitOfWork,
        *,
        project_id: str,
        legacy_version_id: str,
        module_graph_id: str,
    ) -> ActiveDesignSnapshot:
        try:
            return unit.active_designs.create_legacy_snapshot(
                project_id=project_id,
                legacy_version_id=legacy_version_id,
                module_graph_id=module_graph_id,
                updated_at=_utc_now(),
            )
        except ActiveDesignSnapshotConflict:
            snapshot = unit.active_designs.get_snapshot(project_id)
            if snapshot is not None:
                return snapshot
            raise ActiveDesignApiError("ACTIVE_DESIGN_BOOTSTRAP_FAILED", "无法初始化活动设计。", status_code=409)
        except ActiveDesignSnapshotError as exc:
            raise ActiveDesignApiError("ACTIVE_DESIGN_LEGACY_INVALID", str(exc), status_code=409) from exc


def _hash_json(payload: dict[str, Any]) -> str:
    return hashlib.sha256(_canonical_json(payload).encode("utf-8")).hexdigest()


def _canonical_json(payload: Any) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _new_asset_version_id() -> str:
    return f"assetver_restore_{uuid.uuid4().hex[:16]}"


def _navigation_targets(unit: SQLiteUnitOfWork, version: Any) -> tuple[str | None, str | None]:
    """Return logical history targets without making the version row mutable."""
    frame = unit.agent_assets.get_navigation_frame(str(version["asset_version_id"]))
    if frame is None:
        return (
            str(version["parent_asset_version_id"]) if version["parent_asset_version_id"] else None,
            None,
        )
    return (
        str(frame["undo_target_asset_version_id"]) if frame["undo_target_asset_version_id"] else None,
        str(frame["redo_target_asset_version_id"]) if frame["redo_target_asset_version_id"] else None,
    )


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
