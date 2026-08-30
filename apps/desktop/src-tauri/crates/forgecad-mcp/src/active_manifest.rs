//! Pure MCP exposure builders for the default Weaponry Knife profile.
//!
//! This module is intentionally limited to the public manifest boundary.  It
//! does not own session state, call Runtime, read Store/CAS, or know about the
//! legacy compatibility registry.  The entrypoint can therefore compose these
//! values into JSON-RPC responses without coupling manifest construction to
//! transport or backend lifecycle code.

use crate::knife_tool_profile;
use serde_json::{json, Value};

/// The only static resource exposed by the default Knife adapter.
pub(crate) const CAPABILITIES_RESOURCE_URI: &str = "forgecad://capabilities";

/// Keep a capabilities projection within the ordinary MCP resource budget.
/// The transport may apply a stricter operation-specific budget when it wraps
/// the response; this limit protects this pure resource adapter as well.
pub(crate) const RESOURCE_MAX_BYTES: usize = 1024 * 1024;

/// Build the active Knife tool definitions for the `tools/list` result.
///
/// `knife_tool_profile::active_tools` owns the profile and package-schema
/// projection.  Calling it here keeps the manifest extraction independent of
/// the compatibility binary and prevents a default build from constructing
/// the historical raw tool registry.
pub(crate) fn tool_definitions() -> Result<Vec<Value>, String> {
    knife_tool_profile::active_tools()
}

/// Construct the MCP result object for `tools/list`.
pub(crate) fn build_tools_list() -> Result<Value, String> {
    Ok(json!({"tools": tool_definitions()?}))
}

/// Build the default Knife manifest summary without loading compatibility
/// handlers or their 226-operation manifest.
pub(crate) fn build_manifest_summary() -> Result<Value, String> {
    knife_tool_profile::active_manifest_summary()
}

/// Return static resource descriptors in the default MCP wire shape.
///
/// Runtime-backed capability values are supplied later to
/// [`build_capabilities_read`].  Listing the resource never needs a backend.
pub(crate) fn static_resource_descriptors() -> Vec<Value> {
    vec![json!({
        "uri": CAPABILITIES_RESOURCE_URI,
        "name": "ForgeCAD capabilities",
        "description": "Read-only Knife profile and Runtime capability projection",
        "mimeType": "application/json"
    })]
}

/// Construct the MCP result object for `resources/list`.
pub(crate) fn build_resources_list() -> Value {
    json!({"resources": static_resource_descriptors()})
}

/// Wrap a Runtime or static capability projection as an MCP `resources/read`
/// result.  The capability projection is borrowed and never modified.
///
/// This function deliberately handles only the one static URI advertised by
/// [`static_resource_descriptors`].  Dynamic resource routing belongs to the
/// explicit compatibility adapter and is not silently broadened here.
pub(crate) fn build_capabilities_read(uri: &str, capabilities: &Value) -> Result<Value, String> {
    if uri != CAPABILITIES_RESOURCE_URI {
        return Err(format!(
            "INVALID_RESOURCE_URI: unknown default Knife resource URI {uri}"
        ));
    }
    let text = serde_json::to_string(capabilities)
        .map_err(|error| format!("RESOURCE_SERIALIZATION_FAILED: {error}"))?;
    if text.len() > RESOURCE_MAX_BYTES {
        return Err(format!(
            "MCP_RESOURCE_RESPONSE_BUDGET_EXCEEDED: capabilities resource is {} bytes, limit is {}",
            text.len(),
            RESOURCE_MAX_BYTES
        ));
    }
    Ok(json!({
        "contents": [{
            "uri": CAPABILITIES_RESOURCE_URI,
            "mimeType": "application/json",
            "text": text
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knife_tool_profile::FACADE_NAMES;
    use std::collections::BTreeSet;

    fn tool_names(value: &Value) -> Vec<String> {
        value
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn tools_list_is_exactly_the_default_knife_surface() {
        let result = build_tools_list().expect("Knife tools/list result");
        let names = tool_names(&result);
        assert_eq!(names.len(), FACADE_NAMES.len());
        assert_eq!(
            names,
            FACADE_NAMES
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            names.iter().collect::<BTreeSet<_>>().len(),
            FACADE_NAMES.len()
        );
        assert!(result
            .pointer("/tools/0/_meta/forgecad/profileId")
            .and_then(Value::as_str)
            .is_some_and(|profile| profile == knife_tool_profile::KNIFE_PROFILE_ID));
        assert!(!names.iter().any(|name| name == "project_create"));
    }

    #[test]
    fn manifest_summary_is_default_only_and_does_not_claim_compatibility_loaded() {
        let summary = build_manifest_summary().expect("Knife manifest summary");
        assert_eq!(summary["default_tool_count"], 11);
        assert_eq!(summary["active_operation_count"], 125);
        assert_eq!(summary["closed_request_schema_count"], 125);
        assert_eq!(summary["schema_blocked_request_count"], 0);
        assert_eq!(summary["compatibility_manifest_sha256"], Value::Null);
        assert_eq!(summary["compatibility_requires_explicit_profile"], true);
    }

    #[test]
    fn resources_list_is_static_and_contains_only_capabilities() {
        let result = build_resources_list();
        let resources = result
            .get("resources")
            .and_then(Value::as_array)
            .expect("resources array");
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0]["uri"], CAPABILITIES_RESOURCE_URI);
        assert_eq!(resources[0]["mimeType"], "application/json");
    }

    #[test]
    fn capabilities_read_is_bounded_and_does_not_mutate_input() {
        let capabilities = json!({
            "status": "runtime-unavailable",
            "mcp_tool_profile": knife_tool_profile::KNIFE_PROFILE_ID
        });
        let before = capabilities.clone();
        let result = build_capabilities_read(CAPABILITIES_RESOURCE_URI, &capabilities)
            .expect("capabilities resource read");
        assert_eq!(capabilities, before);
        assert_eq!(result["contents"][0]["uri"], CAPABILITIES_RESOURCE_URI);
        assert_eq!(result["contents"][0]["mimeType"], "application/json");
        let text = result["contents"][0]["text"]
            .as_str()
            .expect("serialized capabilities text");
        assert_eq!(
            serde_json::from_str::<Value>(text).expect("capability JSON"),
            capabilities
        );
    }

    #[test]
    fn capabilities_read_rejects_unknown_uri_and_oversized_projection() {
        let error = build_capabilities_read("forgecad://unknown", &json!({}))
            .expect_err("unknown resource URI must fail closed");
        assert!(error.starts_with("INVALID_RESOURCE_URI:"));

        let oversized = json!({"payload": "x".repeat(RESOURCE_MAX_BYTES) });
        let error = build_capabilities_read(CAPABILITIES_RESOURCE_URI, &oversized)
            .expect_err("oversized resource must fail closed");
        assert!(error.starts_with("MCP_RESOURCE_RESPONSE_BUDGET_EXCEEDED:"));
    }
}
