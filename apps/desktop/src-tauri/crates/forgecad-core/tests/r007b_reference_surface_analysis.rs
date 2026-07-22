use forgecad_core::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2,
    validate_reference_surface_analysis_for_plan, ComponentRecipeRef, ImportedGlbInspection,
    RecipeRegistry, RecipeValidator, ReferenceClass, ReferenceEvidence, ReferenceEvidenceKind,
    ReferenceEvidenceObservations, ReferenceGuidedRebuildPlan, ReferenceGuidedRebuildPlanStatus,
    ReferenceSurfaceAnalysis, ReferenceSurfaceBinding, ReferenceSurfaceFidelityCeiling,
    ReferenceSurfaceGlbReadbackFacts, ReferenceSurfaceIntentionalChange,
    ReferenceSurfaceObservationKind, ReferenceSurfaceUnresolved, VisiblePartHypothesis,
    REFERENCE_EVIDENCE_SCHEMA_VERSION, REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION,
    REFERENCE_SURFACE_ANALYSIS_SCHEMA_VERSION,
};

const ROOT: &str = "recipe_c106_arm_desktop_assistant";
const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn recipe_ref(registry: &RecipeRegistry, recipe_id: &str) -> ComponentRecipeRef {
    let recipe = registry.recipe(recipe_id).unwrap();
    ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: recipe_id.into(),
        version: recipe.version,
        recipe_sha256: RecipeValidator::recipe_sha256(recipe).unwrap(),
    }
}

fn image_evidence() -> ReferenceEvidence {
    ReferenceEvidence {
        schema_version: REFERENCE_EVIDENCE_SCHEMA_VERSION.into(),
        evidence_id: "refevid_r007b_image".into(),
        project_id: "prj_r007b".into(),
        kind: ReferenceEvidenceKind::Image,
        reference_class: ReferenceClass::SingleImage,
        domain_pack_id: "pack_robotic_arm_concept".into(),
        source_file_name: "arm.png".into(),
        source_media_type: "image/png".into(),
        source_object_sha256: SHA.into(),
        source_imported_asset_version_id: None,
        source_statement: "user authorized visual reference".into(),
        license_statement: "user declared authorized".into(),
        missing_views: vec!["rear".into(), "right".into()],
        user_notes: "visible robotic arm".into(),
        observations: ReferenceEvidenceObservations {
            silhouette_summary: "Visible articulated arm silhouette.".into(),
            proportion_ranges: vec!["Visible upper link is longer than the base diameter.".into()],
            material_zone_observations: vec![
                "Visible shell is dark with a blue trim accent.".into()
            ],
            visible_part_hypotheses: vec![VisiblePartHypothesis {
                role: "upper_link_form".into(),
                confidence: "medium".into(),
                visible_basis: "Visible side-view arm link.".into(),
            }],
            uncertainties: vec!["Rear and internal structure remain unknown.".into()],
            image_surface_facts: Some(forgecad_core::ReferenceImageSurfaceFacts {
                width: 256,
                height: 192,
                aspect_ratio_milli: 1333,
                dominant_color_buckets: vec![
                    forgecad_core::ReferenceImageColorBucket::Black,
                    forgecad_core::ReferenceImageColorBucket::Blue,
                ],
                brightness: forgecad_core::ReferenceImageBrightnessBucket::Dark,
                edge_density: forgecad_core::ReferenceImageEdgeDensityBucket::Medium,
                foreground_bbox_normalized: [120, 80, 870, 910],
                contact_sheet_layout_evidence: false,
                foreground_confidence: forgecad_core::ReferenceImageForegroundConfidence::Medium,
            }),
        },
        created_at: "2026-07-18T00:00:00Z".into(),
        glb_inspection: None,
    }
}

