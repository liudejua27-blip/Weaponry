//! Public MCP transport for the durable CameraLock registration lineage.
//!
//! The lineage is an additive child of the historical ProductionCameraLock
//! record.  It binds the Runtime-owned subject-frame registration, semantic
//! source ordering, authored view orientation and registered camera rig
//! hashes without copying any of those records into the MCP request.  The
//! Runtime remains the only producer; this module only exposes closed input
//! envelopes and the read/write transport classification.

use serde_json::{json, Map, Value};

const TRANSACTION: &str = "ProductionCameraLockRegistrationLineage@1";

const PREFLIGHT_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "diagnostic_inferred_rotation_degrees",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREFLIGHT_PROJECTION_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "preflight_id",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "proposed_board_rotation_degrees",
    "proposed_subject_screen_order",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "registration_lineage_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "semantic_landmark_ordering_id",
    "authored_orientation_id",
    "registered_rig_v2_id",
    "rear_three_quarter_rotation_degrees",
    "rear_three_quarter_subject_screen_order",
    "rear_three_quarter_camera_orbit_degrees",
    "approval_receipt_id",
    "approval_session_id",
    "approval_expires_at",
    "approval_summary",
    "approved",
    "idempotency_key",
    "input_sha256",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionCameraLockRegistrationLineageTool {
    Get,
    Preflight,
    PreflightProjection,
    Prepare,
}

impl ProductionCameraLockRegistrationLineageTool {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "production_camera_lock_registration_lineage_get",
            Self::Preflight => "production_camera_lock_registration_lineage_preflight_get",
            Self::PreflightProjection => {
                "production_camera_lock_registration_lineage_preflight_projection_get"
            }
            Self::Prepare => "production_camera_lock_registration_lineage_prepare",
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }

    /// The Runtime IPC method is intentionally the public tool name.  This
    /// keeps the adapter compatible with the Runtime method added by the
    /// durable-child implementation without inventing a second transport
    /// alias.
    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn from_name(name: &str) -> Option<ProductionCameraLockRegistrationLineageTool> {
    Some(match name {
        "production_camera_lock_registration_lineage_get" => {
            ProductionCameraLockRegistrationLineageTool::Get
        }
        "production_camera_lock_registration_lineage_preflight_get" => {
            ProductionCameraLockRegistrationLineageTool::Preflight
        }
        "production_camera_lock_registration_lineage_preflight_projection_get" => {
            ProductionCameraLockRegistrationLineageTool::PreflightProjection
        }
        "production_camera_lock_registration_lineage_prepare" => {
            ProductionCameraLockRegistrationLineageTool::Prepare
        }
        _ => return None,
    })
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(ProductionCameraLockRegistrationLineageTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(ProductionCameraLockRegistrationLineageTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![
        ProductionCameraLockRegistrationLineageTool::Get
            .name()
            .to_owned(),
        ProductionCameraLockRegistrationLineageTool::Preflight
            .name()
            .to_owned(),
        ProductionCameraLockRegistrationLineageTool::PreflightProjection
            .name()
            .to_owned(),
    ]
}

pub fn write_tool_names() -> Vec<String> {
    vec![ProductionCameraLockRegistrationLineageTool::Prepare
        .name()
        .to_owned()]
}

pub fn read_tools() -> Vec<Value> {
    vec![
        tool_definition(ProductionCameraLockRegistrationLineageTool::Get),
        tool_definition(ProductionCameraLockRegistrationLineageTool::Preflight),
        tool_definition(ProductionCameraLockRegistrationLineageTool::PreflightProjection),
    ]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(
        ProductionCameraLockRegistrationLineageTool::Prepare,
    )]
}

fn tool_definition(tool: ProductionCameraLockRegistrationLineageTool) -> Value {
    let (description, input_schema) = match tool {
        ProductionCameraLockRegistrationLineageTool::Get => (
            "Read one exact Runtime-owned durable ProductionCameraLockRegistrationLineage child by candidate, registration-lineage identity and exact parent CameraLock canonical hash. The lookup performs no write, stage advancement, confirmation, version creation or export.",
            get_schema(),
        ),
        ProductionCameraLockRegistrationLineageTool::Preflight => (
            "Read-only authority preflight for a CameraLock registration lineage. It reports a supplied 180-degree value only as diagnostic-inferred orientation and never treats it as user approval. The call reads exact parent/child bindings, fails closed when an orientation-specific user receipt is absent, and performs no SQLite/CAS write, Worker start, stage advancement, confirmation, version creation or export.",
            preflight_schema(),
        ),
        ProductionCameraLockRegistrationLineageTool::PreflightProjection => (
            "Read-only Runtime-derived semantic-camera projection for a proposed rear-three-quarter board rotation and stock/muzzle screen order. The caller cannot supply camera orbit, matrices, semantic anchors or geometry. Runtime returns the exact derived camera hash and upright proof for user review without creating approval authority, SQLite/CAS writes, Worker work, stage advancement, confirmation, version creation or export.",
            preflight_projection_schema(),
        ),
        ProductionCameraLockRegistrationLineageTool::Prepare => (
            "Prepare one Runtime-owned durable ProductionCameraLockRegistrationLineage child from a narrow parent CameraLock binding, target lineage/output IDs, and an explicitly approved rear-three-quarter board rotation, stock/muzzle screen order and closed camera-orbit choice. Runtime keeps pixel rotation separate from camera selection, projects source anchors to prove screen order and world-Y upright, and materializes RegisteredCameraRigCalibration@2 plus all hashes. Explicit MCP write opt-in is required and no stage, confirmation, version or export is performed.",
            prepare_schema(),
        ),
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":input_schema,
        "annotations":{
            "readOnlyHint":!tool.is_write(),
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":tool.is_write(),
            "approvalRequired":false
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.runtime_method(),
            "requiresConfirmation":false,
            "transaction":TRANSACTION,
            "definition_only":false
        }}
    })
}

