from __future__ import annotations

from math import isfinite
import re
from typing import Annotated, Any, Dict, List, Literal, Optional, Union

from pydantic import Field, model_validator

from .concept_models import StrictApiModel
from .domain_packs import DomainPackId
from .geometry_models import GeometryCompileReadback
from .mechanical_planner import MechanicalConceptPlan
from .provider_gateway import ProviderConnectionState, ProviderExecutionTrace
from .shape_program import AgentAssetGeometryPayload, ShapeProgramPayload


AgentThreadStatus = Literal["idle", "active", "error", "archived"]
AgentTurnStatus = Literal[
    "queued",
    "running",
    "waiting_for_approval",
    "waiting_for_clarification",
    "completed",
    "failed",
    "cancelled",
]
AgentItemType = Literal[
    "user_message",
    "assistant_message",
    "plan",
    "tool_call",
    "tool_result",
    "preview",
    "approval_request",
    "clarification",
    "artifact",
]
AgentItemStatus = Literal["pending", "completed", "failed", "cancelled"]
ApprovalStatus = Literal["pending", "approved", "rejected"]
BlockoutPresentationProfile = Literal["quick_sketch", "showcase"]


class CreateAgentThreadRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    project_id: Optional[str] = Field(default=None, max_length=160)
    title: str = Field(default="新建设计会话", min_length=1, max_length=160)
    provider_id: str = Field(default="deterministic_kernel", min_length=1, max_length=120)


class StartAgentTurnRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    message: str = Field(min_length=1, max_length=8000)
    clarification_domain_pack_id: Optional[DomainPackId] = None


class ResolveAgentApprovalRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    decision: Literal["approved", "rejected"]
    note: str = Field(default="", max_length=1000)


class CreateAgentApprovalRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    turn_id: str = Field(min_length=1, max_length=160)
    action: str = Field(min_length=1, max_length=120)
    payload: Dict[str, Any] = Field(default_factory=dict)


class AgentItem(StrictApiModel):
    item_id: str
    thread_id: str
    turn_id: str
    sequence: int
    item_type: AgentItemType
    status: AgentItemStatus
    payload: Dict[str, Any] = Field(default_factory=dict)
    created_at: str


class AgentApproval(StrictApiModel):
    approval_id: str
    thread_id: str
    turn_id: str
    item_id: str
    action: str
    status: ApprovalStatus
    payload: Dict[str, Any] = Field(default_factory=dict)
    created_at: str
    resolved_at: Optional[str] = None


class AgentTurn(StrictApiModel):
    turn_id: str
    thread_id: str
    request_text: str
    status: AgentTurnStatus
    error_code: Optional[str] = None
    error_message: Optional[str] = None
    usage: Dict[str, Any] = Field(default_factory=dict)
    created_at: str
    updated_at: str
    items: List[AgentItem] = Field(default_factory=list)
    approvals: List[AgentApproval] = Field(default_factory=list)


class AgentThreadSummary(StrictApiModel):
    thread_id: str
    project_id: Optional[str] = None
    title: str
    status: AgentThreadStatus
    summary: str
    provider_id: str
    created_at: str
    updated_at: str
    last_turn_id: Optional[str] = None


class AgentThreadDetail(AgentThreadSummary):
    turns: List[AgentTurn] = Field(default_factory=list)


class AgentThreadListResponse(StrictApiModel):
    items: List[AgentThreadSummary] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class AgentEvent(StrictApiModel):
    sequence: int
    thread_id: str
    turn_id: str
    item: AgentItem


class AgentApprovalResolution(StrictApiModel):
    approval: AgentApproval
    turn: AgentTurn


class BuildAgentBlockoutRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    plan: MechanicalConceptPlan
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    variant_id: Optional[str] = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(default=0, ge=0, le=2)
    presentation_profile: BlockoutPresentationProfile = "quick_sketch"


class BuildAgentBlockoutResponse(StrictApiModel):
    artifact_id: str = Field(pattern=r"^artifact_[a-z0-9_\-]+$")
    plan_id: str
    direction_id: str
    variant_id: Optional[str] = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(default=0, ge=0, le=2)
    presentation_profile: BlockoutPresentationProfile = "quick_sketch"
    domain_pack_id: str
    triangle_count: int
    bounds_mm: List[float]
    topology_hash: str = Field(pattern=r"^[a-f0-9]{64}$")
    assembly_graph: Dict[str, Any]
    shape_program: ShapeProgramPayload
    glb_base64: str = Field(min_length=1)


class RenderAgentBlockoutConceptPreviewRequest(StrictApiModel):
    """Request an ephemeral software image for a bounded, uncommitted direction.

    This deliberately has no project write, asset id, Snapshot id, or export
    option.  The server rebuilds the bounded ShapeProgram locally instead of
    trusting a client-provided mesh or image.
    """

    client_request_id: str = Field(min_length=1, max_length=120)
    plan: MechanicalConceptPlan
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    variant_id: Optional[str] = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(default=0, ge=0, le=2)
    presentation_profile: BlockoutPresentationProfile = "quick_sketch"


