from __future__ import annotations

import base64
import binascii
import hashlib
import json
import struct
from pathlib import PurePosixPath
from typing import Any, Optional

from forgecad_agent.application.concept_models import (
    ModuleAssetCatalogMetadata,
    ModuleAssetCatalogMetadataInput,
    ModuleAssetListResponse,
    ModuleAssetRecord,
    ModuleGraphRecord,
    ModuleGraphValidationIssue,
    ModuleGraphValidationResponse,
    RegisterModuleAssetRequest,
    UpdateModuleAssetCatalogMetadataRequest,
    ValidateModuleGraphRequest,
)
from forgecad_agent.application.concept_jobs import record_completed_job
from forgecad_agent.domain.concepts.models import ModuleAssetManifest, ModuleCategory, ModuleGraph
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork
from forgecad_agent.infrastructure.storage import ContentAddressedStore
from forgecad_agent.infrastructure.storage import ObjectStoreError


class ConceptModuleError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ConceptModuleIdempotencyConflict(RuntimeError):
    pass


class ConceptModuleService:
    """Register immutable GLB modules and validate/persist ModuleGraph instances."""

    def __init__(
        self,
        connection_factory: SQLiteConnectionFactory,
        object_store: ContentAddressedStore,
    ) -> None:
        self.connection_factory = connection_factory
        self.object_store = object_store

    def register_module(
        self,
        request: RegisterModuleAssetRequest,
        idempotency_key: str,
    ) -> ModuleAssetRecord:
        scope = "POST /api/v1/module-assets"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise ConceptModuleIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ModuleAssetRecord.model_validate_json(existing.response_json)
            if unit_of_work.modules.get_manifest(request.manifest.module_id) is not None:
                raise ConceptModuleError(
                    "MODULE_ALREADY_EXISTS",
                    f"Module ID is already registered: {request.manifest.module_id}",
                )

            logical_path = _validated_logical_path(request.logical_path)
            payload = _decode_glb(request.glb_data_base64)
            _validate_glb_envelope(payload)
            payload_sha256 = hashlib.sha256(payload).hexdigest()
            if payload_sha256 != request.manifest.sha256:
                raise ConceptModuleError(
                    "MODULE_HASH_MISMATCH",
                    "Module manifest sha256 does not match the uploaded GLB.",
                )

            stored = self.object_store.put(payload, extension=".glb")
            now = _utc_now()
            manifest_json = _canonical_json(request.manifest.model_dump(mode="json"))
            unit_of_work.concept_assets.add(
                asset_id=request.manifest.asset_id,
                project_id=None,
                version_id=None,
                role="module_glb",
                logical_path=logical_path,
                object_path=stored.relative_path,
                sha256=stored.sha256,
                byte_size=stored.byte_size,
                mime_type="model/gltf-binary",
                metadata_json=_canonical_json(
                    {
                        "module_id": request.manifest.module_id,
                        "pack_id": request.manifest.pack_id,
                        "category": request.manifest.category,
                    }
                ),
                created_at=now,
            )
            unit_of_work.modules.add_manifest(
                module_id=request.manifest.module_id,
                pack_id=request.manifest.pack_id,
                category=request.manifest.category,
                asset_id=request.manifest.asset_id,
                schema_version=request.manifest.schema_version,
                manifest_json=manifest_json,
                manifest_sha256=hashlib.sha256(manifest_json.encode("utf-8")).hexdigest(),
                created_at=now,
            )
            if request.thumbnail_png_base64:
                thumbnail = _decode_png(request.thumbnail_png_base64)
                thumbnail_stored = self.object_store.put(thumbnail, extension=".png")
                unit_of_work.concept_assets.add(
                    asset_id=_thumbnail_asset_id(request.manifest.asset_id),
                    project_id=None,
                    version_id=None,
                    role="other",
                    logical_path=(
                        f"packs/{request.manifest.pack_id}/"
                        f"{request.manifest.module_id}/thumbnail.png"
                    ),
                    object_path=thumbnail_stored.relative_path,
                    sha256=thumbnail_stored.sha256,
                    byte_size=thumbnail_stored.byte_size,
                    mime_type="image/png",
                    metadata_json=_canonical_json(
                        {
                            "module_id": request.manifest.module_id,
                            "kind": "module_thumbnail",
                            "pack_id": request.manifest.pack_id,
                        }
                    ),
                    created_at=now,
                )
            for connector in request.manifest.connectors:
                unit_of_work.modules.add_connector(
                    connector_id=connector.connector_id,
                    module_id=request.manifest.module_id,
                    slot=connector.slot,
                    connector_type=connector.connector_type,
                    transform_json=_canonical_json(connector.transform.model_dump(mode="json")),
                    scale_min=connector.scale_range[0],
                    scale_max=connector.scale_range[1],
                    exclusive=connector.exclusive,
                    created_at=now,
                )
            catalog_metadata = request.catalog_metadata or _default_catalog_metadata(
                request.manifest,
            )
            unit_of_work.modules.upsert_catalog_metadata(
                module_id=request.manifest.module_id,
                **_catalog_metadata_values(catalog_metadata, updated_at=now),
            )
            response = ModuleAssetRecord(
                manifest=request.manifest,
                logical_path=logical_path,
                object_path=stored.relative_path,
                byte_size=stored.byte_size,
                created_at=now,
                catalog_metadata=_catalog_metadata_record(catalog_metadata, updated_at=now),
            )
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def list_modules(
        self,
        *,
        pack_id: Optional[str] = None,
        category: Optional[ModuleCategory] = None,
        query: Optional[str] = None,
        review_status: Optional[str] = None,
        tag: Optional[str] = None,
        catalog_path: Optional[str] = None,
    ) -> ModuleAssetListResponse:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            rows = unit_of_work.modules.list_manifests(
                pack_id=pack_id,
                category=category,
                query=query,
                review_status=review_status,
                tag=tag,
                catalog_path=catalog_path,
            )
            items = [_module_record(row) for row in rows]
        return ModuleAssetListResponse(
            items=items,
            pack_id=pack_id,
            category=category,
            next_cursor=None,
        )

    def update_catalog_metadata(
        self,
        module_id: str,
        request: UpdateModuleAssetCatalogMetadataRequest,
        idempotency_key: str,
    ) -> ModuleAssetRecord:
        scope = f"PUT /api/v1/module-assets/{module_id}/catalog-metadata"
        request_hash = _hash_json(request.model_dump(mode="json"))
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing is not None:
                if existing.request_hash != request_hash:
                    raise ConceptModuleIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ModuleAssetRecord.model_validate_json(existing.response_json)
            row = unit_of_work.modules.get_manifest(module_id)
            if row is None:
                raise ConceptModuleError("MODULE_NOT_FOUND", f"Module is not registered: {module_id}")
            now = _utc_now()
            metadata = ModuleAssetCatalogMetadataInput.model_validate(
                request.model_dump(exclude={"client_request_id"})
            )
            unit_of_work.modules.upsert_catalog_metadata(
                module_id=module_id,
                **_catalog_metadata_values(metadata, updated_at=now),
            )
            refreshed = unit_of_work.modules.get_manifest(module_id)
            if refreshed is None:
                raise ConceptModuleError("MODULE_NOT_FOUND", f"Module is not registered: {module_id}")
            response = _module_record(refreshed)
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=now,
            )
            return response

    def validate_graph(
        self,
        graph_id: str,
        request: ValidateModuleGraphRequest,
        idempotency_key: str,
    ) -> ModuleGraphValidationResponse:
        if request.graph.graph_id != graph_id:
            raise ConceptModuleError(
                "INVALID_REQUEST",
                "ModuleGraph graph_id does not match the route graph_id.",
            )
        scope = f"POST /api/v1/module-graphs/{graph_id}/validate"
        request_hash = _hash_json(request.model_dump(mode="json"))
        graph_json = _canonical_json(request.graph.model_dump(mode="json"))
        graph_sha256 = hashlib.sha256(graph_json.encode("utf-8")).hexdigest()

        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            existing_idempotency = unit_of_work.idempotency.get(scope, idempotency_key)
            if existing_idempotency is not None:
                if existing_idempotency.request_hash != request_hash:
                    raise ConceptModuleIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return ModuleGraphValidationResponse.model_validate_json(
                    existing_idempotency.response_json
                )

            project = unit_of_work.concept_projects.get_active(request.graph.project_id)
            if project is None:
                raise ConceptModuleError("PROJECT_NOT_FOUND", "Concept project not found.")
            profile = unit_of_work.domain_profiles.get_active(str(project["profile_id"]))
            if profile is None:
                raise ConceptModuleError(
                    "DOMAIN_PROFILE_NOT_FOUND",
                    "The project domain profile is unavailable.",
                )
            profile_pack_id = str(profile["pack_id"])

            issues = validate_registered_graph(
                unit_of_work,
                graph=request.graph,
                profile_pack_id=profile_pack_id,
            )
            persisted = False
            if not issues and request.persist:
                existing_graph = unit_of_work.modules.get_graph(graph_id)
                if existing_graph is not None:
                    if existing_graph["graph_sha256"] != graph_sha256:
                        raise ConceptModuleError(
                            "MODULE_GRAPH_CONFLICT",
                            "ModuleGraph ID already exists with different content.",
                        )
                    persisted = True
                else:
                    now = _utc_now()
                    unit_of_work.modules.add_graph(
                        graph_id=graph_id,
                        project_id=request.graph.project_id,
                        version_id=None,
                        root_node_id=request.graph.root_node_id,
                        schema_version=request.graph.schema_version,
                        graph_json=graph_json,
                        graph_sha256=graph_sha256,
                        validation_status="valid",
                        nodes=[
                            {
                                **node.model_dump(mode="json", exclude={"transform"}),
                                "transform_json": _canonical_json(
                                    node.transform.model_dump(mode="json")
                                ),
                            }
                            for node in request.graph.nodes
                        ],
                        edges=[edge.model_dump(mode="json") for edge in request.graph.edges],
                        created_at=now,
                    )
                    persisted = True

            response = ModuleGraphValidationResponse(
                graph_id=graph_id,
                project_id=request.graph.project_id,
                valid=not issues,
                persisted=persisted,
                graph_sha256=graph_sha256,
                issues=issues,
            )
            job_id = record_completed_job(
                unit_of_work,
                project_id=request.graph.project_id,
                version_id=None,
                job_type="validate_graph",
                input_payload={
                    "graph_id": graph_id,
                    "graph_sha256": graph_sha256,
                    "persist_requested": request.persist,
                },
                output_payload={
                    "graph_id": graph_id,
                    "valid": response.valid,
                    "persisted": response.persisted,
                    "graph_sha256": graph_sha256,
                    "issue_count": len(issues),
                },
                steps=[
                    (
                        "resolve_modules",
                        "Resolved registered modules and connector metadata.",
                        0.35,
                        {"node_count": len(request.graph.nodes)},
                    ),
                    (
                        "validate_graph",
                        "ModuleGraph validation completed.",
                        0.8,
                        {"valid": response.valid, "issue_count": len(issues)},
                    ),
                    (
                        "persist_graph",
                        "Stored validation result and immutable graph when valid.",
                        1.0,
                        {"persisted": response.persisted},
                    ),
                ],
            )
            response = response.model_copy(update={"job_id": job_id})
            unit_of_work.idempotency.add(
                scope=scope,
                key=idempotency_key,
                request_hash=request_hash,
                response_json=_canonical_json(response.model_dump(mode="json")),
                created_at=_utc_now(),
            )
            return response

    def get_graph(self, graph_id: str) -> ModuleGraphRecord:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.modules.get_graph(graph_id)
            if row is None:
                raise ConceptModuleError("MODULE_GRAPH_NOT_FOUND", "ModuleGraph not found.")
            return ModuleGraphRecord(
                graph=ModuleGraph.model_validate_json(row["graph_json"]),
                graph_sha256=row["graph_sha256"],
                validation_status=row["validation_status"],
                created_at=row["created_at"],
                updated_at=row["updated_at"],
            )

    def read_module(self, module_id: str) -> tuple[bytes, str, str]:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.modules.get_manifest(module_id)
            if row is None:
                raise ConceptModuleError("MODULE_NOT_FOUND", "Module asset not found.")
            try:
                payload = self.object_store.read(
                    str(row["object_path"]),
                    expected_sha256=str(row["sha256"]),
                )
            except ObjectStoreError as exc:
                raise ConceptModuleError(
                    "MODULE_ASSET_UNAVAILABLE",
                    f"Module GLB is unavailable: {exc}",
                ) from exc
            return payload, f"{module_id}.glb", str(row["sha256"])

    def ensure_module_thumbnail(self, module_id: str, png_payload: bytes) -> bool:
        """Persist a Pack thumbnail for legacy module registrations when absent."""

        _validate_png(png_payload)
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.modules.get_manifest(module_id)
            if row is None:
                raise ConceptModuleError("MODULE_NOT_FOUND", "Module asset not found.")
            asset_id = _thumbnail_asset_id(str(row["asset_id"]))
            if unit_of_work.concept_assets.get_active(asset_id) is not None:
                return False
            stored = self.object_store.put(png_payload, extension=".png")
            unit_of_work.concept_assets.add(
                asset_id=asset_id,
                project_id=None,
                version_id=None,
                role="other",
                logical_path=f"packs/{row['pack_id']}/{module_id}/thumbnail.png",
                object_path=stored.relative_path,
                sha256=stored.sha256,
                byte_size=stored.byte_size,
                mime_type="image/png",
                metadata_json=_canonical_json(
                    {"module_id": module_id, "kind": "module_thumbnail", "pack_id": row["pack_id"]}
                ),
                created_at=_utc_now(),
            )
            return True

    def read_module_thumbnail(self, module_id: str) -> tuple[bytes, str, str]:
        with SQLiteUnitOfWork(self.connection_factory) as unit_of_work:
            row = unit_of_work.modules.get_manifest(module_id)
            if row is None:
                raise ConceptModuleError("MODULE_NOT_FOUND", "Module asset not found.")
            thumbnail = unit_of_work.concept_assets.get_active(
                _thumbnail_asset_id(str(row["asset_id"]))
            )
            if thumbnail is None:
                raise ConceptModuleError(
                    "MODULE_THUMBNAIL_NOT_FOUND",
                    "Module thumbnail is unavailable; re-import the Module Pack with thumbnails.",
                )
            try:
                payload = self.object_store.read(
                    str(thumbnail["object_path"]), expected_sha256=str(thumbnail["sha256"])
                )
            except ObjectStoreError as exc:
                raise ConceptModuleError(
                    "MODULE_ASSET_UNAVAILABLE", f"Module thumbnail is unavailable: {exc}"
                ) from exc
            return payload, f"{module_id}.png", str(thumbnail["sha256"])


