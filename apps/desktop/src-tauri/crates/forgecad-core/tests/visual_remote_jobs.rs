use std::collections::BTreeSet;

use forgecad_core::{
    ConceptImageBackend, ConceptImageGenerationRequest, ConceptImageResumeBinding,
    ConceptReferenceArtifact, CoreRepository, HiddenSurfacePolicy, Neural3DBackend,
    Neural3DGenerationRequest, Neural3DResumeBinding, NeuralVisualGlbInspection, PbrChannel,
    Project, ProjectStatus, VisualDesignBrief, VisualInputKind, VisualQualityTier,
    VisualRemoteJobRecord, VisualRemoteJobState, CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
    CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION, NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION,
    VISUAL_DESIGN_BRIEF_SCHEMA_VERSION, VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION,
};
use tempfile::tempdir;

fn sha(value: char) -> String {
    std::iter::repeat_n(value, 64).collect()
}

fn brief() -> VisualDesignBrief {
    VisualDesignBrief {
        schema_version: VISUAL_DESIGN_BRIEF_SCHEMA_VERSION.into(),
        brief_id: "visual_brief_restart".into(),
        project_id: "project_visual_restart".into(),
        turn_id: "turn_visual_restart".into(),
        input_kind: VisualInputKind::Text,
        user_intent_sha256: sha('a'),
        object_class: "fictional mechanical collectible".into(),
        visual_summary: "A layered hard-surface collectible with refined PBR materials.".into(),
        style_terms: vec!["industrial_futurism".into()],
        material_terms: vec!["brushed_titanium".into()],
        input_evidence: vec![],
    }
}

fn concept_binding() -> ConceptImageResumeBinding {
    let brief = brief();
    let request = ConceptImageGenerationRequest {
        schema_version: CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION.into(),
        request_id: "concept_request_restart".into(),
        project_id: brief.project_id.clone(),
        turn_id: brief.turn_id.clone(),
        brief_id: brief.brief_id.clone(),
        prompt:
            "One complete isolated fictional mechanical collectible on a clean neutral background."
                .into(),
        input_image_object_sha256: None,
        input_image_media_type: None,
        backend_preferences: vec![ConceptImageBackend::FalFlux2],
        width: 1024,
        height: 1024,
        output_media_type: "image/png".into(),
        isolated_subject: true,
        clean_background: true,
        image_count: 1,
        idempotency_key: "concept_idempotency_restart".into(),
    };
    ConceptImageResumeBinding::from_submitted_request(
        brief,
        &request,
        ConceptImageBackend::FalFlux2,
        "fal_concept_job_restart".into(),
        "concept_reference_restart".into(),
        VisualQualityTier::StandardAsset,
    )
    .unwrap()
}

fn record(state: VisualRemoteJobState, updated_at: &str) -> VisualRemoteJobRecord {
    VisualRemoteJobRecord {
        schema_version: VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION.into(),
        client_request_id: "visual_generation_restart".into(),
        project_id: "project_visual_restart".into(),
        turn_id: "turn_visual_restart".into(),
        state,
        created_at: "2026-07-26T10:00:00Z".into(),
        updated_at: updated_at.into(),
    }
}

fn project() -> Project {
    Project {
        project_id: "project_visual_restart".into(),
        profile_id: "profile_weapon_concept_v1".into(),
        domain_type: "weapon_concept".into(),
        name: "Visual recovery".into(),
        status: ProjectStatus::Active,
        current_version_id: None,
        created_at: "2026-07-26T09:59:00Z".into(),
        updated_at: "2026-07-26T09:59:00Z".into(),
    }
}

