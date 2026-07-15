from __future__ import annotations

import base64
import hashlib
import json
import math
import struct
import uuid
import zipfile
from datetime import datetime, timezone
from io import BytesIO
from typing import Any, Mapping, Optional

from pydantic import ValidationError

from forgecad_agent.application.agent_models import (
    AgentAssetChangeSet,
    AgentAssetChangeSetConfirmResponse,
    AgentAssetExportResponse,
    AgentAssetQualityFinding,
    AgentAssetQualityReport,
    AgentAssetRenderPackageManifest,
    AgentAssetRenderPackageView,
    AgentAssetRenderSet,
    AgentAssetRenderView,
    AgentAssetVersion,
    AgentComponentCandidate,
    AgentComponentCompatibility,
    AgentComponentRecord,
    AgentPartEditOperation,
    AgentStructureSuggestion,
    AgentStructureSuggestionList,
    BlockoutPartCandidate,
    CommitAgentBlockoutRequest,
    ImportAgentGlbRequest,
    ImportAgentGlbResponse,
    ImportedGlbInspectionResponse,
    ProposeAgentAssetChangeSetRequest,
    ResolvedSemanticProportionOption,
    ResolvedSemanticProportionOptions,
    SaveAgentComponentRequest,
)
from forgecad_agent.application.material_catalog import material_preset_map
from forgecad_agent.application.geometry_worker import (
    GeometryCompileReadbackError,
    build_glb_from_shape_program,
    compile_shape_program,
    inspect_imported_glb,
)
from forgecad_agent.application.agent_rendering import AgentRenderError, ExplodedPartOffset, render_agent_views
from forgecad_agent.application.domain_packs import list_domain_packs
from forgecad_agent.application.shape_program import ShapeProgramValidationError, validate_shape_program
from forgecad_agent.application.shape_program_runtime import UnsupportedRuntimeOperationError
from forgecad_agent.application.semantic_proportions import part_id_for_role_selector, recipes_for_domain, style_token_map
from forgecad_agent.infrastructure.db.agent_repositories import (
    ActiveDesignSnapshotConflict,
    ActiveDesignSnapshotError,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage.content_addressed_store import ContentAddressedStore, ObjectStoreError
from .manifold_csg import ManifoldCsgError


class AgentAssetError(RuntimeError):
    def __init__(self, code: str, message: str, *, status_code: int = 400) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code


class AgentAssetIdempotencyConflict(RuntimeError):
    pass


def _runtime_operation_rejected(exc: Exception) -> AgentAssetError:
    """Expose one stable API code for every refused ShapeProgram runtime path."""

    if isinstance(exc, ManifoldCsgError):
        return AgentAssetError(exc.code, str(exc), status_code=409)
    message = str(exc)
    if message.startswith("CSG_"):
        return AgentAssetError(message.split(":", 1)[0].split(" ", 1)[0], message, status_code=409)
    return AgentAssetError("UNSUPPORTED_RUNTIME_OPERATION", str(exc), status_code=409)


def _geometry_readback_failed(exc: Exception) -> AgentAssetError:
    return AgentAssetError("GEOMETRY_READBACK_FAILED", str(exc), status_code=409)


def _quality_request_hash(asset_version_id: str, expected_revision: Optional[int]) -> str:
    # The contract marker prevents an idempotency replay created by the former
    # estimate-based implementation from being mistaken for Q003 evidence.
    return _hash_json({
        "asset_version_id": asset_version_id,
        "expected_revision": expected_revision,
        "quality_contract": "GeometryCompileReadback@1",
    })


class AgentAssetEditingService:
    """Persist Agent blockouts and apply bounded, preview-first asset edits."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: Optional[ContentAddressedStore] = None,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def commit_blockout(
        self,
        request: CommitAgentBlockoutRequest,
        idempotency_key: str,
    ) -> AgentAssetVersion:
        scope = "POST /api/v1/agent/blockouts:commit"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different commit request.")
                return AgentAssetVersion.model_validate_json(replay.response_json)
            row = unit.agent_assets.get_candidate(request.artifact_id)
            if row is None:
                raise AgentAssetError("BLOCKOUT_NOT_FOUND", "分件候选不存在或已清理。", status_code=404)
            if row["status"] != "candidate":
                raise AgentAssetError("BLOCKOUT_STATE_CONFLICT", "该候选已经确认或被放弃。", status_code=409)
            candidate_project_id = str(row["project_id"] or "")
            requested_project_id = str(request.project_id or "")
            if candidate_project_id and requested_project_id and candidate_project_id != requested_project_id:
                raise AgentAssetError("PROJECT_MISMATCH", "候选所属项目与当前项目不一致。", status_code=409)
            project_id = candidate_project_id or requested_project_id
            if not project_id or unit.concept_projects.get_active(project_id) is None:
                raise AgentAssetError("PROJECT_REQUIRED", "请从一个已打开的项目确认该候选。", status_code=409)
            candidate = json.loads(row["candidate_json"])
            shape_program = json.loads(row["shape_program_json"])
            try:
                build_glb_from_shape_program(shape_program)
            except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
                raise _runtime_operation_rejected(exc) from exc
            version = _new_asset_version(
                project_id=project_id,
                parent=None,
                version_no=unit.agent_assets.next_version_number(project_id),
                summary=request.summary,
                stage="segmented_concept",
                plan_id=str(row["plan_id"]),
                direction_id=str(row["direction_id"]),
                domain_pack_id=str(row["domain_pack_id"]),
                artifact_id=str(row["artifact_id"]),
                parts=candidate["parts"],
                shape_program=shape_program,
                assembly_graph=json.loads(row["assembly_graph_json"]),
                material_bindings=json.loads(row["material_bindings_json"]),
            )
            unit.agent_assets.add_version(**_version_row(version))
            unit.agent_assets.set_head(project_id=project_id, asset_version_id=version.asset_version_id, updated_at=version.created_at)
            _sync_agent_snapshot(unit, version=version, updated_at=version.created_at)
            unit.agent_assets.mark_candidate(str(row["artifact_id"]), status="committed", updated_at=version.created_at)
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(version.model_dump(mode="json")),
                created_at=version.created_at,
            )
            return version

    def get_version(self, asset_version_id: str) -> AgentAssetVersion:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            return _version_from_row(row)

    def list_semantic_proportions(
        self,
        asset_version_id: str,
        *,
        part_id: str,
    ) -> ResolvedSemanticProportionOptions:
        """Resolve visual recipes against active Snapshot, G808 bindings and GLB readback.

        This read never creates a version. A returned option is merely a safe
        input for the existing preview-first ``set_part_parameter`` ChangeSet.
        Missing evidence produces an explicit empty state instead of guessing.
        """
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            version = _version_from_row(row)
            locked_part_ids = _active_snapshot_locked_part_ids(unit, version, purpose="读取外观比例配方")
            if _is_external_glb_reference(version):
                raise AgentAssetError(
                    "EXTERNAL_REFERENCE_NOT_EDITABLE",
                    "导入 GLB 当前仅作为参考模型；请让 Agent 重建后再使用外观比例配方。",
                    status_code=409,
                )
            part = next((item for item in version.parts if item.part_id == part_id), None)
            if part is None:
                raise AgentAssetError("PART_NOT_FOUND", "找不到要调整的部件。", status_code=404)
            try:
                compiled = compile_shape_program(version.shape_program)
            except GeometryCompileReadbackError as exc:
                raise _geometry_readback_failed(exc) from exc
            except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
                raise _runtime_operation_rejected(exc) from exc

            readback = compiled.readback
            surface_facts = [
                raw.model_dump(mode="json") if hasattr(raw, "model_dump") else raw
                for raw in readback.surface_provenance
            ]
            surface_facts = [
                item for item in surface_facts
                if item.get("part_role") == part.role
                and item.get("material_zone_id") in set(part.material_zone_ids)
                and item.get("texture_ready") is True
            ]
            source_operation_ids = sorted({
                str(operation_id)
                for item in surface_facts
                for operation_id in item.get("source_operation_ids", [])
                if isinstance(operation_id, str)
            })
            locked = part_id in locked_part_ids or part.locked
            bindings = {
                binding.path: binding for binding in part.editable_parameter_bindings
                if binding.unit == "ratio" and binding.path.startswith("transform.scale.")
            }
            graph_part = next((
                item for item in version.assembly_graph.get("parts", [])
                if isinstance(item, dict) and item.get("part_id") == part_id
            ), None)
            graph_scale = (
                graph_part.get("transform", {}).get("scale", [1.0, 1.0, 1.0])
                if isinstance(graph_part, dict) else [1.0, 1.0, 1.0]
            )
            tokens = style_token_map()
            options: list[ResolvedSemanticProportionOption] = []
            if source_operation_ids and not locked:
                for recipe in recipes_for_domain(version.domain_pack_id):
                    for adjustment in recipe.adjustments:
                        if part_id_for_role_selector(version.parts, version.assembly_graph, adjustment.role_selector) != part.part_id:
                            continue
                        binding = bindings.get(adjustment.path)
                        token = tokens.get(recipe.style_token_id)
                        if binding is None or token is None or version.domain_pack_id not in token.allowed_domains:
                            continue
                        axis = "xyz".index(adjustment.path.rsplit(".", 1)[1])
                        current = float(graph_scale[axis]) if isinstance(graph_scale, list) and len(graph_scale) == 3 else binding.default
                        if not math.isfinite(current) or current < binding.min - 1e-9 or current > binding.max + 1e-9:
                            continue
                        target = round(current + binding.step * adjustment.step_delta, 10)
                        if target < binding.min - 1e-9 or target > binding.max + 1e-9:
                            continue
                        options.append(ResolvedSemanticProportionOption(
                            recipe_id=recipe.recipe_id,
                            style_token=token,
                            display_name=recipe.display_name,
                            description=recipe.description,
                            path=adjustment.path,
                            current_value=current,
                            target_value=target,
                            min=binding.min,
                            max=binding.max,
                            step=binding.step,
                            source_operation_ids=source_operation_ids,
                        ))

            if locked:
                unavailable = "该部件已锁定。解除锁定后才能预览外观比例配方。"
            elif not source_operation_ids:
                unavailable = "真实编译结果没有找到该部件的稳定表面来源，未提供比例配方。"
            elif not bindings:
                unavailable = "该部件没有受限比例参数，Agent 不会猜测或创建自由参数。"
            elif not options:
                unavailable = "当前部件没有适用且仍在安全范围内的领域比例配方。"
            else:
                unavailable = None
            return ResolvedSemanticProportionOptions(
                asset_version_id=version.asset_version_id,
                part_id=part.part_id,
                domain_pack_id=version.domain_pack_id,
                shape_program_sha256=readback.shape_program_sha256,
                glb_sha256=readback.glb_sha256,
                locked=locked,
                options=options,
                unavailable_message=unavailable,
            )

    def list_structure_suggestions(self, asset_version_id: str) -> AgentStructureSuggestionList:
        """Return evidence-bound split/merge ideas without creating a version.

        The current light geometry language cannot safely infer arbitrary mesh
        cuts.  We therefore only suggest a split for independent primitive
        outputs with no connection facts, or a merge for a directly connected,
        leaf AssemblyGraph part whose geometry/material facts can be preserved.
        """
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            version = _version_from_row(row)
            locked_part_ids = _active_snapshot_locked_part_ids(unit, version, purpose="查看部件结构建议")
            if _is_external_glb_reference(version):
                raise AgentAssetError(
                    "EXTERNAL_REFERENCE_NOT_EDITABLE",
                    "导入 GLB 当前仅作为参考模型；请让 Agent 重建为可编辑概念资产。",
                    status_code=409,
                )
            suggestions = _structure_suggestions_for(version, locked_part_ids=locked_part_ids)
            return AgentStructureSuggestionList(
                asset_version_id=version.asset_version_id,
                suggestions=suggestions,
                unavailable_message=None if suggestions else "当前模型没有足够的装配和几何事实，或相关部件已锁定，暂不能建议拆分或合并部件。",
            )

    def import_glb(
        self,
        request: ImportAgentGlbRequest,
        idempotency_key: str,
    ) -> ImportAgentGlbResponse:
        """Store a verified user GLB as an immutable reference asset.

        It is intentionally *not* converted to ShapeProgram or presented as
        editable mechanical geometry.  A later Agent turn may use it as a
        visual reference and rebuild a clean editable asset with user approval.
        """
        scope = "POST /api/v1/agent/imports:glb"
        request_hash = _hash_json({
            "project_id": request.project_id,
            "domain_pack_id": request.domain_pack_id,
            "file_name": request.file_name,
            "summary": request.summary,
            "glb_sha256": hashlib.sha256(request.glb_base64.encode("utf-8")).hexdigest(),
        })
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different GLB import.")
                return ImportAgentGlbResponse.model_validate_json(replay.response_json)
            if self.object_store is None:
                raise AgentAssetError("OBJECT_STORE_UNAVAILABLE", "本机对象存储不可用，无法导入 GLB。", status_code=503)
            if unit.concept_projects.get_active(request.project_id) is None:
                raise AgentAssetError("PROJECT_REQUIRED", "请先打开一个项目，再导入 GLB 参考模型。", status_code=404)
            allowed_packs = {item.pack_id for item in list_domain_packs()}
            if request.domain_pack_id not in allowed_packs:
                raise AgentAssetError("DOMAIN_PACK_UNKNOWN", "导入模型必须归入一个已注册领域包。")
            try:
                payload = base64.b64decode(request.glb_base64, validate=True)
            except (ValueError, TypeError) as exc:
                raise AgentAssetError("GLB_BASE64_INVALID", "导入内容不是有效的 GLB Base64 数据。") from exc
            try:
                inspection = inspect_imported_glb(payload)
                stored = self.object_store.put(payload, extension=".glb")
            except (ValueError, ObjectStoreError) as exc:
                raise AgentAssetError("GLB_IMPORT_REJECTED", str(exc)) from exc
            if stored.sha256 != inspection.sha256:
                raise AgentAssetError("GLB_HASH_MISMATCH", "导入 GLB 的内容校验失败。", status_code=409)
            now = _utc_now()
            artifact_id = f"artifact_import_{inspection.sha256[:16]}"
            part_id = "part_1_imported_model"
            bounds = [max(float(value), 0.1) for value in inspection.bounds_mm]
            version = _new_asset_version(
                project_id=request.project_id,
                parent=None,
                version_no=unit.agent_assets.next_version_number(request.project_id),
                summary=request.summary,
                stage="segmented_concept",
                plan_id="external_glb_import",
                direction_id="external_reference",
                domain_pack_id=request.domain_pack_id,
                artifact_id=artifact_id,
                parts=[
                    BlockoutPartCandidate(
                        part_id=part_id,
                        role="primary_body",
                        parent_part_id=None,
                        position_mm=[0, 0, 0],
                        size_mm=bounds,
                        material_zone_ids=["zone_imported_model"],
                        editable_parameters=[],
                        locked=False,
                        provenance="imported_glb",
                    ).model_dump(mode="json")
                ],
                shape_program={
                    "schema_version": "ExternalGLBReference@1",
                    "source_sha256": inspection.sha256,
                    "editable": False,
                    "reason": "Imported GLB is reference-only until rebuilt as a ShapeProgram asset.",
                },
                assembly_graph={
                    "schema_version": "AssemblyGraph@1",
                    "graph_id": f"mg_import_{inspection.sha256[:16]}",
                    "source_kind": "external_glb_reference",
                    "parts": [{
                        "part_id": part_id,
                        "role": "primary_body",
                        "transform": {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]},
                        "connectors": [],
                        "joints": [],
                    }],
                    "connections": [],
                },
                material_bindings={},
            )
            unit.agent_assets.add_version(**_version_row(version))
            unit.agent_assets.add_imported_glb(
                import_id=_new_id("glbimport"),
                project_id=request.project_id,
                asset_version_id=version.asset_version_id,
                domain_pack_id=request.domain_pack_id,
                file_name=_safe_import_file_name(request.file_name),
                object_path=stored.relative_path,
                sha256=inspection.sha256,
                byte_size=inspection.byte_size,
                triangle_count=inspection.triangle_count,
                bounds_mm_json=_canonical_json(inspection.bounds_mm),
                mesh_count=inspection.mesh_count,
                primitive_count=inspection.primitive_count,
                material_count=inspection.material_count,
                node_count=inspection.node_count,
                created_at=now,
            )
            unit.agent_assets.set_head(project_id=request.project_id, asset_version_id=version.asset_version_id, updated_at=now)
            _sync_agent_snapshot(unit, version=version, updated_at=now)
            response = ImportAgentGlbResponse(
                asset_version=version,
                inspection=ImportedGlbInspectionResponse(
                    sha256=inspection.sha256,
                    byte_size=inspection.byte_size,
                    triangle_count=inspection.triangle_count,
                    bounds_mm=inspection.bounds_mm,
                    mesh_count=inspection.mesh_count,
                    primitive_count=inspection.primitive_count,
                    material_count=inspection.material_count,
                    node_count=inspection.node_count,
                ),
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def save_component(
        self,
        asset_version_id: str,
        request: SaveAgentComponentRequest,
        idempotency_key: str,
    ) -> AgentComponentRecord:
        scope = f"POST /api/v1/agent/asset-versions/{asset_version_id}/components"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different component request.")
                return AgentComponentRecord.model_validate_json(replay.response_json)
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            if row["status"] != "committed":
                raise AgentAssetError("ASSET_VERSION_STALE", "只能从当前可编辑版本保存部件。", status_code=409)
            version = _version_from_row(row)
            if _is_external_glb_reference(version):
                raise AgentAssetError("EXTERNAL_REFERENCE_NOT_EDITABLE", "导入 GLB 当前仅作为参考模型；请让 Agent 重建为可编辑概念资产。", status_code=409)
            part = next((item for item in version.parts if item.part_id == request.part_id), None)
            if part is None:
                raise AgentAssetError("PART_NOT_FOUND", "找不到要保存的部件。", status_code=404)
            shape_operation = next(
                (item for item in version.shape_program.get("operations", [])
                 if isinstance(item, dict) and item.get("args", {}).get("part_role") == part.role),
                None,
            )
            if shape_operation is None:
                raise AgentAssetError("PART_GEOMETRY_NOT_FOUND", "该部件没有可复用的概念几何。", status_code=409)
            now = _utc_now()
            record = AgentComponentRecord(
                component_id=_new_id("agentcomp"),
                project_id=version.project_id,
                domain_pack_id=version.domain_pack_id,
                role=part.role,
                display_name=request.display_name,
                description=request.description,
                source_asset_version_id=version.asset_version_id,
                source_part_id=part.part_id,
                part_template=part,
                shape_operation=json.loads(json.dumps(shape_operation)),
                material_bindings={key: value for key, value in version.material_bindings.items() if key.startswith(f"{part.part_id}:")},
                created_at=now,
                updated_at=now,
            )
            unit.agent_assets.add_component(
                component_id=record.component_id,
                project_id=record.project_id,
                domain_pack_id=record.domain_pack_id,
                role=record.role,
                display_name=record.display_name,
                description=record.description,
                source_asset_version_id=record.source_asset_version_id,
                source_part_id=record.source_part_id,
                part_template_json=_canonical_json(record.part_template.model_dump(mode="json")),
                shape_operation_json=_canonical_json(record.shape_operation),
                material_bindings_json=_canonical_json(record.material_bindings),
                created_at=now,
            )
            unit.idempotency.add(scope=scope, key=idempotency_key, request_hash=request_hash, response_json=_canonical_json(record.model_dump(mode="json")), created_at=now)
            return record

    def list_components(
        self,
        project_id: str,
        *,
        domain_pack_id: Optional[str] = None,
        role: Optional[str] = None,
        query: Optional[str] = None,
    ) -> list[AgentComponentRecord]:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            return [_component_from_row(row) for row in unit.agent_assets.list_components(project_id, domain_pack_id=domain_pack_id, role=role, query=query)]

    def list_component_candidates(
        self,
        asset_version_id: str,
        *,
        part_id: str,
    ) -> list[AgentComponentCandidate]:
        """List the local workspace components with an auditable replacement decision.

        AgentComponent is not the formal Module Asset catalog: it has no review
        state and never claims one.  Eligibility therefore uses only actual local
        facts: component activation, same domain/role, and the latest quality
        result of the immutable source asset.  Replacement keeps the target
        AssemblyGraph identity and its connectors, rather than inventing a source
        connector contract that the component snapshot does not contain.
        """
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            version = _version_from_row(row)
            if _is_external_glb_reference(version):
                raise AgentAssetError(
                    "EXTERNAL_REFERENCE_NOT_EDITABLE",
                    "导入 GLB 当前仅作为参考模型；请让 Agent 重建后再查询可替换部件。",
                    status_code=409,
                )
            target = next((item for item in version.parts if item.part_id == part_id), None)
            if target is None:
                raise AgentAssetError("PART_NOT_FOUND", "找不到要替换的部件。", status_code=404)
            components = [
                _component_from_row(component_row)
                for component_row in unit.agent_assets.list_components(
                    version.project_id,
                    include_disabled=True,
                )
            ]
            return [
                AgentComponentCandidate(
                    component=component,
                    compatibility=_component_compatibility(version, target, component),
                )
                for component in components
            ]

    def quality(
        self,
        asset_version_id: str,
        *,
        expected_revision: Optional[int] = None,
        idempotency_key: Optional[str] = None,
    ) -> AgentAssetQualityReport:
        """Run a Snapshot-bound quality check with optional HTTP replay data.

        Internal migration fixtures may omit both optional fields.  The public
        route always supplies both, so a successful retry returns the same
        immutable report before its own Snapshot revision is checked again.
        """
        if (expected_revision is None) != (idempotency_key is None):
            raise ValueError("quality expected_revision and idempotency_key must be provided together")
        if idempotency_key is not None:
            scope = f"POST /api/v1/agent/asset-versions/{asset_version_id}:quality"
            request_hash = _quality_request_hash(asset_version_id, expected_revision)
            with SQLiteUnitOfWork(self.connection_factory) as unit:
                replay = unit.idempotency.get(scope, idempotency_key)
                if replay is not None:
                    if replay.request_hash != request_hash:
                        raise AgentAssetIdempotencyConflict(
                            "Idempotency-Key was reused with a different quality request."
                        )
                    return AgentAssetQualityReport.model_validate_json(replay.response_json)
        version = self.get_version(asset_version_id)
        imported = self._imported_glb_row(asset_version_id)
        if imported is not None:
            return self._persist_quality_report(version, AgentAssetQualityReport(
                quality_report_id=_new_id("quality"),
                asset_version_id=version.asset_version_id,
                status="warning",
                triangle_count=int(imported["triangle_count"]),
                bounds_mm=json.loads(str(imported["bounds_mm_json"])),
                evidence_source="external_glb_inspection",
                findings=[AgentAssetQualityFinding(
                    check_id="external_glb_reference",
                    severity="warning",
                    message="已验证为自包含 GLB 参考模型；请让 Agent 重建后再进行部件级编辑、连接器或关节检查。",
                    part_ids=[version.parts[0].part_id] if version.parts else [],
                )],
                checked_at=_utc_now(),
            ), expected_revision=expected_revision, idempotency_key=idempotency_key)
        try:
            compiled = compile_shape_program(version.shape_program)
        except GeometryCompileReadbackError as exc:
            return self._persist_quality_report(version, AgentAssetQualityReport(
                quality_report_id=_new_id("quality"),
                asset_version_id=version.asset_version_id,
                status="unavailable",
                triangle_count=0,
                bounds_mm=None,
                evidence_source="compile_failure",
                findings=[AgentAssetQualityFinding(
                    check_id="geometry_compile_readback",
                    severity="error",
                    message=f"真实 GLB 回读失败，未生成质量数字：{exc}",
                )],
                checked_at=_utc_now(),
            ), expected_revision=expected_revision, idempotency_key=idempotency_key)
        except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
            raise _runtime_operation_rejected(exc) from exc
        compile_readback = compiled.readback
        findings: list[AgentAssetQualityFinding] = []
        part_ids = [part.part_id for part in version.parts]
        part_set = set(part_ids)
        if len(part_ids) != len(part_set):
            findings.append(AgentAssetQualityFinding(check_id="unique_part_ids", severity="error", message="部件 ID 重复。"))
        roots = [part for part in version.parts if part.parent_part_id is None]
        if len(roots) != 1:
            findings.append(AgentAssetQualityFinding(check_id="single_root", severity="error", message="装配图必须只有一个根部件。", part_ids=part_ids))
        missing_parents = [part.part_id for part in version.parts if part.parent_part_id and part.parent_part_id not in part_set]
        if missing_parents:
            findings.append(AgentAssetQualityFinding(check_id="parent_references", severity="error", message="存在不存在的父部件引用。", part_ids=missing_parents))
        if _has_part_cycle(version.parts):
            findings.append(AgentAssetQualityFinding(check_id="assembly_cycle", severity="error", message="装配图存在父子环。", part_ids=part_ids))
        for first_part_id, second_part_id in _unexpected_aabb_overlaps(version.parts, version.assembly_graph):
            findings.append(
                AgentAssetQualityFinding(
                    check_id="concept_aabb_overlap",
                    severity="warning",
                    message="两个未声明装配关系的部件在概念包围盒中明显重叠；请移动、缩放或让 Agent 重新布置。",
                    part_ids=[first_part_id, second_part_id],
                )
            )
        triangle_count = compile_readback.triangle_count
        budget = version.shape_program.get("triangle_budget", 100000)
        if isinstance(budget, (int, float)) and triangle_count > budget:
            findings.append(AgentAssetQualityFinding(check_id="triangle_budget", severity="error", message="预览三角形超过预算。"))
        graph_parts = {item.get("part_id"): item for item in version.assembly_graph.get("parts", []) if isinstance(item, dict)}
        connections = version.assembly_graph.get("connections", [])
        for connection in connections if isinstance(connections, list) else []:
            if not isinstance(connection, dict) or connection.get("from_part_id") not in part_set or connection.get("to_part_id") not in part_set:
                findings.append(AgentAssetQualityFinding(check_id="connection_references", severity="error", message="连接引用了不存在的部件。"))
                continue
            source = graph_parts.get(connection.get("from_part_id"))
            target = graph_parts.get(connection.get("to_part_id"))
            source_position = _connector_world_position(source, connection.get("from_connector_id"))
            target_position = _connector_world_position(target, connection.get("to_connector_id"))
            if source_position is None or target_position is None:
                findings.append(AgentAssetQualityFinding(check_id="connector_references", severity="error", message="连接引用了不存在的连接器。", part_ids=[str(connection.get("from_part_id")), str(connection.get("to_part_id"))]))
            else:
                source_connector = _connector_from_graph(source, connection.get("from_connector_id"))
                target_connector = _connector_from_graph(target, connection.get("to_connector_id"))
                if source_connector is None or target_connector is None or not _connector_kinds_compatible(str(source_connector.get("kind")), str(target_connector.get("kind"))):
                    findings.append(AgentAssetQualityFinding(check_id="connector_compatibility", severity="error", message="连接器类型不兼容。", part_ids=[str(connection.get("from_part_id")), str(connection.get("to_part_id"))]))
                elif any(abs(source_position[index] - target_position[index]) > 0.01 for index in range(3)):
                    findings.append(AgentAssetQualityFinding(check_id="connector_alignment", severity="info", message="连接器在当前概念姿态下存在位置参考差异；可使用连接器吸附。", part_ids=[str(connection.get("from_part_id")), str(connection.get("to_part_id"))]))
        for graph_part in graph_parts.values():
            joints = graph_part.get("joints", [])
            for joint in joints if isinstance(joints, list) else []:
                if isinstance(joint, dict) and joint.get("target_part_id") not in part_set:
                    findings.append(AgentAssetQualityFinding(check_id="joint_references", severity="error", message="关节引用了不存在的目标部件。", part_ids=[str(graph_part.get("part_id"))]))
        valid_materials = material_preset_map()
        for binding_key, material_id in version.material_bindings.items():
            part_id = binding_key.split(":", 1)[0]
            if part_id not in part_set or material_id not in valid_materials:
                findings.append(AgentAssetQualityFinding(check_id="material_bindings", severity="error", message="材质绑定引用无效。", part_ids=[part_id]))
        status = "failed" if any(item.severity == "error" for item in findings) else "warning" if any(item.severity == "warning" for item in findings) else "passed"
        return self._persist_quality_report(version, AgentAssetQualityReport(
            quality_report_id=_new_id("quality"),
            asset_version_id=version.asset_version_id,
            status=status,
            triangle_count=triangle_count,
            bounds_mm=compile_readback.bounds_mm,
            evidence_source="geometry_compile_readback",
            compile_readback=compile_readback,
            findings=findings,
            checked_at=_utc_now(),
        ), expected_revision=expected_revision, idempotency_key=idempotency_key)

    def get_quality_report(self, quality_report_id: str) -> AgentAssetQualityReport:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_quality_report(quality_report_id)
            if row is None:
                raise AgentAssetError("ASSET_QUALITY_NOT_FOUND", "资产质量报告不存在。", status_code=404)
            report = AgentAssetQualityReport.model_validate_json(str(row["report_json"]))
            if report.evidence_source == "legacy_estimate":
                return report.model_copy(update={
                    "status": "unavailable",
                    "triangle_count": 0,
                    "bounds_mm": None,
                    "findings": [AgentAssetQualityFinding(
                        check_id="legacy_quality_estimate",
                        severity="error",
                        message="旧质量报告没有真实编译/GLB 回读证据，请重新检查当前资产。",
                    )],
                })
            return report

    def _persist_quality_report(
        self,
        version: AgentAssetVersion,
        report: AgentAssetQualityReport,
        *,
        expected_revision: Optional[int],
        idempotency_key: Optional[str],
    ) -> AgentAssetQualityReport:
        scope = f"POST /api/v1/agent/asset-versions/{version.asset_version_id}:quality"
        request_hash = _quality_request_hash(version.asset_version_id, expected_revision)
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            if idempotency_key is not None:
                replay = unit.idempotency.get(scope, idempotency_key)
                if replay is not None:
                    if replay.request_hash != request_hash:
                        raise AgentAssetIdempotencyConflict(
                            "Idempotency-Key was reused with a different quality request."
                        )
                    return AgentAssetQualityReport.model_validate_json(replay.response_json)
            snapshot = unit.active_designs.get_snapshot(version.project_id)
            if snapshot is None or not hasattr(snapshot.active_design, "asset_version_id"):
                raise AgentAssetError("ACTIVE_DESIGN_NOT_AGENT", "当前项目没有可检查的活动 Agent 资产。", status_code=409)
            if snapshot.active_design.asset_version_id != version.asset_version_id:
                raise AgentAssetError("ACTIVE_DESIGN_STALE", "该资产不是当前活动设计，请刷新后重新检查。", status_code=409)
            if expected_revision is not None and snapshot.revision != expected_revision:
                raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计已更新，请刷新后重新检查。", status_code=409)
            unit.agent_assets.add_quality_report(
                quality_report_id=report.quality_report_id,
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                report_json=_canonical_json(report.model_dump(mode="json")),
                status=report.status,
                created_at=report.checked_at,
            )
            try:
                unit.active_designs.set_quality(
                    project_id=version.project_id,
                    expected_revision=snapshot.revision,
                    quality_report_id=report.quality_report_id,
                    asset_version_id=version.asset_version_id,
                    updated_at=report.checked_at,
                )
            except ActiveDesignSnapshotConflict as exc:
                raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计已更新，请刷新后重新检查。", status_code=409) from exc
            except ActiveDesignSnapshotError as exc:
                raise AgentAssetError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            if idempotency_key is not None:
                unit.idempotency.add(
                    scope=scope,
                    key=idempotency_key,
                    request_hash=request_hash,
                    response_json=_canonical_json(report.model_dump(mode="json")),
                    created_at=report.checked_at,
                )
            return report

    def export_glb(self, asset_version_id: str) -> AgentAssetExportResponse:
        version = self._get_active_agent_version(asset_version_id, purpose="导出")
        imported = self._imported_glb_row(asset_version_id)
        if imported is not None:
            if self.object_store is None:
                raise AgentAssetError("OBJECT_STORE_UNAVAILABLE", "本机对象存储不可用，无法导出导入模型。", status_code=503)
            try:
                glb = self.object_store.read(str(imported["object_path"]), expected_sha256=str(imported["sha256"]))
                inspection = inspect_imported_glb(glb)
            except (ObjectStoreError, ValueError) as exc:
                raise AgentAssetError("EXTERNAL_GLB_UNAVAILABLE", str(exc), status_code=409) from exc
            return AgentAssetExportResponse(
                asset_version_id=version.asset_version_id,
                glb_base64=base64.b64encode(glb).decode("ascii"),
                triangle_count=inspection.triangle_count,
                bounds_mm=inspection.bounds_mm,
                readback_status="passed",
                readback_triangle_count=inspection.triangle_count,
                exported_at=_utc_now(),
            )
        try:
            compiled = compile_shape_program(version.shape_program)
        except GeometryCompileReadbackError as exc:
            raise _geometry_readback_failed(exc) from exc
        except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
            raise _runtime_operation_rejected(exc) from exc
        readback = compiled.readback
        return AgentAssetExportResponse(
            asset_version_id=version.asset_version_id,
            glb_base64=base64.b64encode(compiled.glb_bytes).decode("ascii"),
            triangle_count=readback.triangle_count,
            bounds_mm=readback.bounds_mm,
            readback_status="passed",
            readback_triangle_count=readback.triangle_count,
            exported_at=_utc_now(),
        )

    def render_views(self, asset_version_id: str, *, width: int = 640, height: int = 640) -> AgentAssetRenderSet:
        """Render derived concept views from the current Snapshot-bound Agent asset.

        Rendered PNGs are ephemeral read-only artifacts. They never create a
        version, update the Snapshot, or call a legacy Concept export route.
        """
        version = self._get_active_agent_version(asset_version_id, purpose="渲染")
        try:
            exported = self.export_glb(version.asset_version_id)
            glb = base64.b64decode(exported.glb_base64, validate=True)
            rendered = render_agent_views(
                glb,
                width=width,
                height=height,
                exploded_parts=_exploded_part_offsets(version),
            )
        except (AgentRenderError, ValueError) as exc:
            raise AgentAssetError("AGENT_RENDER_UNAVAILABLE", str(exc), status_code=409) from exc

        rendered_at = _utc_now()
        views: list[AgentAssetRenderView] = []
        for view_id in ("iso", "front", "side", "top", "exploded_iso"):
            if view_id not in rendered.views:
                continue
            payload = rendered.views[view_id]
            _validate_png_payload(payload, width=rendered.width, height=rendered.height)
            views.append(
                AgentAssetRenderView(
                    asset_version_id=version.asset_version_id,
                    view_id=view_id,
                    camera_view="iso" if view_id == "exploded_iso" else view_id,
                    presentation_mode="exploded" if view_id == "exploded_iso" else "standard",
                    background_mode="transparent",
                    part_ids=list(rendered.exploded_part_ids) if view_id == "exploded_iso" else [],
                    width=rendered.width,
                    height=rendered.height,
                    png_base64=base64.b64encode(payload).decode("ascii"),
                    sha256=hashlib.sha256(payload).hexdigest(),
                    byte_size=len(payload),
                    readback_status="passed",
                )
            )
        fingerprint_payload = {
            "schema_version": "AgentAssetRenderSet@1",
            "asset_version_id": version.asset_version_id,
            "renderer_id": "forgecad-agent-software-raster@1",
            "width": rendered.width,
            "height": rendered.height,
            "views": [
                {
                    "view_id": item.view_id,
                    "sha256": item.sha256,
                    "presentation_mode": item.presentation_mode,
                    "background_mode": item.background_mode,
                    "part_ids": item.part_ids,
                }
                for item in views
            ],
            "exploded_view_available": bool(rendered.exploded_part_ids),
            "exploded_unavailable_reason": rendered.exploded_unavailable_reason,
        }
        render_set_sha256 = hashlib.sha256(_canonical_json(fingerprint_payload).encode("utf-8")).hexdigest()
        return AgentAssetRenderSet(
            asset_version_id=version.asset_version_id,
            width=rendered.width,
            height=rendered.height,
            views=views,
            exploded_view_available=bool(rendered.exploded_part_ids),
            exploded_unavailable_reason=rendered.exploded_unavailable_reason,
            render_set_sha256=render_set_sha256,
            render_set_byte_size=sum(item.byte_size for item in views),
            rendered_at=rendered_at,
        )

    def render_view_package(
        self,
        asset_version_id: str,
        *,
        width: int = 640,
        height: int = 640,
        expected_render_set_sha256: Optional[str] = None,
    ) -> tuple[bytes, AgentAssetRenderPackageManifest]:
        """Create a deterministic, presentation-only ZIP for the active render set.

        Nothing is persisted: the method derives the same current PNGs that
        ``render_views`` returns, validates their readback/hash facts again,
        and packages only those bytes plus a machine-readable manifest.  The
        optional expected fingerprint prevents a user from downloading a set
        that changed between the drawer preview and the click.
        """
        render_set = self.render_views(asset_version_id, width=width, height=height)
        if expected_render_set_sha256 and expected_render_set_sha256 != render_set.render_set_sha256:
            raise AgentAssetError(
                "RENDER_SET_STALE",
                "概念图已更新，请重新生成后再下载概念图包。",
                status_code=409,
            )

        entries: list[tuple[str, bytes]] = []
        manifest_views: list[AgentAssetRenderPackageView] = []
        for view in render_set.views:
            try:
                payload = base64.b64decode(view.png_base64, validate=True)
            except (ValueError, TypeError) as exc:
                raise AgentAssetError("RENDER_PACKAGE_INVALID", "概念图内容校验失败。", status_code=409) from exc
            _validate_png_payload(payload, width=view.width, height=view.height)
            if hashlib.sha256(payload).hexdigest() != view.sha256 or len(payload) != view.byte_size:
                raise AgentAssetError("RENDER_PACKAGE_INVALID", "概念图哈希校验失败。", status_code=409)
            file_name = f"{view.view_id}.png"
            entries.append((file_name, payload))
            manifest_views.append(
                AgentAssetRenderPackageView(
                    file_name=file_name,
                    asset_version_id=view.asset_version_id,
                    view_id=view.view_id,
                    camera_view=view.camera_view,
                    presentation_mode=view.presentation_mode,
                    background_mode=view.background_mode,
                    part_ids=view.part_ids,
                    mime_type=view.mime_type,
                    width=view.width,
                    height=view.height,
                    sha256=view.sha256,
                    byte_size=view.byte_size,
                    readback_status=view.readback_status,
                )
            )
        manifest = AgentAssetRenderPackageManifest(
            asset_version_id=render_set.asset_version_id,
            renderer_id=render_set.renderer_id,
            render_set_sha256=render_set.render_set_sha256,
            render_set_byte_size=render_set.render_set_byte_size,
            width=render_set.width,
            height=render_set.height,
            views=manifest_views,
            exploded_view_available=render_set.exploded_view_available,
            exploded_unavailable_reason=render_set.exploded_unavailable_reason,
        )
        archive = BytesIO()
        with zipfile.ZipFile(archive, mode="w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
            _write_stable_zip_entry(bundle, "manifest.json", _canonical_json(manifest.model_dump(mode="json")).encode("utf-8"))
            for file_name, payload in entries:
                _write_stable_zip_entry(bundle, file_name, payload)
        return archive.getvalue(), manifest

    def _get_active_agent_version(self, asset_version_id: str, *, purpose: str) -> AgentAssetVersion:
        """Reject stale direct API calls instead of exporting a historical asset by accident."""
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            row = unit.agent_assets.get_version(asset_version_id)
            if row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            version = _version_from_row(row)
            snapshot = unit.active_designs.get_snapshot(version.project_id)
            if snapshot is None or not hasattr(snapshot.active_design, "asset_version_id"):
                raise AgentAssetError("ACTIVE_DESIGN_NOT_AGENT", f"当前项目没有可{purpose}的活动 Agent 资产。", status_code=409)
            if snapshot.active_design.asset_version_id != asset_version_id:
                raise AgentAssetError("ACTIVE_DESIGN_STALE", f"该资产不是当前活动设计，请刷新后重新{purpose}。", status_code=409)
            return version
    def propose_change_set(
        self,
        asset_version_id: str,
        request: ProposeAgentAssetChangeSetRequest,
        idempotency_key: str,
    ) -> AgentAssetChangeSet:
        scope = f"POST /api/v1/agent/asset-versions/{asset_version_id}/change-sets"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different edit request.")
                return AgentAssetChangeSet.model_validate_json(replay.response_json)
            base_row = unit.agent_assets.get_version(asset_version_id)
            if base_row is None:
                raise AgentAssetError("ASSET_VERSION_NOT_FOUND", "可编辑资产版本不存在。", status_code=404)
            if base_row["status"] != "committed":
                raise AgentAssetError("ASSET_VERSION_STALE", "只能从当前可编辑版本创建修改。", status_code=409)
            base_version = _version_from_row(base_row)
            locked_part_ids = _active_snapshot_locked_part_ids(unit, base_version, purpose="修改")
            if _is_external_glb_reference(base_version):
                raise AgentAssetError("EXTERNAL_REFERENCE_NOT_EDITABLE", "导入 GLB 当前仅作为参考模型；请让 Agent 重建为可编辑概念资产。", status_code=409)
            _validate_operations(
                base_version,
                request.operations,
                protected_part_ids=request.protected_part_ids,
                locked_part_ids=locked_part_ids,
                components={item.replacement_component_id: _component_from_row(unit.agent_assets.get_component(item.replacement_component_id)) for item in request.operations if item.replacement_component_id and unit.agent_assets.get_component(item.replacement_component_id)},
                structure_suggestions=_structure_suggestions_for(base_version, locked_part_ids=locked_part_ids),
            )
            now = _utc_now()
            change_set_id = _new_id("assetcs")
            unit.agent_assets.add_change_set(
                change_set_id=change_set_id,
                project_id=str(base_row["project_id"]),
                base_asset_version_id=asset_version_id,
                summary=request.summary,
                operations_json=_canonical_json([item.model_dump(mode="json") for item in request.operations]),
                protected_part_ids_json=_canonical_json(request.protected_part_ids),
                created_at=now,
            )
            change_set = AgentAssetChangeSet(
                change_set_id=change_set_id,
                project_id=str(base_row["project_id"]),
                base_asset_version_id=asset_version_id,
                summary=request.summary,
                operations=request.operations,
                protected_part_ids=request.protected_part_ids,
                status="proposed",
                created_at=now,
                updated_at=now,
            )
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(change_set.model_dump(mode="json")),
                created_at=now,
            )
            return change_set

    def _imported_glb_row(self, asset_version_id: str) -> Any:
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            return unit.agent_assets.get_imported_glb(asset_version_id)

    def preview_change_set(self, change_set_id: str, idempotency_key: str) -> AgentAssetChangeSet:
        return self._preview_or_confirm(change_set_id, idempotency_key, confirm=False)

    def confirm_change_set(
        self,
        change_set_id: str,
        idempotency_key: str,
    ) -> AgentAssetChangeSetConfirmResponse:
        scope = f"POST /api/v1/agent/change-sets/{change_set_id}:confirm"
        request_hash = _hash_json({"change_set_id": change_set_id})
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different confirm request.")
                return AgentAssetChangeSetConfirmResponse.model_validate_json(replay.response_json)
            row = unit.agent_assets.get_change_set(change_set_id)
            if row is None:
                raise AgentAssetError("ASSET_CHANGE_SET_NOT_FOUND", "Agent 修改不存在。", status_code=404)
            if row["status"] != "previewed" or not row["preview_json"]:
                raise AgentAssetError("ASSET_CHANGE_SET_NOT_PREVIEWED", "请先预览修改，再确认保存。", status_code=409)
            base = unit.agent_assets.get_version(str(row["base_asset_version_id"]))
            if base is None or base["status"] != "committed":
                unit.agent_assets.update_change_set(change_set_id, status="stale", preview_json=None, resulting_asset_version_id=None, updated_at=_utc_now())
                raise AgentAssetError("ASSET_VERSION_STALE", "基础版本已经变化，请重新选择当前版本。", status_code=409)
            head = unit.agent_assets.get_head(str(row["project_id"]))
            if head is None or str(head["asset_version_id"]) != str(row["base_asset_version_id"]):
                unit.agent_assets.update_change_set(change_set_id, status="stale", preview_json=None, resulting_asset_version_id=None, updated_at=_utc_now())
                raise AgentAssetError("ASSET_VERSION_STALE", "基础版本不是当前版本，请重新预览。", status_code=409)
            snapshot = unit.active_designs.get_snapshot(str(row["project_id"]))
            if snapshot is not None:
                active_design = snapshot.active_design
                if not hasattr(active_design, "asset_version_id"):
                    unit.agent_assets.update_change_set(change_set_id, status="stale", preview_json=None, resulting_asset_version_id=None, updated_at=_utc_now())
                    raise AgentAssetError("ACTIVE_DESIGN_LEGACY_READ_ONLY", "当前项目仍处于 legacy 只读设计状态；请先转换为 Agent 资产。", status_code=409)
                if active_design.asset_version_id != str(row["base_asset_version_id"]):
                    unit.agent_assets.update_change_set(change_set_id, status="stale", preview_json=None, resulting_asset_version_id=None, updated_at=_utc_now())
                    raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计版本已变化，请重新预览修改。", status_code=409)
            try:
                preview = AgentAssetVersion.model_validate_json(row["preview_json"])
            except ValidationError as exc:
                raise _runtime_operation_rejected(exc) from exc
            try:
                build_glb_from_shape_program(preview.shape_program)
            except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
                raise _runtime_operation_rejected(exc) from exc
            now = _utc_now()
            version = preview.model_copy(
                update={
                    "asset_version_id": _new_id("assetver"),
                    "parent_asset_version_id": str(base["asset_version_id"]),
                    "version_no": unit.agent_assets.next_version_number(str(row["project_id"])),
                    "status": "committed",
                    "stage": "editable_asset",
                    "created_at": now,
                }
            )
            unit.agent_assets.add_version(**_version_row(version))
            unit.agent_assets.supersede(str(base["asset_version_id"]))
            unit.agent_assets.set_head(project_id=str(row["project_id"]), asset_version_id=version.asset_version_id, updated_at=now)
            _sync_agent_snapshot(unit, version=version, updated_at=now)
            unit.agent_assets.update_change_set(change_set_id, status="confirmed", preview_json=_canonical_json(version.model_dump(mode="json")), resulting_asset_version_id=version.asset_version_id, updated_at=now)
            change_set = _change_set_from_row(unit.agent_assets.get_change_set(change_set_id))
            response = AgentAssetChangeSetConfirmResponse(change_set=change_set, asset_version=version)
            unit.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def reject_change_set(self, change_set_id: str, idempotency_key: str) -> AgentAssetChangeSet:
        scope = f"POST /api/v1/agent/change-sets/{change_set_id}:reject"
        request_hash = _hash_json({"change_set_id": change_set_id})
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different reject request.")
                return AgentAssetChangeSet.model_validate_json(replay.response_json)
            row = unit.agent_assets.get_change_set(change_set_id)
            if row is None:
                raise AgentAssetError("ASSET_CHANGE_SET_NOT_FOUND", "Agent 修改不存在。", status_code=404)
            if row["status"] not in {"proposed", "previewed"}:
                raise AgentAssetError("ASSET_CHANGE_SET_STATE_CONFLICT", "该修改已处理，不能再次取消。", status_code=409)
            now = _utc_now()
            unit.agent_assets.update_change_set(change_set_id, status="rejected", preview_json=None, resulting_asset_version_id=None, updated_at=now)
            snapshot = unit.active_designs.get_snapshot(str(row["project_id"]))
            if (
                snapshot is not None
                and hasattr(snapshot.active_design, "asset_version_id")
                and snapshot.preview is not None
                and snapshot.preview.change_set_id == change_set_id
            ):
                try:
                    unit.active_designs.set_preview(
                        project_id=str(row["project_id"]),
                        expected_revision=snapshot.revision,
                        change_set_id=None,
                        base_asset_version_id=None,
                        updated_at=now,
                    )
                except ActiveDesignSnapshotConflict as exc:
                    raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计已更新，请刷新后重试。", status_code=409) from exc
                except ActiveDesignSnapshotError as exc:
                    raise AgentAssetError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            change_set = _change_set_from_row(unit.agent_assets.get_change_set(change_set_id))
            unit.idempotency.add(scope=scope, key=idempotency_key, request_hash=request_hash, response_json=_canonical_json(change_set.model_dump(mode="json")), created_at=now)
            return change_set

    def _preview_or_confirm(self, change_set_id: str, idempotency_key: str, *, confirm: bool) -> AgentAssetChangeSet:
        scope = f"POST /api/v1/agent/change-sets/{change_set_id}:preview"
        request_hash = _hash_json({"change_set_id": change_set_id})
        with SQLiteUnitOfWork(self.connection_factory) as unit:
            replay = unit.idempotency.get(scope, idempotency_key)
            if replay is not None:
                if replay.request_hash != request_hash:
                    raise AgentAssetIdempotencyConflict("Idempotency-Key was reused with a different preview request.")
                return AgentAssetChangeSet.model_validate_json(replay.response_json)
            row = unit.agent_assets.get_change_set(change_set_id)
            if row is None:
                raise AgentAssetError("ASSET_CHANGE_SET_NOT_FOUND", "Agent 修改不存在。", status_code=404)
            if row["status"] not in {"proposed", "previewed"}:
                raise AgentAssetError("ASSET_CHANGE_SET_STATE_CONFLICT", "当前状态不能预览。", status_code=409)
            base_row = unit.agent_assets.get_version(str(row["base_asset_version_id"]))
            if base_row is None or base_row["status"] != "committed":
                raise AgentAssetError("ASSET_VERSION_STALE", "基础版本已不可用。", status_code=409)
            base = _version_from_row(base_row)
            locked_part_ids = _active_snapshot_locked_part_ids(unit, base, purpose="预览修改")
            operations = [AgentPartEditOperation.model_validate(item) for item in json.loads(row["operations_json"])]
            components = {item.replacement_component_id: _component_from_row(unit.agent_assets.get_component(item.replacement_component_id)) for item in operations if item.replacement_component_id and unit.agent_assets.get_component(item.replacement_component_id)}
            structure_suggestions = _structure_suggestions_for(base, locked_part_ids=locked_part_ids)
            _validate_operations(
                base,
                operations,
                protected_part_ids=json.loads(row["protected_part_ids_json"]),
                locked_part_ids=locked_part_ids,
                components=components,
                structure_suggestions=structure_suggestions,
            )
            try:
                preview = _apply_operations(
                    base,
                    operations,
                    summary=str(row["summary"]),
                    components=components,
                    structure_suggestions=structure_suggestions,
                )
                build_glb_from_shape_program(preview.shape_program)
            except (ShapeProgramValidationError, UnsupportedRuntimeOperationError, ValueError) as exc:
                raise _runtime_operation_rejected(exc) from exc
            now = _utc_now()
            unit.agent_assets.update_change_set(change_set_id, status="previewed", preview_json=_canonical_json(preview.model_dump(mode="json")), resulting_asset_version_id=None, updated_at=now)
            snapshot = unit.active_designs.get_snapshot(str(row["project_id"]))
            if snapshot is not None:
                active_design = snapshot.active_design
                if not hasattr(active_design, "asset_version_id"):
                    raise AgentAssetError("ACTIVE_DESIGN_LEGACY_READ_ONLY", "当前项目仍处于 legacy 只读设计状态；请先转换为 Agent 资产。", status_code=409)
                if active_design.asset_version_id != str(row["base_asset_version_id"]):
                    raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计版本已变化，请刷新后重新预览修改。", status_code=409)
                try:
                    unit.active_designs.set_preview(
                        project_id=str(row["project_id"]),
                        expected_revision=snapshot.revision,
                        change_set_id=change_set_id,
                        base_asset_version_id=str(row["base_asset_version_id"]),
                        updated_at=now,
                    )
                except ActiveDesignSnapshotConflict as exc:
                    raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计已更新，请刷新后重新预览修改。", status_code=409) from exc
                except ActiveDesignSnapshotError as exc:
                    raise AgentAssetError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc
            change_set = _change_set_from_row(unit.agent_assets.get_change_set(change_set_id))
            unit.idempotency.add(scope=scope, key=idempotency_key, request_hash=request_hash, response_json=_canonical_json(change_set.model_dump(mode="json")), created_at=now)
            return change_set


def _exploded_part_offsets(version: AgentAssetVersion) -> tuple[ExplodedPartOffset, ...]:
    """Derive deterministic visual offsets from existing part hierarchy facts.

    This never changes the saved AssemblyGraph or geometry. A mismatch is
    intentionally returned as no candidate, so flattened/imported assets do
    not receive a fabricated part-separation graphic.
    """
    graph_parts = version.assembly_graph.get("parts")
    if not isinstance(graph_parts, list) or not graph_parts:
        return ()
    graph_by_id = {
        str(item.get("part_id")): item
        for item in graph_parts
        if isinstance(item, dict) and isinstance(item.get("part_id"), str)
    }
    stable_part_ids = [part.part_id for part in version.parts]
    if len(graph_by_id) != len(stable_part_ids) or set(graph_by_id) != set(stable_part_ids):
        return ()
    part_by_id = {part.part_id: part for part in version.parts}
    root_id = version.assembly_graph.get("root_part_id")
    if root_id not in part_by_id:
        root_id = stable_part_ids[0]
    root_position = part_by_id[root_id].position_mm
    result: list[ExplodedPartOffset] = []
    for ordinal, part_id in enumerate(stable_part_ids):
        part = part_by_id[part_id]
        if part_id == root_id:
            result.append(ExplodedPartOffset(part_id=part_id, offset=(0.0, 0.0, 0.0)))
            continue
        graph_part = graph_by_id[part_id]
        parent_id = graph_part.get("parent_part_id")
        parent = part_by_id.get(parent_id) if isinstance(parent_id, str) else None
        origin = parent.position_mm if parent is not None else root_position
        direction = [part.position_mm[index] - origin[index] for index in range(3)]
        magnitude = math.sqrt(sum(value * value for value in direction))
        if magnitude <= 1e-6:
            # Stable ID-derived fallback is visual spacing only, never an
            # inferred connector, joint or physical assembly direction.
            digest = hashlib.sha256(part_id.encode("utf-8")).digest()
            angle = (int.from_bytes(digest[:2], "big") / 65535.0) * math.tau
            direction = [math.cos(angle), 0.28 + (digest[2] / 255.0) * 0.32, math.sin(angle)]
            magnitude = math.sqrt(sum(value * value for value in direction))
        normalized = [value / magnitude for value in direction]
        visual_gap = max(90.0, min(520.0, max(part.size_mm) * 0.65)) * (1.0 + min(ordinal, 4) * 0.12)
        result.append(
            ExplodedPartOffset(
                part_id=part_id,
                offset=tuple(round(value * visual_gap, 4) for value in normalized),  # type: ignore[arg-type]
            )
        )
    return tuple(result)


def _new_asset_version(
    *,
    project_id: str,
    parent: Optional[str],
    version_no: int,
    summary: str,
    stage: str,
    plan_id: str,
    direction_id: str,
    domain_pack_id: str,
    artifact_id: str,
    parts: list[dict[str, Any]],
    shape_program: dict[str, Any],
    assembly_graph: dict[str, Any],
    material_bindings: dict[str, str],
) -> AgentAssetVersion:
    now = _utc_now()
    return AgentAssetVersion(
        asset_version_id=_new_id("assetver"),
        project_id=project_id,
        parent_asset_version_id=parent,
        version_no=version_no,
        status="committed",
        summary=summary,
        stage=stage,
        plan_id=plan_id,
        direction_id=direction_id,
        domain_pack_id=domain_pack_id,
        artifact_id=artifact_id,
        parts=[BlockoutPartCandidate.model_validate(item) for item in parts],
        shape_program=shape_program,
        assembly_graph=assembly_graph,
        material_bindings=material_bindings,
        created_at=now,
    )


def _version_row(version: AgentAssetVersion) -> dict[str, Any]:
    return {
        "asset_version_id": version.asset_version_id,
        "project_id": version.project_id,
        "parent_asset_version_id": version.parent_asset_version_id,
        "version_no": version.version_no,
        "status": version.status,
        "summary": version.summary,
        "stage": version.stage,
        "plan_id": version.plan_id,
        "direction_id": version.direction_id,
        "domain_pack_id": version.domain_pack_id,
        "artifact_id": version.artifact_id,
        "parts_json": _canonical_json([item.model_dump(mode="json") for item in version.parts]),
        "shape_program_json": _canonical_json(version.shape_program),
        "assembly_graph_json": _canonical_json(version.assembly_graph),
        "material_bindings_json": _canonical_json(version.material_bindings),
        "created_at": version.created_at,
    }


def _sync_agent_snapshot(unit: SQLiteUnitOfWork, *, version: AgentAssetVersion, updated_at: str) -> None:
    """Advance the Agent head and Snapshot together inside the caller's UoW."""
    graph_id = version.assembly_graph.get("graph_id") if isinstance(version.assembly_graph, dict) else None
    if not isinstance(graph_id, str) or not graph_id:
        raise AgentAssetError("ACTIVE_DESIGN_GRAPH_MISSING", "可编辑资产缺少活动装配图 ID。", status_code=409)
    try:
        snapshot = unit.active_designs.get_snapshot(version.project_id)
        if snapshot is None:
            unit.active_designs.create_agent_snapshot(
                project_id=version.project_id,
                asset_version_id=version.asset_version_id,
                assembly_graph_id=graph_id,
                updated_at=updated_at,
            )
            return
        active_design = snapshot.active_design
        if not hasattr(active_design, "asset_version_id"):
            unit.active_designs.promote_legacy_to_agent_snapshot(
                project_id=version.project_id,
                expected_revision=snapshot.revision,
                asset_version_id=version.asset_version_id,
                assembly_graph_id=graph_id,
                updated_at=updated_at,
            )
            return
        unit.active_designs.advance_agent_snapshot(
            project_id=version.project_id,
            expected_revision=snapshot.revision,
            asset_version_id=version.asset_version_id,
            assembly_graph_id=graph_id,
            updated_at=updated_at,
        )
    except ActiveDesignSnapshotConflict as exc:
        raise AgentAssetError("ACTIVE_DESIGN_STALE", "活动设计已更新，请刷新后重试。", status_code=409) from exc
    except ActiveDesignSnapshotError as exc:
        raise AgentAssetError("ACTIVE_DESIGN_INVALID", str(exc), status_code=409) from exc


def _version_from_row(row: Any) -> AgentAssetVersion:
    try:
        return AgentAssetVersion(
            asset_version_id=str(row["asset_version_id"]),
            project_id=str(row["project_id"]),
            parent_asset_version_id=row["parent_asset_version_id"],
            version_no=int(row["version_no"]),
            status=str(row["status"]),
            summary=str(row["summary"]),
            stage=str(row["stage"]),
            plan_id=str(row["plan_id"]),
            direction_id=str(row["direction_id"]),
            domain_pack_id=str(row["domain_pack_id"]),
            artifact_id=str(row["artifact_id"]),
            parts=[BlockoutPartCandidate.model_validate(item) for item in json.loads(row["parts_json"])],
            shape_program=json.loads(row["shape_program_json"]),
            assembly_graph=json.loads(row["assembly_graph_json"]),
            material_bindings=json.loads(row["material_bindings_json"]),
            created_at=str(row["created_at"]),
        )
    except ValidationError as exc:
        # Persisted program data is untrusted at every entry point too: an old
        # or manually corrupted row must not bypass the Pydantic/manifest gate.
        raise AgentAssetError("UNSUPPORTED_RUNTIME_OPERATION", str(exc), status_code=409) from exc


def _connector_world_position(graph_part: Optional[dict[str, Any]], connector_id: Optional[str]) -> Optional[list[float]]:
    if not graph_part or not connector_id:
        return None
    connector = _connector_from_graph(graph_part, connector_id)
    if connector is None:
        return None
    position = connector.get("position")
    transform = graph_part.get("transform", {})
    origin = transform.get("position", [0, 0, 0])
    if not isinstance(position, list) or len(position) != 3 or not isinstance(origin, list) or len(origin) != 3:
        return None
    return [float(origin[index]) + float(position[index]) for index in range(3)]


def _connector_from_graph(graph_part: Optional[dict[str, Any]], connector_id: Optional[str]) -> Optional[dict[str, Any]]:
    if not graph_part or not connector_id:
        return None
    return next((item for item in graph_part.get("connectors", []) if isinstance(item, dict) and item.get("connector_id") == connector_id), None)


def _connector_kinds_compatible(source_kind: str, target_kind: str) -> bool:
    allowed = {"surface_mount", "axial_mount", "hinge_mount", "tool_mount"}
    if source_kind not in allowed or target_kind not in allowed:
        return False
    if source_kind == target_kind:
        return True
    return {source_kind, target_kind} <= {"surface_mount", "axial_mount"}


def _component_from_row(row: Any) -> AgentComponentRecord:
    return AgentComponentRecord(
        component_id=str(row["component_id"]),
        project_id=str(row["project_id"]),
        domain_pack_id=str(row["domain_pack_id"]),
        role=str(row["role"]),
        display_name=str(row["display_name"]),
        description=str(row["description"]),
        source_asset_version_id=str(row["source_asset_version_id"]),
        source_part_id=str(row["source_part_id"]),
        part_template=BlockoutPartCandidate.model_validate(json.loads(row["part_template_json"])),
        shape_operation=json.loads(row["shape_operation_json"]),
        material_bindings=json.loads(row["material_bindings_json"]),
        status=str(row["status"]),
        source_quality_status=str(row["source_quality_status"]) if "source_quality_status" in row.keys() else "unavailable",
        created_at=str(row["created_at"]),
        updated_at=str(row["updated_at"]),
    )


def _component_compatibility(
    version: AgentAssetVersion,
    target: BlockoutPartCandidate,
    component: AgentComponentRecord,
) -> AgentComponentCompatibility:
    reasons: list[str] = []
    eligible = True
    if component.status == "active":
        reasons.append("component_active")
    else:
        reasons.append("component_disabled")
        eligible = False
    if component.domain_pack_id == version.domain_pack_id:
        reasons.append("same_domain_pack")
    else:
        reasons.append("domain_pack_mismatch")
        eligible = False
    if component.role == target.role:
        reasons.append("same_role")
    else:
        reasons.append("role_mismatch")
        eligible = False
    quality_reason = f"source_quality_{component.source_quality_status}"
    reasons.append(quality_reason)
    if component.source_quality_status not in {"passed", "warning"}:
        eligible = False
    # Agent component replacement deliberately retains the current target part
    # and AssemblyGraph anchor, so the known target connector facts stay valid.
    reasons.append("target_connectors_preserved")
    return AgentComponentCompatibility(
        component_id=component.component_id,
        target_asset_version_id=version.asset_version_id,
        target_part_id=target.part_id,
        eligible=eligible,
        source_quality_status=component.source_quality_status,
        reason_codes=reasons,
    )


def _active_snapshot_locked_part_ids(
    unit: SQLiteUnitOfWork,
    version: AgentAssetVersion,
    *,
    purpose: str,
) -> set[str]:
    """Keep lock enforcement tied to the active Snapshot, never client state."""
    snapshot = unit.active_designs.get_snapshot(version.project_id)
    if snapshot is None or not hasattr(snapshot.active_design, "asset_version_id"):
        raise AgentAssetError("ACTIVE_DESIGN_NOT_AGENT", f"当前项目没有可{purpose}的活动 Agent 资产。", status_code=409)
    if snapshot.active_design.asset_version_id != version.asset_version_id:
        raise AgentAssetError("ACTIVE_DESIGN_STALE", f"该资产不是当前活动设计，请刷新后重新{purpose}。", status_code=409)
    display = snapshot.part_display
    return set(display.locked_part_ids) if display is not None else set()


def _change_set_from_row(row: Any) -> AgentAssetChangeSet:
    preview = AgentAssetVersion.model_validate_json(row["preview_json"]) if row["preview_json"] else None
    return AgentAssetChangeSet(
        change_set_id=str(row["change_set_id"]),
        project_id=str(row["project_id"]),
        base_asset_version_id=str(row["base_asset_version_id"]),
        summary=str(row["summary"]),
        operations=[AgentPartEditOperation.model_validate(item) for item in json.loads(row["operations_json"])],
        protected_part_ids=json.loads(row["protected_part_ids_json"]),
        status=str(row["status"]),
        preview=preview,
        resulting_asset_version_id=row["resulting_asset_version_id"],
        created_at=str(row["created_at"]),
        updated_at=str(row["updated_at"]),
    )


_LEGACY_EDITABLE_PARAMETER_PATHS = frozenset({
    "transform.position.x",
    "transform.position.y",
    "transform.position.z",
    "transform.scale.x",
    "transform.scale.y",
    "transform.scale.z",
})
"""Frozen compatibility policy for AgentAssetVersion@1 rows without bindings.

Pre-G808 rows have no per-part declaration.  They retain the original six-path
and global-bound behaviour only; an empty list never means arbitrary paths are
editable.  New rows with one or more declarations are validated exclusively
against those declarations.
"""


def _validate_editable_parameter_value(part: BlockoutPartCandidate, path: str, value: float) -> None:
    """Validate one `set_part_parameter` value without broadening ShapeProgram.

    A non-empty binding list is authoritative.  The no-binding branch exists
    solely for immutable AgentAssetVersion@1 compatibility and keeps the old
    fixed allowlist plus the same concept-scale safety bounds.
    """
    bindings = {item.path: item for item in part.editable_parameter_bindings}
    if bindings:
        binding = bindings.get(path)
        if binding is None:
            raise AgentAssetError(
                "PARAMETER_NOT_DECLARED",
                f"部件 {part.part_id} 没有声明可调整参数：{path}",
            )
        if not binding.min <= value <= binding.max:
            raise AgentAssetError(
                "PARAMETER_OUT_OF_RANGE",
                f"参数 {binding.display_name} 必须在 {binding.min:g} 到 {binding.max:g} 之间。",
            )
        steps_from_min = (value - binding.min) / binding.step
        if not math.isclose(steps_from_min, round(steps_from_min), rel_tol=1e-9, abs_tol=1e-8):
            raise AgentAssetError(
                "PARAMETER_STEP_MISMATCH",
                f"参数 {binding.display_name} 必须按 {binding.step:g} 的步长调整。",
            )
        return

    if path.startswith("transform.scale.") and not 0.1 <= value <= 10:
        raise AgentAssetError("PARAMETER_OUT_OF_RANGE", "部件比例必须在 0.1 到 10 之间。")
    if path.startswith("transform.position.") and not -100000 <= value <= 100000:
        raise AgentAssetError("PARAMETER_OUT_OF_RANGE", "部件位置超出轻量概念范围。")


def _validate_operations(
    version: AgentAssetVersion,
    operations: list[AgentPartEditOperation],
    *,
    protected_part_ids: list[str],
    locked_part_ids: Optional[set[str]] = None,
    components: Optional[dict[str, AgentComponentRecord]] = None,
    structure_suggestions: Optional[list[AgentStructureSuggestion]] = None,
) -> None:
    components = components or {}
    suggestion_by_id = {item.suggestion_id: item for item in structure_suggestions or []}
    part_ids = {part.part_id for part in version.parts}
    locked_ids = {part.part_id for part in version.parts if part.locked} | (locked_part_ids or set())
    protected = set(protected_part_ids)
    unknown = [operation.part_id for operation in operations if operation.part_id not in part_ids]
    unknown.extend(operation.target_part_id for operation in operations if operation.target_part_id and operation.target_part_id not in part_ids)
    if unknown:
        raise AgentAssetError("PART_NOT_FOUND", f"找不到部件：{', '.join(unknown)}")
    conflicts = [
        operation.operation_id
        for operation in operations
        if operation.part_id in locked_ids
        or operation.part_id in protected
        or (operation.target_part_id is not None and (operation.target_part_id in locked_ids or operation.target_part_id in protected))
    ]
    if conflicts:
        raise AgentAssetError("PART_PROTECTED", f"锁定部件不能修改：{', '.join(conflicts)}")
    valid_materials = material_preset_map()
    for operation in operations:
        if operation.transform:
            for key, vector in operation.transform.items():
                if len(vector) != 3 or any(not math.isfinite(float(value)) for value in vector):
                    raise AgentAssetError("TRANSFORM_INVALID", "部件变换必须包含有限的三维数值。")
                if key == "scale" and any(not 0.1 <= float(value) <= 10 for value in vector):
                    raise AgentAssetError("PARAMETER_OUT_OF_RANGE", "部件比例必须在 0.1 到 10 之间。")
                if key in {"position", "rotation"} and any(abs(float(value)) > 100000 for value in vector):
                    raise AgentAssetError("PARAMETER_OUT_OF_RANGE", "部件变换超出轻量概念范围。")
        if operation.op == "apply_material_preset":
            if operation.material_id not in valid_materials:
                raise AgentAssetError("MATERIAL_NOT_FOUND", f"视觉材质不存在：{operation.material_id}")
            target_part = next(item for item in version.parts if item.part_id == operation.part_id)
            if operation.material_zone_id is not None and operation.material_zone_id not in target_part.material_zone_ids:
                raise AgentAssetError(
                    "MATERIAL_ZONE_NOT_FOUND",
                    f"部件 {operation.part_id} 没有材质区：{operation.material_zone_id}",
                )
        if operation.op == "set_part_parameter":
            if operation.path not in _LEGACY_EDITABLE_PARAMETER_PATHS:
                raise AgentAssetError("PARAMETER_NOT_ALLOWED", "当前只允许调整部件位置和比例。")
            if not isinstance(operation.value, (int, float)) or not math.isfinite(float(operation.value)):
                raise AgentAssetError("PARAMETER_INVALID", "部件参数必须是有限数字。")
            value = float(operation.value)
            target_part = next(item for item in version.parts if item.part_id == operation.part_id)
            _validate_editable_parameter_value(target_part, str(operation.path), value)
        if operation.op == "set_joint_pose":
            graph_part = next(
                (item for item in version.assembly_graph.get("parts", []) if item.get("part_id") == operation.part_id),
                None,
            )
            if not graph_part or not graph_part.get("joints"):
                raise AgentAssetError("JOINT_NOT_FOUND", "当前部件没有可调整的概念关节。")
        if operation.op == "replace_part":
            component = components.get(operation.replacement_component_id or "")
            if component is None or component.status != "active":
                raise AgentAssetError("COMPONENT_NOT_FOUND", "可复用部件不存在或已停用。", status_code=404)
            target = next(item for item in version.parts if item.part_id == operation.part_id)
            if component.domain_pack_id != version.domain_pack_id or component.role != target.role:
                raise AgentAssetError("COMPONENT_INCOMPATIBLE", "部件的领域包或角色不兼容。")
            if component.source_quality_status not in {"passed", "warning"}:
                raise AgentAssetError(
                    "COMPONENT_QUALITY_NOT_READY",
                    "可复用部件的来源模型尚未通过检查；请先检查来源模型后再替换。",
                    status_code=409,
                )
        if operation.op == "snap_part_to_connector":
            source_graph = next((item for item in version.assembly_graph.get("parts", []) if item.get("part_id") == operation.part_id), None)
            target_graph = next((item for item in version.assembly_graph.get("parts", []) if item.get("part_id") == operation.target_part_id), None)
            source_connector = _connector_from_graph(source_graph, operation.connector_id)
            target_connector = _connector_from_graph(target_graph, operation.target_connector_id)
            if source_connector is None or target_connector is None:
                raise AgentAssetError("CONNECTOR_NOT_FOUND", "源或目标连接器不存在。")
            if not _connector_kinds_compatible(str(source_connector.get("kind")), str(target_connector.get("kind"))):
                raise AgentAssetError("CONNECTOR_INCOMPATIBLE", "源和目标连接器类型不兼容。")
            for connector in (source_connector, target_connector):
                normal = connector.get("normal")
                if not isinstance(normal, list) or len(normal) != 3 or math.sqrt(sum(float(value) ** 2 for value in normal)) < 1e-6:
                    raise AgentAssetError("CONNECTOR_INVALID", "连接器法线必须是非零三维向量。")
        if operation.op in {"split_part", "merge_parts"}:
            suggestion = suggestion_by_id.get(operation.structure_suggestion_id or "")
            if suggestion is None:
                raise AgentAssetError(
                    "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                    "这条拆分或合并建议已经失效；请刷新当前模型后重新选择。",
                    status_code=409,
                )
            if suggestion.kind != operation.op or suggestion.part_id != operation.part_id:
                raise AgentAssetError("STRUCTURE_SUGGESTION_MISMATCH", "结构建议与当前部件不一致。", status_code=409)
            if operation.op == "merge_parts" and suggestion.target_part_id != operation.target_part_id:
                raise AgentAssetError("STRUCTURE_SUGGESTION_MISMATCH", "合并建议的目标部件不一致。", status_code=409)


def _apply_operations(
    version: AgentAssetVersion,
    operations: list[AgentPartEditOperation],
    *,
    summary: str,
    components: Optional[dict[str, AgentComponentRecord]] = None,
    structure_suggestions: Optional[list[AgentStructureSuggestion]] = None,
) -> AgentAssetVersion:
    components = components or {}
    parts = [item.model_dump(mode="json") for item in version.parts]
    graph = json.loads(json.dumps(version.assembly_graph))
    program = json.loads(json.dumps(version.shape_program))
    bindings = dict(version.material_bindings)
    part_map = {item["part_id"]: item for item in parts}
    graph_parts = {item["part_id"]: item for item in graph.get("parts", []) if isinstance(item, dict) and item.get("part_id")}
    role_by_part = {item["part_id"]: item.get("role") for item in parts}
    suggestion_by_id = {item.suggestion_id: item for item in structure_suggestions or []}
    for operation in operations:
        part = part_map[operation.part_id]
        graph_part = graph_parts.get(operation.part_id)
        role = role_by_part.get(operation.part_id)
        shape_ops = [item for item in program.get("operations", []) if isinstance(item, dict) and item.get("args", {}).get("part_role") == role]
        if operation.op == "set_part_transform" and operation.transform:
            transform = operation.transform
            part["position_mm"] = list(transform["position"])
            if graph_part is not None:
                graph_part["transform"] = {"position": list(transform["position"]), "rotation": list(transform["rotation"]), "scale": list(transform["scale"])}
            for shape_op in shape_ops:
                args = shape_op["args"]
                args["position"] = list(transform["position"])
                args["rotation"] = list(transform["rotation"])
                if shape_op.get("op") == "cylinder":
                    args["radius"] = round(float(args.get("radius", part["size_mm"][0] / 2)) * max(float(transform["scale"][0]), float(transform["scale"][2])), 4)
                    args["height"] = round(float(args.get("height", part["size_mm"][1])) * float(transform["scale"][1]), 4)
                    args["axis"] = list(args.get("axis", [0, 1, 0]))
                else:
                    args["size"] = [round(float(size) * float(scale), 4) for size, scale in zip(part["size_mm"], transform["scale"])]
        elif operation.op == "set_joint_pose" and operation.transform and graph_part is not None:
            current_rotation = list(graph_part.setdefault("transform", {}).setdefault("rotation", [0, 0, 0]))
            next_rotation = list(operation.transform["rotation"])
            graph_part["transform"]["rotation"] = next_rotation
            for shape_op in shape_ops:
                shape_op.setdefault("args", {})["rotation"] = next_rotation
            _propagate_joint_delta(parts, graph, program, operation.part_id, next_rotation[2] - current_rotation[2])
        elif operation.op == "set_part_parameter" and operation.path:
            _apply_parameter(part, graph_part, shape_ops, operation.path, float(operation.value))
        elif operation.op == "apply_material_preset" and operation.material_id:
            zone = operation.material_zone_id or (part["material_zone_ids"][0] if part["material_zone_ids"] else "primary")
            bindings[f"{operation.part_id}:{zone}"] = operation.material_id
            for shape_op in shape_ops:
                args = shape_op.setdefault("args", {})
                shape_zone = args.get("zone_id")
                if operation.material_zone_id is not None:
                    default_zone = part["material_zone_ids"][0] if part["material_zone_ids"] else "primary"
                    if shape_zone is not None and shape_zone != zone:
                        continue
                    if shape_zone is None and zone != default_zone:
                        continue
                args["material_id"] = operation.material_id
        elif operation.op == "replace_part" and operation.replacement_component_id:
            component = components[operation.replacement_component_id]
            template = component.part_template.model_dump(mode="json")
            # Preserve the target identity and assembly location; copy only the
            # reusable geometry-facing fields from the registry snapshot.
            part["role"] = template["role"]
            part["size_mm"] = list(template["size_mm"])
            part["material_zone_ids"] = list(template["material_zone_ids"])
            part["editable_parameters"] = list(template["editable_parameters"])
            part["editable_parameter_bindings"] = json.loads(json.dumps(template["editable_parameter_bindings"]))
            part["provenance"] = "agent_component"
            source_args = component.shape_operation.get("args", {})
            for shape_op in shape_ops:
                args = shape_op.setdefault("args", {})
                for key in ("size", "material_id"):
                    if key in source_args:
                        args[key] = json.loads(json.dumps(source_args[key]))
                args["part_role"] = role
            for binding_key, material_id in component.material_bindings.items():
                zone = binding_key.split(":", 1)[1] if ":" in binding_key else "primary"
                bindings[f"{operation.part_id}:{zone}"] = material_id
        elif operation.op == "snap_part_to_connector" and operation.target_part_id:
            target = part_map[operation.target_part_id]
            source_graph = graph_part or {}
            target_graph = graph_parts.get(operation.target_part_id, {})
            source_connector = next((item for item in source_graph.get("connectors", []) if item.get("connector_id") == operation.connector_id), {})
            target_connector = next((item for item in target_graph.get("connectors", []) if item.get("connector_id") == operation.target_connector_id), {})
            source_offset = source_connector.get("position", [0, 0, 0])
            target_offset = target_connector.get("position", [0, 0, 0])
            next_position = [round(float(target["position_mm"][axis]) + float(target_offset[axis]) - float(source_offset[axis]), 4) for axis in range(3)]
            part["position_mm"] = next_position
            if graph_part is not None:
                graph_part.setdefault("transform", {})["position"] = list(next_position)
            for shape_op in shape_ops:
                shape_op.setdefault("args", {})["position"] = list(next_position)
        elif operation.op == "split_part":
            suggestion = suggestion_by_id[operation.structure_suggestion_id or ""]
            _apply_split_suggestion(
                parts=parts,
                graph=graph,
                program=program,
                bindings=bindings,
                source_part_id=operation.part_id,
                suggestion=suggestion,
            )
        elif operation.op == "merge_parts" and operation.target_part_id:
            suggestion = suggestion_by_id[operation.structure_suggestion_id or ""]
            _apply_merge_suggestion(
                parts=parts,
                graph=graph,
                program=program,
                bindings=bindings,
                survivor_part_id=operation.part_id,
                absorbed_part_id=operation.target_part_id,
                suggestion=suggestion,
            )
    validate_shape_program(program)
    return version.model_copy(
        update={
            "summary": summary,
            "stage": "editable_asset",
            "parts": [BlockoutPartCandidate.model_validate(item) for item in parts],
            "shape_program": program,
            "assembly_graph": graph,
            "material_bindings": bindings,
        }
    )


_SPLITTABLE_PRIMITIVES = {"box", "cylinder", "capsule", "wedge"}


def _structure_suggestions_for(
    version: AgentAssetVersion,
    *,
    locked_part_ids: Optional[set[str]] = None,
) -> list[AgentStructureSuggestion]:
    """Derive only the restructures that existing data proves are safe enough.

    This intentionally does not inspect triangle topology or invent a cut plane.
    It reads the stable AssemblyGraph relationship, role-to-output mapping and
    existing ShapeProgram primitive facts.  Any missing/ambiguous fact simply
    produces no suggestion.
    """
    graph_parts = {
        str(item.get("part_id")): item
        for item in version.assembly_graph.get("parts", [])
        if isinstance(item, dict) and isinstance(item.get("part_id"), str)
    }
    parts = {part.part_id: part for part in version.parts}
    role_to_part_ids: dict[str, list[str]] = {}
    for part in version.parts:
        role_to_part_ids.setdefault(part.role, []).append(part.part_id)
    outputs_by_role: dict[str, list[str]] = {}
    for output in version.shape_program.get("outputs", []):
        if isinstance(output, dict) and isinstance(output.get("part_role"), str) and isinstance(output.get("operation_id"), str):
            outputs_by_role.setdefault(str(output["part_role"]), []).append(str(output["operation_id"]))
    ops_by_id = {
        str(item.get("operation_id")): item
        for item in version.shape_program.get("operations", [])
        if isinstance(item, dict) and isinstance(item.get("operation_id"), str)
    }
    connections = [
        item for item in version.assembly_graph.get("connections", [])
        if isinstance(item, dict) and isinstance(item.get("from_part_id"), str) and isinstance(item.get("to_part_id"), str)
    ]
    child_count: dict[str, int] = {}
    for graph_part in graph_parts.values():
        parent_id = graph_part.get("parent_part_id")
        if isinstance(parent_id, str):
            child_count[parent_id] = child_count.get(parent_id, 0) + 1

    suggestions: list[AgentStructureSuggestion] = []
    effective_locked = locked_part_ids or set()
    for part in version.parts:
        graph_part = graph_parts.get(part.part_id)
        if graph_part is None or part.part_id in effective_locked or part.locked or bool(graph_part.get("locked")):
            continue
        role_outputs = outputs_by_role.get(part.role, [])
        primitive_outputs = [
            operation_id for operation_id in role_outputs
            if isinstance(ops_by_id.get(operation_id), dict)
            and ops_by_id[operation_id].get("op") in _SPLITTABLE_PRIMITIVES
            and not ops_by_id[operation_id].get("inputs")
        ]
        part_connections = [
            connection for connection in connections
            if part.part_id in {connection.get("from_part_id"), connection.get("to_part_id")}
        ]
        if (
            len(role_to_part_ids.get(part.role, [])) == 1
            and len(primitive_outputs) >= 2
            and not part_connections
            and not graph_part.get("joints")
            and child_count.get(part.part_id, 0) == 0
        ):
            suggestion_id = _structure_suggestion_id(version.asset_version_id, "split_part", part.part_id, primitive_outputs[-1])
            suggestions.append(AgentStructureSuggestion(
                suggestion_id=suggestion_id,
                kind="split_part",
                asset_version_id=version.asset_version_id,
                part_id=part.part_id,
                target_part_id=None,
                affected_part_ids=[part.part_id],
                source_facts=["independent_shape_outputs", "no_connection_or_joint", "no_child_parts"],
                summary="将这个部件拆成两个可单独调整的外观部件",
            ))

    for connection in connections:
        parent_id = str(connection["from_part_id"])
        child_id = str(connection["to_part_id"])
        parent = parts.get(parent_id)
        child = parts.get(child_id)
        parent_graph = graph_parts.get(parent_id)
        child_graph = graph_parts.get(child_id)
        if not parent or not child or not parent_graph or not child_graph:
            continue
        if (
            parent.part_id in effective_locked
            or child.part_id in effective_locked
            or parent.locked
            or child.locked
            or bool(parent_graph.get("locked"))
            or bool(child_graph.get("locked"))
        ):
            continue
        if child_graph.get("joints") or child_count.get(child_id, 0) != 0:
            continue
        if any(
            isinstance(item, dict) and item.get("target_part_id") == child_id
            for item in parent_graph.get("joints", [])
        ):
            continue
        child_connections = [item for item in connections if child_id in {item.get("from_part_id"), item.get("to_part_id")}]
        if len(child_connections) != 1:
            continue
        if len(role_to_part_ids.get(parent.role, [])) != 1 or len(role_to_part_ids.get(child.role, [])) != 1:
            continue
        child_outputs = outputs_by_role.get(child.role, [])
        if not child_outputs or any(ops_by_id.get(operation_id, {}).get("op") not in _SPLITTABLE_PRIMITIVES for operation_id in child_outputs):
            continue
        suggestion_id = _structure_suggestion_id(version.asset_version_id, "merge_parts", parent_id, child_id)
        suggestions.append(AgentStructureSuggestion(
            suggestion_id=suggestion_id,
            kind="merge_parts",
            asset_version_id=version.asset_version_id,
            part_id=parent_id,
            target_part_id=child_id,
            affected_part_ids=[parent_id, child_id],
            source_facts=["direct_leaf_connection", "leaf_has_no_joint", "independent_shape_output"],
            summary="将这两个已连接的外观部件合并为一个可编辑部件",
        ))
    return suggestions


def _structure_suggestion_id(asset_version_id: str, kind: str, part_id: str, target: str) -> str:
    digest = hashlib.sha256(f"{asset_version_id}|{kind}|{part_id}|{target}".encode("utf-8")).hexdigest()[:18]
    return f"structure_{kind}_{digest}"


def _apply_split_suggestion(
    *,
    parts: list[dict[str, Any]],
    graph: dict[str, Any],
    program: dict[str, Any],
    bindings: dict[str, str],
    source_part_id: str,
    suggestion: AgentStructureSuggestion,
) -> None:
    source_part = next(item for item in parts if item["part_id"] == source_part_id)
    source_role = str(source_part["role"])
    output_ids = [
        str(item.get("operation_id")) for item in program.get("outputs", [])
        if isinstance(item, dict) and item.get("part_role") == source_role and isinstance(item.get("operation_id"), str)
    ]
    operation_id = output_ids[-1]
    operation = next(item for item in program.get("operations", []) if isinstance(item, dict) and item.get("operation_id") == operation_id)
    new_part_id = f"part_{hashlib.sha256(suggestion.suggestion_id.encode('utf-8')).hexdigest()[:18]}"
    new_role = _split_role(source_role, new_part_id)
    new_zone_id = f"zone_{new_role}"
    args = operation.setdefault("args", {})
    args["part_role"] = new_role
    args["zone_id"] = new_zone_id
    for output in program.get("outputs", []):
        if isinstance(output, dict) and output.get("operation_id") == operation_id:
            output["part_role"] = new_role
    position, size = _shape_operation_bounds(operation)
    new_part = {
        "part_id": new_part_id,
        "role": new_role,
        # Keep one AssemblyGraph root.  The new visual detail becomes a child
        # of the source part; it carries no invented connector or joint.
        "parent_part_id": source_part_id,
        "position_mm": position,
        "size_mm": size,
        "material_zone_ids": [new_zone_id],
        "editable_parameters": list(source_part.get("editable_parameters", [])),
        "editable_parameter_bindings": json.loads(json.dumps(source_part.get("editable_parameter_bindings", []))),
        "locked": False,
        "provenance": "agent_generated",
    }
    parts.append(new_part)
    source_graph = next(item for item in graph.get("parts", []) if isinstance(item, dict) and item.get("part_id") == source_part_id)
    graph.setdefault("parts", []).append({
        "part_id": new_part_id,
        "role": new_role,
        "parent_part_id": source_part_id,
        "geometry_source": "shape_program",
        "transform": {"position": position, "rotation": [0, 0, 0], "scale": [1, 1, 1]},
        "connectors": [],
        "joints": [],
        "material_zones": [new_zone_id],
        "editable_parameters": list(source_graph.get("editable_parameters", [])),
        "locked": False,
        "provenance": "agent_generated",
    })
    source_zone = str(source_part.get("material_zone_ids", ["primary"])[0])
    source_binding = bindings.get(f"{source_part_id}:{source_zone}")
    if source_binding:
        bindings[f"{new_part_id}:{new_zone_id}"] = source_binding


def _apply_merge_suggestion(
    *,
    parts: list[dict[str, Any]],
    graph: dict[str, Any],
    program: dict[str, Any],
    bindings: dict[str, str],
    survivor_part_id: str,
    absorbed_part_id: str,
    suggestion: AgentStructureSuggestion,
) -> None:
    survivor = next(item for item in parts if item["part_id"] == survivor_part_id)
    absorbed = next(item for item in parts if item["part_id"] == absorbed_part_id)
    survivor_role = str(survivor["role"])
    absorbed_role = str(absorbed["role"])
    for operation in program.get("operations", []):
        if isinstance(operation, dict) and operation.get("args", {}).get("part_role") == absorbed_role:
            operation.setdefault("args", {})["part_role"] = survivor_role
    for output in program.get("outputs", []):
        if isinstance(output, dict) and output.get("part_role") == absorbed_role:
            output["part_role"] = survivor_role
    survivor["material_zone_ids"] = list(dict.fromkeys([*survivor.get("material_zone_ids", []), *absorbed.get("material_zone_ids", [])]))
    survivor["position_mm"], survivor["size_mm"] = _combined_part_bounds(survivor, absorbed)
    parts[:] = [item for item in parts if item["part_id"] != absorbed_part_id]
    survivor_graph = next(item for item in graph.get("parts", []) if isinstance(item, dict) and item.get("part_id") == survivor_part_id)
    absorbed_graph = next(item for item in graph.get("parts", []) if isinstance(item, dict) and item.get("part_id") == absorbed_part_id)
    survivor_graph.setdefault("material_zones", [])[:] = list(dict.fromkeys([*survivor_graph.get("material_zones", []), *absorbed_graph.get("material_zones", [])]))
    survivor_graph.setdefault("transform", {})["position"] = list(survivor["position_mm"])
    original_connections = list(graph.get("connections", []))
    removed_connection_ids = {
        str(item.get("connection_id"))
        for item in original_connections
        if isinstance(item, dict) and absorbed_part_id in {item.get("from_part_id"), item.get("to_part_id")}
    }
    removed_connector_ids = {
        str(item.get("from_connector_id"))
        for item in original_connections if isinstance(item, dict) and item.get("connection_id") in removed_connection_ids
    }
    graph["connections"] = [
        item for item in original_connections
        if isinstance(item, dict) and item.get("connection_id") not in removed_connection_ids
    ]
    survivor_graph["connectors"] = [
        connector for connector in survivor_graph.get("connectors", [])
        if isinstance(connector, dict) and connector.get("connector_id") not in removed_connector_ids
    ]
    graph["parts"] = [item for item in graph.get("parts", []) if isinstance(item, dict) and item.get("part_id") != absorbed_part_id]
    for key, value in list(bindings.items()):
        if key.startswith(f"{absorbed_part_id}:"):
            zone = key.split(":", 1)[1]
            bindings[f"{survivor_part_id}:{zone}"] = value
            del bindings[key]


def _shape_operation_bounds(operation: Mapping[str, Any]) -> tuple[list[float], list[float]]:
    args = operation.get("args", {}) if isinstance(operation.get("args"), Mapping) else {}
    position = args.get("position", [0, 0, 0])
    if not isinstance(position, list) or len(position) != 3:
        position = [0, 0, 0]
    if operation.get("op") in {"cylinder", "capsule"}:
        radius = float(args.get("radius", 1))
        height = float(args.get("height", 1))
        size = [radius * 2, height, radius * 2]
    else:
        size = args.get("size", [1, 1, 1])
    if not isinstance(size, list) or len(size) != 3:
        size = [1, 1, 1]
    return [round(float(value), 4) for value in position], [round(float(value), 4) for value in size]


def _combined_part_bounds(first: Mapping[str, Any], second: Mapping[str, Any]) -> tuple[list[float], list[float]]:
    first_position, first_size = _part_position_and_size(first)
    second_position, second_size = _part_position_and_size(second)
    lower = [min(first_position[index] - first_size[index] / 2, second_position[index] - second_size[index] / 2) for index in range(3)]
    upper = [max(first_position[index] + first_size[index] / 2, second_position[index] + second_size[index] / 2) for index in range(3)]
    return [round((lower[index] + upper[index]) / 2, 4) for index in range(3)], [round(upper[index] - lower[index], 4) for index in range(3)]


def _part_position_and_size(part: Mapping[str, Any]) -> tuple[list[float], list[float]]:
    position = part.get("position_mm", [0, 0, 0])
    size = part.get("size_mm", [1, 1, 1])
    return [float(value) for value in position], [float(value) for value in size]


def _split_role(source_role: str, new_part_id: str) -> str:
    suffix = new_part_id.removeprefix("part_")[:8]
    prefix = source_role[: max(1, 54 - len(suffix))]
    return f"{prefix}_detail_{suffix}"


def _apply_parameter(part: dict[str, Any], graph_part: Optional[dict[str, Any]], shape_ops: list[dict[str, Any]], path: str, value: float) -> None:
    if path.startswith("transform.position."):
        axis = "xyz".index(path.rsplit(".", 1)[1])
        part["position_mm"][axis] = value
        if graph_part is not None:
            graph_part.setdefault("transform", {}).setdefault("position", [0, 0, 0])[axis] = value
        for shape_op in shape_ops:
            shape_op["args"]["position"][axis] = value
        return
    axis = "xyz".index(path.rsplit(".", 1)[1])
    if graph_part is not None:
        graph_part.setdefault("transform", {}).setdefault("scale", [1, 1, 1])[axis] = value
    for shape_op in shape_ops:
        args = shape_op["args"]
        if shape_op.get("op") == "cylinder":
            if axis == 1:
                args["height"] = round(float(args.get("height", part["size_mm"][1])) * value, 4)
            else:
                args["radius"] = round(float(args.get("radius", part["size_mm"][axis] / 2)) * value, 4)
        elif isinstance(args.get("size"), list):
            args["size"][axis] = round(float(part["size_mm"][axis]) * value, 4)


def _has_part_cycle(parts: list[BlockoutPartCandidate]) -> bool:
    parents = {part.part_id: part.parent_part_id for part in parts}
    for part_id in parents:
        seen: set[str] = set()
        current: Optional[str] = part_id
        while current is not None:
            if current in seen:
                return True
            seen.add(current)
            current = parents.get(current)
    return False


def _unexpected_aabb_overlaps(
    parts: list[BlockoutPartCandidate],
    assembly_graph: Mapping[str, Any],
) -> list[tuple[str, str]]:
    """Return obvious sibling collisions without claiming exact mesh collision.

    Direct parent/child pairs and explicit AssemblyGraph connections are normal
    mounts in a concept model, so this guard intentionally ignores them.  The
    remaining broad-phase overlap threshold catches accidental drag/scale
    mistakes cheaply and deterministically.
    """
    connected: set[frozenset[str]] = set()
    for connection in assembly_graph.get("connections", []) if isinstance(assembly_graph.get("connections", []), list) else []:
        if isinstance(connection, dict):
            first = connection.get("from_part_id")
            second = connection.get("to_part_id")
            if isinstance(first, str) and isinstance(second, str):
                connected.add(frozenset((first, second)))
    overlaps: list[tuple[str, str]] = []
    for first_index, first in enumerate(parts):
        first_min, first_max = _part_aabb(first)
        first_volume = _aabb_volume(first_min, first_max)
        if first_volume <= 0:
            continue
        for second in parts[first_index + 1:]:
            if (
                first.parent_part_id == second.part_id
                or second.parent_part_id == first.part_id
                or frozenset((first.part_id, second.part_id)) in connected
            ):
                continue
            second_min, second_max = _part_aabb(second)
            second_volume = _aabb_volume(second_min, second_max)
            if second_volume <= 0:
                continue
            overlap = [min(first_max[axis], second_max[axis]) - max(first_min[axis], second_min[axis]) for axis in range(3)]
            if any(value <= 0 for value in overlap):
                continue
            overlap_volume = overlap[0] * overlap[1] * overlap[2]
            if overlap_volume / min(first_volume, second_volume) >= 0.12:
                overlaps.append((first.part_id, second.part_id))
    return overlaps


def _part_aabb(part: BlockoutPartCandidate) -> tuple[list[float], list[float]]:
    minimum = [float(part.position_mm[axis]) - float(part.size_mm[axis]) / 2 for axis in range(3)]
    maximum = [float(part.position_mm[axis]) + float(part.size_mm[axis]) / 2 for axis in range(3)]
    return minimum, maximum


def _aabb_volume(minimum: list[float], maximum: list[float]) -> float:
    return max(0.0, maximum[0] - minimum[0]) * max(0.0, maximum[1] - minimum[1]) * max(0.0, maximum[2] - minimum[2])


def _propagate_joint_delta(
    parts: list[dict[str, Any]],
    graph: dict[str, Any],
    program: dict[str, Any],
    pivot_part_id: str,
    delta_z: float,
) -> None:
    """Rotate descendant part pivots for a concept-level joint preview.

    This is intentionally a visual 2-D-in-Z propagation, not a dynamics or
    collision solver. It keeps the robotic-arm chain visually connected while
    remaining deterministic and cheap for the zero-install runtime.
    """
    if abs(delta_z) < 1e-9:
        return
    graph_parts = {
        item.get("part_id"): item
        for item in graph.get("parts", [])
        if isinstance(item, dict) and item.get("part_id")
    }
    part_map = {item["part_id"]: item for item in parts}
    role_by_part = {item["part_id"]: item.get("role") for item in parts}
    queue = [pivot_part_id]
    descendants: list[str] = []
    while queue:
        parent_id = queue.pop(0)
        for candidate in parts:
            if candidate.get("parent_part_id") == parent_id:
                descendants.append(candidate["part_id"])
                queue.append(candidate["part_id"])
    pivot = part_map.get(pivot_part_id)
    if pivot is None:
        return
    px, py, _ = (float(value) for value in pivot["position_mm"])
    cos_delta = math.cos(delta_z)
    sin_delta = math.sin(delta_z)
    for descendant_id in descendants:
        descendant = part_map[descendant_id]
        x, y, z = (float(value) for value in descendant["position_mm"])
        dx, dy = x - px, y - py
        descendant["position_mm"] = [
            round(px + dx * cos_delta - dy * sin_delta, 4),
            round(py + dx * sin_delta + dy * cos_delta, 4),
            z,
        ]
        graph_part = graph_parts.get(descendant_id)
        if graph_part is not None:
            transform = graph_part.setdefault("transform", {})
            transform["position"] = list(descendant["position_mm"])
            rotation = transform.setdefault("rotation", [0, 0, 0])
            rotation[2] = round(float(rotation[2]) + delta_z, 6)
        role = role_by_part.get(descendant_id)
        for shape_op in program.get("operations", []):
            if not isinstance(shape_op, dict) or shape_op.get("args", {}).get("part_role") != role:
                continue
            args = shape_op.setdefault("args", {})
            args["position"] = list(descendant["position_mm"])
            rotation = args.setdefault("rotation", [0, 0, 0])
            rotation[2] = round(float(rotation[2]) + delta_z, 6)


def _hash_json(payload: Mapping[str, Any]) -> str:
    return hashlib.sha256(_canonical_json(payload).encode("utf-8")).hexdigest()


def _is_external_glb_reference(version: AgentAssetVersion) -> bool:
    return version.shape_program.get("schema_version") == "ExternalGLBReference@1"


def _safe_import_file_name(value: str) -> str:
    """Keep a display-only filename; object storage never uses user paths."""
    name = value.replace("\\", "/").rsplit("/", 1)[-1].replace("\x00", "").strip()
    return name[:180] or "imported-model.glb"


def _canonical_json(payload: Any) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _write_stable_zip_entry(archive: zipfile.ZipFile, file_name: str, payload: bytes) -> None:
    """Write a controlled ZIP member without wall-clock or platform variance."""
    entry = zipfile.ZipInfo(file_name, date_time=(1980, 1, 1, 0, 0, 0))
    entry.compress_type = zipfile.ZIP_DEFLATED
    entry.create_system = 3
    entry.external_attr = 0o600 << 16
    archive.writestr(entry, payload, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def _validate_png_payload(payload: bytes, *, width: int, height: int) -> None:
    if len(payload) < 33 or payload[:8] != b"\x89PNG\r\n\x1a\n" or payload[12:16] != b"IHDR":
        raise AgentAssetError("AGENT_RENDER_READBACK_FAILED", "概念图不是有效 PNG。", status_code=409)
    actual_width, actual_height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload[16:29])
    if (actual_width, actual_height) != (width, height) or (bit_depth, color_type, compression, filtering, interlace) != (8, 6, 0, 0, 0):
        raise AgentAssetError("AGENT_RENDER_READBACK_FAILED", "概念图 PNG 元数据与请求尺寸不一致。", status_code=409)


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex[:20]}"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()
