//! First-stage domain service boundaries for the Weaponry knife profile.
//!
//! This module is deliberately an organizational seam, not a second Runtime.
//! Every service is a borrow-only view over [`Runtime`].  The private Store is
//! never copied into a service and all writes continue to go through the
//! existing Runtime methods and Store transaction boundary.
//!
//! The current knife profile has eleven public façades.  They are grouped into
//! five implementation-facing domains here while the original `Runtime`
//! methods and IPC operation names remain unchanged.  This keeps the first
//! migration small: callers can discover a bounded domain surface today, and
//! individual backing modules can move behind these seams in later slices.

#[path = "delivery_service.rs"]
pub(crate) mod delivery_service;
#[path = "evaluation_service.rs"]
pub(crate) mod evaluation_service;
#[path = "presentation_service.rs"]
pub(crate) mod presentation_service;
#[path = "surface_service.rs"]
pub(crate) mod surface_service;

use super::{Runtime, RuntimeError};
pub use forgecad_contracts::weaponry_domain_map::{
    KnifeFacadeBinding, WeaponryServiceDomain as RuntimeServiceDomain, KNIFE_FACADE_BINDINGS,
};
use serde_json::Value;

/// Access class for a route exposed through one service boundary.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RuntimeServiceOperationAccess {
    Read,
    Write,
}

/// Immutable route metadata for one domain service.
///
/// `read_operations` and `write_operations` are the current knife-profile
/// ownership projection.  Every profile/native operation appears in exactly
/// one boundary; a write route additionally appears in exactly one write
/// owner.  The unit tests below make both invariants executable.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RuntimeServiceBoundary {
    pub domain: RuntimeServiceDomain,
    pub facade_names: &'static [&'static str],
    pub read_operations: &'static [&'static str],
    pub write_operations: &'static [&'static str],
}

impl RuntimeServiceBoundary {
    pub const fn name(self) -> &'static str {
        self.domain.as_str()
    }

    pub fn supports(&self, operation: &str) -> Option<RuntimeServiceOperationAccess> {
        if self.write_operations.contains(&operation) {
            Some(RuntimeServiceOperationAccess::Write)
        } else if self.read_operations.contains(&operation) {
            Some(RuntimeServiceOperationAccess::Read)
        } else {
            None
        }
    }
}

const AUTHORING_FACADES: &[&str] =
    forgecad_contracts::weaponry_domain_map::knife_facades_for_domain(
        RuntimeServiceDomain::Authoring,
    );
const EVALUATION_FACADES: &[&str] =
    forgecad_contracts::weaponry_domain_map::knife_facades_for_domain(
        RuntimeServiceDomain::Evaluation,
    );
const SURFACE_FACADES: &[&str] = forgecad_contracts::weaponry_domain_map::knife_facades_for_domain(
    RuntimeServiceDomain::Surface,
);
const PRESENTATION_FACADES: &[&str] =
    forgecad_contracts::weaponry_domain_map::knife_facades_for_domain(
        RuntimeServiceDomain::Presentation,
    );
const DELIVERY_FACADES: &[&str] = forgecad_contracts::weaponry_domain_map::knife_facades_for_domain(
    RuntimeServiceDomain::Delivery,
);

const AUTHORING_READ_OPERATIONS: &[&str] = &[
    "capabilities_get",
    "project_get",
    "project_list",
    "runtime_status",
    "skill_get",
    "reference_get",
    "checkpoint_get",
    "session_get",
    "version_list",
    "authoring_mesh_identity_lineage_get",
    "authoring_topology_get",
    "geometry_program_hash",
    "authoring_mesh_v2_high_bridge_get",
    "knife_curve_modifier_graph_get",
    "knife_curve_evaluated_mesh_get",
];

const AUTHORING_WRITE_OPERATIONS: &[&str] = &[
    "project_create",
    "reference_import",
    "reference_mask_prepare",
    "reference_mask_refine_prepare",
    "authoring_mesh_durable_prepare",
    "authoring_mesh_edit_prepare",
    "authoring_mesh_identity_lineage_prepare",
    "authoring_mesh_transaction_prepare",
    "authoring_mesh_v2_candidate_materialize",
    "authoring_mesh_v2_durable_prepare",
    "authoring_mesh_v2_high_bridge_prepare",
    "change_prepare",
    "design_action_run_prepare",
    "geometry_prepare",
    "checkpoint_prepare",
    "checkpoint_restore_prepare",
    "repair_apply_confirm",
    "repair_apply_prepare",
    "repair_intent_run_prepare",
    "restore_confirm",
    "restore_prepare",
    "session_create_or_resume",
    "knife_curve_modifier_graph_prepare",
    "knife_curve_evaluated_mesh_prepare",
];

