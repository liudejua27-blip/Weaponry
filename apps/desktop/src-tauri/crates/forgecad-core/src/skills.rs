//! A005 Product Skill contracts.
//!
//! Skills are immutable data and are intentionally separate from both the
//! Product Tool registry and the ShapeProgram runtime manifest.  A skill may
//! reference entries from those registries, but it cannot add an operation,
//! execute code, or acquire a write capability by naming one.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    component_recipes::RecipeSurfaceAdornmentSlot, semantic_sha256, CoreError, CoreResult,
};

const DOMAINS: &[&str] = &[
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
];
const PRODUCT_TOOLS: &[&str] = &[
    "forgecad.domain.inference.v1",
    "forgecad.reference.research.v1",
    "forgecad.style.recipe_selection.v1",
    "forgecad.profile.author.v1",
    "forgecad.profile.validate.v1",
    "forgecad.shape.author.v1",
    "forgecad.shape.validate.v1",
    "forgecad.plan.complete_concept.v1",
    "forgecad.geometry.build.v1",
    "forgecad.geometry.compile_readback.v1",
    "forgecad.render.concept.v1",
    "forgecad.candidate.evaluate.v1",
    "forgecad.preview.prepare.v1",
];
const G819_OPERATIONS: &[&str] = &[
    "box",
    "cylinder",
    "capsule",
    "wedge",
    "profile",
    "extrude",
    "revolve",
    "loft",
    "sweep",
    "mirror",
    "array",
    "radial_array",
    "union",
    "subtract",
    "bevel_approx",
    "surface_panel",
];
// C106 is intentionally an exact, reviewed first-party allowlist.  New
// Recipe IDs do not gain an A005 surface allowance merely by sharing a prefix
// or by appearing in the robotic-arm domain.
const C106_ARM_RECIPE_IDS: &[&str] = &[
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
    "recipe_c106_arm_turntable",
    "recipe_c106_arm_joint_housing",
    "recipe_c106_arm_link_armor",
    "recipe_c106_arm_cable_harness",
    "recipe_c106_arm_gripper",
    "recipe_c106_arm_surface_trim",
];
const C105_SURFACE_RECIPE_IDS: &[&str] = &[
    "recipe_future_prop_shell",
    "recipe_future_prop_trim",
    "recipe_vehicle_body_shell",
    "recipe_vehicle_lighting_trim",
    "recipe_aircraft_fuselage",
    "recipe_aircraft_trim",
    "recipe_robotic_arm_link",
    "recipe_robotic_arm_detail",
];
/// M101/M102/M108A's code-owned visual material namespace.  This is a
/// deliberately small allow-list rather than a prefix check: a Product Skill
/// may select a material already supported by ForgeCAD, but cannot mint a new
/// material or smuggle a texture source through a material identifier.
pub(crate) const MATERIAL_PRESET_IDS: &[&str] = &[
    "mat_aluminum",
    "mat_automotive_paint",
    "mat_composite",
    "mat_dark_glass",
    "mat_emissive_blue",
    "mat_graphite",
    "mat_rubber",
    "mat_signal_red",
];
const FORBIDDEN_TOKENS: &[&str] = &[
    "shell",
    "script",
    "javascript",
    "python",
    "command",
    "executable",
    "file_path",
    "filename",
    "url",
    "uri",
    "http://",
    "https://",
    "../",
    "..\\",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillExample {
    pub example_id: String,
    pub brief: String,
    pub expected_outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillProvenance {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillLicense {
    pub license_id: String,
    pub redistributable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentSkillManifest {
    pub schema_version: String,
    pub skill_id: String,
    pub version: u32,
    pub display_name: String,
    pub purpose: String,
    pub allowed_domains: Vec<String>,
    pub triggers: Vec<String>,
    pub product_tool_ids: Vec<String>,
    pub g819_operations: Vec<String>,
    pub recipe_ids: Vec<String>,
    pub material_preset_ids: Vec<String>,
    pub reference_hashes: Vec<String>,
    pub success_examples: Vec<SkillExample>,
    pub stop_examples: Vec<SkillExample>,
    pub author: SkillProvenance,
    pub source: SkillProvenance,
    pub license: SkillLicense,
    pub non_functional_only: bool,
}

impl AgentSkillManifest {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "AgentSkillManifest@1"
            || !valid_prefixed_id(&self.skill_id, "skill_")
            || self.version == 0
            || !self.non_functional_only
            || !safe_text(&self.display_name, 80)
            || !safe_text(&self.purpose, 500)
        {
            return Err(invalid(
                "SKILL_MANIFEST_INVALID",
                "Skill identity or visual-only boundary is invalid.",
            ));
        }
        validate_unique(
            "SKILL_DOMAIN_INVALID",
            &self.allowed_domains,
            1,
            4,
            |value| DOMAINS.contains(&value),
        )?;
        validate_unique("SKILL_TRIGGER_INVALID", &self.triggers, 1, 12, |value| {
            safe_text(value, 160)
        })?;
        validate_unique(
            "SKILL_TOOL_POLICY_INVALID",
            &self.product_tool_ids,
            1,
            13,
            |value| PRODUCT_TOOLS.contains(&value),
        )?;
        validate_unique(
            "SKILL_G819_POLICY_INVALID",
            &self.g819_operations,
            0,
            16,
            |value| G819_OPERATIONS.contains(&value),
        )?;
        validate_unique(
            "SKILL_RECIPE_POLICY_INVALID",
            &self.recipe_ids,
            0,
            24,
            |value| valid_prefixed_id(value, "recipe_"),
        )?;
        validate_unique(
            "SKILL_MATERIAL_POLICY_INVALID",
            &self.material_preset_ids,
            0,
            12,
            |value| MATERIAL_PRESET_IDS.contains(&value),
        )?;
        validate_unique(
            "SKILL_REFERENCE_HASH_INVALID",
            &self.reference_hashes,
            0,
            32,
            |value| valid_sha256(value),
        )?;
        validate_examples("success_examples", &self.success_examples)?;
        validate_examples("stop_examples", &self.stop_examples)?;
        validate_provenance(&self.author, "author_")?;
        validate_provenance(&self.source, "source_")?;
        if !matches!(
            self.license.license_id.as_str(),
            "ForgeCAD-Internal-Visual-Only" | "self_declared_original"
        ) || self.license.redistributable
        {
            return Err(invalid(
                "SKILL_LICENSE_INVALID",
                "Skill license must be visual-only and non-redistributable.",
            ));
        }
        Ok(())
    }

    pub fn canonical_sha256(&self) -> CoreResult<String> {
        self.validate()?;
        semantic_sha256(self)
    }
}

/// The opt-in first-party starter for A005.  It is intentionally just data:
/// creating it does not evaluate or enable it, and it exposes no executable
/// tool, URL, path, or new ShapeProgram operation.
pub fn builtin_surface_adornment_manifest() -> AgentSkillManifest {
    let examples = |prefix: &str, expected: &str| {
        (1..=3)
            .map(|number| SkillExample {
                example_id: format!("skillex_{prefix}_{number}"),
                brief: format!("{prefix} visual surface detail {number}"),
                expected_outcome: expected.to_string(),
            })
            .collect()
    };
    AgentSkillManifest {
        schema_version: "AgentSkillManifest@1".to_string(),
        skill_id: "skill_first_party_surface_adornment".to_string(),
        version: 1,
        display_name: "表面细节".to_string(),
        purpose: "为已有材质区添加受限的雕刻感、纹样或流线纹理，不创建几何或工程信息。".to_string(),
        allowed_domains: DOMAINS.iter().map(|value| (*value).to_string()).collect(),
        triggers: vec![
            "表面细节".to_string(),
            "雕刻感".to_string(),
            "纹样".to_string(),
            "流线".to_string(),
        ],
        product_tool_ids: vec![
            "forgecad.geometry.compile_readback.v1".to_string(),
            "forgecad.render.concept.v1".to_string(),
            "forgecad.candidate.evaluate.v1".to_string(),
            "forgecad.preview.prepare.v1".to_string(),
        ],
        // Texture baking is deliberately not a ShapeProgram operation.  The
        // empty policy therefore cannot grow G819 at activation time.
        g819_operations: Vec::new(),
        recipe_ids: C105_SURFACE_RECIPE_IDS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        material_preset_ids: MATERIAL_PRESET_IDS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        reference_hashes: Vec::new(),
        success_examples: examples(
            "ok",
            "Create a bounded texture_bake preview without product-state writes.",
        ),
        stop_examples: examples(
            "stop",
            "Stop without a write when no existing material zone is selected.",
        ),
        author: SkillProvenance {
            kind: "forgecad_first_party".to_string(),
            id: "author_a005_surface".to_string(),
        },
        source: SkillProvenance {
            kind: "forgecad_first_party".to_string(),
            id: "source_a005_surface".to_string(),
        },
        license: SkillLicense {
            license_id: "ForgeCAD-Internal-Visual-Only".to_string(),
            redistributable: false,
        },
        non_functional_only: true,
    }
}

/// A005 v2 is the first immutable manifest that explicitly grants the nine
/// reviewed C106 mechanical-arm Recipes. Keeping v1 constructible preserves
/// historical hashes and guarantees that old activations cannot gain these
/// permissions merely because a newer runtime knows the C106 IDs.
pub fn builtin_surface_adornment_manifest_v2() -> AgentSkillManifest {
    let mut manifest = builtin_surface_adornment_manifest();
    manifest.version = 2;
    manifest
        .recipe_ids
        .extend(C106_ARM_RECIPE_IDS.iter().map(|value| (*value).to_string()));
    manifest
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentSkillActivation {
    pub schema_version: String,
    pub activation_id: String,
    pub skill_id: String,
    pub skill_version: u32,
    pub skill_sha256: String,
    pub enabled: bool,
    pub updated_at: String,
}

impl AgentSkillActivation {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "AgentSkillActivation@1"
            || !valid_prefixed_id(&self.activation_id, "skillact_")
            || !valid_prefixed_id(&self.skill_id, "skill_")
            || self.skill_version == 0
            || !valid_sha256(&self.skill_sha256)
            || !safe_text(&self.updated_at, 128)
        {
            return Err(invalid(
                "SKILL_ACTIVATION_INVALID",
                "Skill activation pointer is invalid.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillEvalStatus {
    Passed,
    Failed,
}

impl SkillEvalStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentSkillEvalReport {
    pub schema_version: String,
    pub report_id: String,
    pub skill_id: String,
    pub skill_version: u32,
    pub skill_sha256: String,
    pub status: SkillEvalStatus,
    pub findings: Vec<String>,
    pub evaluated_at: String,
}

impl AgentSkillEvalReport {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != "AgentSkillEvalReport@1"
            || !valid_prefixed_id(&self.report_id, "skilleval_")
            || !valid_prefixed_id(&self.skill_id, "skill_")
            || self.skill_version == 0
            || !valid_sha256(&self.skill_sha256)
            || self.findings.len() > 32
            || !self.findings.iter().all(|finding| safe_text(finding, 300))
            || !safe_text(&self.evaluated_at, 128)
        {
            return Err(invalid(
                "SKILL_EVAL_REPORT_INVALID",
                "Skill evaluation report is invalid.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentSkillDryRun {
    pub schema_version: String,
    pub skill_id: String,
    pub skill_version: u32,
    pub skill_sha256: String,
    pub allowed_product_tool_ids: Vec<String>,
    pub allowed_g819_operations: Vec<String>,
    pub allowed_recipe_ids: Vec<String>,
    pub allowed_material_preset_ids: Vec<String>,
    pub product_state_write_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceAdornmentProgram {
    pub schema_version: String,
    pub program_id: String,
    pub target_part_id: String,
    pub target_zone_id: String,
    pub kind: String,
    pub motif: String,
    pub intensity: String,
    pub coverage: String,
    pub seed: u32,
    pub base_material: String,
    pub execution: String,
    pub skill_id: String,
    pub skill_version: u32,
    pub skill_sha256: String,
    pub generator: String,
    pub non_functional_only: bool,
}

impl SurfaceAdornmentProgram {
    pub fn validate(&self) -> CoreResult<()> {
        let motif_matches_kind = matches!(
            (self.kind.as_str(), self.motif.as_str()),
            ("normal_relief", "parallel_groove" | "chevron_relief")
                | ("pattern", "hex_microgrid")
                | ("flowline", "double_flowline")
                | ("micro_surface", "hex_microgrid" | "parallel_groove")
        );
        if self.schema_version != "SurfaceAdornmentProgram@1"
            || !valid_prefixed_id(&self.program_id, "adorn_")
            || !valid_prefixed_id(&self.target_part_id, "part_")
            || !valid_prefixed_id(&self.target_zone_id, "zone_")
            || !motif_matches_kind
            || !matches!(
                self.intensity.as_str(),
                "subtle" | "balanced" | "pronounced"
            )
            || !matches!(
                self.coverage.as_str(),
                "full_zone" | "center_band" | "edge_band" | "symmetric_pair"
            )
            || !MATERIAL_PRESET_IDS.contains(&self.base_material.as_str())
            || self.execution != "texture_bake"
            || !valid_prefixed_id(&self.skill_id, "skill_")
            || self.skill_version == 0
            || !valid_sha256(&self.skill_sha256)
            || self.generator != "a005_v1"
            || !self.non_functional_only
        {
            return Err(invalid(
                "SURFACE_ADORNMENT_INVALID",
                "Surface adornment must remain a bounded visual texture bake.",
            ));
        }
        Ok(())
    }

    /// Stable texture-set identity shared with the restricted geometry
    /// executor.  The caller derives `mat_a005_<first 32 hex chars>` only
    /// after this validation succeeds; this program never becomes a
    /// ShapeProgram operation or an executable generator.
    pub fn canonical_sha256(&self) -> CoreResult<String> {
        self.validate()?;
        semantic_sha256(self)
    }

    /// Enforces the reviewed C106 design-surface contract after a target part
    /// has been resolved from an immutable AssemblyGraph.  Older and other
    /// domain Recipes retain their existing A005 lifecycle; C106 entries are
    /// intentionally denied unless their projected slot explicitly permits
    /// this visual-only program.
    pub fn validate_recipe_surface_slot(
        &self,
        recipe_id: &str,
        slots: &[RecipeSurfaceAdornmentSlot],
    ) -> CoreResult<()> {
        self.validate()?;
        if !C106_ARM_RECIPE_IDS.contains(&recipe_id) {
            return Ok(());
        }
        if slots.iter().any(|slot| {
            slot.zone_id == self.target_zone_id
                && slot.allowed_kinds.iter().any(|kind| kind == &self.kind)
                && slot.allowed_motifs.iter().any(|motif| motif == &self.motif)
                && slot
                    .allowed_coverages
                    .iter()
                    .any(|coverage| coverage == &self.coverage)
        }) {
            return Ok(());
        }
        Err(invalid(
            "SURFACE_ADORNMENT_RECIPE_SLOT_DENIED",
            "C106 surface appearance must match a reviewed Recipe material-zone slot.",
        ))
    }
}

pub(crate) fn valid_prefixed_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.len() <= 120
            && suffix.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
    })
}

pub(crate) fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

fn safe_text(value: &str, max: usize) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value.chars().any(char::is_control)
        && !value.starts_with('/')
        && !(value.len() > 2
            && value.as_bytes()[1] == b':'
            && value.as_bytes()[0].is_ascii_alphabetic())
        && !FORBIDDEN_TOKENS.iter().any(|token| lower.contains(token))
}

fn validate_unique(
    code: &'static str,
    values: &[String],
    min: usize,
    max: usize,
    predicate: impl Fn(&str) -> bool,
) -> CoreResult<()> {
    if values.len() < min
        || values.len() > max
        || values.iter().collect::<BTreeSet<_>>().len() != values.len()
        || !values.iter().all(|value| predicate(value))
    {
        return Err(invalid(
            code,
            "Skill namespace references are unknown, duplicated or outside their bounded contract.",
        ));
    }
    Ok(())
}

fn validate_examples(field: &str, values: &[SkillExample]) -> CoreResult<()> {
    if !(3..=20).contains(&values.len())
        || values
            .iter()
            .map(|value| &value.example_id)
            .collect::<BTreeSet<_>>()
            .len()
            != values.len()
        || !values.iter().all(|value| {
            valid_prefixed_id(&value.example_id, "skillex_")
                && safe_text(&value.brief, 600)
                && safe_text(&value.expected_outcome, 600)
        })
    {
        return Err(invalid(
            "SKILL_EXAMPLES_INVALID",
            match field {
                "success_examples" => "Skill must provide at least three safe success examples.",
                _ => "Skill must provide at least three safe stop examples.",
            },
        ));
    }
    Ok(())
}

fn validate_provenance(value: &SkillProvenance, prefix: &str) -> CoreResult<()> {
    if !matches!(
        value.kind.as_str(),
        "forgecad_user" | "forgecad_first_party"
    ) || !valid_prefixed_id(&value.id, prefix)
    {
        return Err(invalid(
            "SKILL_PROVENANCE_INVALID",
            "Skill provenance must be a bounded local author/source declaration.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> AgentSkillManifest {
        AgentSkillManifest {
            schema_version: "AgentSkillManifest@1".into(),
            skill_id: "skill_surface_groove".into(),
            version: 1,
            display_name: "Surface groove".into(),
            purpose: "Add a controlled visual groove.".into(),
            allowed_domains: vec!["pack_future_weapon_prop".into()],
            triggers: vec!["surface groove".into()],
            product_tool_ids: vec!["forgecad.preview.prepare.v1".into()],
            g819_operations: vec!["surface_panel".into()],
            recipe_ids: vec!["recipe_future_prop_shell".into()],
            material_preset_ids: vec!["mat_graphite".into()],
            reference_hashes: vec!["a".repeat(64)],
            success_examples: (1..=3)
                .map(|n| SkillExample {
                    example_id: format!("skillex_ok_{n}"),
                    brief: format!("brief {n}"),
                    expected_outcome: "bounded visual detail".into(),
                })
                .collect(),
            stop_examples: (1..=3)
                .map(|n| SkillExample {
                    example_id: format!("skillex_stop_{n}"),
                    brief: format!("stop {n}"),
                    expected_outcome: "stop without write".into(),
                })
                .collect(),
            author: SkillProvenance {
                kind: "forgecad_user".into(),
                id: "author_local".into(),
            },
            source: SkillProvenance {
                kind: "forgecad_user".into(),
                id: "source_local".into(),
            },
            license: SkillLicense {
                license_id: "self_declared_original".into(),
                redistributable: false,
            },
            non_functional_only: true,
        }
    }

    #[test]
    fn manifest_rejects_unknown_namespaces_and_executable_text() {
        let mut value = manifest();
        value.product_tool_ids = vec!["forgecad.shell.run.v1".into()];
        assert_eq!(
            value.validate().unwrap_err().code(),
            "SKILL_TOOL_POLICY_INVALID"
        );
        let mut value = manifest();
        value.triggers = vec!["https://invalid.example".into()];
        assert_eq!(
            value.validate().unwrap_err().code(),
            "SKILL_TRIGGER_INVALID"
        );
    }

    #[test]
    fn builtin_surface_adornment_v2_is_an_explicit_hash_separated_c106_grant() {
        let legacy = builtin_surface_adornment_manifest();
        let current = builtin_surface_adornment_manifest_v2();
        assert_eq!(legacy.version, 1);
        assert_eq!(legacy.recipe_ids.len(), C105_SURFACE_RECIPE_IDS.len());
        assert!(C106_ARM_RECIPE_IDS
            .iter()
            .all(|recipe_id| !legacy.recipe_ids.iter().any(|id| id == recipe_id)));
        assert_eq!(current.version, 2);
        assert_eq!(
            current.recipe_ids.len(),
            C105_SURFACE_RECIPE_IDS.len() + C106_ARM_RECIPE_IDS.len()
        );
        assert!(C106_ARM_RECIPE_IDS
            .iter()
            .all(|recipe_id| current.recipe_ids.iter().any(|id| id == recipe_id)));
        assert_ne!(
            legacy.canonical_sha256().unwrap(),
            current.canonical_sha256().unwrap()
        );
    }

    #[test]
    fn surface_adornment_is_bounded_and_has_a_stable_texture_identity() {
        let program = SurfaceAdornmentProgram {
            schema_version: "SurfaceAdornmentProgram@1".into(),
            program_id: "adorn_surface_groove".into(),
            target_part_id: "part_surface".into(),
            target_zone_id: "zone_surface".into(),
            kind: "normal_relief".into(),
            motif: "parallel_groove".into(),
            intensity: "subtle".into(),
            coverage: "center_band".into(),
            seed: 7,
            base_material: "mat_graphite".into(),
            execution: "texture_bake".into(),
            skill_id: "skill_surface_groove".into(),
            skill_version: 1,
            skill_sha256: "a".repeat(64),
            generator: "a005_v1".into(),
            non_functional_only: true,
        };
        assert_eq!(program.canonical_sha256().unwrap().len(), 64);
        let mut invalid = program;
        invalid.execution = "shell".into();
        assert_eq!(
            invalid.canonical_sha256().unwrap_err().code(),
            "SURFACE_ADORNMENT_INVALID"
        );
    }

    #[test]
    fn c106_surface_adornment_requires_one_reviewed_recipe_zone_slot() {
        let program = SurfaceAdornmentProgram {
            schema_version: "SurfaceAdornmentProgram@1".into(),
            program_id: "adorn_c106_link_flowline".into(),
            target_part_id: "part_c106_link".into(),
            target_zone_id: "zone_c106_link_shell".into(),
            kind: "flowline".into(),
            motif: "double_flowline".into(),
            intensity: "balanced".into(),
            coverage: "center_band".into(),
            seed: 106,
            base_material: "mat_aluminum".into(),
            execution: "texture_bake".into(),
            skill_id: "skill_surface_groove".into(),
            skill_version: 1,
            skill_sha256: "a".repeat(64),
            generator: "a005_v1".into(),
            non_functional_only: true,
        };
        let slots = vec![RecipeSurfaceAdornmentSlot {
            slot_id: "adornslot_c106_link_shell".into(),
            zone_id: "zone_c106_link_shell".into(),
            allowed_kinds: vec!["flowline".into()],
            allowed_motifs: vec!["double_flowline".into()],
            allowed_coverages: vec!["center_band".into()],
        }];
        program
            .validate_recipe_surface_slot("recipe_c106_arm_link_armor", &slots)
            .unwrap();
        assert_eq!(
            program
                .validate_recipe_surface_slot("recipe_c106_arm_link_armor", &[])
                .unwrap_err()
                .code(),
            "SURFACE_ADORNMENT_RECIPE_SLOT_DENIED"
        );
        let mut wrong_motif = slots;
        wrong_motif[0].allowed_motifs = vec!["parallel_groove".into()];
        assert_eq!(
            program
                .validate_recipe_surface_slot("recipe_c106_arm_link_armor", &wrong_motif)
                .unwrap_err()
                .code(),
            "SURFACE_ADORNMENT_RECIPE_SLOT_DENIED"
        );
        // Exact matching is scoped to the reviewed C106 IDs: an old recipe
        // does not get a retroactive slot requirement.
        program
            .validate_recipe_surface_slot("recipe_robotic_arm_link", &[])
            .unwrap();
    }
}
