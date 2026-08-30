use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

#[path = "agentic_write_tools_session.rs"]
mod agentic_write_tools_session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgenticTool {
    SessionCreateOrResume,
    SessionGet,
    CheckpointPrepare,
    CheckpointGet,
    CheckpointRestorePrepare,
    ProductionStageTransitionPrepare,
    ProductionStageTransitionGet,
    ProductionStageTransitionV2Prepare,
    ProductionStageTransitionV2Get,
    ProductionStageTransitionV3Prepare,
    ProductionStageTransitionV3Get,
    ProductionCameraLockPrepare,
    ProductionCameraLockGet,
    ProductionWeaponFormEvidencePrepare,
    ProductionWeaponFormEvidenceGet,
    ProductionWeaponFormArtEvidencePrepare,
    ProductionWeaponFormArtEvidenceGet,
    ProductionWeaponArtDecisionProposalGet,
    ProductionWeaponAssemblyParameterSinkGet,
    ProductionWeaponFormQualityPrepare,
    ProductionWeaponFormQualityGet,
    ProductionWeaponFormQualityV2Prepare,
    ProductionWeaponFormQualityV2Get,
    ProductionWeaponFormQualityV2PreflightGet,
    ProductionWeaponHighLowBakePreflightGet,
    ProductionWeaponRetopologyCageSourcePrepare,
    ProductionWeaponRetopologyCageSourceGet,
    CandidateTopologyQualityPrepare,
    CandidateTopologyQualityGet,
    CandidateMaterialSurfaceQualityPrepare,
    CandidateMaterialSurfaceQualityGet,
    CandidateAnimationVfxQualityPrepare,
    CandidateAnimationVfxQualityGet,
    CandidateAnimationVfxQualityV2Prepare,
    CandidateAnimationVfxQualityV2Get,
    MechanicalAnimationClipV2Prepare,
    MechanicalAnimationClipV2Get,
    MechanicalAnimationClipV2Preview,
    MechanicalAnimationGlbV2Prepare,
    MechanicalAnimationGlbV2Get,
    GameWeaponAnimatedGlbSocketV2Prepare,
    GameWeaponAnimatedGlbSocketV2Get,
    FictionalEnergyVfxAnimatedSocketAttachmentPrepare,
    FictionalEnergyVfxAnimatedSocketAttachmentGet,
    FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare,
    FictionalEnergyVfxAnimatedSocketAttachmentV2Get,
    FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare,
    FictionalEnergyVfxAnimatedSocketAttachmentV3Get,
    GameWeaponAnimatedGlbSocketTransformProjectionPrepare,
    GameWeaponAnimatedGlbSocketTransformProjectionGet,
    GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare,
    GameWeaponAnimatedGlbSocketTransformProjectionV2Get,
    FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceGet,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare,
    FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get,
    FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceGet,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare,
    FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare,
    FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get,
}

impl AgenticTool {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "session_create_or_resume" => Self::SessionCreateOrResume,
            "session_get" => Self::SessionGet,
            "checkpoint_prepare" => Self::CheckpointPrepare,
            "checkpoint_get" => Self::CheckpointGet,
            "checkpoint_restore_prepare" => Self::CheckpointRestorePrepare,
            "production_stage_transition_prepare" => Self::ProductionStageTransitionPrepare,
            "production_stage_transition_get" => Self::ProductionStageTransitionGet,
            "production_stage_transition_v2_prepare" => Self::ProductionStageTransitionV2Prepare,
            "production_stage_transition_v2_get" => Self::ProductionStageTransitionV2Get,
            "production_stage_transition_v3_prepare" => Self::ProductionStageTransitionV3Prepare,
            "production_stage_transition_v3_get" => Self::ProductionStageTransitionV3Get,
            "production_camera_lock_prepare" => Self::ProductionCameraLockPrepare,
            "production_camera_lock_get" => Self::ProductionCameraLockGet,
            "production_weapon_form_evidence_prepare" => Self::ProductionWeaponFormEvidencePrepare,
            "production_weapon_form_evidence_get" => Self::ProductionWeaponFormEvidenceGet,
            "production_weapon_form_art_evidence_prepare" => {
                Self::ProductionWeaponFormArtEvidencePrepare
            }
            "production_weapon_form_art_evidence_get" => Self::ProductionWeaponFormArtEvidenceGet,
            "production_weapon_art_decision_proposal_get" => {
                Self::ProductionWeaponArtDecisionProposalGet
            }
            "production_weapon_assembly_parameter_sink_get" => {
                Self::ProductionWeaponAssemblyParameterSinkGet
            }
            "production_weapon_form_quality_prepare" => Self::ProductionWeaponFormQualityPrepare,
            "production_weapon_form_quality_get" => Self::ProductionWeaponFormQualityGet,
            "production_weapon_form_quality_v2_prepare" => {
                Self::ProductionWeaponFormQualityV2Prepare
            }
            "production_weapon_form_quality_v2_get" => Self::ProductionWeaponFormQualityV2Get,
            "production_weapon_form_quality_v2_preflight_get" => {
                Self::ProductionWeaponFormQualityV2PreflightGet
            }
            "production_weapon_high_low_bake_preflight_get" => {
                Self::ProductionWeaponHighLowBakePreflightGet
            }
            "production_weapon_retopology_cage_source_prepare" => {
                Self::ProductionWeaponRetopologyCageSourcePrepare
            }
            "production_weapon_retopology_cage_source_get" => {
                Self::ProductionWeaponRetopologyCageSourceGet
            }
            "candidate_topology_quality_prepare" => Self::CandidateTopologyQualityPrepare,
            "candidate_topology_quality_get" => Self::CandidateTopologyQualityGet,
            "candidate_material_surface_quality_prepare" => {
                Self::CandidateMaterialSurfaceQualityPrepare
            }
            "candidate_material_surface_quality_get" => Self::CandidateMaterialSurfaceQualityGet,
            "candidate_animation_vfx_quality_prepare" => Self::CandidateAnimationVfxQualityPrepare,
            "candidate_animation_vfx_quality_get" => Self::CandidateAnimationVfxQualityGet,
            "candidate_animation_vfx_quality_v2_prepare" => {
                Self::CandidateAnimationVfxQualityV2Prepare
            }
            "candidate_animation_vfx_quality_v2_get" => Self::CandidateAnimationVfxQualityV2Get,
            "mechanical_animation_clip_v2_prepare" => Self::MechanicalAnimationClipV2Prepare,
            "mechanical_animation_clip_v2_get" => Self::MechanicalAnimationClipV2Get,
            "mechanical_animation_clip_v2_preview" => Self::MechanicalAnimationClipV2Preview,
            "mechanical_animation_glb_v2_prepare" => Self::MechanicalAnimationGlbV2Prepare,
            "mechanical_animation_glb_v2_get" => Self::MechanicalAnimationGlbV2Get,
            "game_weapon_animated_glb_socket_v2_prepare" => {
                Self::GameWeaponAnimatedGlbSocketV2Prepare
            }
            "game_weapon_animated_glb_socket_v2_get" => Self::GameWeaponAnimatedGlbSocketV2Get,
            "fictional_energy_vfx_animated_socket_attachment_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentPrepare
            }
            "fictional_energy_vfx_animated_socket_attachment_get" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentGet
            }
            "fictional_energy_vfx_animated_socket_attachment_v2_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare
            }
            "fictional_energy_vfx_animated_socket_attachment_v2_get" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Get
            }
            "fictional_energy_vfx_animated_socket_attachment_v3_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare
            }
            "fictional_energy_vfx_animated_socket_attachment_v3_get" => {
                Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Get
            }
            "game_weapon_animated_glb_socket_transform_projection_prepare" => {
                Self::GameWeaponAnimatedGlbSocketTransformProjectionPrepare
            }
            "game_weapon_animated_glb_socket_transform_projection_get" => {
                Self::GameWeaponAnimatedGlbSocketTransformProjectionGet
            }
            "game_weapon_animated_glb_socket_transform_projection_v2_prepare" => {
                Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare
            }
            "game_weapon_animated_glb_socket_transform_projection_v2_get" => {
                Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Get
            }
            "fictional_energy_vfx_animated_socket_particles_sequence_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare
            }
            "fictional_energy_vfx_animated_socket_particles_sequence_get" => {
                Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet
            }
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare
            }
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_get" => {
                Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get
            }
            "fictional_energy_vfx_animated_socket_trails_sequence_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare
            }
            "fictional_energy_vfx_animated_socket_trails_sequence_get" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet
            }
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare
            }
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_get" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get
            }
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare
            }
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet
            }
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare
            }
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get" => {
                Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get
            }
            _ => return None,
        })
    }

    const fn name(self) -> &'static str {
        match self {
            Self::SessionCreateOrResume => "session_create_or_resume",
            Self::SessionGet => "session_get",
            Self::CheckpointPrepare => "checkpoint_prepare",
            Self::CheckpointGet => "checkpoint_get",
            Self::CheckpointRestorePrepare => "checkpoint_restore_prepare",
            Self::ProductionStageTransitionPrepare => "production_stage_transition_prepare",
            Self::ProductionStageTransitionGet => "production_stage_transition_get",
            Self::ProductionStageTransitionV2Prepare => "production_stage_transition_v2_prepare",
            Self::ProductionStageTransitionV2Get => "production_stage_transition_v2_get",
            Self::ProductionStageTransitionV3Prepare => "production_stage_transition_v3_prepare",
            Self::ProductionStageTransitionV3Get => "production_stage_transition_v3_get",
            Self::ProductionCameraLockPrepare => "production_camera_lock_prepare",
            Self::ProductionCameraLockGet => "production_camera_lock_get",
            Self::ProductionWeaponFormEvidencePrepare => "production_weapon_form_evidence_prepare",
            Self::ProductionWeaponFormEvidenceGet => "production_weapon_form_evidence_get",
            Self::ProductionWeaponFormArtEvidencePrepare => {
                "production_weapon_form_art_evidence_prepare"
            }
            Self::ProductionWeaponFormArtEvidenceGet => "production_weapon_form_art_evidence_get",
            Self::ProductionWeaponArtDecisionProposalGet => {
                "production_weapon_art_decision_proposal_get"
            }
            Self::ProductionWeaponAssemblyParameterSinkGet => {
                "production_weapon_assembly_parameter_sink_get"
            }
            Self::ProductionWeaponFormQualityPrepare => "production_weapon_form_quality_prepare",
            Self::ProductionWeaponFormQualityGet => "production_weapon_form_quality_get",
            Self::ProductionWeaponFormQualityV2Prepare => {
                "production_weapon_form_quality_v2_prepare"
            }
            Self::ProductionWeaponFormQualityV2Get => "production_weapon_form_quality_v2_get",
            Self::ProductionWeaponFormQualityV2PreflightGet => {
                "production_weapon_form_quality_v2_preflight_get"
            }
            Self::ProductionWeaponHighLowBakePreflightGet => {
                "production_weapon_high_low_bake_preflight_get"
            }
            Self::ProductionWeaponRetopologyCageSourcePrepare => {
                "production_weapon_retopology_cage_source_prepare"
            }
            Self::ProductionWeaponRetopologyCageSourceGet => {
                "production_weapon_retopology_cage_source_get"
            }
            Self::CandidateTopologyQualityPrepare => "candidate_topology_quality_prepare",
            Self::CandidateTopologyQualityGet => "candidate_topology_quality_get",
            Self::CandidateMaterialSurfaceQualityPrepare => {
                "candidate_material_surface_quality_prepare"
            }
            Self::CandidateMaterialSurfaceQualityGet => "candidate_material_surface_quality_get",
            Self::CandidateAnimationVfxQualityPrepare => "candidate_animation_vfx_quality_prepare",
            Self::CandidateAnimationVfxQualityGet => "candidate_animation_vfx_quality_get",
            Self::CandidateAnimationVfxQualityV2Prepare => {
                "candidate_animation_vfx_quality_v2_prepare"
            }
            Self::CandidateAnimationVfxQualityV2Get => "candidate_animation_vfx_quality_v2_get",
            Self::MechanicalAnimationClipV2Prepare => "mechanical_animation_clip_v2_prepare",
            Self::MechanicalAnimationClipV2Get => "mechanical_animation_clip_v2_get",
            Self::MechanicalAnimationClipV2Preview => "mechanical_animation_clip_v2_preview",
            Self::MechanicalAnimationGlbV2Prepare => "mechanical_animation_glb_v2_prepare",
            Self::MechanicalAnimationGlbV2Get => "mechanical_animation_glb_v2_get",
            Self::GameWeaponAnimatedGlbSocketV2Prepare => {
                "game_weapon_animated_glb_socket_v2_prepare"
            }
            Self::GameWeaponAnimatedGlbSocketV2Get => "game_weapon_animated_glb_socket_v2_get",
            Self::FictionalEnergyVfxAnimatedSocketAttachmentPrepare => {
                "fictional_energy_vfx_animated_socket_attachment_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketAttachmentGet => {
                "fictional_energy_vfx_animated_socket_attachment_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare => {
                "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Get => {
                "fictional_energy_vfx_animated_socket_attachment_v2_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare => {
                "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Get => {
                "fictional_energy_vfx_animated_socket_attachment_v3_get"
            }
            Self::GameWeaponAnimatedGlbSocketTransformProjectionPrepare => {
                "game_weapon_animated_glb_socket_transform_projection_prepare"
            }
            Self::GameWeaponAnimatedGlbSocketTransformProjectionGet => {
                "game_weapon_animated_glb_socket_transform_projection_get"
            }
            Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare => {
                "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
            }
            Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Get => {
                "game_weapon_animated_glb_socket_transform_projection_v2_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare => {
                "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet => {
                "fictional_energy_vfx_animated_socket_particles_sequence_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare => {
                "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get => {
                "fictional_energy_vfx_animated_socket_particles_sequence_v2_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare => {
                "fictional_energy_vfx_animated_socket_trails_sequence_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet => {
                "fictional_energy_vfx_animated_socket_trails_sequence_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare => {
                "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get => {
                "fictional_energy_vfx_animated_socket_trails_sequence_v2_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare => {
                "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet => {
                "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare => {
                "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare"
            }
            Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get => {
                "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get"
            }
        }
    }

    const fn runtime_method(self) -> &'static str {
        match self {
            Self::ProductionWeaponRetopologyCageSourcePrepare => {
                "production_weapon_retopology_cage_source_bundle_prepare"
            }
            Self::ProductionWeaponRetopologyCageSourceGet => {
                "production_weapon_retopology_cage_source_bundle_get"
            }
            _ => self.name(),
        }
    }

    const fn is_write(self) -> bool {
        matches!(
            self,
            Self::SessionCreateOrResume
                | Self::CheckpointPrepare
                | Self::CheckpointRestorePrepare
                | Self::ProductionStageTransitionPrepare
                | Self::ProductionStageTransitionV2Prepare
                | Self::ProductionStageTransitionV3Prepare
                | Self::ProductionCameraLockPrepare
                | Self::ProductionWeaponFormEvidencePrepare
                | Self::ProductionWeaponFormArtEvidencePrepare
                | Self::ProductionWeaponFormQualityPrepare
                | Self::ProductionWeaponFormQualityV2Prepare
                | Self::ProductionWeaponRetopologyCageSourcePrepare
                | Self::CandidateTopologyQualityPrepare
                | Self::CandidateMaterialSurfaceQualityPrepare
                | Self::CandidateAnimationVfxQualityPrepare
                | Self::CandidateAnimationVfxQualityV2Prepare
                | Self::MechanicalAnimationClipV2Prepare
                | Self::MechanicalAnimationGlbV2Prepare
                | Self::GameWeaponAnimatedGlbSocketV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentPrepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare
                | Self::GameWeaponAnimatedGlbSocketTransformProjectionPrepare
                | Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare
        )
    }

    const fn requires_approval(self) -> bool {
        !matches!(
            self,
            Self::ProductionWeaponFormQualityPrepare
                | Self::ProductionWeaponFormEvidencePrepare
                | Self::ProductionWeaponFormArtEvidencePrepare
                | Self::ProductionWeaponFormQualityV2Prepare
                | Self::CandidateTopologyQualityPrepare
                | Self::ProductionWeaponRetopologyCageSourcePrepare
                | Self::CandidateMaterialSurfaceQualityPrepare
                | Self::CandidateAnimationVfxQualityPrepare
                | Self::CandidateAnimationVfxQualityV2Prepare
                | Self::MechanicalAnimationClipV2Prepare
                | Self::MechanicalAnimationGlbV2Prepare
                | Self::GameWeaponAnimatedGlbSocketV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentPrepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare
                | Self::GameWeaponAnimatedGlbSocketTransformProjectionPrepare
                | Self::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare
                | Self::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare
        )
    }

    const fn requires_visual_state(self) -> bool {
        matches!(
            self,
            Self::CheckpointPrepare | Self::CheckpointRestorePrepare
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub candidate_id: Option<String>,
}

impl Binding {
    pub fn is_bound(&self) -> bool {
        self.session_id.is_some() && self.project_id.is_some() && self.candidate_id.is_some()
    }
}

pub fn is_tool(name: &str) -> bool {
    AgenticTool::from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    AgenticTool::from_name(name).is_some_and(AgenticTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    AgenticTool::from_name(name).map(AgenticTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    let tool = AgenticTool::from_name(name).expect("agentic tool name was checked");
    format!(
        "AGENTIC_RUNTIME_METHOD_UNAVAILABLE: {} requires Runtime method {}",
        tool.name(),
        tool.runtime_method()
    )
}

pub fn read_tools() -> Vec<Value> {
    [
        AgenticTool::SessionGet,
        AgenticTool::CheckpointGet,
        AgenticTool::ProductionStageTransitionGet,
        AgenticTool::ProductionStageTransitionV2Get,
        AgenticTool::ProductionStageTransitionV3Get,
        AgenticTool::ProductionCameraLockGet,
        AgenticTool::ProductionWeaponFormEvidenceGet,
        AgenticTool::ProductionWeaponFormArtEvidenceGet,
        AgenticTool::ProductionWeaponArtDecisionProposalGet,
        AgenticTool::ProductionWeaponAssemblyParameterSinkGet,
        AgenticTool::ProductionWeaponFormQualityGet,
        AgenticTool::ProductionWeaponFormQualityV2Get,
        AgenticTool::ProductionWeaponFormQualityV2PreflightGet,
        AgenticTool::ProductionWeaponHighLowBakePreflightGet,
        AgenticTool::ProductionWeaponRetopologyCageSourceGet,
        AgenticTool::CandidateTopologyQualityGet,
        AgenticTool::CandidateMaterialSurfaceQualityGet,
        AgenticTool::CandidateAnimationVfxQualityGet,
        AgenticTool::CandidateAnimationVfxQualityV2Get,
        AgenticTool::MechanicalAnimationClipV2Get,
        AgenticTool::MechanicalAnimationClipV2Preview,
        AgenticTool::MechanicalAnimationGlbV2Get,
        AgenticTool::GameWeaponAnimatedGlbSocketV2Get,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get,
    ]
    .into_iter()
    .map(read_tool_definition)
    .collect()
}

pub fn write_tools() -> Vec<Value> {
    [
        AgenticTool::SessionCreateOrResume,
        AgenticTool::CheckpointPrepare,
        AgenticTool::CheckpointRestorePrepare,
        AgenticTool::ProductionStageTransitionPrepare,
        AgenticTool::ProductionStageTransitionV2Prepare,
        AgenticTool::ProductionStageTransitionV3Prepare,
        AgenticTool::ProductionCameraLockPrepare,
        AgenticTool::ProductionWeaponFormEvidencePrepare,
        AgenticTool::ProductionWeaponFormArtEvidencePrepare,
        AgenticTool::ProductionWeaponFormQualityPrepare,
        AgenticTool::ProductionWeaponFormQualityV2Prepare,
        AgenticTool::ProductionWeaponRetopologyCageSourcePrepare,
        AgenticTool::CandidateTopologyQualityPrepare,
        AgenticTool::CandidateMaterialSurfaceQualityPrepare,
        AgenticTool::CandidateAnimationVfxQualityPrepare,
        AgenticTool::CandidateAnimationVfxQualityV2Prepare,
        AgenticTool::MechanicalAnimationClipV2Prepare,
        AgenticTool::MechanicalAnimationGlbV2Prepare,
        AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare,
    ]
    .into_iter()
    .map(write_tool_definition)
    .collect()
}

pub fn write_tool_names() -> Vec<String> {
    [
        AgenticTool::SessionCreateOrResume,
        AgenticTool::CheckpointPrepare,
        AgenticTool::CheckpointRestorePrepare,
        AgenticTool::ProductionStageTransitionPrepare,
        AgenticTool::ProductionStageTransitionV2Prepare,
        AgenticTool::ProductionStageTransitionV3Prepare,
        AgenticTool::ProductionCameraLockPrepare,
        AgenticTool::ProductionWeaponFormEvidencePrepare,
        AgenticTool::ProductionWeaponFormArtEvidencePrepare,
        AgenticTool::ProductionWeaponFormQualityPrepare,
        AgenticTool::ProductionWeaponFormQualityV2Prepare,
        AgenticTool::ProductionWeaponRetopologyCageSourcePrepare,
        AgenticTool::CandidateTopologyQualityPrepare,
        AgenticTool::CandidateMaterialSurfaceQualityPrepare,
        AgenticTool::CandidateAnimationVfxQualityPrepare,
        AgenticTool::CandidateAnimationVfxQualityV2Prepare,
        AgenticTool::MechanicalAnimationClipV2Prepare,
        AgenticTool::MechanicalAnimationGlbV2Prepare,
        AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare,
    ]
    .into_iter()
    .map(|tool| tool.name().to_owned())
    .collect()
}

fn read_tool_definition(tool: AgenticTool) -> Value {
    debug_assert!(!tool.is_write());
    json!({
        "name": tool.name(),
        "description": read_description(tool),
        "inputSchema": read_schema(tool),
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false,
            "writeIntent": false,
            "approvalRequired": false
        },
        "_meta": {"forgecad": {
            "availability": "available",
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": false,
            "transaction": "ADR-0026"
        }}
    })
}

fn write_tool_definition(tool: AgenticTool) -> Value {
    debug_assert!(tool.is_write());
    let (description, schema, idempotent) = agentic_write_tools_session::write_definition(tool)
        .unwrap_or_else(|| match tool {
        AgenticTool::ProductionStageTransitionPrepare => (
            "Atomically prepare one Runtime-owned production-stage transition. The first supported transition is draft to gray-model and is bound to the current candidate artifact, reference, camera and evidence. It never confirms, versions or exports the candidate.",
            production_stage_transition_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionStageTransitionV2Prepare => (
            "Prepare one approval-gated Runtime-owned V2 production-stage transition from the bound topology root candidate to a distinct material-surface output candidate. Both CandidateTopologyQuality@1 and CandidateMaterialSurfaceQuality@1 must be passed; invalid input is rejected before any write. The durable head advances only the structural topology-to-material-surface stage and never confirms, versions, exports or claims visual or commercial quality.",
            production_stage_transition_v2_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionStageTransitionV3Prepare => (
            "Prepare one hidden approval-gated Runtime-owned ProductionStage@3 transition from reference-intake to reference-coverage-reviewed. The first edge requires one exact same-candidate/same-artifact binding, durable ReferenceCanvas@1/DesignSpec@1 hashes, complete front/back/left/right/rear-three-quarter coverage and bound canvas lineage; it writes one immutable transition/head receipt only and never confirms, versions or exports.",
            production_stage_transition_v3_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionCameraLockPrepare => (
            "Prepare the hidden approval-gated ProductionCameraLock@1 prerequisite for the current passed V3 reference-coverage edge. It requires the exact six reviewed reference views and seven-view CameraRigCalibration@1 (left primary; bottom is calibration-only), writes only immutable rig/lock receipts, and never advances the ProductionStage head, confirms, versions or exports.",
            production_camera_lock_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponFormEvidencePrepare => (
            "Prepare one hidden Runtime-owned ProductionWeaponFormEvidence@1 receipt from the exact candidate-bound six-view ReferenceCanvas, CameraLock and RenderSet inputs. Runtime derives hash-bound Part-ID, negative-space and line-flow observations without rendering or accepting media bytes; it never advances a stage, confirms, versions or exports.",
            production_weapon_form_evidence_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponFormArtEvidencePrepare => (
            "Prepare one hidden Runtime-owned additive ProductionWeaponFormArtEvidence@1 receipt from the exact FormEvidence@1 parent. Runtime derives hash-bound target/AOV, Part-ID, negative-space and line-flow observations without accepting raw PNG/GLB/path/URL/script/secret data; quality remains NOT_PROVEN and it never advances a stage, confirms, versions or exports.",
            production_weapon_form_art_evidence_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponFormQualityPrepare => (
            "Prepare one hidden Runtime-owned FPS-FORM-04 structural form-quality receipt from an existing CrossViewEvidenceBundle. It revalidates the camera-calibrated head, ProductionCameraLock, ReferenceCanvas, DesignSpec, six reviewed views and their fixed-camera CAS without rendering or camera optimization; negative-space and line-flow remain NOT_PROVEN when no typed evidence exists, and no stage, confirmation, version or export is advanced.",
            production_weapon_form_quality_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponFormQualityV2Prepare => (
            "Prepare one hidden Runtime-owned additive ProductionWeaponFormQuality@2 gate from immutable FormQuality@1 and FormArt@1 parents plus the current Stage@3 source head. It records six exact typed view decisions and aggregate thresholds only; it has writeIntent but requires no approval, never advances a stage, confirms, versions or exports, and does not claim human, engine, distribution or commercial quality.",
            production_weapon_form_quality_v2_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponRetopologyCageSourcePrepare => (
            "Prepare one hidden Runtime-owned bounded Low-retopology and topology-correspondent Cage source bundle from the exact immutable High artifact. It persists only hash-bound structural source evidence, never raw GLB/mesh/offset arrays, advances stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            production_weapon_retopology_cage_source_prepare_schema(),
            true,
        ),
        AgenticTool::ProductionWeaponRetopologyCageSourceGet => {
            unreachable!("read-only retopology/Cage tool is not a write definition")
        }
        AgenticTool::CandidateTopologyQualityPrepare => (
            "Prepare a Runtime-owned objective topology quality gate for one exact gray-model candidate and its ordered renderable Parts. It stores bounded metrics and immutable readback bindings only; it never confirms, creates a version or exports, and no raw GLB bytes are returned.",
            candidate_topology_quality_prepare_schema(),
            true,
        ),
        AgenticTool::CandidateMaterialSurfaceQualityPrepare => (
            "Prepare one Runtime-owned structural material-surface quality report that binds a passed topology source candidate to one distinct derived Appearance candidate. It verifies geometry preservation, the first-party 2K MaterialPack, TextureBuild@2, CandidateSurfaceBake@1, UV, tangent and provenance evidence; it never advances the production stage, confirms, versions, exports or claims visual commercial quality.",
            candidate_material_surface_quality_prepare_schema(),
            true,
        ),
        AgenticTool::CandidateAnimationVfxQualityPrepare => (
            "Prepare one Runtime-owned structural animation/VFX quality report for the exact material-surface head candidate. It revalidates the durable delivery, rigid animation, animated sockets and full base/bloom/particles/trails/trail-bloom stack, records current socket-attachment gaps as a blocked technical gate, and never advances the stage, confirms, versions, exports or claims visual commercial quality.",
            candidate_animation_vfx_quality_prepare_schema(),
            true,
        ),
        AgenticTool::CandidateAnimationVfxQualityV2Prepare => (
            "Prepare one hidden Runtime-owned structural-only CandidateAnimationVfxQuality@2 for the exact material-surface head and dual geometry/appearance candidates. Runtime revalidates Projection@2, Particles@2, Trails@2, TrailsBloom@2 and one durable Attachment@3 with an ordered fifteen-frame digest; all twenty technical gates are derived from those durable parents, never from legacy V1 sidecar fields. This write reserves one JSON report only and never advances stage, confirms, versions, exports or claims visual, artistic, commercial-FPS or engine quality.",
            candidate_animation_vfx_quality_v2_prepare_schema(),
            true,
        ),
        AgenticTool::MechanicalAnimationClipV2Prepare => (
            "Prepare one Runtime-owned appearance-aware rigid MechanicalAnimationClip@2 bound to the exact appearance candidate, geometry source, material-surface quality report and AppearanceSourceLineage. Runtime performs deterministic geometry-plus-appearance replay before reservation; this structural-only write never confirms, versions, exports or returns raw GLB bytes.",
            mechanical_animation_clip_v2_prepare_schema(),
            true,
        ),
        AgenticTool::MechanicalAnimationGlbV2Prepare => (
            "Prepare one Runtime-owned appearance-aware MechanicalAnimationGlb@2 from the exact immutable Clip@2 and appearance candidate binding. Runtime writes only the derived animated GLB and receipt; this structural-only write never confirms, versions, exports or returns raw GLB, base64, paths, URLs or scripts.",
            mechanical_animation_glb_v2_prepare_schema(),
            true,
        ),
        AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare => (
            "Prepare one Runtime-owned appearance-aware V2 animated weapon-socket GLB materialization from the exact Clip@2, appearance delivery and AnchorSet bindings. Runtime preserves renderable content and animation structurally; this hidden opt-in write never confirms, versions, exports or returns raw GLB, base64, paths, URLs or scripts.",
            game_weapon_animated_glb_socket_v2_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare => (
            "Prepare one Runtime-owned structural-only animated weapon-socket attachment for the bounded fictional-energy VFX stack. It persists only hash-bound frame transforms and emitter/trail bindings; it never returns GLB, PNG or AOV bytes, advances a production stage, confirms, versions, exports or claims visual, functional or commercial-engine quality.",
            fictional_energy_vfx_animated_socket_attachment_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare => (
            "Prepare one Runtime-owned structural-only projection-bound animated weapon-socket attachment. It composes the exact animated GLB transform projection with the durable particle, trail and trail-Bloom sequences, persists only hash-bound frame summaries, never returns GLB, PNG or AOV bytes, advances a production stage, confirms, versions, exports or claims visual, functional or commercial-engine quality.",
            fictional_energy_vfx_animated_socket_attachment_v2_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare => (
            "Prepare one hidden Runtime-owned structural-only Attachment@3 bridge for the exact dual-candidate Projection@2, Particles@2, Trails@2 and TrailsBloom@2 chain. It requires a bound Ponytail design session and geometry candidate, a distinct appearance candidate, and exactly fifteen hash-only frame readbacks; it never returns GLB/PNG bytes, paths, URLs or scripts, advances a stage, confirms, versions, exports or claims visual or commercial quality.",
            fictional_energy_vfx_animated_socket_attachment_v3_prepare_schema(),
            true,
        ),
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare => (
            "Prepare one Runtime-owned bounded structural projection that independently replays the existing source animated GLB and derived animated-socket GLB into one through sixteen explicit samples of six socket local, parent-world and composed-world TRS transforms. It never returns raw GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            game_weapon_animated_glb_socket_transform_projection_prepare_schema(),
            true,
        ),
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare => (
            "Prepare one Runtime-owned appearance-aware GameWeaponAnimatedGlbSocketTransformProjection@2 from the real MechanicalAnimationGlb@2 and GameWeaponAnimatedGlbSocket@2 get/receipt parents. It records six composed TRS/matrix socket transforms per Clip@2 sample, remains structural-only, and never returns raw GLB/PNG bytes or claims visual, engine, commercial or functional quality.",
            game_weapon_animated_glb_socket_transform_projection_v2_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare => (
            "Prepare one Runtime-owned structural-only animated-socket particle sequence driven by the exact animated GLB socket transform projection. It persists bounded particle color/ID/depth evidence for one through sixteen frames, never returns GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            fictional_energy_vfx_animated_socket_particles_sequence_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare => (
            "Prepare one Runtime-owned structural-only V2 animated-socket particle sequence bound to the exact geometry and appearance candidates, material-surface quality report, animated socket projection and dual AnchorSets. It persists bounded particle color/ID/depth evidence for one through sixteen frames, never returns GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare => (
            "Prepare one Runtime-owned structural-only animated-socket trails sequence driven by the exact transform-projection and particle frames. It persists two bounded trail histories for one through fifteen output frames, never returns GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            fictional_energy_vfx_animated_socket_trails_sequence_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare => (
            "Prepare one additive Runtime-owned structural-only Trails@2 sequence bound to the exact dual-candidate Projection@2 and Particles@2 chain. It preserves explicit frame-zero pre-roll/history for fifteen output frames, never returns GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare => (
            "Prepare one Runtime-owned structural-only animated-socket trail-Bloom sequence bound to the exact trail, particle, base and Bloom frames. It reuses the first three upstream passes byte-exactly and persists only two new additive outputs for one through fifteen frames; it never returns GLB or PNG bytes, advances a stage, confirms, versions, exports or claims visual or commercial FPS quality.",
            fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare_schema(),
            true,
        ),
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare => (
            "Prepare one additive Runtime-owned structural-only TrailsBloom@2 sequence bound to the exact dual-candidate Projection@2, Particles@2 and Trails@2 chain. Runtime reuses the three source trail passes byte-exactly, renders only the emissive-source and Bloom-contribution passes through the fixed same-cohort Worker, and persists hash-bound sidecars without returning GLB/PNG bytes or claiming visual/commercial quality.",
            fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare_schema(),
            true,
        ),
        AgenticTool::SessionCreateOrResume
        | AgenticTool::CheckpointPrepare
        | AgenticTool::CheckpointRestorePrepare => {
            unreachable!("session/checkpoint write definition is delegated")
        }
        AgenticTool::SessionGet
        | AgenticTool::CheckpointGet
        | AgenticTool::ProductionStageTransitionGet
        | AgenticTool::ProductionStageTransitionV2Get
        | AgenticTool::ProductionStageTransitionV3Get
        | AgenticTool::ProductionCameraLockGet
        | AgenticTool::ProductionWeaponFormEvidenceGet
        | AgenticTool::ProductionWeaponFormArtEvidenceGet
        | AgenticTool::ProductionWeaponArtDecisionProposalGet
        | AgenticTool::ProductionWeaponAssemblyParameterSinkGet
        | AgenticTool::ProductionWeaponFormQualityGet
        | AgenticTool::ProductionWeaponFormQualityV2Get
        | AgenticTool::ProductionWeaponFormQualityV2PreflightGet
        | AgenticTool::ProductionWeaponHighLowBakePreflightGet
        | AgenticTool::CandidateTopologyQualityGet
        | AgenticTool::CandidateMaterialSurfaceQualityGet
        | AgenticTool::CandidateAnimationVfxQualityGet
        | AgenticTool::CandidateAnimationVfxQualityV2Get
        | AgenticTool::MechanicalAnimationClipV2Get
        | AgenticTool::MechanicalAnimationClipV2Preview
        | AgenticTool::MechanicalAnimationGlbV2Get
        | AgenticTool::GameWeaponAnimatedGlbSocketV2Get
        | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet
        | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get
        | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get
        | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet
        | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get
        | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet
        | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get
        | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet
        | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get
        | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet
        | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get => {
            unreachable!("read tool cannot be exposed as a write tool")
        }
        });
    let approval_required = tool.requires_approval();
    json!({
        "name": tool.name(),
        "description": description,
        "inputSchema": schema,
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "idempotentHint": idempotent,
            "openWorldHint": false,
            "writeIntent": true,
            "approvalRequired": approval_required
        },
        "_meta": {"forgecad": {
            "availability": "available",
            "runtime_method": tool.runtime_method(),
            "requiresConfirmation": approval_required,
            "transaction": "ADR-0026"
        }}
    })
}

fn read_description(tool: AgenticTool) -> &'static str {
    if let Some(description) = agentic_write_tools_session::read_description(tool) {
        return description;
    }
    match tool {
        AgenticTool::ProductionStageTransitionGet => {
            "Read one immutable Runtime-owned production-stage transition and its current durable production-stage head."
        }
        AgenticTool::ProductionStageTransitionV2Get => {
            "Read one immutable Runtime-owned V2 topology-to-material-surface transition and its durable dual-candidate production head after Runtime restart."
        }
        AgenticTool::ProductionStageTransitionV3Get => {
            "Read one immutable hidden ProductionStage@3 reference-coverage transition and its same-candidate durable head after Runtime restart; this lookup is read-only and fresh-process safe."
        }
        AgenticTool::ProductionCameraLockGet => {
            "Read one immutable hidden ProductionCameraLock@1 prerequisite after Runtime restart. Runtime independently revalidates the source V3 transition/head, exact six-view Canvas and seven-view rig/receipt; this lookup performs no write or stage advance."
        }
        AgenticTool::ProductionWeaponFormEvidenceGet => {
            "Read one immutable ProductionWeaponFormEvidence@1 receipt after Runtime restart. Runtime revalidates all six candidate/artifact/reference/camera/RenderSet bindings and returns only hash-bound typed observations; this lookup is read-only and returns no raw media."
        }
        AgenticTool::ProductionWeaponFormArtEvidenceGet => {
            "Read one immutable additive ProductionWeaponFormArtEvidence@1 receipt after Runtime restart. Runtime revalidates the exact FormEvidence@1 lineage and six target/AOV view bindings; this lookup is read-only, returns hash-bound typed observations only and keeps quality NOT_PROVEN."
        }
        AgenticTool::ProductionWeaponArtDecisionProposalGet => {
            "Read one immutable Runtime-owned ProductionWeaponArtDecisionProposal@1 projection for the exact session/project/candidate and hash-bound authoring evidence. Returns six views, five closed assembly groups, ten gates and explicit blockers only; it never renders, invokes a Worker, creates a candidate, advances a stage, confirms, versions or exports."
        }
        AgenticTool::ProductionWeaponAssemblyParameterSinkGet => {
            "Read one Runtime-owned ProductionWeaponAssemblyParameterSink@1 registry for the exact session/project/candidate and hash-bound GeometryProgram/AssemblyDecision lineage. It returns only zero to six available typed receiver/muzzle sinks plus explicit unavailable parameter IDs; it never writes Runtime/CAS, invokes a Worker, creates a candidate, advances a stage, confirms, versions or exports."
        }
        AgenticTool::ProductionWeaponFormQualityGet => {
            "Read one immutable FPS-FORM-04 structural form-quality receipt after Runtime restart. Runtime revalidates the stage/head, CameraLock, CrossViewEvidenceBundle and fixed-view CAS without rendering and never returns raw media or advances a stage."
        }
        AgenticTool::ProductionWeaponFormQualityV2Get => {
            "Read one immutable additive ProductionWeaponFormQuality@2 receipt after Runtime restart. Runtime revalidates the exact six view decisions, FormQuality@1/FormArt@1 lineage and Stage@3 source head; this lookup is read-only, returns typed hashes/decisions only, and never advances a stage or claims human/commercial quality."
        }
        AgenticTool::ProductionWeaponFormQualityV2PreflightGet => {
            "Read a closed Runtime-owned FormQuality@2 preflight projection from exact legacy FormQuality/FormArt/source-head hashes. It returns validator checks, readiness hashes and blockers only; it never writes Runtime/CAS, starts a Worker, advances a stage, confirms, versions or exports."
        }
        AgenticTool::ProductionWeaponHighLowBakePreflightGet => {
            "Read the closed Runtime-owned High/Low/Cage/Bake preflight projection for one exact session/project/candidate and expected ProductionStage@3 head. It returns structural checks and blockers only; it never writes Runtime/CAS, starts a Worker, advances a stage, confirms, versions or exports."
        }
        AgenticTool::ProductionWeaponRetopologyCageSourceGet => {
            "Read one immutable Runtime-owned Low-retopology and topology-correspondent Cage source bundle after Runtime restart. The response contains only bounded hashes and structural status flags, never raw GLB, mesh or offset arrays."
        }
        AgenticTool::CandidateTopologyQualityGet => {
            "Read one immutable Runtime-owned candidate topology quality gate by exact project and candidate binding. The response contains bounded metrics and references, never raw GLB bytes."
        }
        AgenticTool::CandidateMaterialSurfaceQualityGet => {
            "Read one immutable Runtime-owned material-surface quality report by exact project, topology-source candidate and Appearance-output candidate binding. The response contains hashes and technical gate states, never raw GLB or PNG bytes."
        }
        AgenticTool::CandidateAnimationVfxQualityGet => {
            "Read one immutable Runtime-owned animation/VFX structural quality report by exact project and material-surface head candidate. The response contains dependency hashes and technical gate states, never raw GLB or PNG bytes."
        }
        AgenticTool::CandidateAnimationVfxQualityV2Get => {
            "Read one immutable Runtime-owned CandidateAnimationVfxQuality@2 report after Runtime restart. The response contains only the exact dual-candidate, material-surface, Projection@2/Particles@2/Trails@2/TrailsBloom@2 and Attachment@3 all-fifteen-frame hash bindings plus truthful structural-only statuses, never raw GLB/PNG bytes, paths, URLs or scripts."
        }
        AgenticTool::MechanicalAnimationClipV2Get => {
            "Read one immutable appearance-aware MechanicalAnimationClip@2 by exact project and appearance-candidate binding after Runtime restart. The response contains deterministic replay/readback hashes only, never raw GLB or PNG bytes."
        }
        AgenticTool::MechanicalAnimationClipV2Preview => {
            "Read one scheduled tick from an immutable appearance-aware MechanicalAnimationClip@2. Geometry and appearance are replayed transiently through the fixed workers; no CAS, SQLite, candidate or version state is written and no raw GLB bytes are returned."
        }
        AgenticTool::MechanicalAnimationGlbV2Get => {
            "Read one immutable appearance-aware MechanicalAnimationGlb@2 by exact project, appearance-candidate and Clip@2 binding after Runtime restart. The response contains receipt, durable-link and hash metadata only, never raw GLB, base64, paths, URLs or scripts."
        }
        AgenticTool::GameWeaponAnimatedGlbSocketV2Get => {
            "Read one immutable appearance-aware V2 animated weapon-socket GLB materialization by exact project, appearance-candidate and Clip@2 binding after Runtime restart. The response contains receipt, durable_link and structural hash metadata only, never raw GLB, base64, paths, URLs or scripts."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet => {
            "Read one immutable Runtime-owned animated socket attachment by exact project and candidate binding after Runtime restart. The response contains hash-bound frame summaries only, never raw GLB, PNG or AOV bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get => {
            "Read one immutable Runtime-owned projection-bound animated socket attachment by exact project and candidate binding after Runtime restart. The response contains hash-bound frame summaries only, never raw GLB, PNG or AOV bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get => {
            "Read one immutable Attachment@3 bridge by exact project, geometry candidate, appearance candidate and delivery bindings after Runtime restart. The response contains exactly fifteen hash-bound frame summaries only, never raw GLB, PNG, paths, URLs or scripts."
        }
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet => {
            "Read one immutable Runtime-owned animated GLB socket transform projection by exact project, candidate and projection key after Runtime restart. The response contains bounded frame and transform summaries only, never raw GLB or PNG bytes."
        }
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get => {
            "Read one immutable Runtime-owned appearance-aware GameWeaponAnimatedGlbSocketTransformProjection@2 by exact project, appearance candidate, Clip@2 and projection key after Runtime restart. The response contains bounded six-socket TRS/matrix frame summaries only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet => {
            "Read one immutable Runtime-owned animated-socket particle sequence by exact project, candidate and sequence key after Runtime restart. The response contains bounded particle frame hashes only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get => {
            "Read one immutable Runtime-owned V2 animated-socket particle sequence by exact project, sequence key, geometry candidate, appearance candidate and both delivery manifests after Runtime restart. The response contains bounded particle frame hashes only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet => {
            "Read one immutable Runtime-owned animated-socket trails sequence by exact project, candidate and sequence key after Runtime restart. The response contains bounded trail frame hashes and history summaries only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get => {
            "Read one immutable additive Runtime-owned Trails@2 sequence by exact project, dual candidates, delivery manifests and sequence key after Runtime restart. The response contains bounded trail frame hashes and explicit pre-roll/history summaries only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet => {
            "Read one immutable Runtime-owned animated-socket trail-Bloom sequence by exact project, candidate and sequence key after Runtime restart. The response contains bounded upstream pass bindings and additive output hashes only, never raw GLB or PNG bytes."
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get => {
            "Read one immutable additive Runtime-owned TrailsBloom@2 sequence by exact project, dual candidates, delivery manifests and sequence key after Runtime restart. The response contains bounded upstream pass bindings and the two derived output hashes per frame only, never raw GLB or PNG bytes."
        }
        _ => unreachable!("write tool cannot use read description"),
    }
}

fn read_schema(tool: AgenticTool) -> Value {
    match tool {
        AgenticTool::SessionGet | AgenticTool::CheckpointGet => {
            agentic_write_tools_session::read_schema(tool)
                .expect("session/checkpoint read schema is defined")
        }
        AgenticTool::ProductionStageTransitionGet => production_stage_transition_get_schema(),
        AgenticTool::ProductionStageTransitionV2Get => production_stage_transition_v2_get_schema(),
        AgenticTool::ProductionStageTransitionV3Get => production_stage_transition_v3_get_schema(),
        AgenticTool::ProductionCameraLockGet => production_camera_lock_get_schema(),
        AgenticTool::ProductionWeaponFormEvidenceGet => {
            production_weapon_form_evidence_get_schema()
        }
        AgenticTool::ProductionWeaponFormArtEvidenceGet => {
            production_weapon_form_art_evidence_get_schema()
        }
        AgenticTool::ProductionWeaponArtDecisionProposalGet => {
            production_weapon_art_decision_proposal_get_schema()
        }
        AgenticTool::ProductionWeaponAssemblyParameterSinkGet => {
            production_weapon_assembly_parameter_sink_get_schema()
        }
        AgenticTool::ProductionWeaponFormQualityGet => production_weapon_form_quality_get_schema(),
        AgenticTool::ProductionWeaponFormQualityV2Get => {
            production_weapon_form_quality_v2_get_schema()
        }
        AgenticTool::ProductionWeaponFormQualityV2PreflightGet => {
            production_weapon_form_quality_v2_preflight_get_schema()
        }
        AgenticTool::ProductionWeaponHighLowBakePreflightGet => {
            production_weapon_high_low_bake_preflight_get_schema()
        }
        AgenticTool::ProductionWeaponRetopologyCageSourceGet => {
            production_weapon_retopology_cage_source_get_schema()
        }
        AgenticTool::CandidateTopologyQualityGet => candidate_topology_quality_get_schema(),
        AgenticTool::CandidateMaterialSurfaceQualityGet => {
            candidate_material_surface_quality_get_schema()
        }
        AgenticTool::CandidateAnimationVfxQualityGet => {
            candidate_animation_vfx_quality_get_schema()
        }
        AgenticTool::CandidateAnimationVfxQualityV2Get => {
            candidate_animation_vfx_quality_v2_get_schema()
        }
        AgenticTool::MechanicalAnimationClipV2Get => mechanical_animation_clip_v2_get_schema(),
        AgenticTool::MechanicalAnimationClipV2Preview => {
            mechanical_animation_clip_v2_preview_schema()
        }
        AgenticTool::MechanicalAnimationGlbV2Get => mechanical_animation_glb_v2_get_schema(),
        AgenticTool::GameWeaponAnimatedGlbSocketV2Get => {
            game_weapon_animated_glb_socket_v2_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet => {
            fictional_energy_vfx_animated_socket_attachment_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get => {
            fictional_energy_vfx_animated_socket_attachment_v2_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get => {
            fictional_energy_vfx_animated_socket_attachment_v3_get_schema()
        }
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet => {
            game_weapon_animated_glb_socket_transform_projection_get_schema()
        }
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get => {
            game_weapon_animated_glb_socket_transform_projection_v2_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet => {
            fictional_energy_vfx_animated_socket_particles_sequence_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get => {
            fictional_energy_vfx_animated_socket_particles_sequence_v2_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet => {
            fictional_energy_vfx_animated_socket_trails_sequence_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get => {
            fictional_energy_vfx_animated_socket_trails_sequence_v2_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet => {
            fictional_energy_vfx_animated_socket_trails_bloom_sequence_get_schema()
        }
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get => {
            fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get_schema()
        }
        _ => unreachable!("write tool cannot use read schema"),
    }
}

fn production_stage_transition_get_schema() -> Value {
    let mut properties = scope_properties();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionGetRequest@1"}),
    );
    properties.insert("session_id".to_owned(), id_property());
    properties.insert("transition_id".to_owned(), id_property());
    object_schema(
        vec![
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn production_stage_transition_prepare_schema() -> Value {
    let mut properties = scope_properties();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionPrepareRequest@1"}),
    );
    properties.insert("transition_id".to_owned(), id_property());
    properties.insert("session_id".to_owned(), id_property());
    properties.insert("from_stage".to_owned(), production_stage_property());
    properties.insert("to_stage".to_owned(), production_stage_property());
    properties.insert("candidate_state_sha256".to_owned(), sha256_property());
    properties.insert("artifact_sha256".to_owned(), sha256_property());
    properties.insert("output_kind".to_owned(), output_kind_property());
    properties.insert("output_object_sha256".to_owned(), sha256_property());
    properties.insert(
        "quality_report_object_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert(
        "comparison_report_object_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert("reference_id".to_owned(), id_property());
    properties.insert("reference_sha256".to_owned(), sha256_property());
    properties.insert("camera_hash".to_owned(), sha256_property());
    properties.insert("evidence_sha256".to_owned(), sha256_property());
    properties.insert("parent_checkpoint_id".to_owned(), nullable_id_property());
    properties.insert(
        "parent_checkpoint_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert("idempotency_key".to_owned(), id_property());
    properties.insert("input_sha256".to_owned(), sha256_property());
    properties.insert("approval_session_id".to_owned(), id_property());
    object_schema(
        vec![
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "candidate_id",
            "from_stage",
            "to_stage",
            "candidate_state_sha256",
            "artifact_sha256",
            "output_kind",
            "output_object_sha256",
            "quality_report_object_sha256",
            "comparison_report_object_sha256",
            "reference_id",
            "reference_sha256",
            "camera_hash",
            "evidence_sha256",
            "parent_checkpoint_id",
            "parent_checkpoint_sha256",
            "input_sha256",
            "idempotency_key",
            "approved",
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "approval_session_id",
        ],
        with_approval(properties),
    )
}

fn production_stage_transition_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionGetRequest@2"}),
    );
    for key in [
        "transition_id",
        "session_id",
        "project_id",
        "root_candidate_id",
        "head_candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "head_candidate_id",
        ],
        properties,
    )
}

fn production_stage_transition_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionPrepareRequest@2"}),
    );
    for key in [
        "transition_id",
        "session_id",
        "project_id",
        "root_candidate_id",
        "source_artifact_id",
        "previous_head_candidate_id",
        "previous_head_artifact_id",
        "head_candidate_id",
        "output_artifact_id",
        "topology_quality_id",
        "material_surface_quality_id",
        "reference_id",
        "approval_receipt_id",
        "approval_session_id",
        "parent_topology_transition_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "root_candidate_state_sha256",
        "root_artifact_sha256",
        "previous_head_candidate_state_sha256",
        "previous_head_artifact_sha256",
        "head_candidate_state_sha256",
        "head_artifact_sha256",
        "topology_quality_report_object_sha256",
        "topology_quality_canonical_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "reference_sha256",
        "camera_hash",
        "evidence_sha256",
        "parent_topology_transition_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "root_candidate_role".to_owned(),
        json!({"const":"topology-source"}),
    );
    properties.insert(
        "previous_head_candidate_role".to_owned(),
        json!({"const":"topology-source"}),
    );
    properties.insert(
        "previous_head_stage".to_owned(),
        json!({"const":"topology"}),
    );
    properties.insert(
        "head_candidate_role".to_owned(),
        json!({"const":"material-surface-output"}),
    );
    properties.insert("from_stage".to_owned(), json!({"const":"topology"}));
    properties.insert("to_stage".to_owned(), json!({"const":"material-surface"}));
    properties.insert(
        "topology_quality_status".to_owned(),
        json!({"const":"passed"}),
    );
    properties.insert(
        "material_surface_quality_status".to_owned(),
        json!({"const":"passed"}),
    );
    properties.insert(
        "candidate_binding_status".to_owned(),
        json!({"const":"distinct-root-topology-to-material-surface-head"}),
    );
    properties.insert(
        "parent_topology_transition_schema_version".to_owned(),
        json!({"const":"ProductionStageTransition@1"}),
    );
    properties.insert(
        "approval_expires_at".to_owned(),
        v2_approval_expires_at_property(),
    );
    properties.insert("approved".to_owned(), json!({"const":true}));
    properties.insert(
        "approval_summary".to_owned(),
        json!({"type":"string","minLength":1,"maxLength":512}),
    );
    object_schema(
        vec![
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "root_candidate_role",
            "root_candidate_state_sha256",
            "source_artifact_id",
            "root_artifact_sha256",
            "previous_head_candidate_id",
            "previous_head_candidate_role",
            "previous_head_candidate_state_sha256",
            "previous_head_artifact_id",
            "previous_head_artifact_sha256",
            "previous_head_stage",
            "head_candidate_id",
            "head_candidate_role",
            "head_candidate_state_sha256",
            "output_artifact_id",
            "head_artifact_sha256",
            "from_stage",
            "to_stage",
            "topology_quality_id",
            "topology_quality_status",
            "topology_quality_report_object_sha256",
            "topology_quality_canonical_sha256",
            "material_surface_quality_id",
            "material_surface_quality_status",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "candidate_binding_status",
            "reference_id",
            "reference_sha256",
            "camera_hash",
            "evidence_sha256",
            "approval_receipt_id",
            "approval_session_id",
            "approval_expires_at",
            "parent_topology_transition_id",
            "parent_topology_transition_sha256",
            "parent_topology_transition_schema_version",
            "input_sha256",
            "approved",
            "approval_summary",
            "idempotency_key",
        ],
        properties,
    )
}

fn production_stage_transition_v3_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionGetRequest@3"}),
    );
    for key in [
        "transition_id",
        "session_id",
        "project_id",
        "root_candidate_id",
        "head_candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "head_candidate_id",
        ],
        properties,
    )
}

fn production_stage_transition_v3_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionStageTransitionPrepareRequest@3"}),
    );
    for key in [
        "transition_id",
        "session_id",
        "project_id",
        "root_candidate_id",
        "root_candidate_role",
        "source_artifact_id",
        "previous_head_candidate_id",
        "previous_head_candidate_role",
        "previous_head_artifact_id",
        "head_candidate_id",
        "head_candidate_role",
        "output_artifact_id",
        "reference_id",
        "approval_receipt_id",
        "approval_session_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "root_candidate_state_sha256",
        "root_artifact_sha256",
        "previous_head_candidate_state_sha256",
        "previous_head_artifact_sha256",
        "head_candidate_state_sha256",
        "head_artifact_sha256",
        "reference_sha256",
        "camera_hash",
        "evidence_sha256",
        "reference_canvas_object_sha256",
        "design_spec_object_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "quality_report_object_sha256",
        "comparison_report_object_sha256",
        "visual_receipt_object_sha256",
        "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256",
        "distribution_receipt_object_sha256",
        "parent_transition_sha256",
    ] {
        properties.insert(key.to_owned(), nullable_sha256_property());
    }
    properties.insert("camera_lock_id".to_owned(), nullable_id_property());
    for key in [
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
    ] {
        properties.insert(key.to_owned(), nullable_sha256_property());
    }
    properties.insert(
        "camera_lock_source_transition_id".to_owned(),
        nullable_id_property(),
    );
    properties.insert("parent_transition_id".to_owned(), nullable_id_property());
    properties.insert(
        "parent_transition_schema_version".to_owned(),
        json!({"type":["string","null"],"pattern":"^ProductionStageTransition@[0-9]+$"}),
    );
    properties.insert(
        "previous_head_stage".to_owned(),
        json!({"enum":["reference-intake","reference-coverage-reviewed"]}),
    );
    properties.insert(
        "from_stage".to_owned(),
        json!({"enum":["reference-intake","reference-coverage-reviewed"]}),
    );
    properties.insert(
        "to_stage".to_owned(),
        json!({"enum":["reference-coverage-reviewed","camera-calibrated"]}),
    );
    properties.insert(
        "candidate_binding_status".to_owned(),
        json!({"const":"same-candidate-evidence"}),
    );
    properties.insert(
        "root_candidate_role".to_owned(),
        json!({"const":"reference-intake-candidate"}),
    );
    properties.insert(
        "previous_head_candidate_role".to_owned(),
        json!({"const":"reference-intake-candidate"}),
    );
    properties.insert(
        "head_candidate_role".to_owned(),
        json!({"const":"reference-intake-candidate"}),
    );
    properties.insert(
        "structural_status".to_owned(),
        json!({"const":"PASS_SOURCE_STRUCTURAL"}),
    );
    properties.insert(
        "visual_status".to_owned(),
        json!({"const":"QUALITY_TARGET_NOT_MET"}),
    );
    for key in ["human_status", "engine_status", "distribution_status"] {
        properties.insert(key.to_owned(), json!({"const":"NOT_RUN"}));
    }
    properties.insert(
        "approval_expires_at".to_owned(),
        v2_approval_expires_at_property(),
    );
    let required = vec![
        "schema_version",
        "transition_id",
        "session_id",
        "project_id",
        "root_candidate_id",
        "root_candidate_role",
        "root_candidate_state_sha256",
        "source_artifact_id",
        "root_artifact_sha256",
        "previous_head_candidate_id",
        "previous_head_candidate_role",
        "previous_head_candidate_state_sha256",
        "previous_head_artifact_id",
        "previous_head_artifact_sha256",
        "previous_head_stage",
        "head_candidate_id",
        "head_candidate_role",
        "head_candidate_state_sha256",
        "output_artifact_id",
        "head_artifact_sha256",
        "from_stage",
        "to_stage",
        "candidate_binding_status",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "evidence_sha256",
        "reference_canvas_object_sha256",
        "quality_report_object_sha256",
        "comparison_report_object_sha256",
        "design_spec_object_sha256",
        "visual_receipt_object_sha256",
        "human_review_receipt_object_sha256",
        "engine_validation_receipt_object_sha256",
        "distribution_receipt_object_sha256",
        "structural_status",
        "visual_status",
        "human_status",
        "engine_status",
        "distribution_status",
        "approval_receipt_id",
        "approval_session_id",
        "approval_expires_at",
        "parent_transition_id",
        "parent_transition_sha256",
        "parent_transition_schema_version",
        "input_sha256",
        "approved",
        "approval_summary",
        "idempotency_key",
    ];
    object_schema(required, with_approval(properties))
}

fn candidate_topology_quality_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateTopologyQualityGetRequest@1"}),
    );
    properties.insert("topology_quality_id".to_owned(), topology_id_property());
    properties.insert("project_id".to_owned(), topology_id_property());
    properties.insert("candidate_id".to_owned(), topology_id_property());
    object_schema(
        vec![
            "schema_version",
            "topology_quality_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn production_camera_lock_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionCameraLockGetRequest@1"}),
    );
    for key in ["camera_lock_id", "session_id", "project_id", "candidate_id"] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "camera_lock_id",
            "session_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn production_camera_lock_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionCameraLockPrepareRequest@1"}),
    );
    for key in [
        "camera_lock_id",
        "session_id",
        "project_id",
        "source_transition_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "approval_receipt_id",
        "approval_session_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "source_transition_sha256",
        "source_head_canonical_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "required_reference_view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","rear-three-quarter"]}),
    );
    properties.insert(
        "required_camera_view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","bottom","rear-three-quarter"]}),
    );
    properties.insert("primary_view_kind".to_owned(), json!({"const":"left"}));
    properties.insert(
        "calibration_policy".to_owned(),
        json!({"const":"fps-weapon-reviewed-six-reference-seven-camera-lock@1"}),
    );
    properties.insert(
        "camera_rig".to_owned(),
        json!({"type":"object","maxProperties":16}),
    );
    properties.insert(
        "approval_expires_at".to_owned(),
        v2_approval_expires_at_property(),
    );
    let required = vec![
        "schema_version",
        "camera_lock_id",
        "session_id",
        "project_id",
        "source_transition_id",
        "source_transition_sha256",
        "source_head_canonical_sha256",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
        "required_reference_view_kinds",
        "required_camera_view_kinds",
        "primary_view_kind",
        "calibration_policy",
        "input_sha256",
        "approved",
        "camera_rig",
        "approval_receipt_id",
        "approval_session_id",
        "approval_expires_at",
        "approval_summary",
        "idempotency_key",
    ];
    object_schema(required, with_approval(properties))
}

fn production_weapon_form_evidence_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormEvidenceGetRequest@1"}),
    );
    for key in [
        "form_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "form_evidence_id",
            "session_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn production_weapon_form_evidence_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormEvidencePrepareRequest@1"}),
    );
    for key in [
        "form_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "evidence_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","rear-three-quarter"]}),
    );
    properties.insert(
        "evidence_policy".to_owned(),
        json!({"const":"production-weapon-form-evidence-six-view-typed-observation@1"}),
    );
    let view_input = json!({
        "type":"object",
        "required":[
            "view_kind","view_id","reference_id","reference_sha256","camera_hash",
            "camera_canonical_sha256","render_set_object_sha256",
            "render_set_canonical_sha256","render_set_view_id"
        ],
        "properties":{
            "view_kind":{"enum":["front","back","left","right","top","rear-three-quarter"]},
            "view_id":v2_id_property(),
            "reference_id":v2_id_property(),
            "reference_sha256":sha256_property(),
            "camera_hash":sha256_property(),
            "camera_canonical_sha256":sha256_property(),
            "render_set_object_sha256":sha256_property(),
            "render_set_canonical_sha256":sha256_property(),
            "render_set_view_id":v2_id_property()
        },
        "additionalProperties":false
    });
    properties.insert(
        "views".to_owned(),
        json!({"type":"array","minItems":6,"maxItems":6,"uniqueItems":true,"items":view_input}),
    );
    object_schema(
        vec![
            "schema_version",
            "form_evidence_id",
            "session_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "reference_canvas_object_sha256",
            "reference_canvas_canonical_sha256",
            "design_spec_object_sha256",
            "design_spec_canonical_sha256",
            "camera_lock_id",
            "camera_lock_canonical_sha256",
            "camera_rig_object_sha256",
            "camera_rig_canonical_sha256",
            "camera_lock_receipt_object_sha256",
            "camera_lock_source_transition_id",
            "camera_lock_source_transition_sha256",
            "camera_lock_source_head_canonical_sha256",
            "view_kinds",
            "views",
            "evidence_policy",
            "evidence_policy_sha256",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn production_weapon_form_art_evidence_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormArtEvidenceGetRequest@1"}),
    );
    for key in [
        "art_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    let mut diagnostic_properties = Map::new();
    diagnostic_properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponRasterSourceAttributionDiagnosticGetRequest@1"}),
    );
    for key in [
        "diagnostic_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "view_id",
    ] {
        diagnostic_properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "input_sha256",
    ] {
        diagnostic_properties.insert(key.to_owned(), sha256_property());
    }
    diagnostic_properties.insert(
        "view_kind".to_owned(),
        json!({"enum":["left","right","rear-three-quarter"]}),
    );
    properties.insert(
        "raster_source_attribution_diagnostic".to_owned(),
        object_schema(
            vec![
                "schema_version",
                "diagnostic_id",
                "session_id",
                "project_id",
                "candidate_id",
                "candidate_state_sha256",
                "artifact_id",
                "artifact_sha256",
                "reference_id",
                "reference_sha256",
                "form_art_evidence_object_sha256",
                "form_art_evidence_canonical_sha256",
                "view_kind",
                "view_id",
                "camera_hash",
                "camera_canonical_sha256",
                "input_sha256",
            ],
            diagnostic_properties,
        ),
    );
    object_schema(
        vec![
            "schema_version",
            "art_evidence_id",
            "session_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn production_weapon_art_decision_proposal_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponArtDecisionProposalGetRequest@1"}),
    );
    for key in [
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "camera_lock_id",
        "form_evidence_id",
        "form_art_evidence_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "first_person_profile_id".to_owned(),
        json!({
            "type":["string","null"],
            "pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
        }),
    );
    properties.insert(
        "first_person_profile_sha256".to_owned(),
        nullable_sha256_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "session_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "geometry_program_sha256",
            "geometry_program_canonical_sha256",
            "operator_catalog_sha256",
            "reference_canvas_canonical_sha256",
            "design_spec_canonical_sha256",
            "camera_lock_id",
            "camera_lock_canonical_sha256",
            "form_evidence_id",
            "form_evidence_object_sha256",
            "form_evidence_canonical_sha256",
            "form_art_evidence_id",
            "form_art_evidence_object_sha256",
            "form_art_evidence_canonical_sha256",
            "first_person_profile_id",
            "first_person_profile_sha256",
        ],
        properties,
    )
}

fn production_weapon_assembly_parameter_sink_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponAssemblyParameterSinkGetRequest@1"}),
    );
    for key in [
        "sink_registry_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "assembly_registry_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "assembly_registry_canonical_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    object_schema(
        vec![
            "schema_version",
            "sink_registry_id",
            "session_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "geometry_program_sha256",
            "geometry_program_canonical_sha256",
            "operator_catalog_sha256",
            "assembly_registry_id",
            "assembly_registry_canonical_sha256",
        ],
        properties,
    )
}

fn production_weapon_form_art_evidence_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormArtEvidencePrepareRequest@1"}),
    );
    for key in [
        "art_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "art_evidence_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "art_evidence_policy".to_owned(),
        json!({"const":"production-weapon-form-art-evidence-six-view-typed-observation@1"}),
    );
    object_schema(
        vec![
            "schema_version",
            "art_evidence_id",
            "session_id",
            "project_id",
            "candidate_id",
            "form_evidence_object_sha256",
            "form_evidence_canonical_sha256",
            "art_evidence_policy",
            "art_evidence_policy_sha256",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn production_weapon_form_quality_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormQualityGetRequest@1"}),
    );
    for key in [
        "form_quality_id",
        "session_id",
        "project_id",
        "candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert(
        "form_stage".to_owned(),
        json!({"enum":["blockout","primary","secondary"]}),
    );
    object_schema(
        vec![
            "schema_version",
            "form_quality_id",
            "session_id",
            "project_id",
            "candidate_id",
            "form_stage",
        ],
        properties,
    )
}

fn production_weapon_form_quality_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormQualityPrepareRequest@1"}),
    );
    for key in [
        "form_quality_id",
        "session_id",
        "project_id",
        "camera_calibrated_head_transition_id",
        "camera_calibrated_head_candidate_id",
        "camera_calibrated_head_artifact_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "camera_calibrated_head_transition_sha256",
        "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "form_stage",
        "source_stage",
        "target_stage",
        "camera_calibrated_head_stage",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert(
        "reviewed_reference_view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","rear-three-quarter"]}),
    );
    properties.insert(
        "fixed_camera_view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","bottom","rear-three-quarter"]}),
    );
    properties.insert(
        "cross_view_evidence_view_kinds".to_owned(),
        json!({"const":["front","back","left","right","top","rear-three-quarter"]}),
    );
    properties.insert(
        "form_view_evaluations".to_owned(),
        json!({"type":"array","minItems":6,"maxItems":6,"items":{"type":"object","additionalProperties":false,"maxProperties":32}}),
    );
    properties.insert(
        "previous_form_quality_id".to_owned(),
        nullable_id_property(),
    );
    properties.insert(
        "previous_form_quality_report_object_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert(
        "previous_form_quality_canonical_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert(
        "form_quality_policy".to_owned(),
        json!({"const":"production-weapon-form-quality-six-view-no-regression@1"}),
    );
    properties.insert(
        "threshold_policy".to_owned(),
        json!({"const":"production-weapon-form-view-thresholds@1"}),
    );
    let required = vec![
        "schema_version",
        "form_quality_id",
        "session_id",
        "project_id",
        "form_stage",
        "source_stage",
        "target_stage",
        "camera_calibrated_head_transition_id",
        "camera_calibrated_head_transition_sha256",
        "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_id",
        "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_id",
        "camera_calibrated_head_artifact_sha256",
        "camera_calibrated_head_stage",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "reviewed_reference_view_kinds",
        "fixed_camera_view_kinds",
        "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256",
        "cross_view_evidence_view_kinds",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_view_evaluations",
        "previous_form_quality_id",
        "previous_form_quality_report_object_sha256",
        "previous_form_quality_canonical_sha256",
        "form_quality_policy",
        "form_quality_policy_sha256",
        "threshold_policy",
        "threshold_policy_sha256",
        "input_sha256",
        "idempotency_key",
    ];
    object_schema(required, properties)
}

fn production_weapon_form_quality_v2_stage_property() -> Value {
    json!({
        "enum":[
            "reference-intake",
            "reference-coverage-reviewed",
            "camera-calibrated",
            "blockout-reviewed",
            "primary-form-approved",
            "secondary-form-approved",
            "high-poly-approved",
            "low-poly-approved",
            "uv-approved",
            "cage-approved",
            "bake-approved",
            "material-approved",
            "rig-socket-approved",
            "animation-approved",
            "vfx-approved",
            "lod-collision-approved",
            "hero-art-review-approved",
            "engine-validated",
            "export-confirmed"
        ]
    })
}

fn production_weapon_form_quality_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormQualityGetRequest@2"}),
    );
    for key in [
        "form_quality_id",
        "session_id",
        "project_id",
        "candidate_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert(
        "form_stage".to_owned(),
        json!({"enum":["blockout","primary","secondary"]}),
    );
    production_weapon_form_quality_v2_scope_properties(&mut properties);
    let mut schema = object_schema(
        vec![
            "schema_version",
            "form_quality_id",
            "session_id",
            "project_id",
            "candidate_id",
            "form_stage",
            "evidence_source_kind",
        ],
        properties,
    );
    schema["allOf"] = production_weapon_form_quality_v2_scope_all_of();
    schema
}

fn production_weapon_form_quality_v2_preflight_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormQualityV2PreflightGetRequest@1"}),
    );
    for key in [
        "preflight_id",
        "session_id",
        "project_id",
        "candidate_id",
        "current_source_head_transition_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "form_stage".to_owned(),
        json!({"enum":["blockout","primary","secondary"]}),
    );
    production_weapon_form_quality_v2_scope_properties(&mut properties);
    let mut schema = object_schema(
        vec![
            "schema_version",
            "preflight_id",
            "session_id",
            "project_id",
            "candidate_id",
            "form_stage",
            "legacy_form_quality_object_sha256",
            "legacy_form_quality_canonical_sha256",
            "form_art_evidence_object_sha256",
            "form_art_evidence_canonical_sha256",
            "current_source_head_transition_id",
            "current_source_head_transition_sha256",
            "current_source_head_canonical_sha256",
            "input_sha256",
            "evidence_source_kind",
        ],
        properties,
    );
    schema["allOf"] = production_weapon_form_quality_v2_scope_all_of();
    schema
}

fn production_weapon_high_low_bake_preflight_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponHighLowBakePreflightGetRequest@1"}),
    );
    for key in [
        "preflight_id",
        "session_id",
        "project_id",
        "candidate_id",
        "expected_head_transition_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert(
        "expected_head_stage".to_owned(),
        json!({"enum":[
            "camera-calibrated",
            "blockout-reviewed",
            "primary-form-approved",
            "secondary-form-approved",
            "high-poly-approved",
            "low-poly-approved",
            "uv-approved",
            "cage-approved",
            "bake-approved"
        ]}),
    );
    for key in [
        "expected_head_transition_sha256",
        "expected_head_canonical_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    object_schema(
        vec![
            "schema_version",
            "preflight_id",
            "session_id",
            "project_id",
            "candidate_id",
            "expected_head_stage",
            "expected_head_transition_id",
            "expected_head_transition_sha256",
            "expected_head_canonical_sha256",
            "input_sha256",
        ],
        properties,
    )
}

fn production_weapon_form_quality_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponFormQualityPrepareRequest@2"}),
    );
    for key in [
        "form_quality_id",
        "session_id",
        "project_id",
        "current_source_head_transition_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "form_stage".to_owned(),
        json!({"enum":["blockout","primary","secondary"]}),
    );
    properties.insert(
        "source_stage".to_owned(),
        production_weapon_form_quality_v2_stage_property(),
    );
    properties.insert(
        "target_stage".to_owned(),
        production_weapon_form_quality_v2_stage_property(),
    );
    properties.insert(
        "previous_form_quality_id".to_owned(),
        json!({"type":["string","null"],"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"}),
    );
    properties.insert(
        "previous_form_quality_report_object_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert(
        "previous_form_quality_canonical_sha256".to_owned(),
        nullable_sha256_property(),
    );
    properties.insert(
        "form_quality_policy".to_owned(),
        json!({"const":"production-weapon-form-quality-six-view-art-evidence-gate@2"}),
    );
    properties.insert(
        "threshold_policy".to_owned(),
        json!({"const":"production-weapon-form-view-thresholds@1"}),
    );
    production_weapon_form_quality_v2_scope_properties(&mut properties);
    let mut schema = object_schema(
        vec![
            "schema_version",
            "form_quality_id",
            "session_id",
            "project_id",
            "form_stage",
            "source_stage",
            "target_stage",
            "legacy_form_quality_object_sha256",
            "legacy_form_quality_canonical_sha256",
            "form_art_evidence_object_sha256",
            "form_art_evidence_canonical_sha256",
            "current_source_head_transition_id",
            "current_source_head_transition_sha256",
            "current_source_head_canonical_sha256",
            "previous_form_quality_id",
            "previous_form_quality_report_object_sha256",
            "previous_form_quality_canonical_sha256",
            "form_quality_policy",
            "form_quality_policy_sha256",
            "threshold_policy",
            "threshold_policy_sha256",
            "input_sha256",
            "idempotency_key",
            "evidence_source_kind",
        ],
        properties,
    );
    let mut stage_rules = json!([
        {
            "if":{"properties":{"form_stage":{"const":"blockout"}},"required":["form_stage"]},
            "then":{"properties":{
                "source_stage":{"const":"camera-calibrated"},
                "target_stage":{"const":"blockout-reviewed"},
                "previous_form_quality_id":{"type":"null"},
                "previous_form_quality_report_object_sha256":{"type":"null"},
                "previous_form_quality_canonical_sha256":{"type":"null"}
            }}
        },
        {
            "if":{"properties":{"form_stage":{"const":"primary"}},"required":["form_stage"]},
            "then":{"properties":{
                "source_stage":{"const":"blockout-reviewed"},
                "target_stage":{"const":"primary-form-approved"},
                "previous_form_quality_id":{"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
                "previous_form_quality_report_object_sha256":sha256_property(),
                "previous_form_quality_canonical_sha256":sha256_property()
            }}
        },
        {
            "if":{"properties":{"form_stage":{"const":"secondary"}},"required":["form_stage"]},
            "then":{"properties":{
                "source_stage":{"const":"primary-form-approved"},
                "target_stage":{"const":"secondary-form-approved"},
                "previous_form_quality_id":{"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
                "previous_form_quality_report_object_sha256":sha256_property(),
                "previous_form_quality_canonical_sha256":sha256_property()
            }}
        }
    ]);
    if let Some(rules) = stage_rules.as_array_mut() {
        if let Some(scope_rules) = production_weapon_form_quality_v2_scope_all_of().as_array() {
            rules.splice(0..0, scope_rules.iter().cloned());
        }
    }
    schema["allOf"] = stage_rules;
    schema
}

fn production_weapon_retopology_cage_source_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponRetopologyCageSourceBundleGetRequest@1"}),
    );
    for key in ["bundle_key_sha256", "project_id", "source_candidate_id"] {
        properties.insert(
            key.to_owned(),
            if key == "bundle_key_sha256" {
                sha256_property()
            } else {
                v2_id_property()
            },
        );
    }
    object_schema(
        vec![
            "schema_version",
            "bundle_key_sha256",
            "project_id",
            "source_candidate_id",
        ],
        properties,
    )
}

fn production_weapon_retopology_cage_source_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"ProductionWeaponRetopologyCageSourceBundlePrepareRequest@1"}),
    );
    for key in ["project_id", "source_candidate_id", "idempotency_key"] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert("bundle_key_sha256".to_owned(), nullable_sha256_property());
    for key in [
        "source_candidate_state_sha256",
        "source_high_artifact_sha256",
        "source_high_artifact_readback_object_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "target_triangle_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":1000000}),
    );
    properties.insert(
        "max_collapses".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":1000000}),
    );
    properties.insert(
        "locked_vertices".to_owned(),
        json!({
            "type":"array",
            "maxItems":16384,
            "items":{
                "type":"object",
                "required":["primitive_ordinal","vertex_index"],
                "properties":{
                    "primitive_ordinal":{"type":"integer","minimum":0,"maximum":1000000},
                    "vertex_index":{"type":"integer","minimum":0,"maximum":100000000}
                },
                "additionalProperties":false
            }
        }),
    );
    for key in ["offset_m", "max_offset_m"] {
        properties.insert(
            key.to_owned(),
            json!({"type":"number","exclusiveMinimum":0,"maximum":1}),
        );
    }
    properties.insert(
        "max_coordinate_abs_m".to_owned(),
        json!({"type":"number","exclusiveMinimum":0,"maximum":1000}),
    );
    properties.insert(
        "low_retopology_policy".to_owned(),
        json!({"const":"bounded-low-retopology-topology-correspondent-cage-source-only@1"}),
    );
    properties.insert(
        "cage_policy".to_owned(),
        json!({"const":"bounded-low-retopology-topology-correspondent-cage-source-only@1"}),
    );
    object_schema(
        vec![
            "schema_version",
            "bundle_key_sha256",
            "project_id",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "source_high_artifact_sha256",
            "source_high_artifact_readback_object_sha256",
            "target_triangle_count",
            "max_collapses",
            "locked_vertices",
            "offset_m",
            "max_offset_m",
            "max_coordinate_abs_m",
            "low_retopology_policy",
            "cage_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn candidate_topology_quality_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateTopologyQualityPrepareRequest@1"}),
    );
    for key in [
        "topology_quality_id",
        "project_id",
        "candidate_id",
        "artifact_id",
    ] {
        properties.insert(key.to_owned(), topology_id_property());
    }
    for key in [
        "candidate_state_sha256",
        "artifact_sha256",
        "artifact_readback_sha256",
        "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256",
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "part_inventory_sha256",
        "topology_quality_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "part_ids".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":512,
            "uniqueItems":true,
            "items":topology_id_property()
        }),
    );
    properties.insert(
        "part_topology_snapshot_sha256s".to_owned(),
        sha256_list_property(true),
    );
    properties.insert(
        "authoring_topology_status".to_owned(),
        json!({"enum":["complete","partial","not-available"]}),
    );
    properties.insert(
        "part_authoring_topology_sha256s".to_owned(),
        nullable_sha256_list_property(),
    );
    properties.insert(
        "topology_quality_policy".to_owned(),
        json!({"const":"candidate-topology-hard-gate@1"}),
    );
    properties.insert("from_stage".to_owned(), json!({"const":"gray-model"}));
    properties.insert("to_stage".to_owned(), json!({"const":"topology"}));
    properties.insert("idempotency_key".to_owned(), topology_id_property());
    let mut schema = object_schema(
        vec![
            "schema_version",
            "topology_quality_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "artifact_readback_sha256",
            "artifact_readback_object_sha256",
            "geometry_candidate_evidence_sha256",
            "geometry_program_sha256",
            "geometry_program_object_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "part_inventory_sha256",
            "part_ids",
            "part_topology_snapshot_sha256s",
            "authoring_topology_status",
            "part_authoring_topology_sha256s",
            "topology_quality_policy",
            "topology_quality_policy_sha256",
            "from_stage",
            "to_stage",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    );
    schema["allOf"] = json!([
        {
            "if": {"properties":{"authoring_topology_status":{"const":"complete"}},"required":["authoring_topology_status"]},
            "then": {"properties":{"part_authoring_topology_sha256s":{
                "type":"array","minItems":1,"maxItems":512,"items":sha256_property()
            }}}
        },
        {
            "if": {"properties":{"authoring_topology_status":{"const":"not-available"}},"required":["authoring_topology_status"]},
            "then": {"properties":{"part_authoring_topology_sha256s":{
                "type":"array","minItems":1,"maxItems":512,"items":{"type":"null"}
            }}}
        }
    ]);
    schema
}

fn candidate_material_surface_quality_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateMaterialSurfaceQualityGetRequest@1"}),
    );
    for key in [
        "material_surface_quality_id",
        "project_id",
        "source_candidate_id",
        "output_candidate_id",
    ] {
        properties.insert(key.to_owned(), topology_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "material_surface_quality_id",
            "project_id",
            "source_candidate_id",
            "output_candidate_id",
        ],
        properties,
    )
}

fn candidate_material_surface_quality_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateMaterialSurfaceQualityPrepareRequest@1"}),
    );
    for key in [
        "material_surface_quality_id",
        "project_id",
        "source_candidate_id",
        "source_artifact_id",
        "source_topology_quality_id",
        "output_candidate_id",
        "output_artifact_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), topology_id_property());
    }
    for key in [
        "source_candidate_state_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "source_artifact_readback_object_sha256",
        "source_geometry_candidate_evidence_sha256",
        "source_geometry_program_sha256",
        "source_topology_quality_report_object_sha256",
        "source_topology_quality_canonical_sha256",
        "output_candidate_state_sha256",
        "output_artifact_sha256",
        "output_artifact_readback_sha256",
        "output_artifact_readback_object_sha256",
        "output_geometry_program_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "material_layer_stack_sha256",
        "material_pack_manifest_object_sha256",
        "material_pack_manifest_sha256",
        "material_pack_provenance_sha256",
        "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256",
        "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256",
        "uv_binding_sha256",
        "tangent_binding_sha256",
        "material_zone_inventory_sha256",
        "material_provenance_sha256",
        "geometry_preservation_projection_sha256",
        "material_surface_quality_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert("lod_scope".to_owned(), json!({"const":"lod0-only@1"}));
    properties.insert(
        "material_surface_quality_policy".to_owned(),
        json!({"const":"candidate-material-surface-structural-hard-gate@1"}),
    );
    properties.insert("from_stage".to_owned(), json!({"const":"topology"}));
    properties.insert("to_stage".to_owned(), json!({"const":"material-surface"}));
    object_schema(
        vec![
            "schema_version",
            "material_surface_quality_id",
            "project_id",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "source_artifact_id",
            "source_artifact_sha256",
            "source_artifact_readback_sha256",
            "source_artifact_readback_object_sha256",
            "source_geometry_candidate_evidence_sha256",
            "source_geometry_program_sha256",
            "source_topology_quality_id",
            "source_topology_quality_report_object_sha256",
            "source_topology_quality_canonical_sha256",
            "output_candidate_id",
            "output_candidate_state_sha256",
            "output_artifact_id",
            "output_artifact_sha256",
            "output_artifact_readback_sha256",
            "output_artifact_readback_object_sha256",
            "output_geometry_program_sha256",
            "appearance_source_lineage_sidecar_object_sha256",
            "appearance_source_lineage_canonical_sha256",
            "appearance_program_object_sha256",
            "appearance_program_sha256",
            "material_layer_stack_sha256",
            "material_pack_manifest_object_sha256",
            "material_pack_manifest_sha256",
            "material_pack_provenance_sha256",
            "texture_build_receipt_object_sha256",
            "texture_build_receipt_canonical_sha256",
            "candidate_surface_bake_receipt_object_sha256",
            "candidate_surface_bake_receipt_canonical_sha256",
            "uv_binding_sha256",
            "tangent_binding_sha256",
            "material_zone_inventory_sha256",
            "material_provenance_sha256",
            "lod_scope",
            "geometry_preservation_projection_sha256",
            "material_surface_quality_policy",
            "material_surface_quality_policy_sha256",
            "from_stage",
            "to_stage",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn candidate_animation_vfx_quality_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateAnimationVfxQualityGetRequest@1"}),
    );
    for key in ["animation_vfx_quality_id", "project_id", "candidate_id"] {
        properties.insert(key.to_owned(), topology_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "animation_vfx_quality_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn candidate_animation_vfx_quality_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateAnimationVfxQualityPrepareRequest@1"}),
    );
    for key in [
        "animation_vfx_quality_id",
        "project_id",
        "source_material_surface_transition_id",
        "source_material_surface_quality_id",
        "candidate_id",
        "artifact_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), topology_id_property());
    }
    for key in [
        "source_material_surface_transition_sha256",
        "source_material_surface_head_canonical_sha256",
        "source_material_surface_quality_report_object_sha256",
        "source_material_surface_quality_canonical_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "derived_animated_socket_artifact_sha256",
        "animated_socket_receipt_object_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "vfx_sequence_key_sha256",
        "vfx_sequence_canonical_sha256",
        "vfx_frame_key_sha256",
        "vfx_frame_canonical_sha256",
        "vfx_bloom_key_sha256",
        "vfx_bloom_canonical_sha256",
        "vfx_particle_key_sha256",
        "vfx_particle_canonical_sha256",
        "vfx_trail_key_sha256",
        "vfx_trail_canonical_sha256",
        "vfx_trail_bloom_key_sha256",
        "vfx_trail_bloom_canonical_sha256",
        "sample_request_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "animation_vfx_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "particle_history_key_sha256s".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":4,
            "uniqueItems":true,
            "items":sha256_property()
        }),
    );
    properties.insert(
        "animation_vfx_scope".to_owned(),
        json!({"const":"lod0-rigid-animation-full-vfx-stack-single-frame@1"}),
    );
    properties.insert(
        "animation_vfx_policy".to_owned(),
        json!({"const":"candidate-animation-vfx-structural-hard-gate@1"}),
    );
    properties.insert("from_stage".to_owned(), json!({"const":"material-surface"}));
    properties.insert("to_stage".to_owned(), json!({"const":"animation-vfx"}));
    object_schema(
        vec![
            "schema_version",
            "animation_vfx_quality_id",
            "project_id",
            "source_material_surface_transition_id",
            "source_material_surface_transition_sha256",
            "source_material_surface_head_canonical_sha256",
            "source_material_surface_quality_id",
            "source_material_surface_quality_report_object_sha256",
            "source_material_surface_quality_canonical_sha256",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "derived_animated_socket_artifact_sha256",
            "animated_socket_receipt_object_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "vfx_sequence_key_sha256",
            "vfx_sequence_canonical_sha256",
            "vfx_frame_key_sha256",
            "vfx_frame_canonical_sha256",
            "vfx_bloom_key_sha256",
            "vfx_bloom_canonical_sha256",
            "vfx_particle_key_sha256",
            "vfx_particle_canonical_sha256",
            "vfx_trail_key_sha256",
            "vfx_trail_canonical_sha256",
            "vfx_trail_bloom_key_sha256",
            "vfx_trail_bloom_canonical_sha256",
            "particle_history_key_sha256s",
            "sample_request_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "animation_vfx_scope",
            "animation_vfx_policy",
            "animation_vfx_policy_sha256",
            "from_stage",
            "to_stage",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

const CANDIDATE_ANIMATION_VFX_QUALITY_V2_PREPARE_FIELDS: [&str; 69] = [
    "schema_version",
    "animation_vfx_quality_id",
    "project_id",
    "source_material_surface_transition_id",
    "source_material_surface_transition_sha256",
    "source_material_surface_head_canonical_sha256",
    "source_material_surface_quality_id",
    "source_material_surface_quality_report_object_sha256",
    "source_material_surface_quality_canonical_sha256",
    "candidate_id",
    "geometry_candidate_id",
    "geometry_candidate_state_sha256",
    "geometry_delivery_manifest_object_sha256",
    "geometry_artifact_sha256",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_delivery_manifest_object_sha256",
    "appearance_artifact_sha256",
    "geometry_preservation_projection_sha256",
    "geometry_preservation_status",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "appearance_anchor_set_object_sha256",
    "appearance_anchor_set_canonical_sha256",
    "anchor_binding_policy",
    "anchor_binding_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "particle_sequence_key_sha256",
    "particle_sequence_canonical_sha256",
    "trail_sequence_key_sha256",
    "trail_sequence_canonical_sha256",
    "trail_bloom_sequence_key_sha256",
    "trail_bloom_sequence_canonical_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "trail_bloom_profile_sha256",
    "socket_node_id_encoding_sha256",
    "socket_roles_sha256",
    "camera_object_sha256",
    "camera_identity_sha256",
    "render_profile_sha256",
    "render_worker_build_cohort_sha256",
    "sample_schedule_sha256",
    "sample_count",
    "sample_time_ticks",
    "attachment_policy",
    "frame_scope",
    "attachment_key_sha256",
    "attachment_canonical_sha256",
    "attachment_receipt_object_sha256",
    "attachment_receipt_canonical_sha256",
    "attachment_frame_count",
    "attachment_frame_set_sha256",
    "animation_vfx_scope",
    "animation_vfx_policy",
    "animation_vfx_policy_sha256",
    "from_stage",
    "to_stage",
    "input_sha256",
    "idempotency_key",
];

const CANDIDATE_ANIMATION_VFX_QUALITY_V2_RECORD_FIELDS: [&str; 91] = [
    "schema_version",
    "animation_vfx_quality_id",
    "project_id",
    "source_material_surface_transition_id",
    "source_material_surface_transition_sha256",
    "source_material_surface_head_canonical_sha256",
    "source_material_surface_quality_id",
    "source_material_surface_quality_report_object_sha256",
    "source_material_surface_quality_canonical_sha256",
    "candidate_id",
    "geometry_candidate_id",
    "geometry_candidate_state_sha256",
    "geometry_delivery_manifest_object_sha256",
    "geometry_artifact_sha256",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "appearance_delivery_manifest_object_sha256",
    "appearance_artifact_sha256",
    "geometry_preservation_projection_sha256",
    "geometry_preservation_status",
    "animated_socket_materialization_key_sha256",
    "animated_artifact_sha256",
    "animated_socket_anchor_set_object_sha256",
    "animated_socket_anchor_set_canonical_sha256",
    "appearance_anchor_set_object_sha256",
    "appearance_anchor_set_canonical_sha256",
    "anchor_binding_policy",
    "anchor_binding_sha256",
    "animation_clip_id",
    "animation_clip_object_sha256",
    "animation_clip_canonical_sha256",
    "animation_receipt_object_sha256",
    "animation_receipt_canonical_sha256",
    "projection_key_sha256",
    "projection_object_sha256",
    "projection_canonical_sha256",
    "particle_sequence_key_sha256",
    "particle_sequence_canonical_sha256",
    "trail_sequence_key_sha256",
    "trail_sequence_canonical_sha256",
    "trail_bloom_sequence_key_sha256",
    "trail_bloom_sequence_canonical_sha256",
    "vfx_profile_object_sha256",
    "vfx_profile_canonical_sha256",
    "trail_bloom_profile_sha256",
    "socket_node_id_encoding_sha256",
    "socket_roles_sha256",
    "camera_object_sha256",
    "camera_identity_sha256",
    "render_profile_sha256",
    "render_worker_build_cohort_sha256",
    "sample_schedule_sha256",
    "sample_count",
    "sample_time_ticks",
    "attachment_policy",
    "frame_scope",
    "attachment_key_sha256",
    "attachment_canonical_sha256",
    "attachment_receipt_object_sha256",
    "attachment_receipt_canonical_sha256",
    "attachment_frame_count",
    "attachment_frame_set_sha256",
    "animation_vfx_scope",
    "animation_vfx_policy",
    "animation_vfx_policy_sha256",
    "from_stage",
    "to_stage",
    "input_sha256",
    "candidate_binding_status",
    "hard_gate",
    "validator_status",
    "hard_gate_passed",
    "animation_status",
    "vfx_status",
    "visual_quality_status",
    "artistic_quality_status",
    "human_review_status",
    "commercial_fps_quality_status",
    "commercial_engine_status",
    "actual_engine_roundtrip",
    "functional_semantics",
    "materialization_status",
    "quality_status",
    "runtime_write_performed",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "request_sha256",
    "canonical_sha256",
    "created_at",
];

const CANDIDATE_ANIMATION_VFX_QUALITY_V2_HARD_GATE_FIELDS: [&str; 20] = [
    "material_surface_head_binding",
    "material_surface_quality",
    "delivery_lod0_binding",
    "anchor_set_binding",
    "animation_clip_binding",
    "animation_glb_readback",
    "animated_socket_readback",
    "vfx_profile_binding",
    "base_frame_stack",
    "bloom_stack",
    "particle_stack",
    "trail_stack",
    "trail_bloom_stack",
    "cross_layer_parent_binding",
    "sample_camera_binding",
    "worker_cohort_binding",
    "render_pass_byte_exact",
    "bounded_resource_policy",
    "vfx_glb_socket_attachment",
    "nonfunctional_scope",
];

fn candidate_animation_vfx_quality_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateAnimationVfxQualityGetRequest@2"}),
    );
    for key in ["animation_vfx_quality_id", "project_id", "candidate_id"] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "animation_vfx_quality_id",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn candidate_animation_vfx_quality_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"CandidateAnimationVfxQualityPrepareRequest@2"}),
    );
    for key in [
        "animation_vfx_quality_id",
        "project_id",
        "source_material_surface_transition_id",
        "source_material_surface_quality_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    let mut geometry_candidate_property = v2_id_property();
    geometry_candidate_property["description"] = Value::String(
        "Distinct from appearance_candidate_id; Runtime enforces the dual-candidate binding."
            .to_owned(),
    );
    properties.insert(
        "geometry_candidate_id".to_owned(),
        geometry_candidate_property,
    );
    let mut appearance_candidate_property = v2_id_property();
    appearance_candidate_property["description"] = Value::String(
        "Distinct from geometry_candidate_id; Runtime enforces candidate_id == appearance_candidate_id."
            .to_owned(),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        appearance_candidate_property,
    );
    for key in [
        "source_material_surface_transition_sha256",
        "source_material_surface_head_canonical_sha256",
        "source_material_surface_quality_report_object_sha256",
        "source_material_surface_quality_canonical_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "attachment_key_sha256",
        "attachment_canonical_sha256",
        "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256",
        "animation_vfx_policy_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "attachment_frame_set_sha256".to_owned(),
        json!({
            "type":"string",
            "pattern":"^[0-9a-f]{64}$",
            "description":"SHA-256 of canonical JSON {schema_version: CandidateAnimationVfxQualityAttachmentFrameSet@1, attachment_key_sha256, frames: [{frame_index, canonical_sha256}]} with exactly fifteen frames ordered by frame_index 0..14."
        }),
    );
    properties.insert(
        "geometry_preservation_status".to_owned(),
        json!({"const":"source-output-renderable-geometry-byte-exact"}),
    );
    properties.insert(
        "anchor_binding_policy".to_owned(),
        json!({"const":"geometry-appearance-anchor-role-owner-trs-equivalent@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"const":15,"type":"integer"}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":15,
            "maxItems":15,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "attachment_policy".to_owned(),
        json!({"const":"projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"}),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"}),
    );
    properties.insert(
        "attachment_frame_count".to_owned(),
        json!({"const":15,"type":"integer"}),
    );
    properties.insert(
        "animation_vfx_scope".to_owned(),
        json!({"const":"lod0-rigid-animation-full-vfx-stack-attachment-v3-all-15-frames@2"}),
    );
    properties.insert(
        "animation_vfx_policy".to_owned(),
        json!({"const":"candidate-animation-vfx-attachment-v3-structural-hard-gate@2"}),
    );
    properties.insert("from_stage".to_owned(), json!({"const":"material-surface"}));
    properties.insert("to_stage".to_owned(), json!({"const":"animation-vfx"}));
    object_schema(
        CANDIDATE_ANIMATION_VFX_QUALITY_V2_PREPARE_FIELDS.to_vec(),
        properties,
    )
}

fn mechanical_animation_clip_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationClipGetRequest@2"}),
    );
    properties.insert(
        "project_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    properties.insert(
        "clip_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
        ],
        properties,
    )
}

fn mechanical_animation_clip_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationClipPrepareRequest@2"}),
    );
    for key in [
        "clip_id",
        "project_id",
        "appearance_candidate_id",
        "appearance_artifact_id",
        "source_geometry_candidate_id",
        "source_geometry_artifact_id",
        "material_surface_quality_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), mechanical_animation_clip_v2_id_property());
    }
    for key in [
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "rest_frame".to_owned(),
        mechanical_animation_clip_v2_rest_frame_schema(),
    );
    properties.insert(
        "pose_action".to_owned(),
        mechanical_animation_clip_v2_pose_action_schema(),
    );
    properties.insert(
        "sampling_policy".to_owned(),
        mechanical_animation_clip_v2_sampling_policy_schema(),
    );
    properties.insert(
        "replay_policy".to_owned(),
        json!({"const":"geometry-plus-appearance-double-worker-replay@1"}),
    );
    object_schema(
        vec![
            "schema_version",
            "clip_id",
            "project_id",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_artifact_id",
            "appearance_artifact_sha256",
            "appearance_artifact_readback_sha256",
            "appearance_artifact_readback_object_sha256",
            "source_geometry_candidate_id",
            "source_geometry_candidate_state_sha256",
            "source_geometry_artifact_id",
            "source_geometry_artifact_sha256",
            "source_geometry_candidate_evidence_sha256",
            "material_surface_quality_id",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "appearance_source_lineage_sidecar_object_sha256",
            "appearance_source_lineage_canonical_sha256",
            "appearance_program_object_sha256",
            "appearance_program_sha256",
            "geometry_program_object_sha256",
            "geometry_program_sha256",
            "geometry_preservation_projection_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "rest_frame",
            "pose_action",
            "sampling_policy",
            "replay_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn mechanical_animation_clip_v2_preview_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationClipPreviewRequest@2"}),
    );
    properties.insert(
        "project_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    properties.insert(
        "clip_id".to_owned(),
        mechanical_animation_clip_v2_id_property(),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    properties.insert(
        "preview_policy".to_owned(),
        json!({"const":"single-tick-transient-geometry-plus-appearance-double-worker-replay@1"}),
    );
    properties.insert("canonical_sha256".to_owned(), sha256_property());
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
            "sample_time_ticks",
            "preview_policy",
            "canonical_sha256",
        ],
        properties,
    )
}

fn mechanical_animation_glb_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationGlbGetRequest@2"}),
    );
    for key in ["project_id", "appearance_candidate_id", "clip_id"] {
        properties.insert(key.to_owned(), mechanical_animation_glb_v2_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
        ],
        properties,
    )
}

fn mechanical_animation_glb_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationGlbPrepareRequest@2"}),
    );
    for key in [
        "project_id",
        "appearance_candidate_id",
        "clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), mechanical_animation_glb_v2_id_property());
    }
    for key in [
        "appearance_candidate_state_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "materialization_policy".to_owned(),
        json!({"const":"appearance-aware-rigid-node-trs-gltf-linear-scheduled-samples@2"}),
    );
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "clip_id",
            "clip_object_sha256",
            "clip_sha256",
            "materialization_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketMaterializationGetRequest@2"}),
    );
    for key in ["project_id", "appearance_candidate_id", "clip_id"] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    properties.insert(
        "animated_socket_materialization_key_sha256".to_owned(),
        sha256_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "clip_id",
            "animated_socket_materialization_key_sha256",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@2"}),
    );
    for key in [
        "project_id",
        "appearance_candidate_id",
        "clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "appearance_candidate_state_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "appearance_delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "materialization_policy".to_owned(),
        json!({"const":"appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2"}),
    );
    object_schema(
        vec![
            "schema_version",
            "project_id",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "clip_id",
            "clip_object_sha256",
            "clip_sha256",
            "appearance_delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "materialization_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn mechanical_animation_clip_v2_rest_frame_schema() -> Value {
    let mut link_properties = Map::new();
    for key in ["link_id", "part_id"] {
        link_properties.insert(key.to_owned(), v2_id_property());
    }
    link_properties.insert(
        "joint_type".to_owned(),
        json!({"enum":["fixed","revolute","prismatic"]}),
    );
    link_properties.insert(
        "value_unit".to_owned(),
        json!({"enum":["none","radian","meter"]}),
    );
    link_properties.insert(
        "source_node_ids".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":16,"uniqueItems":true,"items":v2_id_property()}),
    );
    link_properties.insert(
        "rest_translation_m".to_owned(),
        json!({"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10,"maximum":10}}),
    );
    link_properties.insert(
        "rest_rotation_quat_xyzw".to_owned(),
        json!({"type":"array","minItems":4,"maxItems":4,"items":{"type":"number","minimum":-1,"maximum":1}}),
    );
    link_properties.insert(
        "axis_local".to_owned(),
        json!({"oneOf":[{"type":"null"},{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-1,"maximum":1}}]}),
    );
    link_properties.insert(
        "limit_min".to_owned(),
        json!({"type":["number","null"],"minimum":-3.141592653589793,"maximum":3.141592653589793}),
    );
    link_properties.insert(
        "limit_max".to_owned(),
        json!({"type":["number","null"],"minimum":-3.141592653589793,"maximum":3.141592653589793}),
    );
    let link_schema = object_schema(
        vec![
            "link_id",
            "part_id",
            "source_node_ids",
            "joint_type",
            "rest_translation_m",
            "rest_rotation_quat_xyzw",
            "axis_local",
            "limit_min",
            "limit_max",
            "value_unit",
        ],
        link_properties,
    );
    let mut parent_properties = Map::new();
    parent_properties.insert("child_link_id".to_owned(), v2_id_property());
    parent_properties.insert("parent_link_id".to_owned(), v2_id_property());
    let parent_schema = object_schema(vec!["child_link_id", "parent_link_id"], parent_properties);
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalRestFrame@1"}),
    );
    for key in [
        "rest_frame_id",
        "project_id",
        "candidate_id",
        "root_link_id",
    ] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in [
        "artifact_id",
        "program_sha256",
        "parent_map_sha256",
        "canonical_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert(
        "coordinate_system".to_owned(),
        json!({"const":"forgecad-rh-y-up-m@1"}),
    );
    properties.insert(
        "transform_convention".to_owned(),
        json!({"const":"column-vector-trs-quaternion@1"}),
    );
    properties.insert(
        "links".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":64,"items":link_schema}),
    );
    properties.insert(
        "parent_map".to_owned(),
        json!({"type":"array","minItems":0,"maxItems":63,"items":parent_schema}),
    );
    properties.insert(
        "evaluation_order".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":64,"items":v2_id_property()}),
    );
    object_schema(
        vec![
            "schema_version",
            "rest_frame_id",
            "project_id",
            "artifact_id",
            "candidate_id",
            "program_sha256",
            "coordinate_system",
            "transform_convention",
            "root_link_id",
            "links",
            "parent_map",
            "evaluation_order",
            "parent_map_sha256",
            "canonical_sha256",
        ],
        properties,
    )
}

fn mechanical_animation_clip_v2_pose_action_schema() -> Value {
    let mut key_properties = Map::new();
    key_properties.insert(
        "time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    key_properties.insert(
        "value".to_owned(),
        json!({"type":"number","minimum":-3.141592653589793,"maximum":3.141592653589793}),
    );
    let key_schema = object_schema(vec!["time_ticks", "value"], key_properties);
    let mut channel_properties = Map::new();
    channel_properties.insert("link_id".to_owned(), v2_id_property());
    channel_properties.insert("value_unit".to_owned(), json!({"enum":["radian","meter"]}));
    channel_properties.insert(
        "keys".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":32,"items":key_schema}),
    );
    let channel_schema = object_schema(vec!["link_id", "value_unit", "keys"], channel_properties);
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalPoseAction@1"}),
    );
    for key in ["action_id", "project_id", "candidate_id"] {
        properties.insert(key.to_owned(), v2_id_property());
    }
    for key in ["rest_frame_sha256", "program_sha256", "canonical_sha256"] {
        properties.insert(key.to_owned(), sha256_property());
    }
    properties.insert("timebase_hz".to_owned(), json!({"const":1000}));
    properties.insert(
        "duration_ticks".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":1000000}),
    );
    properties.insert("interpolation".to_owned(), json!({"const":"linear@1"}));
    properties.insert("extrapolation".to_owned(), json!({"const":"clamp@1"}));
    properties.insert("unkeyed_policy".to_owned(), json!({"const":"rest@1"}));
    properties.insert(
        "channels".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":64,"items":channel_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "action_id",
            "project_id",
            "candidate_id",
            "rest_frame_sha256",
            "program_sha256",
            "timebase_hz",
            "duration_ticks",
            "interpolation",
            "extrapolation",
            "unkeyed_policy",
            "channels",
            "canonical_sha256",
        ],
        properties,
    )
}

fn mechanical_animation_clip_v2_sampling_policy_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"MechanicalAnimationSamplingPolicy@1"}),
    );
    properties.insert("timebase_hz".to_owned(), json!({"const":1000}));
    properties.insert(
        "interpolation".to_owned(),
        json!({"const":"scalar-linear-integer-ticks-clamped"}),
    );
    properties.insert("unkeyed".to_owned(), json!({"const":"rest"}));
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":16,"uniqueItems":true,"items":{"type":"integer","minimum":0,"maximum":1000000}}),
    );
    properties.insert("max_samples".to_owned(), json!({"const":16}));
    properties.insert("frame_preview_batch_size".to_owned(), json!({"const":1}));
    object_schema(
        vec![
            "schema_version",
            "timebase_hz",
            "interpolation",
            "unkeyed",
            "sample_time_ticks",
            "max_samples",
            "frame_preview_batch_size",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@1"}),
    );
    properties.insert("attachment_key_sha256".to_owned(), sha256_property());
    properties.insert("project_id".to_owned(), attachment_id_property());
    properties.insert("candidate_id".to_owned(), attachment_id_property());
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@1"}),
    );
    properties.insert("attachment_key_sha256".to_owned(), sha256_property());
    properties.insert("project_id".to_owned(), attachment_id_property());
    properties.insert(
        "delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert("candidate_id".to_owned(), attachment_id_property());
    properties.insert("candidate_state_sha256".to_owned(), sha256_property());
    properties.insert("source_artifact_sha256".to_owned(), sha256_property());
    properties.insert(
        "animated_socket_materialization_key_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "animated_socket_anchor_set_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "animated_socket_anchor_set_canonical_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert("animation_clip_id".to_owned(), attachment_id_property());
    properties.insert("animation_clip_object_sha256".to_owned(), sha256_property());
    properties.insert(
        "animation_clip_canonical_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert("animated_artifact_sha256".to_owned(), sha256_property());
    properties.insert(
        "animation_receipt_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "animation_receipt_canonical_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert("vfx_profile_object_sha256".to_owned(), sha256_property());
    properties.insert("vfx_profile_canonical_sha256".to_owned(), sha256_property());
    properties.insert("vfx_sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "vfx_sequence_canonical_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "attachment_policy".to_owned(),
        json!({"const":"fictional-energy-vfx-animated-socket-attachment-structural-only@1"}),
    );
    properties.insert(
        "socket_node_id_encoding_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert("socket_roles_sha256".to_owned(), sha256_property());
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-vfx-frame-range-1-16@1"}),
    );
    properties.insert("input_sha256".to_owned(), sha256_property());
    properties.insert("idempotency_key".to_owned(), attachment_id_property());
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "delivery_manifest_object_sha256",
            "candidate_id",
            "candidate_state_sha256",
            "source_artifact_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animated_artifact_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "vfx_sequence_key_sha256",
            "vfx_sequence_canonical_sha256",
            "attachment_policy",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "frame_scope",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@2"}),
    );
    properties.insert("attachment_key_sha256".to_owned(), sha256_property());
    properties.insert("project_id".to_owned(), attachment_id_property());
    properties.insert("candidate_id".to_owned(), attachment_id_property());
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@2"}),
    );
    for key in [
        "attachment_key_sha256",
        "delivery_manifest_object_sha256",
        "candidate_state_sha256",
        "source_artifact_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animated_artifact_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), attachment_id_property());
    }
    properties.insert(
        "attachment_policy".to_owned(),
        json!({"const":"fictional-energy-vfx-animated-socket-attachment-projection-bound@2"}),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-vfx-trail-frame-range-1-15@2"}),
    );
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "delivery_manifest_object_sha256",
            "candidate_id",
            "candidate_state_sha256",
            "source_artifact_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animated_artifact_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_canonical_sha256",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_bloom_sequence_key_sha256",
            "trail_bloom_sequence_canonical_sha256",
            "attachment_policy",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "frame_scope",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_v3_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentGetRequest@3"}),
    );
    properties.insert("attachment_key_sha256".to_owned(), sha256_property());
    properties.insert("project_id".to_owned(), attachment_id_property());
    properties.insert("geometry_candidate_id".to_owned(), attachment_id_property());
    properties.insert(
        "appearance_candidate_id".to_owned(),
        attachment_id_property(),
    );
    properties.insert(
        "geometry_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "appearance_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "appearance_candidate_id",
            "geometry_delivery_manifest_object_sha256",
            "appearance_delivery_manifest_object_sha256",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_attachment_v3_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest@3"}),
    );
    for key in [
        "attachment_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "material_surface_quality_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), attachment_id_property());
    }
    properties.insert(
        "geometry_preservation_status".to_owned(),
        json!({"const":"source-output-renderable-geometry-byte-exact"}),
    );
    properties.insert(
        "anchor_binding_policy".to_owned(),
        json!({"const":"geometry-appearance-anchor-role-owner-trs-equivalent@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"const":15,"type":"integer"}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":15,
            "maxItems":15,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "attachment_policy".to_owned(),
        json!({"const":"projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3"}),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3"}),
    );
    object_schema(
        vec![
            "schema_version",
            "attachment_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "geometry_candidate_state_sha256",
            "geometry_delivery_manifest_object_sha256",
            "geometry_artifact_sha256",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "material_surface_quality_id",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "geometry_preservation_projection_sha256",
            "geometry_preservation_status",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "appearance_anchor_set_object_sha256",
            "appearance_anchor_set_canonical_sha256",
            "anchor_binding_policy",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_canonical_sha256",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_bloom_sequence_key_sha256",
            "trail_bloom_sequence_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "trail_bloom_profile_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "attachment_policy",
            "frame_scope",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_transform_projection_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@1"}),
    );
    properties.insert("projection_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_transform_projection_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@1"}),
    );
    for key in [
        "projection_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "derived_animated_socket_receipt_object_sha256",
        "derived_animated_socket_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_node_inventory_sha256",
        "socket_roles_sha256",
        "part_hierarchy_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "socket_roles".to_owned(),
        json!({"const":["weapon-root","grip-primary","muzzle-vfx","magazine-well","sight-primary","energy-core-vfx"]}),
    );
    properties.insert(
        "part_hierarchy_policy".to_owned(),
        json!({"const":"flat-identity-rest-part-hierarchy-only@1"}),
    );
    properties.insert(
        "transform_representation_policy".to_owned(),
        json!({"const":"trs-quaternion-no-matrix-no-shear@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":16}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":16,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-frame-range-1-16@1"}),
    );
    properties.insert("timebase_hz".to_owned(), json!({"const":1000}));
    properties.insert(
        "transform_projection_policy".to_owned(),
        json!({"const":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"}),
    );
    properties.insert(
        "coordinate_system".to_owned(),
        json!({"const":"forgecad-rh-y-up-m@1"}),
    );
    properties.insert(
        "transform_convention".to_owned(),
        json!({"const":"column-vector-parent-world-times-trs-quaternion-xyzw@1"}),
    );
    properties.insert(
        "float_quantization_policy".to_owned(),
        json!({"const":"f32-round-nearest-canonical-json@1"}),
    );
    object_schema(
        vec![
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "delivery_manifest_object_sha256",
            "source_artifact_sha256",
            "source_artifact_readback_sha256",
            "animated_artifact_sha256",
            "animated_artifact_readback_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "derived_animated_socket_artifact_sha256",
            "derived_animated_socket_artifact_readback_sha256",
            "derived_animated_socket_receipt_object_sha256",
            "derived_animated_socket_receipt_canonical_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_node_inventory_sha256",
            "socket_roles_sha256",
            "socket_roles",
            "part_hierarchy_sha256",
            "part_hierarchy_policy",
            "transform_representation_policy",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "timebase_hz",
            "transform_projection_policy",
            "coordinate_system",
            "transform_convention",
            "float_quantization_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_transform_projection_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketTransformProjectionGetRequest@2"}),
    );
    properties.insert("projection_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "animation_clip_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "appearance_candidate_id",
            "animation_clip_id",
        ],
        properties,
    )
}

fn game_weapon_animated_glb_socket_transform_projection_v2_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest@2"}),
    );
    for key in [
        "projection_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "derived_animated_socket_receipt_object_sha256",
        "derived_animated_socket_receipt_canonical_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_node_inventory_sha256",
        "socket_roles_sha256",
        "part_hierarchy_sha256",
        "sampling_policy_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "appearance_candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "socket_roles".to_owned(),
        json!({"const":["weapon-root","grip-primary","muzzle-vfx","magazine-well","sight-primary","energy-core-vfx"]}),
    );
    properties.insert(
        "part_hierarchy_policy".to_owned(),
        json!({"const":"flat-identity-rest-part-hierarchy-only@2"}),
    );
    properties.insert(
        "transform_representation_policy".to_owned(),
        json!({"const":"trs-quaternion-no-shear-plus-column-major-matrix@2"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":16}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":16,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-frame-range-1-16@2"}),
    );
    properties.insert("timebase_hz".to_owned(), json!({"const":1000}));
    properties.insert(
        "transform_projection_policy".to_owned(),
        json!({"const":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"}),
    );
    properties.insert(
        "coordinate_system".to_owned(),
        json!({"const":"forgecad-rh-y-up-m@1"}),
    );
    properties.insert(
        "transform_convention".to_owned(),
        json!({"const":"column-vector-parent-world-times-trs-quaternion-xyzw@1"}),
    );
    properties.insert(
        "float_quantization_policy".to_owned(),
        json!({"const":"f32-round-nearest-canonical-json@1"}),
    );
    object_schema(
        vec![
            "schema_version",
            "projection_key_sha256",
            "project_id",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "appearance_artifact_readback_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_glb_key_sha256",
            "animated_artifact_sha256",
            "animated_artifact_readback_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "derived_animated_socket_artifact_sha256",
            "derived_animated_socket_artifact_readback_sha256",
            "derived_animated_socket_receipt_object_sha256",
            "derived_animated_socket_receipt_canonical_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_node_inventory_sha256",
            "socket_roles_sha256",
            "socket_roles",
            "part_hierarchy_sha256",
            "part_hierarchy_policy",
            "transform_representation_policy",
            "sampling_policy_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "timebase_hz",
            "transform_projection_policy",
            "coordinate_system",
            "transform_convention",
            "float_quantization_policy",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_particles_sequence_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@1"}),
    );
    properties.insert("sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_particles_sequence_prepare_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@1"}),
    );
    for key in [
        "sequence_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":16}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":16,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-particles-frame-range-1-16@1"}),
    );
    properties.insert(
        "particles_sequence_policy".to_owned(),
        json!({"const":"projection-driven-animated-socket-particles@1"}),
    );
    properties.insert(
        "emitter_binding_policy".to_owned(),
        json!({"const":"projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1"}),
    );
    properties.insert(
        "transform_projection_policy".to_owned(),
        json!({"const":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1"}),
    );
    properties.insert(
        "frames".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":16,
            "items":{
                "type":"object",
                "required":[
                    "frame_index",
                    "sample_time_ticks",
                    "projection_frame_canonical_sha256",
                    "projection_socket_transform_inventory_sha256",
                    "projection_socket_transform_readback_sha256",
                    "base_frame_key_sha256",
                    "bloom_key_sha256"
                ],
                "properties":{
                    "frame_index":{"type":"integer","minimum":0,"maximum":15},
                    "sample_time_ticks":{"type":"integer","minimum":0,"maximum":1000000},
                    "projection_frame_canonical_sha256":sha256_property(),
                    "projection_socket_transform_inventory_sha256":sha256_property(),
                    "projection_socket_transform_readback_sha256":sha256_property(),
                    "base_frame_key_sha256":sha256_property(),
                    "bloom_key_sha256":sha256_property()
                },
                "additionalProperties":false
            }
        }),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "delivery_manifest_object_sha256",
            "source_artifact_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "particles_sequence_policy",
            "emitter_binding_policy",
            "transform_projection_policy",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_particles_sequence_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest@2"}),
    );
    properties.insert("sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "geometry_candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "geometry_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "appearance_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "appearance_candidate_id",
            "geometry_delivery_manifest_object_sha256",
            "appearance_delivery_manifest_object_sha256",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare_schema() -> Value {
    let mut frame_properties = Map::new();
    frame_properties.insert(
        "frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":15}),
    );
    frame_properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    for key in [
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
    ] {
        frame_properties.insert(key.to_owned(), sha256_property());
    }
    let frame_schema = object_schema(
        vec![
            "frame_index",
            "sample_time_ticks",
            "projection_frame_canonical_sha256",
            "projection_socket_transform_inventory_sha256",
            "projection_socket_transform_readback_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
        ],
        frame_properties,
    );

    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest@2"}),
    );
    for key in [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "material_surface_quality_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "anchor_binding_policy".to_owned(),
        json!({"const":"geometry-appearance-anchor-role-owner-trs-equivalent@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":16}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":16,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-particles-frame-range-1-16@2"}),
    );
    properties.insert(
        "particles_sequence_policy".to_owned(),
        json!({"const":"projection-v2-driven-animated-socket-particles-dual-candidate@2"}),
    );
    properties.insert(
        "emitter_binding_policy".to_owned(),
        json!({"const":"projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1"}),
    );
    properties.insert(
        "transform_projection_policy".to_owned(),
        json!({"const":"glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2"}),
    );
    properties.insert(
        "frames".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":16,"items":frame_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "geometry_candidate_state_sha256",
            "geometry_delivery_manifest_object_sha256",
            "geometry_artifact_sha256",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "material_surface_quality_id",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "appearance_anchor_set_object_sha256",
            "appearance_anchor_set_canonical_sha256",
            "anchor_binding_policy",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "particles_sequence_policy",
            "emitter_binding_policy",
            "transform_projection_policy",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_sequence_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@2"}),
    );
    properties.insert("sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "geometry_candidate_id".to_owned(),
        json!({
            "type":"string",
            "minLength":1,
            "maxLength":128,
            "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
        }),
    );
    properties.insert(
        "appearance_candidate_id".to_owned(),
        json!({
            "type":"string",
            "minLength":1,
            "maxLength":128,
            "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
        }),
    );
    properties.insert(
        "geometry_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    properties.insert(
        "appearance_delivery_manifest_object_sha256".to_owned(),
        sha256_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "appearance_candidate_id",
            "geometry_delivery_manifest_object_sha256",
            "appearance_delivery_manifest_object_sha256",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare_schema() -> Value {
    let mut frame_properties = Map::new();
    frame_properties.insert(
        "frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":14}),
    );
    frame_properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    frame_properties.insert(
        "history_origin".to_owned(),
        json!({"const":"same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"}),
    );
    for key in [
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ] {
        frame_properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "current_projection_frame_index",
        "current_particle_frame_index",
        "previous_projection_frame_index",
        "previous_particle_frame_index",
    ] {
        frame_properties.insert(
            key.to_owned(),
            json!({"type":"integer","minimum":0,"maximum":15}),
        );
    }
    let frame_schema = object_schema(
        vec![
            "frame_index",
            "sample_time_ticks",
            "history_origin",
            "current_projection_frame_index",
            "current_particle_frame_index",
            "current_particle_key_sha256",
            "current_particle_frame_canonical_sha256",
            "current_projection_frame_canonical_sha256",
            "current_projection_socket_transform_inventory_sha256",
            "current_projection_socket_transform_readback_sha256",
            "previous_projection_frame_index",
            "previous_particle_frame_index",
            "previous_particle_sequence_frame_canonical_sha256",
            "previous_projection_frame_canonical_sha256",
            "previous_projection_socket_transform_inventory_sha256",
            "previous_projection_socket_transform_readback_sha256",
            "particle_sequence_key_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
        ],
        frame_properties,
    );

    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@2"}),
    );
    for key in [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "material_surface_quality_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "anchor_binding_policy".to_owned(),
        json!({"const":"geometry-appearance-anchor-role-owner-trs-equivalent@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":15}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":15,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2"}),
    );
    properties.insert(
        "trails_sequence_policy".to_owned(),
        json!({"const":"projection-v2-driven-animated-socket-trails-dual-candidate@2"}),
    );
    properties.insert(
        "history_policy".to_owned(),
        json!({"const":"particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"}),
    );
    properties.insert(
        "history_pre_roll_policy".to_owned(),
        json!({"const":"same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"}),
    );
    properties.insert(
        "trail_count".to_owned(),
        json!({"type":"integer","const":2}),
    );
    properties.insert(
        "trail_emitter_roles".to_owned(),
        json!({
            "type":"array",
            "minItems":2,
            "maxItems":2,
            "items":{"type":"string"},
            "const":["muzzle-vfx","energy-core-vfx"]
        }),
    );
    properties.insert(
        "frames".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":15,"items":frame_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "geometry_candidate_state_sha256",
            "geometry_delivery_manifest_object_sha256",
            "geometry_artifact_sha256",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "material_surface_quality_id",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "appearance_anchor_set_object_sha256",
            "appearance_anchor_set_canonical_sha256",
            "anchor_binding_policy",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "trails_sequence_policy",
            "history_policy",
            "history_pre_roll_policy",
            "trail_count",
            "trail_emitter_roles",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_sequence_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest@1"}),
    );
    properties.insert("sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_sequence_prepare_schema() -> Value {
    let mut frame_properties = Map::new();
    frame_properties.insert(
        "frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":14}),
    );
    frame_properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    frame_properties.insert(
        "history_origin".to_owned(),
        json!({"const":"same-parent-sequence-source-frame-zero-preroll@1"}),
    );
    for key in [
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ] {
        frame_properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "current_projection_frame_index",
        "current_particle_frame_index",
        "previous_projection_frame_index",
        "previous_particle_frame_index",
    ] {
        frame_properties.insert(
            key.to_owned(),
            json!({"type":"integer","minimum":0,"maximum":15}),
        );
    }
    let frame_required = vec![
        "frame_index",
        "sample_time_ticks",
        "history_origin",
        "current_projection_frame_index",
        "current_particle_frame_index",
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_projection_frame_index",
        "previous_particle_frame_index",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ];
    let frame_schema = object_schema(frame_required, frame_properties);

    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest@1"}),
    );
    for key in [
        "sequence_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":15}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":15,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-trails-source-frames-1-15@1"}),
    );
    properties.insert(
        "trails_sequence_policy".to_owned(),
        json!({"const":"projection-driven-animated-socket-trails@1"}),
    );
    properties.insert(
        "history_policy".to_owned(),
        json!({"const":"one-to-eight-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@1"}),
    );
    properties.insert(
        "history_pre_roll_policy".to_owned(),
        json!({"const":"same-parent-source-frame-zero-is-preroll-output-frames-one-to-fifteen@1"}),
    );
    properties.insert(
        "trail_count".to_owned(),
        json!({"type":"integer","const":2}),
    );
    properties.insert(
        "trail_emitter_roles".to_owned(),
        json!({
            "type":"array",
            "minItems":2,
            "maxItems":2,
            "items":{"type":"string"},
            "const":["muzzle-vfx","energy-core-vfx"]
        }),
    );
    properties.insert(
        "frames".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":15,"items":frame_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "delivery_manifest_object_sha256",
            "source_artifact_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "trails_sequence_policy",
            "history_policy",
            "history_pre_roll_policy",
            "trail_count",
            "trail_emitter_roles",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_bloom_sequence_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@1"}),
    );
    properties.insert("sequence_key_sha256".to_owned(), sha256_property());
    properties.insert(
        "project_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    properties.insert(
        "candidate_id".to_owned(),
        animated_socket_projection_id_property(),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare_schema() -> Value {
    let mut frame_properties = Map::new();
    frame_properties.insert(
        "frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":14}),
    );
    frame_properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    for key in [
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_frame_canonical_sha256",
        "particle_sequence_frame_canonical_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ] {
        frame_properties.insert(key.to_owned(), sha256_property());
    }
    let frame_schema = object_schema(
        vec![
            "frame_index",
            "sample_time_ticks",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_frame_canonical_sha256",
            "particle_sequence_frame_canonical_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
        ],
        frame_properties,
    );
    let trail_bloom_profile = object_schema(
        vec![
            "threshold",
            "source_gain",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "blur_passes",
            "kernel",
        ],
        Map::from_iter([
            ("threshold".to_owned(), json!({"type":"number","const":1})),
            ("source_gain".to_owned(), json!({"type":"number","const":8})),
            ("radius_px".to_owned(), json!({"type":"integer","const":8})),
            ("intensity".to_owned(), json!({"type":"number","const":4})),
            ("hdr_clamp".to_owned(), json!({"type":"number","const":16})),
            (
                "blur_passes".to_owned(),
                json!({"type":"integer","const":2}),
            ),
            (
                "kernel".to_owned(),
                json!({"type":"string","const":"separable-box-two-pass-fixed-radius@1"}),
            ),
        ]),
    );

    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@1"}),
    );
    for key in [
        "sequence_key_sha256",
        "candidate_state_sha256",
        "delivery_manifest_object_sha256",
        "source_artifact_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
        "input_sha256",
    ] {
        properties.insert(key.to_owned(), sha256_property());
    }
    for key in [
        "project_id",
        "candidate_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(key.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":15}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":15,
            "uniqueItems":true,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-trails-bloom-source-frames-1-15@1"}),
    );
    properties.insert(
        "trails_bloom_sequence_policy".to_owned(),
        json!({"const":"projection-driven-animated-socket-trails-bloom@1"}),
    );
    properties.insert(
        "trail_key_scope".to_owned(),
        json!({"const":"animated-socket-trails-sequence-frame-binding@1"}),
    );
    properties.insert(
        "trail_count".to_owned(),
        json!({"type":"integer","const":2}),
    );
    properties.insert(
        "trail_emitter_roles".to_owned(),
        json!({
            "type":"array",
            "minItems":2,
            "maxItems":2,
            "items":{"type":"string"},
            "const":["muzzle-vfx","energy-core-vfx"]
        }),
    );
    properties.insert("trail_bloom_profile".to_owned(), trail_bloom_profile);
    properties.insert(
        "frames".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":15,"items":frame_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "delivery_manifest_object_sha256",
            "source_artifact_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "trails_bloom_sequence_policy",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_key_scope",
            "trail_count",
            "trail_emitter_roles",
            "trail_bloom_profile_sha256",
            "trail_bloom_profile",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest@2"}),
    );
    for field in [
        "sequence_key_sha256",
        "geometry_delivery_manifest_object_sha256",
        "appearance_delivery_manifest_object_sha256",
    ] {
        properties.insert(field.to_owned(), sha256_property());
    }
    for field in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
    ] {
        properties.insert(field.to_owned(), animated_socket_projection_id_property());
    }
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "appearance_candidate_id",
            "geometry_delivery_manifest_object_sha256",
            "appearance_delivery_manifest_object_sha256",
        ],
        properties,
    )
}

fn fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare_schema() -> Value {
    let frame_sha_fields = [
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_frame_canonical_sha256",
        "trail_key_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "emitter_binding_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ];
    let mut frame_properties = Map::new();
    frame_properties.insert(
        "frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":14}),
    );
    frame_properties.insert(
        "sample_time_ticks".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":1000000}),
    );
    frame_properties.insert(
        "trail_frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":14}),
    );
    for field in &frame_sha_fields {
        frame_properties.insert((*field).to_owned(), sha256_property());
    }
    frame_properties.insert(
        "current_projection_frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":15}),
    );
    frame_properties.insert(
        "current_particle_frame_index".to_owned(),
        json!({"type":"integer","minimum":0,"maximum":15}),
    );
    let frame_schema = object_schema(
        [
            [
                "frame_index",
                "sample_time_ticks",
                "trail_frame_index",
                "current_projection_frame_index",
                "current_particle_frame_index",
            ]
            .as_slice(),
            &frame_sha_fields,
        ]
        .concat()
        .to_vec(),
        frame_properties,
    );
    let trail_bloom_profile = object_schema(
        vec![
            "threshold",
            "source_gain",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "blur_passes",
            "kernel",
        ],
        Map::from_iter([
            ("threshold".to_owned(), json!({"type":"number","const":1})),
            ("source_gain".to_owned(), json!({"type":"number","const":8})),
            ("radius_px".to_owned(), json!({"type":"integer","const":8})),
            ("intensity".to_owned(), json!({"type":"number","const":4})),
            ("hdr_clamp".to_owned(), json!({"type":"number","const":16})),
            (
                "blur_passes".to_owned(),
                json!({"type":"integer","const":2}),
            ),
            (
                "kernel".to_owned(),
                json!({"type":"string","const":"separable-box-two-pass-fixed-radius@1"}),
            ),
        ]),
    );

    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":"FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest@2"}),
    );
    for field in [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
        "input_sha256",
    ] {
        properties.insert(field.to_owned(), sha256_property());
    }
    for field in [
        "project_id",
        "geometry_candidate_id",
        "appearance_candidate_id",
        "material_surface_quality_id",
        "animation_clip_id",
        "idempotency_key",
    ] {
        properties.insert(field.to_owned(), animated_socket_projection_id_property());
    }
    properties.insert(
        "anchor_binding_policy".to_owned(),
        json!({"const":"geometry-appearance-anchor-role-owner-trs-equivalent@1"}),
    );
    properties.insert(
        "sample_count".to_owned(),
        json!({"type":"integer","minimum":1,"maximum":15}),
    );
    properties.insert(
        "sample_time_ticks".to_owned(),
        json!({
            "type":"array",
            "minItems":1,
            "maxItems":15,
            "items":{"type":"integer","minimum":0,"maximum":1000000}
        }),
    );
    properties.insert(
        "frame_scope".to_owned(),
        json!({"const":"lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2"}),
    );
    properties.insert(
        "trails_bloom_sequence_policy".to_owned(),
        json!({"const":"projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2"}),
    );
    properties.insert(
        "history_policy".to_owned(),
        json!({"const":"particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2"}),
    );
    properties.insert(
        "history_pre_roll_policy".to_owned(),
        json!({"const":"same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2"}),
    );
    properties.insert(
        "trail_key_scope".to_owned(),
        json!({"const":"animated-socket-trails-sequence-v2-frame-binding@2"}),
    );
    properties.insert(
        "trail_count".to_owned(),
        json!({"type":"integer","const":2}),
    );
    properties.insert(
        "trail_emitter_roles".to_owned(),
        json!({
            "type":"array",
            "const":["muzzle-vfx","energy-core-vfx"],
            "minItems":2,
            "maxItems":2,
            "items":{"type":"string"}
        }),
    );
    properties.insert("trail_bloom_profile".to_owned(), trail_bloom_profile);
    properties.insert(
        "frames".to_owned(),
        json!({"type":"array","minItems":1,"maxItems":15,"items":frame_schema}),
    );
    object_schema(
        vec![
            "schema_version",
            "sequence_key_sha256",
            "project_id",
            "geometry_candidate_id",
            "geometry_candidate_state_sha256",
            "geometry_delivery_manifest_object_sha256",
            "geometry_artifact_sha256",
            "appearance_candidate_id",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "material_surface_quality_id",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "appearance_anchor_set_object_sha256",
            "appearance_anchor_set_canonical_sha256",
            "anchor_binding_policy",
            "animation_clip_id",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "sample_count",
            "sample_time_ticks",
            "frame_scope",
            "trails_bloom_sequence_policy",
            "history_policy",
            "history_pre_roll_policy",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_key_scope",
            "trail_count",
            "trail_emitter_roles",
            "trail_bloom_profile_sha256",
            "trail_bloom_profile",
            "frames",
            "input_sha256",
            "idempotency_key",
        ],
        properties,
    )
}

fn scope_properties() -> Map<String, Value> {
    Map::from_iter([
        ("project_id".to_owned(), id_property()),
        ("candidate_id".to_owned(), id_property()),
    ])
}

fn attachment_id_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._:-]+$"})
}

fn animated_socket_projection_id_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"})
}

fn with_approval(mut properties: Map<String, Value>) -> Map<String, Value> {
    properties.insert("approved".to_owned(), json!({"const": true}));
    properties.insert("approval_receipt_id".to_owned(), id_property());
    properties.insert(
        "approval_summary".to_owned(),
        json!({"type":"string","minLength":1,"maxLength":512}),
    );
    properties.insert(
        "approval_expires_at".to_owned(),
        json!({"type":"string","minLength":1,"maxLength":64}),
    );
    properties
}

fn object_schema(required: Vec<&str>, properties: Map<String, Value>) -> Value {
    json!({
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

fn id_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128})
}

/// V2 production-stage contracts use the same bounded opaque-id pattern as
/// their JSON schemas. Keep this helper separate from the historical
/// length-only id_property so existing MCP tools remain wire-compatible.
fn v2_id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn mechanical_animation_clip_v2_id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"})
}

fn mechanical_animation_glb_v2_id_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
    })
}

fn v2_approval_expires_at_property() -> Value {
    json!({"type":"string","pattern":"^[0-9]{1,10}$"})
}

fn nullable_id_property() -> Value {
    json!({"type":["string","null"],"maxLength":128})
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn topology_id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn sha256_list_property(unique: bool) -> Value {
    json!({
        "type":"array",
        "minItems":1,
        "maxItems":512,
        "uniqueItems":unique,
        "items":sha256_property()
    })
}

fn nullable_sha256_list_property() -> Value {
    json!({
        "type":"array",
        "minItems":1,
        "maxItems":512,
        "items":{"oneOf":[sha256_property(),{"type":"null"}]}
    })
}

fn nullable_sha256_property() -> Value {
    json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"})
}

fn production_stage_property() -> Value {
    json!({"enum":["draft","gray-model","topology","material-surface","animation-vfx","game-delivery"]})
}

fn output_kind_property() -> Value {
    json!({"enum":["gray-model-artifact","topology-quality","appearance-lineage","animation-vfx-bundle","game-asset-delivery"]})
}

fn visual_state_property() -> Value {
    json!({"enum":["pass","fail","unknown"]})
}

fn stage_property() -> Value {
    json!({"enum":["reference-canvas","primary-form","secondary-structure","tertiary-detail","uv-pbr","final-review"]})
}

// FormQuality@2 accepts the historical FormQuality/FormArt join and the
// fresh baseline/proposal join through one closed, opt-in source union.  The
// fields are optional at the wire envelope for old callers, but a declared
// fresh-baseline-proposal mode is validated fail-closed below before it can
// reach Runtime.  Keeping the scopes flat mirrors the durable baseline and
// proposal receipts, making source-vs-evaluated candidate identity explicit.
const FORM_QUALITY_V2_SCOPE_FIELDS: [&str; 28] = [
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_fresh_baseline_id",
    "source_fresh_baseline_canonical_sha256",
    "source_fresh_baseline_receipt_object_sha256",
    "source_registration_lineage_id",
    "source_registration_lineage_canonical_sha256",
    "source_registration_lineage_receipt_object_sha256",
    "source_registered_rig_v2_id",
    "source_registered_rig_v2_object_sha256",
    "source_registered_rig_v2_canonical_sha256",
    "source_runtime_build_cohort_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_sha256",
    "proposal_worker_build_cohort_sha256",
    "cross_view_evidence_bundle_sha256",
    "proposal_form_art_evidence_id",
    "proposal_form_art_evidence_object_sha256",
    "proposal_form_art_evidence_canonical_sha256",
    "proposal_part_id_evidence_sha256",
    "proposal_negative_space_evidence_sha256",
    "proposal_line_flow_evidence_sha256",
    "evidence_source_kind",
];

const FORM_QUALITY_V2_FRESH_REQUIRED_FIELDS: [&str; 27] = [
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_artifact_id",
    "source_artifact_sha256",
    "source_fresh_baseline_id",
    "source_fresh_baseline_canonical_sha256",
    "source_fresh_baseline_receipt_object_sha256",
    "source_registration_lineage_id",
    "source_registration_lineage_canonical_sha256",
    "source_registration_lineage_receipt_object_sha256",
    "source_registered_rig_v2_id",
    "source_registered_rig_v2_object_sha256",
    "source_registered_rig_v2_canonical_sha256",
    "source_runtime_build_cohort_sha256",
    "proposal_candidate_id",
    "proposal_candidate_state_sha256",
    "proposal_artifact_id",
    "proposal_artifact_sha256",
    "proposal_artifact_readback_sha256",
    "proposal_worker_build_cohort_sha256",
    "cross_view_evidence_bundle_sha256",
    "proposal_form_art_evidence_id",
    "proposal_form_art_evidence_object_sha256",
    "proposal_form_art_evidence_canonical_sha256",
    "proposal_part_id_evidence_sha256",
    "proposal_negative_space_evidence_sha256",
    "proposal_line_flow_evidence_sha256",
];

fn production_weapon_form_quality_v2_scope_properties(properties: &mut Map<String, Value>) {
    properties.insert(
        "evidence_source_kind".to_owned(),
        json!({
            "enum":["legacy-source","fresh-baseline-proposal"]
        }),
    );
    for key in FORM_QUALITY_V2_SCOPE_FIELDS {
        if key == "evidence_source_kind" {
            continue;
        }
        let is_id = key.ends_with("_id");
        properties.insert(
            key.to_owned(),
            if is_id {
                nullable_id_property()
            } else {
                nullable_sha256_property()
            },
        );
    }
}

fn production_weapon_form_quality_v2_scope_all_of() -> Value {
    let mut legacy_properties = Map::new();
    for field in FORM_QUALITY_V2_SCOPE_FIELDS {
        if field != "evidence_source_kind" {
            legacy_properties.insert(field.to_owned(), json!({"type":"null"}));
        }
    }
    json!([
        {
            "if":{"properties":{"evidence_source_kind":{"const":"fresh-baseline-proposal"}},"required":["evidence_source_kind"]},
            "then":{"required":FORM_QUALITY_V2_FRESH_REQUIRED_FIELDS}
        },
        {
            "if":{"properties":{"evidence_source_kind":{"const":"legacy-source"}},"required":["evidence_source_kind"]},
            "then":{"properties":Value::Object(legacy_properties)}
        }
    ])
}

fn validate_production_weapon_form_quality_v2_scope_input(arguments: &Value) -> Result<(), String> {
    let object = arguments.as_object().ok_or_else(|| {
        "AGENTIC_INVALID_INPUT: FormQuality@2 request must be an object".to_owned()
    })?;
    let mode = object
        .get("evidence_source_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "AGENTIC_INVALID_INPUT: FormQuality@2 evidence_source_kind is required".to_owned()
        })?;
    if !matches!(mode, "legacy-source" | "fresh-baseline-proposal") {
        return Err(
            "AGENTIC_INVALID_INPUT: FormQuality@2 evidence_source_kind is invalid".to_owned(),
        );
    }
    for field in FORM_QUALITY_V2_SCOPE_FIELDS {
        if field == "evidence_source_kind" {
            continue;
        }
        let Some(value) = object.get(field) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let valid = if field.ends_with("_id") {
            valid_v2_id(value.as_str())
        } else {
            valid_sha256(value.as_str())
        };
        if !valid {
            return Err(format!(
                "AGENTIC_INVALID_INPUT: FormQuality@2 scope field {field} is not a bounded id/hash"
            ));
        }
    }
    if mode == "legacy-source" {
        for field in FORM_QUALITY_V2_SCOPE_FIELDS {
            if field == "evidence_source_kind" {
                continue;
            }
            if object.get(field).is_some_and(|value| !value.is_null()) {
                return Err(format!(
                    "AGENTIC_INVALID_INPUT: legacy-source FormQuality@2 cannot carry {field}"
                ));
            }
        }
        return Ok(());
    }
    for field in FORM_QUALITY_V2_FRESH_REQUIRED_FIELDS {
        let value = object
            .get(field)
            .filter(|value| !value.is_null())
            .ok_or_else(|| {
                format!(
                    "AGENTIC_INVALID_INPUT: fresh-baseline-proposal FormQuality@2 requires {field}"
                )
            })?;
        let valid = if field.ends_with("_id") {
            valid_v2_id(value.as_str())
        } else {
            valid_sha256(value.as_str())
        };
        if !valid {
            return Err(format!(
                "AGENTIC_INVALID_INPUT: fresh-baseline-proposal FormQuality@2 field {field} is invalid"
            ));
        }
    }
    if object.get("source_candidate_id") == object.get("proposal_candidate_id")
        || object.get("source_artifact_id") == object.get("proposal_artifact_id")
    {
        return Err(
            "AGENTIC_INVALID_INPUT: fresh-baseline-proposal source and evaluated proposal candidates must be distinct"
                .to_owned(),
        );
    }
    Ok(())
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticTool::from_name(name) else {
        return Ok(());
    };
    if tool.is_write() && tool.requires_approval() {
        if arguments.get("approved") != Some(&Value::Bool(true)) {
            return Err(
                "AGENTIC_APPROVAL_REQUIRED: approved=true is required for Agentic write tools"
                    .to_owned(),
            );
        }
        for key in ["approval_receipt_id", "approval_summary", "idempotency_key"] {
            if arguments
                .get(key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(format!(
                    "AGENTIC_APPROVAL_REQUIRED: {key} is required for Agentic writes"
                ));
            }
        }
    }
    validate_scope(tool, arguments, binding)?;
    if tool.requires_visual_state() {
        let state = arguments
            .get("visual_state")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "AGENTIC_VISUAL_STATE_REQUIRED: checkpoint prepare requires known visual_state"
                    .to_owned()
            })?;
        if !matches!(state, "pass" | "fail") {
            return Err(
                "AGENTIC_VISUAL_STATE_UNKNOWN: unknown visual state cannot prepare or restore a checkpoint"
                    .to_owned(),
            );
        }
    }
    let has_forbidden_transport_field =
        agentic_write_tools_session::contains_forbidden_transport_field_for_tool(tool, arguments)
            .unwrap_or_else(|| contains_forbidden_transport_field(arguments));
    if has_forbidden_transport_field {
        return Err(
            "AGENTIC_INVALID_INPUT: Agentic tools accept hash-bound typed fields only; raw PNG/GLB bytes, paths and URLs are forbidden"
                .to_owned(),
        );
    }
    if matches!(
        tool,
        AgenticTool::ProductionWeaponFormQualityV2Prepare
            | AgenticTool::ProductionWeaponFormQualityV2Get
            | AgenticTool::ProductionWeaponFormQualityV2PreflightGet
    ) {
        validate_production_weapon_form_quality_v2_scope_input(arguments)?;
    }
    Ok(())
}

fn validate_scope(tool: AgenticTool, arguments: &Value, binding: &Binding) -> Result<(), String> {
    if let Some(result) = agentic_write_tools_session::validate_scope(tool, arguments, binding) {
        return result;
    }
    let project_id = required_string(arguments, "project_id")?;
    let v3_tool = matches!(
        tool,
        AgenticTool::ProductionStageTransitionV3Prepare
            | AgenticTool::ProductionStageTransitionV3Get
    );
    if v3_tool {
        let root_candidate_id = required_string(arguments, "root_candidate_id")?;
        let head_candidate_id = required_string(arguments, "head_candidate_id")?;
        if root_candidate_id != head_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: V3 reference coverage edge requires one same candidate root/head"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionStageTransitionV3Get {
                // A fresh MCP process may perform an exact, read-only V3
                // transition/head lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V3 production-stage write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(root_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V3 project and same-candidate root must remain bound to one design session"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V3 session must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let camera_lock_tool = matches!(
        tool,
        AgenticTool::ProductionCameraLockPrepare | AgenticTool::ProductionCameraLockGet
    );
    if camera_lock_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if tool == AgenticTool::ProductionCameraLockPrepare {
            let session_id = required_string(arguments, "session_id")?;
            let approval_session_id = required_string(arguments, "approval_session_id")?;
            if approval_session_id != session_id {
                return Err("AGENTIC_APPROVAL_SESSION_MISMATCH".to_owned());
            }
            let expiry = required_string(arguments, "approval_expires_at")?;
            if expiry.len() > 10 || !expiry.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("AGENTIC_APPROVAL_EXPIRY_INVALID".to_owned());
            }
        }
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionCameraLockGet {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this camera-lock write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: camera lock must remain bound to the session project/candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: camera lock must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let form_evidence_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormEvidencePrepare
            | AgenticTool::ProductionWeaponFormEvidenceGet
    );
    if form_evidence_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionWeaponFormEvidenceGet {
                // A fresh MCP process may perform an exact, read-only
                // evidence lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this form-evidence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-evidence receipt must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-evidence receipt must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let form_art_evidence_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormArtEvidencePrepare
            | AgenticTool::ProductionWeaponFormArtEvidenceGet
    );
    if form_art_evidence_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionWeaponFormArtEvidenceGet {
                // A fresh MCP process may perform an exact, read-only art
                // evidence lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this form-art-evidence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-art-evidence receipt must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-art-evidence receipt must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if tool == AgenticTool::ProductionWeaponArtDecisionProposalGet {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            // A fresh MCP process may perform this exact, read-only proposal
            // projection lookup after Runtime restart.
            return Ok(());
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: art-decision proposal must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: art-decision proposal must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if tool == AgenticTool::ProductionWeaponAssemblyParameterSinkGet {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            // A fresh MCP process may perform this exact, read-only registry
            // projection lookup after Runtime restart.
            return Ok(());
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: assembly parameter sink registry must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: assembly parameter sink registry must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let form_quality_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormQualityPrepare
            | AgenticTool::ProductionWeaponFormQualityGet
    );
    if form_quality_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionWeaponFormQualityGet {
                // A fresh MCP process may perform an exact, read-only
                // FPS-FORM-04 receipt lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this form-quality write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-quality receipt must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: form-quality receipt must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if tool == AgenticTool::ProductionWeaponFormQualityV2PreflightGet {
        let session_id = required_string(arguments, "session_id")?;
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            // A fresh MCP process may perform this exact, read-only preflight
            // lookup after Runtime restart; no local session write is needed.
            return Ok(());
        }
        if binding.session_id.as_deref() != Some(session_id)
            || binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: FormQuality@2 preflight must remain bound to one design-session session/project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if tool == AgenticTool::ProductionWeaponHighLowBakePreflightGet {
        let session_id = required_string(arguments, "session_id")?;
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            // A fresh MCP process may perform this exact, read-only preflight
            // lookup after Runtime restart; no local session write is needed.
            return Ok(());
        }
        if binding.session_id.as_deref() != Some(session_id)
            || binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: HighLowBake preflight must remain bound to one design-session session/project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let form_quality_v2_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormQualityV2Prepare
            | AgenticTool::ProductionWeaponFormQualityV2Get
    );
    if form_quality_v2_tool {
        let candidate_id = arguments.get("candidate_id").and_then(Value::as_str);
        if tool == AgenticTool::ProductionWeaponFormQualityV2Get && candidate_id.is_none() {
            return Err(
                "AGENTIC_INVALID_INPUT: FormQuality@2 get requires candidate_id scope".to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionWeaponFormQualityV2Get {
                // A fresh MCP process may perform an exact, read-only
                // FormQuality@2 receipt lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this FormQuality@2 write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: FormQuality@2 receipt must remain inside the bound project"
                    .to_owned(),
            );
        }
        if let Some(candidate_id) = candidate_id {
            if binding.candidate_id.as_deref() != Some(candidate_id) {
                return Err(
                    "AGENTIC_SCOPE_MISMATCH: FormQuality@2 get must remain inside the bound candidate"
                        .to_owned(),
                );
            }
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: FormQuality@2 receipt must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let retopology_cage_source_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponRetopologyCageSourcePrepare
            | AgenticTool::ProductionWeaponRetopologyCageSourceGet
    );
    if retopology_cage_source_tool {
        let source_candidate_id = required_string(arguments, "source_candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionWeaponRetopologyCageSourceGet {
                // A fresh MCP process may perform an exact, read-only source
                // bundle lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this retopology/Cage source write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(source_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: retopology/Cage source must remain inside the bound project and source candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let v2_tool = matches!(
        tool,
        AgenticTool::ProductionStageTransitionV2Prepare
            | AgenticTool::ProductionStageTransitionV2Get
    );
    if v2_tool {
        let root_candidate_id = required_string(arguments, "root_candidate_id")?;
        let head_candidate_id = required_string(arguments, "head_candidate_id")?;
        if root_candidate_id == head_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: V2 topology root and material-surface head candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::ProductionStageTransitionV2Get {
                // A fresh MCP process may perform an exact, read-only V2
                // head/transition lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 production-stage write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(root_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 project and topology root candidate must remain bound to one design session"
                    .to_owned(),
            );
        }
        let session_id = required_string(arguments, "session_id")?;
        if binding.session_id.as_deref() != Some(session_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 session must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animation_vfx_v2_tool = matches!(
        tool,
        AgenticTool::CandidateAnimationVfxQualityV2Prepare
            | AgenticTool::CandidateAnimationVfxQualityV2Get
    );
    if animation_vfx_v2_tool {
        if tool == AgenticTool::CandidateAnimationVfxQualityV2Get {
            let _candidate_id = required_string(arguments, "candidate_id")?;
            if !binding.is_bound() {
                // A fresh MCP process may perform an exact, read-only lookup
                // after Runtime restart without recreating the session binding.
                return Ok(());
            }
            if binding.project_id.as_deref() != Some(project_id) {
                return Err(
                    "AGENTIC_SCOPE_MISMATCH: CandidateAnimationVfxQuality@2 get must remain bound to the project of one design session"
                        .to_owned(),
                );
            }
            return Ok(());
        }
        let candidate_id = required_string(arguments, "candidate_id")?;
        let geometry_candidate_id = required_string(arguments, "geometry_candidate_id")?;
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if candidate_id != appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: CandidateAnimationVfxQuality@2 candidate_id must equal appearance_candidate_id"
                    .to_owned(),
            );
        }
        if geometry_candidate_id == appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: CandidateAnimationVfxQuality@2 geometry and appearance candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this CandidateAnimationVfxQuality@2 write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(geometry_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: CandidateAnimationVfxQuality@2 must remain bound to the project and geometry candidate of one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animation_vfx_tool = matches!(
        tool,
        AgenticTool::CandidateAnimationVfxQualityPrepare
            | AgenticTool::CandidateAnimationVfxQualityGet
    );
    if animation_vfx_tool {
        let _candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::CandidateAnimationVfxQualityGet {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animation-vfx quality write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id) {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animation-vfx quality must remain inside the bound design-session project"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animation_clip_v2_tool = matches!(
        tool,
        AgenticTool::MechanicalAnimationClipV2Prepare
            | AgenticTool::MechanicalAnimationClipV2Get
            | AgenticTool::MechanicalAnimationClipV2Preview
    );
    if animation_clip_v2_tool {
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if !binding.is_bound() {
            if tool != AgenticTool::MechanicalAnimationClipV2Prepare {
                // A fresh MCP process may perform an exact, read-only clip
                // lookup or transient preview after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this appearance-aware animation clip write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(appearance_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: appearance-aware animation clip must remain inside the bound design-session project and appearance candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animation_glb_v2_tool = matches!(
        tool,
        AgenticTool::MechanicalAnimationGlbV2Prepare | AgenticTool::MechanicalAnimationGlbV2Get
    );
    if animation_glb_v2_tool {
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::MechanicalAnimationGlbV2Get {
                // A fresh MCP process may perform an exact, read-only GLB
                // lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this appearance-aware animated GLB write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(appearance_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: appearance-aware animated GLB must remain inside the bound design-session project and appearance candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_v2_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare
            | AgenticTool::GameWeaponAnimatedGlbSocketV2Get
    );
    if animated_socket_v2_tool {
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::GameWeaponAnimatedGlbSocketV2Get {
                // A fresh MCP process may perform an exact, read-only V2
                // socket materialization lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this appearance-aware animated socket materialization write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(appearance_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: appearance-aware animated socket materialization must remain inside the bound design-session project and appearance candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_attachment_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet
    );
    if animated_socket_attachment_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet {
                // A fresh MCP process may perform an exact, read-only
                // attachment lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animated-socket attachment write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket attachment must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_attachment_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get
    );
    if animated_socket_attachment_v2_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get {
                // A fresh MCP process may perform an exact, read-only V2
                // attachment lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 animated-socket attachment write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 animated-socket attachment must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_attachment_v3_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get
    );
    if animated_socket_attachment_v3_tool {
        let geometry_candidate_id = required_string(arguments, "geometry_candidate_id")?;
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if geometry_candidate_id == appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: Attachment@3 geometry and appearance candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get {
                // A fresh MCP process may perform an exact, read-only
                // Attachment@3 lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this Attachment@3 write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(geometry_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: Attachment@3 must remain inside the bound design-session project and geometry candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_transform_projection_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare
            | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet
    );
    if animated_socket_transform_projection_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet {
                // A fresh MCP process may perform an exact, read-only
                // projection lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animated GLB socket transform projection write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated GLB socket transform projection must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_transform_projection_v2_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare
            | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get
    );
    if animated_socket_transform_projection_v2_tool {
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 transform projection write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(appearance_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 transform projection must remain inside the bound design-session project and appearance candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_particles_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet
    );
    if animated_socket_particles_sequence_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet {
                // A fresh MCP process may perform an exact, read-only particle
                // sequence lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animated-socket particle sequence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket particle sequence must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_particles_sequence_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get
    );
    if animated_socket_particles_sequence_v2_tool {
        let geometry_candidate_id = required_string(arguments, "geometry_candidate_id")?;
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if geometry_candidate_id == appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: V2 particle geometry and appearance candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 animated-socket particle sequence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(geometry_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 particle geometry candidate must remain bound to the design session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_trails_sequence_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get
    );
    if animated_socket_trails_sequence_v2_tool {
        let geometry_candidate_id = required_string(arguments, "geometry_candidate_id")?;
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if geometry_candidate_id == appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: Trails@2 geometry and appearance candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 animated-socket trails sequence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(geometry_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: Trails@2 geometry candidate must remain bound to the design session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_trails_bloom_sequence_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get
    );
    if animated_socket_trails_bloom_sequence_v2_tool {
        let geometry_candidate_id = required_string(arguments, "geometry_candidate_id")?;
        let appearance_candidate_id = required_string(arguments, "appearance_candidate_id")?;
        if geometry_candidate_id == appearance_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: TrailsBloom@2 geometry and appearance candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this V2 animated-socket TrailsBloom write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(geometry_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: TrailsBloom@2 geometry candidate must remain bound to the design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_trails_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet
    );
    if animated_socket_trails_sequence_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet {
                // A fresh MCP process may perform an exact, read-only trails
                // sequence lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animated-socket trails sequence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket trails sequence must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_trails_bloom_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet
    );
    if animated_socket_trails_bloom_sequence_tool {
        let candidate_id = required_string(arguments, "candidate_id")?;
        if !binding.is_bound() {
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet {
                // A fresh MCP process may perform an exact, read-only trail
                // Bloom sequence lookup after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this animated-socket trails Bloom sequence write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket trails Bloom sequence must remain inside the bound design-session project and candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let material_surface_tool = matches!(
        tool,
        AgenticTool::CandidateMaterialSurfaceQualityPrepare
            | AgenticTool::CandidateMaterialSurfaceQualityGet
    );
    if material_surface_tool {
        let source_candidate_id = required_string(arguments, "source_candidate_id")?;
        let output_candidate_id = required_string(arguments, "output_candidate_id")?;
        if source_candidate_id == output_candidate_id {
            return Err(
                "AGENTIC_INVALID_INPUT: material-surface source and output candidates must be distinct"
                    .to_owned(),
            );
        }
        if !binding.is_bound() {
            if tool == AgenticTool::CandidateMaterialSurfaceQualityGet {
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this material-surface quality write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(source_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: project and topology-source candidate must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let candidate_id = required_string(arguments, "candidate_id")?;
    let topology_tool = matches!(
        tool,
        AgenticTool::CandidateTopologyQualityPrepare | AgenticTool::CandidateTopologyQualityGet
    );
    if topology_tool {
        if !binding.is_bound() {
            if tool == AgenticTool::CandidateTopologyQualityGet {
                // A fresh MCP process may perform an exact, read-only
                // topology readback after Runtime restart.
                return Ok(());
            }
            return Err(
                "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this topology quality write"
                    .to_owned(),
            );
        }
        if binding.project_id.as_deref() != Some(project_id)
            || binding.candidate_id.as_deref() != Some(candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: project and candidate must remain bound to one design session"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if !binding.is_bound() {
        if tool == AgenticTool::ProductionStageTransitionGet {
            // A fresh MCP process after Runtime restart may perform an exact,
            // read-only binding lookup before it has a local session state.
            return Ok(());
        }
        return Err(
            "AGENTIC_SESSION_BINDING_REQUIRED: call session_create_or_resume successfully before this tool"
                .to_owned(),
        );
    }
    let session_id = required_string(arguments, "session_id")?;
    if binding.session_id.as_deref() != Some(session_id)
        || binding.project_id.as_deref() != Some(project_id)
        || binding.candidate_id.as_deref() != Some(candidate_id)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: session, project and candidate must remain bound to one design session"
                .to_owned(),
        );
    }
    Ok(())
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("AGENTIC_INVALID_INPUT: {key} is required"))
}

fn validate_animated_socket_trails_sequence_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare;
    let expected_schema = if is_prepare {
        "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@2"
    } else {
        "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@2"
    };
    let sequence = value
        .get("sequence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Trails@2 response is missing its sequence record"
                .to_owned()
        })?;
    let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
    let response_project = sequence.get("project_id").and_then(Value::as_str);
    let geometry_candidate = sequence
        .get("geometry_candidate_id")
        .and_then(Value::as_str);
    let appearance_candidate = sequence
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let hashes_are_valid = |object: &Map<String, Value>, fields: &[&str]| {
        fields
            .iter()
            .all(|field| valid_sha256(object.get(*field).and_then(Value::as_str)))
    };
    let sequence_hashes = [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "input_sha256",
        "canonical_sha256",
    ];
    let frame_hashes = [
        "current_particle_key_sha256",
        "current_particle_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "previous_particle_sequence_frame_canonical_sha256",
        "previous_projection_frame_canonical_sha256",
        "previous_projection_socket_transform_inventory_sha256",
        "previous_projection_socket_transform_readback_sha256",
        "projection_sample_set_sha256",
        "particle_sequence_key_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "trail_key_sha256",
        "trail_seed_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "emitter_binding_sha256",
        "trail_color_object_sha256",
        "trail_id_object_sha256",
        "trail_depth_object_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ];
    let history_ok = |sample: &Value| {
        sample
            .get("history_ordinal")
            .and_then(Value::as_u64)
            .is_some_and(|ordinal| ordinal <= 7)
            && hashes_are_valid(
                sample.as_object().unwrap_or(&Map::new()),
                &[
                    "projection_key_sha256",
                    "projection_frame_canonical_sha256",
                    "projection_socket_transform_inventory_sha256",
                    "projection_socket_transform_readback_sha256",
                    "particle_sequence_key_sha256",
                    "particle_key_sha256",
                    "particle_frame_canonical_sha256",
                ],
            )
            && sample
                .get("projection_frame_index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index <= 15)
            && sample
                .get("particle_frame_index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index <= 15)
            && sample
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks <= 1_000_000)
    };
    let point_ok = |point: &Value| {
        let Some(point) = point.as_object() else {
            return false;
        };
        hashes_are_valid(point, &["source_particle_key_sha256"])
            && point
                .get("source_frame_index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index <= 15)
            && point
                .get("source_particle_frame_index")
                .and_then(Value::as_u64)
                .is_some_and(|index| index <= 15)
            && point
                .get("source_particle_id")
                .and_then(Value::as_u64)
                .is_some_and(|id| id == 10_000 || id == 20_000)
            && point
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks <= 1_000_000)
            && point
                .get("depth_micrometers")
                .and_then(Value::as_u64)
                .is_some()
            && ["local_offset_micrometers", "world_position_micrometers"]
                .iter()
                .all(|field| {
                    point
                        .get(*field)
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.len() == 3 && values.iter().all(|value| value.as_i64().is_some())
                        })
                })
    };
    let frame_ok = |(index, frame): (usize, &Value)| {
        let Some(frame) = frame.as_object() else {
            return false;
        };
        let trails_ok = frame
            .get("trails")
            .and_then(Value::as_array)
            .is_some_and(|trails| {
                trails.len() == 2
                    && trails.iter().enumerate().all(|(trail_index, trail)| {
                        let role = trail.get("emitter_role").and_then(Value::as_str);
                        let expected_role = if trail_index == 0 {
                            "muzzle-vfx"
                        } else {
                            "energy-core-vfx"
                        };
                        let expected_id = if trail_index == 0 { 10_000 } else { 20_000 };
                        role == Some(expected_role)
                            && trail.get("trail_id").and_then(Value::as_u64) == Some(expected_id)
                            && trail
                                .get("points")
                                .and_then(Value::as_array)
                                .is_some_and(|points| {
                                    (2..=9).contains(&points.len()) && points.iter().all(point_ok)
                                })
                    })
            });
        frame.get("schema_version").and_then(Value::as_str)
            == Some("FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame@2")
            && frame.get("frame_index") == Some(&Value::from(index as u64))
            && frame
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks <= 1_000_000)
            && frame.get("history_origin").and_then(Value::as_str)
                == Some(
                    "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2",
                )
            && frame.get("current_projection_frame_index") == Some(&Value::from(index as u64 + 1))
            && frame.get("current_particle_frame_index") == Some(&Value::from(index as u64 + 1))
            && frame.get("previous_projection_frame_index") == Some(&Value::from(index as u64))
            && frame.get("previous_particle_frame_index") == Some(&Value::from(index as u64))
            && hashes_are_valid(frame, &frame_hashes)
            && frame
                .get("history_samples")
                .and_then(Value::as_array)
                .is_some_and(|samples| {
                    (1..=8).contains(&samples.len()) && samples.iter().all(history_ok)
                })
            && frame.get("trail_count") == Some(&Value::from(2_u64))
            && frame.get("trail_emitter_roles") == Some(&json!(["muzzle-vfx", "energy-core-vfx"]))
            && trails_ok
            && frame
                .get("created_at")
                .and_then(Value::as_str)
                .is_some_and(|created_at| !created_at.is_empty())
    };
    let frames = sequence.get("frames").and_then(Value::as_array);
    let frames_ok = frames.is_some_and(|frames| {
        (1..=15).contains(&frames.len())
            && frames.len()
                == sequence
                    .get("sample_count")
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as usize
            && frames.iter().enumerate().all(frame_ok)
    });
    let schedule_ok = sequence
        .get("sample_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| (1..=15).contains(&(count as usize)))
        && sequence
            .get("sample_time_ticks")
            .and_then(Value::as_array)
            .is_some_and(|ticks| {
                ticks.len()
                    == sequence
                        .get("sample_count")
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as usize
                    && ticks
                        .iter()
                        .all(|tick| tick.as_u64().is_some_and(|tick| tick <= 1_000_000))
            });
    let top_flags_safe = [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]
    .iter()
    .all(|field| value.get(*field) == Some(&Value::Bool(false)));
    let sequence_flags_safe = sequence.get("runtime_write_performed") == Some(&Value::Bool(true))
        && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
        && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
        && sequence.get("version_created") == Some(&Value::Bool(false))
        && sequence.get("export_performed") == Some(&Value::Bool(false))
        && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
        && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
        || !valid_sha256(value.get("sequence_key_sha256").and_then(Value::as_str))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value.get("commercial_fps_quality_status").and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
        || !top_flags_safe
        || geometry_candidate.is_none_or(str::is_empty)
        || appearance_candidate.is_none_or(str::is_empty)
        || geometry_candidate == appearance_candidate
        || !valid_sha256(sequence_key)
        || !hashes_are_valid(sequence, &sequence_hashes)
        || !frames_ok
        || !schedule_ok
        || sequence.get("schema_version").and_then(Value::as_str)
            != Some("FictionalEnergyVfxAnimatedSocketTrailsSequence@2")
        || sequence.get("sequence_status").and_then(Value::as_str)
            != Some("runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-sequence-v2")
        || sequence.get("geometry_preservation_status").and_then(Value::as_str)
            != Some("source-output-renderable-geometry-byte-exact")
        || sequence.get("anchor_binding_policy").and_then(Value::as_str)
            != Some("geometry-appearance-anchor-role-owner-trs-equivalent@1")
        || sequence.get("frame_scope").and_then(Value::as_str)
            != Some("lod0-animation-trails-v2-source-frames-1-15-with-particles-v2-frame-zero-preroll@2")
        || sequence.get("trails_sequence_policy").and_then(Value::as_str)
            != Some("projection-v2-driven-animated-socket-trails-dual-candidate@2")
        || sequence.get("history_policy").and_then(Value::as_str)
            != Some("particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2")
        || sequence.get("history_pre_roll_policy").and_then(Value::as_str)
            != Some("same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2")
        || sequence.get("trail_count") != Some(&Value::from(2_u64))
        || sequence.get("trail_emitter_roles")
            != Some(&json!(["muzzle-vfx", "energy-core-vfx"]))
        || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || sequence.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || sequence.get("commercial_fps_quality_status").and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || sequence.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || !sequence_flags_safe
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Trails@2 schema, dual-candidate bindings, bounded history/trail hashes, side-effect flags or media boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.project_id.as_deref() != response_project
            || binding.candidate_id.as_deref() != geometry_candidate)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: Trails@2 response crossed the bound project/geometry candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_animated_socket_trails_bloom_sequence_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare =
        tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare;
    let expected_schema = if is_prepare {
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@2"
    } else {
        "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@2"
    };
    let sequence = value
        .get("sequence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: TrailsBloom@2 response is missing its sequence record"
                .to_owned()
        })?;
    let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
    let response_project = sequence.get("project_id").and_then(Value::as_str);
    let geometry_candidate = sequence
        .get("geometry_candidate_id")
        .and_then(Value::as_str);
    let appearance_candidate = sequence
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let hash_is_valid = |hash: Option<&str>| valid_sha256(hash);
    let parent_hashes = [
        "sequence_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_profile_sha256",
        "input_sha256",
        "canonical_sha256",
    ];
    let parent_hashes_are_valid = parent_hashes
        .iter()
        .all(|field| hash_is_valid(sequence.get(*field).and_then(Value::as_str)));
    let frames = sequence.get("frames").and_then(Value::as_array);
    let frame_hashes = [
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_frame_canonical_sha256",
        "trail_key_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "emitter_binding_sha256",
        "trail_color_object_sha256",
        "trail_id_object_sha256",
        "trail_depth_object_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_frame_canonical_sha256",
        "current_projection_frame_canonical_sha256",
        "current_projection_socket_transform_inventory_sha256",
        "current_projection_socket_transform_readback_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "trail_bloom_profile_sha256",
        "base_opaque_depth_object_sha256",
        "trail_bloom_key_sha256",
        "trail_bloom_seed_sha256",
        "trail_emissive_source_object_sha256",
        "trail_bloom_contribution_object_sha256",
        "render_set_object_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ];
    let frames_are_safe = frames.is_some_and(|items| {
        items.len() == 15
            && items.iter().enumerate().all(|(index, frame)| {
                frame.get("schema_version").and_then(Value::as_str)
                    == Some("FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame@2")
                    && frame.get("frame_index") == Some(&Value::from(index as u64))
                    && frame.get("trail_frame_index") == Some(&Value::from(index as u64))
                    && frame.get("current_projection_frame_index")
                        == Some(&Value::from(index as u64 + 1))
                    && frame.get("current_particle_frame_index")
                        == Some(&Value::from(index as u64 + 1))
                    && frame_hashes
                        .iter()
                        .all(|field| hash_is_valid(frame.get(*field).and_then(Value::as_str)))
                    && frame.get("base_aov_byte_exact_verified") == Some(&Value::Bool(true))
                    && frame.get("base_opaque_depth_byte_exact_reused") == Some(&Value::Bool(true))
                    && frame.get("bloom_pass_byte_exact_reused") == Some(&Value::Bool(true))
                    && frame.get("particle_passes_byte_exact_reused") == Some(&Value::Bool(true))
                    && frame.get("trail_passes_byte_exact_reused") == Some(&Value::Bool(true))
                    && frame.get("base_bloom_mutated") == Some(&Value::Bool(false))
                    && frame.get("particle_passes_mutated") == Some(&Value::Bool(false))
                    && frame.get("trail_passes_mutated") == Some(&Value::Bool(false))
                    && frame.get("trail_bloom_input") == Some(&Value::Bool(true))
                    && frame.get("trail_emissive_source_rendered") == Some(&Value::Bool(true))
                    && frame.get("trail_bloom_contribution_rendered") == Some(&Value::Bool(true))
                    && frame.get("trail_bloom_rendered") == Some(&Value::Bool(true))
                    && frame
                        .get("trail_bloom_contributions")
                        .and_then(Value::as_array)
                        .is_some_and(|items| items.len() == 2)
            })
    });
    let flags_are_safe = [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]
    .iter()
    .all(|field| value.get(*field) == Some(&Value::Bool(false)));
    let sequence_flags_are_safe = sequence.get("runtime_write_performed")
        == Some(&Value::Bool(true))
        && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
        && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
        && sequence.get("version_created") == Some(&Value::Bool(false))
        && sequence.get("export_performed") == Some(&Value::Bool(false))
        && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
        && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
    let profile_is_fixed = sequence.get("trail_bloom_profile")
        == Some(&json!({
            "threshold":1,
            "source_gain":8,
            "radius_px":8,
            "intensity":4,
            "hdr_clamp":16,
            "blur_passes":2,
            "kernel":"separable-box-two-pass-fixed-radius@1"
        }));
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
        || !hash_is_valid(sequence_key)
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || value.get("runtime_write")
            != Some(&Value::Bool(
                is_prepare,
            ))
        || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value.get("commercial_fps_quality_status").and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
        || !flags_are_safe
        || !parent_hashes_are_valid
        || !frames_are_safe
        || response_project.is_none()
        || geometry_candidate.is_none()
        || appearance_candidate.is_none()
        || geometry_candidate == appearance_candidate
        || sequence.get("schema_version").and_then(Value::as_str)
            != Some("FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@2")
        || sequence.get("sequence_status").and_then(Value::as_str)
            != Some("runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence-v2")
        || sequence.get("frame_scope").and_then(Value::as_str) != Some(
            "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2",
        )
        || sequence.get("trails_bloom_sequence_policy").and_then(Value::as_str)
            != Some("projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2")
        || sequence.get("history_policy").and_then(Value::as_str)
            != Some("particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2")
        || sequence.get("history_pre_roll_policy").and_then(Value::as_str)
            != Some("same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2")
        || sequence.get("trail_key_scope").and_then(Value::as_str)
            != Some("animated-socket-trails-sequence-v2-frame-binding@2")
        || sequence.get("trail_count") != Some(&Value::from(2_u64))
        || sequence.get("trail_emitter_roles")
            != Some(&json!(["muzzle-vfx", "energy-core-vfx"]))
        || !profile_is_fixed
        || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || sequence.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || sequence.get("commercial_fps_quality_status").and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || sequence.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || !sequence_flags_are_safe
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: TrailsBloom@2 schema, dual lineage, exact fifteen frame hashes, structural status or media boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.project_id.as_deref() != response_project
            || binding.candidate_id.as_deref() != geometry_candidate)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: TrailsBloom@2 response crossed the bound project/geometry candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_animated_socket_attachment_v3_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare;
    let expected_schema = if is_prepare {
        "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@3"
    } else {
        "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@3"
    };
    let attachment = value
        .get("attachment")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Attachment@3 response is missing its attachment record"
                .to_owned()
        })?;
    let attachment_key = attachment
        .get("attachment_key_sha256")
        .and_then(Value::as_str);
    let response_project = attachment.get("project_id").and_then(Value::as_str);
    let geometry_candidate = attachment
        .get("geometry_candidate_id")
        .and_then(Value::as_str);
    let appearance_candidate = attachment
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let hash_fields = [
        "attachment_key_sha256",
        "geometry_candidate_state_sha256",
        "geometry_delivery_manifest_object_sha256",
        "geometry_artifact_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "geometry_preservation_projection_sha256",
        "animated_socket_materialization_key_sha256",
        "animated_artifact_sha256",
        "animated_socket_anchor_set_object_sha256",
        "animated_socket_anchor_set_canonical_sha256",
        "appearance_anchor_set_object_sha256",
        "appearance_anchor_set_canonical_sha256",
        "anchor_binding_sha256",
        "animation_clip_object_sha256",
        "animation_clip_canonical_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "projection_key_sha256",
        "projection_object_sha256",
        "projection_canonical_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_canonical_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_canonical_sha256",
        "vfx_profile_object_sha256",
        "vfx_profile_canonical_sha256",
        "trail_bloom_profile_sha256",
        "socket_node_id_encoding_sha256",
        "socket_roles_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "sample_schedule_sha256",
        "attachment_receipt_object_sha256",
        "attachment_receipt_canonical_sha256",
        "input_sha256",
        "canonical_sha256",
    ];
    let hashes_are_valid = hash_fields
        .iter()
        .all(|field| valid_sha256(attachment.get(*field).and_then(Value::as_str)));
    let parent_key_matches = |field: &str, expected: Option<&str>| {
        attachment.get(field).and_then(Value::as_str) == expected
    };
    let ticks = attachment
        .get("sample_time_ticks")
        .and_then(Value::as_array);
    let schedule_is_valid = ticks.is_some_and(|ticks| {
        ticks.len() == 15
            && ticks
                .iter()
                .all(|tick| tick.as_u64().is_some_and(|tick| tick <= 1_000_000))
            && ticks.windows(2).all(|pair| {
                pair[0].as_u64().unwrap_or_default() < pair[1].as_u64().unwrap_or_default()
            })
    });
    let frame_hashes = [
        "projection_frame_canonical_sha256",
        "projection_socket_transform_inventory_sha256",
        "projection_socket_transform_readback_sha256",
        "particle_sequence_key_sha256",
        "particle_sequence_frame_canonical_sha256",
        "trail_sequence_key_sha256",
        "trail_sequence_frame_canonical_sha256",
        "trail_key_sha256",
        "trail_inventory_sha256",
        "trail_id_encoding_sha256",
        "emitter_binding_sha256",
        "trail_bloom_sequence_key_sha256",
        "trail_bloom_sequence_frame_canonical_sha256",
        "trail_bloom_key_sha256",
        "trail_bloom_seed_sha256",
        "base_frame_key_sha256",
        "bloom_key_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
        "canonical_sha256",
    ];
    let frames_are_valid = attachment
        .get("frames")
        .and_then(Value::as_array)
        .is_some_and(|frames| {
            frames.len() == 15
                && frames.iter().enumerate().all(|(index, frame)| {
                    let tick_matches = ticks
                        .and_then(|ticks| ticks.get(index))
                        .and_then(Value::as_u64)
                        == frame.get("sample_time_ticks").and_then(Value::as_u64);
                    let frame_key_matches =
                        frame.get("attachment_key_sha256").and_then(Value::as_str)
                            == attachment_key;
                    frame.get("schema_version").and_then(Value::as_str)
                        == Some("FictionalEnergyVfxAnimatedSocketAttachmentFrame@3")
                        && frame_key_matches
                        && frame.get("frame_index") == Some(&Value::from(index as u64))
                        && frame.get("projection_frame_index").and_then(Value::as_u64)
                            == Some(index as u64 + 1)
                        && frame
                            .get("particle_sequence_frame_index")
                            .and_then(Value::as_u64)
                            == Some(index as u64 + 1)
                        && frame.get("trail_frame_index") == Some(&Value::from(index as u64))
                        && frame.get("trail_bloom_frame_index") == Some(&Value::from(index as u64))
                        && frame
                            .get("sample_time_ticks")
                            .and_then(Value::as_u64)
                            .is_some_and(|tick| tick <= 1_000_000)
                        && tick_matches
                        && frame_hashes
                            .iter()
                            .all(|field| valid_sha256(frame.get(*field).and_then(Value::as_str)))
                        && frame
                            .get("particle_sequence_key_sha256")
                            .and_then(Value::as_str)
                            == attachment
                                .get("particle_sequence_key_sha256")
                                .and_then(Value::as_str)
                        && frame
                            .get("trail_sequence_key_sha256")
                            .and_then(Value::as_str)
                            == attachment
                                .get("trail_sequence_key_sha256")
                                .and_then(Value::as_str)
                        && frame
                            .get("trail_bloom_sequence_key_sha256")
                            .and_then(Value::as_str)
                            == attachment
                                .get("trail_bloom_sequence_key_sha256")
                                .and_then(Value::as_str)
                        && frame.get("camera_object_sha256").and_then(Value::as_str)
                            == attachment
                                .get("camera_object_sha256")
                                .and_then(Value::as_str)
                        && frame.get("camera_identity_sha256").and_then(Value::as_str)
                            == attachment
                                .get("camera_identity_sha256")
                                .and_then(Value::as_str)
                        && frame.get("render_profile_sha256").and_then(Value::as_str)
                            == attachment
                                .get("render_profile_sha256")
                                .and_then(Value::as_str)
                        && frame
                            .get("render_worker_build_cohort_sha256")
                            .and_then(Value::as_str)
                            == attachment
                                .get("render_worker_build_cohort_sha256")
                                .and_then(Value::as_str)
                        && frame
                            .get("created_at")
                            .and_then(Value::as_str)
                            .is_some_and(|created_at| !created_at.is_empty())
                })
        });
    let top_flags_are_safe = [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]
    .iter()
    .all(|field| value.get(*field) == Some(&Value::Bool(false)));
    let attachment_flags_are_safe = attachment.get("runtime_write_performed")
        == Some(&Value::Bool(true))
        && attachment.get("restart_hash_verified") == Some(&Value::Bool(true))
        && attachment.get("candidate_confirmed") == Some(&Value::Bool(false))
        && attachment.get("version_created") == Some(&Value::Bool(false))
        && attachment.get("export_performed") == Some(&Value::Bool(false))
        && attachment.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
        && attachment.get("production_stage_advanced") == Some(&Value::Bool(false));
    let ids_are_valid = [
        response_project,
        geometry_candidate,
        appearance_candidate,
        attachment
            .get("material_surface_quality_id")
            .and_then(Value::as_str),
        attachment.get("animation_clip_id").and_then(Value::as_str),
    ]
    .into_iter()
    .all(|value| valid_mechanical_animation_glb_id(value));
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || !valid_sha256(value.get("attachment_key_sha256").and_then(Value::as_str))
        || value.get("attachment_key_sha256").and_then(Value::as_str) != attachment_key
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            != Some("NOT_RUN")
        || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
        || !top_flags_are_safe
        || !ids_are_valid
        || geometry_candidate.is_none()
        || appearance_candidate.is_none()
        || geometry_candidate == appearance_candidate
        || response_project.is_none()
        || !hashes_are_valid
        || !schedule_is_valid
        || attachment.get("schema_version").and_then(Value::as_str)
            != Some("FictionalEnergyVfxAnimatedSocketAttachment@3")
        || attachment.get("attachment_policy").and_then(Value::as_str)
            != Some("projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3")
        || attachment.get("frame_scope").and_then(Value::as_str)
            != Some("lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3")
        || attachment.get("attachment_status").and_then(Value::as_str)
            != Some("runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v3")
        || attachment.get("geometry_preservation_status").and_then(Value::as_str)
            != Some("source-output-renderable-geometry-byte-exact")
        || attachment.get("anchor_binding_policy").and_then(Value::as_str)
            != Some("geometry-appearance-anchor-role-owner-trs-equivalent@1")
        || attachment.get("sample_count") != Some(&Value::from(15_u64))
        || attachment.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || attachment.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || attachment
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            != Some("NOT_PROVEN")
        || attachment.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || attachment.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || !attachment_flags_are_safe
        || !frames_are_valid
        || !parent_key_matches("attachment_key_sha256", attachment_key)
        || attachment
            .get("created_at")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Attachment@3 schema, dual-candidate bindings, exact fifteen frame hashes, structural status or media boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.project_id.as_deref() != response_project
            || binding.candidate_id.as_deref() != geometry_candidate)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: Attachment@3 response crossed the bound project/geometry candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn object_has_exact_fields(object: &Map<String, Value>, fields: &[&str]) -> bool {
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn candidate_animation_vfx_quality_v2_input_sha256(quality: &Map<String, Value>) -> Option<String> {
    let mut preimage = Map::new();
    for field in CANDIDATE_ANIMATION_VFX_QUALITY_V2_PREPARE_FIELDS {
        if matches!(field, "input_sha256" | "idempotency_key") {
            continue;
        }
        preimage.insert(field.to_owned(), quality.get(field)?.clone());
    }
    Some(forgecad_runtime::canonical_json_hash(&Value::Object(
        preimage,
    )))
}

fn candidate_animation_vfx_quality_v2_canonical_sha256(
    quality: &Map<String, Value>,
) -> Option<String> {
    let mut preimage = quality.clone();
    preimage.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    Some(forgecad_runtime::canonical_json_hash(&Value::Object(
        preimage,
    )))
}

fn validate_candidate_animation_vfx_quality_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const TOP_LEVEL_FIELDS: [&str; 8] = [
        "schema_version",
        "animation_vfx_quality",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 result is not an object"
            .to_owned()
    })?;
    let is_prepare = tool == AgenticTool::CandidateAnimationVfxQualityV2Prepare;
    let expected_schema = if is_prepare {
        "CandidateAnimationVfxQualityPrepareResult@2"
    } else {
        "CandidateAnimationVfxQualityGetResult@2"
    };
    if !object_has_exact_fields(object, &TOP_LEVEL_FIELDS)
        || object.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || object.get("replayed").and_then(Value::as_bool).is_none()
        || object.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .any(|field| object.get(*field) != Some(&Value::Bool(false)))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 result schema or side-effect boundary differs"
                .to_owned(),
        );
    }
    let quality = object
        .get("animation_vfx_quality")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 result is missing its record"
                .to_owned()
        })?;
    if !object_has_exact_fields(quality, &CANDIDATE_ANIMATION_VFX_QUALITY_V2_RECORD_FIELDS) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 record is not closed"
                .to_owned(),
        );
    }
    let geometry_candidate = quality.get("geometry_candidate_id").and_then(Value::as_str);
    let appearance_candidate = quality
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let response_candidate = quality.get("candidate_id").and_then(Value::as_str);
    let response_project = quality.get("project_id").and_then(Value::as_str);
    let durable_ids_are_valid = [
        "animation_vfx_quality_id",
        "source_material_surface_transition_id",
        "source_material_surface_quality_id",
        "animation_clip_id",
    ]
    .iter()
    .all(|field| valid_v2_id(quality.get(*field).and_then(Value::as_str)));
    if !valid_v2_id(response_candidate)
        || !valid_v2_id(geometry_candidate)
        || !valid_v2_id(appearance_candidate)
        || response_candidate != appearance_candidate
        || geometry_candidate == appearance_candidate
        || !valid_v2_id(response_project)
        || !durable_ids_are_valid
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 dual-candidate or project binding differs"
                .to_owned(),
        );
    }
    let hard_gate = quality
        .get("hard_gate")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 result is missing hard_gate"
                .to_owned()
        })?;
    if !object_has_exact_fields(
        hard_gate,
        &CANDIDATE_ANIMATION_VFX_QUALITY_V2_HARD_GATE_FIELDS,
    ) || CANDIDATE_ANIMATION_VFX_QUALITY_V2_HARD_GATE_FIELDS
        .iter()
        .any(|field| hard_gate.get(*field) != Some(&Value::Bool(true)))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 hard_gate is not fully derived and passed"
                .to_owned(),
        );
    }
    let hash_fields_are_valid = quality
        .iter()
        .all(|(field, value)| !field.ends_with("_sha256") || valid_sha256(value.as_str()));
    let input_sha256_is_bound = candidate_animation_vfx_quality_v2_input_sha256(quality)
        .zip(quality.get("input_sha256").and_then(Value::as_str))
        .is_some_and(|(expected, actual)| expected == actual)
        && quality.get("request_sha256").and_then(Value::as_str)
            == quality.get("input_sha256").and_then(Value::as_str);
    let canonical_sha256_is_bound = candidate_animation_vfx_quality_v2_canonical_sha256(quality)
        .zip(quality.get("canonical_sha256").and_then(Value::as_str))
        .is_some_and(|(expected, actual)| expected == actual);
    let ticks_are_valid = quality
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .is_some_and(|ticks| {
            ticks.len() == 15
                && ticks
                    .iter()
                    .all(|tick| tick.as_u64().is_some_and(|tick| tick <= 1_000_000))
                && ticks
                    .windows(2)
                    .all(|pair| pair[0].as_u64().unwrap_or(0) < pair[1].as_u64().unwrap_or(0))
        });
    let expected_animation_vfx_policy_sha256 = forgecad_runtime::sha256_hex(
        b"candidate-animation-vfx-attachment-v3-structural-hard-gate@2",
    );
    let structural_status = quality.get("schema_version").and_then(Value::as_str)
        == Some("CandidateAnimationVfxQuality@2")
        && quality.get("candidate_binding_status").and_then(Value::as_str)
            == Some(
                "same-material-surface-head-candidate-exact-attachment-v3-all-15-frames-no-geometry-mutation",
            )
        && quality.get("validator_status").and_then(Value::as_str) == Some("passed")
        && quality.get("hard_gate_passed") == Some(&Value::Bool(true))
        && quality.get("animation_status").and_then(Value::as_str) == Some("structural_only")
        && quality.get("vfx_status").and_then(Value::as_str) == Some("structural_only")
        && quality.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && quality.get("artistic_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && quality.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
        && quality
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            == Some("NOT_PROVEN")
        && quality.get("commercial_engine_status").and_then(Value::as_str) == Some("NOT_RUN")
        && quality.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
        && quality.get("functional_semantics") == Some(&Value::Bool(false))
        && quality.get("materialization_status").and_then(Value::as_str)
            == Some("runtime-owned-durable-candidate-animation-vfx-quality-v2")
        && quality.get("quality_status").and_then(Value::as_str) == Some("structural_only")
        && quality.get("runtime_write_performed") == Some(&Value::Bool(true))
        && quality.get("production_stage_advanced") == Some(&Value::Bool(false))
        && quality.get("candidate_confirmed") == Some(&Value::Bool(false))
        && quality.get("version_created") == Some(&Value::Bool(false))
        && quality.get("export_performed") == Some(&Value::Bool(false))
        && quality.get("sample_count") == Some(&Value::from(15_u64))
        && quality.get("attachment_frame_count") == Some(&Value::from(15_u64))
        && quality.get("geometry_preservation_status").and_then(Value::as_str)
            == Some("source-output-renderable-geometry-byte-exact")
        && quality.get("anchor_binding_policy").and_then(Value::as_str)
            == Some("geometry-appearance-anchor-role-owner-trs-equivalent@1")
        && quality.get("attachment_policy").and_then(Value::as_str)
            == Some("projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3")
        && quality.get("frame_scope").and_then(Value::as_str)
            == Some("lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3")
        && quality.get("animation_vfx_scope").and_then(Value::as_str)
            == Some("lod0-rigid-animation-full-vfx-stack-attachment-v3-all-15-frames@2")
        && quality.get("animation_vfx_policy").and_then(Value::as_str)
            == Some("candidate-animation-vfx-attachment-v3-structural-hard-gate@2")
        && quality
            .get("animation_vfx_policy_sha256")
            .and_then(Value::as_str)
            == Some(expected_animation_vfx_policy_sha256.as_str())
        && quality.get("from_stage").and_then(Value::as_str) == Some("material-surface")
        && quality.get("to_stage").and_then(Value::as_str) == Some("animation-vfx")
        && valid_sha256(quality.get("attachment_key_sha256").and_then(Value::as_str))
        && valid_sha256(quality.get("attachment_canonical_sha256").and_then(Value::as_str))
        && valid_sha256(
            quality
                .get("attachment_receipt_object_sha256")
                .and_then(Value::as_str),
        )
        && valid_sha256(
            quality
                .get("attachment_receipt_canonical_sha256")
                .and_then(Value::as_str),
        )
        && valid_sha256(
            quality
                .get("attachment_frame_set_sha256")
                .and_then(Value::as_str),
        )
        && ticks_are_valid
        && hash_fields_are_valid
        && input_sha256_is_bound
        && canonical_sha256_is_bound
        && quality
            .get("created_at")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty() && value.len() <= 128);
    if !structural_status
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: CandidateAnimationVfxQuality@2 attachment binding, structural status or media boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound() {
        let candidate_binding_safe = if is_prepare {
            binding.candidate_id.as_deref() == geometry_candidate
        } else {
            binding.candidate_id.as_deref() == geometry_candidate
                || binding.candidate_id.as_deref() == response_candidate
        };
        if binding.project_id.as_deref() != response_project || !candidate_binding_safe {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: CandidateAnimationVfxQuality@2 response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_form_art_raster_attribution_response(
    value: &Value,
    base: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 44] = [
        "schema_version",
        "diagnostic_id",
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "artifact_readback_canonical_sha256",
        "reference_id",
        "reference_sha256",
        "view_kind",
        "view_id",
        "camera_hash",
        "camera_canonical_sha256",
        "camera_object_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "form_art_view_receipt_object_sha256",
        "form_art_view_receipt_canonical_sha256",
        "target_object_sha256",
        "target_canonical_sha256",
        "target_mask_source_sha256",
        "projected_target_mask_sha256",
        "reviewed_region_mask_sha256",
        "expected_void_mask_sha256",
        "render_set_object_sha256",
        "render_set_canonical_sha256",
        "reviewed_region_structure_id",
        "reviewed_region_canonical_sha256",
        "owner_changed_status",
        "policy",
        "diagnostic",
        "quality_status",
        "runtime_write",
        "worker_started",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "input_sha256",
        "diagnostic_canonical_sha256",
    ];
    const DIAGNOSTIC_FIELDS: [&str; 22] = [
        "width",
        "height",
        "visible_pixel_count",
        "background_pixel_count",
        "reviewed_region_attributed_pixel_count",
        "expected_void_attributed_pixel_count",
        "owner_changed_attributed_pixel_count",
        "triangle_ids_sha256",
        "source_table_sha256",
        "sources",
        "highest_impact_source",
        "highest_impact_basis",
        "highest_impact_pixel_count",
        "expected_semantic_part_ids",
        "highest_impact_semantic_match",
        "repair_target_status",
        "render_worker_build_cohort_sha256",
        "status",
        "diagnostic_only",
        "promotable",
        "runtime_write",
        "production_stage_advanced",
    ];
    const SOURCE_FIELDS: [&str; 11] = [
        "semantic_part_id",
        "source_node_id",
        "lineage_source_node_ids",
        "material_zone_ids",
        "mesh_indices",
        "primitive_indices",
        "triangle_count",
        "visible_pixel_count",
        "reviewed_region_pixel_count",
        "expected_void_pixel_count",
        "owner_changed_pixel_count",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution diagnostic must be an object".to_owned()
    })?;
    let inner = object
        .get("diagnostic")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution diagnostic payload is missing"
                .to_owned()
        })?;
    if !object_has_exact_fields(object, &RESULT_FIELDS) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution result fields differ".to_owned(),
        );
    }
    if !object_has_exact_fields(inner, &DIAGNOSTIC_FIELDS) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution diagnostic fields differ"
                .to_owned(),
        );
    }
    if object.get("schema_version").and_then(Value::as_str)
        != Some("ProductionWeaponRasterSourceAttributionDiagnosticGetResult@1")
        || object.get("owner_changed_status").and_then(Value::as_str)
            != Some("NOT_APPLICABLE_SINGLE_CANDIDATE")
        || object.get("policy").and_then(Value::as_str)
            != Some("production-weapon-raster-source-attribution-single-candidate-diagnostic@1")
        || object.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || object.get("runtime_write") != Some(&Value::Bool(false))
        || object.get("worker_started") != Some(&Value::Bool(true))
        || [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .any(|field| object.get(*field) != Some(&Value::Bool(false)))
        || inner.get("width").and_then(Value::as_u64) != Some(512)
        || inner.get("height").and_then(Value::as_u64) != Some(512)
        || inner.get("status").and_then(Value::as_str)
            != Some("TRANSIENT_SOURCE_ATTRIBUTION_OBSERVED")
        || inner.get("diagnostic_only") != Some(&Value::Bool(true))
        || inner.get("promotable") != Some(&Value::Bool(false))
        || inner.get("runtime_write") != Some(&Value::Bool(false))
        || inner.get("production_stage_advanced") != Some(&Value::Bool(false))
        || !matches!(
            inner.get("highest_impact_basis").and_then(Value::as_str),
            Some("owner-changed-pixels" | "expected-void-pixels")
        )
        || inner
            .get("highest_impact_pixel_count")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
        || inner
            .get("owner_changed_attributed_pixel_count")
            .and_then(Value::as_u64)
            != Some(0)
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution schema or zero-write policy differs".to_owned());
    }
    let art = base
        .get("art_evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution is missing its FormArt record"
                .to_owned()
        })?;
    for field in [
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
    ] {
        if object.get(field) != art.get(field) {
            return Err(format!(
                "AGENTIC_SCOPE_MISMATCH: raster attribution {field} differs from FormArt"
            ));
        }
    }
    if object.get("form_art_evidence_object_sha256") != art.get("receipt_object_sha256")
        || object.get("form_art_evidence_canonical_sha256") != art.get("canonical_sha256")
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: raster attribution FormArt hash binding differs".to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != object.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != object.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != object.get("candidate_id").and_then(Value::as_str))
    {
        return Err("AGENTIC_SCOPE_MISMATCH: raster attribution crossed the bound session/project/candidate".to_owned());
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "artifact_readback_canonical_sha256",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "camera_object_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "form_art_view_receipt_object_sha256",
        "form_art_view_receipt_canonical_sha256",
        "target_object_sha256",
        "target_canonical_sha256",
        "target_mask_source_sha256",
        "projected_target_mask_sha256",
        "reviewed_region_mask_sha256",
        "expected_void_mask_sha256",
        "render_set_object_sha256",
        "render_set_canonical_sha256",
        "reviewed_region_canonical_sha256",
        "input_sha256",
        "diagnostic_canonical_sha256",
    ] {
        if !valid_sha256(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution {field} is invalid"
            ));
        }
    }
    for field in ["triangle_ids_sha256", "source_table_sha256"] {
        if !valid_sha256(inner.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution {field} is invalid"
            ));
        }
    }
    if !matches!(
        inner.get("render_worker_build_cohort_sha256"),
        Some(Value::Null)
    ) && !valid_sha256(
        inner
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str),
    ) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution Worker cohort is invalid"
                .to_owned(),
        );
    }
    let visible = inner.get("visible_pixel_count").and_then(Value::as_u64);
    let background = inner.get("background_pixel_count").and_then(Value::as_u64);
    if visible
        .zip(background)
        .is_none_or(|(visible, background)| visible + background != 512 * 512)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution pixel accounting differs"
                .to_owned(),
        );
    }
    let sources = inner
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution sources are missing".to_owned()
        })?;
    if sources.len() > 1_000_000
        || sources.iter().any(|source| {
            let Some(source) = source.as_object() else {
                return true;
            };
            !object_has_exact_fields(source, &SOURCE_FIELDS)
                || !valid_v2_id(source.get("semantic_part_id").and_then(Value::as_str))
                || !valid_v2_id(source.get("source_node_id").and_then(Value::as_str))
                || ["lineage_source_node_ids", "material_zone_ids"]
                    .iter()
                    .any(|field| {
                        source
                            .get(*field)
                            .and_then(Value::as_array)
                            .is_none_or(|items| {
                                items.is_empty()
                                    || items.len() > 256
                                    || items.iter().any(|item| !valid_v2_id(item.as_str()))
                            })
                    })
                || ["mesh_indices", "primitive_indices"].iter().any(|field| {
                    source
                        .get(*field)
                        .and_then(Value::as_array)
                        .is_none_or(|items| {
                            items.is_empty()
                                || items.len() > 1_000_000
                                || items.iter().any(|item| {
                                    item.as_u64().is_none_or(|value| value > u32::MAX as u64)
                                })
                        })
                })
                || [
                    "triangle_count",
                    "visible_pixel_count",
                    "reviewed_region_pixel_count",
                    "expected_void_pixel_count",
                    "owner_changed_pixel_count",
                ]
                .iter()
                .any(|field| source.get(*field).and_then(Value::as_u64).is_none())
        })
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution source table differs".to_owned(),
        );
    }
    let highest = inner
        .get("highest_impact_source")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: highest-impact raster source is missing".to_owned()
        })?;
    let maximum = sources
        .iter()
        .filter_map(|source| {
            source
                .get("expected_void_pixel_count")
                .and_then(Value::as_u64)
        })
        .max()
        .unwrap_or(0);
    let top_sources = sources
        .iter()
        .filter(|source| {
            source
                .get("expected_void_pixel_count")
                .and_then(Value::as_u64)
                == Some(maximum)
        })
        .collect::<Vec<_>>();
    if !object_has_exact_fields(highest, &SOURCE_FIELDS)
        || inner.get("highest_impact_basis").and_then(Value::as_str) != Some("expected-void-pixels")
        || maximum == 0
        || top_sources.len() != 1
        || top_sources[0] != inner.get("highest_impact_source").unwrap_or(&Value::Null)
        || inner
            .get("highest_impact_pixel_count")
            .and_then(Value::as_u64)
            != Some(maximum)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: highest-impact source selection is ambiguous or differs"
                .to_owned(),
        );
    }
    let expected_semantic_part_ids = inner
        .get("expected_semantic_part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution semantic scope is missing"
                .to_owned()
        })?;
    if expected_semantic_part_ids != &vec![Value::String("rear-stock".to_owned())] {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution semantic scope differs".to_owned(),
        );
    }
    let semantic_match = highest
        .get("semantic_part_id")
        .and_then(Value::as_str)
        .is_some_and(|part_id| {
            expected_semantic_part_ids
                .iter()
                .any(|expected| expected.as_str() == Some(part_id))
        });
    let expected_repair_status = if semantic_match {
        "UNIQUE_HIGHEST_IMPACT_SOURCE_OBSERVED"
    } else {
        "BLOCKED_REVIEW_REGION_SEMANTIC_MISMATCH"
    };
    if inner
        .get("highest_impact_semantic_match")
        .and_then(Value::as_bool)
        != Some(semantic_match)
        || inner.get("repair_target_status").and_then(Value::as_str) != Some(expected_repair_status)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution semantic repair gate differs"
                .to_owned(),
        );
    }
    if contains_raw_media_field(value) || contains_forbidden_transport_field(value) {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution contains forbidden media or transport data".to_owned());
    }
    let mut preimage = object.clone();
    preimage.insert(
        "diagnostic_canonical_sha256".to_owned(),
        Value::String(String::new()),
    );
    let expected = forgecad_runtime::canonical_json_hash(&Value::Object(preimage));
    if object
        .get("diagnostic_canonical_sha256")
        .and_then(Value::as_str)
        != Some(expected.as_str())
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution canonical hash differs".to_owned(),
        );
    }
    Ok(())
}

/// Validate Runtime readback before it is exposed as a successful Agentic
/// response. This prevents a Runtime with a missing or divergent binding from
/// turning an unscoped payload into a usable checkpoint/session.
pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = AgenticTool::from_name(name) else {
        return Ok(());
    };
    if !value.is_object() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Runtime response must be a typed object".to_owned(),
        );
    }
    let session_id = find_string(value, "session_id", 0);
    let project_id = find_string(value, "project_id", 0);
    let candidate_id = find_string(value, "candidate_id", 0);
    let animation_clip_v2_tool = matches!(
        tool,
        AgenticTool::MechanicalAnimationClipV2Prepare
            | AgenticTool::MechanicalAnimationClipV2Get
            | AgenticTool::MechanicalAnimationClipV2Preview
    );
    if animation_clip_v2_tool {
        return validate_mechanical_animation_clip_v2_response(tool, value, binding);
    }
    let animation_glb_v2_tool = matches!(
        tool,
        AgenticTool::MechanicalAnimationGlbV2Prepare | AgenticTool::MechanicalAnimationGlbV2Get
    );
    if animation_glb_v2_tool {
        return validate_mechanical_animation_glb_v2_response(tool, value, binding);
    }
    let animated_socket_v2_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare
            | AgenticTool::GameWeaponAnimatedGlbSocketV2Get
    );
    if animated_socket_v2_tool {
        return validate_game_weapon_animated_glb_socket_v2_response(tool, value, binding);
    }
    let animation_vfx_v2_tool = matches!(
        tool,
        AgenticTool::CandidateAnimationVfxQualityV2Prepare
            | AgenticTool::CandidateAnimationVfxQualityV2Get
    );
    if animation_vfx_v2_tool {
        return validate_candidate_animation_vfx_quality_v2_response(tool, value, binding);
    }
    let form_evidence_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormEvidencePrepare
            | AgenticTool::ProductionWeaponFormEvidenceGet
    );
    if form_evidence_tool {
        return validate_production_weapon_form_evidence_response(tool, value, binding);
    }
    let form_art_evidence_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormArtEvidencePrepare
            | AgenticTool::ProductionWeaponFormArtEvidenceGet
    );
    if form_art_evidence_tool {
        if let Some(diagnostic) = value.get("raster_source_attribution_diagnostic") {
            if !diagnostic.is_object() {
                return Err(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: raster attribution diagnostic must be an object"
                        .to_owned(),
                );
            }
            let mut base = value.clone();
            base.as_object_mut()
                .expect("validated Runtime response object")
                .remove("raster_source_attribution_diagnostic");
            validate_production_weapon_form_art_evidence_response(tool, &base, binding)?;
            validate_form_art_raster_attribution_response(diagnostic, &base, binding)?;
            return Ok(());
        }
        return validate_production_weapon_form_art_evidence_response(tool, value, binding);
    }
    if tool == AgenticTool::ProductionWeaponArtDecisionProposalGet {
        return validate_production_weapon_art_decision_proposal_response(value, binding);
    }
    if tool == AgenticTool::ProductionWeaponAssemblyParameterSinkGet {
        return validate_production_weapon_assembly_parameter_sink_response(value, binding);
    }
    if tool == AgenticTool::ProductionWeaponFormQualityV2PreflightGet {
        return validate_production_weapon_form_quality_v2_preflight_response(value, binding);
    }
    if tool == AgenticTool::ProductionWeaponHighLowBakePreflightGet {
        return validate_production_weapon_high_low_bake_preflight_response(value, binding);
    }
    let form_quality_v2_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormQualityV2Prepare
            | AgenticTool::ProductionWeaponFormQualityV2Get
    );
    if form_quality_v2_tool {
        return validate_production_weapon_form_quality_v2_response(tool, value, binding);
    }
    let form_quality_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponFormQualityPrepare
            | AgenticTool::ProductionWeaponFormQualityGet
    );
    if form_quality_tool {
        return validate_production_weapon_form_quality_response(tool, value, binding);
    }
    let retopology_cage_source_tool = matches!(
        tool,
        AgenticTool::ProductionWeaponRetopologyCageSourcePrepare
            | AgenticTool::ProductionWeaponRetopologyCageSourceGet
    );
    if retopology_cage_source_tool {
        return validate_production_weapon_retopology_cage_source_response(tool, value, binding);
    }
    let camera_lock_tool = matches!(
        tool,
        AgenticTool::ProductionCameraLockPrepare | AgenticTool::ProductionCameraLockGet
    );
    if camera_lock_tool {
        const RESULT_FIELDS: [&str; 9] = [
            "schema_version",
            "camera_lock",
            "replayed",
            "runtime_write",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "restart_hash_verified",
        ];
        const LOCK_FIELDS: [&str; 39] = [
            "schema_version",
            "camera_lock_id",
            "session_id",
            "project_id",
            "source_transition_id",
            "source_transition_sha256",
            "source_head_canonical_sha256",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "reference_id",
            "reference_sha256",
            "reference_canvas_object_sha256",
            "reference_canvas_canonical_sha256",
            "design_spec_object_sha256",
            "design_spec_canonical_sha256",
            "camera_rig_object_sha256",
            "camera_rig_canonical_sha256",
            "required_reference_view_kinds",
            "required_camera_view_kinds",
            "primary_view_kind",
            "calibration_policy",
            "review_status",
            "calibration_status",
            "structural_status",
            "visual_status",
            "human_status",
            "engine_status",
            "distribution_status",
            "approval_receipt_id",
            "approval_session_id",
            "approval_expires_at",
            "approval_summary_sha256",
            "input_sha256",
            "request_key_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
            "created_at",
        ];
        let lock = value
            .get("camera_lock")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: camera lock result is missing camera_lock"
                    .to_owned()
            })?;
        let expected_schema = if tool == AgenticTool::ProductionCameraLockPrepare {
            "ProductionCameraLockPrepareResult@1"
        } else {
            "ProductionCameraLockGetResult@1"
        };
        let ids_are_non_empty = [
            "camera_lock_id",
            "session_id",
            "project_id",
            "source_transition_id",
            "candidate_id",
            "artifact_id",
            "reference_id",
            "approval_receipt_id",
            "approval_session_id",
        ]
        .iter()
        .all(|field| {
            lock.get(*field)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        });
        let hash_fields = [
            "source_transition_sha256",
            "source_head_canonical_sha256",
            "candidate_state_sha256",
            "artifact_sha256",
            "reference_sha256",
            "reference_canvas_object_sha256",
            "reference_canvas_canonical_sha256",
            "design_spec_object_sha256",
            "design_spec_canonical_sha256",
            "camera_rig_object_sha256",
            "camera_rig_canonical_sha256",
            "approval_summary_sha256",
            "input_sha256",
            "request_key_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
        ];
        let hashes_are_valid = hash_fields
            .iter()
            .all(|field| valid_sha256(lock.get(*field).and_then(Value::as_str)));
        let fixed_views = lock.get("required_reference_view_kinds")
            == Some(&json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "rear-three-quarter"
            ]))
            && lock.get("required_camera_view_kinds")
                == Some(&json!([
                    "front",
                    "back",
                    "left",
                    "right",
                    "top",
                    "bottom",
                    "rear-three-quarter"
                ]))
            && lock.get("primary_view_kind").and_then(Value::as_str) == Some("left")
            && lock.get("calibration_policy").and_then(Value::as_str)
                == Some("fps-weapon-reviewed-six-reference-seven-camera-lock@1");
        let fixed_status = lock.get("schema_version").and_then(Value::as_str)
            == Some("ProductionCameraLock@1")
            && lock.get("review_status").and_then(Value::as_str)
                == Some("user-approved-reference-coverage")
            && lock.get("calibration_status").and_then(Value::as_str) == Some("passed")
            && lock.get("structural_status").and_then(Value::as_str)
                == Some("PASS_SOURCE_STRUCTURAL")
            && lock.get("visual_status").and_then(Value::as_str) == Some("QUALITY_TARGET_NOT_MET")
            && lock.get("human_status").and_then(Value::as_str) == Some("NOT_RUN")
            && lock.get("engine_status").and_then(Value::as_str) == Some("NOT_RUN")
            && lock.get("distribution_status").and_then(Value::as_str) == Some("NOT_RUN")
            && lock.get("approval_session_id") == lock.get("session_id");
        let flags_are_safe = value.get("production_stage_advanced") == Some(&Value::Bool(false))
            && value.get("candidate_confirmed") == Some(&Value::Bool(false))
            && value.get("version_created") == Some(&Value::Bool(false))
            && value.get("export_performed") == Some(&Value::Bool(false))
            && value.get("restart_hash_verified") == Some(&Value::Bool(true));
        if !object_has_exact_fields(value.as_object().expect("checked object"), &RESULT_FIELDS)
            || !object_has_exact_fields(lock, &LOCK_FIELDS)
            || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::ProductionCameraLockPrepare,
                ))
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || !ids_are_non_empty
            || !hashes_are_valid
            || !fixed_views
            || !fixed_status
            || !flags_are_safe
            || contains_forbidden_transport_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: ProductionCameraLock@1 result schema, exact view profiles, truthful statuses or side-effect flags differ"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.session_id.as_deref() != lock.get("session_id").and_then(Value::as_str)
                || binding.project_id.as_deref() != lock.get("project_id").and_then(Value::as_str)
                || binding.candidate_id.as_deref()
                    != lock.get("candidate_id").and_then(Value::as_str))
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: camera lock response crossed the bound session/project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_attachment_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentGet
    );
    if animated_socket_attachment_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare {
                "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@1"
            } else {
                "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@1"
            };
        let attachment = value
            .get("attachment")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket attachment response is missing its record"
                    .to_owned()
            })?;
        let frames = attachment.get("frames").and_then(Value::as_array);
        let attachment_key = attachment
            .get("attachment_key_sha256")
            .and_then(Value::as_str);
        let response_project = attachment.get("project_id").and_then(Value::as_str);
        let response_candidate = attachment.get("candidate_id").and_then(Value::as_str);
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let no_raw_media = !contains_raw_media_field(&Value::Object(attachment.clone()));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || value.get("attachment_key_sha256").and_then(Value::as_str) != attachment_key
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentPrepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || frames.is_none_or(|items| items.is_empty() || items.len() > 16)
            || !no_raw_media
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket attachment schema, side-effect flags, frame bounds or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket attachment response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_attachment_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Get
    );
    if animated_socket_attachment_v2_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare {
                "FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult@2"
            } else {
                "FictionalEnergyVfxAnimatedSocketAttachmentGetResult@2"
            };
        let attachment = value
            .get("attachment")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated-socket attachment response is missing its record"
                    .to_owned()
            })?;
        let frames = attachment.get("frames").and_then(Value::as_array);
        let attachment_key = attachment
            .get("attachment_key_sha256")
            .and_then(Value::as_str);
        let response_project = attachment.get("project_id").and_then(Value::as_str);
        let response_candidate = attachment.get("candidate_id").and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|candidate| {
                candidate.len() == 64
                    && candidate
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let parent_hashes_are_valid = [
            "attachment_key_sha256",
            "delivery_manifest_object_sha256",
            "candidate_state_sha256",
            "source_artifact_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animated_artifact_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "particle_sequence_key_sha256",
            "particle_sequence_canonical_sha256",
            "trail_sequence_key_sha256",
            "trail_sequence_canonical_sha256",
            "trail_bloom_sequence_key_sha256",
            "trail_bloom_sequence_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "canonical_sha256",
        ]
        .iter()
        .all(|field| hash_is_valid(attachment.get(*field).and_then(Value::as_str)));
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=15).contains(&items.len())
                && items.iter().enumerate().all(|(index, frame)| {
                    frame.get("schema_version").and_then(Value::as_str)
                        == Some("FictionalEnergyVfxAnimatedSocketAttachmentFrame@2")
                        && frame.get("attachment_key_sha256").and_then(Value::as_str)
                            == attachment_key
                        && frame.get("frame_index") == Some(&Value::from(index as u64))
                        && frame
                            .get("projection_frame_index")
                            .and_then(Value::as_u64)
                            .is_some_and(|frame_index| (1..=15).contains(&frame_index))
                        && frame
                            .get("particle_sequence_frame_index")
                            .and_then(Value::as_u64)
                            .is_some_and(|frame_index| (1..=15).contains(&frame_index))
                        && frame
                            .get("sample_time_ticks")
                            .and_then(Value::as_u64)
                            .is_some_and(|ticks| ticks <= 1_000_000)
                        && [
                            "animation_pose_readback_sha256",
                            "socket_transform_inventory_sha256",
                            "socket_transform_readback_sha256",
                            "emitter_socket_bindings_sha256",
                            "trail_socket_bindings_sha256",
                            "base_frame_key_sha256",
                            "bloom_key_sha256",
                            "particle_key_sha256",
                            "trail_key_sha256",
                            "trail_bloom_key_sha256",
                            "projection_frame_canonical_sha256",
                            "particle_sequence_frame_canonical_sha256",
                            "trail_sequence_frame_canonical_sha256",
                            "trail_bloom_sequence_frame_canonical_sha256",
                            "canonical_sha256",
                        ]
                        .iter()
                        .all(|field| hash_is_valid(frame.get(*field).and_then(Value::as_str)))
                        && frame
                            .get("created_at")
                            .and_then(Value::as_str)
                            .is_some_and(|created_at| !created_at.is_empty())
                })
        });
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(value.get("attachment_key_sha256").and_then(Value::as_str))
            || value.get("attachment_key_sha256").and_then(Value::as_str) != attachment_key
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV2Prepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || attachment.get("schema_version").and_then(Value::as_str)
                != Some("FictionalEnergyVfxAnimatedSocketAttachment@2")
            || attachment.get("attachment_policy").and_then(Value::as_str)
                != Some("fictional-energy-vfx-animated-socket-attachment-projection-bound@2")
            || attachment.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-vfx-trail-frame-range-1-15@2")
            || attachment.get("attachment_status").and_then(Value::as_str)
                != Some("runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v2")
            || !parent_hashes_are_valid
            || !frame_bounds_are_safe
            || contains_raw_media_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated-socket attachment schema, projection-bound frame hashes, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 animated-socket attachment response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketAttachmentV3Get
    ) {
        return validate_animated_socket_attachment_v3_response(tool, value, binding);
    }
    let animated_socket_transform_projection_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare
            | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionGet
    );
    if animated_socket_transform_projection_tool {
        let expected_schema =
            if tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare {
                "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@1"
            } else {
                "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@1"
            };
        let projection = value
            .get("projection")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated GLB socket transform projection response is missing its record"
                    .to_owned()
            })?;
        let frames = projection.get("frames").and_then(Value::as_array);
        let projection_key = projection
            .get("projection_key_sha256")
            .and_then(Value::as_str);
        let response_project = projection.get("project_id").and_then(Value::as_str);
        let response_candidate = projection.get("candidate_id").and_then(Value::as_str);
        let projection_object_sha256 = value
            .get("projection_object_sha256")
            .and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=16).contains(&items.len())
                && items.iter().all(|frame| {
                    frame
                        .get("socket_transforms")
                        .and_then(Value::as_array)
                        .is_some_and(|sockets| sockets.len() == 6)
                })
        });
        let projection_flags_are_safe = projection.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && projection.get("restart_hash_verified") == Some(&Value::Bool(true))
            && projection.get("candidate_confirmed") == Some(&Value::Bool(false))
            && projection.get("version_created") == Some(&Value::Bool(false))
            && projection.get("export_performed") == Some(&Value::Bool(false))
            && projection.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && projection.get("production_stage_advanced") == Some(&Value::Bool(false));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(projection_key)
            || value.get("projection_key_sha256").and_then(Value::as_str) != projection_key
            || !hash_is_valid(projection_object_sha256)
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionPrepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || !frame_bounds_are_safe
            || projection.get("schema_version").and_then(Value::as_str)
                != Some("GameWeaponAnimatedGlbSocketTransformProjection@1")
            || projection.get("projection_status").and_then(Value::as_str)
                != Some(
                    "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection",
                )
            || projection.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || projection
                .get("visual_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || projection
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || projection
                .get("human_review_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || projection
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || !projection_flags_are_safe
            || contains_raw_media_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated GLB socket transform projection schema, six-socket frame bounds, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated GLB socket transform projection response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_transform_projection_v2_tool = matches!(
        tool,
        AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare
            | AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Get
    );
    if animated_socket_transform_projection_v2_tool {
        let expected_schema =
            if tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare {
                "GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult@2"
            } else {
                "GameWeaponAnimatedGlbSocketTransformProjectionGetResult@2"
            };
        let projection = value
            .get("projection")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated GLB socket transform projection response is missing its record"
                    .to_owned()
            })?;
        let projection_key = projection
            .get("projection_key_sha256")
            .and_then(Value::as_str);
        let projection_object_sha256 = value
            .get("projection_object_sha256")
            .and_then(Value::as_str);
        let response_project = projection.get("project_id").and_then(Value::as_str);
        let response_candidate = projection
            .get("appearance_candidate_id")
            .and_then(Value::as_str);
        let required_hashes = [
            "projection_key_sha256",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "appearance_artifact_readback_sha256",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_glb_key_sha256",
            "animated_artifact_sha256",
            "animated_artifact_readback_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "derived_animated_socket_artifact_sha256",
            "derived_animated_socket_artifact_readback_sha256",
            "derived_animated_socket_receipt_object_sha256",
            "derived_animated_socket_receipt_canonical_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_node_inventory_sha256",
            "socket_roles_sha256",
            "part_hierarchy_sha256",
            "sampling_policy_sha256",
            "sample_schedule_sha256",
            "input_sha256",
            "canonical_sha256",
        ];
        let hashes_are_valid = required_hashes
            .iter()
            .all(|field| valid_sha256(projection.get(*field).and_then(Value::as_str)));
        let socket_is_complete = |socket: &Value| {
            let Some(socket) = socket.as_object() else {
                return false;
            };
            [
                "socket_node_id",
                "anchor_id",
                "role",
                "node_name",
                "node_kind",
                "parent_kind",
                "local_transform",
                "parent_world_transform",
                "composed_world_transform",
            ]
            .iter()
            .all(|field| socket.get(*field).is_some())
                && socket.get("node_index").and_then(Value::as_u64).is_some()
                && socket
                    .get("parent_node_index")
                    .and_then(Value::as_i64)
                    .is_some()
                && [
                    "local_matrix_4x4",
                    "parent_world_matrix_4x4",
                    "composed_world_matrix_4x4",
                ]
                .iter()
                .all(|field| {
                    socket
                        .get(*field)
                        .and_then(Value::as_array)
                        .is_some_and(|matrix| matrix.len() == 16)
                })
        };
        let frame_bounds_are_safe = projection
            .get("frames")
            .and_then(Value::as_array)
            .is_some_and(|frames| {
                (1..=16).contains(&frames.len())
                    && frames.iter().enumerate().all(|(index, frame)| {
                        frame.get("schema_version").and_then(Value::as_str)
                            == Some("GameWeaponAnimatedGlbSocketTransformProjectionFrame@2")
                            && frame.get("projection_key_sha256").and_then(Value::as_str)
                                == projection_key
                            && frame.get("frame_index") == Some(&Value::from(index as u64))
                            && frame
                                .get("sample_time_ticks")
                                .and_then(Value::as_u64)
                                .is_some_and(|ticks| ticks <= 1_000_000)
                            && [
                                "source_animation_sample_sha256",
                                "derived_socket_sample_sha256",
                                "socket_transform_inventory_sha256",
                                "socket_transform_readback_sha256",
                                "projection_frame_canonical_sha256",
                                "canonical_sha256",
                            ]
                            .iter()
                            .all(|field| valid_sha256(frame.get(*field).and_then(Value::as_str)))
                            && frame
                                .get("created_at")
                                .and_then(Value::as_str)
                                .is_some_and(|created_at| !created_at.is_empty())
                            && frame
                                .get("socket_transforms")
                                .and_then(Value::as_array)
                                .is_some_and(|sockets| {
                                    sockets.len() == 6 && sockets.iter().all(socket_is_complete)
                                })
                    })
            });
        let projection_flags_are_safe = projection.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && projection.get("restart_hash_verified") == Some(&Value::Bool(true))
            && projection.get("candidate_confirmed") == Some(&Value::Bool(false))
            && projection.get("version_created") == Some(&Value::Bool(false))
            && projection.get("export_performed") == Some(&Value::Bool(false))
            && projection.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && projection.get("production_stage_advanced") == Some(&Value::Bool(false));
        let top_flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let socket_roles_are_fixed = projection.get("socket_roles")
            == Some(&json!([
                "weapon-root",
                "grip-primary",
                "muzzle-vfx",
                "magazine-well",
                "sight-primary",
                "energy-core-vfx"
            ]));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !valid_sha256(projection_key)
            || value.get("projection_key_sha256").and_then(Value::as_str) != projection_key
            || !valid_sha256(projection_object_sha256)
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write_performed")
                != Some(&Value::Bool(
                    tool == AgenticTool::GameWeaponAnimatedGlbSocketTransformProjectionV2Prepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !top_flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || projection.get("schema_version").and_then(Value::as_str)
                != Some("GameWeaponAnimatedGlbSocketTransformProjection@2")
            || projection.get("projection_status").and_then(Value::as_str)
                != Some(
                    "runtime-owned-durable-game-weapon-animated-glb-socket-transform-projection-v2",
                )
            || projection.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || projection
                .get("visual_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || projection
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || projection
                .get("human_review_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || projection
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || !projection_flags_are_safe
            || !hashes_are_valid
            || !socket_roles_are_fixed
            || projection.get("sample_count").and_then(Value::as_u64)
                != projection
                    .get("frames")
                    .and_then(Value::as_array)
                    .map(|frames| frames.len() as u64)
            || projection.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-frame-range-1-16@2")
            || !frame_bounds_are_safe
            || contains_raw_media_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated GLB socket transform projection schema, six complete sockets, hashes, status, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 animated GLB socket transform projection response crossed the bound project/appearance-candidate binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_particles_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceGet
    );
    if animated_socket_particles_sequence_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare {
                "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@1"
            } else {
                "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@1"
            };
        let sequence = value
            .get("sequence")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket particle sequence response is missing its record"
                    .to_owned()
            })?;
        let frames = sequence.get("frames").and_then(Value::as_array);
        let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
        let response_project = sequence.get("project_id").and_then(Value::as_str);
        let response_candidate = sequence.get("candidate_id").and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=16).contains(&items.len())
                && items.iter().enumerate().all(|(index, frame)| {
                    frame.get("schema_version").and_then(Value::as_str)
                        == Some("FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@1")
                        && frame.get("frame_index") == Some(&Value::from(index as u64))
                        && frame
                            .get("sample_time_ticks")
                            .and_then(Value::as_u64)
                            .is_some_and(|ticks| ticks <= 1_000_000)
                        && hash_is_valid(
                            frame
                                .get("projection_frame_canonical_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("projection_socket_transform_inventory_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("projection_socket_transform_readback_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("base_frame_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("bloom_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame
                                .get("emitter_socket_bindings_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("input_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("particle_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("particle_seed_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame
                                .get("render_set_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("receipt_object_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame
                                .get("particle_color_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("particle_id_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("particle_depth_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("canonical_sha256").and_then(Value::as_str))
                        && frame
                            .get("created_at")
                            .and_then(Value::as_str)
                            .is_some_and(|created_at| !created_at.is_empty())
                })
        });
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let sequence_flags_are_safe = sequence.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
            && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
            && sequence.get("version_created") == Some(&Value::Bool(false))
            && sequence.get("export_performed") == Some(&Value::Bool(false))
            && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(sequence_key)
            || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequencePrepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || !frame_bounds_are_safe
            || sequence.get("schema_version").and_then(Value::as_str)
                != Some("FictionalEnergyVfxAnimatedSocketParticlesSequence@1")
            || sequence.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || !hash_is_valid(
                sequence
                    .get("geometry_preservation_projection_sha256")
                    .and_then(Value::as_str),
            )
            || sequence
                .get("geometry_preservation_status")
                .and_then(Value::as_str)
                != Some("source-output-renderable-geometry-byte-exact")
            || sequence.get("sequence_status").and_then(Value::as_str)
                != Some(
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence",
                )
            || sequence.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-particles-frame-range-1-16@1")
            || sequence
                .get("particles_sequence_policy")
                .and_then(Value::as_str)
                != Some("projection-driven-animated-socket-particles@1")
            || sequence
                .get("emitter_binding_policy")
                .and_then(Value::as_str)
                != Some("projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1")
            || sequence
                .get("transform_projection_policy")
                .and_then(Value::as_str)
                != Some("glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs@1")
            || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || sequence
                .get("visual_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || sequence
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || sequence
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || !sequence_flags_are_safe
            || contains_raw_media_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket particle sequence schema, bounded frame hashes, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket particle sequence response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_particles_sequence_v2_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Get
    );
    if animated_socket_particles_sequence_v2_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare {
                "FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult@2"
            } else {
                "FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult@2"
            };
        let sequence = value
            .get("sequence")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated-socket particle sequence response is missing its record"
                    .to_owned()
            })?;
        let frames = sequence.get("frames").and_then(Value::as_array);
        let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
        let response_project = sequence.get("project_id").and_then(Value::as_str);
        let geometry_candidate = sequence
            .get("geometry_candidate_id")
            .and_then(Value::as_str);
        let appearance_candidate = sequence
            .get("appearance_candidate_id")
            .and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|candidate| {
                candidate.len() == 64
                    && candidate
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let required_sequence_hashes = [
            "sequence_key_sha256",
            "geometry_candidate_state_sha256",
            "geometry_delivery_manifest_object_sha256",
            "geometry_artifact_sha256",
            "appearance_candidate_state_sha256",
            "appearance_delivery_manifest_object_sha256",
            "appearance_artifact_sha256",
            "material_surface_quality_report_object_sha256",
            "material_surface_quality_canonical_sha256",
            "geometry_preservation_projection_sha256",
            "projection_key_sha256",
            "projection_object_sha256",
            "projection_canonical_sha256",
            "animated_socket_materialization_key_sha256",
            "animated_artifact_sha256",
            "animated_socket_anchor_set_object_sha256",
            "animated_socket_anchor_set_canonical_sha256",
            "appearance_anchor_set_object_sha256",
            "appearance_anchor_set_canonical_sha256",
            "anchor_binding_sha256",
            "animation_clip_object_sha256",
            "animation_clip_canonical_sha256",
            "animation_receipt_object_sha256",
            "animation_receipt_canonical_sha256",
            "vfx_profile_object_sha256",
            "vfx_profile_canonical_sha256",
            "socket_node_id_encoding_sha256",
            "socket_roles_sha256",
            "camera_object_sha256",
            "camera_identity_sha256",
            "render_profile_sha256",
            "render_worker_build_cohort_sha256",
            "sample_schedule_sha256",
            "input_sha256",
            "canonical_sha256",
        ];
        let sequence_hashes_are_valid = required_sequence_hashes
            .iter()
            .all(|field| hash_is_valid(sequence.get(*field).and_then(Value::as_str)));
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=16).contains(&items.len())
                && items.iter().enumerate().all(|(index, frame)| {
                    frame.get("schema_version").and_then(Value::as_str)
                        == Some("FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame@2")
                        && frame.get("frame_index") == Some(&Value::from(index as u64))
                        && frame
                            .get("sample_time_ticks")
                            .and_then(Value::as_u64)
                            .is_some_and(|ticks| ticks <= 1_000_000)
                        && [
                            "projection_frame_canonical_sha256",
                            "projection_socket_transform_inventory_sha256",
                            "projection_socket_transform_readback_sha256",
                            "base_frame_key_sha256",
                            "bloom_key_sha256",
                            "emitter_socket_bindings_sha256",
                            "input_sha256",
                            "particle_key_sha256",
                            "particle_seed_sha256",
                            "render_set_object_sha256",
                            "receipt_object_sha256",
                            "particle_color_object_sha256",
                            "particle_id_object_sha256",
                            "particle_depth_object_sha256",
                            "canonical_sha256",
                        ]
                        .iter()
                        .all(|field| hash_is_valid(frame.get(*field).and_then(Value::as_str)))
                        && frame
                            .get("created_at")
                            .and_then(Value::as_str)
                            .is_some_and(|created_at| !created_at.is_empty())
                })
        });
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let sequence_flags_are_safe = sequence.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
            && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
            && sequence.get("version_created") == Some(&Value::Bool(false))
            && sequence.get("export_performed") == Some(&Value::Bool(false))
            && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
        let anchor_binding_is_valid = sequence
            .get("anchor_binding_policy")
            .and_then(Value::as_str)
            == Some("geometry-appearance-anchor-role-owner-trs-equivalent@1");
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(value.get("sequence_key_sha256").and_then(Value::as_str))
            || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Prepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || geometry_candidate.is_none_or(str::is_empty)
            || appearance_candidate.is_none_or(str::is_empty)
            || geometry_candidate == appearance_candidate
            || !sequence_hashes_are_valid
            || !frame_bounds_are_safe
            || sequence.get("schema_version").and_then(Value::as_str)
                != Some("FictionalEnergyVfxAnimatedSocketParticlesSequence@2")
            || sequence.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || sequence.get("sequence_status").and_then(Value::as_str)
                != Some(
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-particles-sequence-v2",
                )
            || sequence.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-particles-frame-range-1-16@2")
            || sequence
                .get("particles_sequence_policy")
                .and_then(Value::as_str)
                != Some("projection-v2-driven-animated-socket-particles-dual-candidate@2")
            || sequence
                .get("emitter_binding_policy")
                .and_then(Value::as_str)
                != Some("projection-role-muzzle-vfx-energy-core-vfx-to-particle-emitter@1")
            || sequence
                .get("transform_projection_policy")
                .and_then(Value::as_str)
                != Some("glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2")
            || !anchor_binding_is_valid
            || !hash_is_valid(sequence.get("anchor_binding_sha256").and_then(Value::as_str))
            || !hash_is_valid(sequence.get("input_sha256").and_then(Value::as_str))
            || !hash_is_valid(sequence.get("canonical_sha256").and_then(Value::as_str))
            || sequence
                .get("created_at")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            || sequence
                .get("geometry_delivery_manifest_object_sha256")
                .and_then(Value::as_str)
                .is_none_or(|hash| !hash_is_valid(Some(hash)))
            || sequence
                .get("appearance_delivery_manifest_object_sha256")
                .and_then(Value::as_str)
                .is_none_or(|hash| !hash_is_valid(Some(hash)))
            || sequence.get("geometry_preservation_status").and_then(Value::as_str)
                != Some("source-output-renderable-geometry-byte-exact")
            || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || sequence.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || sequence
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || sequence.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
            || !sequence_flags_are_safe
            || contains_raw_media_field(value)
            || contains_forbidden_transport_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated-socket particle sequence schema, dual-candidate bindings, bounded frame hashes, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != geometry_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 animated-socket particle sequence response crossed the bound project/geometry candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Get
    ) {
        return validate_animated_socket_trails_sequence_v2_response(tool, value, binding);
    }
    if matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Prepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Get
    ) {
        return validate_animated_socket_trails_bloom_sequence_v2_response(tool, value, binding);
    }
    let animated_socket_trails_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequenceGet
    );
    if animated_socket_trails_sequence_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare {
                "FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult@1"
            } else {
                "FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult@1"
            };
        let sequence = value
            .get("sequence")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket trails sequence response is missing its record"
                    .to_owned()
            })?;
        let frames = sequence.get("frames").and_then(Value::as_array);
        let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
        let response_project = sequence.get("project_id").and_then(Value::as_str);
        let response_candidate = sequence.get("candidate_id").and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=15).contains(&items.len())
                && items.iter().enumerate().all(|(index, frame)| {
                    frame.get("frame_index") == Some(&Value::from(index as u64))
                        && hash_is_valid(frame.get("trail_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("trail_seed_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame.get("trail_inventory_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_id_encoding_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame.get("emitter_binding_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_color_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame.get("trail_id_object_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_depth_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("render_set_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("receipt_object_sha256").and_then(Value::as_str))
                })
        });
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let sequence_flags_are_safe = sequence.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
            && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
            && sequence.get("version_created") == Some(&Value::Bool(false))
            && sequence.get("export_performed") == Some(&Value::Bool(false))
            && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(sequence_key)
            || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsSequencePrepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || !frame_bounds_are_safe
            || sequence.get("schema_version").and_then(Value::as_str)
                != Some("FictionalEnergyVfxAnimatedSocketTrailsSequence@1")
            || sequence.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || sequence.get("sequence_status").and_then(Value::as_str)
                != Some("runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-sequence")
            || sequence.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-trails-source-frames-1-15@1")
            || sequence.get("trails_sequence_policy").and_then(Value::as_str)
                != Some("projection-driven-animated-socket-trails@1")
            || sequence.get("history_policy").and_then(Value::as_str)
                != Some("one-to-eight-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@1")
            || sequence.get("history_pre_roll_policy").and_then(Value::as_str)
                != Some("same-parent-source-frame-zero-is-preroll-output-frames-one-to-fifteen@1")
            || sequence.get("trail_count") != Some(&Value::from(2_u64))
            || sequence.get("trail_emitter_roles")
                != Some(&json!(["muzzle-vfx", "energy-core-vfx"]))
            || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || sequence.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || sequence
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || sequence.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
            || !sequence_flags_are_safe
            || contains_forbidden_transport_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket trails sequence schema, bounded frame hashes, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket trails sequence response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animated_socket_trails_bloom_sequence_tool = matches!(
        tool,
        AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare
            | AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGet
    );
    if animated_socket_trails_bloom_sequence_tool {
        let expected_schema =
            if tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare {
                "FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult@1"
            } else {
                "FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult@1"
            };
        let sequence = value
            .get("sequence")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket trails Bloom sequence response is missing its record"
                    .to_owned()
            })?;
        let frames = sequence.get("frames").and_then(Value::as_array);
        let sequence_key = sequence.get("sequence_key_sha256").and_then(Value::as_str);
        let response_project = sequence.get("project_id").and_then(Value::as_str);
        let response_candidate = sequence.get("candidate_id").and_then(Value::as_str);
        let hash_is_valid = |hash: Option<&str>| {
            hash.is_some_and(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        };
        let frame_bounds_are_safe = frames.is_some_and(|items| {
            (1..=15).contains(&items.len())
                && items.iter().enumerate().all(|(index, frame)| {
                    frame.get("frame_index") == Some(&Value::from(index as u64))
                        && hash_is_valid(
                            frame
                                .get("trail_sequence_key_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_sequence_canonical_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_frame_canonical_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_color_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame.get("trail_id_object_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_depth_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("particle_sequence_frame_canonical_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("base_frame_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("bloom_key_sha256").and_then(Value::as_str))
                        && hash_is_valid(frame.get("camera_object_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame.get("camera_identity_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("render_profile_sha256").and_then(Value::as_str))
                        && hash_is_valid(
                            frame
                                .get("render_worker_build_cohort_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_bloom_profile_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("base_opaque_depth_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && frame.get("base_aov_byte_exact_verified") == Some(&Value::Bool(true))
                        && frame.get("base_opaque_depth_byte_exact_reused")
                            == Some(&Value::Bool(true))
                        && frame.get("bloom_pass_byte_exact_reused") == Some(&Value::Bool(true))
                        && frame.get("particle_passes_byte_exact_reused")
                            == Some(&Value::Bool(true))
                        && frame.get("trail_passes_byte_exact_reused") == Some(&Value::Bool(true))
                        && frame.get("base_bloom_mutated") == Some(&Value::Bool(false))
                        && frame.get("particle_passes_mutated") == Some(&Value::Bool(false))
                        && frame.get("trail_passes_mutated") == Some(&Value::Bool(false))
                        && frame.get("trail_bloom_input") == Some(&Value::Bool(true))
                        && frame.get("trail_emissive_source_rendered") == Some(&Value::Bool(true))
                        && frame.get("trail_bloom_contribution_rendered")
                            == Some(&Value::Bool(true))
                        && frame.get("trail_bloom_rendered") == Some(&Value::Bool(true))
                        && hash_is_valid(
                            frame.get("trail_bloom_key_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame.get("trail_bloom_seed_sha256").and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_emissive_source_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("trail_bloom_contribution_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(
                            frame
                                .get("render_set_object_sha256")
                                .and_then(Value::as_str),
                        )
                        && hash_is_valid(frame.get("receipt_object_sha256").and_then(Value::as_str))
                })
        });
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let sequence_flags_are_safe = sequence.get("runtime_write_performed")
            == Some(&Value::Bool(true))
            && sequence.get("restart_hash_verified") == Some(&Value::Bool(true))
            && sequence.get("candidate_confirmed") == Some(&Value::Bool(false))
            && sequence.get("version_created") == Some(&Value::Bool(false))
            && sequence.get("export_performed") == Some(&Value::Bool(false))
            && sequence.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
            && sequence.get("production_stage_advanced") == Some(&Value::Bool(false));
        let profile_is_fixed = sequence.get("trail_bloom_profile")
            == Some(&json!({
                "threshold":1,
                "source_gain":8,
                "radius_px":8,
                "intensity":4,
                "hdr_clamp":16,
                "blur_passes":2,
                "kernel":"separable-box-two-pass-fixed-radius@1"
            }));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !hash_is_valid(sequence_key)
            || value.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || value.get("replayed").and_then(Value::as_bool).is_none()
            || value.get("restart_hash_verified") != Some(&Value::Bool(true))
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepare,
                ))
            || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
            || value.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || !flags_are_safe
            || response_project.is_none()
            || response_candidate.is_none()
            || !frame_bounds_are_safe
            || sequence.get("schema_version").and_then(Value::as_str)
                != Some("FictionalEnergyVfxAnimatedSocketTrailsBloomSequence@1")
            || sequence.get("sequence_key_sha256").and_then(Value::as_str) != sequence_key
            || sequence.get("sequence_status").and_then(Value::as_str)
                != Some(
                    "runtime-owned-durable-fictional-energy-vfx-animated-socket-trails-bloom-sequence",
                )
            || sequence.get("frame_scope").and_then(Value::as_str)
                != Some("lod0-animation-trails-bloom-source-frames-1-15@1")
            || sequence
                .get("trails_bloom_sequence_policy")
                .and_then(Value::as_str)
                != Some("projection-driven-animated-socket-trails-bloom@1")
            || sequence.get("trail_key_scope").and_then(Value::as_str)
                != Some("animated-socket-trails-sequence-frame-binding@1")
            || sequence.get("trail_count") != Some(&Value::from(2_u64))
            || sequence.get("trail_emitter_roles")
                != Some(&json!(["muzzle-vfx", "energy-core-vfx"]))
            || !hash_is_valid(
                sequence
                    .get("trail_bloom_profile_sha256")
                    .and_then(Value::as_str),
            )
            || !profile_is_fixed
            || sequence.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || sequence.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || sequence
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || sequence.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || sequence.get("commercial_engine_status").and_then(Value::as_str) != Some("NOT_RUN")
            || !sequence_flags_are_safe
            || contains_forbidden_transport_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animated-socket trails Bloom sequence schema, exact upstream pass reuse, bounded frame hashes, side-effect flags or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != response_project
                || binding.candidate_id.as_deref() != response_candidate)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animated-socket trails Bloom sequence response crossed the bound project/candidate"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let v3_tool = matches!(
        tool,
        AgenticTool::ProductionStageTransitionV3Prepare
            | AgenticTool::ProductionStageTransitionV3Get
    );
    if v3_tool {
        const RESULT_FIELDS: [&str; 10] = [
            "schema_version",
            "transition",
            "production_stage_head",
            "compatibility_projection",
            "replayed",
            "runtime_write",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ];
        const TRANSITION_FIELDS: [&str; 62] = [
            "schema_version",
            "transition_id",
            "session_id",
            "project_id",
            "root_candidate_id",
            "root_candidate_role",
            "root_candidate_state_sha256",
            "source_artifact_id",
            "root_artifact_sha256",
            "previous_head_candidate_id",
            "previous_head_candidate_role",
            "previous_head_candidate_state_sha256",
            "previous_head_artifact_id",
            "previous_head_artifact_sha256",
            "previous_head_stage",
            "head_candidate_id",
            "head_candidate_role",
            "head_candidate_state_sha256",
            "output_artifact_id",
            "head_artifact_sha256",
            "from_stage",
            "to_stage",
            "candidate_binding_status",
            "reference_id",
            "reference_sha256",
            "camera_hash",
            "camera_lock_id",
            "camera_lock_canonical_sha256",
            "camera_rig_object_sha256",
            "camera_rig_canonical_sha256",
            "camera_lock_receipt_object_sha256",
            "camera_lock_source_transition_id",
            "camera_lock_source_transition_sha256",
            "camera_lock_source_head_canonical_sha256",
            "evidence_sha256",
            "reference_canvas_object_sha256",
            "quality_report_object_sha256",
            "comparison_report_object_sha256",
            "design_spec_object_sha256",
            "visual_receipt_object_sha256",
            "human_review_receipt_object_sha256",
            "engine_validation_receipt_object_sha256",
            "distribution_receipt_object_sha256",
            "structural_status",
            "visual_status",
            "human_status",
            "engine_status",
            "distribution_status",
            "approval_receipt_id",
            "approval_session_id",
            "approval_expires_at",
            "approval_summary_sha256",
            "request_key_sha256",
            "parent_transition_id",
            "parent_transition_sha256",
            "parent_transition_schema_version",
            "gate_status",
            "status",
            "input_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
            "created_at",
        ];
        const HEAD_FIELDS: [&str; 61] = [
            "schema_version",
            "session_id",
            "project_id",
            "root_candidate_id",
            "root_candidate_role",
            "root_candidate_state_sha256",
            "source_artifact_id",
            "root_artifact_sha256",
            "root_stage",
            "previous_head_candidate_id",
            "previous_head_candidate_role",
            "previous_head_candidate_state_sha256",
            "previous_head_artifact_id",
            "previous_head_artifact_sha256",
            "previous_head_stage",
            "head_candidate_id",
            "head_candidate_role",
            "head_candidate_state_sha256",
            "output_artifact_id",
            "head_artifact_sha256",
            "head_stage",
            "candidate_binding_status",
            "reference_id",
            "reference_sha256",
            "camera_hash",
            "camera_lock_id",
            "camera_lock_canonical_sha256",
            "camera_rig_object_sha256",
            "camera_rig_canonical_sha256",
            "camera_lock_receipt_object_sha256",
            "camera_lock_source_transition_id",
            "camera_lock_source_transition_sha256",
            "camera_lock_source_head_canonical_sha256",
            "evidence_sha256",
            "reference_canvas_object_sha256",
            "quality_report_object_sha256",
            "comparison_report_object_sha256",
            "design_spec_object_sha256",
            "visual_receipt_object_sha256",
            "human_review_receipt_object_sha256",
            "engine_validation_receipt_object_sha256",
            "distribution_receipt_object_sha256",
            "structural_status",
            "visual_status",
            "human_status",
            "engine_status",
            "distribution_status",
            "approval_receipt_id",
            "approval_session_id",
            "approval_expires_at",
            "approval_summary_sha256",
            "head_transition_id",
            "head_transition_sha256",
            "compatibility_projection",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "materialization_status",
            "canonical_sha256",
            "payload_json",
            "updated_at",
        ];
        const PROJECTION_FIELDS: [&str; 12] = [
            "schema_version",
            "source_schema_version",
            "v3_stage",
            "v3_stage_complete",
            "v1_projection_stage",
            "v1_projection_complete",
            "v2_projection_stage",
            "v2_projection_complete",
            "projection_status",
            "legacy_head_transition_id",
            "legacy_head_transition_sha256",
            "projection_policy_sha256",
        ];
        let expected_schema = if tool == AgenticTool::ProductionStageTransitionV3Prepare {
            "ProductionStageTransitionPrepareResult@3"
        } else {
            "ProductionStageTransitionGetResult@3"
        };
        let Some(transition) = value.get("transition").and_then(Value::as_object) else {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V3 response is missing transition".to_owned(),
            );
        };
        let Some(head) = value
            .get("production_stage_head")
            .and_then(Value::as_object)
        else {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V3 response is missing production_stage_head"
                    .to_owned(),
            );
        };
        let Some(projection) = value
            .get("compatibility_projection")
            .and_then(Value::as_object)
        else {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V3 response is missing compatibility_projection"
                    .to_owned(),
            );
        };
        let transition_root = transition.get("root_candidate_id").and_then(Value::as_str);
        let transition_head = transition.get("head_candidate_id").and_then(Value::as_str);
        let head_root = head.get("root_candidate_id").and_then(Value::as_str);
        let head_candidate = head.get("head_candidate_id").and_then(Value::as_str);
        let transition_session = transition.get("session_id").and_then(Value::as_str);
        let transition_project = transition.get("project_id").and_then(Value::as_str);
        let transition_id = transition.get("transition_id").and_then(Value::as_str);
        let transition_from = transition.get("from_stage").and_then(Value::as_str);
        let transition_to = transition.get("to_stage").and_then(Value::as_str);
        let first_edge = transition_from == Some("reference-intake")
            && transition_to == Some("reference-coverage-reviewed");
        let camera_edge = transition_from == Some("reference-coverage-reviewed")
            && transition_to == Some("camera-calibrated");
        let hash_fields = [
            "root_candidate_state_sha256",
            "root_artifact_sha256",
            "previous_head_candidate_state_sha256",
            "previous_head_artifact_sha256",
            "head_candidate_state_sha256",
            "head_artifact_sha256",
            "reference_sha256",
            "camera_hash",
            "evidence_sha256",
            "reference_canvas_object_sha256",
            "design_spec_object_sha256",
            "approval_summary_sha256",
            "request_key_sha256",
            "input_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
        ];
        let hashes_are_valid = hash_fields
            .iter()
            .all(|field| valid_sha256(transition.get(*field).and_then(Value::as_str)));
        let camera_hashes_are_valid = if first_edge {
            [
                "camera_lock_id",
                "camera_lock_canonical_sha256",
                "camera_rig_object_sha256",
                "camera_rig_canonical_sha256",
                "camera_lock_receipt_object_sha256",
                "camera_lock_source_transition_id",
                "camera_lock_source_transition_sha256",
                "camera_lock_source_head_canonical_sha256",
            ]
            .iter()
            .all(|field| transition.get(*field) == Some(&Value::Null))
        } else if camera_edge {
            valid_v2_id(transition.get("camera_lock_id").and_then(Value::as_str))
                && [
                    "camera_lock_canonical_sha256",
                    "camera_rig_object_sha256",
                    "camera_rig_canonical_sha256",
                    "camera_lock_receipt_object_sha256",
                    "camera_lock_source_transition_sha256",
                    "camera_lock_source_head_canonical_sha256",
                ]
                .iter()
                .all(|field| valid_sha256(transition.get(*field).and_then(Value::as_str)))
                && valid_v2_id(
                    transition
                        .get("camera_lock_source_transition_id")
                        .and_then(Value::as_str),
                )
        } else {
            false
        };
        let head_hash_fields = [
            "root_candidate_state_sha256",
            "root_artifact_sha256",
            "previous_head_candidate_state_sha256",
            "previous_head_artifact_sha256",
            "head_candidate_state_sha256",
            "head_artifact_sha256",
            "reference_sha256",
            "camera_hash",
            "evidence_sha256",
            "reference_canvas_object_sha256",
            "design_spec_object_sha256",
            "approval_summary_sha256",
            "head_transition_sha256",
            "canonical_sha256",
        ];
        let head_hashes_are_valid = head_hash_fields
            .iter()
            .all(|field| valid_sha256(head.get(*field).and_then(Value::as_str)));
        let head_camera_hashes_are_valid = if first_edge {
            [
                "camera_lock_id",
                "camera_lock_canonical_sha256",
                "camera_rig_object_sha256",
                "camera_rig_canonical_sha256",
                "camera_lock_receipt_object_sha256",
                "camera_lock_source_transition_id",
                "camera_lock_source_transition_sha256",
                "camera_lock_source_head_canonical_sha256",
            ]
            .iter()
            .all(|field| head.get(*field) == Some(&Value::Null))
        } else if camera_edge {
            valid_v2_id(head.get("camera_lock_id").and_then(Value::as_str))
                && [
                    "camera_lock_canonical_sha256",
                    "camera_rig_object_sha256",
                    "camera_rig_canonical_sha256",
                    "camera_lock_receipt_object_sha256",
                    "camera_lock_source_transition_sha256",
                    "camera_lock_source_head_canonical_sha256",
                ]
                .iter()
                .all(|field| valid_sha256(head.get(*field).and_then(Value::as_str)))
                && valid_v2_id(
                    head.get("camera_lock_source_transition_id")
                        .and_then(Value::as_str),
                )
        } else {
            false
        };
        let head_ids_are_non_empty = [
            "session_id",
            "project_id",
            "root_candidate_id",
            "root_candidate_role",
            "source_artifact_id",
            "previous_head_candidate_id",
            "previous_head_candidate_role",
            "previous_head_artifact_id",
            "head_candidate_id",
            "head_candidate_role",
            "output_artifact_id",
            "reference_id",
            "approval_receipt_id",
            "approval_session_id",
            "head_transition_id",
        ]
        .iter()
        .all(|field| {
            head.get(*field)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
        });
        let optional_receipts_are_null = [
            "quality_report_object_sha256",
            "comparison_report_object_sha256",
            "visual_receipt_object_sha256",
            "human_review_receipt_object_sha256",
            "engine_validation_receipt_object_sha256",
            "distribution_receipt_object_sha256",
        ]
        .iter()
        .all(|field| transition.get(*field) == Some(&Value::Null));
        let parent_fields_are_valid = if first_edge {
            [
                "parent_transition_id",
                "parent_transition_sha256",
                "parent_transition_schema_version",
            ]
            .iter()
            .all(|field| transition.get(*field) == Some(&Value::Null))
        } else {
            valid_v2_id(
                transition
                    .get("parent_transition_id")
                    .and_then(Value::as_str),
            ) && valid_sha256(
                transition
                    .get("parent_transition_sha256")
                    .and_then(Value::as_str),
            ) && transition
                .get("parent_transition_schema_version")
                .and_then(Value::as_str)
                == Some("ProductionStageTransition@3")
        };
        let head_optional_receipts_are_null = [
            "quality_report_object_sha256",
            "comparison_report_object_sha256",
            "visual_receipt_object_sha256",
            "human_review_receipt_object_sha256",
            "engine_validation_receipt_object_sha256",
            "distribution_receipt_object_sha256",
        ]
        .iter()
        .all(|field| head.get(*field) == Some(&Value::Null));
        let head_lineage_matches = [
            ("root_candidate_role", "root_candidate_role"),
            ("root_candidate_state_sha256", "root_candidate_state_sha256"),
            ("source_artifact_id", "source_artifact_id"),
            ("root_artifact_sha256", "root_artifact_sha256"),
            ("previous_head_candidate_id", "previous_head_candidate_id"),
            (
                "previous_head_candidate_role",
                "previous_head_candidate_role",
            ),
            (
                "previous_head_candidate_state_sha256",
                "previous_head_candidate_state_sha256",
            ),
            ("previous_head_artifact_id", "previous_head_artifact_id"),
            (
                "previous_head_artifact_sha256",
                "previous_head_artifact_sha256",
            ),
            ("head_candidate_role", "head_candidate_role"),
            ("head_candidate_state_sha256", "head_candidate_state_sha256"),
            ("output_artifact_id", "output_artifact_id"),
            ("head_artifact_sha256", "head_artifact_sha256"),
            ("candidate_binding_status", "candidate_binding_status"),
            ("reference_id", "reference_id"),
            ("reference_sha256", "reference_sha256"),
            ("camera_hash", "camera_hash"),
            ("camera_lock_id", "camera_lock_id"),
            (
                "camera_lock_canonical_sha256",
                "camera_lock_canonical_sha256",
            ),
            ("camera_rig_object_sha256", "camera_rig_object_sha256"),
            ("camera_rig_canonical_sha256", "camera_rig_canonical_sha256"),
            (
                "camera_lock_receipt_object_sha256",
                "camera_lock_receipt_object_sha256",
            ),
            (
                "camera_lock_source_transition_id",
                "camera_lock_source_transition_id",
            ),
            (
                "camera_lock_source_transition_sha256",
                "camera_lock_source_transition_sha256",
            ),
            (
                "camera_lock_source_head_canonical_sha256",
                "camera_lock_source_head_canonical_sha256",
            ),
            ("evidence_sha256", "evidence_sha256"),
            (
                "reference_canvas_object_sha256",
                "reference_canvas_object_sha256",
            ),
            ("design_spec_object_sha256", "design_spec_object_sha256"),
            ("structural_status", "structural_status"),
            ("visual_status", "visual_status"),
            ("human_status", "human_status"),
            ("engine_status", "engine_status"),
            ("distribution_status", "distribution_status"),
            ("approval_receipt_id", "approval_receipt_id"),
            ("approval_session_id", "approval_session_id"),
            ("approval_expires_at", "approval_expires_at"),
            ("approval_summary_sha256", "approval_summary_sha256"),
        ]
        .iter()
        .all(|(head_field, transition_field)| {
            head.get(*head_field) == transition.get(*transition_field)
        });
        let expected_projection = json!({
            "schema_version":"ProductionStageCompatibilityProjection@3",
            "source_schema_version":"ProductionStageHead@3",
            "v3_stage":transition_to,
            "v3_stage_complete":true,
            "v1_projection_stage":null,
            "v1_projection_complete":false,
            "v2_projection_stage":null,
            "v2_projection_complete":false,
            "projection_status":"not-proven",
            "legacy_head_transition_id":null,
            "legacy_head_transition_sha256":null,
            "projection_policy_sha256":projection.get("projection_policy_sha256")
        });
        let fixed_projection = projection.get("schema_version").and_then(Value::as_str)
            == Some("ProductionStageCompatibilityProjection@3")
            && projection
                .get("source_schema_version")
                .and_then(Value::as_str)
                == Some("ProductionStageHead@3")
            && projection.get("v3_stage").and_then(Value::as_str) == transition_to
            && projection.get("v3_stage_complete") == Some(&Value::Bool(true))
            && projection.get("v1_projection_stage") == Some(&Value::Null)
            && projection.get("v1_projection_complete") == Some(&Value::Bool(false))
            && projection.get("v2_projection_stage") == Some(&Value::Null)
            && projection.get("v2_projection_complete") == Some(&Value::Bool(false))
            && projection.get("projection_status").and_then(Value::as_str) == Some("not-proven")
            && projection.get("legacy_head_transition_id") == Some(&Value::Null)
            && projection.get("legacy_head_transition_sha256") == Some(&Value::Null)
            && projection
                .get("projection_policy_sha256")
                .and_then(Value::as_str)
                == Some("3855241e8e3bba0b4966beda1f29ee7aea5e54eb6d66bc5aa961cec6d738d9f6");
        let flags_are_safe = ["candidate_confirmed", "version_created", "export_performed"]
            .iter()
            .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        let side_effects = value.get("production_stage_advanced") == Some(&Value::Bool(true))
            && value.get("runtime_write")
                == Some(&Value::Bool(
                    tool == AgenticTool::ProductionStageTransitionV3Prepare,
                ))
            && value.get("replayed").is_some_and(Value::is_boolean);
        let fixed_transition = transition.get("schema_version").and_then(Value::as_str)
            == Some("ProductionStageTransition@3")
            && transition
                .get("root_candidate_role")
                .and_then(Value::as_str)
                == Some("reference-intake-candidate")
            && transition
                .get("previous_head_candidate_role")
                .and_then(Value::as_str)
                == Some("reference-intake-candidate")
            && transition
                .get("head_candidate_role")
                .and_then(Value::as_str)
                == Some("reference-intake-candidate")
            && ((first_edge
                && transition
                    .get("previous_head_stage")
                    .and_then(Value::as_str)
                    == Some("reference-intake"))
                || (camera_edge
                    && transition
                        .get("previous_head_stage")
                        .and_then(Value::as_str)
                        == Some("reference-coverage-reviewed")))
            && (first_edge || camera_edge)
            && transition
                .get("candidate_binding_status")
                .and_then(Value::as_str)
                == Some("same-candidate-evidence")
            && transition.get("structural_status").and_then(Value::as_str)
                == Some("PASS_SOURCE_STRUCTURAL")
            && transition.get("visual_status").and_then(Value::as_str)
                == Some("QUALITY_TARGET_NOT_MET")
            && transition.get("human_status").and_then(Value::as_str) == Some("NOT_RUN")
            && transition.get("engine_status").and_then(Value::as_str) == Some("NOT_RUN")
            && transition
                .get("distribution_status")
                .and_then(Value::as_str)
                == Some("NOT_RUN")
            && transition.get("gate_status").and_then(Value::as_str) == Some("pass")
            && transition.get("status").and_then(Value::as_str) == Some("passed")
            && transition
                .get("created_at")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty());
        let fixed_head = head.get("schema_version").and_then(Value::as_str)
            == Some("ProductionStageHead@3")
            && head.get("root_stage").and_then(Value::as_str) == Some("reference-intake")
            && head.get("head_stage").and_then(Value::as_str) == transition_to
            && head.get("candidate_binding_status").and_then(Value::as_str)
                == Some("same-candidate-evidence")
            && head.get("structural_status").and_then(Value::as_str)
                == Some("PASS_SOURCE_STRUCTURAL")
            && head.get("visual_status").and_then(Value::as_str) == Some("QUALITY_TARGET_NOT_MET")
            && head.get("human_status").and_then(Value::as_str) == Some("NOT_RUN")
            && head.get("engine_status").and_then(Value::as_str) == Some("NOT_RUN")
            && head.get("distribution_status").and_then(Value::as_str) == Some("NOT_RUN")
            && head.get("candidate_confirmed") == Some(&Value::Bool(false))
            && head.get("version_created") == Some(&Value::Bool(false))
            && head.get("export_performed") == Some(&Value::Bool(false))
            && head.get("head_transition_id").and_then(Value::as_str) == transition_id
            && head.get("materialization_status").and_then(Value::as_str)
                == Some("runtime-owned-durable-production-stage-head-v3")
            && valid_sha256(head.get("canonical_sha256").and_then(Value::as_str))
            && head
                .get("payload_json")
                .and_then(Value::as_str)
                .is_some_and(|payload| !payload.is_empty())
            && head
                .get("updated_at")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty());
        let same_binding = transition_root.is_some()
            && transition_root == transition_head
            && transition_root == head_root
            && transition_head == head_candidate
            && head.get("head_transition_id").and_then(Value::as_str) == transition_id
            && head.get("session_id").and_then(Value::as_str) == transition_session
            && head.get("project_id").and_then(Value::as_str) == transition_project;
        if !object_has_exact_fields(value.as_object().expect("checked object"), &RESULT_FIELDS)
            || !object_has_exact_fields(transition, &TRANSITION_FIELDS)
            || !object_has_exact_fields(head, &HEAD_FIELDS)
            || !object_has_exact_fields(projection, &PROJECTION_FIELDS)
            || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || !fixed_transition
            || !fixed_head
            || !same_binding
            || !hashes_are_valid
            || !head_hashes_are_valid
            || !camera_hashes_are_valid
            || !head_camera_hashes_are_valid
            || !head_ids_are_non_empty
            || !optional_receipts_are_null
            || !parent_fields_are_valid
            || !head_optional_receipts_are_null
            || !head_lineage_matches
            || !fixed_projection
            || value.get("compatibility_projection") != head.get("compatibility_projection")
            || Value::Object(projection.clone()) != expected_projection
            || !side_effects
            || !flags_are_safe
            || contains_forbidden_transport_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V3 response schema, strict closed fields, same-candidate binding, fixed statuses or side-effect flags differ"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.session_id.as_deref() != transition_session
                || binding.project_id.as_deref() != transition_project
                || binding.candidate_id.as_deref() != transition_root)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V3 response crossed the session/project/candidate binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let v2_tool = matches!(
        tool,
        AgenticTool::ProductionStageTransitionV2Prepare
            | AgenticTool::ProductionStageTransitionV2Get
    );
    if v2_tool {
        let expected_schema = if tool == AgenticTool::ProductionStageTransitionV2Prepare {
            "ProductionStageTransitionPrepareResult@2"
        } else {
            "ProductionStageTransitionGetResult@2"
        };
        let transition = value
            .get("transition")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 response is missing transition".to_owned()
            })?;
        let head = value
            .get("production_stage_head")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 response is missing production_stage_head"
                    .to_owned()
            })?;
        let transition_root = transition.get("root_candidate_id").and_then(Value::as_str);
        let transition_head = transition.get("head_candidate_id").and_then(Value::as_str);
        let head_root = head.get("root_candidate_id").and_then(Value::as_str);
        let head_candidate = head.get("head_candidate_id").and_then(Value::as_str);
        let transition_id = transition.get("transition_id").and_then(Value::as_str);
        let flags_are_safe = ["candidate_confirmed", "version_created", "export_performed"]
            .iter()
            .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::ProductionStageTransitionV2Prepare,
                ))
            || value.get("production_stage_advanced") != Some(&Value::Bool(true))
            || !flags_are_safe
            || transition.get("schema_version").and_then(Value::as_str)
                != Some("ProductionStageTransition@2")
            || transition
                .get("root_candidate_role")
                .and_then(Value::as_str)
                != Some("topology-source")
            || transition
                .get("head_candidate_role")
                .and_then(Value::as_str)
                != Some("material-surface-output")
            || transition.get("from_stage").and_then(Value::as_str) != Some("topology")
            || transition.get("to_stage").and_then(Value::as_str) != Some("material-surface")
            || transition
                .get("candidate_binding_status")
                .and_then(Value::as_str)
                != Some("distinct-root-topology-to-material-surface-head")
            || transition
                .get("topology_quality_status")
                .and_then(Value::as_str)
                != Some("passed")
            || transition
                .get("material_surface_quality_status")
                .and_then(Value::as_str)
                != Some("passed")
            || transition.get("gate_status").and_then(Value::as_str) != Some("pass")
            || transition.get("status").and_then(Value::as_str) != Some("passed")
            || head.get("schema_version").and_then(Value::as_str) != Some("ProductionStageHead@2")
            || head.get("root_candidate_role").and_then(Value::as_str) != Some("topology-source")
            || head.get("root_stage").and_then(Value::as_str) != Some("topology")
            || head.get("head_candidate_role").and_then(Value::as_str)
                != Some("material-surface-output")
            || head.get("head_stage").and_then(Value::as_str) != Some("material-surface")
            || head.get("candidate_binding_status").and_then(Value::as_str)
                != Some("distinct-root-topology-to-material-surface-head")
            || head.get("topology_quality_status").and_then(Value::as_str) != Some("passed")
            || head
                .get("material_surface_quality_status")
                .and_then(Value::as_str)
                != Some("passed")
            || head.get("candidate_confirmed") != Some(&Value::Bool(false))
            || head.get("version_created") != Some(&Value::Bool(false))
            || head.get("export_performed") != Some(&Value::Bool(false))
            || transition_root.is_none()
            || transition_head.is_none()
            || head_root.is_none()
            || head_candidate.is_none()
            || transition_root == transition_head
            || transition_root != head_root
            || transition_head != head_candidate
            || head.get("head_transition_id").and_then(Value::as_str) != transition_id
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 response schema, dual-candidate binding or side-effect flags differ"
                    .to_owned(),
            );
        }
        let transition_session = transition.get("session_id").and_then(Value::as_str);
        let transition_project = transition.get("project_id").and_then(Value::as_str);
        if head.get("session_id").and_then(Value::as_str) != transition_session
            || head.get("project_id").and_then(Value::as_str) != transition_project
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 transition/head session and project bindings differ"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.session_id.as_deref() != transition_session
                || binding.project_id.as_deref() != transition_project
                || binding.candidate_id.as_deref() != transition_root)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: V2 response crossed the session/project/topology-root binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let animation_vfx_tool = matches!(
        tool,
        AgenticTool::CandidateAnimationVfxQualityPrepare
            | AgenticTool::CandidateAnimationVfxQualityGet
    );
    if animation_vfx_tool {
        let expected_schema = if tool == AgenticTool::CandidateAnimationVfxQualityPrepare {
            "CandidateAnimationVfxQualityPrepareResult@1"
        } else {
            "CandidateAnimationVfxQualityGetResult@1"
        };
        let quality = value
            .get("animation_vfx_quality")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animation-vfx quality response is missing its record"
                    .to_owned()
            })?;
        let hard_gate = quality
            .get("hard_gate")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animation-vfx quality response is missing hard_gate"
                    .to_owned()
            })?;
        const HARD_GATE_FIELDS: [&str; 20] = [
            "material_surface_head_binding",
            "material_surface_quality",
            "delivery_lod0_binding",
            "anchor_set_binding",
            "animation_clip_binding",
            "animation_glb_readback",
            "animated_socket_readback",
            "vfx_profile_binding",
            "base_frame_stack",
            "bloom_stack",
            "particle_stack",
            "trail_stack",
            "trail_bloom_stack",
            "cross_layer_parent_binding",
            "sample_camera_binding",
            "worker_cohort_binding",
            "render_pass_byte_exact",
            "bounded_resource_policy",
            "vfx_glb_socket_attachment",
            "nonfunctional_scope",
        ];
        let all_hard_gates = hard_gate.len() == HARD_GATE_FIELDS.len()
            && HARD_GATE_FIELDS
                .iter()
                .all(|field| hard_gate.get(*field) == Some(&Value::Bool(true)));
        let hard_gate_passed = quality.get("hard_gate_passed").and_then(Value::as_bool);
        let validator_status = quality.get("validator_status").and_then(Value::as_str);
        let flags_are_safe = [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ]
        .iter()
        .all(|field| value.get(*field) == Some(&Value::Bool(false)));
        if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::CandidateAnimationVfxQualityPrepare,
                ))
            || !flags_are_safe
            || quality.get("schema_version").and_then(Value::as_str)
                != Some("CandidateAnimationVfxQuality@1")
            || quality
                .get("candidate_binding_status")
                .and_then(Value::as_str)
                != Some("same-material-surface-head-candidate-no-geometry-mutation")
            || quality.get("from_stage").and_then(Value::as_str) != Some("material-surface")
            || quality.get("to_stage").and_then(Value::as_str) != Some("animation-vfx")
            || quality.get("quality_status").and_then(Value::as_str) != Some("structural_only")
            || quality.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
            || quality
                .get("artistic_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || quality.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
            || quality
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                != Some("NOT_PROVEN")
            || quality
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                != Some("NOT_RUN")
            || quality.get("actual_engine_roundtrip") != Some(&Value::Bool(false))
            || quality.get("functional_semantics") != Some(&Value::Bool(false))
            || quality.get("runtime_write_performed") != Some(&Value::Bool(true))
            || hard_gate_passed != Some(all_hard_gates)
            || validator_status != Some(if all_hard_gates { "passed" } else { "failed" })
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animation-vfx quality schema, technical gate or truthful side-effect boundary differs"
                    .to_owned(),
            );
        }
        let response_project = quality.get("project_id").and_then(Value::as_str);
        let response_candidate = quality.get("candidate_id").and_then(Value::as_str);
        if response_project.is_none() || response_candidate.is_none() {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: animation-vfx quality scope is missing".to_owned(),
            );
        }
        if binding.is_bound() && binding.project_id.as_deref() != response_project {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: animation-vfx quality response crossed the bound project"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let material_surface_tool = matches!(
        tool,
        AgenticTool::CandidateMaterialSurfaceQualityPrepare
            | AgenticTool::CandidateMaterialSurfaceQualityGet
    );
    if material_surface_tool {
        let source_candidate_id = find_string(value, "source_candidate_id", 0);
        let output_candidate_id = find_string(value, "output_candidate_id", 0);
        let expected_schema = if tool == AgenticTool::CandidateMaterialSurfaceQualityPrepare {
            "CandidateMaterialSurfaceQualityPrepareResult@1"
        } else {
            "CandidateMaterialSurfaceQualityGetResult@1"
        };
        if project_id.is_none()
            || source_candidate_id.is_none()
            || output_candidate_id.is_none()
            || source_candidate_id == output_candidate_id
            || value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
            || value.get("runtime_write")
                != Some(&Value::Bool(
                    tool == AgenticTool::CandidateMaterialSurfaceQualityPrepare,
                ))
            || [
                "production_stage_advanced",
                "candidate_confirmed",
                "version_created",
                "export_performed",
            ]
            .iter()
            .any(|field| value.get(*field) != Some(&Value::Bool(false)))
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: material-surface quality response schema, side-effect flags or distinct source/output binding differ"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != project_id
                || binding.candidate_id.as_deref() != source_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: material-surface quality response crossed the project/topology-source candidate binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    let topology_tool = matches!(
        tool,
        AgenticTool::CandidateTopologyQualityPrepare | AgenticTool::CandidateTopologyQualityGet
    );
    if topology_tool {
        if project_id.is_none() || candidate_id.is_none() {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: topology quality response is missing project/candidate binding"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != project_id
                || binding.candidate_id.as_deref() != candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: topology quality response crossed the project/candidate binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }
    if session_id.is_none() || project_id.is_none() || candidate_id.is_none() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: Runtime response is missing session/project/candidate binding"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != session_id
            || binding.project_id.as_deref() != project_id
            || binding.candidate_id.as_deref() != candidate_id)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: Runtime response crossed the session project/candidate binding"
                .to_owned(),
        );
    }
    if let Some(result) =
        agentic_write_tools_session::validate_response_binding(tool, value, binding, session_id)
    {
        return result;
    }
    Ok(())
}

fn validate_form_evidence_binding(value: &Value, label: &str) -> Result<(), String> {
    const FIELDS: [&str; 4] = [
        "source_kind",
        "source_object_sha256",
        "evidence_object_sha256",
        "status",
    ];
    let object = value.as_object().ok_or_else(|| {
        format!("AGENTIC_RUNTIME_OUTPUT_INVALID: {label} binding must be an object")
    })?;
    if !object_has_exact_fields(object, &FIELDS) {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: {label} binding fields differ"
        ));
    }
    let source_kind = value.get("source_kind").and_then(Value::as_str);
    let status = value.get("status").and_then(Value::as_str);
    if !matches!(
        status,
        Some("PASS" | "QUALITY_TARGET_NOT_MET" | "NOT_PROVEN" | "BLOCKED")
    ) {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: {label} status differs"
        ));
    }
    match source_kind {
        Some("not-proven") => {
            if !value["source_object_sha256"].is_null()
                || !value["evidence_object_sha256"].is_null()
                || status != Some("NOT_PROVEN")
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: {label} NOT_PROVEN binding differs"
                ));
            }
        }
        Some("cross-view-evidence-bundle" | "design-spec") => {
            if !valid_sha256(value["source_object_sha256"].as_str())
                || !valid_sha256(value["evidence_object_sha256"].as_str())
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: {label} evidence hashes are invalid"
                ));
            }
        }
        _ => {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: {label} source kind differs"
            ));
        }
    }
    Ok(())
}

fn validate_form_view_evaluations(value: &Value) -> Result<(), String> {
    const VIEW_FIELDS: [&str; 6] = [
        "view_kind",
        "view_id",
        "part_id_evidence",
        "negative_space_evidence",
        "line_flow_evidence",
        "no_regression",
    ];
    const PART_FIELDS: [&str; 6] = [
        "source",
        "expected_part_ids",
        "observed_part_ids",
        "missing_part_ids",
        "unexpected_part_ids",
        "coverage_milli",
    ];
    const NEGATIVE_FIELDS: [&str; 6] = [
        "source",
        "expected_count",
        "observed_count",
        "missing_count",
        "sealed_count",
        "coverage_milli",
    ];
    const LINE_FIELDS: [&str; 6] = [
        "source",
        "expected_count",
        "observed_count",
        "coverage_milli",
        "continuity_milli",
        "deviation_milli",
    ];
    const NO_REGRESSION_FIELDS: [&str; 5] = [
        "status",
        "metrics_not_regressed",
        "part_id_not_regressed",
        "negative_space_not_regressed",
        "line_flow_not_regressed",
    ];
    const VIEW_KINDS: [&str; 6] = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    let views = value.as_array().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form view evaluations must be an array".to_owned()
    })?;
    if views.len() != VIEW_KINDS.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form view evaluation count differs".to_owned(),
        );
    }
    for (view, expected_kind) in views.iter().zip(VIEW_KINDS) {
        let object = view.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form view evaluation must be an object".to_owned()
        })?;
        if !object_has_exact_fields(object, &VIEW_FIELDS)
            || view.get("view_kind").and_then(Value::as_str) != Some(expected_kind)
            || view
                .get("view_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form view identity or fields differ".to_owned(),
            );
        }
        let part = view["part_id_evidence"]
            .as_object()
            .ok_or_else(|| "AGENTIC_RUNTIME_OUTPUT_INVALID: Part-ID evidence missing".to_owned())?;
        let negative = view["negative_space_evidence"].as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space evidence missing".to_owned()
        })?;
        let line = view["line_flow_evidence"].as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow evidence missing".to_owned()
        })?;
        let no_regression = view["no_regression"].as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: no-regression evidence missing".to_owned()
        })?;
        if !object_has_exact_fields(part, &PART_FIELDS)
            || !object_has_exact_fields(negative, &NEGATIVE_FIELDS)
            || !object_has_exact_fields(line, &LINE_FIELDS)
            || !object_has_exact_fields(no_regression, &NO_REGRESSION_FIELDS)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: nested form evidence fields differ".to_owned(),
            );
        }
        validate_form_evidence_binding(&view["part_id_evidence"]["source"], "Part-ID")?;
        validate_form_evidence_binding(
            &view["negative_space_evidence"]["source"],
            "negative-space",
        )?;
        validate_form_evidence_binding(&view["line_flow_evidence"]["source"], "line-flow")?;
        for field in [
            "expected_part_ids",
            "observed_part_ids",
            "missing_part_ids",
            "unexpected_part_ids",
        ] {
            let items = view["part_id_evidence"][field].as_array().ok_or_else(|| {
                format!("AGENTIC_RUNTIME_OUTPUT_INVALID: Part-ID {field} must be an array")
            })?;
            if items.len() > 512
                || items
                    .iter()
                    .any(|item| item.as_str().is_none_or(str::is_empty))
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: Part-ID {field} differs"
                ));
            }
        }
        for field in ["coverage_milli"] {
            if view["part_id_evidence"][field]
                .as_u64()
                .is_none_or(|metric| metric > 1000)
            {
                return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: Part-ID coverage differs".to_owned());
            }
        }
        for field in [
            "expected_count",
            "observed_count",
            "missing_count",
            "sealed_count",
        ] {
            if view["negative_space_evidence"][field]
                .as_u64()
                .is_none_or(|count| count > 512)
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space {field} differs"
                ));
            }
        }
        if view["negative_space_evidence"]["coverage_milli"]
            .as_u64()
            .is_none_or(|metric| metric > 1000)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space coverage differs".to_owned(),
            );
        }
        for field in ["expected_count", "observed_count"] {
            if view["line_flow_evidence"][field]
                .as_u64()
                .is_none_or(|count| count > 512)
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow {field} differs"
                ));
            }
        }
        for field in ["coverage_milli", "continuity_milli"] {
            if view["line_flow_evidence"][field]
                .as_u64()
                .is_none_or(|metric| metric > 1000)
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow {field} differs"
                ));
            }
        }
        if view["line_flow_evidence"]["deviation_milli"]
            .as_u64()
            .is_none_or(|metric| metric > 100_000)
        {
            return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow deviation differs".to_owned());
        }
        if !matches!(
            view["no_regression"]["status"].as_str(),
            Some("NOT_APPLICABLE" | "NOT_PROVEN" | "PASS" | "FAILED")
        ) || [
            "metrics_not_regressed",
            "part_id_not_regressed",
            "negative_space_not_regressed",
            "line_flow_not_regressed",
        ]
        .iter()
        .any(|field| view["no_regression"][field].as_bool().is_none())
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: no-regression projection differs".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_production_weapon_form_evidence_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::ProductionWeaponFormEvidencePrepare;
    const RESULT_FIELDS_PREPARE: [&str; 8] = [
        "schema_version",
        "form_evidence",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    const RESULT_FIELDS_GET: [&str; 9] = [
        "schema_version",
        "form_evidence",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "restart_hash_verified",
    ];
    const RECORD_FIELDS_EXACT: [&str; 35] = [
        "schema_version",
        "form_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "view_kinds",
        "views",
        "evidence_policy",
        "evidence_policy_sha256",
        "quality_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
        "export_performed",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence result must be an object".to_owned()
    })?;
    let result_fields = if is_prepare {
        &RESULT_FIELDS_PREPARE[..]
    } else {
        &RESULT_FIELDS_GET[..]
    };
    if !object_has_exact_fields(object, result_fields) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence result fields differ".to_owned(),
        );
    }
    let expected_schema = if is_prepare {
        "ProductionWeaponFormEvidencePrepareResult@1"
    } else {
        "ProductionWeaponFormEvidenceGetResult@1"
    };
    let expected_evidence_policy_sha256 = forgecad_runtime::sha256_hex(
        b"production-weapon-form-evidence-six-view-typed-observation@1",
    );
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || (!is_prepare && value.get("restart_hash_verified") != Some(&Value::Bool(true)))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence side-effect or restart flags differ"
                .to_owned(),
        );
    }
    let evidence = value
        .get("form_evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence record missing".to_owned())?;
    if !object_has_exact_fields(evidence, &RECORD_FIELDS_EXACT)
        || evidence.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormEvidence@1")
        || evidence.get("evidence_policy").and_then(Value::as_str)
            != Some("production-weapon-form-evidence-six-view-typed-observation@1")
        || evidence
            .get("evidence_policy_sha256")
            .and_then(Value::as_str)
            != Some(expected_evidence_policy_sha256.as_str())
        || evidence.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || evidence.get("runtime_write_performed") != Some(&Value::Bool(true))
        || evidence.get("production_stage_advanced") != Some(&Value::Bool(false))
        || evidence.get("candidate_confirmed") != Some(&Value::Bool(false))
        || evidence.get("version_created") != Some(&Value::Bool(false))
        || evidence.get("export_performed") != Some(&Value::Bool(false))
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence record status, fields or media boundary differs"
                .to_owned(),
        );
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "evidence_policy_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(evidence.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence hash {field} is invalid"
            ));
        }
    }
    for field in [
        "form_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
    ] {
        if !valid_v2_id(evidence.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence id {field} is invalid"
            ));
        }
    }
    let expected_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    if evidence.get("view_kinds") != Some(&json!(expected_views)) {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view kinds differ".to_owned());
    }
    let views = evidence
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence views missing".to_owned())?;
    if views.len() != expected_views.len() {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view count differs".to_owned());
    }
    let mut view_ids = BTreeSet::new();
    for (view, expected_kind) in views.iter().zip(expected_views) {
        validate_production_weapon_form_evidence_view(
            view,
            expected_kind,
            evidence,
            &mut view_ids,
        )?;
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != evidence.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != evidence.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != evidence.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: form-evidence response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_production_weapon_form_evidence_view(
    value: &Value,
    expected_kind: &str,
    parent: &Map<String, Value>,
    view_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    const VIEW_FIELDS: [&str; 23] = [
        "schema_version",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "view_kind",
        "view_id",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "render_set_object_sha256",
        "render_set_canonical_sha256",
        "render_set_view_id",
        "part_id_evidence",
        "negative_space_evidence",
        "line_flow_evidence",
        "view_observation_status",
        "quality_status",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view must be an object".to_owned()
    })?;
    if !object_has_exact_fields(object, &VIEW_FIELDS)
        || object.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormEvidenceView@1")
        || object.get("view_kind").and_then(Value::as_str) != Some(expected_kind)
        || !matches!(
            object
                .get("view_observation_status")
                .and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || object.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || object
            .get("created_at")
            .and_then(Value::as_str)
            .is_none_or(|s| s.is_empty() || s.len() > 128)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view identity/status differs".to_owned(),
        );
    }
    for field in [
        "project_id",
        "candidate_id",
        "artifact_id",
        "view_id",
        "reference_id",
        "render_set_view_id",
    ] {
        if !valid_v2_id(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view id {field} is invalid"
            ));
        }
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "render_set_object_sha256",
        "render_set_canonical_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view hash {field} is invalid"
            ));
        }
    }
    if object.get("project_id") != parent.get("project_id")
        || object.get("candidate_id") != parent.get("candidate_id")
        || object.get("candidate_state_sha256") != parent.get("candidate_state_sha256")
        || object.get("artifact_id") != parent.get("artifact_id")
        || object.get("artifact_sha256") != parent.get("artifact_sha256")
        || object.get("render_set_view_id") != object.get("view_id")
        || !view_ids.insert(object["view_id"].as_str().unwrap_or_default().to_owned())
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence view lineage differs".to_owned(),
        );
    }
    validate_production_weapon_form_evidence_observation(
        object.get("part_id_evidence"),
        "part-id",
        true,
    )?;
    validate_production_weapon_form_evidence_observation(
        object.get("negative_space_evidence"),
        "negative-space",
        false,
    )?;
    validate_production_weapon_form_evidence_observation(
        object.get("line_flow_evidence"),
        "line-flow",
        false,
    )?;
    Ok(())
}

fn validate_production_weapon_form_evidence_observation(
    value: Option<&Value>,
    expected_kind: &str,
    part_ids: bool,
) -> Result<(), String> {
    let object = value.and_then(Value::as_object).ok_or_else(|| {
        format!("AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} evidence missing")
    })?;
    let expected_fields: &[&str] = if part_ids {
        &[
            "observation",
            "expected_part_ids",
            "observed_part_ids",
            "missing_part_ids",
            "unexpected_part_ids",
            "coverage_milli",
        ]
    } else if expected_kind == "negative-space" {
        &[
            "observation",
            "expected_count",
            "observed_count",
            "missing_count",
            "sealed_count",
            "coverage_milli",
        ]
    } else {
        &[
            "observation",
            "expected_count",
            "observed_count",
            "coverage_milli",
            "continuity_milli",
            "deviation_milli",
        ]
    };
    if !object_has_exact_fields(object, expected_fields) {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} fields differ"
        ));
    }
    let observation = object
        .get("observation")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} observation missing"
            )
        })?;
    const OBSERVATION_FIELDS: [&str; 3] = ["evidence_kind", "observation_status", "quality_status"];
    if !object_has_exact_fields(observation, &OBSERVATION_FIELDS)
        || observation.get("evidence_kind").and_then(Value::as_str) != Some(expected_kind)
        || !matches!(
            observation
                .get("observation_status")
                .and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || observation.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
    {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} observation differs"
        ));
    }
    if part_ids {
        for field in [
            "expected_part_ids",
            "observed_part_ids",
            "missing_part_ids",
            "unexpected_part_ids",
        ] {
            let values = object.get(field).and_then(Value::as_array).ok_or_else(|| {
                format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} {field} differs"
                )
            })?;
            if values.len() > 512 || values.iter().any(|value| !valid_v2_id(value.as_str())) {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} {field} differs"
                ));
            }
        }
        if object
            .get("coverage_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence Part-ID coverage differs".to_owned(),
            );
        }
    } else {
        for field in ["expected_count", "observed_count"] {
            if object
                .get(field)
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 512)
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} {field} differs"
                ));
            }
        }
        if object
            .get("coverage_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence {expected_kind} coverage differs"
            ));
        }
        if expected_kind == "negative-space"
            && ["missing_count", "sealed_count"].iter().any(|field| {
                object
                    .get(*field)
                    .and_then(Value::as_u64)
                    .is_none_or(|value| value > 512)
            })
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence negative-space counts differ"
                    .to_owned(),
            );
        }
        if expected_kind == "line-flow"
            && object
                .get("continuity_milli")
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 1000)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence line-flow continuity differs"
                    .to_owned(),
            );
        }
        if expected_kind == "line-flow"
            && object
                .get("deviation_milli")
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 100_000)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-evidence line-flow deviation differs"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_production_weapon_form_art_evidence_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::ProductionWeaponFormArtEvidencePrepare;
    const RESULT_FIELDS_PREPARE: [&str; 8] = [
        "schema_version",
        "art_evidence",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    const RESULT_FIELDS_GET: [&str; 9] = [
        "schema_version",
        "art_evidence",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "restart_hash_verified",
    ];
    const RECORD_FIELDS: [&str; 38] = [
        "schema_version",
        "art_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "view_kinds",
        "views",
        "part_id_aggregate",
        "art_evidence_policy",
        "art_evidence_policy_sha256",
        "quality_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence result must be an object".to_owned()
    })?;
    let result_fields = if is_prepare {
        &RESULT_FIELDS_PREPARE[..]
    } else {
        &RESULT_FIELDS_GET[..]
    };
    if !object_has_exact_fields(object, result_fields) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence result fields differ".to_owned(),
        );
    }
    let expected_schema = if is_prepare {
        "ProductionWeaponFormArtEvidencePrepareResult@1"
    } else {
        "ProductionWeaponFormArtEvidenceGetResult@1"
    };
    let expected_policy_sha256 = forgecad_runtime::sha256_hex(
        b"production-weapon-form-art-evidence-six-view-typed-observation@1",
    );
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || (!is_prepare && value.get("restart_hash_verified") != Some(&Value::Bool(true)))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence side-effect or restart flags differ"
                .to_owned(),
        );
    }
    let evidence = value
        .get("art_evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence record missing".to_owned()
        })?;
    if !object_has_exact_fields(evidence, &RECORD_FIELDS)
        || evidence.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormArtEvidence@1")
        || evidence.get("art_evidence_policy").and_then(Value::as_str)
            != Some("production-weapon-form-art-evidence-six-view-typed-observation@1")
        || evidence
            .get("art_evidence_policy_sha256")
            .and_then(Value::as_str)
            != Some(expected_policy_sha256.as_str())
        || evidence.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || evidence.get("runtime_write_performed") != Some(&Value::Bool(true))
        || evidence.get("production_stage_advanced") != Some(&Value::Bool(false))
        || evidence.get("candidate_confirmed") != Some(&Value::Bool(false))
        || evidence.get("version_created") != Some(&Value::Bool(false))
        || evidence.get("export_performed") != Some(&Value::Bool(false))
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence record status, fields or media boundary differs"
                .to_owned(),
        );
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "art_evidence_policy_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(evidence.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence hash {field} is invalid"
            ));
        }
    }
    for field in [
        "art_evidence_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
    ] {
        if !valid_v2_id(evidence.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence id {field} is invalid"
            ));
        }
    }
    if evidence
        .get("created_at")
        .and_then(Value::as_str)
        .is_none_or(|value| value.is_empty() || value.len() > 128)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence timestamp is invalid".to_owned(),
        );
    }
    let expected_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    if evidence.get("view_kinds") != Some(&json!(expected_views)) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view kinds differ".to_owned(),
        );
    }
    let aggregate = evidence
        .get("part_id_aggregate")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence Part-ID aggregate missing".to_owned()
        })?;
    const AGGREGATE_FIELDS: [&str; 6] = [
        "status",
        "expected_count",
        "observed_count",
        "missing_count",
        "unexpected_count",
        "coverage_milli",
    ];
    if !object_has_exact_fields(aggregate, &AGGREGATE_FIELDS)
        || !matches!(
            aggregate.get("status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || [
            "expected_count",
            "observed_count",
            "missing_count",
            "unexpected_count",
        ]
        .iter()
        .any(|field| {
            aggregate
                .get(*field)
                .and_then(Value::as_u64)
                .is_none_or(|value| value > 512)
        })
        || aggregate
            .get("coverage_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence Part-ID aggregate differs"
                .to_owned(),
        );
    }
    let views = evidence
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence views missing".to_owned()
        })?;
    if views.len() != expected_views.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view count differs".to_owned(),
        );
    }
    let mut view_ids = BTreeSet::new();
    for (view, expected_kind) in views.iter().zip(expected_views) {
        validate_production_weapon_form_art_evidence_view(
            view,
            expected_kind,
            evidence,
            &mut view_ids,
        )?;
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != evidence.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != evidence.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != evidence.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: form-art-evidence response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_production_weapon_form_art_evidence_view(
    value: &Value,
    expected_kind: &str,
    parent: &Map<String, Value>,
    view_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    const VIEW_FIELDS: [&str; 37] = [
        "schema_version",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "view_kind",
        "view_id",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "form_evidence_view_receipt_object_sha256",
        "form_evidence_view_receipt_canonical_sha256",
        "target_object_sha256",
        "target_canonical_sha256",
        "visual_structure_canonical_sha256",
        "visual_structure_review_status",
        "silhouette_pass_object_sha256",
        "part_id_pass_object_sha256",
        "depth_pass_object_sha256",
        "normal_pass_object_sha256",
        "part_id_status",
        "part_id_expected_count",
        "part_id_observed_count",
        "part_id_missing_count",
        "part_id_unexpected_count",
        "part_id_coverage_milli",
        "negative_space_status",
        "negative_space_rows",
        "line_flow_status",
        "line_flow_rows",
        "view_observation_status",
        "quality_status",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view must be an object".to_owned()
    })?;
    if !object_has_exact_fields(object, &VIEW_FIELDS)
        || object.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormArtEvidenceView@1")
        || object.get("view_kind").and_then(Value::as_str) != Some(expected_kind)
        || !matches!(
            object
                .get("visual_structure_review_status")
                .and_then(Value::as_str),
            Some("user_confirmed" | "inferred" | "unknown")
        )
        || !matches!(
            object.get("part_id_status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || !matches!(
            object.get("negative_space_status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown" | "not-applicable")
        )
        || !matches!(
            object.get("line_flow_status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown" | "not-applicable")
        )
        || !matches!(
            object
                .get("view_observation_status")
                .and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || object.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || object
            .get("created_at")
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty() || value.len() > 128)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view identity/status differs"
                .to_owned(),
        );
    }
    for field in [
        "project_id",
        "candidate_id",
        "artifact_id",
        "view_id",
        "reference_id",
    ] {
        if !valid_v2_id(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view id {field} is invalid"
            ));
        }
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "form_evidence_view_receipt_object_sha256",
        "form_evidence_view_receipt_canonical_sha256",
        "target_object_sha256",
        "target_canonical_sha256",
        "visual_structure_canonical_sha256",
        "silhouette_pass_object_sha256",
        "part_id_pass_object_sha256",
        "depth_pass_object_sha256",
        "normal_pass_object_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view hash {field} is invalid"
            ));
        }
    }
    if object.get("project_id") != parent.get("project_id")
        || object.get("candidate_id") != parent.get("candidate_id")
        || object.get("candidate_state_sha256") != parent.get("candidate_state_sha256")
        || object.get("artifact_id") != parent.get("artifact_id")
        || object.get("artifact_sha256") != parent.get("artifact_sha256")
        || !view_ids.insert(object["view_id"].as_str().unwrap_or_default().to_owned())
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view lineage differs".to_owned(),
        );
    }
    for field in [
        "part_id_expected_count",
        "part_id_observed_count",
        "part_id_missing_count",
        "part_id_unexpected_count",
    ] {
        if object
            .get(field)
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 512)
        {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence view {field} differs"
            ));
        }
    }
    if object
        .get("part_id_coverage_milli")
        .and_then(Value::as_u64)
        .is_none_or(|value| value > 1000)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence Part-ID coverage differs".to_owned(),
        );
    }
    let negative_rows = object
        .get("negative_space_rows")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence negative-space rows missing"
                .to_owned()
        })?;
    let line_rows = object
        .get("line_flow_rows")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence line-flow rows missing".to_owned()
        })?;
    if negative_rows.len() > 512
        || line_rows.len() > 512
        || (object.get("negative_space_status").and_then(Value::as_str) == Some("observed")
            && negative_rows.is_empty())
        || (object.get("line_flow_status").and_then(Value::as_str) == Some("observed")
            && line_rows.is_empty())
        || (object.get("negative_space_status").and_then(Value::as_str) == Some("not-applicable")
            && !negative_rows.is_empty())
        || (object.get("line_flow_status").and_then(Value::as_str) == Some("not-applicable")
            && !line_rows.is_empty())
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-art-evidence typed row cardinality differs"
                .to_owned(),
        );
    }
    for row in negative_rows {
        validate_production_weapon_form_art_evidence_negative_space_row(row)?;
    }
    for row in line_rows {
        validate_production_weapon_form_art_evidence_line_flow_row(row)?;
    }
    Ok(())
}

fn validate_production_weapon_form_art_evidence_negative_space_row(
    value: &Value,
) -> Result<(), String> {
    const FIELDS: [&str; 9] = [
        "structure_id",
        "expected_region_canonical_sha256",
        "iou_milli",
        "boundary_f1_milli",
        "area_ratio_milli",
        "centroid_error_milli",
        "sealed",
        "missing",
        "status",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space row must be an object".to_owned()
    })?;
    if object.len() != FIELDS.len() {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space row fields differ".to_owned());
    }
    for field in FIELDS {
        if !object.contains_key(field) {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space row fields differ".to_owned(),
            );
        }
    }
    if !valid_v2_id(object.get("structure_id").and_then(Value::as_str))
        || !valid_sha256(
            object
                .get("expected_region_canonical_sha256")
                .and_then(Value::as_str),
        )
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || object.get("sealed").and_then(Value::as_bool).is_none()
        || object.get("missing").and_then(Value::as_bool).is_none()
        || object
            .get("iou_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        || object
            .get("boundary_f1_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        || object
            .get("area_ratio_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 10000)
        || object
            .get("centroid_error_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 100000)
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: negative-space row values differ".to_owned());
    }
    Ok(())
}

fn validate_production_weapon_form_art_evidence_line_flow_row(value: &Value) -> Result<(), String> {
    const FIELDS: [&str; 9] = [
        "line_flow_id",
        "expected_line_canonical_sha256",
        "coverage_milli",
        "continuity_milli",
        "symmetric_chamfer_milli",
        "max_deviation_milli",
        "direction_order_milli",
        "duplicate_crossing_count",
        "status",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow row must be an object".to_owned()
    })?;
    if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow row fields differ".to_owned());
    }
    if !valid_v2_id(object.get("line_flow_id").and_then(Value::as_str))
        || !valid_sha256(
            object
                .get("expected_line_canonical_sha256")
                .and_then(Value::as_str),
        )
        || !matches!(
            object.get("status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        )
        || object
            .get("coverage_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        || object
            .get("continuity_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        || object
            .get("symmetric_chamfer_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 100000)
        || object
            .get("max_deviation_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 100000)
        || object
            .get("direction_order_milli")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 1000)
        || object
            .get("duplicate_crossing_count")
            .and_then(Value::as_u64)
            .is_none_or(|value| value > 512)
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: line-flow row values differ".to_owned());
    }
    Ok(())
}

fn validate_production_weapon_assembly_parameter_sink_response(
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 18] = [
        "schema_version",
        "registry",
        "registry_canonical_sha256",
        "recomputed",
        "restart_hash_verified",
        "read_only",
        "structural_status",
        "quality_status",
        "visual_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "worker_invoked",
        "candidate_generated",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    const REGISTRY_FIELDS: [&str; 28] = [
        "schema_version",
        "sink_registry_id",
        "profile_id",
        "sink_policy",
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "assembly_registry_id",
        "assembly_registry_canonical_sha256",
        "supported_group_ids",
        "sinks",
        "unavailable_parameter_ids",
        "status",
        "read_only",
        "runtime_write_performed",
        "worker_invoked",
        "candidate_generated",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "canonical_sha256",
    ];
    const SINK_FIELDS: [&str; 14] = [
        "parameter_id",
        "group_id",
        "mutator_id",
        "current",
        "min",
        "max",
        "step",
        "unit",
        "application_status",
        "blocker_codes",
        "target_part_ids",
        "source_node_ids",
        "operator_ids",
        "evidence_requirements",
    ];
    const SUPPORTED_GROUPS: [&str; 2] = ["receiver-envelope", "muzzle-axis"];
    const SUPPORTED_PARAMETERS: [&str; 6] = [
        "receiver-envelope-width",
        "receiver-envelope-height",
        "receiver-envelope-shoulder",
        "muzzle-axis-shroud-envelope",
        "muzzle-axis-emitter-envelope",
        "muzzle-axis-core-aperture",
    ];
    const UNAVAILABLE_PARAMETERS: [&str; 6] = [
        "stock-open-frame-clearance",
        "stock-open-frame-angle",
        "trigger-void-clearance",
        "trigger-void-centroid",
        "rail-spine-continuity",
        "rail-spine-offset",
    ];
    const MUTATORS: [&str; 2] = [
        "forgecad.assembly.mutator.receiver-envelope@1",
        "forgecad.assembly.mutator.muzzle-axis@1",
    ];
    const OPERATOR_IDS: [&str; 28] = [
        "forgecad.geometry.primitive@2",
        "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1",
        "forgecad.geometry.profile-loft@2",
        "forgecad.geometry.multi-loop-profile-loft@1",
        "forgecad.geometry.longitudinal-section-loft@1",
        "forgecad.geometry.subd-cage@1",
        "forgecad.geometry.subd-cage@2",
        "forgecad.geometry.authoring-mesh@1",
        "forgecad.geometry.surface-patch@1",
        "forgecad.geometry.surface-shell@1",
        "forgecad.geometry.revolve@1",
        "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.transform@2",
        "forgecad.geometry.mirror@1",
        "forgecad.geometry.array@1",
        "forgecad.geometry.bevel@1",
        "forgecad.geometry.bevel@2",
        "forgecad.geometry.normal-policy@1",
        "forgecad.geometry.panel@1",
        "forgecad.geometry.panel@2",
        "forgecad.geometry.vent-array@1",
        "forgecad.geometry.vent-array@2",
        "forgecad.geometry.recessed-channel@1",
        "forgecad.geometry.energy-core@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.part-output@1",
        "forgecad.geometry.boolean@1",
    ];
    const EVIDENCE_REQUIREMENTS: [&str; 5] = [
        "assembly-registry",
        "geometry-program",
        "operator-catalog",
        "artifact-readback",
        "candidate-state",
    ];

    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink result must be an object"
            .to_owned()
    })?;
    if contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
        || !object_has_exact_fields(object, &RESULT_FIELDS)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink result fields or transport boundary differs"
                .to_owned(),
        );
    }
    if object.get("schema_version").and_then(Value::as_str)
        != Some("ProductionWeaponAssemblyParameterSinkGetResult@1")
        || object.get("registry_canonical_sha256")
            != object
                .get("registry")
                .and_then(|registry| registry.get("canonical_sha256"))
        || object.get("recomputed") != Some(&Value::Bool(true))
        || object.get("restart_hash_verified") != Some(&Value::Bool(true))
        || object.get("read_only") != Some(&Value::Bool(true))
        || object.get("structural_status").and_then(Value::as_str) != Some("structural_only")
        || object.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || object.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || object.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            != Some("NOT_RUN")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("worker_invoked") != Some(&Value::Bool(false))
        || object.get("candidate_generated") != Some(&Value::Bool(false))
        || object.get("production_stage_advanced") != Some(&Value::Bool(false))
        || object.get("candidate_confirmed") != Some(&Value::Bool(false))
        || object.get("version_created") != Some(&Value::Bool(false))
        || object.get("export_performed") != Some(&Value::Bool(false))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink result status or read-only flags differ"
                .to_owned(),
        );
    }

    let registry = object
        .get("registry")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry is missing".to_owned()
        })?;
    if !object_has_exact_fields(registry, &REGISTRY_FIELDS)
        || registry.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponAssemblyParameterSinkRegistry@1")
        || registry.get("profile_id").and_then(Value::as_str) != Some("fps-weapon-form-assembly@1")
        || registry.get("sink_policy").and_then(Value::as_str)
            != Some("fps-weapon-product-owned-aggregate-parameter-sink-registry@1")
        || registry.get("supported_group_ids") != Some(&json!(SUPPORTED_GROUPS))
        || registry.get("read_only") != Some(&Value::Bool(true))
        || registry.get("runtime_write_performed") != Some(&Value::Bool(false))
        || registry.get("worker_invoked") != Some(&Value::Bool(false))
        || registry.get("candidate_generated") != Some(&Value::Bool(false))
        || registry.get("production_stage_advanced") != Some(&Value::Bool(false))
        || registry.get("candidate_confirmed") != Some(&Value::Bool(false))
        || registry.get("version_created") != Some(&Value::Bool(false))
        || registry.get("export_performed") != Some(&Value::Bool(false))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry identity or flags differ"
                .to_owned(),
        );
    }
    for field in [
        "sink_registry_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "assembly_registry_id",
    ] {
        if !valid_v2_id(registry.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry id {field} is invalid"
            ));
        }
    }
    for field in [
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "assembly_registry_canonical_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(registry.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry hash {field} is invalid"
            ));
        }
    }
    let registry_canonical_sha256 = registry
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink canonical hash is missing"
                .to_owned()
        })?;
    let mut canonical_registry = registry.clone();
    canonical_registry.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    if forgecad_runtime::canonical_json_hash(&Value::Object(canonical_registry))
        != registry_canonical_sha256
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry canonical hash was not recomputed"
                .to_owned(),
        );
    }

    let status = registry
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry status is missing"
                .to_owned()
        })?;
    if !matches!(status, "PARTIAL_TYPED_SINKS" | "READY") {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: assembly parameter sink registry status differs"
                .to_owned(),
        );
    }
    let unavailable = registry
        .get("unavailable_parameter_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter ids are missing".to_owned()
        })?;
    if unavailable.len() < 6
        || unavailable.len() > 12
        || unavailable.iter().any(|value| {
            !value.as_str().is_some_and(|id| {
                SUPPORTED_PARAMETERS.contains(&id) || UNAVAILABLE_PARAMETERS.contains(&id)
            })
        })
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter ids are not closed".to_owned(),
        );
    }
    let mut unavailable_ids = BTreeSet::new();
    for value in unavailable {
        let id = value.as_str().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter id is not a string".to_owned()
        })?;
        if !unavailable_ids.insert(id) {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter ids are duplicated"
                    .to_owned(),
            );
        }
    }
    let expected_unavailable = UNAVAILABLE_PARAMETERS
        .iter()
        .map(|id| Value::String((*id).to_owned()))
        .collect::<Vec<_>>();
    if UNAVAILABLE_PARAMETERS
        .iter()
        .any(|parameter_id| !unavailable_ids.contains(parameter_id))
        || (status == "READY" && unavailable.as_slice() != expected_unavailable.as_slice())
        || (status == "PARTIAL_TYPED_SINKS" && unavailable.len() < 7)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter status or ordering differs"
                .to_owned(),
        );
    }

    let sinks = registry
        .get("sinks")
        .and_then(Value::as_array)
        .ok_or_else(|| "AGENTIC_RUNTIME_OUTPUT_INVALID: sinks are missing".to_owned())?;
    if sinks.len() > 6
        || (status == "READY" && sinks.len() != 6)
        || (status == "PARTIAL_TYPED_SINKS" && sinks.len() > 5)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: sink count does not match registry status".to_owned(),
        );
    }
    let mut parameter_ids = BTreeSet::new();
    for sink in sinks {
        let sink_object = sink.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: sink row must be an object".to_owned()
        })?;
        if !object_has_exact_fields(sink_object, &SINK_FIELDS)
            || sink_object
                .get("application_status")
                .and_then(Value::as_str)
                != Some("AVAILABLE")
            || sink_object
                .get("blocker_codes")
                .and_then(Value::as_array)
                .is_none_or(|codes| !codes.is_empty())
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink row fields or availability differs"
                    .to_owned(),
            );
        }
        let parameter_id = sink_object
            .get("parameter_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink parameter id is missing".to_owned()
            })?;
        let group_id = sink_object.get("group_id").and_then(Value::as_str);
        let mutator_id = sink_object.get("mutator_id").and_then(Value::as_str);
        if !SUPPORTED_PARAMETERS.contains(&parameter_id)
            || !parameter_ids.insert(parameter_id)
            || unavailable_ids.contains(parameter_id)
            || !matches!(group_id, Some("receiver-envelope" | "muzzle-axis"))
            || !MUTATORS.contains(&mutator_id.unwrap_or_default())
            || ((parameter_id.starts_with("receiver-") && group_id != Some("receiver-envelope"))
                || (parameter_id.starts_with("muzzle-") && group_id != Some("muzzle-axis")))
            || ((group_id == Some("receiver-envelope")
                && mutator_id != Some("forgecad.assembly.mutator.receiver-envelope@1"))
                || (group_id == Some("muzzle-axis")
                    && mutator_id != Some("forgecad.assembly.mutator.muzzle-axis@1")))
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink semantic/group/mutator binding differs"
                    .to_owned(),
            );
        }
        for field in ["current", "min", "max", "step"] {
            if !sink_object
                .get(field)
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
            {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: sink numeric field {field} is invalid"
                ));
            }
        }
        let current = sink_object.get("current").and_then(Value::as_f64).unwrap();
        let min = sink_object.get("min").and_then(Value::as_f64).unwrap();
        let max = sink_object.get("max").and_then(Value::as_f64).unwrap();
        let step = sink_object.get("step").and_then(Value::as_f64).unwrap();
        if min > max
            || current < min
            || current > max
            || step <= 0.0
            || !matches!(
                sink_object.get("unit").and_then(Value::as_str),
                Some("meter" | "ratio")
            )
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink bounds, unit or step differs".to_owned(),
            );
        }
        for field in ["target_part_ids", "source_node_ids"] {
            validate_assembly_sink_id_array(sink_object, field, 64)?;
        }
        let operators = sink_object
            .get("operator_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink operator ids are missing".to_owned()
            })?;
        if operators.is_empty()
            || operators.len() > OPERATOR_IDS.len()
            || operators.iter().any(|operator| {
                !operator
                    .as_str()
                    .is_some_and(|id| OPERATOR_IDS.contains(&id))
            })
            || !unique_string_values(operators)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink operator ids are not closed".to_owned(),
            );
        }
        let evidence = sink_object
            .get("evidence_requirements")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink evidence requirements are missing".to_owned()
            })?;
        if evidence.is_empty()
            || evidence.len() > EVIDENCE_REQUIREMENTS.len()
            || evidence.iter().any(|requirement| {
                !requirement
                    .as_str()
                    .is_some_and(|id| EVIDENCE_REQUIREMENTS.contains(&id))
            })
            || !unique_string_values(evidence)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: sink evidence requirements are not closed"
                    .to_owned(),
            );
        }
    }
    if status == "READY" && parameter_ids.len() != SUPPORTED_PARAMETERS.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: READY registry does not expose all supported sinks"
                .to_owned(),
        );
    }
    let mut exact_unavailable = UNAVAILABLE_PARAMETERS
        .iter()
        .map(|id| Value::String((*id).to_owned()))
        .collect::<Vec<_>>();
    exact_unavailable.extend(
        SUPPORTED_PARAMETERS
            .iter()
            .filter(|parameter_id| !parameter_ids.contains(**parameter_id))
            .map(|id| Value::String((*id).to_owned())),
    );
    if unavailable.as_slice() != exact_unavailable.as_slice()
        || (status == "READY" && exact_unavailable.len() != UNAVAILABLE_PARAMETERS.len())
        || (status == "PARTIAL_TYPED_SINKS"
            && exact_unavailable.len() == UNAVAILABLE_PARAMETERS.len())
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: unavailable parameter ordering is not the exact complement of the available sinks"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != registry.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != registry.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != registry.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: assembly parameter sink registry crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_assembly_sink_id_array(
    object: &Map<String, Value>,
    field: &str,
    max_items: usize,
) -> Result<(), String> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("AGENTIC_RUNTIME_OUTPUT_INVALID: sink {field} is missing"))?;
    if values.is_empty()
        || values.len() > max_items
        || values.iter().any(|value| !valid_v2_id(value.as_str()))
        || !unique_string_values(values)
    {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: sink {field} is not a unique bounded id list"
        ));
    }
    Ok(())
}

fn unique_string_values(values: &[Value]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().all(|value| {
        value
            .as_str()
            .is_some_and(|string| seen.insert(string.to_owned()))
    })
}

fn validate_production_weapon_art_decision_proposal_response(
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 42] = [
        "schema_version",
        "proposal_projection_id",
        "session_id",
        "project_id",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "assembly_registry_id",
        "assembly_registry_canonical_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "form_evidence_id",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_art_evidence_id",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "first_person_profile_id",
        "first_person_profile_sha256",
        "view_bindings",
        "assembly_group_decisions",
        "objective_policy",
        "gate_results",
        "blockers",
        "proposal_status",
        "read_only",
        "runtime_write_performed",
        "worker_invoked",
        "candidate_generated",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "replayed",
        "restart_hash_verified",
        "canonical_sha256",
    ];
    const VIEW_FIELDS: [&str; 18] = [
        "view_kind",
        "view_id",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "camera_canonical_sha256",
        "render_set_object_sha256",
        "render_set_canonical_sha256",
        "form_evidence_view_receipt_object_sha256",
        "form_evidence_view_receipt_canonical_sha256",
        "form_art_evidence_view_receipt_object_sha256",
        "form_art_evidence_view_receipt_canonical_sha256",
        "target_sha256",
        "visual_structure_canonical_sha256",
        "part_id_status",
        "negative_space_status",
        "line_flow_status",
        "view_observation_status",
    ];
    const GROUP_FIELDS: [&str; 10] = [
        "group_id",
        "status",
        "part_ids",
        "source_node_ids",
        "parameter_ids",
        "allowed_operator_ids",
        "coupling_mode",
        "invariants",
        "affected_view_kinds",
        "blocker_codes",
    ];
    const GATE_FIELDS: [&str; 4] = ["gate_id", "status", "evidence_sha256", "blocker_codes"];
    const BLOCKER_FIELDS: [&str; 5] = [
        "blocker_code",
        "scope",
        "group_id",
        "view_kind",
        "evidence_sha256",
    ];
    const EXPECTED_VIEWS: [&str; 6] = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    const EXPECTED_GROUPS: [&str; 5] = [
        "receiver-envelope",
        "muzzle-axis",
        "stock-open-frame",
        "trigger-void",
        "rail-spine",
    ];
    const EXPECTED_GATES: [&str; 10] = [
        "lineage",
        "reference-annotation",
        "camera",
        "assembly-registry",
        "parameter-sink",
        "negative-space",
        "line-flow",
        "first-person-readability",
        "candidate-search-critic",
        "surface-scope",
    ];
    const HASH_FIELDS: [&str; 14] = [
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "geometry_program_canonical_sha256",
        "operator_catalog_sha256",
        "assembly_registry_canonical_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "canonical_sha256",
    ];
    const ID_FIELDS: [&str; 9] = [
        "proposal_projection_id",
        "session_id",
        "project_id",
        "candidate_id",
        "artifact_id",
        "assembly_registry_id",
        "camera_lock_id",
        "form_evidence_id",
        "form_art_evidence_id",
    ];

    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal result must be an object".to_owned()
    })?;
    if contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
        || !object_has_exact_fields(object, &RESULT_FIELDS)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal result fields or media boundary differs"
                .to_owned(),
        );
    }
    if value.get("schema_version").and_then(Value::as_str)
        != Some("ProductionWeaponArtDecisionProposalGetResult@1")
        || value.get("objective_policy").and_then(Value::as_str)
            != Some("assembly-form-search-negative-space-line-flow-first-person@1")
        || value.get("read_only") != Some(&Value::Bool(true))
        || value.get("runtime_write_performed") != Some(&Value::Bool(false))
        || value.get("worker_invoked") != Some(&Value::Bool(false))
        || value.get("candidate_generated") != Some(&Value::Bool(false))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || !matches!(
            value.get("proposal_status").and_then(Value::as_str),
            Some(
                "READY_ASSEMBLY_FORM_SEARCH"
                    | "BLOCKED_LINEAGE"
                    | "BLOCKED_REFERENCE_ANNOTATION"
                    | "BLOCKED_CAMERA"
                    | "BLOCKED_NEGATIVE_SPACE"
                    | "BLOCKED_LINE_FLOW"
                    | "BLOCKED_FIRST_PERSON_PROFILE"
                    | "BLOCKED_ASSEMBLY_REGISTRY"
                    | "BLOCKED_PARAMETER_SINK"
                    | "NO_STRICT_MULTI_VIEW_IMPROVEMENT"
            )
        )
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal status or read-only flags differ"
                .to_owned(),
        );
    }
    for field in ID_FIELDS {
        if !valid_v2_id(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal id {field} is invalid"
            ));
        }
    }
    for field in HASH_FIELDS {
        if !valid_sha256(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal hash {field} is invalid"
            ));
        }
    }
    let profile_id = object.get("first_person_profile_id");
    let profile_sha256 = object.get("first_person_profile_sha256");
    match (profile_id, profile_sha256) {
        (Some(Value::Null), Some(Value::Null)) => {}
        (Some(Value::String(id)), Some(Value::String(hash)))
            if valid_v2_id(Some(id)) && valid_sha256(Some(hash)) => {}
        _ => {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision first-person profile binding is not paired"
                    .to_owned(),
            )
        }
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != object.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != object.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != object.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: art-decision proposal response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }

    let views = object
        .get("view_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal views are missing".to_owned()
        })?;
    if views.len() != EXPECTED_VIEWS.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal must contain six views"
                .to_owned(),
        );
    }
    let mut view_ids = BTreeSet::new();
    for (view, expected_kind) in views.iter().zip(EXPECTED_VIEWS) {
        let view_object = view.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision view must be an object".to_owned()
        })?;
        if !object_has_exact_fields(view_object, &VIEW_FIELDS)
            || view_object.get("view_kind").and_then(Value::as_str) != Some(expected_kind)
            || !view_ids.insert(
                view_object
                    .get("view_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision view order, identity or fields differ"
                    .to_owned(),
            );
        }
        for field in ["view_id", "reference_id"] {
            if !valid_v2_id(view_object.get(field).and_then(Value::as_str)) {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision view id {field} is invalid"
                ));
            }
        }
        for field in [
            "reference_sha256",
            "camera_hash",
            "camera_canonical_sha256",
            "render_set_object_sha256",
            "render_set_canonical_sha256",
            "form_evidence_view_receipt_object_sha256",
            "form_evidence_view_receipt_canonical_sha256",
            "form_art_evidence_view_receipt_object_sha256",
            "form_art_evidence_view_receipt_canonical_sha256",
            "target_sha256",
            "visual_structure_canonical_sha256",
        ] {
            if !valid_sha256(view_object.get(field).and_then(Value::as_str)) {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision view hash {field} is invalid"
                ));
            }
        }
        if !matches!(
            view_object.get("part_id_status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        ) || !matches!(
            view_object
                .get("negative_space_status")
                .and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown" | "not-applicable")
        ) || !matches!(
            view_object.get("line_flow_status").and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown" | "not-applicable")
        ) || !matches!(
            view_object
                .get("view_observation_status")
                .and_then(Value::as_str),
            Some("observed" | "inferred" | "unknown")
        ) {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision view observation status differs"
                    .to_owned(),
            );
        }
    }

    let groups = object
        .get("assembly_group_decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision assembly groups are missing".to_owned()
        })?;
    if groups.len() != EXPECTED_GROUPS.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal must contain five assembly groups"
                .to_owned(),
        );
    }
    for (group, expected_group_id) in groups.iter().zip(EXPECTED_GROUPS) {
        let group_object = group.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision assembly group must be an object"
                .to_owned()
        })?;
        if !object_has_exact_fields(group_object, &GROUP_FIELDS)
            || group_object.get("group_id").and_then(Value::as_str) != Some(expected_group_id)
            || group_object.get("affected_view_kinds") != Some(&json!(EXPECTED_VIEWS))
            || !matches!(
                group_object.get("status").and_then(Value::as_str),
                Some(
                    "READY_FOR_SEARCH"
                        | "BLOCKED_PARAMETER_SINK"
                        | "BLOCKED_ASSEMBLY_REGISTRY"
                        | "NOT_RUN"
                )
            )
            || !matches!(
                group_object.get("coupling_mode").and_then(Value::as_str),
                Some("independent" | "linked" | "mirror")
            )
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision assembly group identity or status differs"
                    .to_owned(),
            );
        }
        validate_art_decision_string_array(group_object, "part_ids", 1, 16, true)?;
        validate_art_decision_string_array(group_object, "source_node_ids", 1, 32, true)?;
        validate_art_decision_string_array(group_object, "parameter_ids", 1, 32, true)?;
        validate_art_decision_operator_array(group_object)?;
        validate_art_decision_invariant_array(group_object)?;
        validate_art_decision_blocker_array(group_object, "blocker_codes", 9)?;
    }

    let gates = object
        .get("gate_results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision gates are missing".to_owned()
        })?;
    if gates.len() != EXPECTED_GATES.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision proposal must contain ten gates"
                .to_owned(),
        );
    }
    for (gate, expected_gate_id) in gates.iter().zip(EXPECTED_GATES) {
        let gate_object = gate.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision gate must be an object".to_owned()
        })?;
        if !object_has_exact_fields(gate_object, &GATE_FIELDS)
            || gate_object.get("gate_id").and_then(Value::as_str) != Some(expected_gate_id)
            || !matches!(
                gate_object.get("status").and_then(Value::as_str),
                Some("PASS" | "BLOCKED" | "NOT_RUN" | "LOCKED")
            )
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision gate identity or status differs"
                    .to_owned(),
            );
        }
        if let Some(evidence) = gate_object.get("evidence_sha256") {
            if !evidence.is_null() && !valid_sha256(evidence.as_str()) {
                return Err(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision gate evidence hash is invalid"
                        .to_owned(),
                );
            }
        } else {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision gate evidence field is missing"
                    .to_owned(),
            );
        }
        validate_art_decision_blocker_array(gate_object, "blocker_codes", 9)?;
    }

    let blockers = object
        .get("blockers")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blockers are missing".to_owned()
        })?;
    if blockers.len() > 64 {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker count exceeds 64".to_owned(),
        );
    }
    let mut blocker_keys = BTreeSet::new();
    for blocker in blockers {
        let blocker_object = blocker.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker must be an object".to_owned()
        })?;
        if !object_has_exact_fields(blocker_object, &BLOCKER_FIELDS)
            || !valid_art_decision_blocker_code(
                blocker_object.get("blocker_code").and_then(Value::as_str),
            )
            || !matches!(
                blocker_object.get("scope").and_then(Value::as_str),
                Some("global" | "assembly" | "view")
            )
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker identity differs".to_owned(),
            );
        }
        if let Some(group_id) = blocker_object.get("group_id") {
            if !group_id.is_null() && !valid_v2_id(group_id.as_str()) {
                return Err(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker group id is invalid"
                        .to_owned(),
                );
            }
        }
        if let Some(view_kind) = blocker_object.get("view_kind") {
            if !view_kind.is_null()
                && !EXPECTED_VIEWS.contains(&view_kind.as_str().unwrap_or_default())
            {
                return Err(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker view kind is invalid"
                        .to_owned(),
                );
            }
        }
        if let Some(evidence) = blocker_object.get("evidence_sha256") {
            if !evidence.is_null() && !valid_sha256(evidence.as_str()) {
                return Err(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker evidence hash is invalid"
                        .to_owned(),
                );
            }
        }
        let key = serde_json::to_string(blocker).map_err(|_| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blocker is not canonical JSON".to_owned()
        })?;
        if !blocker_keys.insert(key) {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision blockers must be unique".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_art_decision_string_array(
    object: &Map<String, Value>,
    field: &str,
    min_items: usize,
    max_items: usize,
    bounded_ids: bool,
) -> Result<(), String> {
    let values = object.get(field).and_then(Value::as_array).ok_or_else(|| {
        format!("AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision {field} is missing")
    })?;
    if values.len() < min_items
        || values.len() > max_items
        || values.iter().any(|value| {
            if bounded_ids {
                !valid_v2_id(value.as_str())
            } else {
                value.as_str().is_none_or(str::is_empty)
            }
        })
        || values
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
    {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision {field} array differs"
        ));
    }
    Ok(())
}

fn validate_art_decision_operator_array(object: &Map<String, Value>) -> Result<(), String> {
    const ALLOWED_OPERATORS: [&str; 28] = [
        "forgecad.geometry.primitive@2",
        "forgecad.geometry.profile-extrude@1",
        "forgecad.geometry.profile-loft@1",
        "forgecad.geometry.profile-loft@2",
        "forgecad.geometry.multi-loop-profile-loft@1",
        "forgecad.geometry.longitudinal-section-loft@1",
        "forgecad.geometry.subd-cage@1",
        "forgecad.geometry.subd-cage@2",
        "forgecad.geometry.authoring-mesh@1",
        "forgecad.geometry.surface-patch@1",
        "forgecad.geometry.surface-shell@1",
        "forgecad.geometry.revolve@1",
        "forgecad.geometry.tube-sweep@1",
        "forgecad.geometry.transform@2",
        "forgecad.geometry.mirror@1",
        "forgecad.geometry.array@1",
        "forgecad.geometry.bevel@1",
        "forgecad.geometry.bevel@2",
        "forgecad.geometry.normal-policy@1",
        "forgecad.geometry.panel@1",
        "forgecad.geometry.panel@2",
        "forgecad.geometry.vent-array@1",
        "forgecad.geometry.vent-array@2",
        "forgecad.geometry.recessed-channel@1",
        "forgecad.geometry.energy-core@1",
        "forgecad.geometry.joint-stack@1",
        "forgecad.geometry.part-output@1",
        "forgecad.geometry.boolean@1",
    ];
    let values = object
        .get("allowed_operator_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision allowed operators are missing".to_owned()
        })?;
    if values.is_empty()
        || values.len() > 16
        || values.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|operator| !ALLOWED_OPERATORS.contains(&operator))
        })
        || values
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision allowed operator ids differ".to_owned(),
        );
    }
    Ok(())
}

fn validate_art_decision_invariant_array(object: &Map<String, Value>) -> Result<(), String> {
    const INVARIANTS: [&str; 6] = [
        "shared-axis",
        "coaxial",
        "mirror-symmetric",
        "clearance-min",
        "enclosed-void",
        "continuous-spine",
    ];
    let values = object
        .get("invariants")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision invariants are missing".to_owned()
        })?;
    if values.is_empty()
        || values.len() > 6
        || values.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|invariant| !INVARIANTS.contains(&invariant))
        })
        || values
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision invariants differ".to_owned());
    }
    Ok(())
}

fn valid_art_decision_blocker_code(value: Option<&str>) -> bool {
    matches!(
        value,
        Some(
            "BLOCKED_LINEAGE"
                | "BLOCKED_REFERENCE_ANNOTATION"
                | "BLOCKED_CAMERA"
                | "BLOCKED_NEGATIVE_SPACE"
                | "BLOCKED_LINE_FLOW"
                | "BLOCKED_FIRST_PERSON_PROFILE"
                | "BLOCKED_ASSEMBLY_REGISTRY"
                | "BLOCKED_PARAMETER_SINK"
                | "NO_STRICT_MULTI_VIEW_IMPROVEMENT"
        )
    )
}

fn validate_art_decision_blocker_array(
    object: &Map<String, Value>,
    field: &str,
    max_items: usize,
) -> Result<(), String> {
    let values = object.get(field).and_then(Value::as_array).ok_or_else(|| {
        format!("AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision {field} is missing")
    })?;
    if values.len() > max_items
        || values
            .iter()
            .any(|value| !valid_art_decision_blocker_code(value.as_str()))
        || values
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
    {
        return Err(format!(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: art-decision {field} differs"
        ));
    }
    Ok(())
}

fn validate_production_weapon_form_quality_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::ProductionWeaponFormQualityPrepare;
    let result_fields = [
        "schema_version",
        "form_quality",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    let record_fields = [
        "schema_version",
        "form_quality_id",
        "session_id",
        "project_id",
        "form_stage",
        "source_stage",
        "target_stage",
        "camera_calibrated_head_transition_id",
        "camera_calibrated_head_transition_sha256",
        "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_id",
        "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_id",
        "camera_calibrated_head_artifact_sha256",
        "camera_calibrated_head_stage",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "reviewed_reference_view_kinds",
        "fixed_camera_view_kinds",
        "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256",
        "cross_view_evidence_view_kinds",
        "form_view_evaluations",
        "previous_form_quality_id",
        "previous_form_quality_report_object_sha256",
        "previous_form_quality_canonical_sha256",
        "form_quality_policy",
        "form_quality_policy_sha256",
        "threshold_policy",
        "threshold_policy_sha256",
        "layer_status",
        "hard_gate",
        "hard_gate_passed",
        "form_gate",
        "form_gate_passed",
        "validator_status",
        "structural_status",
        "visual_status",
        "human_status",
        "engine_status",
        "distribution_status",
        "quality_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality result must be an object".to_owned()
    })?;
    if !object_has_exact_fields(object, &result_fields) {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality result fields differ".to_owned());
    }
    let expected_schema = if is_prepare {
        "ProductionWeaponFormQualityPrepareResult@1"
    } else {
        "ProductionWeaponFormQualityGetResult@1"
    };
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("replayed").and_then(Value::as_bool).is_none()
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality side-effect flags differ".to_owned(),
        );
    }
    let quality = value
        .get("form_quality")
        .and_then(Value::as_object)
        .ok_or_else(|| "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality record missing".to_owned())?;
    if !object_has_exact_fields(quality, &record_fields)
        || quality.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormQuality@1")
        || quality.get("form_quality_policy").and_then(Value::as_str)
            != Some("production-weapon-form-quality-six-view-no-regression@1")
        || quality.get("threshold_policy").and_then(Value::as_str)
            != Some("production-weapon-form-view-thresholds@1")
        || quality.get("structural_status").and_then(Value::as_str)
            != Some("PASS_SOURCE_STRUCTURAL")
        || quality.get("visual_status").and_then(Value::as_str) != Some("QUALITY_TARGET_NOT_MET")
        || quality.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || quality.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("distribution_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("runtime_write_performed") != Some(&Value::Bool(true))
        || quality.get("production_stage_advanced") != Some(&Value::Bool(false))
        || quality.get("candidate_confirmed") != Some(&Value::Bool(false))
        || quality.get("version_created") != Some(&Value::Bool(false))
        || quality.get("export_performed") != Some(&Value::Bool(false))
        || contains_forbidden_transport_field(value)
        || contains_raw_media_field(value)
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality status, lineage or media boundary differs".to_owned());
    }
    for field in [
        "camera_calibrated_head_transition_sha256",
        "camera_calibrated_head_canonical_sha256",
        "camera_calibrated_head_candidate_state_sha256",
        "camera_calibrated_head_artifact_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "cross_view_evidence_object_sha256",
        "cross_view_evidence_canonical_sha256",
        "form_evidence_object_sha256",
        "form_evidence_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(quality.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality hash {field} is invalid"
            ));
        }
    }
    for field in [
        "form_quality_id",
        "session_id",
        "project_id",
        "camera_calibrated_head_transition_id",
        "camera_calibrated_head_candidate_id",
        "camera_calibrated_head_artifact_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
    ] {
        if quality
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality id {field} is invalid"
            ));
        }
    }
    if quality.get("reviewed_reference_view_kinds")
        != Some(&json!([
            "front",
            "back",
            "left",
            "right",
            "top",
            "rear-three-quarter"
        ]))
        || quality.get("fixed_camera_view_kinds")
            != Some(&json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "bottom",
                "rear-three-quarter"
            ]))
        || quality.get("cross_view_evidence_view_kinds")
            != Some(&json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "rear-three-quarter"
            ]))
        || quality
            .get("form_view_evaluations")
            .and_then(Value::as_array)
            .is_none_or(|views| views.len() != 6)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality view coverage differs".to_owned(),
        );
    }
    validate_form_view_evaluations(&quality["form_view_evaluations"])?;
    if binding.is_bound()
        && (binding.session_id.as_deref() != quality.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != quality.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != quality.get("candidate_id").and_then(Value::as_str))
    {
        return Err("AGENTIC_SCOPE_MISMATCH: form-quality response crossed the bound session/project/candidate".to_owned());
    }
    Ok(())
}

fn validate_production_weapon_form_quality_v2_preflight_response(
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 21] = [
        "schema_version",
        "preflight_id",
        "session_id",
        "project_id",
        "candidate_id",
        "form_stage",
        "checks",
        "ready_for_v2_prepare",
        "blocking_reasons",
        "quality_status",
        "visual_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write",
        "worker_started",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "restart_hash_verified",
        "readiness_sha256",
    ];
    const CHECK_FIELDS: [&str; 4] = ["status", "reason_code", "object_sha256", "canonical_sha256"];
    const CHECK_NAMES: [&str; 7] = [
        "legacy_form_quality",
        "form_art_evidence",
        "form_art_target_observation",
        "cross_view_evidence",
        "camera_lock_stage",
        "reference_authoring",
        "candidate_artifact",
    ];
    const FRESH_CHECK_NAMES: [&str; 9] = [
        "legacy_form_quality",
        "form_art_evidence",
        "form_art_target_observation",
        "cross_view_evidence",
        "camera_lock_stage",
        "reference_authoring",
        "candidate_artifact",
        "fresh_baseline_registration",
        "proposal_candidate_evidence",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight result must be an object"
            .to_owned()
    })?;
    let result_fields_allowed = RESULT_FIELDS
        .iter()
        .all(|field| object.contains_key(*field))
        && object.keys().all(|field| {
            RESULT_FIELDS.contains(&field.as_str())
                || FORM_QUALITY_V2_SCOPE_FIELDS.contains(&field.as_str())
        });
    if !result_fields_allowed {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight result fields differ"
                .to_owned(),
        );
    }
    if value.get("schema_version").and_then(Value::as_str)
        != Some("ProductionWeaponFormQualityV2PreflightGetResult@1")
        || value.get("quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            != Some("NOT_RUN")
        || value.get("runtime_write") != Some(&Value::Bool(false))
        || value.get("worker_started") != Some(&Value::Bool(false))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight status, side-effect flags or media boundary differs"
                .to_owned(),
        );
    }
    for field in ["preflight_id", "session_id", "project_id", "candidate_id"] {
        if !valid_v2_id(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight id {field} is invalid"
            ));
        }
    }
    if !matches!(
        object.get("form_stage").and_then(Value::as_str),
        Some("blockout" | "primary" | "secondary")
    ) || !valid_sha256(object.get("readiness_sha256").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight stage or readiness hash is invalid"
                .to_owned(),
        );
    }

    if object.contains_key("evidence_source_kind") {
        validate_production_weapon_form_quality_v2_scope_input(value)?;
    }
    let checks = object
        .get("checks")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight checks are missing".to_owned()
        })?;
    let check_names: &[&str] = if object.get("evidence_source_kind").and_then(Value::as_str)
        == Some("fresh-baseline-proposal")
    {
        &FRESH_CHECK_NAMES
    } else {
        &CHECK_NAMES
    };
    if !object_has_exact_fields(checks, check_names) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check names differ".to_owned(),
        );
    }
    let optional_hash_valid = |value: Option<&Value>| {
        value.is_some_and(|value| value.is_null() || valid_sha256(value.as_str()))
    };
    let valid_reason_code = |reason: &str| {
        !reason.is_empty()
            && reason.len() <= 96
            && reason
                .bytes()
                .all(|byte| matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | b'_'))
    };
    let mut expected_blocking = Vec::new();
    for check_name in CHECK_NAMES {
        let check = checks.get(check_name).and_then(Value::as_object).ok_or_else(|| {
            format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check {check_name} is not an object"
            )
        })?;
        if !object_has_exact_fields(check, &CHECK_FIELDS) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check {check_name} fields differ"
            ));
        }
        let status = check.get("status").and_then(Value::as_str).ok_or_else(|| {
            format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check {check_name} status is invalid"
            )
        })?;
        let reason_code = check
            .get("reason_code")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check {check_name} reason is invalid"
                )
            })?;
        let object_hash = check.get("object_sha256");
        let canonical_hash = check.get("canonical_sha256");
        let ready_hash_pair = (object_hash.is_some_and(|value| value.is_null())
            && canonical_hash.is_some_and(|value| value.is_null()))
            || (valid_sha256(object_hash.and_then(Value::as_str))
                && valid_sha256(canonical_hash.and_then(Value::as_str)));
        if !matches!(status, "ready" | "blocked" | "invalid")
            || !valid_reason_code(reason_code)
            || (status == "ready" && reason_code != "READY")
            || (status != "ready" && reason_code == "READY")
            || !optional_hash_valid(object_hash)
            || !optional_hash_valid(canonical_hash)
            || (status == "ready" && !ready_hash_pair)
            || (status == "blocked"
                && (!object_hash.is_some_and(|value| value.is_null())
                    || !canonical_hash.is_some_and(|value| value.is_null())))
            || (status == "invalid" && !canonical_hash.is_some_and(|value| value.is_null()))
        {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight check {check_name} status, reason or hash differs"
            ));
        }
        if status != "ready" {
            expected_blocking.push(format!("{check_name}:{reason_code}"));
        }
    }
    expected_blocking.sort();
    let actual_blocking = object
        .get("blocking_reasons")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight blockers are missing"
                .to_owned()
        })?
        .iter()
        .map(|reason| reason.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight blocker is not a string"
                .to_owned()
        })?;
    if actual_blocking != expected_blocking
        || object.get("ready_for_v2_prepare") != Some(&Value::Bool(expected_blocking.is_empty()))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight readiness or blockers differ"
                .to_owned(),
        );
    }
    let readiness_sha256 = object
        .get("readiness_sha256")
        .and_then(Value::as_str)
        .expect("readiness_sha256 was validated above");
    let mut readiness_preimage = value.clone();
    readiness_preimage["readiness_sha256"] = Value::String(String::new());
    if readiness_sha256 != forgecad_runtime::canonical_json_hash(&readiness_preimage) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: FormQuality@2 preflight readiness hash differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != object.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != object.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != object.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: FormQuality@2 preflight response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_production_weapon_high_low_bake_preflight_response(
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 26] = [
        "schema_version",
        "preflight_id",
        "session_id",
        "project_id",
        "candidate_id",
        "expected_head_stage",
        "observed_head_stage",
        "observed_head_transition_id",
        "observed_head_transition_sha256",
        "observed_head_canonical_sha256",
        "checks",
        "ready_for_formal_bake",
        "blocking_reasons",
        "quality_status",
        "visual_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "distribution_status",
        "runtime_write",
        "worker_started",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "restart_hash_verified",
        "readiness_sha256",
    ];
    const CHECK_FIELDS: [&str; 4] = ["status", "reason_code", "object_sha256", "canonical_sha256"];
    // BTreeMap order is part of the Runtime projection: blocking_reasons are
    // emitted in this exact lexical order and contain reason codes only.
    const CHECK_NAMES: [&str; 8] = [
        "authoring_low_topology",
        "formal_bake",
        "formal_cage_artifact",
        "formal_high_artifact",
        "hero_uv_layout",
        "high_low_correspondence",
        "ray_diagnostic",
        "secondary_form_head",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight result must be an object".to_owned()
    })?;
    if !object_has_exact_fields(object, &RESULT_FIELDS) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight result fields differ".to_owned(),
        );
    }
    if value.get("schema_version").and_then(Value::as_str)
        != Some("ProductionWeaponHighLowBakePreflightGetResult@1")
        || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || value.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || value.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            != Some("NOT_RUN")
        || value.get("distribution_status").and_then(Value::as_str) != Some("NOT_RUN")
        || value.get("runtime_write") != Some(&Value::Bool(false))
        || value.get("worker_started") != Some(&Value::Bool(false))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight status, side-effect flags or media boundary differs"
                .to_owned(),
        );
    }
    for field in ["preflight_id", "session_id", "project_id", "candidate_id"] {
        if !valid_v2_id(object.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight id {field} is invalid"
            ));
        }
    }
    if object
        .get("expected_head_stage")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
        || !valid_sha256(object.get("readiness_sha256").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight expected stage or readiness hash is invalid"
                .to_owned(),
        );
    }

    let nullable_id_valid = |value: Option<&Value>| {
        value.is_some_and(|value| value.is_null() || valid_v2_id(value.as_str()))
    };
    let nullable_hash_valid = |value: Option<&Value>| {
        value.is_some_and(|value| value.is_null() || valid_sha256(value.as_str()))
    };
    let nullable_stage_valid = |value: Option<&Value>| {
        value.is_some_and(|value| {
            value.is_null()
                || value
                    .as_str()
                    .is_some_and(|stage| !stage.is_empty() && stage.len() <= 64)
        })
    };
    let observed_all_null = object
        .get("observed_head_stage")
        .is_some_and(Value::is_null)
        && object
            .get("observed_head_transition_id")
            .is_some_and(Value::is_null)
        && object
            .get("observed_head_transition_sha256")
            .is_some_and(Value::is_null)
        && object
            .get("observed_head_canonical_sha256")
            .is_some_and(Value::is_null);
    let observed_all_present = object
        .get("observed_head_stage")
        .and_then(Value::as_str)
        .is_some()
        && object
            .get("observed_head_transition_id")
            .and_then(Value::as_str)
            .is_some()
        && object
            .get("observed_head_transition_sha256")
            .and_then(Value::as_str)
            .is_some()
        && object
            .get("observed_head_canonical_sha256")
            .and_then(Value::as_str)
            .is_some();
    if !nullable_stage_valid(object.get("observed_head_stage"))
        || !nullable_id_valid(object.get("observed_head_transition_id"))
        || !nullable_hash_valid(object.get("observed_head_transition_sha256"))
        || !nullable_hash_valid(object.get("observed_head_canonical_sha256"))
        || !(observed_all_null || observed_all_present)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake observed head binding is partial or invalid"
                .to_owned(),
        );
    }

    let checks = object
        .get("checks")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight checks are missing".to_owned()
        })?;
    if !object_has_exact_fields(checks, &CHECK_NAMES) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check names differ".to_owned(),
        );
    }
    let valid_reason_code = |reason: &str| {
        !reason.is_empty()
            && reason.len() <= 128
            && reason
                .bytes()
                .all(|byte| matches!(byte, b'A'..=b'Z' | b'0'..=b'9' | b'_'))
    };
    let mut expected_blocking = Vec::new();
    for check_name in CHECK_NAMES {
        let check = checks.get(check_name).and_then(Value::as_object).ok_or_else(|| {
            format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check {check_name} is not an object"
            )
        })?;
        if !object_has_exact_fields(check, &CHECK_FIELDS) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check {check_name} fields differ"
            ));
        }
        let status = check.get("status").and_then(Value::as_str).ok_or_else(|| {
            format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check {check_name} status is invalid"
            )
        })?;
        let reason_code = check
            .get("reason_code")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check {check_name} reason is invalid"
                )
            })?;
        if !matches!(status, "passed" | "blocked" | "missing" | "failed")
            || (check_name != "secondary_form_head" && status == "passed")
            || !valid_reason_code(reason_code)
            || !nullable_hash_valid(check.get("object_sha256"))
            || !nullable_hash_valid(check.get("canonical_sha256"))
        {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight check {check_name} status, reason or hash differs"
            ));
        }
        if check_name != "formal_bake" && status != "passed" {
            expected_blocking.push(reason_code.to_owned());
        }
    }
    let actual_blocking = object
        .get("blocking_reasons")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight blockers are missing".to_owned()
        })?
        .iter()
        .map(|reason| reason.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight blocker is not a string"
                .to_owned()
        })?;
    if actual_blocking != expected_blocking
        || object.get("ready_for_formal_bake") != Some(&Value::Bool(expected_blocking.is_empty()))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight readiness or blockers differ"
                .to_owned(),
        );
    }

    let readiness_sha256 = object
        .get("readiness_sha256")
        .and_then(Value::as_str)
        .expect("readiness_sha256 was validated above");
    let mut readiness_preimage = value.clone();
    readiness_preimage["readiness_sha256"] = Value::String(String::new());
    if readiness_sha256 != forgecad_runtime::canonical_json_hash(&readiness_preimage) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: HighLowBake preflight readiness hash differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != object.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != object.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != object.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: HighLowBake preflight response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_production_weapon_form_quality_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::ProductionWeaponFormQualityV2Prepare;
    const RESULT_FIELDS_PREPARE: [&str; 8] = [
        "schema_version",
        "form_quality",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    const RESULT_FIELDS_GET: [&str; 9] = [
        "schema_version",
        "form_quality",
        "replayed",
        "runtime_write",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "restart_hash_verified",
    ];
    const RECORD_FIELDS: [&str; 68] = [
        "schema_version",
        "form_quality_id",
        "session_id",
        "project_id",
        "form_stage",
        "source_stage",
        "target_stage",
        "current_source_head_transition_id",
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "current_source_head_stage",
        "current_source_head_candidate_id",
        "current_source_head_candidate_state_sha256",
        "current_source_head_artifact_id",
        "current_source_head_artifact_sha256",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_id",
        "artifact_sha256",
        "reference_id",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_hash",
        "camera_lock_id",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_id",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "reviewed_reference_view_kinds",
        "fixed_camera_view_kinds",
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "view_decisions",
        "aggregate",
        "previous_form_quality_id",
        "previous_form_quality_report_object_sha256",
        "previous_form_quality_canonical_sha256",
        "form_quality_policy",
        "form_quality_policy_sha256",
        "threshold_policy",
        "threshold_policy_sha256",
        "hard_gate_passed",
        "form_gate_passed",
        "validator_status",
        "structural_status",
        "visual_status",
        "human_status",
        "engine_status",
        "distribution_status",
        "quality_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ];
    const DECISION_FIELDS: [&str; 18] = [
        "view_kind",
        "legacy_form_quality_view_id",
        "legacy_form_quality_view_canonical_sha256",
        "form_art_view_id",
        "form_art_view_canonical_sha256",
        "form_art_view_receipt_object_sha256",
        "target_object_sha256",
        "target_canonical_sha256",
        "silhouette_pass_object_sha256",
        "part_id_pass_object_sha256",
        "depth_pass_object_sha256",
        "normal_pass_object_sha256",
        "cross_view_thresholds_passed",
        "no_regression_passed",
        "part_id_passed",
        "negative_space_passed",
        "line_flow_passed",
        "view_passed",
    ];
    const AGGREGATE_FIELDS: [&str; 7] = [
        "view_count",
        "all_cross_view_thresholds_passed",
        "all_no_regression_passed",
        "all_part_id_passed",
        "all_negative_space_passed",
        "all_line_flow_passed",
        "all_view_passed",
    ];
    let object = value.as_object().ok_or_else(|| {
        "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 result must be an object".to_owned()
    })?;
    let result_fields = if is_prepare {
        &RESULT_FIELDS_PREPARE[..]
    } else {
        &RESULT_FIELDS_GET[..]
    };
    if !object_has_exact_fields(object, result_fields) {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 result fields differ".to_owned(),
        );
    }
    let expected_schema = if is_prepare {
        "ProductionWeaponFormQualityPrepareResult@2"
    } else {
        "ProductionWeaponFormQualityGetResult@2"
    };
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("runtime_write") != Some(&Value::Bool(is_prepare))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || (!is_prepare && value.get("restart_hash_verified") != Some(&Value::Bool(true)))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 side-effect or restart flags differ"
                .to_owned(),
        );
    }
    let quality = value
        .get("form_quality")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 record missing".to_owned()
        })?;
    let policy_sha256 = forgecad_runtime::sha256_hex(
        b"production-weapon-form-quality-six-view-art-evidence-gate@2",
    );
    let threshold_sha256 =
        forgecad_runtime::sha256_hex(b"production-weapon-form-view-thresholds@1");
    let record_fields_allowed = RECORD_FIELDS
        .iter()
        .all(|field| quality.contains_key(*field))
        && quality.keys().all(|field| {
            RECORD_FIELDS.contains(&field.as_str())
                || FORM_QUALITY_V2_SCOPE_FIELDS.contains(&field.as_str())
        });
    if !record_fields_allowed
        || quality.get("schema_version").and_then(Value::as_str)
            != Some("ProductionWeaponFormQuality@2")
        || quality.get("form_quality_policy").and_then(Value::as_str)
            != Some("production-weapon-form-quality-six-view-art-evidence-gate@2")
        || quality
            .get("form_quality_policy_sha256")
            .and_then(Value::as_str)
            != Some(policy_sha256.as_str())
        || quality.get("threshold_policy").and_then(Value::as_str)
            != Some("production-weapon-form-view-thresholds@1")
        || quality
            .get("threshold_policy_sha256")
            .and_then(Value::as_str)
            != Some(threshold_sha256.as_str())
        || quality.get("hard_gate_passed") != Some(&Value::Bool(true))
        || quality.get("form_gate_passed") != Some(&Value::Bool(true))
        || quality.get("validator_status").and_then(Value::as_str) != Some("passed")
        || quality.get("structural_status").and_then(Value::as_str)
            != Some("PASS_SOURCE_STRUCTURAL")
        || quality.get("visual_status").and_then(Value::as_str)
            != Some("PASS_STAGE_VISUAL_STRUCTURE_ONLY")
        || quality.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("distribution_status").and_then(Value::as_str) != Some("NOT_RUN")
        || quality.get("quality_status").and_then(Value::as_str) != Some("PASS_FORM_GATE")
        || quality.get("runtime_write_performed") != Some(&Value::Bool(true))
        || quality.get("production_stage_advanced") != Some(&Value::Bool(false))
        || quality.get("candidate_confirmed") != Some(&Value::Bool(false))
        || quality.get("version_created") != Some(&Value::Bool(false))
        || quality.get("export_performed") != Some(&Value::Bool(false))
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 status, fields or media boundary differs"
                .to_owned(),
        );
    }
    if quality.contains_key("evidence_source_kind") {
        validate_production_weapon_form_quality_v2_scope_input(&Value::Object(quality.clone()))?;
    }
    for field in [
        "current_source_head_transition_sha256",
        "current_source_head_canonical_sha256",
        "current_source_head_candidate_state_sha256",
        "current_source_head_artifact_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "reference_sha256",
        "reference_canvas_object_sha256",
        "reference_canvas_canonical_sha256",
        "design_spec_object_sha256",
        "design_spec_canonical_sha256",
        "camera_hash",
        "camera_lock_canonical_sha256",
        "camera_rig_object_sha256",
        "camera_rig_canonical_sha256",
        "camera_lock_receipt_object_sha256",
        "camera_lock_source_transition_sha256",
        "camera_lock_source_head_canonical_sha256",
        "legacy_form_quality_object_sha256",
        "legacy_form_quality_canonical_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "form_quality_policy_sha256",
        "threshold_policy_sha256",
        "request_sha256",
        "input_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ] {
        if !valid_sha256(quality.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 hash {field} is invalid"
            ));
        }
    }
    for field in [
        "form_quality_id",
        "session_id",
        "project_id",
        "current_source_head_transition_id",
        "current_source_head_candidate_id",
        "current_source_head_artifact_id",
        "candidate_id",
        "artifact_id",
        "reference_id",
        "camera_lock_id",
        "camera_lock_source_transition_id",
    ] {
        if !valid_v2_id(quality.get(field).and_then(Value::as_str)) {
            return Err(format!(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 id {field} is invalid"
            ));
        }
    }
    let form_stage = quality.get("form_stage").and_then(Value::as_str);
    let expected_stages = match form_stage {
        Some("blockout") => ("camera-calibrated", "blockout-reviewed"),
        Some("primary") => ("blockout-reviewed", "primary-form-approved"),
        Some("secondary") => ("primary-form-approved", "secondary-form-approved"),
        _ => {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 form stage is invalid".to_owned(),
            )
        }
    };
    if quality.get("source_stage").and_then(Value::as_str) != Some(expected_stages.0)
        || quality.get("target_stage").and_then(Value::as_str) != Some(expected_stages.1)
        || quality
            .get("current_source_head_stage")
            .and_then(Value::as_str)
            != Some(expected_stages.0)
        || quality.get("current_source_head_candidate_id") != quality.get("candidate_id")
        || quality.get("current_source_head_candidate_state_sha256")
            != quality.get("candidate_state_sha256")
        || quality.get("current_source_head_artifact_id") != quality.get("artifact_id")
        || quality.get("current_source_head_artifact_sha256") != quality.get("artifact_sha256")
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 stage/source-head binding differs"
                .to_owned(),
        );
    }
    if quality.get("reviewed_reference_view_kinds")
        != Some(&json!([
            "front",
            "back",
            "left",
            "right",
            "top",
            "rear-three-quarter"
        ]))
        || quality.get("fixed_camera_view_kinds")
            != Some(&json!([
                "front",
                "back",
                "left",
                "right",
                "top",
                "bottom",
                "rear-three-quarter"
            ]))
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 fixed/reviewed view coverage differs"
                .to_owned(),
        );
    }
    let previous_id = quality.get("previous_form_quality_id");
    let previous_object = quality.get("previous_form_quality_report_object_sha256");
    let previous_canonical = quality.get("previous_form_quality_canonical_sha256");
    let previous_is_null = previous_id == Some(&Value::Null)
        && previous_object == Some(&Value::Null)
        && previous_canonical == Some(&Value::Null);
    let previous_is_bound = valid_v2_id(previous_id.and_then(Value::as_str))
        && valid_sha256(previous_object.and_then(Value::as_str))
        && valid_sha256(previous_canonical.and_then(Value::as_str));
    if (form_stage == Some("blockout") && !previous_is_null)
        || (matches!(form_stage, Some("primary" | "secondary")) && !previous_is_bound)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 previous quality lineage differs"
                .to_owned(),
        );
    }
    let decisions = quality
        .get("view_decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 view decisions missing".to_owned()
        })?;
    let expected_views = [
        "front",
        "back",
        "left",
        "right",
        "top",
        "rear-three-quarter",
    ];
    if decisions.len() != expected_views.len() {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 view decision count differs".to_owned(),
        );
    }
    for (decision, expected_view) in decisions.iter().zip(expected_views) {
        let decision = decision.as_object().ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 view decision is not an object"
                .to_owned()
        })?;
        if !object_has_exact_fields(decision, &DECISION_FIELDS)
            || decision.get("view_kind").and_then(Value::as_str) != Some(expected_view)
            || ![
                "cross_view_thresholds_passed",
                "no_regression_passed",
                "part_id_passed",
                "negative_space_passed",
                "line_flow_passed",
                "view_passed",
            ]
            .iter()
            .all(|field| decision.get(*field) == Some(&Value::Bool(true)))
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 view decision flags differ"
                    .to_owned(),
            );
        }
        for field in ["legacy_form_quality_view_id", "form_art_view_id"] {
            if !valid_v2_id(decision.get(field).and_then(Value::as_str)) {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 decision id {field} is invalid"
                ));
            }
        }
        for field in [
            "legacy_form_quality_view_canonical_sha256",
            "form_art_view_canonical_sha256",
            "form_art_view_receipt_object_sha256",
            "target_object_sha256",
            "target_canonical_sha256",
            "silhouette_pass_object_sha256",
            "part_id_pass_object_sha256",
            "depth_pass_object_sha256",
            "normal_pass_object_sha256",
        ] {
            if !valid_sha256(decision.get(field).and_then(Value::as_str)) {
                return Err(format!(
                    "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 decision hash {field} is invalid"
                ));
            }
        }
    }
    let aggregate = quality
        .get("aggregate")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 aggregate missing".to_owned()
        })?;
    if !object_has_exact_fields(aggregate, &AGGREGATE_FIELDS)
        || aggregate.get("view_count") != Some(&Value::Number(6.into()))
        || [
            "all_cross_view_thresholds_passed",
            "all_no_regression_passed",
            "all_part_id_passed",
            "all_negative_space_passed",
            "all_line_flow_passed",
            "all_view_passed",
        ]
        .iter()
        .any(|field| aggregate.get(*field) != Some(&Value::Bool(true)))
    {
        return Err("AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 aggregate differs".to_owned());
    }
    if quality
        .get("created_at")
        .and_then(Value::as_str)
        .is_none_or(|created_at| created_at.is_empty() || created_at.len() > 128)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: form-quality@2 timestamp is invalid".to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.session_id.as_deref() != quality.get("session_id").and_then(Value::as_str)
            || binding.project_id.as_deref() != quality.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != quality.get("candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: form-quality@2 response crossed the bound session/project/candidate"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_mechanical_animation_glb_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::MechanicalAnimationGlbV2Prepare;
    let expected_schema = if is_prepare {
        "MechanicalAnimationGlbPrepareResult@2"
    } else {
        "MechanicalAnimationGlbGetResult@2"
    };
    let receipt = value
        .get("receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationGlb@2 response is missing receipt"
                .to_owned()
        })?;
    let durable_link = value
        .get("durable_link")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationGlb@2 response is missing durable_link"
                .to_owned()
        })?;

    let key = value
        .get("animation_glb_key_sha256")
        .and_then(Value::as_str);
    let artifact = value
        .get("animated_artifact_sha256")
        .and_then(Value::as_str);
    let receipt_object = value.get("receipt_object_sha256").and_then(Value::as_str);
    let project_id = receipt.get("project_id").and_then(Value::as_str);
    let appearance_candidate_id = receipt
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let clip_id = receipt.get("clip_id").and_then(Value::as_str);
    let candidate_state = receipt
        .get("appearance_candidate_state_sha256")
        .and_then(Value::as_str);

    const RECEIPT_HASHES: &[&str] = &[
        "animation_glb_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "material_pack_manifest_object_sha256",
        "material_pack_manifest_sha256",
        "material_pack_provenance_sha256",
        "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256",
        "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "sampling_policy_sha256",
        "source_replay_worker_cohort_sha256",
        "frame_preview_hashes_sha256",
        "frame_preview_worker_cohort_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_validation_sha256",
        "source_static_projection_sha256",
        "appearance_material_projection_sha256",
        "canonical_sha256",
    ];
    const LINK_HASHES: &[&str] = &[
        "animation_glb_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "material_pack_manifest_object_sha256",
        "material_pack_manifest_sha256",
        "material_pack_provenance_sha256",
        "texture_build_receipt_object_sha256",
        "texture_build_receipt_canonical_sha256",
        "candidate_surface_bake_receipt_object_sha256",
        "candidate_surface_bake_receipt_canonical_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "sampling_policy_sha256",
        "source_replay_worker_cohort_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "receipt_object_sha256",
        "receipt_canonical_sha256",
        "request_sha256",
        "canonical_sha256",
    ];
    let receipt_hashes_valid = RECEIPT_HASHES
        .iter()
        .all(|field| valid_sha256(receipt.get(*field).and_then(Value::as_str)));
    let link_hashes_valid = LINK_HASHES
        .iter()
        .all(|field| valid_sha256(durable_link.get(*field).and_then(Value::as_str)));
    let receipt_ticks_valid = receipt
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .is_some_and(|ticks| {
            (2..=16).contains(&ticks.len())
                && ticks.iter().all(Value::is_u64)
                && ticks
                    .windows(2)
                    .all(|pair| pair[0].as_u64() < pair[1].as_u64())
        });
    let part_ids_valid = receipt
        .get("part_ids")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            !parts.is_empty()
                && parts.len() <= 64
                && parts.iter().all(|part| {
                    part.as_str()
                        .is_some_and(|value| !value.is_empty() && value.len() <= 128)
                })
        });
    let counts_valid = receipt
        .get("node_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| (1..=64).contains(&count))
        && receipt
            .get("sampler_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (2..=128).contains(&count))
        && receipt
            .get("channel_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (2..=128).contains(&count))
        && receipt
            .get("accessor_count_added")
            .and_then(Value::as_u64)
            .is_some_and(|count| (3..=129).contains(&count))
        && receipt
            .get("buffer_view_count_added")
            .and_then(Value::as_u64)
            .is_some_and(|count| (3..=129).contains(&count));
    let receipt_flags_safe = receipt.get("runtime_write_performed") == Some(&Value::Bool(true))
        && receipt.get("production_stage_advanced") == Some(&Value::Bool(false))
        && receipt.get("candidate_confirmed") == Some(&Value::Bool(false))
        && receipt.get("version_created") == Some(&Value::Bool(false))
        && receipt.get("export_performed") == Some(&Value::Bool(false));
    let link_flags_safe = durable_link.get("runtime_write_performed") == Some(&Value::Bool(true))
        && durable_link.get("production_stage_advanced") == Some(&Value::Bool(false))
        && durable_link.get("candidate_confirmed") == Some(&Value::Bool(false))
        && durable_link.get("version_created") == Some(&Value::Bool(false))
        && durable_link.get("export_performed") == Some(&Value::Bool(false));
    let receipt_projection_flags = [
        "source_static_projection_exact",
        "binary_prefix_exact",
        "appearance_material_projection_exact",
        "material_pack_identity_exact",
        "no_skinning",
        "no_morph_targets",
    ]
    .iter()
    .all(|field| receipt.get(*field) == Some(&Value::Bool(true)));
    let structural_status = |object: &Map<String, Value>| {
        object.get("quality_status").and_then(Value::as_str) == Some("structural_only")
            && object.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
            && object
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                == Some("NOT_PROVEN")
            && object.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
            && object
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                == Some("NOT_RUN")
    };
    let top_flags_safe = value.get("restart_hash_verified") == Some(&Value::Bool(true))
        && value.get("replayed").and_then(Value::as_bool).is_some()
        && value.get("runtime_write_performed") == Some(&Value::Bool(is_prepare))
        && value.get("production_stage_advanced") == Some(&Value::Bool(false))
        && value.get("candidate_confirmed") == Some(&Value::Bool(false))
        && value.get("version_created") == Some(&Value::Bool(false))
        && value.get("export_performed") == Some(&Value::Bool(false))
        && value.get("quality_status").and_then(Value::as_str) == Some("structural_only");
    let replay_flag_safe = is_prepare || value.get("replayed") == Some(&Value::Bool(false));
    let binding_safe = binding
        .is_bound()
        .then_some(
            binding.project_id.as_deref() == project_id
                && binding.candidate_id.as_deref() == appearance_candidate_id,
        )
        .unwrap_or(true);

    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || !valid_sha256(key)
        || !valid_sha256(artifact)
        || !valid_sha256(receipt_object)
        || value
            .get("animated_artifact_size_bytes")
            .and_then(Value::as_u64)
            .is_none_or(|size| size == 0 || size > 64 * 1024 * 1024)
        || receipt.get("schema_version").and_then(Value::as_str)
            != Some("MechanicalAnimationGlbReceipt@2")
        || durable_link.get("schema_version").and_then(Value::as_str)
            != Some("MechanicalAnimationGlbLink@2")
        || receipt
            .get("animation_glb_key_sha256")
            .and_then(Value::as_str)
            != key
        || durable_link
            .get("animation_glb_key_sha256")
            .and_then(Value::as_str)
            != key
        || receipt
            .get("animated_artifact_sha256")
            .and_then(Value::as_str)
            != artifact
        || durable_link
            .get("animated_artifact_sha256")
            .and_then(Value::as_str)
            != artifact
        || durable_link
            .get("receipt_object_sha256")
            .and_then(Value::as_str)
            != receipt_object
        || durable_link
            .get("receipt_canonical_sha256")
            .and_then(Value::as_str)
            != receipt.get("canonical_sha256").and_then(Value::as_str)
        || durable_link.get("project_id").and_then(Value::as_str) != project_id
        || durable_link
            .get("appearance_candidate_id")
            .and_then(Value::as_str)
            != appearance_candidate_id
        || durable_link
            .get("appearance_candidate_state_sha256")
            .and_then(Value::as_str)
            != candidate_state
        || !receipt_hashes_valid
        || !link_hashes_valid
        || receipt.get("validator_status").and_then(Value::as_str)
            != Some("strict-appearance-aware-rigid-gltf-animation-readback-pass")
        || durable_link.get("validator_status").and_then(Value::as_str)
            != Some("strict-appearance-aware-rigid-gltf-animation-readback-pass")
        || receipt.get("hard_gate_passed") != Some(&Value::Bool(true))
        || durable_link.get("hard_gate_passed") != Some(&Value::Bool(true))
        || receipt
            .get("materialization_status")
            .and_then(Value::as_str)
            != Some("runtime-owned-cas-appearance-aware-animated-glb")
        || durable_link
            .get("materialization_status")
            .and_then(Value::as_str)
            != Some("runtime-owned-cas-appearance-aware-animated-glb")
        || !receipt_flags_safe
        || !link_flags_safe
        || !receipt_projection_flags
        || !structural_status(receipt)
        || !structural_status(durable_link)
        || !receipt_ticks_valid
        || !part_ids_valid
        || !counts_valid
        || !top_flags_safe
        || !replay_flag_safe
        || !valid_mechanical_animation_glb_id(project_id)
        || !valid_mechanical_animation_glb_id(appearance_candidate_id)
        || !valid_mechanical_animation_glb_id(clip_id)
        || !binding_safe
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationGlb@2 response schema, receipt/durable-link binding, replay/restart flags, structural status or transport boundary differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_game_weapon_animated_glb_socket_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::GameWeaponAnimatedGlbSocketV2Prepare;
    let expected_schema = if is_prepare {
        "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@2"
    } else {
        "GameWeaponAnimatedGlbSocketMaterializationGetResult@2"
    };
    let receipt = value
        .get("receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated socket materialization response is missing receipt"
                .to_owned()
        })?;
    let durable_link = value
        .get("durable_link")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated socket materialization response is missing durable_link"
                .to_owned()
        })?;

    let key = value
        .get("animated_socket_materialization_key_sha256")
        .and_then(Value::as_str);
    let artifact = value
        .get("derived_animated_socket_artifact_sha256")
        .and_then(Value::as_str);
    let receipt_object = value.get("receipt_object_sha256").and_then(Value::as_str);
    let project_id = receipt.get("project_id").and_then(Value::as_str);
    let appearance_candidate_id = receipt
        .get("appearance_candidate_id")
        .and_then(Value::as_str);
    let clip_id = receipt.get("clip_id").and_then(Value::as_str);

    const RECEIPT_HASHES: &[&str] = &[
        "animated_socket_materialization_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "source_animation_projection_sha256",
        "derived_animation_projection_sha256",
        "source_animation_validation_sha256",
        "derived_animation_validation_sha256",
        "source_renderable_inventory_sha256",
        "derived_renderable_inventory_sha256",
        "source_bin_sha256",
        "derived_bin_sha256",
        "source_appearance_material_projection_sha256",
        "derived_appearance_material_projection_sha256",
        "sampling_policy_sha256",
        "socket_node_inventory_sha256",
        "canonical_sha256",
    ];
    const LINK_HASHES: &[&str] = &[
        "animated_socket_materialization_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
    ];
    let receipt_hashes_valid = RECEIPT_HASHES
        .iter()
        .all(|field| valid_sha256(receipt.get(*field).and_then(Value::as_str)));
    let link_hashes_valid = LINK_HASHES
        .iter()
        .all(|field| valid_sha256(durable_link.get(*field).and_then(Value::as_str)));
    let sample_ticks_valid = receipt
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .is_some_and(|ticks| {
            (2..=16).contains(&ticks.len())
                && ticks
                    .iter()
                    .all(|tick| tick.as_u64().is_some_and(|value| value <= 1_000_000))
                && ticks
                    .windows(2)
                    .all(|pair| pair[0].as_u64() < pair[1].as_u64())
        });
    let part_ids_valid = receipt
        .get("part_ids")
        .and_then(Value::as_array)
        .is_some_and(|parts| {
            !parts.is_empty()
                && parts.len() <= 64
                && parts.iter().all(|part| valid_v2_id(part.as_str()))
                && parts
                    .iter()
                    .enumerate()
                    .all(|(index, part)| parts[..index].iter().all(|previous| previous != part))
        });
    let counts_valid = receipt
        .get("sampler_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| (2..=128).contains(&count))
        && receipt
            .get("channel_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (2..=128).contains(&count))
        && receipt
            .get("node_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (1..=64).contains(&count))
        && receipt
            .get("source_node_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (1..=64).contains(&count))
        && receipt
            .get("derived_node_count")
            .and_then(Value::as_u64)
            .is_some_and(|count| (7..=70).contains(&count))
        && receipt
            .get("accessor_count_added")
            .and_then(Value::as_u64)
            .is_some_and(|count| (3..=129).contains(&count))
        && receipt
            .get("buffer_view_count_added")
            .and_then(Value::as_u64)
            .is_some_and(|count| (3..=129).contains(&count));
    let bounded_numbers =
        |object: &Map<String, Value>, field: &str, length: usize, min: f64, max: f64| {
            object
                .get(field)
                .and_then(Value::as_array)
                .is_some_and(|values| {
                    values.len() == length
                        && values.iter().all(|value| {
                            value.as_f64().is_some_and(|value| {
                                value.is_finite() && (min..=max).contains(&value)
                            })
                        })
                })
        };
    let socket_nodes_valid = receipt
        .get("socket_nodes")
        .and_then(Value::as_array)
        .is_some_and(|nodes| {
            nodes.len() == 6
                && nodes
                    .iter()
                    .enumerate()
                    .all(|(index, node)| nodes[..index].iter().all(|previous| previous != node))
                && nodes.iter().all(|node| {
                    let Some(node) = node.as_object() else {
                        return false;
                    };
                    let role_valid = node
                        .get("role")
                        .and_then(Value::as_str)
                        .is_some_and(|role| {
                            matches!(
                                role,
                                "weapon-root"
                                    | "grip-primary"
                                    | "muzzle-vfx"
                                    | "magazine-well"
                                    | "sight-primary"
                                    | "energy-core-vfx"
                            )
                        });
                    node.get("socket_node_id")
                        .and_then(Value::as_str)
                        .is_some_and(|value| valid_v2_id(Some(value)))
                        && node
                            .get("anchor_id")
                            .and_then(Value::as_str)
                            .is_some_and(|value| valid_v2_id(Some(value)))
                        && node
                            .get("node_name")
                            .and_then(Value::as_str)
                            .is_some_and(|value| valid_v2_id(Some(value)))
                        && role_valid
                        && node.get("node_kind").and_then(Value::as_str) == Some("empty")
                        && node
                            .get("parent_kind")
                            .and_then(Value::as_str)
                            .is_some_and(|kind| {
                                matches!(kind, "synthetic-scene-root" | "part-node")
                            })
                        && bounded_numbers(node, "local_translation_m", 3, -10.0, 10.0)
                        && bounded_numbers(node, "local_rotation_quat_xyzw", 4, -1.0, 1.0)
                        && node.get("local_scale_xyz") == Some(&json!([1.0, 1.0, 1.0]))
                })
        });
    let receipt_projection_flags = [
        "animations_preserved",
        "channels_preserved",
        "samplers_preserved",
        "renderable_projection_exact",
        "bin_byte_exact",
        "source_static_projection_exact",
        "appearance_material_projection_exact",
        "material_pack_identity_exact",
        "no_skinning",
        "no_morph_targets",
        "socket_nodes_materialized",
    ]
    .iter()
    .all(|field| receipt.get(*field) == Some(&Value::Bool(true)));
    let receipt_quality_safe = receipt.get("quality_status").and_then(Value::as_str)
        == Some("structural_only")
        && receipt.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && receipt
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            == Some("NOT_PROVEN")
        && receipt.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
        && receipt
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            == Some("NOT_RUN");
    let created_at_is_bounded = |object: &Map<String, Value>| {
        object
            .get("created_at")
            .and_then(Value::as_str)
            .is_some_and(|created_at| !created_at.is_empty() && created_at.len() <= 64)
    };
    let receipt_flags_safe = receipt.get("runtime_write_performed") == Some(&Value::Bool(true))
        && receipt.get("restart_hash_verified") == Some(&Value::Bool(true))
        && receipt.get("candidate_confirmed") == Some(&Value::Bool(false))
        && receipt.get("version_created") == Some(&Value::Bool(false))
        && receipt.get("export_performed") == Some(&Value::Bool(false))
        && receipt.get("production_stage_advanced") == Some(&Value::Bool(false))
        && receipt.get("actual_engine_roundtrip") == Some(&Value::Bool(false));
    let top_flags_safe = value.get("replayed").and_then(Value::as_bool).is_some()
        && value.get("restart_hash_verified") == Some(&Value::Bool(true))
        && value.get("runtime_write_performed") == Some(&Value::Bool(is_prepare))
        && value.get("candidate_confirmed") == Some(&Value::Bool(false))
        && value.get("version_created") == Some(&Value::Bool(false))
        && value.get("export_performed") == Some(&Value::Bool(false))
        && value.get("production_stage_advanced") == Some(&Value::Bool(false))
        && value.get("actual_engine_roundtrip") == Some(&Value::Bool(false))
        && value.get("quality_status").and_then(Value::as_str) == Some("structural_only");
    let link_status_safe = durable_link.get("schema_version").and_then(Value::as_str)
        == Some("GameWeaponAnimatedGlbSocketMaterializationLink@2")
        && durable_link.get("validator_status").and_then(Value::as_str)
            == Some("strict-appearance-aware-animated-glb-socket-materialization-readback-pass")
        && durable_link.get("hard_gate_passed") == Some(&Value::Bool(true))
        && durable_link
            .get("materialization_status")
            .and_then(Value::as_str)
            == Some("runtime-owned-durable-game-weapon-animated-glb-v2-socket-materialization")
        && durable_link.get("quality_status").and_then(Value::as_str) == Some("structural_only");
    let link_receipt_bindings_are_exact = [
        "animated_socket_materialization_key_sha256",
        "appearance_candidate_state_sha256",
        "appearance_delivery_manifest_object_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "animation_glb_key_sha256",
        "animated_artifact_sha256",
        "animated_artifact_readback_sha256",
        "animation_receipt_object_sha256",
        "animation_receipt_canonical_sha256",
        "clip_id",
        "clip_object_sha256",
        "clip_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "request_sha256",
        "socket_materialization_policy",
        "lod_scope",
        "socket_node_id_encoding_sha256",
        "derived_animated_socket_artifact_sha256",
        "derived_animated_socket_artifact_readback_sha256",
    ]
    .iter()
    .all(|field| durable_link.get(*field) == receipt.get(*field));
    let binding_safe = binding
        .is_bound()
        .then_some(
            binding.project_id.as_deref() == project_id
                && binding.candidate_id.as_deref() == appearance_candidate_id,
        )
        .unwrap_or(true);

    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || !valid_sha256(key)
        || !valid_sha256(artifact)
        || !valid_sha256(receipt_object)
        || receipt.get("schema_version").and_then(Value::as_str)
            != Some("GameWeaponAnimatedGlbSocketMaterializationReceipt@2")
        || receipt.get("animated_socket_materialization_key_sha256").and_then(Value::as_str)
            != key
        || receipt.get("derived_animated_socket_artifact_sha256").and_then(Value::as_str)
            != artifact
        || durable_link.get("animated_socket_materialization_key_sha256").and_then(Value::as_str)
            != key
        || durable_link.get("derived_animated_socket_artifact_sha256").and_then(Value::as_str)
            != artifact
        || durable_link.get("receipt_object_sha256").and_then(Value::as_str)
            != receipt_object
        || durable_link.get("project_id").and_then(Value::as_str) != project_id
        || durable_link
            .get("appearance_candidate_id")
            .and_then(Value::as_str)
            != appearance_candidate_id
        || receipt.get("socket_materialization_policy").and_then(Value::as_str)
            != Some("appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2")
        || receipt.get("lod_scope").and_then(Value::as_str)
            != Some("lod0-appearance-animated-source-only@2")
        || receipt.get("materialization_status").and_then(Value::as_str)
            != Some("runtime-owned-durable-game-weapon-animated-glb-v2-socket-materialization")
        || receipt.get("validator_status").and_then(Value::as_str)
            != Some("strict-appearance-aware-animated-glb-socket-materialization-readback-pass")
        || receipt.get("hard_gate_passed") != Some(&Value::Bool(true))
        || receipt.get("semantic_scope").and_then(Value::as_str)
            != Some("fictional-nonfunctional-game-visual-authoring-only@1")
        || receipt.get("functional_semantics") != Some(&Value::Bool(false))
        || receipt.get("limitations")
            != Some(&json!([
                "appearance-candidate-bound-rigid-Part-TRS-only",
                "scheduled-integer-ticks-and-LINEAR-interpolation-only",
                "no-skinning-morph-targets-armature-IK-constraints-NLA-or-drivers",
                "source-BIN-and-appearance-material-projection-must-remain-exact",
                "structural-readback-does-not-prove-visual-quality-or-engine-roundtrip"
            ]))
        || receipt.get("socket_node_count") != Some(&Value::from(6_u64))
        || receipt.get("owned_cas_kinds")
            != Some(&json!([
                "game-weapon-animated-glb-v2-socket-materialized-glb",
                "game-weapon-animated-glb-v2-socket-materialization-receipt"
            ]))
        || !receipt_hashes_valid
        || !link_hashes_valid
        || !sample_ticks_valid
        || !part_ids_valid
        || !counts_valid
        || !socket_nodes_valid
        || !receipt_projection_flags
        || !receipt_quality_safe
        || !created_at_is_bounded(receipt)
        || !created_at_is_bounded(durable_link)
        || !receipt_flags_safe
        || !link_status_safe
        || !link_receipt_bindings_are_exact
        || !top_flags_safe
        || !valid_v2_id(project_id)
        || !valid_v2_id(appearance_candidate_id)
        || !valid_v2_id(clip_id)
        || !binding_safe
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: V2 animated socket materialization schema, receipt/durable_link binding, replay/restart flags, structural quality boundary or transport boundary differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_mechanical_animation_clip_v2_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    let is_prepare = tool == AgenticTool::MechanicalAnimationClipV2Prepare;
    if tool == AgenticTool::MechanicalAnimationClipV2Preview {
        let required_hashes = [
            "appearance_candidate_state_sha256",
            "appearance_artifact_sha256",
            "appearance_artifact_readback_sha256",
            "appearance_artifact_readback_object_sha256",
            "source_geometry_candidate_state_sha256",
            "source_geometry_artifact_sha256",
            "source_geometry_candidate_evidence_sha256",
            "clip_object_sha256",
            "clip_sha256",
            "rest_frame_sha256",
            "pose_action_sha256",
            "frame_sha256",
            "source_replay_worker_cohort_sha256",
            "appearance_transient_artifact_sha256",
            "appearance_transient_artifact_readback_sha256",
            "appearance_transient_program_sha256",
            "appearance_replay_worker_cohort_sha256",
            "appearance_program_sha256",
            "material_pack_manifest_sha256",
            "geometry_preservation_projection_sha256",
            "canonical_sha256",
        ];
        let all_hashes_valid = required_hashes
            .iter()
            .all(|field| valid_sha256(value.get(*field).and_then(Value::as_str)));
        let project_id = value.get("project_id").and_then(Value::as_str);
        let appearance_candidate_id = value.get("appearance_candidate_id").and_then(Value::as_str);
        let clip_id = value.get("clip_id").and_then(Value::as_str);
        let source_replay_cohort = value
            .get("source_replay_worker_cohort_sha256")
            .and_then(Value::as_str);
        let appearance_replay_cohort = value
            .get("appearance_replay_worker_cohort_sha256")
            .and_then(Value::as_str);
        let posed_program_sha256 = value
            .pointer("/pose_geometry_preview/posed_program_sha256")
            .and_then(Value::as_str);
        let source_geometry_candidate_id = value
            .get("source_geometry_candidate_id")
            .and_then(Value::as_str);
        let source_geometry_artifact_sha256 = value
            .get("source_geometry_artifact_sha256")
            .and_then(Value::as_str);
        let pose_preview_is_bound = value
            .get("pose_geometry_preview")
            .and_then(Value::as_object)
            .is_some_and(|preview| {
                preview.get("project_id").and_then(Value::as_str) == project_id
                    && preview.get("candidate_id").and_then(Value::as_str)
                        == source_geometry_candidate_id
                    && preview.get("source_artifact_id").and_then(Value::as_str)
                        == source_geometry_artifact_sha256
                    && preview.get("runtime_write_performed") == Some(&Value::Bool(false))
                    && preview.get("validator_status").and_then(Value::as_str) == Some("passed")
                    && preview.get("quality_status").and_then(Value::as_str)
                        == Some("structural_only")
            });
        let flags_are_safe = value.get("runtime_write_performed") == Some(&Value::Bool(false))
            && value.get("persistent_user_data_touched") == Some(&Value::Bool(false));
        let status_is_structural = value.get("quality_status").and_then(Value::as_str)
            == Some("structural_only")
            && value.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
            && value
                .get("commercial_fps_quality_status")
                .and_then(Value::as_str)
                == Some("NOT_PROVEN")
            && value.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
            && value
                .get("commercial_engine_status")
                .and_then(Value::as_str)
                == Some("NOT_RUN");
        if value.get("schema_version").and_then(Value::as_str)
            != Some("MechanicalAnimationClipPreview@2")
            || project_id.is_none()
            || appearance_candidate_id.is_none()
            || clip_id.is_none()
            || value
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .is_none_or(|ticks| ticks > 1_000_000)
            || value
                .get("pose_geometry_preview")
                .is_none_or(|preview| !preview.is_object())
            || value
                .get("geometry_materialization")
                .and_then(Value::as_str)
                != Some("transient-double-worker-glb-not-persisted")
            || value
                .get("appearance_materialization")
                .and_then(Value::as_str)
                != Some("transient-double-worker-appearance-not-persisted")
            || !all_hashes_valid
            || source_replay_cohort != appearance_replay_cohort
            || value
                .get("appearance_transient_program_sha256")
                .and_then(Value::as_str)
                != posed_program_sha256
            || !pose_preview_is_bound
            || !flags_are_safe
            || !status_is_structural
            || !value.get("limitations").is_some_and(|limitations| {
                limitations
                    == &json!([
                        "rigid-parts-only-no-skinning-or-deformation",
                        "single-scheduled-tick-per-preview-call",
                        "transient-geometry-and-appearance-not-persisted",
                        "no-ik-constraints-nla-fcurves-drivers-or-timeline",
                        "not-blender-armature-animation-or-python-parity",
                        "structural-replay-does-not-prove-visual-quality"
                    ])
            })
            || contains_raw_media_field(value)
        {
            return Err(
                "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationClip@2 preview schema, transient-only flags, structural status or media boundary differs"
                    .to_owned(),
            );
        }
        if binding.is_bound()
            && (binding.project_id.as_deref() != project_id
                || binding.candidate_id.as_deref() != appearance_candidate_id)
        {
            return Err(
                "AGENTIC_SCOPE_MISMATCH: MechanicalAnimationClip@2 preview crossed the bound project/appearance-candidate binding"
                    .to_owned(),
            );
        }
        return Ok(());
    }

    let expected_schema = if is_prepare {
        "MechanicalAnimationClipPrepareResult@2"
    } else {
        "MechanicalAnimationClipGetResult@2"
    };
    let clip = value
        .get("clip")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationClip@2 response is missing clip"
                .to_owned()
        })?;
    let durable_link = value
        .get("durable_link")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationClip@2 response is missing durable_link"
                .to_owned()
        })?;
    let project_id = clip.get("project_id").and_then(Value::as_str);
    let appearance_candidate_id = clip.get("appearance_candidate_id").and_then(Value::as_str);
    let clip_id = clip.get("clip_id").and_then(Value::as_str);
    let required_clip_hashes = [
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "request_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "sampling_policy_sha256",
        "source_replay_worker_cohort_sha256",
        "canonical_sha256",
    ];
    let required_link_hashes = [
        "appearance_candidate_state_sha256",
        "appearance_artifact_sha256",
        "appearance_artifact_readback_sha256",
        "appearance_artifact_readback_object_sha256",
        "source_geometry_candidate_state_sha256",
        "source_geometry_artifact_sha256",
        "source_geometry_candidate_evidence_sha256",
        "material_surface_quality_report_object_sha256",
        "material_surface_quality_canonical_sha256",
        "appearance_source_lineage_sidecar_object_sha256",
        "appearance_source_lineage_canonical_sha256",
        "appearance_program_object_sha256",
        "appearance_program_sha256",
        "geometry_program_object_sha256",
        "geometry_program_sha256",
        "geometry_preservation_projection_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "request_sha256",
        "source_replay_worker_cohort_sha256",
        "canonical_sha256",
    ];
    let all_hashes_valid = required_clip_hashes
        .iter()
        .all(|field| valid_sha256(clip.get(*field).and_then(Value::as_str)))
        && required_link_hashes
            .iter()
            .all(|field| valid_sha256(durable_link.get(*field).and_then(Value::as_str)));
    let source_replay_is_safe = clip
        .get("source_replay")
        .and_then(Value::as_object)
        .is_some_and(|replay| {
            let appearance_artifact_sha256 = clip
                .get("appearance_artifact_sha256")
                .and_then(Value::as_str);
            let worker_cohort_sha256 = clip
                .get("source_replay_worker_cohort_sha256")
                .and_then(Value::as_str);
            valid_sha256(
                replay
                    .get("worker_build_cohort_sha256")
                    .and_then(Value::as_str),
            ) && valid_sha256(replay.get("first_artifact_sha256").and_then(Value::as_str))
                && valid_sha256(replay.get("repeat_artifact_sha256").and_then(Value::as_str))
                && replay.get("byte_exact_with_appearance_artifact") == Some(&Value::Bool(true))
                && replay
                    .get("worker_build_cohort_sha256")
                    .and_then(Value::as_str)
                    == worker_cohort_sha256
                && replay.get("first_artifact_sha256").and_then(Value::as_str)
                    == appearance_artifact_sha256
                && replay.get("repeat_artifact_sha256").and_then(Value::as_str)
                    == appearance_artifact_sha256
                && replay.get("appearance_materials_replayed") == Some(&Value::Bool(true))
                && replay.get("strict_readback_passed") == Some(&Value::Bool(true))
        });
    let status_is_structural = clip.get("quality_status").and_then(Value::as_str)
        == Some("structural_only")
        && clip.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && clip
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            == Some("NOT_PROVEN")
        && clip.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
        && clip.get("commercial_engine_status").and_then(Value::as_str) == Some("NOT_RUN")
        && durable_link.get("quality_status").and_then(Value::as_str) == Some("structural_only")
        && durable_link
            .get("visual_quality_status")
            .and_then(Value::as_str)
            == Some("NOT_PROVEN")
        && durable_link
            .get("commercial_fps_quality_status")
            .and_then(Value::as_str)
            == Some("NOT_PROVEN")
        && durable_link
            .get("human_review_status")
            .and_then(Value::as_str)
            == Some("NOT_RUN")
        && durable_link
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            == Some("NOT_RUN");
    let clip_flags_are_safe = [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]
    .iter()
    .all(|field| clip.get(*field) == Some(&Value::Bool(false)));
    let link_flags_are_safe = [
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ]
    .iter()
    .all(|field| durable_link.get(*field) == Some(&Value::Bool(false)));
    if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema)
        || value.get("restart_hash_verified") != Some(&Value::Bool(true))
        || value.get("replayed").and_then(Value::as_bool).is_none()
        || value.get("runtime_write_performed") != Some(&Value::Bool(is_prepare))
        || value.get("production_stage_advanced") != Some(&Value::Bool(false))
        || value.get("candidate_confirmed") != Some(&Value::Bool(false))
        || value.get("version_created") != Some(&Value::Bool(false))
        || value.get("export_performed") != Some(&Value::Bool(false))
        || value.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || clip.get("schema_version").and_then(Value::as_str) != Some("MechanicalAnimationClip@2")
        || durable_link.get("schema_version").and_then(Value::as_str)
            != Some("MechanicalAnimationClipLink@2")
        || durable_link.get("project_id").and_then(Value::as_str) != project_id
        || durable_link
            .get("appearance_candidate_id")
            .and_then(Value::as_str)
            != appearance_candidate_id
        || durable_link.get("clip_id").and_then(Value::as_str) != clip_id
        || clip.get("runtime_write_performed") != Some(&Value::Bool(true))
        || durable_link.get("runtime_write_performed") != Some(&Value::Bool(true))
        || !clip_flags_are_safe
        || !link_flags_are_safe
        || !all_hashes_valid
        || !source_replay_is_safe
        || !status_is_structural
        || project_id.is_none()
        || appearance_candidate_id.is_none()
        || clip_id.is_none()
        || contains_raw_media_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: MechanicalAnimationClip@2 response schema, appearance replay binding, structural status, side-effect flags or media boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.project_id.as_deref() != project_id
            || binding.candidate_id.as_deref() != appearance_candidate_id)
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: MechanicalAnimationClip@2 response crossed the bound project/appearance-candidate binding"
                .to_owned(),
        );
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    if name == AgenticTool::SessionCreateOrResume.name() {
        binding.session_id = find_string(value, "session_id", 0).map(str::to_owned);
        binding.project_id = find_string(value, "project_id", 0).map(str::to_owned);
        binding.candidate_id = find_string(value, "candidate_id", 0).map(str::to_owned);
    }
    Ok(())
}

fn find_string<'a>(value: &'a Value, key: &str, depth: usize) -> Option<&'a str> {
    if depth > 4 {
        return None;
    }
    let object = value.as_object()?;
    if let Some(found) = object.get(key).and_then(Value::as_str) {
        return Some(found);
    }
    object
        .values()
        .filter(|child| child.is_object())
        .find_map(|child| find_string(child, key, depth + 1))
}

fn valid_sha256(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

fn valid_mechanical_animation_glb_id(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value.as_bytes().first().is_some_and(|byte| {
                matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9')
            })
            && value
                .bytes()
                .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b':' | b'-'))
    })
}

fn valid_v2_id(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .as_bytes()
                .first()
                .is_some_and(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'))
            && value.bytes().all(|byte| {
                matches!(
                    byte,
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'
                )
            })
    })
}

fn contains_raw_media_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(
                key.as_str(),
                "png_base64"
                    | "base64"
                    | "glb_base64"
                    | "raw_base64"
                    | "png_bytes"
                    | "raw_png_bytes"
                    | "aov_base64"
                    | "raw_aov_base64"
                    | "glb"
                    | "raw_glb"
                    | "glb_data"
                    | "glb_bytes"
                    | "raw_glb_bytes"
                    | "aov_bytes"
                    | "raw_aov_bytes"
                    | "aov_data"
                    | "image_bytes"
                    | "raw_image_bytes"
            ) || contains_raw_media_field(child)
        }),
        Value::Array(items) => items.iter().any(contains_raw_media_field),
        _ => false,
    }
}

fn contains_forbidden_transport_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(
                key.as_str(),
                "path"
                    | "file_path"
                    | "absolute_path"
                    | "url"
                    | "uri"
                    | "script"
                    | "script_source"
                    | "python"
                    | "python_source"
                    | "javascript"
                    | "javascript_source"
                    | "shell"
                    | "shell_command"
                    | "command"
                    | "secret"
                    | "secret_key"
                    | "api_key"
                    | "access_token"
                    | "authorization"
                    | "password"
                    | "credential"
                    | "credentials"
                    | "executable"
                    | "png_base64"
                    | "base64"
                    | "glb_base64"
                    | "raw_base64"
                    | "aov_base64"
                    | "raw_aov_base64"
                    | "raw_png_bytes"
                    | "glb"
                    | "raw_glb"
                    | "glb_data"
                    | "glb_bytes"
                    | "raw_glb_bytes"
                    | "aov_bytes"
                    | "raw_aov_bytes"
                    | "aov_data"
                    | "image_bytes"
                    | "raw_image_bytes"
            ) || contains_forbidden_transport_field(child)
        }),
        Value::Array(items) => items.iter().any(contains_forbidden_transport_field),
        _ => false,
    }
}

fn contains_retopology_payload_field(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(
                key.as_str(),
                "values"
                    | "offset_field"
                    | "offsets"
                    | "vertices"
                    | "indices"
                    | "vertex_positions"
                    | "triangle_indices"
                    | "part_bindings"
                    | "faces"
                    | "mesh"
                    | "mapping"
                    | "mesh_vertices"
                    | "mesh_indices"
            ) || contains_retopology_payload_field(child)
        }),
        Value::Array(items) => items.iter().any(contains_retopology_payload_field),
        _ => false,
    }
}

fn validate_production_weapon_retopology_cage_source_response(
    tool: AgenticTool,
    value: &Value,
    binding: &Binding,
) -> Result<(), String> {
    const RESULT_FIELDS: [&str; 14] = [
        "schema_version",
        "bundle_key_sha256",
        "bundle",
        "replayed",
        "restart_hash_verified",
        "runtime_write",
        "quality_status",
        "visual_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ];
    const BUNDLE_FIELDS: [&str; 30] = [
        "schema_version",
        "bundle_key_sha256",
        "project_id",
        "source_candidate_id",
        "source_candidate_state_sha256",
        "source_high_artifact_sha256",
        "source_high_artifact_readback_object_sha256",
        "low_artifact_sha256",
        "low_artifact_readback_object_sha256",
        "cage_artifact_sha256",
        "cage_artifact_readback_object_sha256",
        "low_mesh_object_sha256",
        "correspondence_object_sha256",
        "cage_offset_field_object_sha256",
        "receipt_object_sha256",
        "low_retopology_policy",
        "cage_policy",
        "source_status",
        "quality_status",
        "visual_quality_status",
        "human_review_status",
        "commercial_engine_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "request_sha256",
        "canonical_sha256",
        "created_at",
    ];
    let is_prepare = tool == AgenticTool::ProductionWeaponRetopologyCageSourcePrepare;
    let object = value
        .as_object()
        .expect("response object checked by caller");
    let bundle = value
        .get("bundle")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "AGENTIC_RUNTIME_OUTPUT_INVALID: retopology/Cage response is missing bundle".to_owned()
        })?;
    let hash_fields = [
        "bundle_key_sha256",
        "source_candidate_state_sha256",
        "source_high_artifact_sha256",
        "source_high_artifact_readback_object_sha256",
        "low_artifact_sha256",
        "low_artifact_readback_object_sha256",
        "cage_artifact_sha256",
        "cage_artifact_readback_object_sha256",
        "low_mesh_object_sha256",
        "correspondence_object_sha256",
        "cage_offset_field_object_sha256",
        "receipt_object_sha256",
        "request_sha256",
        "canonical_sha256",
    ];
    let hashes_are_valid = hash_fields
        .iter()
        .all(|field| valid_sha256(bundle.get(*field).and_then(Value::as_str)));
    let ids_are_valid = valid_v2_id(bundle.get("project_id").and_then(Value::as_str))
        && valid_v2_id(bundle.get("source_candidate_id").and_then(Value::as_str));
    let normalized_key = {
        let mut normalized = Value::Object(bundle.clone());
        let normalized_object = normalized.as_object_mut().expect("bundle object");
        for field in [
            "bundle_key_sha256",
            "receipt_object_sha256",
            "canonical_sha256",
            "created_at",
        ] {
            normalized_object.insert(field.to_owned(), Value::String(String::new()));
        }
        forgecad_runtime::canonical_json_hash(&normalized)
    };
    let canonical_binding_is_valid = bundle.get("bundle_key_sha256").and_then(Value::as_str)
        == Some(normalized_key.as_str())
        && bundle.get("canonical_sha256").and_then(Value::as_str) == Some(normalized_key.as_str());
    let fixed_bundle = bundle.get("schema_version").and_then(Value::as_str)
        == Some("ProductionWeaponRetopologyCageSourceBundle@1")
        && bundle.get("low_retopology_policy").and_then(Value::as_str)
            == Some("bounded-low-retopology-topology-correspondent-cage-source-only@1")
        && bundle.get("cage_policy").and_then(Value::as_str)
            == Some("bounded-low-retopology-topology-correspondent-cage-source-only@1")
        && bundle.get("source_status").and_then(Value::as_str)
            == Some("runtime-owned-durable-production-weapon-retopology-cage-source-bundle")
        && bundle.get("quality_status").and_then(Value::as_str) == Some("structural_only")
        && bundle.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && bundle.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
        && bundle
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            == Some("NOT_RUN")
        && bundle.get("runtime_write_performed") == Some(&Value::Bool(true))
        && bundle.get("production_stage_advanced") == Some(&Value::Bool(false))
        && bundle.get("candidate_confirmed") == Some(&Value::Bool(false))
        && bundle.get("version_created") == Some(&Value::Bool(false))
        && bundle.get("export_performed") == Some(&Value::Bool(false))
        && bundle
            .get("created_at")
            .and_then(Value::as_str)
            .is_some_and(|created_at| !created_at.is_empty() && created_at.len() <= 128);
    let top_flags_are_safe = value.get("restart_hash_verified") == Some(&Value::Bool(true))
        && value.get("runtime_write") == Some(&Value::Bool(is_prepare))
        && value.get("quality_status").and_then(Value::as_str) == Some("structural_only")
        && value.get("visual_quality_status").and_then(Value::as_str) == Some("NOT_PROVEN")
        && value.get("human_review_status").and_then(Value::as_str) == Some("NOT_RUN")
        && value
            .get("commercial_engine_status")
            .and_then(Value::as_str)
            == Some("NOT_RUN")
        && value.get("production_stage_advanced") == Some(&Value::Bool(false))
        && value.get("candidate_confirmed") == Some(&Value::Bool(false))
        && value.get("version_created") == Some(&Value::Bool(false))
        && value.get("export_performed") == Some(&Value::Bool(false))
        && value.get("replayed").and_then(Value::as_bool).is_some();
    if !object_has_exact_fields(object, &RESULT_FIELDS)
        || !object_has_exact_fields(bundle, &BUNDLE_FIELDS)
        || value.get("schema_version").and_then(Value::as_str)
            != Some(if is_prepare {
                "ProductionWeaponRetopologyCageSourceBundlePrepareResult@1"
            } else {
                "ProductionWeaponRetopologyCageSourceBundleGetResult@1"
            })
        || value.get("bundle_key_sha256") != bundle.get("bundle_key_sha256")
        || !hashes_are_valid
        || !ids_are_valid
        || !canonical_binding_is_valid
        || !fixed_bundle
        || !top_flags_are_safe
        || contains_raw_media_field(value)
        || contains_forbidden_transport_field(value)
        || contains_retopology_payload_field(value)
    {
        return Err(
            "AGENTIC_RUNTIME_OUTPUT_INVALID: retopology/Cage source bundle schema, hash binding, structural-only flags or raw mesh boundary differs"
                .to_owned(),
        );
    }
    if binding.is_bound()
        && (binding.project_id.as_deref() != bundle.get("project_id").and_then(Value::as_str)
            || binding.candidate_id.as_deref()
                != bundle.get("source_candidate_id").and_then(Value::as_str))
    {
        return Err(
            "AGENTIC_SCOPE_MISMATCH: retopology/Cage source response crossed the bound project/source candidate"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "agentic_write_tools_tests.rs"]
mod tests;