const EVALUATION_READ_OPERATIONS: &[&str] = evaluation_service::EVALUATION_READ_OPERATIONS;

const EVALUATION_WRITE_OPERATIONS: &[&str] = evaluation_service::EVALUATION_WRITE_OPERATIONS;

const SURFACE_READ_OPERATIONS: &[&str] = &[
    "appearance_source_lineage_get",
    "authoring_mesh_v2_high_artifact_get",
    "hero_uv_durable_get",
    "low_quad_draft_durable_get",
    "production_weapon_form_quality_v2_preflight_get",
    "production_weapon_formal_high_get",
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_preflight_get",
    "production_weapon_retopology_cage_source_get",
];

const SURFACE_WRITE_OPERATIONS: &[&str] = &[
    "appearance_prepare",
    "appearance_source_lineage_prepare",
    "authoring_mesh_v2_high_artifact_prepare",
    "hero_uv_durable_prepare",
    "low_quad_draft_durable_prepare",
    "production_weapon_formal_high_prepare",
    "production_weapon_high_low_bake_prepare",
    "production_weapon_retopology_cage_source_prepare",
];

const DELIVERY_READ_OPERATIONS: &[&str] = delivery_service::DELIVERY_READ_OPERATIONS;

const DELIVERY_WRITE_OPERATIONS: &[&str] = delivery_service::DELIVERY_WRITE_OPERATIONS;

/// The five domain boundaries.  This is intentionally a static projection;
/// the JSON knife profile and existing Runtime method set remain compatible
/// sources of truth until a later migration physically moves implementation.
pub const KNIFE_SERVICE_BOUNDARIES: [RuntimeServiceBoundary; 5] = [
    RuntimeServiceBoundary {
        domain: RuntimeServiceDomain::Authoring,
        facade_names: AUTHORING_FACADES,
        read_operations: AUTHORING_READ_OPERATIONS,
        write_operations: AUTHORING_WRITE_OPERATIONS,
    },
    RuntimeServiceBoundary {
        domain: RuntimeServiceDomain::Evaluation,
        facade_names: EVALUATION_FACADES,
        read_operations: EVALUATION_READ_OPERATIONS,
        write_operations: EVALUATION_WRITE_OPERATIONS,
    },
    RuntimeServiceBoundary {
        domain: RuntimeServiceDomain::Surface,
        facade_names: SURFACE_FACADES,
        read_operations: SURFACE_READ_OPERATIONS,
        write_operations: SURFACE_WRITE_OPERATIONS,
    },
    RuntimeServiceBoundary {
        domain: RuntimeServiceDomain::Presentation,
        facade_names: PRESENTATION_FACADES,
        read_operations: presentation_service::PRESENTATION_READ_OPERATIONS,
        write_operations: presentation_service::PRESENTATION_WRITE_OPERATIONS,
    },
    RuntimeServiceBoundary {
        domain: RuntimeServiceDomain::Delivery,
        facade_names: DELIVERY_FACADES,
        read_operations: DELIVERY_READ_OPERATIONS,
        write_operations: DELIVERY_WRITE_OPERATIONS,
    },
];

/// A borrow-only access façade over the existing Runtime.
pub trait RuntimeService {
    fn boundary(&self) -> &'static RuntimeServiceBoundary;

    /// Invoke a profile operation after enforcing this domain's route
    /// boundary.  Each domain service keeps the existing Runtime method and
    /// Store/CAS transaction as the single writer; no service owns a second
    /// state store.
    fn invoke(&self, operation: &str, payload: &Value) -> Result<Value, RuntimeError>;
}