#[test]
fn prompt_free_concept_receipt_recovers_and_advances_once() {
    let root = tempdir().unwrap();
    let db = root.path().join("library.db");
    let repository = CoreRepository::open(&db, root.path(), "visual_remote_first").unwrap();
    repository
        .ensure_default_domain_profile("2026-07-26T09:58:00Z")
        .unwrap();
    repository.create_project(&project()).unwrap();

    let concept = concept_binding();
    let concept_record = record(
        VisualRemoteJobState::ConceptSubmitted {
            binding: concept.clone(),
        },
        "2026-07-26T10:00:01Z",
    );
    repository.put_visual_remote_job(&concept_record).unwrap();
    let persisted_json = serde_json::to_string(
        &repository
            .visual_remote_job("visual_generation_restart")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(!persisted_json.contains("One complete isolated"));
    assert!(persisted_json.contains(&concept.prompt_sha256));
    repository.publish().unwrap();
    drop(repository);

    let restarted = CoreRepository::open(&db, root.path(), "visual_remote_restart").unwrap();
    restarted.publish().unwrap();
    assert_eq!(
        restarted
            .recoverable_visual_remote_jobs(Some("project_visual_restart"))
            .unwrap(),
        vec![concept_record.clone()]
    );

    let concept_reference = ConceptReferenceArtifact {
        schema_version: CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION.into(),
        reference_id: concept.reference_id.clone(),
        brief_id: concept.brief.brief_id.clone(),
        image_object_sha256: sha('b'),
        media_type: "image/png".into(),
        provider_id: "fal_flux_2".into(),
        provider_job_id: concept.provider_job_id.clone(),
        isolated_subject: true,
        clean_background: true,
        hidden_surface_policy: HiddenSurfacePolicy::AiInferred,
    };
    let neural_request = Neural3DGenerationRequest {
        schema_version: NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION.into(),
        request_id: "neural_request_restart".into(),
        project_id: concept.brief.project_id.clone(),
        turn_id: concept.brief.turn_id.clone(),
        brief_id: concept.brief.brief_id.clone(),
        concept_reference_id: concept_reference.reference_id.clone(),
        concept_reference_sha256: concept_reference.image_object_sha256.clone(),
        additional_views: vec![],
        quality_tier: VisualQualityTier::StandardAsset,
        backend_preferences: vec![Neural3DBackend::Hunyuan3dV31Pro],
        idempotency_key: "neural_idempotency_restart".into(),
    };
    let neural_binding = Neural3DResumeBinding {
        brief: concept.brief,
        concept_reference,
        request: neural_request,
        backend: Neural3DBackend::Hunyuan3dV31Pro,
        provider_job_id: "fal_neural_job_restart".into(),
    };
    let neural_record = record(
        VisualRemoteJobState::NeuralSubmitted {
            binding: neural_binding.clone(),
        },
        "2026-07-26T10:05:00Z",
    );
    restarted.put_visual_remote_job(&neural_record).unwrap();

    let terminal = record(
        VisualRemoteJobState::Completed {
            binding: neural_binding,
            inspection: NeuralVisualGlbInspection {
                sha256: sha('c'),
                byte_size: 4096,
                triangle_count: 1000,
                mesh_count: 1,
                primitive_count: 1,
                material_count: 1,
                node_count: 1,
                pbr_channels: BTreeSet::from([
                    PbrChannel::BaseColor,
                    PbrChannel::Normal,
                    PbrChannel::Roughness,
                    PbrChannel::Metallic,
                ]),
                every_primitive_has_uv0: true,
                every_primitive_has_tangent: true,
            },
        },
        "2026-07-26T10:10:00Z",
    );
    restarted.put_visual_remote_job(&terminal).unwrap();
    assert!(restarted
        .recoverable_visual_remote_jobs(None)
        .unwrap()
        .is_empty());
    assert_eq!(
        restarted
            .put_visual_remote_job(&concept_record)
            .unwrap_err()
            .code(),
        "VISUAL_REMOTE_JOB_TRANSITION_INVALID"
    );
}

#[test]
fn scope_drift_and_neural_lineage_drift_fail_closed() {
    let concept = concept_binding();
    let mut scoped = record(
        VisualRemoteJobState::ConceptSubmitted {
            binding: concept.clone(),
        },
        "2026-07-26T10:00:01Z",
    );
    scoped.project_id = "project_other".into();
    assert_eq!(
        scoped.validate().unwrap_err().code(),
        "VISUAL_REMOTE_JOB_SCOPE_MISMATCH"
    );

    let reference = ConceptReferenceArtifact {
        schema_version: CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION.into(),
        reference_id: concept.reference_id,
        brief_id: concept.brief.brief_id.clone(),
        image_object_sha256: sha('b'),
        media_type: "image/png".into(),
        provider_id: "fal_flux_2".into(),
        provider_job_id: concept.provider_job_id,
        isolated_subject: true,
        clean_background: true,
        hidden_surface_policy: HiddenSurfacePolicy::AiInferred,
    };
    let binding = Neural3DResumeBinding {
        brief: concept.brief.clone(),
        concept_reference: reference,
        request: Neural3DGenerationRequest {
            schema_version: NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "neural_request_bad".into(),
            project_id: concept.brief.project_id,
            turn_id: concept.brief.turn_id,
            brief_id: concept.brief.brief_id,
            concept_reference_id: "different_reference".into(),
            concept_reference_sha256: sha('b'),
            additional_views: vec![],
            quality_tier: VisualQualityTier::StandardAsset,
            backend_preferences: vec![Neural3DBackend::Hunyuan3dV31Pro],
            idempotency_key: "neural_idempotency_bad".into(),
        },
        backend: Neural3DBackend::Hunyuan3dV31Pro,
        provider_job_id: "fal_neural_job_bad".into(),
    };
    assert_eq!(
        binding.validate().unwrap_err().code(),
        "VISUAL_REMOTE_NEURAL_LINEAGE_INVALID"
    );
}
