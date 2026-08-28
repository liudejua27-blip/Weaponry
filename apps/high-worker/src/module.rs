//! Closed ForgeCAD module descriptors for the Native High evaluator.
//!
//! A module is a product-owned, typed capability boundary.  The descriptor is
//! intentionally data-only: it does not load a dynamic library, read a path,
//! or let a caller select a process.  Backends that are not present in the
//! build cohort remain explicit `unavailable` capabilities and are rejected by
//! the evaluator before any partial result can be returned.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MODULE_SCHEMA_VERSION: &str = "ForgeCadModule@1";
pub const MANIFOLD_MODULE_ID: &str = "forgecad.module.manifold-boolean@1";
pub const CPU_SUBDIVISION_MODULE_ID: &str = "forgecad.module.cpu-subdivision@1";
pub const OPENSUBDIV_MODULE_ID: &str = "forgecad.module.opensubdiv-compatible@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleAvailability {
    Active,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeCadModuleCapabilities {
    pub network: bool,
    pub dynamic_plugin: bool,
    pub script: bool,
    pub direct_db_write: bool,
    pub direct_cas_write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeCadModuleDescriptor {
    pub schema_version: String,
    pub module_id: String,
    pub module_version: String,
    pub availability: ModuleAvailability,
    pub backend: String,
    pub source_revision: String,
    pub license: String,
    pub license_status: String,
    pub operator_ids: Vec<String>,
    pub capabilities: ForgeCadModuleCapabilities,
    pub actual_third_party_link: bool,
    pub unavailable_reason: Option<String>,
    pub module_sha256: String,
}

/// Minimal trait shared by fixed evaluators.  It is deliberately narrower
/// than a plugin ABI: only a signed/static descriptor crosses the seam.
pub trait ForgeCadModule {
    fn descriptor(&self) -> ForgeCadModuleDescriptor;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ManifoldBooleanModule;

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuSubdivisionModule;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenSubdivCompatibleModule;

impl ForgeCadModule for ManifoldBooleanModule {
    fn descriptor(&self) -> ForgeCadModuleDescriptor {
        manifold_descriptor()
    }
}

impl ForgeCadModule for CpuSubdivisionModule {
    fn descriptor(&self) -> ForgeCadModuleDescriptor {
        cpu_subdivision_descriptor()
    }
}

impl ForgeCadModule for OpenSubdivCompatibleModule {
    fn descriptor(&self) -> ForgeCadModuleDescriptor {
        opensubdiv_descriptor()
    }
}

pub fn module_descriptors() -> Vec<ForgeCadModuleDescriptor> {
    vec![
        ManifoldBooleanModule.descriptor(),
        CpuSubdivisionModule.descriptor(),
        OpenSubdivCompatibleModule.descriptor(),
    ]
}

pub fn descriptor_for(module_id: &str) -> Option<ForgeCadModuleDescriptor> {
    module_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.module_id == module_id)
}

fn common_capabilities() -> ForgeCadModuleCapabilities {
    ForgeCadModuleCapabilities {
        network: false,
        dynamic_plugin: false,
        script: false,
        direct_db_write: false,
        direct_cas_write: false,
    }
}

fn manifold_descriptor() -> ForgeCadModuleDescriptor {
    finalize(ForgeCadModuleDescriptor {
        schema_version: MODULE_SCHEMA_VERSION.to_owned(),
        module_id: MANIFOLD_MODULE_ID.to_owned(),
        module_version: "1.0.0".to_owned(),
        availability: if cfg!(feature = "manifold-backend") {
            ModuleAvailability::Active
        } else {
            ModuleAvailability::Unavailable
        },
        backend: "manifold-c-api-ffi-static@1".to_owned(),
        source_revision: "969b1417afdee87dbc6147cf676bc04799418ec2".to_owned(),
        license: "Apache-2.0".to_owned(),
        license_status: "accepted-product-isolated-worker".to_owned(),
        operator_ids: vec!["forgecad.module.boolean@1".to_owned()],
        capabilities: common_capabilities(),
        actual_third_party_link: cfg!(feature = "manifold-backend"),
        unavailable_reason: (!cfg!(feature = "manifold-backend"))
            .then(|| "MANIFOLD_BACKEND_FEATURE_DISABLED".to_owned()),
        module_sha256: String::new(),
    })
}

fn cpu_subdivision_descriptor() -> ForgeCadModuleDescriptor {
    finalize(ForgeCadModuleDescriptor {
        schema_version: MODULE_SCHEMA_VERSION.to_owned(),
        module_id: CPU_SUBDIVISION_MODULE_ID.to_owned(),
        module_version: "1.0.0".to_owned(),
        availability: ModuleAvailability::Active,
        backend: "forgecad-owned-cpu-catmull-clark-regular-quad@1".to_owned(),
        source_revision: "forgecad-source".to_owned(),
        license: "ForgeCAD-owned".to_owned(),
        license_status: "first-party".to_owned(),
        operator_ids: vec!["forgecad.module.subdivision@1".to_owned()],
        capabilities: common_capabilities(),
        actual_third_party_link: false,
        unavailable_reason: None,
        module_sha256: String::new(),
    })
}

fn opensubdiv_descriptor() -> ForgeCadModuleDescriptor {
    finalize(ForgeCadModuleDescriptor {
        schema_version: MODULE_SCHEMA_VERSION.to_owned(),
        module_id: OPENSUBDIV_MODULE_ID.to_owned(),
        module_version: "1.0.0".to_owned(),
        availability: ModuleAvailability::Unavailable,
        backend: "opensubdiv-compatible-closed-typed-subset@1".to_owned(),
        source_revision: "4951f30c00f395aa831a9fc42577cc28ce46fa81".to_owned(),
        license: "Tomorrow Open Source Technology License 1.0".to_owned(),
        license_status: "research-authorized-not-adopted".to_owned(),
        operator_ids: vec!["forgecad.module.subdivision@opensubdiv-compatible@1".to_owned()],
        capabilities: common_capabilities(),
        actual_third_party_link: false,
        unavailable_reason: Some("OPENSUBDIV_NOT_VENDORED_OR_LINKED".to_owned()),
        module_sha256: String::new(),
    })
}

fn finalize(mut descriptor: ForgeCadModuleDescriptor) -> ForgeCadModuleDescriptor {
    let mut preimage = serde_json::to_value(&descriptor).expect("module descriptor serializes");
    preimage["module_sha256"] = serde_json::Value::String(String::new());
    let bytes = serde_json::to_vec(&preimage).expect("module descriptor preimage serializes");
    let digest = Sha256::digest(bytes);
    descriptor.module_sha256 = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    descriptor
}