macro_rules! define_runtime_service {
    ($name:ident, $domain:ident, $accessor:expr) => {
        /// Borrow-only service view for one Runtime domain.
        pub struct $name<'runtime> {
            runtime: &'runtime Runtime,
        }

        impl<'runtime> $name<'runtime> {
            pub const fn domain(&self) -> RuntimeServiceDomain {
                RuntimeServiceDomain::$domain
            }

            pub const fn boundary(&self) -> &'static RuntimeServiceBoundary {
                &KNIFE_SERVICE_BOUNDARIES[$accessor]
            }

            pub fn invoke(&self, operation: &str, payload: &Value) -> Result<Value, RuntimeError> {
                <Self as RuntimeService>::invoke(self, operation, payload)
            }
        }

        impl RuntimeService for $name<'_> {
            fn boundary(&self) -> &'static RuntimeServiceBoundary {
                Self::boundary(self)
            }

            fn invoke(&self, operation: &str, payload: &Value) -> Result<Value, RuntimeError> {
                let boundary = self.boundary();
                if boundary.supports(operation).is_none() {
                    return Err(RuntimeError::InvalidInput(format!(
                        "RUNTIME_SERVICE_OPERATION_OUT_OF_BOUND: {} does not own {operation}",
                        boundary.name()
                    )));
                }
                match boundary.domain {
                    RuntimeServiceDomain::Authoring => {
                        crate::authoring_service::invoke(self.runtime, operation, payload)
                    }
                    RuntimeServiceDomain::Evaluation => {
                        evaluation_service::invoke(self.runtime, operation, payload)
                    }
                    RuntimeServiceDomain::Surface => {
                        surface_service::invoke(self.runtime, operation, payload)
                    }
                    RuntimeServiceDomain::Presentation => {
                        presentation_service::invoke(self.runtime, operation, payload)
                    }
                    RuntimeServiceDomain::Delivery => {
                        delivery_service::invoke(self.runtime, operation, payload)
                    }
                }
            }
        }
    };
}

define_runtime_service!(AuthoringService, Authoring, 0);
define_runtime_service!(EvaluationService, Evaluation, 1);
define_runtime_service!(SurfaceService, Surface, 2);
define_runtime_service!(PresentationService, Presentation, 3);
define_runtime_service!(DeliveryService, Delivery, 4);