fn object_schema(required: &[&str], properties: Map<String, Value>) -> Value {
    json!({
        "type":"object",
        "required":required,
        "properties":properties,
        "additionalProperties":false
    })
}

fn identifier_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
    })
}

fn sha256_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn idempotency_key_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$"
    })
}

fn get_schema() -> Value {
    object_schema(
        GET_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"ProductionCameraLockRegistrationLineageGetRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.camera-lock-registration-lineage-get@1"}),
            ),
            ("registration_lineage_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("camera_lock_id".to_owned(), identifier_property()),
            ("camera_lock_canonical_sha256".to_owned(), sha256_property()),
            ("max_response_bytes".to_owned(), json!({"const":1_048_576})),
            (
                "writer_policy".to_owned(),
                json!({"const":"forgecad-runtime-only-state-writer@1"}),
            ),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn preflight_schema() -> Value {
    object_schema(
        PREFLIGHT_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"ProductionCameraLockRegistrationLineagePreflightGetRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.camera-lock-registration-lineage-preflight-get@1"}),
            ),
            ("preflight_id".to_owned(), identifier_property()),
            ("registration_lineage_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("camera_lock_id".to_owned(), identifier_property()),
            ("camera_lock_canonical_sha256".to_owned(), sha256_property()),
            (
                "diagnostic_inferred_rotation_degrees".to_owned(),
                json!({"type":"integer","enum":[-180,-90,0,90,180]}),
            ),
            ("max_response_bytes".to_owned(), json!({"const":1_048_576})),
            (
                "writer_policy".to_owned(),
                json!({"const":"forgecad-runtime-only-state-writer@1"}),
            ),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn preflight_projection_schema() -> Value {
    object_schema(
        PREFLIGHT_PROJECTION_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.camera-lock-registration-lineage-preflight-projection-get@1"}),
            ),
            ("preflight_id".to_owned(), identifier_property()),
            ("registration_lineage_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("camera_lock_id".to_owned(), identifier_property()),
            ("camera_lock_canonical_sha256".to_owned(), sha256_property()),
            (
                "proposed_board_rotation_degrees".to_owned(),
                json!({"type":"integer","enum":[-180,-90,0,90,180]}),
            ),
            (
                "proposed_subject_screen_order".to_owned(),
                json!({"enum":["stock-left-muzzle-right","muzzle-left-stock-right"]}),
            ),
            ("max_response_bytes".to_owned(), json!({"const":1_048_576})),
            (
                "writer_policy".to_owned(),
                json!({"const":"forgecad-runtime-only-state-writer@1"}),
            ),
            ("runtime_write_performed".to_owned(), json!({"const":false})),
            (
                "persistent_user_data_touched".to_owned(),
                json!({"const":false}),
            ),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

fn prepare_schema() -> Value {
    object_schema(
        PREPARE_FIELDS,
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"ProductionCameraLockRegistrationLineagePrepareRequest@1"}),
            ),
            (
                "operation".to_owned(),
                json!({"const":"forgecad.production.camera-lock-registration-lineage-prepare@1"}),
            ),
            ("registration_lineage_id".to_owned(), identifier_property()),
            ("session_id".to_owned(), identifier_property()),
            ("project_id".to_owned(), identifier_property()),
            ("candidate_id".to_owned(), identifier_property()),
            ("candidate_state_sha256".to_owned(), sha256_property()),
            ("camera_lock_id".to_owned(), identifier_property()),
            ("camera_lock_canonical_sha256".to_owned(), sha256_property()),
            (
                "semantic_landmark_ordering_id".to_owned(),
                identifier_property(),
            ),
            ("authored_orientation_id".to_owned(), identifier_property()),
            ("registered_rig_v2_id".to_owned(), identifier_property()),
            (
                "rear_three_quarter_rotation_degrees".to_owned(),
                json!({"type":"integer","enum":[-180,-90,0,90,180]}),
            ),
            (
                "rear_three_quarter_subject_screen_order".to_owned(),
                json!({"enum":["stock-left-muzzle-right","muzzle-left-stock-right"]}),
            ),
            (
                "rear_three_quarter_camera_orbit_degrees".to_owned(),
                json!({"type":"integer","enum":[0,180]}),
            ),
            ("approval_receipt_id".to_owned(), identifier_property()),
            ("approval_session_id".to_owned(), identifier_property()),
            (
                "approval_expires_at".to_owned(),
                json!({"type":"string","pattern":"^[0-9]{1,10}$"}),
            ),
            (
                "approval_summary".to_owned(),
                json!({"type":"string","minLength":1,"maxLength":512}),
            ),
            ("approved".to_owned(), json!({"const":true})),
            ("idempotency_key".to_owned(), idempotency_key_property()),
            ("input_sha256".to_owned(), sha256_property()),
        ]),
    )
}