def validate_registered_graph(
    unit_of_work: SQLiteUnitOfWork,
    *,
    graph: ModuleGraph,
    profile_pack_id: str,
) -> list[ModuleGraphValidationIssue]:
    issues: list[ModuleGraphValidationIssue] = []
    nodes = {node.node_id: node for node in graph.nodes}
    module_rows: dict[str, Any] = {}
    for module_id in sorted({node.module_id for node in graph.nodes}):
        row = unit_of_work.modules.get_manifest(module_id)
        if row is None:
            issues.append(
                ModuleGraphValidationIssue(
                    code="MODULE_NOT_FOUND",
                    message=f"Module is not registered: {module_id}",
                )
            )
            continue
        module_rows[module_id] = row
        if row["pack_id"] != profile_pack_id:
            issues.append(
                ModuleGraphValidationIssue(
                    code="MODULE_PACK_MISMATCH",
                    message=f"Module {module_id} does not belong to project pack {profile_pack_id}.",
                )
            )
    if issues:
        return issues

    connector_rows = unit_of_work.modules.connector_map(list(module_rows))
    for edge in graph.edges:
        source_node = nodes[edge.from_node_id]
        target_node = nodes[edge.to_node_id]
        source = connector_rows.get(edge.from_connector_id)
        target = connector_rows.get(edge.to_connector_id)
        if source is None or source["module_id"] != source_node.module_id:
            issues.append(
                ModuleGraphValidationIssue(
                    code="CONNECTOR_NOT_FOUND",
                    message="Source connector does not belong to the source module.",
                    node_id=edge.from_node_id,
                    edge_id=edge.edge_id,
                )
            )
            continue
        if target is None or target["module_id"] != target_node.module_id:
            issues.append(
                ModuleGraphValidationIssue(
                    code="CONNECTOR_NOT_FOUND",
                    message="Target connector does not belong to the target module.",
                    node_id=edge.to_node_id,
                    edge_id=edge.edge_id,
                )
            )
            continue
        if source["connector_type"] != target["connector_type"]:
            issues.append(
                ModuleGraphValidationIssue(
                    code="CONNECTOR_TYPE_MISMATCH",
                    message=(
                        f"Connector types do not match: {source['connector_type']} vs "
                        f"{target['connector_type']}."
                    ),
                    edge_id=edge.edge_id,
                )
            )
        for node, connector in ((source_node, source), (target_node, target)):
            if any(
                scale < connector["scale_min"] or scale > connector["scale_max"]
                for scale in node.transform.scale
            ):
                issues.append(
                    ModuleGraphValidationIssue(
                        code="CONNECTOR_SCALE_OUT_OF_RANGE",
                        message=f"Node scale exceeds connector range: {node.node_id}.",
                        node_id=node.node_id,
                        edge_id=edge.edge_id,
                    )
                )
    return issues