fn glb_evidence() -> ReferenceEvidence {
    let inspection = ImportedGlbInspection {
        sha256: SHA.into(),
        byte_size: 2048,
        triangle_count: 1200,
        bounds_mm: [120.0, 80.0, 60.0],
        mesh_count: 4,
        primitive_count: 5,
        material_count: 3,
        node_count: 4,
    };
    ReferenceEvidence {
        schema_version: REFERENCE_EVIDENCE_SCHEMA_VERSION.into(),
        evidence_id: "refevid_r007b_glb".into(),
        project_id: "prj_r007b".into(),
        kind: ReferenceEvidenceKind::Glb,
        reference_class: ReferenceClass::GlbReadback,
        domain_pack_id: "pack_robotic_arm_concept".into(),
        source_file_name: "arm.glb".into(),
        source_media_type: "model/gltf-binary".into(),
        source_object_sha256: SHA.into(),
        source_imported_asset_version_id: None,
        source_statement: "user authorized read-only GLB reference".into(),
        license_statement: "user declared authorized".into(),
        missing_views: vec![],
        user_notes: "".into(),
        observations: ReferenceEvidenceObservations {
            silhouette_summary: "基于用户授权的只读 GLB 外观证据：4 个网格、5 个 primitive；仅用于可见轮廓与比例，不恢复隐藏结构。".into(),
            proportion_ranges: vec!["已读取包围范围 120.0 × 80.0 × 60.0 mm；仅作为相对比例区间，不是制造尺寸。".into()],
            material_zone_observations: vec!["GLB 读取到 3 个可见材质槽；材质名称和物理属性不作为事实恢复。".into()],
            visible_part_hypotheses: vec![],
            uncertainties: vec!["导入 GLB 保持只读；其内部层级、连接关系、材料配方和功能均未被推断。".into()],
            image_surface_facts: None,
        },
        created_at: "2026-07-18T00:00:00Z".into(),
        glb_inspection: Some(inspection),
    }
}

