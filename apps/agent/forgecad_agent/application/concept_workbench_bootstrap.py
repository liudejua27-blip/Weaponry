"""First-run installation for the bundled non-functional Concept Workbench Pack."""

from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path

from forgecad_agent.application.concept_models import (
    AppendConceptVersionRequest,
    ConceptProjectDetail,
    ModuleAssetManifest,
    ModuleAssetCatalogMetadataInput,
    RegisterModuleAssetRequest,
    ValidateModuleGraphRequest,
)
from forgecad_agent.application.concept_modules import ConceptModuleService
from forgecad_agent.application.concept_projects import ConceptProjectError, ConceptProjectService
from forgecad_agent.domain.concepts.models import ModuleGraph


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
DEFAULT_PACK_ROOT = REPOSITORY_ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"


class ConceptWorkbenchBootstrapService:
    """Create a usable first project without requiring hidden smoke-test seeding."""

    def __init__(
        self,
        projects: ConceptProjectService,
        modules: ConceptModuleService,
    ) -> None:
        self.projects = projects
        self.modules = modules

    def initialize_project(self, project_id: str, idempotency_key: str) -> ConceptProjectDetail:
        project = self.projects.get_project(project_id)
        current_version_id = project.current_version_id
        if not current_version_id:
            raise ConceptProjectError("VERSION_NOT_FOUND", "Project has no current version.")
        current_version = self.projects.get_version(current_version_id)
        if current_version.module_graph_id:
            return project

        self._ensure_profile_pack(project.profile.pack_id)
        graph = _starter_graph(project_id)
        validated = self.modules.validate_graph(
            graph.graph_id,
            ValidateModuleGraphRequest(
                client_request_id=f"{idempotency_key}:graph",
                graph=graph,
                persist=True,
            ),
            f"{idempotency_key}:graph",
        )
        if not validated.valid:
            issues = "; ".join(f"{item.code}: {item.message}" for item in validated.issues)
            raise ConceptProjectError("DEFAULT_WORKBENCH_INVALID", issues or "Starter ModuleGraph is invalid.")

        return self.projects.append_version(
            project_id,
            AppendConceptVersionRequest(
                client_request_id=f"{idempotency_key}:bind",
                parent_version_id=current_version.version_id,
                summary="初始化内置 Module Pack 与首个可交互 ModuleGraph。",
                spec=current_version.spec,
                module_graph_id=graph.graph_id,
            ),
            f"{idempotency_key}:bind",
        )

    def _ensure_profile_pack(self, expected_pack_id: str) -> None:
        root = _configured_pack_root()
        try:
            raw_pack = json.loads((root / "pack.json").read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ConceptProjectError(
                "DEFAULT_PACK_UNAVAILABLE", f"Bundled Module Pack cannot be loaded: {exc}"
            ) from exc
        if raw_pack.get("pack_id") != expected_pack_id:
            raise ConceptProjectError(
                "DEFAULT_PACK_MISMATCH",
                f"Bundled Pack {raw_pack.get('pack_id')!r} does not match profile {expected_pack_id!r}.",
            )
        entries = raw_pack.get("modules")
        if not isinstance(entries, list) or not entries:
            raise ConceptProjectError("DEFAULT_PACK_INVALID", "Bundled Module Pack has no modules.")

        existing = {
            item.manifest.module_id: item
            for item in self.modules.list_modules(pack_id=expected_pack_id).items
        }
        for entry in entries:
            if not isinstance(entry, dict):
                raise ConceptProjectError("DEFAULT_PACK_INVALID", "Bundled Pack module entry is invalid.")
            manifest_path = _pack_file(root, str(entry.get("manifest_path", "")))
            glb_path = _pack_file(root, str(entry.get("glb_path", "")))
            thumbnail_path = _pack_file(root, str(entry.get("thumbnail_path", "")))
            try:
                manifest = ModuleAssetManifest.model_validate_json(manifest_path.read_text(encoding="utf-8"))
                glb_payload = glb_path.read_bytes()
                thumbnail_payload = thumbnail_path.read_bytes()
            except OSError as exc:
                raise ConceptProjectError(
                    "DEFAULT_PACK_UNAVAILABLE", f"Bundled Pack file is unavailable: {exc}"
                ) from exc
            registered = existing.get(manifest.module_id)
            if registered is None:
                self.modules.register_module(
                    RegisterModuleAssetRequest(
                        client_request_id=f"builtin-pack-{expected_pack_id}-{manifest.module_id}",
                        manifest=manifest,
                        logical_path=f"packs/{expected_pack_id}/{entry['glb_path']}",
                        glb_data_base64=base64.b64encode(glb_payload).decode("ascii"),
                        thumbnail_png_base64=base64.b64encode(thumbnail_payload).decode("ascii"),
                        catalog_metadata=_bundled_catalog_metadata(manifest, raw_pack),
                    ),
                    f"builtin-pack-{expected_pack_id}-{manifest.module_id}-{hashlib.sha256(glb_payload).hexdigest()[:16]}",
                )
                continue
            if (
                registered.manifest.pack_id != expected_pack_id
                or registered.manifest.sha256 != manifest.sha256
            ):
                raise ConceptProjectError(
                    "DEFAULT_PACK_CONFLICT",
                    f"Registered module {manifest.module_id} differs from the bundled Pack.",
                )
            self.modules.ensure_module_thumbnail(manifest.module_id, thumbnail_payload)


def _configured_pack_root() -> Path:
    configured = os.environ.get("FORGECAD_BUNDLED_MODULE_PACK")
    return Path(configured).expanduser().resolve() if configured else DEFAULT_PACK_ROOT


def _bundled_catalog_metadata(
    manifest: ModuleAssetManifest,
    raw_pack: dict[str, object],
) -> ModuleAssetCatalogMetadataInput:
    """Keep authorship and review state honest for the Pack actually loaded.

    A Blender authoring starter must not silently become an original-author
    declaration merely because it is bundled into a local workbench.
    """
    license_value = raw_pack.get("license")
    spdx = (
        str(license_value.get("spdx_expression", ""))
        if isinstance(license_value, dict)
        else ""
    )
    display_name = manifest.module_id.removeprefix("module_").replace("_", " ").title()
    if spdx == "LicenseRef-ForgeCAD-Original-Author":
        reviewer = os.environ.get("FORGECAD_ASSET_REVIEWER_NAME", "").strip() or "刘邦"
        return ModuleAssetCatalogMetadataInput(
            display_name=display_name,
            description="本人原创的非功能概念展示组件；可在 CAD 工作台中组合、预览与导出。",
            tags=["original", "visual", "non-functional"],
            catalog_path=manifest.category,
            origin_claim="self_declared_original",
            creator_name=os.environ.get("FORGECAD_ASSET_CREATOR_NAME", "").strip() or "刘崇江",
            review_status="pending_review",
            reviewer_name=reviewer,
            review_note=(
                f"已指派独立审阅人 {reviewer}；尚未完成实际审阅。"
                if reviewer
                else "已声明为本人原创，等待独立审阅。"
            ),
        )
    return ModuleAssetCatalogMetadataInput(
        display_name=display_name,
        description="待确认来源的本机技术候选组件；不能作为已批准的正式美术资产。",
        tags=["candidate", "visual", "non-functional"],
        catalog_path=manifest.category,
        origin_claim="unknown",
        creator_name="Unclassified Asset Author",
        review_status="draft",
        review_note="需先完成权属声明和独立审阅，才能以原创正式资产显示。",
    )


def _pack_file(root: Path, value: str) -> Path:
    candidate = (root / value).resolve()
    if not value or not candidate.is_relative_to(root) or not candidate.is_file():
        raise ConceptProjectError("DEFAULT_PACK_INVALID", f"Unsafe or missing bundled Pack file: {value!r}")
    return candidate


def _starter_graph(project_id: str) -> ModuleGraph:
    suffix = project_id.removeprefix("prj_")
    return ModuleGraph.model_validate(
        {
            "schema_version": "ModuleGraph@1",
            "graph_id": f"mg_starter_{suffix}",
            "project_id": project_id,
            "root_node_id": "node_core",
            "nodes": [
                _node("node_core", "module_core_shell_01", [0, 0, 0], locked=True),
                _node("node_front", "module_front_shell_01", [-50, 0, 0]),
                _node("node_rear", "module_rear_shell_01", [50, 0, 0]),
                _node("node_grip", "module_grip_shell_01", [14, -24, 0]),
                _node("node_top", "module_top_accessory_01", [0, 24, 0]),
                _node("node_side", "module_side_accessory_01", [0, 0, 20]),
                _node("node_lower", "module_lower_structure_01", [-12, -24, 0]),
                _node("node_storage", "module_storage_visual_01", [30, -24, 0]),
                _node("node_armor", "module_armor_panel_01", [0, 0, -20]),
            ],
            "edges": [
                _edge("front", "connector_core_front", "connector_front_01_core"),
                _edge("rear", "connector_core_rear", "connector_rear_core"),
                _edge("grip", "connector_core_grip", "connector_grip_core"),
                _edge("top", "connector_core_top", "connector_top_core"),
                _edge("side", "connector_core_side", "connector_side_core"),
                _edge("lower", "connector_core_lower", "connector_lower_core"),
                _edge("storage", "connector_core_storage", "connector_storage_core"),
                _edge("armor", "connector_core_armor", "connector_armor_core"),
            ],
        }
    )


def _node(node_id: str, module_id: str, position: list[float], *, locked: bool = False) -> dict[str, object]:
    return {
        "node_id": node_id,
        "module_id": module_id,
        "transform": {"position": position, "rotation": [0, 0, 0], "scale": [1, 1, 1]},
        "mirror_axis": "none",
        "locked": locked,
        "visible": True,
    }


def _edge(name: str, source_connector_id: str, target_connector_id: str) -> dict[str, str]:
    return {
        "edge_id": f"edge_core_{name}",
        "from_node_id": "node_core",
        "from_connector_id": source_connector_id,
        "to_node_id": f"node_{name}",
        "to_connector_id": target_connector_id,
        "status": "connected",
    }
