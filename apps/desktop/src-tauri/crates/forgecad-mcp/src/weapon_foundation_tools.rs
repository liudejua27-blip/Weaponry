use serde_json::{json, Map, Value};

const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Get,
    Prepare,
}

impl Tool {
    const fn name(self) -> &'static str {
        match self {
            Self::Get => "weapon_foundation_asset_get",
            Self::Prepare => "weapon_foundation_asset_prepare",
        }
    }

    const fn is_write(self) -> bool {
        matches!(self, Self::Prepare)
    }
}

fn from_name(name: &str) -> Option<Tool> {
    match name {
        "weapon_foundation_asset_get" => Some(Tool::Get),
        "weapon_foundation_asset_prepare" => Some(Tool::Prepare),
        _ => None,
    }
}

pub fn is_tool(name: &str) -> bool {
    from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    from_name(name).is_some_and(Tool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    from_name(name).map(Tool::name)
}

pub fn unavailable_error(name: &str) -> String {
    format!(
        "WEAPON_FOUNDATION_IMPORT_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}"
    )
}

pub fn read_tool_names() -> Vec<String> {
    vec![Tool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![Tool::Prepare.name().to_owned()]
}

fn id_property() -> Value {
    json!({
        "type":"string",
        "minLength":1,
        "maxLength":128,
        "pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"
    })
}

fn sha_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn object_schema(required: &[&str], properties: Map<String, Value>) -> Value {
    json!({
        "type":"object",
        "required":required,
        "properties":properties,
        "additionalProperties":false
    })
}

fn source_to_target_schema() -> Value {
    json!({
        "type":"object",
        "required":["mapping_evidence","axis_mapping","matrix_row_major","translation_m","scale_xyz"],
        "properties":{
            "mapping_evidence":{"enum":["PROVEN","PENDING_SOURCE_VERIFICATION"]},
            "axis_mapping":{"enum":[["-Z","+Y","+X"],["+Z","+Y","-X"]]},
            "matrix_row_major":{"enum":[[[0,0,-1],[0,1,0],[1,0,0]],[[0,0,1],[0,1,0],[-1,0,0]]]},
            "translation_m":{"const":[0.0,0.0,0.0]},
            "scale_xyz":{"const":[1.0,1.0,1.0]}
        },
        "additionalProperties":false
    })
}

fn degenerate_policy_schema() -> Value {
    json!({
        "type":"object",
        "required":["policy","test","area_epsilon_m2","area_comparison","ordering","reindexing"],
        "properties":{
            "policy":{"const":"drop-degenerate-faces-deterministic-source-order@1"},
            "test":{"const":"non-finite-or-area-less-than-threshold-after-source-to-target-transform@1"},
            "area_epsilon_m2":{"const":1e-12},
            "area_comparison":{"const":"strict-less-than"},
            "ordering":{"const":"source-primitive-index-then-face-index@1"},
            "reindexing":{"const":"stable-first-pass-compaction@1"}
        },
        "additionalProperties":false
    })
}

fn budgets_schema() -> Value {
    json!({
        "type":"object",
        "required":["max_source_nodes","max_source_meshes","max_source_triangles","max_cas_objects","max_wire_size"],
        "properties":{
            "max_source_nodes":{"const":512},
            "max_source_meshes":{"const":128},
            "max_source_triangles":{"const":250000},
            "max_cas_objects":{"const":32},
            "max_wire_size":{"const":1048576}
        },
        "additionalProperties":false
    })
}

fn prepare_schema() -> Value {
    object_schema(
        &[
            "schema_version",
            "request_id",
            "foundation_pack_id",
            "foundation_pack_version",
            "foundation_manifest_sha256",
            "asset_id",
            "asset_sha256",
            "asset_role",
            "source_format",
            "coordinate_spec_sha256",
            "coordinate_frame",
            "units",
            "source_to_target",
            "import_profile",
            "strict_readback_policy",
            "degenerate_face_policy",
            "budgets",
            "canonicalization_policy",
            "canonical_sha256",
        ],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"WeaponFoundationAssetRequest@1"}),
            ),
            ("request_id".to_owned(), id_property()),
            (
                "foundation_pack_id".to_owned(),
                json!({"const":"forgecad-fps-production-foundation"}),
            ),
            (
                "foundation_pack_version".to_owned(),
                json!({"const":"0.1.0-proposal"}),
            ),
            ("foundation_manifest_sha256".to_owned(), sha_property()),
            (
                "asset_id".to_owned(),
                json!({"enum":["pichuliru-weapon-west","wrad-arms","lightning-low-pbr"]}),
            ),
            ("asset_sha256".to_owned(), sha_property()),
            (
                "asset_role".to_owned(),
                json!({"enum":["rigged-weapon-semantic-source","first-person-armature-source","high-low-bake-pbr-animation-benchmark"]}),
            ),
            ("source_format".to_owned(), json!({"const":"glb"})),
            ("coordinate_spec_sha256".to_owned(), sha_property()),
            (
                "coordinate_frame".to_owned(),
                json!({"const":"weapon-right-handed-x-muzzle-y-up-z-right"}),
            ),
            ("units".to_owned(), json!({"const":"meter"})),
            ("source_to_target".to_owned(), source_to_target_schema()),
            (
                "import_profile".to_owned(),
                json!({"const":"forgecad-foundation-typed-import@1"}),
            ),
            (
                "strict_readback_policy".to_owned(),
                json!({"const":"glb-gltf-embedded-resource-strict-readback-no-external-reference@1"}),
            ),
            (
                "degenerate_face_policy".to_owned(),
                degenerate_policy_schema(),
            ),
            ("budgets".to_owned(), budgets_schema()),
            (
                "canonicalization_policy".to_owned(),
                json!({"const":CANONICALIZATION_POLICY}),
            ),
            ("canonical_sha256".to_owned(), sha_property()),
        ]),
    )
}