fn plan(registry: &RecipeRegistry, evidence_id: &str) -> ReferenceGuidedRebuildPlan {
    ReferenceGuidedRebuildPlan {
        schema_version: REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION.into(),
        rebuild_plan_id: "rebuildplan_r007b_arm".into(),
        project_id: "prj_r007b".into(),
        evidence_id: evidence_id.into(),
        base_asset_version_id: None,
        domain_pack_id: "pack_robotic_arm_concept".into(),
        recipe_id: ROOT.into(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        rebuild_summary: "Rebuild visible arm appearance with reviewed C106 components.".into(),
        intended_differences: vec!["Use a non-functional reviewed recipe interpretation.".into()],
        retained_evidence: vec!["Retain visible silhouette and relative proportions.".into()],
        unresolved_uncertainties: vec!["Hidden structure remains unknown.".into()],
        status: ReferenceGuidedRebuildPlanStatus::Draft,
        preview_change_set_id: None,
        confirmed_asset_version_id: None,
        created_at: "2026-07-18T00:00:00Z".into(),
        updated_at: "2026-07-18T00:00:00Z".into(),
    }
}

fn binding(
    registry: &RecipeRegistry,
    id: &str,
    kind: ReferenceSurfaceObservationKind,
    slot: Option<&str>,
    recipe_id: &str,
    role: &str,
    zone: &str,
    surface_slot: &str,
) -> ReferenceSurfaceBinding {
    ReferenceSurfaceBinding {
        binding_id: format!("refsrfbind_{id}"),
        observation_kind: kind,
        observation_index: 0,
        target_part_slot_id: slot.map(str::to_string),
        target_recipe: recipe_ref(registry, recipe_id),
        target_part_role: role.into(),
        target_material_zone_id: zone.into(),
        target_surface_slot_id: surface_slot.into(),
    }
}

fn analysis(registry: &RecipeRegistry, evidence: &ReferenceEvidence) -> ReferenceSurfaceAnalysis {
    let skill = builtin_surface_adornment_manifest_v2();
    let skill_sha256 = skill.canonical_sha256().unwrap();
    ReferenceSurfaceAnalysis {
        schema_version: REFERENCE_SURFACE_ANALYSIS_SCHEMA_VERSION.into(),
        analysis_id: "refsrfanalysis_r007b_arm".into(),
        rebuild_plan_id: "rebuildplan_r007b_arm".into(),
        evidence_id: evidence.evidence_id.clone(),
        source_object_sha256: evidence.source_object_sha256.clone(),
        domain_pack_id: "pack_robotic_arm_concept".into(),
        target_root_recipe: recipe_ref(registry, ROOT),
        c106_registry_sha256: registry.registry_sha256().into(),
        surface_skill_id: skill.skill_id,
        surface_skill_version: skill.version,
        surface_skill_sha256: skill_sha256,
        fidelity_ceiling: ReferenceSurfaceFidelityCeiling::SingleImageVisibleSurfaceOnly,
        bindings: vec![
            binding(
                registry,
                "silhouette",
                ReferenceSurfaceObservationKind::Silhouette,
                None,
                ROOT,
                "base_form",
                "zone_arm_base",
                "adornslot_desktop_base",
            ),
            binding(
                registry,
                "proportion",
                ReferenceSurfaceObservationKind::Proportion,
                Some("slot_desktop_lower_link"),
                "recipe_c106_arm_link_armor",
                "link_armor",
                "zone_arm_link_shell",
                "adornslot_link_shell",
            ),
            binding(
                registry,
                "material",
                ReferenceSurfaceObservationKind::MaterialZone,
                Some("slot_desktop_trim"),
                "recipe_c106_arm_surface_trim",
                "surface_trim",
                "zone_arm_surface_trim",
                "adornslot_surface_trim",
            ),
            binding(
                registry,
                "visible",
                ReferenceSurfaceObservationKind::VisiblePart,
                Some("slot_desktop_upper_link"),
                "recipe_c106_arm_link_armor",
                "link_armor",
                "zone_arm_link_shell",
                "adornslot_link_shell",
            ),
        ],
        retained_observation_kinds: vec![
            ReferenceSurfaceObservationKind::Silhouette,
            ReferenceSurfaceObservationKind::Proportion,
            ReferenceSurfaceObservationKind::VisiblePart,
            ReferenceSurfaceObservationKind::MaterialZone,
        ],
        intentionally_changed: vec![
            ReferenceSurfaceIntentionalChange::NonFunctionalRecipeInterpretation,
            ReferenceSurfaceIntentionalChange::MaterialPresetNormalization,
        ],
        unresolved: vec![
            ReferenceSurfaceUnresolved::MissingViews,
            ReferenceSurfaceUnresolved::HiddenStructure,
            ReferenceSurfaceUnresolved::ExactDimensions,
            ReferenceSurfaceUnresolved::MaterialPhysics,
            ReferenceSurfaceUnresolved::FunctionalBehavior,
        ],
        glb_readback_facts: None,
        created_at: "2026-07-18T00:00:00Z".into(),
    }
}

#[test]
fn r007b_image_analysis_maps_visible_evidence_to_exact_c106_recipe_zone_and_a005_v2_slot() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let evidence = image_evidence();
    let plan = plan(&registry, &evidence.evidence_id);
    let analysis = analysis(&registry, &evidence);
    validate_reference_surface_analysis_for_plan(&analysis, &evidence, &plan).unwrap();
}

#[test]
fn r007b_glb_analysis_binds_only_exact_readback_facts_and_denies_visible_part_projection() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let evidence = glb_evidence();
    let plan = plan(&registry, &evidence.evidence_id);
    let mut analysis = analysis(&registry, &evidence);
    analysis.fidelity_ceiling = ReferenceSurfaceFidelityCeiling::StrictGlbReadbackVisibleBoundsOnly;
    analysis
        .bindings
        .retain(|binding| binding.observation_kind != ReferenceSurfaceObservationKind::VisiblePart);
    analysis
        .retained_observation_kinds
        .retain(|kind| *kind != ReferenceSurfaceObservationKind::VisiblePart);
    analysis
        .unresolved
        .retain(|kind| *kind != ReferenceSurfaceUnresolved::MissingViews);
    analysis.glb_readback_facts = Some(ReferenceSurfaceGlbReadbackFacts::from_inspection(
        evidence.glb_inspection.as_ref().unwrap(),
    ));
    validate_reference_surface_analysis_for_plan(&analysis, &evidence, &plan).unwrap();

    analysis.bindings.push(binding(
        &registry,
        "forbidden_visible",
        ReferenceSurfaceObservationKind::VisiblePart,
        Some("slot_desktop_upper_link"),
        "recipe_c106_arm_link_armor",
        "link_armor",
        "zone_arm_link_shell",
        "adornslot_link_shell",
    ));
    analysis
        .retained_observation_kinds
        .push(ReferenceSurfaceObservationKind::VisiblePart);
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&analysis, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_OBSERVATION_INVALID"
    );
}

