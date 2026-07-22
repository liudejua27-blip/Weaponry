use serde::{Deserialize, Serialize};

/// Fixed C105 v1 resource caps.  A caller may only tighten these values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecipeExpansionPolicy {
    pub schema_version: String,
    pub max_depth: u32,
    pub max_instances: u32,
    pub max_operations: u32,
    pub max_profiles: u32,
    pub max_sections: u32,
    pub max_material_zones: u32,
    pub max_triangles: u64,
}

impl Default for RecipeExpansionPolicy {
    fn default() -> Self {
        Self {
            schema_version: "RecipeExpansionPolicy@1".into(),
            max_depth: 6,
            max_instances: 64,
            max_operations: 512,
            max_profiles: 128,
            max_sections: 512,
            max_material_zones: 128,
            max_triangles: 100_000,
        }
    }
}