def _module_record(row: Any) -> ModuleAssetRecord:
    return ModuleAssetRecord(
        manifest=ModuleAssetManifest.model_validate_json(row["manifest_json"]),
        logical_path=row["logical_path"],
        object_path=row["object_path"],
        byte_size=row["byte_size"],
        mime_type=row["mime_type"],
        created_at=row["created_at"],
        catalog_metadata=ModuleAssetCatalogMetadata(
            display_name=row["display_name"],
            description=row["description"],
            tags=json.loads(row["tags_json"]),
            catalog_path=row["catalog_path"],
            origin_claim=row["origin_claim"],
            creator_name=row["creator_name"],
            review_status=row["review_status"],
            reviewer_name=row["reviewer_name"],
            reviewed_at=row["reviewed_at"],
            review_note=row["review_note"],
            updated_at=row["metadata_updated_at"],
        ),
    )


def _default_catalog_metadata(manifest: ModuleAssetManifest) -> ModuleAssetCatalogMetadataInput:
    return ModuleAssetCatalogMetadataInput(
        display_name=manifest.module_id.removeprefix("module_").replace("_", " ").title(),
        description="资产信息待补充。",
        tags=[],
        catalog_path=manifest.category,
        origin_claim="self_declared_original",
        creator_name="ForgeCAD Author",
        review_status="pending_review",
        review_note="已声明为本人原创，等待独立审阅。",
    )


