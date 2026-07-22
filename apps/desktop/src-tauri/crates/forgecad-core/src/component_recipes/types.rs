use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A bounded local transform.  C105 uses Euler XYZ only so it can reject
/// transforms that would silently introduce shear into a ShapeProgram IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeTransform {
    #[serde(default = "zero_vector")]
    pub position: [f64; 3],
    #[serde(default = "zero_vector", alias = "rotation_degrees")]
    pub rotation: [f64; 3],
    #[serde(default = "one_vector")]
    pub scale: [f64; 3],
}

fn zero_vector() -> [f64; 3] {
    [0.0, 0.0, 0.0]
}

fn one_vector() -> [f64; 3] {
    [1.0, 1.0, 1.0]
}

impl Default for RecipeTransform {
    fn default() -> Self {
        Self {
            position: zero_vector(),
            rotation: zero_vector(),
            scale: one_vector(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeConnector {
    pub connector_id: String,
    pub kind: String,
    #[serde(default = "zero_vector")]
    pub position: [f64; 3],
    pub normal: [f64; 3],
    pub up: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeChildSlot {
    pub slot_id: String,
    pub accepted_roles: Vec<String>,
    pub child_recipe_id: String,
    pub parent_connector_id: String,
    pub child_connector_id: String,
    #[serde(default = "one_count")]
    pub count: u32,
    pub required: bool,
    #[serde(default)]
    pub parent_local_transform: RecipeTransform,
}

/// A reviewed A005 design-surface allowance.  It is descriptive recipe data,
/// never an executable geometry instruction or a new ShapeProgram operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecipeSurfaceAdornmentSlot {
    pub slot_id: String,
    pub zone_id: String,
    pub allowed_kinds: Vec<String>,
    pub allowed_motifs: Vec<String>,
    pub allowed_coverages: Vec<String>,
}

fn one_count() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeFrame {
    pub position: [f64; 3],
    pub normal: [f64; 3],
    pub up: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EditableComponentRecipe {
    pub schema_version: String,
    pub recipe_id: String,
    pub version: u32,
    pub component_role: String,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub profiles: Vec<Value>,
    #[serde(default)]
    pub section_sets: Vec<Value>,
    pub shape_program_template: Value,
    #[serde(default)]
    pub feature_template: Vec<Value>,
    #[serde(default)]
    pub parameter_bindings: Vec<Value>,
    #[serde(default)]
    pub material_zones: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_adornment_slots: Vec<RecipeSurfaceAdornmentSlot>,
    #[serde(default)]
    pub connectors: Vec<RecipeConnector>,
    pub pivot: RecipeFrame,
    #[serde(default)]
    pub child_slots: Vec<RecipeChildSlot>,
    pub allowed_domains: Vec<String>,
    pub review_state: Value,
    pub quality_status: String,
    pub source: Value,
    pub license: Value,
    pub triangle_estimate: u64,
    pub non_functional_only: bool,
    pub root_local_transform: RecipeTransform,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeInstantiationRequest {
    pub schema_version: String,
    pub context_mode: String,
    pub request_id: String,
    pub project_id: Option<String>,
    pub base_asset_version_id: Option<String>,
    pub snapshot_revision: Option<u64>,
    pub domain_pack_id: String,
    /// The exact reviewed catalog identity. The repository uses it with
    /// provenance before selecting C105 versus C106; it is deliberately not
    /// part of the historical request hash because the candidate already
    /// seals the same registry hash independently.
    pub recipe_registry_sha256: String,
    pub recipe: ComponentRecipeRef,
    pub target_part_id: Option<String>,
    pub slot_bindings: Vec<RecipeSlotBinding>,
    pub parameter_values: Vec<RecipeParameterValue>,
    pub material_zone_overrides: Vec<RecipeMaterialZoneOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ComponentRecipeRef {
    pub schema_version: String,
    pub recipe_id: String,
    pub version: u32,
    pub recipe_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecipeSlotBinding {
    pub slot_id: String,
    pub child_recipe: ComponentRecipeRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeParameterValue {
    pub parameter_id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecipeMaterialZoneOverride {
    pub zone_id: String,
    pub material_preset_id: String,
}

/// Persistable provenance for an instance once a higher layer has previewed
/// and confirmed it.  The engine itself never persists this value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ComponentRecipeInstanceProvenance {
    pub schema_version: String,
    pub instance_id: String,
    pub instance_path: String,
    pub recipe: ComponentRecipeRef,
    pub registry_sha256: String,
    pub policy_version: String,
    pub domain_pack_id: String,
    pub parent_instance_id: Option<String>,
    pub parent_slot_id: Option<String>,
    pub source: Value,
    pub license: Value,
    pub review_state: Value,
    pub quality_status: String,
    pub non_functional_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedComponentInstance {
    pub instance_id: String,
    pub instance_path: String,
    pub component_role: String,
    pub world_transform: [[f64; 4]; 4],
    pub recipe: EditableComponentRecipe,
    pub provenance: ComponentRecipeInstanceProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExpandedComponentCandidate {
    pub schema_version: String,
    pub candidate_id: String,
    pub request_id: String,
    pub context_mode: String,
    pub project_id: Option<String>,
    pub base_asset_version_id: Option<String>,
    pub snapshot_revision: Option<u64>,
    pub recipe: ComponentRecipeRef,
    pub target_part_id: Option<String>,
    pub instance_path: String,
    pub changeset_id: Option<String>,
    pub expanded_shape_program: Value,
    pub expanded_assembly_graph: Value,
    pub component_recipe_instances: Vec<ComponentRecipeInstanceProvenance>,
    pub registry_sha256: String,
    pub candidate_sha256: String,
    pub status: String,
    pub quality_profile: String,
    pub non_functional_only: bool,
    /// Deterministic expansion facts for the Rust caller. This is deliberately
    /// not serialized into ComponentRecipeCandidate@1; the schema persists the
    /// matching provenance through `expanded_assembly_graph` instead.
    #[serde(skip, default)]
    pub instances: Vec<ExpandedComponentInstance>,
}
