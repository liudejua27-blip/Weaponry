from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Dict, List, Literal, Optional

from pydantic import BaseModel, Field


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


JobStatus = Literal[
    "created",
    "queued",
    "running",
    "waiting_provider",
    "waiting_user",
    "retrying",
    "succeeded",
    "failed",
    "cancelled",
    "partial_succeeded",
]


class ErrorEnvelope(BaseModel):
    code: str
    message: str
    recoverable: bool = True
    details: Dict[str, Any] = Field(default_factory=dict)


class TargetOptions(BaseModel):
    phase: Literal["concept_to_rough_3d"] = "concept_to_rough_3d"
    engine: Literal["unity"] = "unity"
    output_format: Literal["glb"] = "glb"


class GenerationOptions(BaseModel):
    concept_count: int = Field(default=1, ge=1, le=1)
    seed: Optional[int] = None
    llm_provider_id: str = "mock_llm"
    image_provider_id: str = "mock_comfyui"
    three_d_provider_id: str = "mock_3d"


class CreateWeaponRequest(BaseModel):
    client_request_id: str
    text: str
    sketch_asset_id: Optional[str] = None
    reference_asset_ids: List[str] = Field(default_factory=list)
    auto_run: Literal[True] = True
    target: TargetOptions = Field(default_factory=TargetOptions)
    generation_options: GenerationOptions = Field(default_factory=GenerationOptions)


CombatAffordance = Literal[
    "attack",
    "defense",
    "area_control",
    "summon",
    "mobility",
    "reflect",
    "transform",
    "recover",
    "seal",
    "projectile",
]


class CreativeStructureGraph(BaseModel):
    skeleton: List[str] = Field(default_factory=list)
    interaction_path: List[str] = Field(default_factory=list)
    attack_sources: List[str] = Field(default_factory=list)
    movable_nodes: List[str] = Field(default_factory=list)
    energy_flow: List[str] = Field(default_factory=list)


class CreativeInterpretationRequest(BaseModel):
    client_request_id: str
    source_object: str
    raw_description: str
    desired_style: str = "3渲2国风神兵"
    freedom_level: Literal["conservative", "strange", "alien", "surreal"] = "strange"
    mythology_level: Literal["realistic_material", "guofeng_divine", "xianxia_mechanism", "mythic_concept"] = "guofeng_divine"
    gameplay_complexity: Literal["light", "multi_stage", "transform_linked", "multi_form"] = "multi_stage"
    asset_priority: Literal["concept_first", "lowpoly_first", "unity_ready_first"] = "lowpoly_first"


class CreativeInterpretationCandidate(BaseModel):
    candidate_id: str
    rank: int = Field(ge=1, le=3)
    name: str
    summary: str
    recast_summary: str
    combat_affordances: List[CombatAffordance] = Field(default_factory=list, min_length=1)
    confidence: float = Field(ge=0.0, le=1.0)
    anchor_points: List[str] = Field(default_factory=list, min_length=1)
    protected_regions: List[str] = Field(default_factory=list, min_length=1)
    skill_anchor_points: List[str] = Field(default_factory=list, min_length=1)
    risk_tags: List[str] = Field(default_factory=list, min_length=1)
    structure_graph: CreativeStructureGraph
    candidate_seed: int


class CreativeInterpretationResponse(BaseModel):
    interpretation_id: str
    weapon_id: str
    source_object: str
    raw_description: str
    status: Literal["ready", "resampled_ready", "failed"]
    needs_confirm: bool = True
    candidate_count: int = Field(ge=0, le=3)
    candidates: List[CreativeInterpretationCandidate] = Field(default_factory=list)
    stable_seed: int
    resample_attempted: bool = False
    preserved_candidate_id: Optional[str] = None
    candidate_snapshot_hash: str
    failure_code: Optional[str] = None
    failure_reason: Optional[str] = None
    created_at: str


class CreativeRecastConfirmRequest(BaseModel):
    client_request_id: str
    interpretation_id: str
    selected_candidate_id: str
    selected_candidate_rank: int = Field(ge=1, le=3)
    recast_mode: Literal["stylized_artifact", "game_asset", "mythic_mechanism"] = "stylized_artifact"
    recast_choice_text: Optional[str] = None


class CreativeWeaponGraphPayload(BaseModel):
    schema_version: Literal["CreativeWeaponGraph@1"] = "CreativeWeaponGraph@1"
    creative_graph_id: str
    weapon_id: str
    source_interpretation_id: str
    selected_candidate_id: str
    selected_candidate_rank: int = Field(ge=1, le=3)
    source_object: str
    recast_summary: str
    combat_affordances: List[CombatAffordance] = Field(default_factory=list, min_length=1)
    structure_graph: CreativeStructureGraph
    anchor_points: List[str] = Field(default_factory=list, min_length=1)
    protected_regions: List[str] = Field(default_factory=list, min_length=1)
    skill_anchor_points: List[str] = Field(default_factory=list, min_length=1)
    unity_handoff: Dict[str, str] = Field(default_factory=dict)
    non_manufacturing_asset: Literal[True] = True
    created_at: str


