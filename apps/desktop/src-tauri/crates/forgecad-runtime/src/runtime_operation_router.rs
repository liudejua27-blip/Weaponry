//! Typed Runtime operation routing for the Weaponry knife profile.
//!
//! The checked-in Knife profile is the operation-to-façade source.  The
//! Contract domain map is the sole façade-to-domain source.  Keeping those
//! responsibilities separate prevents Runtime from growing another hand
//! maintained operation ownership table while still allowing old handlers to
//! remain behind a compatibility forwarding seam.

use super::{
    authoring_service,
    runtime_services::{
        delivery_service, evaluation_service, presentation_service, surface_service,
    },
    Runtime, RuntimeError,
};
use forgecad_contracts::weaponry_domain_map::{
    knife_facade_binding, knife_operation_execution_target, WeaponryOperationExecutionTarget,
    WeaponryServiceDomain, KNIFE_CAPABILITY_MAPPINGS,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RuntimeOperationAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RuntimeOperationRoute {
    pub operation: &'static str,
    pub facade_name: &'static str,
    pub domain: WeaponryServiceDomain,
    pub access: RuntimeOperationAccess,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOperationEnvelope {
    pub domain: WeaponryServiceDomain,
    pub operation: String,
    pub payload: Value,
}

impl RuntimeOperationEnvelope {
    pub fn new(
        domain: WeaponryServiceDomain,
        operation: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            domain,
            operation: operation.into(),
            payload,
        }
    }

    /// Encode the stable local-IPC envelope.  The authentication token and
    /// transport framing remain owned by `ipc.rs`; this value contains only a
    /// typed domain, operation name and the operation's already typed JSON
    /// payload.
    pub fn to_ipc_payload(&self) -> Value {
        json!({
            "domain": self.domain.as_str(),
            "operation": self.operation,
            "payload": self.payload,
        })
    }

    pub(crate) fn from_ipc_payload(value: &Value) -> Result<Self, RuntimeError> {
        let object = value.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "RUNTIME_OPERATION_ENVELOPE_INVALID: payload must be an object".to_owned(),
            )
        })?;
        let domain_value = object
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "RUNTIME_OPERATION_ENVELOPE_INVALID: domain is required".to_owned(),
                )
            })?;
        let domain = parse_domain(domain_value).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "RUNTIME_OPERATION_ENVELOPE_INVALID: unsupported domain {domain_value}"
            ))
        })?;
        let operation = object
            .get("operation")
            .and_then(Value::as_str)
            .filter(|operation| !operation.is_empty())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "RUNTIME_OPERATION_ENVELOPE_INVALID: operation is required".to_owned(),
                )
            })?;
        let payload = object.get("payload").cloned().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "RUNTIME_OPERATION_ENVELOPE_INVALID: payload is required".to_owned(),
            )
        })?;
        Ok(Self::new(domain, operation, payload))
    }
}

pub fn parse_domain(value: &str) -> Option<WeaponryServiceDomain> {
    WeaponryServiceDomain::all()
        .into_iter()
        .find(|domain| domain.as_str() == value)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GeneratedOperationAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct GeneratedOperationRoute {
    pub(crate) operation: &'static str,
    pub(crate) facade: &'static str,
    pub(crate) access: GeneratedOperationAccess,
}

include!(concat!(env!("OUT_DIR"), "/weaponry_operation_routes.rs"));

/// A borrow-only router over the Runtime's existing single-writer state.
pub struct RuntimeOperationRouter<'runtime> {
    runtime: &'runtime Runtime,
}

impl<'runtime> RuntimeOperationRouter<'runtime> {
    pub const fn new(runtime: &'runtime Runtime) -> Self {
        Self { runtime }
    }

