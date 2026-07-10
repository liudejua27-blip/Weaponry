from __future__ import annotations

from typing import List, Optional

from pydantic import BaseModel, ConfigDict, Field

from forgecad_agent.domain.concepts.models import (
    ConceptConstraints,
    ConceptProportions,
    ConceptStyle,
    DesignDomainProfile,
    IntendedUse,
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