class SkillCard(BaseModel):
    slot: Literal["normal", "heavy", "mobility_or_defense", "control", "passive", "ultimate"]
    name: str
    trigger: str
    effect: str
    anchor_point: str
    combat_affordances: List[CombatAffordance] = Field(default_factory=list, min_length=1)
    cooldown_hint: Optional[str] = None
    cost_hint: Optional[str] = None


class SkillGraphPayload(BaseModel):
    schema_version: Literal["SkillGraph@1"] = "SkillGraph@1"
    skill_graph_id: str
    weapon_id: str
    origin_graph_id: str
    source_interpretation_id: str
    skills: List[SkillCard] = Field(default_factory=list, min_length=6, max_length=6)
    non_manufacturing_asset: Literal[True] = True
    created_at: str


class CreativeRecastConfirmResponse(BaseModel):
    weapon_id: str
    interpretation_id: str
    selected_candidate_id: str
    selected_candidate_rank: int
    creative_graph_id: str
    skill_graph_id: str
    creative_graph: CreativeWeaponGraphPayload
    skill_graph: SkillGraphPayload
    status: Literal["confirmed"] = "confirmed"
    created_at: str


class CreativeGraphResponse(BaseModel):
    weapon_id: str
    creative_graph_id: str
    skill_graph_id: Optional[str] = None
    interpretation_id: str
    creative_graph: CreativeWeaponGraphPayload
    skill_graph: Optional[SkillGraphPayload] = None


class PatchWeaponRequest(BaseModel):
    client_request_id: str
    source_version_id: str
    source_image_asset_id: str
    mask_asset_id: str
    patch_manifest_asset_id: str
    target_area: str
    instruction: str
    preserve: List[str] = Field(default_factory=list)
    strength: Literal["subtle", "medium", "strong"] = "medium"
    regenerate_3d: bool = False
    provider_id: str = "mock_comfyui"


class AssetUploadRequest(BaseModel):
    client_request_id: str
    role: Literal["patch_mask", "patch_manifest"]
    filename: str
    mime_type: Literal["image/png", "application/json"]
    data_base64: str
    metadata: Dict[str, Any] = Field(default_factory=dict)


class AssetUploadResponse(BaseModel):
    weapon_id: str
    version_id: str
    asset_id: str
    role: Literal["patch_mask", "patch_manifest"]
    logical_path: str
    sha256: str
    byte_size: int
    mime_type: str
    width: Optional[int] = None
    height: Optional[int] = None


class Generate3DRequest(BaseModel):
    client_request_id: str
    source_version_id: str
    source_image_asset_id: str
    provider_id: str = "mock_3d"
    target_format: Literal["glb"] = "glb"
    style: Literal["stylized_toon_weapon"] = "stylized_toon_weapon"
    orientation_policy: Dict[str, str] = Field(
        default_factory=lambda: {
            "forward_axis": "+Z",
            "long_axis": "+Y",
            "pivot": "grip_center",
        }
    )
    scale_policy: Literal["normalized_game_asset_scale"] = "normalized_game_asset_scale"
    build_unity_export: bool = True


class ExportUnityRequest(BaseModel):
    client_request_id: str
    model_id: Optional[str] = None
    export_type: Literal["unity_glb"] = "unity_glb"
    include_source_spec: bool = True
    include_quality_reports: bool = True


class JobAcceptedResponse(BaseModel):
    weapon_id: str
    job_id: str
    status: JobStatus
    event_stream_url: str


class JobActionResponse(BaseModel):
    action_id: str
    job_id: str
    status: JobStatus
    previous_status: JobStatus
    current_step: Optional[str] = None
    event_id: Optional[str] = None
    message: str
    event_stream_url: str
    retry_from: Optional[str] = None


class JobSummary(BaseModel):
    job_id: str
    weapon_id: Optional[str] = None
    weapon_name: Optional[str] = None
    type: str
    status: JobStatus
    current_step: Optional[str] = None
    error_code: Optional[str] = None
    error_message: Optional[str] = None
    event_count: int = 0
    action_count: int = 0
    latest_event_status: Optional[str] = None
    latest_event_message: Optional[str] = None
    latest_event_created_at: Optional[str] = None
    output_version_id: Optional[str] = None
    output_model_id: Optional[str] = None
    created_at: str
    updated_at: str
    finished_at: Optional[str] = None


class JobListResponse(BaseModel):
    items: List[JobSummary] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class JobActionAuditEntry(BaseModel):
    action_id: str
    job_id: str
    action_type: Literal["cancel", "retry", "retry_from_step"]
    requested_step: Optional[str] = None
    status: Literal["accepted", "rejected", "noop"]
    previous_job_status: str
    resulting_job_status: str
    event_id: Optional[str] = None
    message: str
    metadata: Dict[str, Any] = Field(default_factory=dict)
    created_at: str


