//! C105 deterministic, Rust-owned component Recipe expansion.
//!
//! Recipes are reviewed, versioned library entries.  This module deliberately
//! creates only an in-memory candidate: it has no repository, Snapshot, CAS,
//! Provider, filesystem or geometry-executor dependency.  A higher layer must
//! still take the candidate through ChangeSet preview and confirm before it can
//! affect an immutable asset version.

mod expand;
mod ids;
mod policy;
mod registry;
pub(crate) mod transform;
mod types;
mod validate;

pub use expand::RecipeExpander;
pub use policy::RecipeExpansionPolicy;
pub use registry::RecipeRegistry;
pub use types::{
    ComponentRecipeInstanceProvenance, ComponentRecipeRef, EditableComponentRecipe,
    ExpandedComponentCandidate, ExpandedComponentInstance, RecipeConnector, RecipeFrame,
    RecipeInstantiationRequest, RecipeMaterialZoneOverride, RecipeParameterValue,
    RecipeSlotBinding, RecipeSurfaceAdornmentSlot, RecipeTransform,
};
pub use validate::RecipeValidator;
