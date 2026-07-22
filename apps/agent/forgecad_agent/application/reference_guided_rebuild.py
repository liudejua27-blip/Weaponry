"""FGC-R007: deterministic, read-only reference evidence for constrained rebuilds.

This module deliberately does *not* perform image generation, reverse image
search, mesh copying, or mutable import.  It converts a user-authorized CAS
object plus small, explicit observations into bounded design evidence.  A
subsequent Rust-owned lifecycle layer may persist the evidence and turn the
plan into a normal preview -> confirm ChangeSet, but this Python boundary has
no database, object-store path, project write, or provider capability.

The first supported route is a robotic-arm concept MVP.  Its output selects
only existing C105 recipes, D005 semantic-proportion recipes and G819 runtime
operations.  Reference bytes never enter the proposed ShapeProgram.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import asdict
from typing import Any, Literal

from pydantic import Field, model_validator

from forgecad_agent.application.combined_glb import CombinedGlbError, read_glb
from forgecad_agent.application.concept_models import StrictApiModel
from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts
from forgecad_agent.application.semantic_proportions import recipes_for_domain, style_token_map


R007_SCHEMA_VERSION = "ReferenceEvidence@1"
R007_PLAN_SCHEMA_VERSION = "ReferenceGuidedRebuildPlan@1"
_SHA256 = r"^[a-f0-9]{64}$"
_CAS_OBJECT = r"^cas_[a-z0-9][a-z0-9_\-]{7,159}$"
_ID = r"^[a-z][a-z0-9_\-]{2,120}$"
_ALLOWED_DOMAIN = "pack_robotic_arm_concept"
_IMAGE_MAGIC: dict[str, bytes] = {
    "image/png": b"\x89PNG\r\n\x1a\n",
    "image/jpeg": b"\xff\xd8\xff",
    "image/webp": b"RIFF",
}


class ReferenceEvidenceError(ValueError):
    """Stable, value-free error for invalid or unsupported reference evidence."""


class ReferenceSourceDeclaration(StrictApiModel):
    """Metadata supplied by the user-facing Rust/CAS owner, never a file path."""

    source_id: str = Field(pattern=_ID)
    cas_object_id: str = Field(pattern=_CAS_OBJECT)
    source_kind: Literal["image", "glb"]
    source_statement: str = Field(min_length=3, max_length=500)
    license_statement: str = Field(min_length=3, max_length=500)
    authorization: Literal["user_authorized"] = "user_authorized"
    declared_view: Literal["front", "side", "rear", "top", "iso", "detail", "unknown"] = "unknown"
    # These are visual observations, not asserted mechanical facts.  Keeping
    # them bounded prevents the input from becoming an arbitrary instruction
    # channel or a hidden geometry program.
    visible_features: list[
        Literal[
            "base",
            "joint",
            "upper_link",
            "forearm_link",
            "end_effector",
            "panel_gap",
            "cable",
            "accent_light",
            "surface_pattern",
        ]
    ] = Field(default_factory=list, max_length=12)
    color_blocks: list[
        Literal["dark_metal", "light_metal", "blue_accent", "red_accent", "black_rubber", "neutral"]
    ] = Field(default_factory=list, max_length=6)

    @model_validator(mode="after")
    def _unique_observations(self) -> "ReferenceSourceDeclaration":
        if len(self.visible_features) != len(set(self.visible_features)):
            raise ValueError("visible features must be unique")
        if len(self.color_blocks) != len(set(self.color_blocks)):
            raise ValueError("color blocks must be unique")
        return self


class ReferenceProportionRange(StrictApiModel):
    axis: Literal["width", "height", "depth"]
    relative_min: float = Field(ge=0.02, le=1.0)
    relative_max: float = Field(ge=0.02, le=1.0)

    @model_validator(mode="after")
    def _ordered(self) -> "ReferenceProportionRange":
        if self.relative_min > self.relative_max:
            raise ValueError("relative range is reversed")
        return self


class ReferenceVisiblePartHypothesis(StrictApiModel):
    part_role: Literal[
        "base_form",
        "joint_housing",
        "upper_link_form",
        "forearm_link_form",
        "end_effector_form",
        "visual_detail",
    ]
    evidence: Literal["declared_visual_observation", "glb_node_name", "forgecad_readback"]
    confidence: Literal["low", "medium"]
    note: str = Field(min_length=3, max_length=240)


class ReferenceEvidence(StrictApiModel):
    schema_version: Literal["ReferenceEvidence@1"] = R007_SCHEMA_VERSION
    evidence_id: str = Field(pattern=r"^refev_[a-f0-9]{24}$")
    reference_sha256: str = Field(pattern=_SHA256)
    source: ReferenceSourceDeclaration
    source_byte_size: int = Field(gt=0, le=64 * 1024 * 1024)
    source_read_only: Literal[True] = True
    source_unchanged_sha256: str = Field(pattern=_SHA256)
    readback_kind: Literal["forgecad_glb_readback", "generic_glb_metadata", "image_declaration"]
    view_coverage: list[Literal["front", "side", "rear", "top", "iso", "detail"]] = Field(min_length=1, max_length=6)
    missing_views: list[Literal["front", "side", "rear", "top", "iso", "detail"]] = Field(min_length=1, max_length=6)
    silhouette_summary: str = Field(min_length=8, max_length=360)
    proportion_ranges: list[ReferenceProportionRange] = Field(min_length=1, max_length=3)
    color_blocks: list[str] = Field(min_length=1, max_length=12)
    visible_part_hypotheses: list[ReferenceVisiblePartHypothesis] = Field(min_length=1, max_length=12)
    uncertainties: list[str] = Field(min_length=1, max_length=12)
    facts: dict[str, Any] = Field(default_factory=dict, max_length=32)
    evidence_sha256: str = Field(pattern=_SHA256)


class ReferenceGuidedRebuildPlan(StrictApiModel):
    schema_version: Literal["ReferenceGuidedRebuildPlan@1"] = R007_PLAN_SCHEMA_VERSION
    plan_id: str = Field(pattern=r"^refplan_[a-f0-9]{24}$")
    evidence_id: str = Field(pattern=r"^refev_[a-f0-9]{24}$")
    reference_sha256: str = Field(pattern=_SHA256)
    domain_pack_id: Literal["pack_robotic_arm_concept"] = _ALLOWED_DOMAIN
    read_only_reference: Literal[True] = True
    rebuild_mode: Literal["new_constrained_candidate"] = "new_constrained_candidate"
    c105_recipe_ids: list[Literal["recipe_robotic_arm_link", "recipe_robotic_arm_detail"]] = Field(min_length=1, max_length=2)
    d005_semantic_proportion_recipe_id: Literal[
        "proportion_arm_compact", "proportion_arm_sleek", "proportion_arm_substantial", "proportion_arm_clean"
    ]
    style_token_id: Literal[
        "style_compact_rounded", "style_aerodynamic_sleek", "style_industrial_substantial", "style_clean_balanced"
    ]
    g819_operation_allowlist: list[Literal["sweep", "box"]] = Field(min_length=1, max_length=2)
    expected_visible_roles: list[str] = Field(min_length=1, max_length=8)
    reference_constraints: list[str] = Field(min_length=2, max_length=12)
    known_differences: list[str] = Field(min_length=1, max_length=12)
    unresolved_uncertainties: list[str] = Field(min_length=1, max_length=12)
    plan_sha256: str = Field(pattern=_SHA256)


def extract_reference_evidence(
    source: ReferenceSourceDeclaration | Mapping[str, Any],
    payload: bytes,
) -> ReferenceEvidence:
    """Normalize user-authorized reference bytes without persisting or changing them.

    GLB parsing is read-only.  ForgeCAD-produced GLBs additionally go through
    the existing strict GeometryCompileReadback parser; foreign GLBs are only
    represented as conservative metadata evidence, never executable geometry.
    Images intentionally use declaration-only evidence in this MVP: no vision
    model, OCR, reverse search, EXIF path, or hidden material inference.
    """

    declaration = ReferenceSourceDeclaration.model_validate(source)
    if not isinstance(payload, bytes) or not payload:
        raise ReferenceEvidenceError("REFERENCE_PAYLOAD_REQUIRED")
    if len(payload) > 64 * 1024 * 1024:
        raise ReferenceEvidenceError("REFERENCE_PAYLOAD_TOO_LARGE")
    digest = hashlib.sha256(payload).hexdigest()
    if declaration.source_kind == "image":
        body = _extract_image_evidence(declaration, payload, digest)
    else:
        body = _extract_glb_evidence(declaration, payload, digest)
    provisional = {
        "schema_version": R007_SCHEMA_VERSION,
        "evidence_id": f"refev_{digest[:24]}",
        "reference_sha256": digest,
        "source": declaration.model_dump(mode="json"),
        "source_byte_size": len(payload),
        "source_read_only": True,
        "source_unchanged_sha256": digest,
        **body,
    }
    provisional["evidence_sha256"] = _canonical_sha256(provisional)
    return ReferenceEvidence.model_validate(provisional)


def build_robotic_arm_rebuild_plan(evidence: ReferenceEvidence | Mapping[str, Any]) -> ReferenceGuidedRebuildPlan:
    """Map evidence to existing C105/D005/G819 contracts, never copied GLB data."""

    normalized = ReferenceEvidence.model_validate(evidence)
    features = {item.part_role for item in normalized.visible_part_hypotheses}
    color_blocks = set(normalized.color_blocks)
    detail_needed = bool({"visual_detail", "joint_housing"} & features) or bool(
        {"blue_accent", "red_accent", "surface_pattern"} & (set(normalized.source.visible_features) | color_blocks)
    )
    proportion_recipe_id = _select_arm_proportion_recipe(normalized)
    semantic_recipe = next(
        recipe for recipe in recipes_for_domain(_ALLOWED_DOMAIN) if recipe.recipe_id == proportion_recipe_id
    )
    style_token = style_token_map()[semantic_recipe.style_token_id]
    recipe_ids: list[str] = ["recipe_robotic_arm_link"]
    if detail_needed:
        recipe_ids.append("recipe_robotic_arm_detail")
    expected_roles = sorted(features or {"upper_link_form"})
    constraints = [
        "仅重建可见轮廓、相对比例、色块与非功能外观语言。",
        "新候选必须只使用 G819 已声明的 sweep/box 与 C105 机械臂 Recipe。",
        "参考 GLB/图片保持只读；其字节和网格不会进入新 ShapeProgram。",
    ]
    differences = [
        "新资产使用 ForgeCAD 受限组件配方，不是参考网格的副本或编辑结果。",
        "比例是归一化视觉范围，不包含尺寸、公差、载荷、材料性能或功能结论。",
    ]
    base = {
        "schema_version": R007_PLAN_SCHEMA_VERSION,
        "plan_id": f"refplan_{_canonical_sha256({'reference': normalized.reference_sha256, 'evidence': normalized.evidence_sha256})[:24]}",
        "evidence_id": normalized.evidence_id,
        "reference_sha256": normalized.reference_sha256,
        "domain_pack_id": _ALLOWED_DOMAIN,
        "read_only_reference": True,
        "rebuild_mode": "new_constrained_candidate",
        "c105_recipe_ids": recipe_ids,
        "d005_semantic_proportion_recipe_id": semantic_recipe.recipe_id,
        "style_token_id": style_token.token_id,
        "g819_operation_allowlist": ["sweep", "box"],
        "expected_visible_roles": expected_roles,
        "reference_constraints": constraints,
        "known_differences": differences,
        "unresolved_uncertainties": normalized.uncertainties,
    }
    base["plan_sha256"] = _canonical_sha256(base)
    return ReferenceGuidedRebuildPlan.model_validate(base)


def _extract_image_evidence(source: ReferenceSourceDeclaration, payload: bytes, digest: str) -> dict[str, Any]:
    _assert_image_magic(payload)
    view = _effective_view(source.declared_view)
    features = _hypotheses_from_features(source.visible_features, evidence="declared_visual_observation")
    if not features:
        features = [
            ReferenceVisiblePartHypothesis(
                part_role="upper_link_form",
                evidence="declared_visual_observation",
                confidence="low",
                note="图片仅被声明为机械臂外观参考；未从单图推断隐藏部件。",
            )
        ]
    return {
        "readback_kind": "image_declaration",
        "view_coverage": [view],
        "missing_views": _missing_views([view]),
        "silhouette_summary": "用户声明的单张机械臂外观参考；仅提取可见轮廓和色块，不恢复隐藏结构。",
        "proportion_ranges": _declaration_proportion_ranges(source.declared_view),
        "color_blocks": sorted(set(source.color_blocks) or {"neutral"}),
        "visible_part_hypotheses": [item.model_dump(mode="json") for item in features],
        "uncertainties": _image_uncertainties(source.declared_view),
        "facts": {
            "content_type": _image_content_type(payload),
            "reference_hash": digest,
            "analysis": "declaration_only_no_reverse_search_no_hidden_geometry",
        },
    }


def _extract_glb_evidence(source: ReferenceSourceDeclaration, payload: bytes, digest: str) -> dict[str, Any]:
    try:
        document, _ = read_glb(payload)
    except CombinedGlbError as exc:
        raise ReferenceEvidenceError("REFERENCE_GLB_INVALID") from exc
    if document.get("asset", {}).get("version") != "2.0":
        raise ReferenceEvidenceError("REFERENCE_GLB_VERSION_INVALID")
    try:
        readback = read_shape_program_glb_facts(payload)
    except (ValueError, KeyError, TypeError):
        return _generic_glb_metadata_evidence(source, document, digest)
    roles = sorted(
        {
            str(item["part_role"] if isinstance(item, Mapping) else item.part_role)
            for item in readback.surface_provenance
        }
    )
    hypotheses = [
        ReferenceVisiblePartHypothesis(
            part_role=_safe_arm_role(role),
            evidence="forgecad_readback",
            confidence="medium",
            note=f"GLB readback reports visual role: {role}。",
        )
        for role in roles
    ] or _hypotheses_from_features(source.visible_features, evidence="declared_visual_observation")
    width, height, depth = (max(float(value), 0.001) for value in readback.bounds_mm)
    return {
        "readback_kind": "forgecad_glb_readback",
        "view_coverage": ["front", "side", "rear", "top", "iso"],
        "missing_views": ["detail"],
        "silhouette_summary": "只读 GLB 的真实 readback 表明其包含机械臂可见外观部件；不把网格当作可复制的重建输入。",
        "proportion_ranges": _ranges_from_bounds(width, height, depth),
        "color_blocks": _color_blocks_from_readback(readback) or sorted(set(source.color_blocks) or {"neutral"}),
        "visible_part_hypotheses": [item.model_dump(mode="json") for item in hypotheses],
        "uncertainties": [
            "GLB readback 只说明可见网格/材质区，不说明内部结构、尺寸、公差或功能。",
            "未提供近景细节图；接缝、雕刻和局部纹样仅能作为后续可选外观设计。",
        ],
        "facts": {
            "content_type": "model/gltf-binary",
            "reference_hash": digest,
            "triangle_count": readback.triangle_count,
            "bounds_relative_source": "GeometryCompileReadback@2",
            "material_zone_count": len(readback.material_zone_faces),
                "readback_sha256": _canonical_sha256(asdict(readback)),
        },
    }


def _generic_glb_metadata_evidence(source: ReferenceSourceDeclaration, document: Mapping[str, Any], digest: str) -> dict[str, Any]:
    names = _bounded_node_names(document)
    hypotheses = _hypotheses_from_features(source.visible_features, evidence="declared_visual_observation")
    for name in names:
        role = _role_from_name(name)
        if role and role not in {item.part_role for item in hypotheses}:
            hypotheses.append(
                ReferenceVisiblePartHypothesis(
                    part_role=role,
                    evidence="glb_node_name",
                    confidence="low",
                    note="GLB 节点名称只作为可见部件提示，不代表真实机构或尺寸。",
                )
            )
    if not hypotheses:
        hypotheses.append(
            ReferenceVisiblePartHypothesis(
                part_role="upper_link_form",
                evidence="glb_node_name",
                confidence="low",
                note="外部 GLB 未提供 ForgeCAD readback；只保留保守轮廓提示。",
            )
        )
    material_names = _bounded_material_names(document)
    return {
        "readback_kind": "generic_glb_metadata",
        "view_coverage": ["iso"],
        "missing_views": _missing_views(["iso"]),
        "silhouette_summary": "外部只读 GLB 的节点/材质元数据仅用于可见轮廓提示；未执行网格复制或工程解析。",
        "proportion_ranges": _declaration_proportion_ranges("iso"),
        "color_blocks": sorted(set(source.color_blocks) or {"neutral"}),
        "visible_part_hypotheses": [item.model_dump(mode="json") for item in hypotheses],
        "uncertainties": [
            "该外部 GLB 未满足 ForgeCAD readback 合同，因此不使用其网格或拓扑作为重建输入。",
            "没有确认的正/侧/顶视图，比例仅为概念范围。",
        ],
        "facts": {
            "content_type": "model/gltf-binary",
            "reference_hash": digest,
            "mesh_count": len(document.get("meshes", [])) if isinstance(document.get("meshes"), list) else 0,
            "node_names": names,
            "material_names": material_names,
            "analysis": "metadata_only_no_mesh_copy",
        },
    }


def _select_arm_proportion_recipe(evidence: ReferenceEvidence) -> str:
    # The choice is explainable and bounded.  No ML score or visual claim is
    # made: an elongated relative height selects the existing sleek D005 option;
    # a strong base observation selects substantial; otherwise retain compact.
    ratios = {item.axis: item.relative_max for item in evidence.proportion_ranges}
    if "base_form" in {item.part_role for item in evidence.visible_part_hypotheses} and "black_rubber" in evidence.color_blocks:
        return "proportion_arm_substantial"
    if ratios.get("height", 0.0) >= 0.82:
        return "proportion_arm_sleek"
    if "end_effector_form" in {item.part_role for item in evidence.visible_part_hypotheses} and "detail" in evidence.view_coverage:
        return "proportion_arm_clean"
    return "proportion_arm_compact"


def _ranges_from_bounds(width: float, height: float, depth: float) -> list[dict[str, float | str]]:
    largest = max(width, height, depth)
    return [
        {"axis": axis, "relative_min": round(value / largest * 0.92, 4), "relative_max": round(value / largest, 4)}
        for axis, value in (("width", width), ("height", height), ("depth", depth))
    ]


def _declaration_proportion_ranges(view: str) -> list[dict[str, float | str]]:
    # A declaration must not fabricate dimensions.  The range expresses only
    # that the visible silhouette is normalized against its own largest span.
    if view in {"front", "rear"}:
        return [{"axis": "width", "relative_min": 0.45, "relative_max": 1.0}, {"axis": "height", "relative_min": 0.45, "relative_max": 1.0}]
    if view == "top":
        return [{"axis": "width", "relative_min": 0.45, "relative_max": 1.0}, {"axis": "depth", "relative_min": 0.45, "relative_max": 1.0}]
    return [{"axis": "width", "relative_min": 0.35, "relative_max": 1.0}, {"axis": "height", "relative_min": 0.35, "relative_max": 1.0}, {"axis": "depth", "relative_min": 0.2, "relative_max": 0.8}]


def _hypotheses_from_features(features: Sequence[str], *, evidence: str) -> list[ReferenceVisiblePartHypothesis]:
    mapping = {
        "base": "base_form", "joint": "joint_housing", "upper_link": "upper_link_form",
        "forearm_link": "forearm_link_form", "end_effector": "end_effector_form",
        "panel_gap": "visual_detail", "cable": "visual_detail", "accent_light": "visual_detail", "surface_pattern": "visual_detail",
    }
    seen: set[str] = set()
    result: list[ReferenceVisiblePartHypothesis] = []
    for feature in features:
        role = mapping.get(feature)
        if role and role not in seen:
            seen.add(role)
            result.append(ReferenceVisiblePartHypothesis(part_role=role, evidence=evidence, confidence="low", note="用户声明的可见外观特征；不表示内部机构或功能。"))
    return result


def _safe_arm_role(value: str) -> str:
    lowered = value.lower()
    if "base" in lowered:
        return "base_form"
    if "joint" in lowered or "collar" in lowered:
        return "joint_housing"
    if "forearm" in lowered or "lower" in lowered:
        return "forearm_link_form"
    if "tool" in lowered or "grip" in lowered or "end" in lowered:
        return "end_effector_form"
    if "detail" in lowered or "trim" in lowered or "decal" in lowered:
        return "visual_detail"
    return "upper_link_form"


def _role_from_name(value: str) -> str | None:
    lowered = value.lower()
    if any(word in lowered for word in ("base", "pedestal")):
        return "base_form"
    if any(word in lowered for word in ("joint", "elbow", "shoulder")):
        return "joint_housing"
    if any(word in lowered for word in ("forearm", "lower_link")):
        return "forearm_link_form"
    if any(word in lowered for word in ("gripper", "tool", "end_effector")):
        return "end_effector_form"
    if any(word in lowered for word in ("detail", "trim", "decal", "cable")):
        return "visual_detail"
    if any(word in lowered for word in ("link", "arm")):
        return "upper_link_form"
    return None


def _color_blocks_from_readback(readback: Any) -> list[str]:
    material_ids = {
        str(item["material_id"] if isinstance(item, Mapping) else item.material_id)
        for item in readback.visual_texture_sets
    }
    result: set[str] = set()
    if any("signal" in item or "red" in item for item in material_ids):
        result.add("red_accent")
    if any("blue" in item for item in material_ids):
        result.add("blue_accent")
    if any("rubber" in item for item in material_ids):
        result.add("black_rubber")
    if any("aluminum" in item or "metal" in item or "graphite" in item for item in material_ids):
        result.add("dark_metal")
    return sorted(result)


def _image_uncertainties(view: str) -> list[str]:
    output = [
        "单张图片不恢复隐藏结构、精确尺寸、材料配方、功能或性能。",
        "色块来自用户声明；当前 MVP 不进行反向搜索、OCR 或自动版权判断。",
    ]
    if view in {"front", "rear", "side", "top", "unknown"}:
        output.append("缺少多视图，深度比例、遮挡部件和背面外观保持不确定。")
    return output


def _effective_view(view: str) -> Literal["front", "side", "rear", "top", "iso", "detail"]:
    return "iso" if view == "unknown" else view  # type: ignore[return-value]


def _missing_views(covered: Sequence[str]) -> list[str]:
    return [view for view in ("front", "side", "rear", "top", "iso", "detail") if view not in covered]


def _assert_image_magic(payload: bytes) -> None:
    content_type = _image_content_type(payload)
    if content_type is None:
        raise ReferenceEvidenceError("REFERENCE_IMAGE_FORMAT_INVALID")


def _image_content_type(payload: bytes) -> str | None:
    if payload.startswith(_IMAGE_MAGIC["image/png"]):
        return "image/png"
    if payload.startswith(_IMAGE_MAGIC["image/jpeg"]):
        return "image/jpeg"
    if payload.startswith(_IMAGE_MAGIC["image/webp"]) and len(payload) >= 12 and payload[8:12] == b"WEBP":
        return "image/webp"
    return None


def _bounded_node_names(document: Mapping[str, Any]) -> list[str]:
    nodes = document.get("nodes")
    if not isinstance(nodes, list):
        return []
    return [str(node["name"])[:80] for node in nodes if isinstance(node, Mapping) and isinstance(node.get("name"), str)][:32]


def _bounded_material_names(document: Mapping[str, Any]) -> list[str]:
    materials = document.get("materials")
    if not isinstance(materials, list):
        return []
    return [str(item["name"])[:80] for item in materials if isinstance(item, Mapping) and isinstance(item.get("name"), str)][:32]


def _canonical_sha256(value: Mapping[str, Any]) -> str:
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
