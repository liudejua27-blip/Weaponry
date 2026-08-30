#[path = "../src/result_adapter.rs"]
mod result_adapter;

use result_adapter::{
    apply_mcp_response_budget, apply_read_model_mcp_wire_budget, error_response,
    normalize_typed_error, runtime_error_value, runtime_error_value_for, safe_error, summary_text,
    summary_text_with, tool_error, tool_error_for, tool_success, tool_success_for, AdapterProfile,
    ErrorFamily, SummaryProvider, COMPATIBILITY_BOUNDED_OPERATION_NAMES,
    HERO_UV_MCP_RESPONSE_MAX_BYTES, MCP_RESPONSE_MAX_BYTES,
};
use serde_json::{json, Value};

fn curve_summary(operation: &str, _value: &Value) -> Option<String> {
    (operation == "knife_curve_modifier_graph_get").then(|| "curve-summary".to_owned())
}

#[test]
fn runtime_error_policies_keep_default_and_compatibility_differences() {
    let default_busy = runtime_error_value("RUNTIME_BUSY: worker is occupied\nwith private detail");
    assert_eq!(default_busy["code"], "RUNTIME_BUSY");
    assert_eq!(
        default_busy["message"],
        "worker is occupiedwith private detail"
    );
    assert_eq!(default_busy["retryable"], true);

    let compat_busy = runtime_error_value_for(
        AdapterProfile::Compatibility,
        "RUNTIME_BUSY: worker is occupied",
    );
    assert_eq!(compat_busy["retryable"], false);
    assert!(compat_busy["next_action"]
        .as_str()
        .expect("next action")
        .contains("required MCP task"));

    let unknown = runtime_error_value("untyped failure");
    assert_eq!(unknown["code"], "RUNTIME_REQUEST_FAILED");
    assert_eq!(unknown["message"], "untyped failure");
    assert_eq!(unknown["evidence_ids"], json!([]));
}

#[test]
fn safe_error_is_single_line_and_bounded() {
    let detail = format!("a\r\nb{}", "x".repeat(700));
    let safe = safe_error(&detail);
    assert_eq!(safe.len(), 512);
    assert!(!safe.contains('\n'));
    assert!(!safe.contains('\r'));
    assert!(safe.starts_with("ab"));
}

#[test]
fn jsonrpc_and_tool_errors_preserve_id_and_data_shape() {
    assert!(error_response(None, -32600, "Invalid Request", None).is_none());
    let response = error_response(
        Some(Value::Null),
        -32602,
        "Invalid params",
        Some(json!({"code":"INVALID_TOOL_PARAMS"})),
    )
    .expect("json-rpc response");
    assert_eq!(response["id"], Value::Null);
    assert_eq!(response["error"]["data"]["code"], "INVALID_TOOL_PARAMS");

    let result =
        tool_error(json!(7), "RUNTIME_UNAVAILABLE: socket missing").expect("tool error response");
    assert_eq!(result["id"], 7);
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(
        result["result"]["structuredContent"]["schema_version"],
        "RuntimeError@1"
    );
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("serialized error text");
    let text_value: Value = serde_json::from_str(text).expect("error text JSON");
    assert_eq!(text_value, result["result"]["structuredContent"]);

    assert!(tool_error_for(
        None,
        "RUNTIME_UNAVAILABLE: no id",
        AdapterProfile::Compatibility
    )
    .is_none());
}

#[test]
fn summary_provider_only_changes_text_and_keeps_exact_structured_content() {
    let value = json!({"operation":"knife_curve_modifier_graph_get","sha256":"a".repeat(64)});
    let providers: &[SummaryProvider] = &[curve_summary];
    assert_eq!(
        summary_text_with("knife_curve_modifier_graph_get", &value, providers),
        "curve-summary"
    );
    assert_eq!(
        summary_text("other_operation", &value),
        serde_json::to_string(&value).unwrap()
    );

    let response = tool_success_for(
        Some(json!(9)),
        "knife_curve_modifier_graph_get",
        value.clone(),
        providers,
        AdapterProfile::DefaultKnife,
    )
    .expect("success response");
    assert_eq!(response["result"]["content"][0]["text"], "curve-summary");
    assert_eq!(response["result"]["structuredContent"], value);

    let default_wrapper = tool_success(json!(10), "other_operation", json!({"ok":true}), &[])
        .expect("default success wrapper");
    assert_eq!(default_wrapper["id"], 10);
}