class JobActionListResponse(BaseModel):
    items: List[JobActionAuditEntry] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class JobEvent(BaseModel):
    id: str
    seq: int
    job_id: str
    weapon_id: Optional[str] = None
    step: str
    level: Literal["info", "warning", "error"] = "info"
    status: str
    message: str
    artifact_asset_id: Optional[str] = None
    progress: Optional[float] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)
    created_at: str = Field(default_factory=utc_now)


class JobDetail(BaseModel):
    job_id: str
    weapon_id: str
    type: str
    status: JobStatus
    current_step: Optional[str]
    created_at: str
    updated_at: str
    outputs: Dict[str, Any] = Field(default_factory=dict)
    error: Optional[ErrorEnvelope] = None
    events: List[JobEvent] = Field(default_factory=list)


class ProviderTaskSummary(BaseModel):
    task_record_id: str
    job_id: str
    step: str
    attempt: int
    provider_kind: Literal["llm", "image", "three_d", "asset_store", "quality_checker", "unity"]
    provider_id: str
    provider_task_id: Optional[str] = None
    status: Literal["submitted", "polling", "cancel_requested", "cancelled", "succeeded", "failed", "unknown"]
    cancel_requested_at: Optional[str] = None
    last_seen_at: Optional[str] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)
    created_at: str
    updated_at: str


class JobCheckpointSummary(BaseModel):
    checkpoint_id: str
    job_id: str
    step: str
    attempt: int
    status: Literal["ready", "leased", "completed", "cancelled", "superseded"]
    resume_policy: Literal["restart_step", "skip_completed", "manual_review"]
    provider_task_record_id: Optional[str] = None
    state: Dict[str, Any] = Field(default_factory=dict)
    created_at: str
    updated_at: str


class JobRuntimeStateResponse(BaseModel):
    job_id: str
    status: JobStatus
    current_step: Optional[str] = None
    resumable: bool
    cancellable: bool
    provider_tasks: List[ProviderTaskSummary] = Field(default_factory=list)
    checkpoints: List[JobCheckpointSummary] = Field(default_factory=list)


class RuntimeRecoveryItem(BaseModel):
    job_id: str
    weapon_id: Optional[str] = None
    previous_status: JobStatus
    status: JobStatus
    resume_from_step: Optional[str] = None
    provider_task_id: Optional[str] = None
    event_id: str
    message: str


class RuntimeRecoveryResponse(BaseModel):
    recovered_count: int
    items: List[RuntimeRecoveryItem] = Field(default_factory=list)


class RuntimeWorkOnceResponse(BaseModel):
    claimed: bool
    job_id: Optional[str] = None
    job_type: Optional[str] = None
    status: Optional[JobStatus] = None
    message: str


class WeaponSummary(BaseModel):
    weapon_id: str
    display_name: str
    weapon_family: str
    stage: str
    current_version_id: Optional[str] = None
    current_model_id: Optional[str] = None
    thumbnail_asset_id: Optional[str] = None
    updated_at: str


class AssetFileSummary(BaseModel):
    asset_id: str
    role: str
    version_id: Optional[str] = None
    job_id: Optional[str] = None
    logical_path: str
    sha256: str
    byte_size: int
    mime_type: str
    width: Optional[int] = None
    height: Optional[int] = None
    created_at: str


class AssetFileResponse(AssetFileSummary):
    weapon_id: Optional[str] = None


class AssetRevealResponse(BaseModel):
    asset_id: str
    filename: str
    role: str
    dry_run: bool
    opened: bool
    target: str
    message: str


class WeaponVersionSummary(BaseModel):
    version_id: str
    parent_version_id: Optional[str] = None
    job_id: Optional[str] = None
    version_no: int
    version_type: str
    status: str
    summary: Optional[str] = None
    created_at: str
    assets: List[AssetFileSummary] = Field(default_factory=list)


class WeaponDetail(BaseModel):
    weapon_id: str
    display_name: str
    weapon_family: str
    fantasy_category: Optional[str] = None
    style: str
    stage: str
    current_version_id: Optional[str] = None
    current_model_id: Optional[str] = None
    thumbnail_asset_id: Optional[str] = None
    updated_at: str
    versions: List[WeaponVersionSummary] = Field(default_factory=list)
    current_spec: Dict[str, Any] = Field(default_factory=dict)
    current_model: Dict[str, Any] = Field(default_factory=dict)
    latest_jobs: List[Dict[str, Any]] = Field(default_factory=list)


class ProviderSettings(BaseModel):
    provider_id: str
    kind: Literal["llm", "image", "three_d"]
    type: str
    display_name: str
    enabled: bool = True
    status: str = "configured"
    base_url: Optional[str] = None
    has_secret: bool = False
    updated_at: str = Field(default_factory=utc_now)


class HealthResponse(BaseModel):
    status: str
    service: str
    mode: str


class WeaponListResponse(BaseModel):
    items: List[WeaponSummary] = Field(default_factory=list)
    next_cursor: Optional[str] = None


class ProviderSettingsListResponse(BaseModel):
    providers: List[ProviderSettings] = Field(default_factory=list)