    pub const fn runtime(&self) -> &'runtime Runtime {
        self.runtime
    }

    /// Resolve one Knife-profile operation through the generated
    /// operation→façade directory and the Contract façade→domain map.
    ///
    /// A small set of newly extracted Runtime capabilities is intentionally
    /// not in the public Knife profile yet.  Those operations are admitted
    /// only through `KNIFE_CAPABILITY_MAPPINGS`, which remains the Contract
    /// authority for their owner until a successor profile includes them.
    pub fn route(&self, operation: &str) -> Option<RuntimeOperationRoute> {
        // Capability-level mappings are the ownership authority for extracted
        // operations. Their façade and domain are required to agree with the
        // generated profile route by exhaustive tests below.
        if let Some(mapping) = KNIFE_CAPABILITY_MAPPINGS.iter().find(|mapping| {
            mapping
                .mcp_operations
                .iter()
                .any(|candidate| *candidate == operation)
        }) {
            let operation = mapping
                .mcp_operations
                .iter()
                .find(|candidate| **candidate == operation)
                .copied()?;
            return Some(RuntimeOperationRoute {
                operation,
                facade_name: mapping.mcp_facade.unwrap_or(mapping.domain.as_str()),
                domain: mapping.domain,
                access: if mapping.persistence
                    == forgecad_contracts::weaponry_domain_map::PersistenceKind::None
                {
                    RuntimeOperationAccess::Read
                } else {
                    capability_operation_access(operation)
                },
            });
        }

        if let Some(generated) = KNIFE_OPERATION_ROUTES
            .iter()
            .find(|route| route.operation == operation)
        {
            let binding = knife_facade_binding(generated.facade)?;
            let access = match generated.access {
                GeneratedOperationAccess::Read => RuntimeOperationAccess::Read,
                GeneratedOperationAccess::Write => RuntimeOperationAccess::Write,
            };
            return Some(RuntimeOperationRoute {
                operation: generated.operation,
                facade_name: generated.facade,
                domain: binding.domain,
                access,
            });
        }
        None
    }

    pub fn owner(&self, operation: &str) -> Option<WeaponryServiceDomain> {
        self.route(operation).map(|route| route.domain)
    }

    pub fn invoke(
        &self,
        domain: WeaponryServiceDomain,
        operation: &str,
        payload: &Value,
    ) -> Result<Value, RuntimeError> {
        // MCP-local control-plane operations are declared by the central
        // Contract map. Runtime must never manufacture or proxy their values.
        if knife_operation_execution_target(operation)
            == WeaponryOperationExecutionTarget::McpAdapter
        {
            return Err(RuntimeError::InvalidInput(
                format!(
                    "RUNTIME_OPERATION_TARGET_MISMATCH: operation {operation} is owned by the MCP adapter"
                ),
            ));
        }
        let route = self.route(operation).ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "RUNTIME_OPERATION_UNKNOWN: operation {operation} is not in the Knife domain directory"
            ))
        })?;
        if route.domain != domain {
            return Err(RuntimeError::InvalidInput(format!(
                "RUNTIME_OPERATION_DOMAIN_MISMATCH: operation {operation} belongs to {} but envelope selected {}",
                route.domain.as_str(),
                domain.as_str()
            )));
        }

        match route.domain {
            WeaponryServiceDomain::Authoring => {
                authoring_service::invoke(self.runtime, operation, payload)
            }
            WeaponryServiceDomain::Surface => {
                surface_service::invoke(self.runtime, operation, payload)
            }
            WeaponryServiceDomain::Evaluation => {
                evaluation_service::invoke(self.runtime, operation, payload)
            }
            WeaponryServiceDomain::Presentation => {
                presentation_service::invoke(self.runtime, operation, payload)
            }
            WeaponryServiceDomain::Delivery => {
                delivery_service::invoke(self.runtime, operation, payload)
            }
        }
    }

    pub fn invoke_envelope(
        &self,
        envelope: &RuntimeOperationEnvelope,
    ) -> Result<Value, RuntimeError> {
        self.invoke(envelope.domain, &envelope.operation, &envelope.payload)
    }

    pub fn operation_count(&self) -> usize {
        KNIFE_OPERATION_ROUTE_COUNT
            + KNIFE_CAPABILITY_MAPPINGS
                .iter()
                .flat_map(|mapping| mapping.mcp_operations.iter().copied())
                .filter(|operation| {
                    !KNIFE_OPERATION_ROUTES
                        .iter()
                        .any(|route| route.operation == *operation)
                })
                .count()
    }
}

fn capability_operation_access(operation: &str) -> RuntimeOperationAccess {
    if operation == "authoring_mesh_v2_candidate_materialize"
        || operation.ends_with("_prepare")
        || operation.ends_with("_confirm")
        || operation.ends_with("_reject")
        || operation.ends_with("_import")
        || operation.ends_with("_apply")
        || operation.ends_with("_run")
    {
        RuntimeOperationAccess::Write
    } else {
        RuntimeOperationAccess::Read
    }
}