fn get_schema() -> Value {
    object_schema(
        &["schema_version", "request_id"],
        Map::from_iter([
            (
                "schema_version".to_owned(),
                json!({"const":"WeaponFoundationAssetGetRequest@1"}),
            ),
            ("request_id".to_owned(), id_property()),
            (
                "request_sha256".to_owned(),
                json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}),
            ),
            (
                "result_object_sha256".to_owned(),
                json!({"type":["string","null"],"pattern":"^[0-9a-f]{64}$"}),
            ),
        ]),
    )
}

fn tool_definition(tool: Tool) -> Value {
    let (description, schema) = match tool {
        Tool::Get => (
            "Read and reverify one Runtime-owned typed FPS foundation import after restart. The response contains only bounded status and CAS hashes; source/topology bytes are never returned.",
            get_schema(),
        ),
        Tool::Prepare => (
            "Prepare one closed, embedded FPS foundation typed import. Runtime normalizes coordinates, sockets, rig mapping and a draft FpsPresentationPackage into compact CAS; AuthoringMesh materialization remains pending and explicit write opt-in is required.",
            prepare_schema(),
        ),
    };
    json!({
        "name":tool.name(),
        "description":description,
        "inputSchema":schema,
        "annotations":{
            "readOnlyHint":!tool.is_write(),
            "destructiveHint":false,
            "idempotentHint":true,
            "openWorldHint":false,
            "writeIntent":tool.is_write()
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.name(),
            "requiresConfirmation":false,
            "transaction":"WeaponFoundationImport@1",
            "maxResponseBytes":MAX_RESPONSE_BYTES,
            "definition_only":false
        }}
    })
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(Tool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![tool_definition(Tool::Prepare)]
}

pub fn summary(name: &str, value: &Value) -> Option<String> {
    let tool = from_name(name)?;
    serde_json::to_string(&json!({
        "schema_version":"WeaponFoundationImportMcpSummary@1",
        "operation":name,
        "write_intent":if tool.is_write() { "explicit_runtime_durable_foundation_import_write" } else { "read_only_runtime_durable_foundation_import_lookup" },
        "request_id":value.get("request_id"),
        "request_sha256":value.get("request_sha256"),
        "result_id":value.get("result_id"),
        "result_object_sha256":value.get("result_object_sha256"),
        "import_record_sha256":value.get("import_record_sha256"),
        "asset_id":value.get("asset_id"),
        "topology_object_sha256":value.get("topology_object_sha256"),
        "socket_map_object_sha256":value.get("socket_map_object_sha256"),
        "rig_map_object_sha256":value.get("rig_map_object_sha256"),
        "fps_presentation_package_object_sha256":value.get("fps_presentation_package_object_sha256"),
        "fps_presentation_package":value.get("fps_presentation_package"),
        "authoring_mesh_materialization_status":value.get("authoring_mesh_materialization_status"),
        "quality_status":value.get("quality_status"),
        "promotion_eligible":value.get("promotion_eligible"),
        "runtime_write_performed":value.get("runtime_write_performed"),
        "persistent_user_data_touched":false,
        "candidate_confirmed":value.get("candidate_confirmed"),
        "version_created":value.get("version_created"),
        "export_performed":value.get("export_performed"),
        "actual_engine_roundtrip":value.get("actual_engine_roundtrip"),
        "human_review_status":value.get("human_review_status"),
        "replayed":value.get("replayed"),
        "restart_hash_verified":value.get("restart_hash_verified"),
        "structured_content_complete":true
    }))
    .ok()
}
