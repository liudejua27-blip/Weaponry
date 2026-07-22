use serde::{Deserialize, Serialize};

use crate::{contract_validation::require_schema, RpcError};

pub const MIGRATION_OWNERSHIP_SCHEMA_VERSION: &str = "ForgeCADMigrationOwnership@1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationFacetOwner {
    RustAppServer,
    PythonCompatibilityAdapter,
}

/// Faceted migration truth used after K001.  The legacy initialize
/// `state_owner` field remains untouched for K001 wire compatibility; K002
/// publishes this contract so lifecycle ownership can move independently from
/// persistence and product-core ownership without ever implying dual writes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MigrationOwnership {
    pub schema_version: String,
    pub agent_lifecycle: MigrationFacetOwner,
    pub lifecycle_persistence: MigrationFacetOwner,
    pub product_core: MigrationFacetOwner,
}

impl MigrationOwnership {
    pub fn k002_transition() -> Self {
        Self {
            schema_version: MIGRATION_OWNERSHIP_SCHEMA_VERSION.to_string(),
            agent_lifecycle: MigrationFacetOwner::RustAppServer,
            lifecycle_persistence: MigrationFacetOwner::PythonCompatibilityAdapter,
            product_core: MigrationFacetOwner::PythonCompatibilityAdapter,
        }
    }

    /// Final K003 ownership: both durable Agent history and authoritative
    /// product state are written by the Rust app-server/core.  Python is no
    /// longer a persistence facet and is therefore intentionally absent from
    /// this contract.
    pub fn k003_rust_first() -> Self {
        Self {
            schema_version: MIGRATION_OWNERSHIP_SCHEMA_VERSION.to_string(),
            agent_lifecycle: MigrationFacetOwner::RustAppServer,
            lifecycle_persistence: MigrationFacetOwner::RustAppServer,
            product_core: MigrationFacetOwner::RustAppServer,
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "ownership.schema_version",
            &self.schema_version,
            MIGRATION_OWNERSHIP_SCHEMA_VERSION,
        )?;
        let k002_transition = self.agent_lifecycle == MigrationFacetOwner::RustAppServer
            && self.lifecycle_persistence == MigrationFacetOwner::PythonCompatibilityAdapter
            && self.product_core == MigrationFacetOwner::PythonCompatibilityAdapter;
        let k003_rust_first = self.agent_lifecycle == MigrationFacetOwner::RustAppServer
            && self.lifecycle_persistence == MigrationFacetOwner::RustAppServer
            && self.product_core == MigrationFacetOwner::RustAppServer;
        if !k002_transition && !k003_rust_first {
            return Err(RpcError::invalid_params(
                "Ownership must describe either the K002 single-Python-writer transition or the K003 all-Rust writer state.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MigrationOwnershipResult {
    pub schema_version: String,
    pub ownership: MigrationOwnership,
}

impl MigrationOwnershipResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "ownership_result.schema_version",
            &self.schema_version,
            MIGRATION_OWNERSHIP_SCHEMA_VERSION,
        )?;
        self.ownership.validate()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn k002_ownership_is_faceted_without_claiming_product_core() {
        let ownership = MigrationOwnership::k002_transition();
        ownership.validate().unwrap();
        assert_eq!(
            serde_json::to_value(ownership).unwrap(),
            json!({
                "schema_version": "ForgeCADMigrationOwnership@1",
                "agent_lifecycle": "rust_app_server",
                "lifecycle_persistence": "python_compatibility_adapter",
                "product_core": "python_compatibility_adapter"
            })
        );
    }

    #[test]
    fn ownership_rejects_premature_rust_persistence_claim() {
        let mut ownership = MigrationOwnership::k002_transition();
        ownership.lifecycle_persistence = MigrationFacetOwner::RustAppServer;
        assert!(ownership.validate().is_err());
    }

    #[test]
    fn k003_ownership_has_no_python_writer_facet() {
        let ownership = MigrationOwnership::k003_rust_first();
        ownership.validate().unwrap();
        assert_eq!(
            serde_json::to_value(ownership).unwrap(),
            json!({
                "schema_version": "ForgeCADMigrationOwnership@1",
                "agent_lifecycle": "rust_app_server",
                "lifecycle_persistence": "rust_app_server",
                "product_core": "rust_app_server"
            })
        );
    }
}