class AgentBlockoutConceptPreview(StrictApiModel):
    """A disposable low-resolution software concept image, never a render asset."""

    schema_version: Literal["AgentBlockoutConceptPreview@1"] = "AgentBlockoutConceptPreview@1"
    plan_id: str = Field(min_length=1, max_length=160)
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    variant_id: str = Field(pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(ge=0, le=2)
    domain_pack_id: DomainPackId
    topology_hash: str = Field(pattern=r"^[a-f0-9]{64}$")
    render_context_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    renderer_id: Literal["forgecad-agent-software-raster@1"] = "forgecad-agent-software-raster@1"
    width: Literal[320] = 320
    height: Literal[240] = 240
    png_base64: str = Field(min_length=1)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    byte_size: int = Field(gt=0)


class SegmentAgentBlockoutRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    plan: MechanicalConceptPlan
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    variant_id: Optional[str] = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(default=0, ge=0, le=2)
    presentation_profile: BlockoutPresentationProfile = "quick_sketch"
    artifact_id: Optional[str] = Field(default=None, pattern=r"^artifact_[a-z0-9_\-]+$")


EditableParameterPath = Literal[
    "transform.position.x",
    "transform.position.y",
    "transform.position.z",
    "transform.scale.x",
    "transform.scale.y",
    "transform.scale.z",
]
EditableParameterUnit = Literal["millimeter", "ratio"]


class EditableParameterBinding(StrictApiModel):
    """A bounded declaration for a future zero-basis parameter control.

    The declaration is data only. It neither executes ShapeProgram nor broadens
    the currently accepted ChangeSet operation paths.
    """

    schema_version: Literal["EditableParameterBinding@1"]
    parameter_id: str = Field(pattern=r"^editparam_[a-z0-9_\-]+$")
    path: EditableParameterPath
    display_name: str = Field(min_length=1, max_length=60)
    unit: EditableParameterUnit
    default: float
    min: float
    max: float
    step: float = Field(gt=0)

    @model_validator(mode="after")
    def validate_bounds_and_unit(self) -> "EditableParameterBinding":
        values = (self.default, self.min, self.max, self.step)
        if any(not isfinite(value) for value in values):
            raise ValueError("editable parameter values must be finite")
        if self.min >= self.max:
            raise ValueError("editable parameter min must be less than max")
        if not self.min <= self.default <= self.max:
            raise ValueError("editable parameter default must be within min and max")
        if self.step > self.max - self.min:
            raise ValueError("editable parameter step must fit within its range")
        if self.path.startswith("transform.scale."):
            if self.unit != "ratio":
                raise ValueError("scale parameters must use ratio")
            if self.min < 0.1 or self.max > 10:
                raise ValueError("scale parameter range must stay within 0.1 to 10")
        else:
            if self.unit != "millimeter":
                raise ValueError("position parameters must use millimeter")
            if self.min < -100000 or self.max > 100000:
                raise ValueError("position parameter range exceeds the concept boundary")
        return self


class BlockoutPartCandidate(StrictApiModel):
    part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    role: str = Field(min_length=1, max_length=120)
    parent_part_id: Optional[str] = None
    position_mm: List[float] = Field(min_length=3, max_length=3)
    size_mm: List[float] = Field(min_length=3, max_length=3)
    material_zone_ids: List[str] = Field(min_length=1, max_length=8)
    editable_parameters: List[str] = Field(default_factory=list, max_length=16)
    editable_parameter_bindings: List[EditableParameterBinding] = Field(default_factory=list, max_length=6)
    locked: bool = False
    provenance: Literal["agent_generated", "agent_component", "imported_glb"] = "agent_generated"

    @model_validator(mode="after")
    def validate_editable_parameter_bindings(self) -> "BlockoutPartCandidate":
        parameter_ids = [item.parameter_id for item in self.editable_parameter_bindings]
        paths = [item.path for item in self.editable_parameter_bindings]
        if len(parameter_ids) != len(set(parameter_ids)):
            raise ValueError("editable parameter binding ids must be unique per part")
        if len(paths) != len(set(paths)):
            raise ValueError("editable parameter binding paths must be unique per part")
        return self


class SegmentAgentBlockoutResponse(StrictApiModel):
    artifact_id: str = Field(pattern=r"^artifact_[a-z0-9_\-]+$")
    plan_id: str
    direction_id: str
    variant_id: Optional[str] = Field(default=None, pattern=r"^[a-z][a-z0-9_\-]{1,119}$")
    variation_index: int = Field(default=0, ge=0, le=2)
    presentation_profile: BlockoutPresentationProfile = "quick_sketch"
    domain_pack_id: str
    segmentation_status: Literal["candidate"] = "candidate"
    parts: List[BlockoutPartCandidate] = Field(min_length=1)
    assembly_graph: Dict[str, Any]


MaterialCategory = Literal[
    "metal",
    "polymer",
    "rubber",
    "composite",
    "glass",
    "coating",
    "natural",
    "emissive",
]


MaterialSource = Literal["forgecad_builtin", "user_created", "imported_reference"]
MaterialLicense = Literal["not_applicable", "self_declared_original", "third_party", "unknown"]
MaterialTextureRole = Literal[
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
    "thumbnail",
]
VisualTextureColorSpace = Literal["srgb", "linear"]
VisualTextureFallback = Literal["none", "parameter", "unavailable"]


class AgentMaterialTextureSummary(StrictApiModel):
    """Safe catalog provenance; never exposes a filesystem path."""

    texture_asset_id: str = Field(pattern=r"^asset_tex_[a-f0-9]{24}$")
    texture_role: MaterialTextureRole
    exists: bool
    source: Optional[MaterialSource] = None
    license: Optional[MaterialLicense] = None
    license_ref: Optional[str] = Field(default=None, min_length=1, max_length=240)


class AgentMaterialTextureObject(StrictApiModel):
    schema_version: Literal["MaterialTextureObject@1"] = "MaterialTextureObject@1"
    texture_asset_id: str = Field(pattern=r"^asset_tex_[a-f0-9]{24}$")
    texture_role: MaterialTextureRole
    display_name: str = Field(min_length=1, max_length=120)
    mime_type: Literal["image/png", "image/jpeg", "image/webp"]
    byte_size: int = Field(gt=0, le=4_000_000)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    object_path: str = Field(min_length=1, max_length=500)
    width: int = Field(gt=0, le=4096)
    height: int = Field(gt=0, le=4096)
    source: MaterialSource
    license: MaterialLicense
    license_ref: Optional[str] = Field(default=None, min_length=1, max_length=240)
    thumbnail_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_tex_[a-f0-9]{24}$")
    visual_only: Literal[True] = True
    object_exists: bool
    created_at: str
    updated_at: str

    @model_validator(mode="after")
    def validate_provenance_and_path(self) -> "AgentMaterialTextureObject":
        _validate_texture_provenance(self.source, self.license, self.license_ref)
        _validate_relative_object_path(self.object_path)
        return self


class RegisterAgentMaterialTextureRequest(StrictApiModel):
    display_name: str = Field(min_length=1, max_length=120)
    texture_role: MaterialTextureRole
    mime_type: Literal["image/png", "image/jpeg", "image/webp"]
    payload_base64: str = Field(min_length=1, max_length=5_600_000)
    source: MaterialSource
    license: MaterialLicense
    license_ref: Optional[str] = Field(default=None, min_length=1, max_length=240)
    thumbnail_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_tex_[a-f0-9]{24}$")

    @model_validator(mode="after")
    def validate_provenance(self) -> "RegisterAgentMaterialTextureRequest":
        _validate_texture_provenance(self.source, self.license, self.license_ref)
        if self.payload_base64.startswith("data:") or "://" in self.payload_base64:
            raise ValueError("payload_base64 must be raw base64, not a URL or data URI")
        return self


class AgentMaterialTextureListResponse(StrictApiModel):
    items: List[AgentMaterialTextureObject] = Field(default_factory=list)


class VisualTextureMap(StrictApiModel):
    """One content-addressed visual-only PBR texture channel.

    The map describes bytes embedded in a ForgeCAD GLB (or a separately
    registered bounded texture object).  It intentionally contains no URL,
    filesystem path, or engineering material claim.
    """

    texture_id: str = Field(pattern=r"^vtex_[a-z0-9_\-]+$")
    texture_role: Literal["base_color", "metallic_roughness", "normal", "occlusion", "emissive"]
    mime_type: Literal["image/png", "image/jpeg", "image/webp"]
    byte_size: int = Field(gt=0, le=4_000_000)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    color_space: VisualTextureColorSpace
    width: int = Field(gt=0, le=4096)
    height: int = Field(gt=0, le=4096)
    source: MaterialSource
    license: MaterialLicense
    license_ref: Optional[str] = Field(default=None, min_length=1, max_length=240)
    fallback: VisualTextureFallback = "none"
    visual_only: Literal[True] = True

    @model_validator(mode="after")
    def validate_provenance(self) -> "VisualTextureMap":
        _validate_texture_provenance(self.source, self.license, self.license_ref)
        if self.texture_role in {"base_color", "emissive"} and self.color_space != "srgb":
            raise ValueError("base-color and emissive textures must use sRGB")
        if self.texture_role in {"metallic_roughness", "normal", "occlusion"} and self.color_space != "linear":
            raise ValueError("non-colour PBR textures must use linear colour space")
        return self


class VisualTextureSet(StrictApiModel):
    """The one versioned PBR texture contract consumed by GLB/readback."""

    schema_version: Literal["VisualTextureSet@1"] = "VisualTextureSet@1"
    visual_texture_set_id: str = Field(pattern=r"^vtexset_[a-z0-9_\-]+$")
    material_id: str = Field(pattern=r"^mat_[a-z0-9_\-]+$")
    display_name: str = Field(min_length=1, max_length=120)
    maps: List[VisualTextureMap] = Field(min_length=5, max_length=5)
    source: MaterialSource
    license: MaterialLicense
    license_ref: Optional[str] = Field(default=None, min_length=1, max_length=240)
    version: str = Field(pattern=r"^[0-9]+(?:\.[0-9]+){0,2}$")
    visual_only: Literal[True] = True

    @model_validator(mode="after")
    def validate_complete_pbr_map_set(self) -> "VisualTextureSet":
        _validate_texture_provenance(self.source, self.license, self.license_ref)
        roles = [item.texture_role for item in self.maps]
        required = {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}
        if len(roles) != len(set(roles)) or set(roles) != required:
            raise ValueError("visual texture set must contain every supported PBR map exactly once")
        if any(item.source != self.source or item.license != self.license for item in self.maps):
            raise ValueError("visual texture map provenance must match its set")
        return self


def _validate_texture_provenance(
    source: MaterialSource,
    license: MaterialLicense,
    license_ref: Optional[str],
) -> None:
    allowed = {
        "forgecad_builtin": {"not_applicable"},
        "user_created": {"self_declared_original", "unknown"},
        "imported_reference": {"third_party", "unknown"},
    }
    if license not in allowed[source]:
        raise ValueError("source and license combination is not allowed")
    if license == "third_party" and not license_ref:
        raise ValueError("third_party textures require a license_ref")


def _validate_relative_object_path(value: str) -> None:
    if value.startswith("/") or value.startswith("\\") or "://" in value:
        raise ValueError("object_path must be relative and internal")
    if len(value) >= 2 and value[1] == ":":
        raise ValueError("object_path must not contain a drive prefix")
    if "\\" in value or ".." in value.split("/"):
        raise ValueError("object_path must be relative and internal")


class AgentMaterialPbr(StrictApiModel):
    """Appearance-only PBR values; texture ids are content-addressed asset ids."""

    base_color: str = Field(pattern=r"^#[0-9A-Fa-f]{6}$")
    metallic: float = Field(ge=0, le=1)
    roughness: float = Field(ge=0, le=1)
    opacity: float = Field(gt=0, le=1)
    base_color_texture_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_[a-z0-9_\-]+$")
    metallic_roughness_texture_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_[a-z0-9_\-]+$")
    normal_texture_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_[a-z0-9_\-]+$")
    occlusion_texture_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_[a-z0-9_\-]+$")
    emissive_texture_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_[a-z0-9_\-]+$")
    normal_strength: float = Field(default=1, ge=0, le=2)
    emissive_color: str = Field(default="#000000", pattern=r"^#[0-9A-Fa-f]{6}$")
    emissive_strength: float = Field(default=0, ge=0, le=100)
    transmission: float = Field(default=0, ge=0, le=1)
    ior: float = Field(default=1.5, ge=1, le=3)
    clearcoat: float = Field(default=0, ge=0, le=1)
    clearcoat_roughness: float = Field(default=0.5, ge=0, le=1)
    texture_scale: List[float] = Field(default_factory=lambda: [1, 1], min_length=2, max_length=2)

    @model_validator(mode="after")
    def validate_texture_scale(self) -> "AgentMaterialPbr":
        if any(not isfinite(value) or value <= 0 or value > 100 for value in self.texture_scale):
            raise ValueError("texture_scale must be greater than 0 and at most 100")
        return self


class AgentMaterialPreset(StrictApiModel):
    schema_version: Literal["MaterialPreset@1"] = "MaterialPreset@1"
    material_id: str = Field(pattern=r"^mat_[a-z0-9_\-]+$")
    display_name: str = Field(min_length=1, max_length=80)
    category: MaterialCategory
    pbr: AgentMaterialPbr
    visual_only: Literal[True] = True
    allowed_domains: List[str] = Field(min_length=1)
    provenance: MaterialSource
    visual_tags: List[str] = Field(default_factory=list, max_length=12)
    source: Optional[MaterialSource] = None
    license: Optional[MaterialLicense] = None
    version: Optional[str] = Field(default=None, pattern=r"^[0-9]+(?:\.[0-9]+){0,2}$")
    thumbnail_asset_id: Optional[str] = Field(default=None, pattern=r"^asset_tex_[a-f0-9]{24}$")
    thumbnail_fallback: Literal["parameter", "texture", "unavailable"] = "parameter"
    texture_summary: List[AgentMaterialTextureSummary] = Field(default_factory=list, max_length=8)

    @model_validator(mode="after")
    def migrate_legacy_metadata(self) -> "AgentMaterialPreset":
        """Fill M101 metadata for old MaterialPreset@1 payloads without changing IDs."""
        if self.source is None:
            self.source = self.provenance
        if self.license is None:
            self.license = "not_applicable" if self.provenance == "forgecad_builtin" else "unknown"
        if self.version is None:
            self.version = "1"
        if not self.visual_tags:
            self.visual_tags = [self.category]
        return self


class CommitAgentBlockoutRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    artifact_id: str = Field(pattern=r"^artifact_[a-z0-9_\-]+$")
    project_id: Optional[str] = Field(default=None, min_length=1, max_length=160)
    summary: str = Field(default="确认分件候选并保存为可编辑资产", min_length=1, max_length=500)


class AgentAssetVersion(StrictApiModel):
    schema_version: Literal["AgentAssetVersion@1"] = "AgentAssetVersion@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    project_id: str = Field(min_length=1, max_length=160)
    parent_asset_version_id: Optional[str] = Field(default=None, pattern=r"^assetver_[a-z0-9_\-]+$")
    version_no: int = Field(ge=1)
    status: Literal["committed", "superseded"]
    summary: str = Field(min_length=1, max_length=500)
    stage: Literal["segmented_concept", "editable_asset"]
    plan_id: str
    direction_id: str
    domain_pack_id: str
    artifact_id: str = Field(pattern=r"^artifact_[a-z0-9_\-]+$")
    parts: List[BlockoutPartCandidate] = Field(min_length=1)
    shape_program: AgentAssetGeometryPayload
    assembly_graph: Dict[str, Any]
    material_bindings: Dict[str, str] = Field(default_factory=dict)
    created_at: str


AgentAssetEditOp = Literal[
    "set_part_transform",
    "set_part_parameter",
    "set_joint_pose",
    "apply_material_preset",
    "replace_part",
    "snap_part_to_connector",
    "split_part",
    "merge_parts",
]


class AgentPartEditOperation(StrictApiModel):
    operation_id: str = Field(pattern=r"^op_[a-z0-9_\-]+$")
    op: AgentAssetEditOp
    part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    path: Optional[str] = Field(default=None, max_length=120)
    value: Optional[Any] = None
    transform: Optional[Dict[str, List[float]]] = None
    material_id: Optional[str] = Field(default=None, pattern=r"^mat_[a-z0-9_\-]+$")
    material_zone_id: Optional[str] = Field(default=None, max_length=120, pattern=r"^zone_[a-z0-9_\-]+$")
    replacement_component_id: Optional[str] = Field(default=None, pattern=r"^agentcomp_[a-z0-9_\-]+$")
    target_part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")
    target_connector_id: Optional[str] = Field(default=None, max_length=120)
    connector_id: Optional[str] = Field(default=None, max_length=120)
    structure_suggestion_id: Optional[str] = Field(default=None, pattern=r"^structure_[a-z0-9_\-]+$")

    @model_validator(mode="after")
    def validate_operation(self) -> "AgentPartEditOperation":
        if self.op == "set_part_transform":
            if not self.transform or any(
                key not in self.transform or len(self.transform[key]) != 3
                for key in ("position", "rotation", "scale")
            ):
                raise ValueError("set_part_transform requires position, rotation and scale vectors")
        elif self.op == "set_part_parameter":
            if not self.path or self.value is None:
                raise ValueError("set_part_parameter requires path and value")
        elif self.op == "set_joint_pose":
            if not self.transform or "rotation" not in self.transform:
                raise ValueError("set_joint_pose requires a rotation vector")
        elif self.op == "apply_material_preset":
            if not self.material_id:
                raise ValueError("apply_material_preset requires material_id")
            if self.material_zone_id is not None and not self.material_zone_id.startswith("zone_"):
                raise ValueError("apply_material_preset material_zone_id must be a stable zone id")
        elif self.op == "replace_part":
            if not self.replacement_component_id:
                raise ValueError("replace_part requires replacement_component_id")
        elif self.op == "snap_part_to_connector":
            if not self.target_part_id or not self.target_connector_id or not self.connector_id:
                raise ValueError("snap_part_to_connector requires source and target connectors")
        elif self.op == "split_part":
            if not self.structure_suggestion_id:
                raise ValueError("split_part requires structure_suggestion_id")
        elif self.op == "merge_parts":
            if not self.target_part_id or not self.structure_suggestion_id:
                raise ValueError("merge_parts requires target_part_id and structure_suggestion_id")
        return self


class AgentComponentRecord(StrictApiModel):
    schema_version: Literal["AgentComponent@1"] = "AgentComponent@1"
    component_id: str = Field(pattern=r"^agentcomp_[a-z0-9_\-]+$")
    project_id: str
    domain_pack_id: str
    role: str
    display_name: str
    description: str = ""
    source_asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    source_part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    part_template: BlockoutPartCandidate
    shape_operation: Dict[str, Any]
    material_bindings: Dict[str, str] = Field(default_factory=dict)
    status: Literal["active", "disabled"] = "active"
    # Computed from the newest report for the immutable source asset.  It is
    # intentionally not a second saved quality record on the component.
    source_quality_status: Literal["passed", "warning", "failed", "unavailable"] = "unavailable"
    created_at: str
    updated_at: str


AgentComponentCompatibilityReason = Literal[
    "same_domain_pack",
    "domain_pack_mismatch",
    "same_role",
    "role_mismatch",
    "component_active",
    "component_disabled",
    "source_quality_passed",
    "source_quality_warning",
    "source_quality_failed",
    "source_quality_unavailable",
    "target_connectors_preserved",
]


class AgentComponentCompatibility(StrictApiModel):
    """A bounded replacement decision, never an engineering fitness score."""

    schema_version: Literal["AgentComponentCompatibility@1"] = "AgentComponentCompatibility@1"
    component_id: str = Field(pattern=r"^agentcomp_[a-z0-9_\-]+$")
    target_asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    target_part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    eligible: bool
    source_quality_status: Literal["passed", "warning", "failed", "unavailable"]
    reason_codes: List[AgentComponentCompatibilityReason] = Field(min_length=1, max_length=8)


class AgentComponentCandidate(StrictApiModel):
    schema_version: Literal["AgentComponentCandidate@1"] = "AgentComponentCandidate@1"
    component: AgentComponentRecord
    compatibility: AgentComponentCompatibility


AgentStructureSuggestionKind = Literal["split_part", "merge_parts"]


class AgentStructureSuggestion(StrictApiModel):
    """Read-only, evidence-bound AssemblyGraph restructuring candidate."""

    schema_version: Literal["AgentStructureSuggestion@1"] = "AgentStructureSuggestion@1"
    suggestion_id: str = Field(pattern=r"^structure_[a-z0-9_\-]+$")
    kind: AgentStructureSuggestionKind
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    target_part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")
    affected_part_ids: List[str] = Field(min_length=1, max_length=4)
    source_facts: List[str] = Field(min_length=1, max_length=6)
    summary: str = Field(min_length=1, max_length=240)


class AgentStructureSuggestionList(StrictApiModel):
    schema_version: Literal["AgentStructureSuggestionList@1"] = "AgentStructureSuggestionList@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    suggestions: List[AgentStructureSuggestion] = Field(default_factory=list, max_length=32)
    unavailable_message: Optional[str] = Field(default=None, max_length=240)


SemanticProportionPath = Literal[
    "transform.scale.x",
    "transform.scale.y",
    "transform.scale.z",
]


class MechanicalStyleToken(StrictApiModel):
    """A visual-only mechanical language token, never an engineering preset."""

    schema_version: Literal["MechanicalStyleToken@1"] = "MechanicalStyleToken@1"
    token_id: str = Field(pattern=r"^style_[a-z0-9_\-]+$")
    version: Literal["1"] = "1"
    display_name: str = Field(min_length=1, max_length=80)
    description: str = Field(min_length=1, max_length=240)
    proportion_profile: Literal["compact", "balanced", "elongated", "substantial"]
    edge_language: Literal["soft", "controlled", "crisp"]
    surface_tension: Literal["relaxed", "neutral", "taut"]
    detail_density: Literal["low", "medium"]
    symmetry: Literal["bilateral", "radial", "assembly_driven"]
    material_palette: Literal["dark_metal", "clean_coating", "technical_composite", "mixed_industrial"]
    lighting_profile: Literal["cad_neutral", "soft_studio", "concept_contrast"]
    allowed_domains: List[DomainPackId] = Field(min_length=1, max_length=4)
    visual_only: Literal[True] = True
    provenance: Literal["forgecad_builtin"] = "forgecad_builtin"


SemanticProportionRoleSelector = Literal[
    "primary_form",
    "secondary_form",
    "cabin_form",
    "base_form",
    "upper_link_form",
    "end_effector_form",
]


class SemanticProportionAdjustment(StrictApiModel):
    role_selector: SemanticProportionRoleSelector
    path: SemanticProportionPath
    step_delta: Literal[-1, 1]


class DomainSemanticProportionRecipe(StrictApiModel):
    """One bounded domain recipe resolved only against a real G808 binding."""

    schema_version: Literal["DomainSemanticProportionRecipe@1"] = "DomainSemanticProportionRecipe@1"
    recipe_id: str = Field(pattern=r"^proportion_[a-z0-9_\-]+$")
    version: Literal["1"] = "1"
    domain_pack_id: DomainPackId
    style_token_id: str = Field(pattern=r"^style_[a-z0-9_\-]+$")
    display_name: str = Field(min_length=1, max_length=80)
    description: str = Field(min_length=1, max_length=240)
    intent_phrases: List[str] = Field(min_length=1, max_length=8)
    adjustments: List[SemanticProportionAdjustment] = Field(min_length=1, max_length=8)
    non_functional_only: Literal[True] = True


class ResolvedSemanticProportionOption(StrictApiModel):
    schema_version: Literal["ResolvedSemanticProportionOption@1"] = "ResolvedSemanticProportionOption@1"
    recipe_id: str = Field(pattern=r"^proportion_[a-z0-9_\-]+$")
    style_token: MechanicalStyleToken
    display_name: str = Field(min_length=1, max_length=80)
    description: str = Field(min_length=1, max_length=240)
    path: SemanticProportionPath
    current_value: float
    target_value: float
    min: float
    max: float
    step: float = Field(gt=0)
    unit: Literal["ratio"] = "ratio"
    source_operation_ids: List[str] = Field(min_length=1, max_length=32)


class ResolvedSemanticProportionOptions(StrictApiModel):
    schema_version: Literal["ResolvedSemanticProportionOptions@1"] = "ResolvedSemanticProportionOptions@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    domain_pack_id: DomainPackId
    runtime_manifest_version: Literal["ShapeProgramRuntimeManifest@1"] = "ShapeProgramRuntimeManifest@1"
    shape_program_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    glb_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    locked: bool
    options: List[ResolvedSemanticProportionOption] = Field(default_factory=list, max_length=16)
    unavailable_message: Optional[str] = Field(default=None, max_length=240)


class SaveAgentComponentRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    part_id: str = Field(pattern=r"^part_[a-z0-9_\-]+$")
    display_name: str = Field(min_length=1, max_length=120)
    description: str = Field(default="", max_length=500)


class ProposeAgentAssetChangeSetRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    summary: str = Field(min_length=1, max_length=500)
    operations: List[AgentPartEditOperation] = Field(min_length=1, max_length=16)
    protected_part_ids: List[str] = Field(default_factory=list, max_length=64)


class AgentAssetChangeSet(StrictApiModel):
    schema_version: Literal["AgentAssetChangeSet@1"] = "AgentAssetChangeSet@1"
    change_set_id: str = Field(pattern=r"^assetcs_[a-z0-9_\-]+$")
    project_id: str
    base_asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    summary: str
    operations: List[AgentPartEditOperation]
    protected_part_ids: List[str] = Field(default_factory=list)
    status: Literal["proposed", "previewed", "confirmed", "rejected", "stale"]
    preview: Optional[AgentAssetVersion] = None
    resulting_asset_version_id: Optional[str] = None
    created_at: str
    updated_at: str


class AgentAssetChangeSetConfirmResponse(StrictApiModel):
    change_set: AgentAssetChangeSet
    asset_version: AgentAssetVersion


class AgentAssetQualityFinding(StrictApiModel):
    check_id: str
    severity: Literal["info", "warning", "error"]
    message: str
    part_ids: List[str] = Field(default_factory=list)


class AgentAssetQualityReport(StrictApiModel):
    schema_version: Literal["AgentAssetQualityReport@1"] = "AgentAssetQualityReport@1"
    quality_report_id: str = Field(pattern=r"^quality_[a-z0-9_\-]+$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    status: Literal["passed", "warning", "failed", "unavailable"]
    triangle_count: int = Field(ge=0)
    bounds_mm: Optional[List[float]] = Field(default=None, min_length=3, max_length=3)
    evidence_source: Literal[
        "geometry_compile_readback",
        "external_glb_inspection",
        "compile_failure",
        "stale_compile_readback",
        "legacy_estimate",
    ] = "legacy_estimate"
    compile_readback: Optional[GeometryCompileReadback] = None
    findings: List[AgentAssetQualityFinding] = Field(default_factory=list)
    checked_at: str

    @model_validator(mode="after")
    def validate_geometry_evidence(self) -> "AgentAssetQualityReport":
        if self.bounds_mm is not None and any(not isfinite(value) or value <= 0 for value in self.bounds_mm):
            raise ValueError("quality bounds must be finite positive values")
        if self.evidence_source == "geometry_compile_readback":
            if self.compile_readback is None or self.bounds_mm is None:
                raise ValueError("compiled quality requires compile readback and bounds")
            if self.triangle_count != self.compile_readback.triangle_count or self.bounds_mm != self.compile_readback.bounds_mm:
                raise ValueError("quality facts must match compile readback")
        elif self.compile_readback is not None:
            raise ValueError("only compiled quality may carry compile readback")
        if self.evidence_source == "external_glb_inspection" and self.bounds_mm is None:
            raise ValueError("external GLB quality requires inspected bounds")
        if self.evidence_source == "compile_failure" and (self.status != "unavailable" or self.triangle_count != 0 or self.bounds_mm is not None):
            raise ValueError("compile failure quality must be unavailable without geometry facts")
        if self.evidence_source == "stale_compile_readback" and (
            self.status != "unavailable"
            or self.triangle_count != 0
            or self.bounds_mm is not None
        ):
            raise ValueError("stale compile quality must be unavailable without geometry facts")
        return self


class AgentAssetExportResponse(StrictApiModel):
    schema_version: Literal["AgentAssetExport@1"] = "AgentAssetExport@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    format: Literal["glb"] = "glb"
    glb_base64: str = Field(min_length=1)
    triangle_count: int = Field(ge=0)
    bounds_mm: List[float] = Field(min_length=3, max_length=3)
    readback_status: Literal["passed"] = "passed"
    readback_triangle_count: int = Field(ge=0)
    exported_at: str


class AgentAssetRenderView(StrictApiModel):
    schema_version: Literal["AgentAssetRenderView@1"] = "AgentAssetRenderView@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    view_id: Literal["iso", "front", "side", "top", "exploded_iso"]
    camera_view: Literal["iso", "front", "side", "top"]
    presentation_mode: Literal["standard", "exploded"] = "standard"
    background_mode: Literal["transparent"] = "transparent"
    # Only the exploded candidate declares the stable parts whose visual
    # spacing was derived from the current AssemblyGraph.
    part_ids: List[str] = Field(default_factory=list)
    mime_type: Literal["image/png"] = "image/png"
    width: int = Field(ge=64, le=2048)
    height: int = Field(ge=64, le=2048)
    png_base64: str = Field(min_length=1)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    byte_size: int = Field(ge=24)
    readback_status: Literal["passed"] = "passed"


class AgentAssetRenderSet(StrictApiModel):
    schema_version: Literal["AgentAssetRenderSet@1"] = "AgentAssetRenderSet@1"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    renderer_id: Literal["forgecad-agent-software-raster@1"] = "forgecad-agent-software-raster@1"
    width: int = Field(ge=64, le=2048)
    height: int = Field(ge=64, le=2048)
    views: List[AgentAssetRenderView] = Field(min_length=4, max_length=5)
    exploded_view_available: bool = False
    exploded_unavailable_reason: Optional[str] = Field(default=None, max_length=300)
    render_set_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    render_set_byte_size: int = Field(ge=96)
    rendered_at: str


class AgentAssetRenderPackageView(StrictApiModel):
    """A single PNG entry in an offline, presentation-only render package.

    This deliberately mirrors provenance from ``AgentAssetRenderView`` but
    excludes the PNG's base64 data.  The bytes live in the ZIP entry named by
    ``file_name``; keeping the manifest small and path-free makes it safe to
    inspect without unpacking arbitrary project files.
    """

    file_name: str = Field(pattern=r"^[a-z0-9_\-]+\.png$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    view_id: Literal["iso", "front", "side", "top", "exploded_iso"]
    camera_view: Literal["iso", "front", "side", "top"]
    presentation_mode: Literal["standard", "exploded"]
    background_mode: Literal["transparent"]
    part_ids: List[str] = Field(default_factory=list)
    mime_type: Literal["image/png"] = "image/png"
    width: int = Field(ge=64, le=2048)
    height: int = Field(ge=64, le=2048)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    byte_size: int = Field(ge=24)
    readback_status: Literal["passed"] = "passed"


class AgentAssetRenderPackageManifest(StrictApiModel):
    """The deterministic JSON manifest included in a concept-view ZIP.

    A package is a read-only representation of one current render set.  It
    is not an Agent asset export, a source package, or an assembly document.
    """

    schema_version: Literal["AgentAssetRenderPackage@1"] = "AgentAssetRenderPackage@1"
    package_kind: Literal["concept_view_png_bundle"] = "concept_view_png_bundle"
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    renderer_id: Literal["forgecad-agent-software-raster@1"] = "forgecad-agent-software-raster@1"
    render_set_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    render_set_byte_size: int = Field(ge=96)
    width: int = Field(ge=64, le=2048)
    height: int = Field(ge=64, le=2048)
    views: List[AgentAssetRenderPackageView] = Field(min_length=4, max_length=5)
    exploded_view_available: bool = False
    exploded_unavailable_reason: Optional[str] = Field(default=None, max_length=300)
    non_engineering_notice: Literal[
        "concept_views_only_not_engineering_or_manufacturing_data"
    ] = "concept_views_only_not_engineering_or_manufacturing_data"


class AgentActiveDesignReference(StrictApiModel):
    source: Literal["agent_asset"] = "agent_asset"
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    assembly_graph_id: str = Field(pattern=r"^mg_[a-z0-9_\-]+$")


class LegacyActiveDesignReference(StrictApiModel):
    source: Literal["legacy_concept_read_only"] = "legacy_concept_read_only"
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    legacy_version_id: str = Field(pattern=r"^ver_[a-z0-9_\-]+$")
    module_graph_id: str = Field(pattern=r"^mg_[a-z0-9_\-]+$")


ActiveDesignReference = Annotated[
    Union[AgentActiveDesignReference, LegacyActiveDesignReference],
    Field(discriminator="source"),
]


class ActiveDesignPreviewReference(StrictApiModel):
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    change_set_id: str = Field(pattern=r"^assetcs_[a-z0-9_\-]+$")
    base_asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")


class ActiveDesignQualityReference(StrictApiModel):
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    quality_report_id: str = Field(pattern=r"^quality_[a-z0-9_\-]+$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")


class ActiveDesignExportReference(StrictApiModel):
    source: Literal["agent_asset", "legacy_concept_read_only"]
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    source_version_id: str = Field(pattern=r"^(assetver|ver)_[a-z0-9_\-]+$")


class ActiveDesignRenderPreset(StrictApiModel):
    """Deterministic, visual-only camera and light selection for one Agent asset."""

    schema_version: Literal["ActiveDesignRenderPreset@1"] = "ActiveDesignRenderPreset@1"
    preset_id: str = Field(pattern=r"^render_[a-z0-9_\-]+$")
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    camera_view: Literal["iso", "front", "top", "right"] = "iso"
    light_preset: Literal["cad_neutral", "soft_studio", "concept_contrast"] = "cad_neutral"
    updated_at: str


class ActiveDesignPartDisplay(StrictApiModel):
    """Server-owned, non-geometric part presentation and edit-protection state."""

    schema_version: Literal["ActiveDesignPartDisplay@1"] = "ActiveDesignPartDisplay@1"
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    asset_version_id: str = Field(pattern=r"^assetver_[a-z0-9_\-]+$")
    locked_part_ids: List[str] = Field(default_factory=list)
    hidden_part_ids: List[str] = Field(default_factory=list)
    isolated_part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")

    @model_validator(mode="after")
    def validate_part_ids(self) -> "ActiveDesignPartDisplay":
        for label, values in (("locked_part_ids", self.locked_part_ids), ("hidden_part_ids", self.hidden_part_ids)):
            if len(values) != len(set(values)):
                raise ValueError(f"{label} must not contain duplicate part ids")
            if any(not re.fullmatch(r"part_[a-z0-9_\-]+", value) for value in values):
                raise ValueError(f"{label} must contain stable part ids")
        if self.isolated_part_id is not None and self.isolated_part_id in self.hidden_part_ids:
            raise ValueError("isolated_part_id cannot also be hidden")
        return self


class ActiveDesignSnapshot(StrictApiModel):
    """The future single server-owned workbench state; storage/API follow in S002/S003."""

    schema_version: Literal["ActiveDesignSnapshot@1"] = "ActiveDesignSnapshot@1"
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    active_design: ActiveDesignReference
    selected_part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")
    selected_material_zone_id: Optional[str] = Field(default=None, pattern=r"^zone_[a-z0-9_\-]+$")
    preview: Optional[ActiveDesignPreviewReference] = None
    quality: Optional[ActiveDesignQualityReference] = None
    export: ActiveDesignExportReference
    render_preset: Optional[ActiveDesignRenderPreset] = None
    part_display: Optional[ActiveDesignPartDisplay] = None
    revision: int = Field(ge=1)
    updated_at: str

    @model_validator(mode="after")
    def validate_snapshot_references(self) -> "ActiveDesignSnapshot":
        if self.active_design.project_id != self.project_id:
            raise ValueError("active_design.project_id must match snapshot project_id")
        if self.export.project_id != self.project_id:
            raise ValueError("export.project_id must match snapshot project_id")

        if self.render_preset is not None:
            if self.render_preset.project_id != self.project_id:
                raise ValueError("render_preset.project_id must match snapshot project_id")

        if isinstance(self.active_design, LegacyActiveDesignReference):
            if self.selected_part_id is not None:
                raise ValueError("legacy_concept_read_only snapshots cannot select an Agent part")
            if self.selected_material_zone_id is not None:
                raise ValueError("legacy_concept_read_only snapshots cannot select an Agent material zone")
            if self.preview is not None or self.quality is not None:
                raise ValueError("legacy_concept_read_only snapshots cannot attach Agent preview or quality")
            if self.export.source != "legacy_concept_read_only":
                raise ValueError("legacy snapshot export source must be legacy_concept_read_only")
            if self.export.source_version_id != self.active_design.legacy_version_id:
                raise ValueError("legacy snapshot export must reference the active legacy version")
            if self.render_preset is not None:
                raise ValueError("legacy_concept_read_only snapshots cannot attach an Agent render preset")
            if self.part_display is not None:
                raise ValueError("legacy_concept_read_only snapshots cannot attach Agent part display state")
            return self

        if self.selected_part_id is None and self.selected_material_zone_id is not None:
            raise ValueError("selected_material_zone_id requires selected_part_id")

        active_version_id = self.active_design.asset_version_id
        if self.render_preset is not None and self.render_preset.asset_version_id != active_version_id:
            raise ValueError("render_preset.asset_version_id must match the active Agent asset version")
        if self.part_display is not None:
            if self.part_display.project_id != self.project_id:
                raise ValueError("part_display.project_id must match snapshot project_id")
            if self.part_display.asset_version_id != active_version_id:
                raise ValueError("part_display.asset_version_id must match the active Agent asset version")
        if self.preview is not None:
            if self.preview.project_id != self.project_id:
                raise ValueError("preview.project_id must match snapshot project_id")
            if self.preview.base_asset_version_id != active_version_id:
                raise ValueError("preview base_asset_version_id must match the active Agent asset version")
        if self.quality is not None:
            if self.quality.project_id != self.project_id:
                raise ValueError("quality.project_id must match snapshot project_id")
            if self.quality.asset_version_id != active_version_id:
                raise ValueError("quality asset_version_id must match the active Agent asset version")
        if self.export.source != "agent_asset":
            raise ValueError("Agent snapshot export source must be agent_asset")
        if self.export.source_version_id != active_version_id:
            raise ValueError("Agent snapshot export must reference the active Agent asset version")
        return self


class SelectActiveDesignRequest(StrictApiModel):
    """Select (or clear) one part of the active Agent assembly."""

    client_request_id: str = Field(min_length=1, max_length=120)
    snapshot_revision: Optional[int] = Field(default=None, ge=1)
    selected_part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")
    selected_material_zone_id: Optional[str] = Field(default=None, pattern=r"^zone_[a-z0-9_\-]+$")


class SetActiveDesignRenderPresetRequest(StrictApiModel):
    """Update one visual preset without changing the Agent asset version."""

    client_request_id: str = Field(min_length=1, max_length=120)
    snapshot_revision: Optional[int] = Field(default=None, ge=1)
    camera_view: Literal["iso", "front", "top", "right"] = "iso"
    light_preset: Literal["cad_neutral", "soft_studio", "concept_contrast"] = "cad_neutral"


ActiveDesignPartDisplayAction = Literal["lock", "unlock", "hide", "show", "isolate", "clear_isolation", "show_all"]


class SetActiveDesignPartDisplayRequest(StrictApiModel):
    """Apply one intentionally small zero-basis part control action."""

    client_request_id: str = Field(min_length=1, max_length=120)
    snapshot_revision: Optional[int] = Field(default=None, ge=1)
    action: ActiveDesignPartDisplayAction
    part_id: Optional[str] = Field(default=None, pattern=r"^part_[a-z0-9_\-]+$")

    @model_validator(mode="after")
    def validate_action_target(self) -> "SetActiveDesignPartDisplayRequest":
        actions_with_part = {"lock", "unlock", "hide", "show", "isolate"}
        if self.action in actions_with_part and self.part_id is None:
            raise ValueError(f"{self.action} requires part_id")
        if self.action in {"clear_isolation", "show_all"} and self.part_id is not None:
            raise ValueError(f"{self.action} does not accept part_id")
        return self


class ConvertLegacyActiveDesignRequest(StrictApiModel):
    """Explicitly ask to rebuild a read-only legacy Concept as an Agent asset."""

    client_request_id: str = Field(min_length=1, max_length=120)
    snapshot_revision: Optional[int] = Field(default=None, ge=1)


class NavigateActiveDesignRequest(StrictApiModel):
    """Request a server-owned undo or redo of the active Agent asset."""

    client_request_id: str = Field(min_length=1, max_length=120)
    snapshot_revision: Optional[int] = Field(default=None, ge=1)


class ActiveDesignNavigation(StrictApiModel):
    """The only server-authoritative undo/redo availability shown by the workbench."""

    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    active_asset_version_id: Optional[str] = Field(default=None, pattern=r"^assetver_[a-z0-9_\-]+$")
    can_undo: bool
    can_redo: bool
    preview_pending: bool = False


class LegacyActiveDesignConversionResponse(StrictApiModel):
    """A safe conversion hand-off, not a claim that legacy geometry is editable."""

    schema_version: Literal["LegacyActiveDesignConversion@1"] = "LegacyActiveDesignConversion@1"
    project_id: str = Field(pattern=r"^prj_[a-z0-9_\-]+$")
    source: LegacyActiveDesignReference
    snapshot_revision: int = Field(ge=1)
    status: Literal["ready_for_agent_rebuild"] = "ready_for_agent_rebuild"
    message: str


class ImportAgentGlbRequest(StrictApiModel):
    """A self-contained GLB supplied by the user as a local reference asset."""

    client_request_id: str = Field(min_length=1, max_length=120)
    project_id: str = Field(min_length=1, max_length=160)
    domain_pack_id: str = Field(pattern=r"^pack_[a-z0-9_\-]+$")
    file_name: str = Field(min_length=1, max_length=180)
    glb_base64: str = Field(min_length=16, max_length=44_739_244)
    summary: str = Field(default="导入 GLB 参考模型", min_length=1, max_length=500)


class ImportedGlbInspectionResponse(StrictApiModel):
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    byte_size: int = Field(ge=20)
    triangle_count: int = Field(ge=1)
    bounds_mm: List[float] = Field(min_length=3, max_length=3)
    mesh_count: int = Field(ge=1)
    primitive_count: int = Field(ge=1)
    material_count: int = Field(ge=0)
    node_count: int = Field(ge=0)


class ImportAgentGlbResponse(StrictApiModel):
    asset_version: AgentAssetVersion
    inspection: ImportedGlbInspectionResponse


class AgentProviderCheckResponse(StrictApiModel):
    status: Literal["ready", "not_configured", "offline", "failed", "cancelled"]
    provider_id: str
    model: Optional[str] = None
    message: str
    network_call_made: bool
    connection: ProviderConnectionState
    execution_trace: List[ProviderExecutionTrace] = Field(default_factory=list)