impl Runtime {
    /// Discover the five current Weaponry service boundaries.
    pub const fn knife_service_boundaries() -> &'static [RuntimeServiceBoundary; 5] {
        &KNIFE_SERVICE_BOUNDARIES
    }

    /// Discover the eleven knife-profile façade-to-domain bindings.
    pub const fn knife_facade_bindings() -> &'static [KnifeFacadeBinding; 11] {
        &KNIFE_FACADE_BINDINGS
    }

    pub fn authoring_service(&self) -> AuthoringService<'_> {
        AuthoringService { runtime: self }
    }

    pub fn evaluation_service(&self) -> EvaluationService<'_> {
        EvaluationService { runtime: self }
    }

    pub fn surface_service(&self) -> SurfaceService<'_> {
        SurfaceService { runtime: self }
    }

    pub fn presentation_service(&self) -> PresentationService<'_> {
        PresentationService { runtime: self }
    }

    pub fn delivery_service(&self) -> DeliveryService<'_> {
        DeliveryService { runtime: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::mem::size_of;

    #[test]
    fn knife_profile_facades_are_discoverable_in_five_domains() {
        let boundaries = Runtime::knife_service_boundaries();
        assert_eq!(boundaries.len(), 5);
        assert_eq!(
            boundaries
                .iter()
                .map(|boundary| boundary.name())
                .collect::<Vec<_>>(),
            vec![
                "authoring",
                "evaluation",
                "surface",
                "presentation",
                "delivery"
            ]
        );

        let facades = Runtime::knife_facade_bindings();
        assert_eq!(facades.len(), 11);
        let names = facades
            .iter()
            .map(|binding| binding.facade_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 11);
        assert_eq!(
            names,
            [
                "approval",
                "authoring_transaction",
                "delivery",
                "fps_presentation",
                "job",
                "observe",
                "quality_review",
                "recovery",
                "reference_intake",
                "surface_pipeline",
                "weapon_preflight",
            ]
            .into_iter()
            .collect()
        );

        for binding in facades {
            let boundary = boundaries
                .iter()
                .find(|boundary| boundary.domain == binding.domain)
                .expect("every façade has a domain");
            assert!(boundary.facade_names.contains(&binding.facade_name));
        }
    }

    #[test]
    fn write_routes_have_exactly_one_runtime_service_owner() {
        let boundaries = Runtime::knife_service_boundaries();
        let mut all_operations = BTreeSet::new();
        let mut all_writes = BTreeSet::new();
        for boundary in boundaries {
            for operation in boundary.read_operations {
                assert!(
                    all_operations.insert(*operation),
                    "profile operation has more than one domain owner: {operation}"
                );
            }
            for operation in boundary.write_operations {
                assert!(
                    !boundary.read_operations.contains(operation),
                    "route cannot be read and written by one boundary: {operation}"
                );
                assert!(
                    all_writes.insert(*operation),
                    "write route has more than one domain owner: {operation}"
                );
                assert!(
                    all_operations.insert(*operation),
                    "profile operation has more than one domain owner: {operation}"
                );
            }
        }
        // `doctor` is the sole profile operation intentionally owned by the
        // MCP adapter rather than a Runtime service boundary.
        // The active Runtime inventory includes the native KnifePassState
        // prepare/get pair in addition to the legacy-backed routes. `doctor`
        // remains MCP-local and is intentionally absent here.
        assert_eq!(all_operations.len(), 131);
        assert_eq!(all_writes.len(), 60);
    }

    #[test]
    fn service_handles_are_borrow_only_runtime_views() {
        assert_eq!(
            size_of::<AuthoringService<'static>>(),
            size_of::<&Runtime>()
        );
        assert_eq!(
            size_of::<EvaluationService<'static>>(),
            size_of::<&Runtime>()
        );
        assert_eq!(size_of::<SurfaceService<'static>>(), size_of::<&Runtime>());
        assert_eq!(
            size_of::<PresentationService<'static>>(),
            size_of::<&Runtime>()
        );
        assert_eq!(size_of::<DeliveryService<'static>>(), size_of::<&Runtime>());
    }

    #[test]
    fn service_rejects_a_write_owned_by_another_domain_before_dispatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .delivery_service()
            .invoke("authoring_mesh_transaction_prepare", &Value::Null)
            .expect_err("delivery cannot invoke authoring write route");
        assert!(error
            .to_string()
            .contains("RUNTIME_SERVICE_OPERATION_OUT_OF_BOUND"));
    }

    #[test]
    fn knife_native_operations_are_authoring_owned_and_fail_closed_elsewhere() {
        let boundaries = Runtime::knife_service_boundaries();
        let authoring = boundaries
            .iter()
            .find(|boundary| boundary.domain == RuntimeServiceDomain::Authoring)
            .expect("authoring boundary");
        assert_eq!(
            authoring.supports("knife_curve_modifier_graph_prepare"),
            Some(RuntimeServiceOperationAccess::Write)
        );
        assert_eq!(
            authoring.supports("knife_curve_evaluated_mesh_prepare"),
            Some(RuntimeServiceOperationAccess::Write)
        );
        assert_eq!(
            authoring.supports("knife_curve_modifier_graph_get"),
            Some(RuntimeServiceOperationAccess::Read)
        );
        assert_eq!(
            authoring.supports("knife_curve_evaluated_mesh_get"),
            Some(RuntimeServiceOperationAccess::Read)
        );

        for boundary in boundaries {
            if boundary.domain != RuntimeServiceDomain::Authoring {
                assert_eq!(
                    boundary.supports("knife_curve_modifier_graph_prepare"),
                    None
                );
                assert_eq!(
                    boundary.supports("knife_curve_evaluated_mesh_prepare"),
                    None
                );
            }
        }
    }

    #[test]
    fn delivery_compatibility_bridge_reuses_the_direct_typed_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let delivery = &KNIFE_SERVICE_BOUNDARIES[4];
        let mut operations = BTreeSet::new();
        operations.extend(delivery.read_operations.iter().copied());
        operations.extend(delivery.write_operations.iter().copied());

        for operation in operations {
            let direct = runtime
                .delivery_service()
                .invoke(operation, &Value::Null)
                .expect_err("invalid direct Delivery request");
            let bridged = runtime
                .dispatch_ipc(operation, &Value::Null)
                .expect_err("invalid compatibility Delivery request");
            assert_eq!(direct.to_string(), bridged.to_string(), "{operation}");
        }
    }
}