#[test]
fn r007b_contact_sheet_analysis_selects_a_distinct_c106_root_with_a_multi_view_ceiling() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let mut evidence = image_evidence();
    evidence.evidence_id = "refevid_r007b_contact".into();
    evidence.source_file_name = "arm_contact_sheet.png".into();
    evidence.missing_views.clear();
    evidence.observations.silhouette_summary =
        "Visible multi-view articulated arm silhouette.".into();
    let mut plan = plan(&registry, &evidence.evidence_id);
    plan.recipe_id = "recipe_c106_arm_gallery_industrial".into();
    let mut analysis = analysis(&registry, &evidence);
    analysis.target_root_recipe = recipe_ref(&registry, "recipe_c106_arm_gallery_industrial");
    analysis.fidelity_ceiling = ReferenceSurfaceFidelityCeiling::MultiViewImageVisibleSurfaceOnly;
    analysis
        .unresolved
        .retain(|kind| *kind != ReferenceSurfaceUnresolved::MissingViews);
    analysis.bindings[0].target_recipe =
        recipe_ref(&registry, "recipe_c106_arm_gallery_industrial");
    analysis.bindings[0].target_surface_slot_id = "adornslot_gallery_base".into();
    analysis.bindings[1].target_part_slot_id = Some("slot_gallery_lower_link".into());
    analysis.bindings[2].target_part_slot_id = Some("slot_gallery_trim".into());
    analysis.bindings[3].target_part_slot_id = Some("slot_gallery_upper_link".into());

    validate_reference_surface_analysis_for_plan(&analysis, &evidence, &plan).unwrap();
    assert_ne!(analysis.target_root_recipe.recipe_id, ROOT);
}

#[test]
fn r007b_rejects_tamper_cross_domain_wrong_recipe_hidden_structure_and_legacy_a005_v1() {
    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
    let evidence = image_evidence();
    let plan = plan(&registry, &evidence.evidence_id);
    let baseline = analysis(&registry, &evidence);

    let mut source_hash_tamper = baseline.clone();
    source_hash_tamper.source_object_sha256 =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&source_hash_tamper, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_CONTEXT_MISMATCH"
    );

    let mut wrong_zone = baseline.clone();
    wrong_zone.bindings[1].target_material_zone_id = "zone_arm_joint_shell".into();
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&wrong_zone, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_TARGET_INVALID"
    );

    let mut cross_domain = baseline.clone();
    cross_domain.domain_pack_id = "pack_vehicle_concept".into();
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&cross_domain, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_CONTEXT_MISMATCH"
    );

    let mut wrong_recipe = baseline.clone();
    wrong_recipe.bindings[1].target_recipe = recipe_ref(&registry, "recipe_c106_arm_joint_housing");
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&wrong_recipe, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_TARGET_INVALID"
    );

    let mut hidden_structure = baseline.clone();
    hidden_structure
        .unresolved
        .retain(|kind| *kind != ReferenceSurfaceUnresolved::HiddenStructure);
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&hidden_structure, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_IMAGE_CEILING_INVALID"
    );

    let legacy = builtin_surface_adornment_manifest();
    let mut legacy_v1 = baseline;
    legacy_v1.surface_skill_version = legacy.version;
    legacy_v1.surface_skill_sha256 = legacy.canonical_sha256().unwrap();
    assert_eq!(
        validate_reference_surface_analysis_for_plan(&legacy_v1, &evidence, &plan)
            .unwrap_err()
            .code(),
        "REFERENCE_SURFACE_A005_V2_REQUIRED"
    );
}
