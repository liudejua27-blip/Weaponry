from __future__ import annotations

from typing import Any, Dict, List, Literal, Optional

from pydantic import BaseModel, ConfigDict, Field, model_validator

from forgecad_agent.domain.concepts.models import (
    ConceptExportManifest,
    ConceptConstraints,
    ConceptProportions,
    ConceptStyle,
    DesignChangeSet,
    DesignDomainProfile,
    IntendedUse,
    JobEventV2,
    ModuleAssetManifest,
    ModuleCategory,
    ModuleGraph,
    ModelQualityReport,
    WeaponConceptSpec,
)


class StrictApiModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class CreateConceptProjectRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    profile_id: str = "profile_weapon_concept_v1"
    name: str = Field(min_length=1, max_length=120)
    intended_uses: List[IntendedUse] = Field(min_length=1)
    style: ConceptStyle
    proportions: ConceptProportions
    required_slots: List[str] = Field(
        default_factory=lambda: ["core", "front", "rear", "grip"],
        min_length=1,
    )
    optional_slots: List[str] = Field(
        default_factory=lambda: ["top", "left", "right", "bottom", "side_panels"]
    )
    constraints: ConceptConstraints
    assumptions: List[str] = Field(
        default_factory=lambda: ["非功能性概念模型，不用于真实制造或使用"],
        min_length=1,
    )


class AppendConceptVersionRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    parent_version_id: str
    summary: str = Field(min_length=1, max_length=500)
    spec: WeaponConceptSpec
    module_graph_id: Optional[str] = None


class ConceptVersionSummary(StrictApiModel):
    version_id: str
    parent_version_id: Optional[str] = None
    version_no: int
    status: str
    summary: str
    spec_schema_version: str
    spec_sha256: str
    module_graph_id: Optional[str] = None
    change_set_id: Optional[str] = None
    created_at: str


class ConceptVersionDetail(ConceptVersionSummary):
    project_id: str
    spec: WeaponConceptSpec


class ConceptProjectSummary(StrictApiModel):
    project_id: str
    profile_id: str
    domain_type: str
    name: str
    status: str
    current_version_id: Optional[str] = None
    created_at: str
    updated_at: str


class ConceptProjectDetail(ConceptProjectSummary):
    profile: DesignDomainProfile
    current_spec: WeaponConceptSpec
    versions: List[ConceptVersionSummary] = Field(default_factory=list)


class ConceptProjectListResponse(StrictApiModel):
    items: List[ConceptProjectSummary] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class RegisterModuleAssetRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    manifest: ModuleAssetManifest
    logical_path: str = Field(min_length=1, max_length=500)
    glb_data_base64: str = Field(min_length=1)


class ModuleAssetRecord(StrictApiModel):
    manifest: ModuleAssetManifest
    logical_path: str
    object_path: str
    byte_size: int
    mime_type: str = "model/gltf-binary"
    created_at: str


class ModuleAssetListResponse(StrictApiModel):
    items: List[ModuleAssetRecord] = Field(default_factory=list)
    pack_id: Optional[str] = None
    category: Optional[ModuleCategory] = None
    next_cursor: Optional[str] = None


class ValidateModuleGraphRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    graph: ModuleGraph
    persist: bool = True


class ModuleGraphValidationIssue(StrictApiModel):
    code: str
    message: str
    node_id: Optional[str] = None
    edge_id: Optional[str] = None


class ModuleGraphValidationResponse(StrictApiModel):
    graph_id: str
    project_id: str
    valid: bool
    persisted: bool
    graph_sha256: str
    issues: List[ModuleGraphValidationIssue] = Field(default_factory=list)
    job_id: Optional[str] = None


class ModuleGraphRecord(StrictApiModel):
    graph: ModuleGraph
    graph_sha256: str
    validation_status: str
    created_at: str
    updated_at: str


PlannerGenerator = Literal[
    "auto",
    "configured_provider",
    "deterministic_rules",
    "deterministic_template",
]


class ConceptPlannerProvenance(StrictApiModel):
    generator: Literal["deterministic_rules", "openai_compatible"]
    provider_id: str = Field(min_length=1, max_length=120)
    provider_type: Literal["deterministic", "openai_compatible"]
    model: Optional[str] = Field(default=None, max_length=200)
    attempted_provider_id: Optional[str] = Field(default=None, max_length=120)
    attempted_provider_type: Optional[
        Literal["deterministic", "openai_compatible"]
    ] = None
    attempted_model: Optional[str] = Field(default=None, max_length=200)
    fallback_used: bool = False
    input_sha256: str = Field(pattern=r"^[0-9a-f]{64}$")
    output_sha256: str = Field(pattern=r"^[0-9a-f]{64}$")
    registry_module_ids: List[str] = Field(default_factory=list)
    warnings: List[str] = Field(default_factory=list, max_length=12)
    latency_ms: Optional[int] = Field(default=None, ge=0)
    input_tokens: Optional[int] = Field(default=None, ge=0)
    output_tokens: Optional[int] = Field(default=None, ge=0)
    total_tokens: Optional[int] = Field(default=None, ge=0)


class ProposeChangeSetRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    change_set: "DesignChangeSet"


class PlanDesignChangeSetRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    instruction: str = Field(min_length=3, max_length=2000)
    generator: PlannerGenerator = "auto"
    selected_node_id: Optional[str] = Field(default=None, max_length=160)
    selected_module_id: Optional[str] = Field(default=None, max_length=160)


class PlannedChangeSetRecord(StrictApiModel):
    change_set: "DesignChangeSet"
    instruction: str
    rationale: List[str] = Field(default_factory=list, max_length=12)
    planner_provenance: ConceptPlannerProvenance
    job_id: str