def _catalog_metadata_values(
    metadata: ModuleAssetCatalogMetadataInput,
    *,
    updated_at: str,
) -> dict[str, Any]:
    return {
        "display_name": metadata.display_name,
        "description": metadata.description,
        "tags_json": _canonical_json(metadata.tags),
        "catalog_path": metadata.catalog_path,
        "origin_claim": metadata.origin_claim,
        "creator_name": metadata.creator_name,
        "review_status": metadata.review_status,
        "reviewer_name": metadata.reviewer_name,
        "reviewed_at": metadata.reviewed_at,
        "review_note": metadata.review_note,
        "updated_at": updated_at,
    }


def _catalog_metadata_record(
    metadata: ModuleAssetCatalogMetadataInput,
    *,
    updated_at: str,
) -> ModuleAssetCatalogMetadata:
    return ModuleAssetCatalogMetadata(**metadata.model_dump(), updated_at=updated_at)


def _decode_glb(value: str) -> bytes:
    try:
        payload = base64.b64decode(value, validate=True)
    except binascii.Error as exc:
        raise ConceptModuleError("INVALID_GLB", "glb_data_base64 is not valid base64.") from exc
    if len(payload) > 64 * 1024 * 1024:
        raise ConceptModuleError("INVALID_GLB", "Module GLB exceeds the 64 MiB R2 limit.")
    return payload


