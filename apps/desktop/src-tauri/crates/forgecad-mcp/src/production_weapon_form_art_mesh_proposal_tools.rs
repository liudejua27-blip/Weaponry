use serde_json::{json, Map, Value};

const GET_TOOL_NAME: &str = "production_weapon_form_art_mesh_proposal_get";
const PREPARE_TOOL_NAME: &str = "production_weapon_form_art_mesh_proposal_prepare";
const REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponFormArtMeshProposalGetRequest@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const REQUIRED_FIELDS: [&str; 22] = [
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "mesh_id",
    "lineage_id",
    "parent_revision_id",
    "parent_revision_sha256",
    "parent_revision_object_sha256",
    "source_node_id",
    "part_id",
    "source_binding_sha256",
    "form_art_evidence_id",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "edit",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn identifier_property() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._:-]+$"})
}

fn sha_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn move_vertices_edit_schema() -> Value {
    json!({
        "type":"object",
        "required":["schema_version","operation","source_node_id","part_id","coordinate_space","selection_policy","vertex_moves","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshMoveVertices@1"},
            "operation":{"const":"move_vertices"},
            "source_node_id":identifier_property(),
            "part_id":identifier_property(),
            "coordinate_space":{"const":"source-local"},
            "selection_policy":{"const":"explicit-stable-vertex-ids@1"},
            "vertex_moves":{
                "type":"array","minItems":1,"maxItems":32,
                "items":{
                    "type":"object","additionalProperties":false,
                    "required":["vertex_id","before_position_m","after_position_m"],
                    "properties":{
                        "vertex_id":identifier_property(),
                        "before_position_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10,"maximum":10}},
                        "after_position_m":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-10,"maximum":10}}
                    }
                }
            },
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn open_frame_notch_edit_schema() -> Value {
    json!({
        "type":"object",
        "required":["schema_version","operation","source_node_id","part_id","coordinate_space","selection_policy","opening_width_milli","opening_height_milli","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshOpenFrameNotch@1"},
            "operation":{"const":"open_frame_notch"},
            "source_node_id":identifier_property(),
            "part_id":identifier_property(),
            "coordinate_space":{"const":"source-local"},
            "selection_policy":{"const":"runtime-derived-box-open-frame@1"},
            "opening_width_milli":{"type":"integer","minimum":1,"maximum":999},
            "opening_height_milli":{"type":"integer","minimum":1,"maximum":999},
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn rear_stock_void_rail_bow_edit_schema() -> Value {
    json!({
        "type":"object",
        "required":["schema_version","operation","source_node_id","part_id","coordinate_space","selection_policy","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshRearStockVoidRailBow@1"},
            "operation":{"const":"rear_stock_void_rail_bow"},
            "source_node_id":identifier_property(),
            "part_id":identifier_property(),
            "coordinate_space":{"const":"source-local"},
            "selection_policy":{"const":"runtime-derived-rear-stock-void-rail-bow@1"},
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn rear_stock_void_boundary_bridge_edit_schema() -> Value {
    json!({
        "type":"object",
        "required":["schema_version","operation","source_node_id","part_id","coordinate_space","selection_policy","profile_id","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"AuthoringMeshRearStockVoidBoundaryBridge@1"},
            "operation":{"const":"rear_stock_void_boundary_bridge"},
            "source_node_id":{"const":"rear-stock"},
            "part_id":{"const":"rear-stock"},
            "coordinate_space":{"const":"source-local"},
            "selection_policy":{"const":"runtime-derived-rear-stock-void-boundary-bridge@1"},
            "profile_id":{"const":"registered-void-boundary-depth-wedge-5@1"},
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn edit_schema() -> Value {
    json!({
        "oneOf":[move_vertices_edit_schema(), open_frame_notch_edit_schema(), rear_stock_void_rail_bow_edit_schema(), rear_stock_void_boundary_bridge_edit_schema()]
    })
}

fn input_schema() -> Value {
    let mut properties = Map::new();
    properties.insert(
        "schema_version".to_owned(),
        json!({"const":REQUEST_SCHEMA_VERSION}),
    );
    for field in [
        "project_id",
        "candidate_id",
        "mesh_id",
        "lineage_id",
        "parent_revision_id",
        "source_node_id",
        "part_id",
        "form_art_evidence_id",
        "idempotency_key",
    ] {
        properties.insert(field.to_owned(), identifier_property());
    }
    for field in [
        "candidate_state_sha256",
        "parent_revision_sha256",
        "parent_revision_object_sha256",
        "source_binding_sha256",
        "form_art_evidence_object_sha256",
        "form_art_evidence_canonical_sha256",
        "input_sha256",
    ] {
        properties.insert(field.to_owned(), sha_property());
    }
    properties.insert("edit".to_owned(), edit_schema());
    properties.insert(
        "max_response_bytes".to_owned(),
        json!({"const":MAX_RESPONSE_BYTES}),
    );
    properties.insert("runtime_write_performed".to_owned(), json!({"const":false}));
    properties.insert("writer_policy".to_owned(), json!({"const":WRITER_POLICY}));
    properties.insert(
        "canonicalization_policy".to_owned(),
        json!({"const":CANONICALIZATION_POLICY}),
    );
    json!({
        "type":"object",
        "required":REQUIRED_FIELDS,
        "properties":properties,
        "additionalProperties":false
    })
}

pub fn is_tool(name: &str) -> bool {
    matches!(name, GET_TOOL_NAME | PREPARE_TOOL_NAME)
}

pub fn is_write_tool(name: &str) -> bool {
    name == PREPARE_TOOL_NAME
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(GET_TOOL_NAME)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(PREPARE_TOOL_NAME)]
}

fn tool_definition(name: &str) -> Value {
    let write = is_write_tool(name);
    let (description, transaction) = if write {
        (
            "Prepare a real-D1 FormArt single-source-node typed authoring edit (MoveVertices, OpenFrameNotch, RearStockVoidRailBow or RearStockVoidBoundaryBridge). Runtime revalidates the durable AuthoringMesh@2 parent, materializes a durable child revision, replaces only the bound GeometryProgram source node with authoring-mesh@1 while preserving Part outputs and other nodes, then hashes, compiles and prepares a new candidate. Six-view FormArt review remains required; this does not approve secondary form, advance Stage, confirm, version or export.",
            "FormArt→AuthoringMesh@2→authoring-mesh@1 GeometryProgram→candidate prepare",
        )
    } else {
        (
            "Read-only real-D1 FormArt single-source-node typed authoring proposal (MoveVertices, OpenFrameNotch, RearStockVoidRailBow or RearStockVoidBoundaryBridge). Runtime revalidates the durable AuthoringMesh@2 parent, exact candidate/source-node/Part binding and six-view FormArt evidence, then returns a compatibility payload and deterministic proposed child-revision identity. It does not apply a typed edit, write state, approve secondary form, advance Stage, confirm, version or export.",
            "FormArt→AuthoringMesh@2 proposal",
        )
    };
    json!({
        "name":name,
        "description":description,
        "inputSchema":input_schema(),
        "annotations":{
            "readOnlyHint":!write,
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":write,
            "approvalRequired":false
        },
        "_meta":{"forgecad":{"availability":"available","runtime_method":name,"requiresConfirmation":false,"transaction":transaction,"definition_only":false}}
    })
}

pub fn read_tool_names() -> Vec<String> {
    vec![GET_TOOL_NAME.to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![PREPARE_TOOL_NAME.to_owned()]
}

pub fn from_name(name: &str) -> Option<&'static str> {
    match name {
        GET_TOOL_NAME => Some(GET_TOOL_NAME),
        PREPARE_TOOL_NAME => Some(PREPARE_TOOL_NAME),
        _ => None,
    }
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name)
}

pub fn unavailable_error(name: &str) -> String {
    format!("PRODUCTION_WEAPON_FORM_ART_MESH_PROPOSAL_RUNTIME_METHOD_UNAVAILABLE: {name}")
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    if !is_tool(name) {
        return None;
    }
    let write_intent = if is_write_tool(name) {
        "runtime_prepare_form_art_typed_authoring_candidate"
    } else {
        "read_only_runtime_form_art_typed_authoring_proposal"
    };
    serde_json::to_string(&json!({
        "schema_version":"ProductionWeaponFormArtMeshProposalMcpSummary@1",
        "operation":name,
        "write_intent":write_intent,
        "proposal_id":value.get("proposal_id"),
        "project_id":value.get("project_id"),
        "candidate_id":value.get("candidate_id"),
        "mesh_id":value.get("mesh_id"),
        "lineage_id":value.get("lineage_id"),
        "source_node_id":value.get("source_node_id"),
        "part_id":value.get("part_id"),
        "edit_operation":value.pointer("/typed_edit/operation"),
        "parent_revision_id":value.pointer("/parent_revision/revision_id"),
        "child_revision_id":value.pointer("/child_revision/revision_id"),
        "proposal_status":value.get("proposal_status"),
        "source_form_art_cohort_status":value.get("source_form_art_cohort_status"),
        "prepare_eligible_by_form_art_cohort":value.get("prepare_eligible_by_form_art_cohort"),
        "blocking_reasons":value.get("blocking_reasons"),
        "secondary_form_approved":value.get("secondary_form_approved"),
        "runtime_write_performed":value.get("runtime_write_performed"),
        "stage_advanced":value.get("stage_advanced"),
        "quality_status":value.get("quality_status"),
        "canonical_sha256":value.get("canonical_sha256"),
        "structured_content_complete":true
    }))
    .ok()
}