impl Runtime {
    pub fn weaponry_operation_router(&self) -> RuntimeOperationRouter<'_> {
        RuntimeOperationRouter::new(self)
    }

    /// Invoke a typed Weaponry operation.  The supplied Contract domain must
    /// equal the sole owner in `forgecad_contracts::weaponry_domain_map`; a
    /// mismatch or an operation outside the directory is rejected before any
    /// handler, Store transaction or CAS write can run.
    pub fn invoke_weaponry_operation(
        &self,
        domain: WeaponryServiceDomain,
        operation: &str,
        payload: &Value,
    ) -> Result<Value, RuntimeError> {
        self.weaponry_operation_router()
            .invoke(domain, operation, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn generated_profile_routes_are_unique_and_resolve_through_contract_domains() {
        assert_eq!(KNIFE_OPERATION_ROUTE_COUNT, 139);
        let runtime = Runtime::ephemeral().expect("runtime");
        let router = runtime.weaponry_operation_router();
        let mut operations = BTreeMap::new();
        for route in KNIFE_OPERATION_ROUTES {
            assert!(
                operations.insert(route.operation, route.facade).is_none(),
                "duplicate generated operation {}",
                route.operation
            );
            let resolved = router.route(route.operation).expect("resolved route");
            assert_eq!(resolved.facade_name, route.facade);
            let binding =
                knife_facade_binding(resolved.facade_name).expect("Contract façade binding");
            assert_eq!(resolved.domain, binding.domain);
        }
        assert_eq!(operations.len(), 139);
        for (operation, access) in [
            (
                "weaponry_knife_production_brief_get",
                RuntimeOperationAccess::Read,
            ),
            (
                "weaponry_knife_production_brief_prepare",
                RuntimeOperationAccess::Write,
            ),
            ("knife_source_binding_get", RuntimeOperationAccess::Read),
            (
                "knife_source_binding_prepare",
                RuntimeOperationAccess::Write,
            ),
            (
                "production_weapon_authoring_mesh_v2_source_prepare",
                RuntimeOperationAccess::Write,
            ),
            ("knife_pass_state_get", RuntimeOperationAccess::Read),
            ("knife_pass_state_prepare", RuntimeOperationAccess::Write),
        ] {
            assert_eq!(
                router.route(operation).expect("new Knife route").access,
                access,
                "{operation} access classification drifted"
            );
        }
        assert_eq!(
            router
                .route("knife_curve_modifier_graph_prepare")
                .expect("native prepare")
                .access,
            RuntimeOperationAccess::Write
        );
        assert_eq!(
            router
                .route("knife_curve_modifier_graph_get")
                .expect("native get")
                .access,
            RuntimeOperationAccess::Read
        );
        assert_eq!(
            router
                .route("knife_curve_evaluated_mesh_prepare")
                .expect("evaluated mesh prepare")
                .access,
            RuntimeOperationAccess::Write
        );
        assert_eq!(
            router
                .route("knife_curve_evaluated_mesh_get")
                .expect("evaluated mesh get")
                .access,
            RuntimeOperationAccess::Read
        );
        assert_eq!(
            router
                .route("authoring_mesh_v2_candidate_materialize")
                .expect("AuthoringMeshV2 candidate materializer")
                .access,
            RuntimeOperationAccess::Write
        );
    }

    #[test]
    fn capability_operations_keep_contract_domain_ownership() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let router = runtime.weaponry_operation_router();
        for mapping in KNIFE_CAPABILITY_MAPPINGS {
            for operation in mapping.mcp_operations {
                assert_eq!(router.owner(operation), Some(mapping.domain));
            }
        }
        // KnifePassState and the V2 High bridge remain central-map-only seams;
        // the direct V2 High artifact is now part of the generated profile.
        assert_eq!(router.operation_count(), 143);
    }

    #[test]
    fn wrong_domain_and_unknown_operations_fail_before_dispatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let wrong = runtime.invoke_weaponry_operation(
            WeaponryServiceDomain::Delivery,
            "authoring_mesh_transaction_prepare",
            &Value::Null,
        );
        assert!(wrong
            .expect_err("cross-domain route must fail closed")
            .to_string()
            .starts_with("invalid runtime input: RUNTIME_OPERATION_DOMAIN_MISMATCH:"));

        let unknown = runtime.invoke_weaponry_operation(
            WeaponryServiceDomain::Authoring,
            "not_a_weaponry_operation",
            &Value::Null,
        );
        assert!(unknown
            .expect_err("unknown route must fail closed")
            .to_string()
            .starts_with("invalid runtime input: RUNTIME_OPERATION_UNKNOWN:"));

        let local_status = runtime
            .invoke_weaponry_operation(
                WeaponryServiceDomain::Authoring,
                "runtime_status",
                &Value::Null,
            )
            .expect_err("MCP-local status must not be synthesized by Runtime");
        assert!(local_status
            .to_string()
            .starts_with("invalid runtime input: RUNTIME_OPERATION_TARGET_MISMATCH:"));
    }

    #[test]
    fn surface_routes_enter_the_typed_surface_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                WeaponryServiceDomain::Surface,
                "appearance_prepare",
                &Value::Null,
            )
            .expect_err("invalid Surface payload");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: project_id is required"
        );
    }

    #[test]
    fn presentation_routes_enter_the_typed_presentation_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                WeaponryServiceDomain::Presentation,
                "fps_presentation_package_v2_get",
                &Value::Null,
            )
            .expect_err("invalid Presentation payload");
        assert!(error
            .to_string()
            .contains("FPS_PRESENTATION_PACKAGE_V2_REJECTED"));
    }

    #[test]
    fn evaluation_routes_enter_the_typed_evaluation_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                WeaponryServiceDomain::Evaluation,
                "quality_get",
                &Value::Null,
            )
            .expect_err("invalid Evaluation payload");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: candidate_id is required"
        );
    }

    #[test]
    fn observe_authoring_readbacks_do_not_cross_the_authoring_command_domain() {
        let runtime = Runtime::ephemeral().expect("runtime");
        for operation in [
            "authoring_mesh_transaction_get",
            "authoring_mesh_v2_durable_get",
        ] {
            let route = runtime
                .weaponry_operation_router()
                .route(operation)
                .expect("readback route");
            assert_eq!(route.facade_name, "observe");
            assert_eq!(route.domain, WeaponryServiceDomain::Evaluation);
            let error = runtime
                .invoke_weaponry_operation(
                    WeaponryServiceDomain::Evaluation,
                    operation,
                    &Value::Null,
                )
                .expect_err("empty readback request");
            assert!(
                !error
                    .to_string()
                    .contains("RUNTIME_OPERATION_DOMAIN_MISMATCH"),
                "{operation} crossed into the Authoring command domain: {error}"
            );
        }
    }

    #[test]
    fn envelope_wire_shape_is_stable_and_domains_are_typed() {
        let envelope = RuntimeOperationEnvelope::new(
            WeaponryServiceDomain::Evaluation,
            "authoring_mesh_transaction_get",
            json!({"transaction_id":"tx-1"}),
        );
        assert_eq!(
            envelope.to_ipc_payload(),
            json!({
                "domain":"evaluation",
                "operation":"authoring_mesh_transaction_get",
                "payload":{"transaction_id":"tx-1"}
            })
        );
        let decoded = RuntimeOperationEnvelope::from_ipc_payload(&envelope.to_ipc_payload())
            .expect("wire envelope");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn envelope_rejects_unknown_domain_and_missing_payload() {
        let unknown = RuntimeOperationEnvelope::from_ipc_payload(&json!({
            "domain":"unknown",
            "operation":"candidate_get",
            "payload":{}
        }))
        .expect_err("unknown domain");
        assert!(unknown
            .to_string()
            .starts_with("invalid runtime input: RUNTIME_OPERATION_ENVELOPE_INVALID:"));

        let missing = RuntimeOperationEnvelope::from_ipc_payload(&json!({
            "domain":"evaluation",
            "operation":"candidate_get"
        }))
        .expect_err("missing payload");
        assert!(missing
            .to_string()
            .starts_with("invalid runtime input: RUNTIME_OPERATION_ENVELOPE_INVALID:"));
    }
}