def _decode_png(value: str) -> bytes:
    try:
        payload = base64.b64decode(value, validate=True)
    except binascii.Error as exc:
        raise ConceptModuleError("INVALID_REQUEST", "thumbnail_png_base64 is not valid base64.") from exc
    _validate_png(payload)
    return payload


def _validate_png(payload: bytes) -> None:
    if len(payload) < 24 or len(payload) > 16 * 1024 * 1024:
        raise ConceptModuleError("INVALID_REQUEST", "Module thumbnail must be a PNG under 16 MiB.")
    if payload[:8] != b"\x89PNG\r\n\x1a\n" or payload[12:16] != b"IHDR":
        raise ConceptModuleError("INVALID_REQUEST", "Module thumbnail must be a PNG file.")


def _thumbnail_asset_id(asset_id: str) -> str:
    return f"{asset_id}_thumbnail"


def _validate_glb_envelope(payload: bytes) -> None:
    if len(payload) < 20:
        raise ConceptModuleError("INVALID_GLB", "Module GLB is too short.")
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    if magic != b"glTF" or version != 2 or declared_length != len(payload):
        raise ConceptModuleError("INVALID_GLB", "Module GLB header is invalid.")
    json_chunk_length, json_chunk_type = struct.unpack_from("<II", payload, 12)
    if json_chunk_type != 0x4E4F534A or 20 + json_chunk_length > len(payload):
        raise ConceptModuleError("INVALID_GLB", "Module GLB JSON chunk is invalid.")
    try:
        document = json.loads(payload[20 : 20 + json_chunk_length].decode("utf-8").rstrip(" \x00"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ConceptModuleError("INVALID_GLB", "Module GLB JSON cannot be decoded.") from exc
    if document.get("asset", {}).get("version") != "2.0":
        raise ConceptModuleError("INVALID_GLB", "Module GLB must declare glTF 2.0.")


def _validated_logical_path(value: str) -> str:
    path = PurePosixPath(value.strip())
    if path.is_absolute() or ".." in path.parts or "://" in value or path.suffix.lower() != ".glb":
        raise ConceptModuleError(
            "INVALID_REQUEST",
            "Module logical_path must be a relative .glb path without traversal.",
        )
    return str(path)


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _hash_json(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def _utc_now() -> str:
    from datetime import datetime, timezone

    return datetime.now(timezone.utc).isoformat()
