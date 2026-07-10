from __future__ import annotations

from typing import Any, Dict, List, Literal, Optional

from pydantic import BaseModel, ConfigDict, Field

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


class ProposeChangeSetRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    change_set: "DesignChangeSet"


class ChangeSetPreviewResponse(StrictApiModel):
    change_set: "DesignChangeSet"
    preview_spec: WeaponConceptSpec
    preview_graph: ModuleGraph
    preview_sha256: str
    issues: List[ModuleGraphValidationIssue] = Field(default_factory=list)


class ChangeSetConfirmResponse(StrictApiModel):
    change_set: "DesignChangeSet"
    project: ConceptProjectDetail


class CreateQualityRunRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    report: ModelQualityReport


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


class DesignBriefRecord(StrictApiModel):
    brief_id: str
    project_id: str
    source_text: str
    reference_asset_ids: List[str] = Field(default_factory=list)
    interpreted_spec: WeaponConceptSpec
    status: Literal["draft", "interpreted", "confirmed", "failed"]
    created_at: str
    updated_at: str
    job_id: Optional[str] = None


class GenerateDesignVariantsRequest(StrictApiModel):
    client_request_id: str = Field(min_length=1, max_length=120)
    brief_id: str
    count: Literal[3] = 3
    generator: Literal["deterministic_template"] = "deterministic_template"


class DesignVariantRecord(StrictApiModel):
    variant_id: str
    project_id: str
    brief_id: str
    rank: int = Field(ge=1, le=3)
    name: str
    summary: str
    module_graph: ModuleGraph
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
    include_quality_report: bool = True


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
    manifest: ConceptExportManifest
    created_at: str
