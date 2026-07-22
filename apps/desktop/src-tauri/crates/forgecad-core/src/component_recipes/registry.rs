use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{semantic_sha256, CoreError, CoreResult};

use super::{EditableComponentRecipe, RecipeValidator};

const EMBEDDED_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
));

// M108B production recipes are deliberately a second immutable registry.  The
// C105 v1 aggregate stays byte-for-byte stable because persisted recipe refs
// and its expansion golden carry its semantic hash.
const EMBEDDED_PRODUCTION_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/production-component-recipe-registry.json"
));

// C106 keeps the mechanical-arm golden path in a dedicated immutable pack.
// It must never rewrite the broad M108B v1 catalog because existing recipe
// references are hash-pinned to that registry.
const EMBEDDED_C106_ROBOTIC_ARM_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c106-robotic-arm-component-recipe-registry.json"
));

// C110C keeps additive attachment Recipes in a separate immutable catalog so
// the existing C106 root/child registry hash and its production evidence do
// not drift when the first composable attachment is introduced.
const EMBEDDED_C110C_ROBOTIC_ARM_ATTACHMENT_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c110c-robotic-arm-attachment-recipe-registry.json"
));

// C110G is an independent visual Recipe/Connector catalog for the parallel-
// link arm family.  It intentionally does not alter the C106 serial-chain
// registry: an architecture choice must change the immutable recipe lineage,
// not merely move C106 parts in the viewport.
const EMBEDDED_C110G_PARALLEL_LINK_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/c110g-parallel-link-component-recipe-registry.json"
));

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    schema_version: String,
    registry_id: String,
    policy_version: String,
    recipes: Vec<EditableComponentRecipe>,
}

/// An immutable, reviewed Recipe catalog.  IDs are indexed in a BTreeMap so
/// lookups and the registry identity never depend on fixture ordering.
#[derive(Debug, Clone)]
pub struct RecipeRegistry {
    registry_id: String,
    version: String,
    recipes: BTreeMap<String, EditableComponentRecipe>,
    registry_sha256: String,
}

impl RecipeRegistry {
    pub fn from_embedded() -> CoreResult<Self> {
        Self::from_json(EMBEDDED_REGISTRY)
    }

    /// Explicit M108B selection entry.  Callers must opt in rather than
    /// allowing the historical C105 catalog to drift as new production assets
    /// are introduced.
    pub fn from_embedded_production() -> CoreResult<Self> {
        Self::from_json(EMBEDDED_PRODUCTION_REGISTRY)
    }

    /// C106 mechanical-arm-only production pack.  Selection is explicit at
    /// the app-server boundary; loading it cannot mutate the frozen M108B
    /// v1 registry or its persisted refs.
    pub fn from_embedded_c106_robotic_arm() -> CoreResult<Self> {
        Self::from_json(EMBEDDED_C106_ROBOTIC_ARM_REGISTRY)
    }

    /// C110C additive visual attachments.  The catalog is deliberately
    /// separate from C106 so the root asset lineage remains byte-stable while
    /// an attachment ChangeSet seals this second registry identity.
    pub fn from_embedded_c110c_robotic_arm_attachments() -> CoreResult<Self> {
        Self::from_json(EMBEDDED_C110C_ROBOTIC_ARM_ATTACHMENT_REGISTRY)
    }

    /// C110G parallel-link visual assembly catalog.  This is a separate
    /// registry so its root, connectors, material zones and GLB provenance
    /// remain distinguishable from the C106 serial-chain golden.
    pub fn from_embedded_c110g_parallel_link() -> CoreResult<Self> {
        Self::from_json(EMBEDDED_C110G_PARALLEL_LINK_REGISTRY)
    }

    pub fn from_json(value: &str) -> CoreResult<Self> {
        let canonical_value = serde_json::from_str::<serde_json::Value>(value).map_err(|e| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_REGISTRY_INVALID",
                format!("Component Recipe registry is invalid JSON: {e}"),
            )
        })?;
        let registry_sha256 = semantic_sha256(&canonical_value)?;
        let document = serde_json::from_str::<RegistryDocument>(value).map_err(|e| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_REGISTRY_INVALID",
                format!("Component Recipe registry is invalid JSON: {e}"),
            )
        })?;
        Self::from_document(document, registry_sha256)
    }

    fn from_document(document: RegistryDocument, registry_sha256: String) -> CoreResult<Self> {
        if document.schema_version != "EditableComponentRecipeRegistry@1"
            || document.registry_id.is_empty()
            || document.policy_version != "ComponentRecipePolicy@1"
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REGISTRY_INVALID",
                "Component Recipe registry must use its stable v1 identity.",
            ));
        }
        let mut recipes = BTreeMap::new();
        for recipe in document.recipes {
            if recipes.insert(recipe.recipe_id.clone(), recipe).is_some() {
                return Err(CoreError::invalid_data(
                    "COMPONENT_RECIPE_DUPLICATE_ID",
                    "Component Recipe registry contains a duplicate recipe_id.",
                ));
            }
        }
        let registry = Self {
            registry_id: document.registry_id,
            version: document.policy_version,
            recipes,
            registry_sha256: String::new(),
        };
        RecipeValidator::validate_registry(&registry)?;
        Ok(Self {
            registry_sha256,
            ..registry
        })
    }

    pub fn registry_id(&self) -> &str {
        &self.registry_id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn registry_sha256(&self) -> &str {
        &self.registry_sha256
    }

    pub fn recipe(&self, recipe_id: &str) -> Option<&EditableComponentRecipe> {
        self.recipes.get(recipe_id)
    }

    pub fn recipes(&self) -> impl Iterator<Item = &EditableComponentRecipe> {
        self.recipes.values()
    }

    pub(crate) fn recipe_map(&self) -> &BTreeMap<String, EditableComponentRecipe> {
        &self.recipes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_recipes::{
        ComponentRecipeRef, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
    };

    #[test]
    fn c110g_parallel_link_registry_is_independent_and_expandable() {
        let registry = RecipeRegistry::from_embedded_c110g_parallel_link().unwrap();
        assert_eq!(
            registry.registry_id(),
            "registry_c110g_parallel_link_robotic_arm_v1"
        );
        assert_eq!(registry.recipes().count(), 5);
        let root = registry.recipe("recipe_c110g_parallel_link_root").unwrap();
        let candidate = RecipeExpander::expand(
            &registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: "recipereq_c110g_parallel_link_fixture".into(),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                recipe_registry_sha256: registry.registry_sha256().into(),
                recipe: ComponentRecipeRef {
                    schema_version: "ComponentRecipeRef@1".into(),
                    recipe_id: root.recipe_id.clone(),
                    version: root.version,
                    recipe_sha256: RecipeValidator::recipe_sha256(root).unwrap(),
                },
                target_part_id: None,
                slot_bindings: vec![],
                parameter_values: vec![],
                material_zone_overrides: vec![],
            },
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        assert_eq!(candidate.instances.len(), 6);
        assert_eq!(
            candidate.expanded_assembly_graph["parts"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            candidate.expanded_assembly_graph["connections"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert!(candidate
            .component_recipe_instances
            .iter()
            .all(|instance| instance.registry_sha256 == registry.registry_sha256()));
    }
}