class ChangeSetPreviewResponse(StrictApiModel):
    change_set: "DesignChangeSet"
    preview_spec: WeaponConceptSpec
    preview_graph: ModuleGraph
    preview_sha256: str
    issues: List[ModuleGraphValidationIssue] = Field(default_factory=list)


class ChangeSetConfirmResponse(StrictApiModel):
    change_set: "DesignChangeSet"
    project: ConceptProjectDetail


class ChangeSetDiagnostic(StrictApiModel):
    code: str
    message: str
    stage: Literal["preview", "confirm"]
    recoverable: bool
    operation_ids: List[str] = Field(default_factory=list)
    node_ids: List[str] = Field(default_factory=list)
    recorded_at: str


class ChangeSetTimelineItem(StrictApiModel):
    change_set: "DesignChangeSet"
    base_version_id: str
    result_version_id: Optional[str] = None
    status: Literal["proposed", "previewed", "confirmed", "rejected", "stale"]
    actor_type: Literal["user", "planner"] = "user"
    planner_instruction: Optional[str] = None
    planner_rationale: List[str] = Field(default_factory=list, max_length=12)
    planner_provenance: Optional["ConceptPlannerProvenance"] = None
    planner_job_id: Optional[str] = None
    preview_sha256: Optional[str] = None
    diagnostic: Optional[ChangeSetDiagnostic] = None
    created_at: str
    updated_at: str
    confirmed_at: Optional[str] = None


class ChangeSetTimelineResponse(StrictApiModel):
    project_id: str
    items: List[ChangeSetTimelineItem] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class CreateQualityRunRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    report: ModelQualityReport


class InspectConceptVersionRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    ruleset_version: Literal["weapon-concept-geometry/1.3"] = "weapon-concept-geometry/1.3"


class QualityRunRecord(StrictApiModel):
    quality_run_id: str
    project_id: str
    version_id: str
    report: ModelQualityReport
    created_at: str
    job_id: Optional[str] = None


class InterpretDesignBriefRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    source_text: str = Field(min_length=3, max_length=4000)
    reference_asset_ids: List[str] = Field(default_factory=list, max_length=12)
    generator: PlannerGenerator = "auto"


class DesignBriefRecord(StrictApiModel):
    brief_id: str
    project_id: str
    source_text: str
    reference_asset_ids: List[str] = Field(default_factory=list)
    interpreted_spec: WeaponConceptSpec
    status: Literal["draft", "interpreted", "confirmed", "failed"]
    planner_provenance: ConceptPlannerProvenance
    created_at: str
    updated_at: str
    job_id: Optional[str] = None


class GenerateDesignVariantsRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    brief_id: str
    count: Literal[3] = 3
    generator: PlannerGenerator = "auto"


class DesignVariantRecord(StrictApiModel):
    variant_id: str
    project_id: str
    brief_id: str
    rank: int = Field(ge=1, le=3)
    name: str
    summary: str
    module_graph: ModuleGraph
    recommended_module_ids: List[str] = Field(default_factory=list)
    rationale: List[str] = Field(default_factory=list, max_length=12)
    planner_provenance: ConceptPlannerProvenance
    status: Literal["proposed", "selected", "rejected"]
    created_at: str


class DesignVariantListResponse(StrictApiModel):
    items: List[DesignVariantRecord] = Field(default_factory=list)
    next_cursor: Optional[str] = None
    job_id: Optional[str] = None


class SelectDesignVariantRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)


class ConceptJobRecord(StrictApiModel):
    job_id: str
    project_id: str
    version_id: Optional[str] = None
    type: str
    status: str
    current_step: Optional[str] = None
    input_hash: str
    input: Dict[str, Any] = Field(default_factory=dict)
    outputs: Dict[str, Any] = Field(default_factory=dict)
    error_code: Optional[str] = None
    error_message: Optional[str] = None
    created_at: str
    updated_at: str
    finished_at: Optional[str] = None
    events: List[JobEventV2] = Field(default_factory=list)


class ConceptJobEventListResponse(StrictApiModel):
    items: List[JobEventV2] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class CreateConceptExportRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    profile: IntendedUse
    include_modules: Literal[True] = True
    include_combined_glb: Literal[True] = True
    include_combined_obj: bool = False
    include_render_png: bool = False
    include_turntable_video: bool = False
    include_quality_report: bool = True

    @model_validator(mode="after")
    def validate_render_options(self) -> "CreateConceptExportRequest":
        if self.include_turntable_video and not self.include_render_png:
            raise ValueError("include_turntable_video requires include_render_png")
        return self


class ConceptExportRecord(StrictApiModel):
    export_id: str
    project_id: str
    version_id: str
    profile: IntendedUse
    status: str
    job_id: Optional[str] = None
    package_asset_id: str
    package_sha256: str
    package_byte_size: int = Field(ge=0)
    combined_glb_sha256: str
    combined_glb_byte_size: int = Field(ge=0)
    combined_obj_sha256: Optional[str] = None
    combined_obj_byte_size: Optional[int] = Field(default=None, ge=0)
    preview_png_sha256: Optional[str] = None
    preview_png_byte_size: Optional[int] = Field(default=None, ge=0)
    exploded_png_sha256: Optional[str] = None
    exploded_png_byte_size: Optional[int] = Field(default=None, ge=0)
    render_set_sha256: Optional[str] = None
    render_set_byte_size: Optional[int] = Field(default=None, ge=0)
    render_view_count: Optional[int] = Field(default=None, ge=0)
    turntable_frame_count: Optional[int] = Field(default=None, ge=0)
    turntable_video_sha256: Optional[str] = None
    turntable_video_byte_size: Optional[int] = Field(default=None, ge=0)
    turntable_video_mime_type: Optional[str] = None
    manifest: ConceptExportManifest
    created_at: str
