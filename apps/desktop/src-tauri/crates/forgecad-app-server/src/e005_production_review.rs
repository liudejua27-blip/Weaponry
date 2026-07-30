//! FGC-E005-R3 same-source production review.
//!
//! The R2 visual decision is made against a bounded interactive candidate.
//! R3 recompiles the exact final author source once with the code-owned
//! `production_concept` profile and Rust-derived A005/PBR programs. No
//! Provider is called here and no second asset truth is introduced.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Instant,
};

use forgecad_core::{
    builtin_surface_adornment_manifest_v3, compiled_visual_base_material_id,
    lower_forge_visual_author_source_v1, normalized_geometry_sha256, semantic_sha256,
    AuthorSurfaceProfileV1, ForgeVisualAuthorLoweringV1, ForgeVisualAuthorSourceV1,
    SurfaceAdornmentProgram,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    CancellationToken, RestrictedGeometryError, RestrictedGeometryInput, RestrictedGeometryOutput,
    RestrictedGeometryPort, RestrictedQualityProfile, RestrictedRenderViewProfile,
    RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION, RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION,
};

pub const E005_PRODUCTION_REVIEW_SCHEMA_VERSION: &str = "E005ProductionReview@1";
const MAX_E005_SURFACE_ADORNMENTS: usize = 32;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct E005ProductionReviewV1 {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub surface_plan_sha256: String,
    pub surface_adornment_sha256: String,
    pub restricted_geometry_input_sha256: String,
    pub surface_adornment_count: u16,
    pub glb_sha256: String,
    pub normalized_geometry_sha256: String,
    pub fixed_view_sha256: String,
    pub fixed_views: BTreeMap<String, String>,
    pub compile_readback_sha256: String,
    pub restricted_geometry_evidence_sha256: String,
    pub artifact_profile_id: String,
    pub material_zone_count: u32,
    pub visual_texture_set_count: u32,
    pub visual_texture_map_count: u32,
    pub visual_texture_provenance_verified: bool,
    pub lower_duration_ms: u64,
    pub compile_duration_ms: u64,
    pub render_duration_ms: u64,
    pub elapsed_ms: u64,
}

impl E005ProductionReviewV1 {
    pub fn validate(&self) -> Result<(), forgecad_core::CoreError> {
        let valid_hash = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        let required_views = BTreeSet::from([
            "turntable_000",
            "turntable_045",
            "turntable_090",
            "turntable_135",
            "turntable_180",
            "turntable_225",
            "turntable_270",
            "turntable_315",
        ]);
        if self.schema_version != E005_PRODUCTION_REVIEW_SCHEMA_VERSION
            || [
                self.source_program_sha256.as_str(),
                self.surface_plan_sha256.as_str(),
                self.surface_adornment_sha256.as_str(),
                self.restricted_geometry_input_sha256.as_str(),
                self.glb_sha256.as_str(),
                self.normalized_geometry_sha256.as_str(),
                self.fixed_view_sha256.as_str(),
                self.compile_readback_sha256.as_str(),
                self.restricted_geometry_evidence_sha256.as_str(),
            ]
            .into_iter()
            .any(|hash| !valid_hash(hash))
            || !(1..=MAX_E005_SURFACE_ADORNMENTS as u16).contains(&self.surface_adornment_count)
            || self.artifact_profile_id != "production_concept"
            || self
                .fixed_views
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != required_views
            || self.fixed_views.values().any(|hash| !valid_hash(hash))
            || semantic_sha256(&self.fixed_views)? != self.fixed_view_sha256
            || self.material_zone_count < u32::from(self.surface_adornment_count)
            || self.visual_texture_set_count != u32::from(self.surface_adornment_count)
            || self.visual_texture_map_count
                != u32::from(self.surface_adornment_count).saturating_mul(5)
            || !self.visual_texture_provenance_verified
            || self.compile_duration_ms > 240_000
            || self.render_duration_ms > 240_000
            || self.elapsed_ms < self.lower_duration_ms
        {
            return Err(forgecad_core::CoreError::invalid_data(
                "E005_R3_PRODUCTION_REVIEW_INVALID",
                "E005 production review is missing exact source, PBR, view, readback or wall-clock evidence.",
            ));
        }
        Ok(())
    }

