//! Default Weaponry MCP domain router.
//!
//! The knife façade is the only place where Codex selects a public workflow
//! entry point.  This adapter resolves that façade through the central
//! `forgecad-contracts` ownership map and forwards a typed domain envelope to
//! Runtime.  It intentionally contains no operation-to-domain table: the
//! profile validates the façade/operation pair and Runtime remains the
//! authority for the operation implementation.

use forgecad_contracts::weaponry_domain_map::{
    knife_facade_binding, knife_operation_execution_target, WeaponryOperationExecutionTarget,
    WeaponryServiceDomain, KNIFE_CAPABILITY_MAPPINGS,
};
use serde_json::{json, Value};

/// A validated default-profile route.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ResolvedRoute<'operation> {
    pub(crate) facade_name: &'static str,
    pub(crate) domain: WeaponryServiceDomain,
    pub(crate) operation: &'operation str,
    pub(crate) execution_target: WeaponryOperationExecutionTarget,
}

/// Resolve one Knife façade call without duplicating the Contract mapping.
///
/// `unwrap_facade_call` is the profile's closed façade/operation validator.
/// The Contract map supplies the sole façade/domain binding.  Runtime then
/// validates that the operation is implemented by the selected typed domain;
/// unknown and cross-façade calls fail here before transport or legacy root
/// dispatch is reached.
pub(crate) fn resolve<'operation>(
    facade_name: &str,
    operation: &'operation str,
) -> Result<ResolvedRoute<'operation>, String> {
    let binding = knife_facade_binding(facade_name).ok_or_else(|| {
        format!(
            "WEAPONRY_DOMAIN_ROUTE_UNKNOWN_FACADE: {facade_name} is not in the Contract knife façade map"
        )
    })?;
    if operation.is_empty() {
        return Err(
            "WEAPONRY_DOMAIN_ROUTE_UNKNOWN_OPERATION: operation must not be empty".to_owned(),
        );
    }

    // Reuse the closed profile validator so this adapter has no second
    // façade-to-operation allowlist.  The request body is not inspected by
    // this routing step; each operation adapter performs its own validation.
    crate::knife_tool_profile::unwrap_facade_call(
        crate::knife_tool_profile::ToolProfile::Knife,
        facade_name,
        &json!({"operation": operation, "request": {}}),
    )
    .map_err(|error| {
        if error.contains("ROUTE_DENIED") || error.contains("UNKNOWN") {
            format!("WEAPONRY_DOMAIN_ROUTE_CROSS_DOMAIN: {error}")
        } else {
            error
        }
    })?;

    // The capability directory is intentionally partial while physical
    // extraction is in progress. Where it names an operation, both its
    // façade and domain must agree with the validated profile route. Checking
    // only mappings that already name `facade_name` would miss a stale owner
    // and let MCP select one domain while Runtime selects another.
    if let Some(mapping) = KNIFE_CAPABILITY_MAPPINGS.iter().find(|mapping| {
        mapping
            .mcp_operations
            .iter()
            .any(|candidate| *candidate == operation)
    }) {
        if mapping.mcp_facade != Some(facade_name) || mapping.domain != binding.domain {
            return Err(format!(
                "WEAPONRY_DOMAIN_ROUTE_MAPPING_DRIFT: {facade_name}.{operation} conflicts with capability {} owned by {}.{}",
                mapping.capability,
                mapping.mcp_facade.unwrap_or("none"),
                mapping.domain.as_str()
            ));
        }
    }

    Ok(ResolvedRoute {
        facade_name: binding.facade_name,
        domain: binding.domain,
        operation,
        execution_target: knife_operation_execution_target(operation),
    })
}

