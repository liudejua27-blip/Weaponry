from __future__ import annotations

from typing import List, Optional

from pydantic import BaseModel, ConfigDict, Field

from forgecad_agent.domain.concepts.models import (
    ConceptConstraints,
    ConceptProportions,
    ConceptStyle,
    DesignDomainProfile,
    IntendedUse,
    ModuleAssetManifest,
    ModuleCategory,
    ModuleGraph,
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


class ModuleGraphRecord(StrictApiModel):
    graph: ModuleGraph
    graph_sha256: str
    validation_status: str
    created_at: str
    updated_at: str