/// Keep the text content bounded and hash-only.  The complete Runtime-owned
/// record remains in structuredContent; renderer views and nested contracts
/// never get copied into the human-readable MCP text projection.
pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    let lookup = |field: &str| value.get(field).cloned().unwrap_or(Value::Null);
    let fallback_bool = |field: &str| value.get(field).cloned().unwrap_or(Value::Bool(false));
    let summary = json!({
        "schema_version":"ProductionCameraLockRegistrationLineageMcpSummary@1",
        "tool":tool.name(),
        "runtime_method":tool.runtime_method(),
        "write_intent":if tool.is_write() { "explicit_runtime_camera_lock_registration_lineage_prepare_write" } else if matches!(tool, ProductionCameraLockRegistrationLineageTool::Preflight) { "read_only_runtime_camera_lock_registration_lineage_authority_preflight" } else if matches!(tool, ProductionCameraLockRegistrationLineageTool::PreflightProjection) { "read_only_runtime_derived_semantic_camera_projection" } else { "read_only_runtime_camera_lock_registration_lineage_lookup" },
        "result_schema_version":lookup("schema_version"),
        "registration_lineage_id":lookup("registration_lineage_id"),
        "registration_lineage_object_sha256":lookup("registration_lineage_object_sha256"),
        "registration_lineage_canonical_sha256":lookup("registration_lineage_canonical_sha256"),
        "request_sha256":lookup("request_sha256"),
        "request_input_sha256":lookup("request_input_sha256"),
        "receipt_object_sha256":lookup("receipt_object_sha256"),
        "receipt_sha256":lookup("receipt_sha256"),
        "replayed":fallback_bool("replayed"),
        "restart_hash_verified":fallback_bool("restart_hash_verified"),
        "parent_camera_lock_status":lookup("parent_camera_lock_status"),
        "durable_lineage_status":lookup("durable_lineage_status"),
        "orientation_authority_status":lookup("orientation_authority_status"),
        "user_approved_orientation_present":fallback_bool("user_approved_orientation_present"),
        "diagnostic_inferred_orientation_present":fallback_bool("diagnostic_inferred_orientation_present"),
        "diagnostic_inferred_rotation_degrees":lookup("diagnostic_inferred_rotation_degrees"),
        "proposed_board_rotation_degrees":lookup("proposed_board_rotation_degrees"),
        "proposed_subject_screen_order":lookup("proposed_subject_screen_order"),
        "derived_camera_orbit_degrees":lookup("derived_camera_orbit_degrees"),
        "derived_camera_hash":lookup("derived_camera_hash"),
        "derived_camera_canonical_sha256":lookup("derived_camera_canonical_sha256"),
        "projection_status":lookup("projection_status"),
        "projection_input_sha256":lookup("projection_input_sha256"),
        "projection_ready_for_user_review":fallback_bool("projection_ready_for_user_review"),
        "existing_lineage_matches_proposal":fallback_bool("existing_lineage_matches_proposal"),
        "ready_for_promotable_lineage":fallback_bool("ready_for_promotable_lineage"),
        "blocking_reasons":lookup("blocking_reasons"),
        "policy":lookup("policy"),
        "runtime_write_performed":fallback_bool("runtime_write_performed"),
        "runtime_write":fallback_bool("runtime_write"),
        "worker_started":fallback_bool("worker_started"),
        "persistent_user_data_touched":fallback_bool("persistent_user_data_touched"),
        "production_stage_advanced":fallback_bool("production_stage_advanced"),
        "candidate_confirmed":fallback_bool("candidate_confirmed"),
        "version_created":fallback_bool("version_created"),
        "export_performed":fallback_bool("export_performed"),
        "depth_status":lookup("depth_status"),
        "quality_status":lookup("quality_status"),
        "canonical_sha256":lookup("canonical_sha256"),
        "structured_content_complete":true
    });
    serde_json::to_string(&summary).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_lineage_tools_are_closed_and_opt_in_classified() {
        let read = read_tools();
        let write = write_tools();
        assert_eq!(read.len(), 3);
        assert_eq!(write.len(), 1);
        assert_eq!(
            read[0]["name"],
            "production_camera_lock_registration_lineage_get"
        );
        assert_eq!(
            write[0]["name"],
            "production_camera_lock_registration_lineage_prepare"
        );
        assert_eq!(read[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(read[0]["annotations"]["writeIntent"], false);
        assert_eq!(write[0]["annotations"]["readOnlyHint"], false);
        assert_eq!(write[0]["annotations"]["writeIntent"], true);
        assert_eq!(read[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(write[0]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "ProductionCameraLockRegistrationLineageGetRequest@1"
        );
        assert_eq!(
            read[1]["name"],
            "production_camera_lock_registration_lineage_preflight_get"
        );
        assert_eq!(
            read[1]["inputSchema"]["properties"]["schema_version"]["const"],
            "ProductionCameraLockRegistrationLineagePreflightGetRequest@1"
        );
        assert_eq!(
            read[1]["inputSchema"]["properties"]["diagnostic_inferred_rotation_degrees"]["enum"],
            json!([-180, -90, 0, 90, 180])
        );
        assert_eq!(read[1]["annotations"]["readOnlyHint"], true);
        assert_eq!(read[1]["annotations"]["writeIntent"], false);
        assert_eq!(read[1]["inputSchema"]["additionalProperties"], false);
        assert_eq!(
            read[2]["name"],
            "production_camera_lock_registration_lineage_preflight_projection_get"
        );
        assert_eq!(
            read[2]["inputSchema"]["properties"]["schema_version"]["const"],
            "ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest@1"
        );
        assert!(read[2]["inputSchema"]["properties"]
            .get("proposed_camera_orbit_degrees")
            .is_none());
        assert_eq!(read[2]["annotations"]["readOnlyHint"], true);
        assert_eq!(
            write[0]["inputSchema"]["properties"]["schema_version"]["const"],
            "ProductionCameraLockRegistrationLineagePrepareRequest@1"
        );
        assert_eq!(
            read[0]["inputSchema"]["properties"]["max_response_bytes"]["const"],
            1_048_576
        );
        assert_eq!(
            write[0]["inputSchema"]["properties"]["approved"]["const"],
            true
        );
        for forbidden in [
            "lineage_scope",
            "parent_camera_lock",
            "parent_camera_lock_object_sha256",
            "geometry_program",
            "geometry_program_object_sha256",
            "semantic_landmark_ordering",
            "rear_three_quarter_authored_orientation",
            "registered_rig_v2",
            "registered_rig_v2_object_sha256",
            "registration_lineage_object_sha256",
            "registration_lineage_canonical_sha256",
        ] {
            assert!(
                write[0]["inputSchema"]["properties"]
                    .get(forbidden)
                    .is_none(),
                "{forbidden}"
            );
        }
        assert_eq!(
            runtime_method("production_camera_lock_registration_lineage_get"),
            Some("production_camera_lock_registration_lineage_get")
        );
        assert_eq!(
            runtime_method("production_camera_lock_registration_lineage_prepare"),
            Some("production_camera_lock_registration_lineage_prepare")
        );
        assert_eq!(
            runtime_method("production_camera_lock_registration_lineage_preflight_get"),
            Some("production_camera_lock_registration_lineage_preflight_get")
        );
        assert!(!is_write_tool(
            "production_camera_lock_registration_lineage_get"
        ));
        assert!(!is_write_tool(
            "production_camera_lock_registration_lineage_preflight_get"
        ));
        assert!(is_write_tool(
            "production_camera_lock_registration_lineage_prepare"
        ));
    }

    #[test]
    fn summary_is_hash_only() {
        let text = summary(
            "production_camera_lock_registration_lineage_get",
            &json!({
                "schema_version":"ProductionCameraLockRegistrationLineageGetResult@1",
                "registration_lineage_id":"lineage-1",
                "canonical_sha256":"a".repeat(64)
            }),
        )
        .expect("summary");
        let value: Value = serde_json::from_str(&text).expect("summary JSON");
        assert_eq!(value["structured_content_complete"], true);
        assert_eq!(value["runtime_write_performed"], false);
        assert_eq!(value["registration_lineage_id"], "lineage-1");
    }
}