/// Build the closed typed envelope used by the Runtime IPC router.
pub(crate) fn ipc_payload(route: ResolvedRoute<'_>, payload: &Value) -> Value {
    json!({
        "domain": route.domain.as_str(),
        "operation": route.operation,
        "payload": payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knife_tool_profile::{active_tools, ToolProfile};

    fn operations_for_tool(tool: &Value) -> Vec<String> {
        tool.pointer("/inputSchema/oneOf")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|branch| {
                branch
                    .pointer("/properties/operation/const")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect()
    }

    #[test]
    fn all_eleven_facades_resolve_through_the_contract_map() {
        let tools = active_tools().expect("active profile");
        assert_eq!(tools.len(), 11);
        for tool in tools {
            let facade = tool["name"].as_str().expect("facade name");
            let operations = operations_for_tool(&tool);
            assert!(!operations.is_empty(), "{facade} has no operations");
            for operation in operations {
                let route = resolve(facade, &operation).expect("closed façade route");
                assert_eq!(route.facade_name, facade);
                assert_eq!(route.operation, operation);
                assert!(route.domain.as_str().len() > 0);
            }
        }
    }

    #[test]
    fn unknown_operation_fails_closed_before_runtime() {
        let error = resolve("observe", "not_a_knife_operation").expect_err("unknown route");
        assert!(error.contains("WEAPONRY_DOMAIN_ROUTE_CROSS_DOMAIN"));
    }

    #[test]
    fn cross_facade_operation_fails_closed() {
        let error = resolve("delivery", "authoring_mesh_transaction_prepare")
            .expect_err("cross-domain route");
        assert!(error.contains("WEAPONRY_DOMAIN_ROUTE_CROSS_DOMAIN"));
    }

    #[test]
    fn ipc_payload_is_typed_and_does_not_mutate_operation_payload() {
        let route = resolve("weapon_preflight", "runtime_status").expect("route");
        let payload = json!({"request_id":"opaque"});
        assert_eq!(
            ipc_payload(route, &payload),
            json!({
                "domain":"authoring",
                "operation":"runtime_status",
                "payload":{"request_id":"opaque"}
            })
        );
        assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(
            route.execution_target,
            WeaponryOperationExecutionTarget::McpAdapter
        );
        assert_eq!(ToolProfile::Knife.id(), "weaponry-knife-p0-default@1");
    }

    #[test]
    fn execution_target_comes_from_the_contract_map() {
        assert_eq!(
            resolve("weapon_preflight", "runtime_status")
                .expect("MCP adapter route")
                .execution_target,
            WeaponryOperationExecutionTarget::McpAdapter
        );
        assert_eq!(
            resolve(
                "authoring_transaction",
                "authoring_mesh_transaction_prepare"
            )
            .expect("runtime route")
            .execution_target,
            WeaponryOperationExecutionTarget::Runtime
        );
    }

    #[test]
    fn observe_authoring_readbacks_resolve_to_the_evaluation_query_domain() {
        for operation in [
            "authoring_mesh_transaction_get",
            "authoring_mesh_v2_durable_get",
        ] {
            let route = resolve("observe", operation).expect("observe readback route");
            assert_eq!(route.facade_name, "observe");
            assert_eq!(route.domain, WeaponryServiceDomain::Evaluation);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }

    #[test]
    fn knife_production_brief_is_runtime_owned_reference_intake() {
        for operation in [
            "weaponry_knife_production_brief_get",
            "weaponry_knife_production_brief_prepare",
        ] {
            let route = resolve("reference_intake", operation).expect("brief route");
            assert_eq!(route.facade_name, "reference_intake");
            assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }

    #[test]
    fn knife_reference_intent_is_runtime_owned_reference_intake() {
        for operation in [
            "knife_reference_intent_bundle_get",
            "knife_reference_intent_bundle_prepare",
        ] {
            let route = resolve("reference_intake", operation).expect("intent route");
            assert_eq!(route.facade_name, "reference_intake");
            assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }

    #[test]
    fn knife_source_binding_is_runtime_owned_authoring_transaction() {
        for operation in ["knife_source_binding_get", "knife_source_binding_prepare"] {
            let route = resolve("authoring_transaction", operation).expect("source binding route");
            assert_eq!(route.facade_name, "authoring_transaction");
            assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }

    #[test]
    fn authoring_mesh_v2_source_prepare_is_runtime_owned_authoring_transaction() {
        let route = resolve(
            "authoring_transaction",
            "production_weapon_authoring_mesh_v2_source_prepare",
        )
        .expect("AuthoringMeshV2 source route");
        assert_eq!(route.facade_name, "authoring_transaction");
        assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(
            route.execution_target,
            WeaponryOperationExecutionTarget::Runtime
        );
    }

    #[test]
    fn authoring_mesh_v2_candidate_materialize_is_runtime_owned_authoring_transaction() {
        let route = resolve(
            "authoring_transaction",
            "authoring_mesh_v2_candidate_materialize",
        )
        .expect("AuthoringMeshV2 candidate materializer route");
        assert_eq!(route.facade_name, "authoring_transaction");
        assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(
            route.execution_target,
            WeaponryOperationExecutionTarget::Runtime
        );
    }

    #[test]
    fn authoring_mesh_v2_high_bridge_prepare_and_get_are_runtime_owned_authoring_transaction() {
        for operation in [
            "authoring_mesh_v2_high_bridge_prepare",
            "authoring_mesh_v2_high_bridge_get",
        ] {
            let route = resolve("authoring_transaction", operation)
                .expect("AuthoringMeshV2 High bridge route");
            assert_eq!(route.facade_name, "authoring_transaction");
            assert_eq!(route.domain, WeaponryServiceDomain::Authoring);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }

    #[test]
    fn authoring_mesh_v2_high_artifact_prepare_and_get_are_runtime_owned_surface_pipeline() {
        for operation in [
            "authoring_mesh_v2_high_artifact_prepare",
            "authoring_mesh_v2_high_artifact_get",
        ] {
            let route = resolve("surface_pipeline", operation)
                .expect("AuthoringMeshV2 High artifact route");
            assert_eq!(route.facade_name, "surface_pipeline");
            assert_eq!(route.domain, WeaponryServiceDomain::Surface);
            assert_eq!(
                route.execution_target,
                WeaponryOperationExecutionTarget::Runtime
            );
        }
    }
}