    pub fn validate_against(
        &self,
        source: &Value,
        geometry: &RestrictedGeometryOutput,
    ) -> Result<(), forgecad_core::CoreError> {
        self.validate()?;
        let lowering = lower_forge_visual_author_source_v1(source)?;
        let adornments = compile_e005_surface_adornments(source, &lowering).map_err(|error| {
            forgecad_core::CoreError::invalid_data(
                "E005_R3_PRODUCTION_LINEAGE_INVALID",
                format!("{}: {}", error.code, error.message),
            )
        })?;
        let input = production_input(&lowering, adornments.clone());
        input.validate().map_err(|error| {
            forgecad_core::CoreError::invalid_data(
                "E005_R3_PRODUCTION_LINEAGE_INVALID",
                format!("{}: {}", error.code, error.message),
            )
        })?;
        geometry.validate(&input).map_err(|_| {
            forgecad_core::CoreError::invalid_data(
                "E005_R3_PRODUCTION_LINEAGE_INVALID",
                "Production geometry does not validate against the exact reconstructed input.",
            )
        })?;
        if self.source_program_sha256 != lowering.source_program_sha256
            || self.surface_plan_sha256 != lowering.surface_plan_sha256
            || self.surface_adornment_sha256 != semantic_sha256(&adornments)?
            || self.restricted_geometry_input_sha256 != semantic_sha256(&input)?
            || self.glb_sha256 != geometry.glb_sha256
            || self.normalized_geometry_sha256 != normalized_geometry_sha256(&geometry.glb_bytes)?
            || self.fixed_views != geometry.view_sha256
            || self.compile_readback_sha256 != geometry.readback.compile_readback_sha256
            || self.restricted_geometry_evidence_sha256
                != semantic_sha256(&geometry.execution_evidence)?
        {
            return Err(forgecad_core::CoreError::invalid_data(
                "E005_R3_PRODUCTION_LINEAGE_INVALID",
                "Production review does not bind the exact source, PBR input and geometry output.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct E005ProductionReviewResultV1 {
    pub review: E005ProductionReviewV1,
    pub geometry: RestrictedGeometryOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct E005ProductionReviewFailureV1 {
    pub code: String,
    pub message: String,
}

impl E005ProductionReviewFailureV1 {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct E005ProductionReviewCoordinatorV1 {
    geometry: Arc<dyn RestrictedGeometryPort>,
}

impl E005ProductionReviewCoordinatorV1 {
    pub fn new(geometry: Arc<dyn RestrictedGeometryPort>) -> Self {
        Self { geometry }
    }

    pub async fn execute(
        &self,
        source: &Value,
        cancellation: CancellationToken,
    ) -> Result<E005ProductionReviewResultV1, E005ProductionReviewFailureV1> {
        let started = Instant::now();
        if cancellation.is_cancelled() {
            return Err(E005ProductionReviewFailureV1::new(
                "E005_R3_CANCELLED",
                "production review was cancelled before lowering",
            ));
        }
        let lower_started = Instant::now();
        let lowering = lower_forge_visual_author_source_v1(source).map_err(core_failure)?;
        let adornments = compile_e005_surface_adornments(source, &lowering)?;
        let lower_duration_ms = bounded_elapsed_ms(lower_started);
        let input = production_input(&lowering, adornments.clone());
        input.validate().map_err(geometry_failure)?;
        let restricted_geometry_input_sha256 = semantic_sha256(&input).map_err(core_failure)?;
        let geometry = self
            .geometry
            .build_compile_render(input.clone(), cancellation)
            .await
            .map_err(geometry_failure)?;
        geometry.validate(&input).map_err(geometry_failure)?;
        verify_production_geometry(&geometry, adornments.len())?;
        let review = E005ProductionReviewV1 {
            schema_version: E005_PRODUCTION_REVIEW_SCHEMA_VERSION.into(),
            source_program_sha256: lowering.source_program_sha256,
            surface_plan_sha256: lowering.surface_plan_sha256,
            surface_adornment_sha256: semantic_sha256(&adornments).map_err(core_failure)?,
            restricted_geometry_input_sha256,
            surface_adornment_count: u16::try_from(adornments.len()).map_err(|_| {
                E005ProductionReviewFailureV1::new(
                    "E005_R3_SURFACE_BUDGET_INVALID",
                    "surface adornment count exceeds the bounded receipt field",
                )
            })?,
            glb_sha256: geometry.glb_sha256.clone(),
            normalized_geometry_sha256: normalized_geometry_sha256(&geometry.glb_bytes)
                .map_err(core_failure)?,
            fixed_view_sha256: semantic_sha256(&geometry.view_sha256).map_err(core_failure)?,
            fixed_views: geometry.view_sha256.clone(),
            compile_readback_sha256: geometry.readback.compile_readback_sha256.clone(),
            restricted_geometry_evidence_sha256: semantic_sha256(&geometry.execution_evidence)
                .map_err(core_failure)?,
            artifact_profile_id: geometry.readback.artifact_profile_id.clone(),
            material_zone_count: geometry.readback.material_zone_count,
            visual_texture_set_count: geometry.readback.visual_texture_set_count,
            visual_texture_map_count: geometry.readback.visual_texture_map_count,
            visual_texture_provenance_verified: geometry
                .readback
                .visual_texture_provenance_verified,
            lower_duration_ms,
            compile_duration_ms: geometry.execution_evidence.compile_duration_ms,
            render_duration_ms: geometry.execution_evidence.render_duration_ms,
            elapsed_ms: bounded_elapsed_ms(started),
        };
        review
            .validate_against(source, &geometry)
            .map_err(core_failure)?;
        Ok(E005ProductionReviewResultV1 { review, geometry })
    }
}

fn production_input(
    lowering: &ForgeVisualAuthorLoweringV1,
    surface_adornment_programs: Vec<SurfaceAdornmentProgram>,
) -> RestrictedGeometryInput {
    RestrictedGeometryInput {
        schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
        shape_program: lowering.shape_program.clone(),
        profile_sketch: None,
        section_set: None,
        surface_adornment_programs,
        surface_layer_input: None,
        surface_layer_inputs: Vec::new(),
        reference_uv_evidence_bakes: Vec::new(),
        render_view_profile: RestrictedRenderViewProfile::TurntableEight,
        quality_profile: RestrictedQualityProfile {
            profile_id: "production_concept".into(),
            runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
            max_triangle_count: 150_000,
            render_width: 640,
            render_height: 640,
            require_closed_manifold: true,
            require_surface_provenance: true,
        },
    }
}

pub(crate) fn compile_e005_surface_adornments(
    source_value: &Value,
    lowering: &ForgeVisualAuthorLoweringV1,
) -> Result<Vec<SurfaceAdornmentProgram>, E005ProductionReviewFailureV1> {
    let source: ForgeVisualAuthorSourceV1 =
        serde_json::from_value(source_value.clone()).map_err(|error| {
            E005ProductionReviewFailureV1::new("E005_R3_SOURCE_INVALID", error.to_string())
        })?;
    if lowering.source_program_sha256
        != lower_forge_visual_author_source_v1(source_value)
            .map_err(core_failure)?
            .source_program_sha256
        || lowering.surface_plan.source_program_sha256 != lowering.source_program_sha256
        || lowering.surface_plan.bindings.is_empty()
        || lowering.surface_plan.bindings.len() > MAX_E005_SURFACE_ADORNMENTS
    {
        return Err(E005ProductionReviewFailureV1::new(
            "E005_R3_SURFACE_PLAN_INVALID",
            "R1 SurfacePlan must bind the exact source and fit the bounded PBR compiler.",
        ));
    }
    let materials = source
        .geometry_templates
        .get("materials")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            E005ProductionReviewFailureV1::new(
                "E005_R3_MATERIAL_GRAPH_INVALID",
                "R1 geometry templates must carry a typed material graph.",
            )
        })?;
    let material_map = materials
        .iter()
        .filter_map(|material| {
            Some((
                material.get("material_id")?.as_str()?.to_owned(),
                material.get("base_material_id")?.as_str()?.to_owned(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    if material_map.len() != materials.len() {
        return Err(E005ProductionReviewFailureV1::new(
            "E005_R3_MATERIAL_GRAPH_INVALID",
            "R1 material identities are incomplete or duplicated.",
        ));
    }
    let skill = builtin_surface_adornment_manifest_v3();
    skill.validate().map_err(core_failure)?;
    let skill_sha256 = skill.canonical_sha256().map_err(core_failure)?;
    let mut target_zones = BTreeSet::new();
    let mut programs = Vec::with_capacity(lowering.surface_plan.bindings.len());
    for binding in &lowering.surface_plan.bindings {
        let authored_material = material_map.get(&binding.material_id).ok_or_else(|| {
            E005ProductionReviewFailureV1::new(
                "E005_R3_MATERIAL_BINDING_MISSING",
                "SurfacePlan references a material absent from the exact geometry template.",
            )
        })?;
        let base_material =
            compiled_visual_base_material_id(authored_material).ok_or_else(|| {
                E005ProductionReviewFailureV1::new(
                    "E005_R3_MATERIAL_PRESET_UNSUPPORTED",
                    "SurfacePlan material has no reviewed A005/PBR compiler slot.",
                )
            })?;
        if !target_zones.insert(binding.material_zone_id.as_str()) {
            return Err(E005ProductionReviewFailureV1::new(
                "E005_R3_SURFACE_ZONE_DUPLICATE",
                "SurfacePlan may compile at most one PBR program per Material Zone.",
            ));
        }
        let (kind, motif, coverage) = match binding.surface_profile {
            AuthorSurfaceProfileV1::PaintedMetal => ("micro_surface", "hex_microgrid", "full_zone"),
            AuthorSurfaceProfileV1::BrushedMetal => {
                ("normal_relief", "parallel_groove", "full_zone")
            }
            AuthorSurfaceProfileV1::DarkInset => ("pattern", "hex_microgrid", "center_band"),
            AuthorSurfaceProfileV1::Rubberized => ("normal_relief", "chevron_relief", "full_zone"),
            AuthorSurfaceProfileV1::EmissiveTrim => ("flowline", "double_flowline", "edge_band"),
        };
        let tuning = binding.edge_wear.max(binding.micro_detail);
        let intensity = if tuning < 0.34 {
            "subtle"
        } else if tuning < 0.67 {
            "balanced"
        } else {
            "pronounced"
        };
        let identity = semantic_sha256(&json!({
            "schema_version":"E005SurfaceAdornmentIdentity@1",
            "source_program_sha256":lowering.source_program_sha256,
            "surface_plan_sha256":lowering.surface_plan_sha256,
            "binding":binding,
            "kind":kind,
            "motif":motif,
            "intensity":intensity,
            "coverage":coverage,
            "base_material":base_material,
            "skill_sha256":skill_sha256,
        }))
        .map_err(core_failure)?;
        let seed = u32::from_str_radix(&identity[..8], 16).unwrap_or(0) & 0x7fff_ffff;
        let program = SurfaceAdornmentProgram {
            schema_version: "SurfaceAdornmentProgram@1".into(),
            program_id: format!("adorn_e005_{}", &identity[..40]),
            target_part_id: binding.part_id.clone(),
            target_zone_id: binding.material_zone_id.clone(),
            kind: kind.into(),
            motif: motif.into(),
            intensity: intensity.into(),
            coverage: coverage.into(),
            seed,
            base_material: base_material.into(),
            execution: "texture_bake".into(),
            skill_id: skill.skill_id.clone(),
            skill_version: skill.version,
            skill_sha256: skill_sha256.clone(),
            generator: "a005_v1".into(),
            non_functional_only: true,
        };
        program.validate().map_err(core_failure)?;
        programs.push(program);
    }
    Ok(programs)
}

fn verify_production_geometry(
    geometry: &RestrictedGeometryOutput,
    adornment_count: usize,
) -> Result<(), E005ProductionReviewFailureV1> {
    if geometry.readback.artifact_profile_id != "production_concept"
        || geometry.readback.glb_sha256 != geometry.glb_sha256
        || geometry.readback.triangle_count == 0
        || !geometry.readback.closed_manifold
        || !geometry.readback.surface_provenance_present
        || geometry.readback.visual_texture_set_count != adornment_count as u32
        || geometry.readback.visual_texture_map_count != adornment_count as u32 * 5
        || !geometry.readback.visual_texture_provenance_verified
    {
        return Err(E005ProductionReviewFailureV1::new(
            "E005_R3_PRODUCTION_GATE_FAILED",
            "Production GLB failed profile, manifold, surface or five-channel PBR readback.",
        ));
    }
    Ok(())
}

fn bounded_elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn core_failure(error: forgecad_core::CoreError) -> E005ProductionReviewFailureV1 {
    E005ProductionReviewFailureV1::new(error.code(), error.to_string())
}

fn geometry_failure(error: RestrictedGeometryError) -> E005ProductionReviewFailureV1 {
    E005ProductionReviewFailureV1::new(error.code, error.message)
}

#[cfg(test)]
mod tests {
    use std::{future::Future, sync::Arc};

    use serde_json::Value;

    use super::*;
    use crate::e005_visual_review::tests::GeometryFixture;

    fn source() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        ))
        .unwrap()
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn e005_r3_surface_plan_compiles_every_expanded_zone_and_tuning_changes_pbr_identity() {
        let source = source();
        let lowering = lower_forge_visual_author_source_v1(&source).unwrap();
        let first = compile_e005_surface_adornments(&source, &lowering).unwrap();
        assert_eq!(first.len(), lowering.surface_plan.bindings.len());
        assert_eq!(first.len(), 11);
        assert!(first.iter().all(|program| program.validate().is_ok()));

        let mut tuned = source;
        tuned["surface_bindings"][0]["micro_detail"] = serde_json::json!(0.91);
        let tuned_lowering = lower_forge_visual_author_source_v1(&tuned).unwrap();
        let second = compile_e005_surface_adornments(&tuned, &tuned_lowering).unwrap();
        assert_ne!(
            semantic_sha256(&first).unwrap(),
            semantic_sha256(&second).unwrap()
        );
        assert_eq!(
            lowering.shape_program_sha256,
            tuned_lowering.shape_program_sha256
        );
    }

    #[test]
    fn e005_r3_same_source_production_review_seals_real_pbr_readback_and_eight_views() {
        let geometry = GeometryFixture::default();
        let coordinator = E005ProductionReviewCoordinatorV1::new(Arc::new(geometry.clone()));
        let result = run(coordinator.execute(&source(), CancellationToken::new())).unwrap();
        assert_eq!(geometry.call_count(), 1);
        assert_eq!(result.review.artifact_profile_id, "production_concept");
        assert_eq!(result.review.surface_adornment_count, 11);
        assert_eq!(result.review.visual_texture_set_count, 11);
        assert_eq!(result.review.visual_texture_map_count, 55);
        assert!(result.review.visual_texture_provenance_verified);
        assert_eq!(result.review.fixed_views.len(), 8);
        assert_eq!(result.review.glb_sha256, result.geometry.glb_sha256);
        result.review.validate().unwrap();
        result
            .review
            .validate_against(&source(), &result.geometry)
            .unwrap();
        let mut stale = source();
        stale["seed"] = serde_json::json!(999);
        assert_eq!(
            result
                .review
                .validate_against(&stale, &result.geometry)
                .unwrap_err()
                .code(),
            "E005_R3_PRODUCTION_LINEAGE_INVALID"
        );
    }
}