#[test]
fn default_budget_is_always_enforced_and_hero_uv_is_larger() {
    let oversized = json!({
        "jsonrpc":"2.0",
        "id":1,
        "result":{"structuredContent":{"payload":"x".repeat(MCP_RESPONSE_MAX_BYTES)}}
    });
    let bounded = apply_mcp_response_budget("candidate_get", oversized);
    assert_eq!(
        bounded["result"]["structuredContent"]["code"],
        "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED"
    );
    assert!(serde_json::to_vec(&bounded).unwrap().len() < MCP_RESPONSE_MAX_BYTES);

    let hero_under_eight = json!({
        "jsonrpc":"2.0",
        "id":2,
        "result":{"structuredContent":{"payload":"x".repeat(MCP_RESPONSE_MAX_BYTES + 1024)}}
    });
    let hero = apply_mcp_response_budget("hero_uv_durable_get", hero_under_eight.clone());
    assert_eq!(
        hero["result"]["structuredContent"]["payload"]
            .as_str()
            .unwrap()
            .len(),
        MCP_RESPONSE_MAX_BYTES + 1024
    );
    assert_eq!(hero["id"], 2);
    assert_eq!(HERO_UV_MCP_RESPONSE_MAX_BYTES, 8 * MCP_RESPONSE_MAX_BYTES);
}

#[test]
fn compatibility_budget_only_applies_to_historical_bounded_operations() {
    assert_eq!(COMPATIBILITY_BOUNDED_OPERATION_NAMES.len(), 140);
    assert!(COMPATIBILITY_BOUNDED_OPERATION_NAMES.contains(&"geometry_program_hash"));
    assert!(COMPATIBILITY_BOUNDED_OPERATION_NAMES.contains(&"hero_uv_durable_get"));
    assert!(!COMPATIBILITY_BOUNDED_OPERATION_NAMES.contains(&"project_list"));

    let oversized = json!({
        "jsonrpc":"2.0",
        "id":3,
        "result":{"structuredContent":{"payload":"x".repeat(MCP_RESPONSE_MAX_BYTES)}}
    });
    let unbounded = apply_read_model_mcp_wire_budget("project_list", oversized.clone());
    assert_eq!(unbounded, oversized);

    let bounded = apply_read_model_mcp_wire_budget("geometry_program_hash", oversized);
    assert_eq!(
        bounded["result"]["structuredContent"]["code"],
        "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED"
    );
    assert_eq!(bounded["id"], 3);
    assert!(serde_json::to_vec(&bounded).unwrap().len() < MCP_RESPONSE_MAX_BYTES);
}

#[test]
fn compatibility_render_pass_adapter_removes_raw_png_from_structured_content() {
    let response = result_adapter::render_pass_result_for(
        json!(4),
        json!({"png_base64":"aGVsbG8=","render_set_hash":"abc"}),
        AdapterProfile::Compatibility,
    );
    assert_eq!(response["result"]["content"][0]["type"], "image");
    assert_eq!(response["result"]["content"][0]["data"], "aGVsbG8=");
    assert!(response["result"]["structuredContent"]
        .get("png_base64")
        .is_none());
    assert_eq!(
        response["result"]["structuredContent"]["render_set_hash"],
        "abc"
    );

    let missing = result_adapter::render_pass_result_for(
        json!(5),
        json!({"render_set_hash":"abc"}),
        AdapterProfile::Compatibility,
    );
    assert_eq!(missing["result"]["isError"], true);
    assert_eq!(
        missing["result"]["structuredContent"]["code"],
        "RENDER_PASS_INVALID"
    );
    assert_eq!(
        missing["result"]["content"][0]["text"],
        "Runtime returned an invalid render pass"
    );
}

#[test]
fn compatibility_typed_error_normalizers_preserve_code_and_drop_detail() {
    let identity = normalize_typed_error(
        "invalid runtime input: AUTHORING_MESH_IDENTITY_LINEAGE_HASH_MISMATCH: /private/path",
        ErrorFamily::IdentityLineage,
    );
    assert_eq!(
        identity,
        "AUTHORING_MESH_IDENTITY_LINEAGE_HASH_MISMATCH: Runtime identity lineage request rejected"
    );

    let topology = normalize_typed_error(
        "invalid runtime input: AUTHORING_MESH_EDIT_STALE: user/path",
        ErrorFamily::AuthoringTopology,
    );
    assert_eq!(
        topology,
        "AUTHORING_MESH_EDIT_STALE: Runtime topology edit request rejected"
    );

    let bake = normalize_typed_error(
        "PRODUCTION_WEAPON_HIGH_LOW_BAKE_MISS: source/path",
        ErrorFamily::ProductionWeaponHighLowBake,
    );
    assert_eq!(
        bake,
        "PRODUCTION_WEAPON_HIGH_LOW_BAKE_MISS: Runtime formal High/Low/Cage bake request rejected"
    );

    assert_eq!(
        normalize_typed_error("UNRELATED: keep", ErrorFamily::IdentityLineage),
        "UNRELATED: keep"
    );
}
