use forgecad_runtime::canonical_json_hash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const PROFILE_ENV: &str = "WEAPONRY_MCP_TOOL_PROFILE";
pub const KNIFE_PROFILE_ID: &str = "weaponry-knife-p0-default@1";
pub const COMPATIBILITY_PROFILE_ID: &str = "weaponry-legacy-compatibility@1";

const PROFILE_JSON: &str =
    include_str!("../../../../../../packages/forgecad-contracts/profiles/weaponry-knife-p0.json");

pub const FACADE_NAMES: [&str; 11] = [
    "weapon_preflight",
    "reference_intake",
    "observe",
    "authoring_transaction",
    "surface_pipeline",
    "fps_presentation",
    "quality_review",
    "delivery",
    "approval",
    "recovery",
    "job",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolProfile {
    Knife,
    Compatibility,
}

impl ToolProfile {
    pub fn from_environment() -> Result<Self, String> {
        match std::env::var(PROFILE_ENV) {
            Err(std::env::VarError::NotPresent) => Ok(Self::Knife),
            Ok(value) if value == "knife" => Ok(Self::Knife),
            Ok(value) if value == "compatibility" => {
                crate::compatibility_registry::ensure_enabled().map(|_| Self::Compatibility)
            }
            Ok(_) | Err(std::env::VarError::NotUnicode(_)) => Err(format!(
                "WEAPONRY_TOOL_PROFILE_INVALID: {PROFILE_ENV} must be knife or compatibility"
            )),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Knife => KNIFE_PROFILE_ID,
            Self::Compatibility => COMPATIBILITY_PROFILE_ID,
        }
    }
}

#[derive(Debug, Clone)]
struct FacadeSpec {
    name: String,
    read_operations: Vec<String>,
    write_operations: Vec<String>,
    operations: Vec<String>,
    allowlist_sha256: String,
}

#[derive(Debug, Clone)]
struct NativeSpec {
    operation: String,
    read_only: bool,
    facade_name: String,
    request_schema: String,
    result_schema: String,
}

fn parse_specs() -> Result<Vec<FacadeSpec>, String> {
    let profile: Value = serde_json::from_str(PROFILE_JSON)
        .map_err(|error| format!("WEAPONRY_KNIFE_PROFILE_INVALID: {error}"))?;
    let facades = profile
        .get("facades")
        .and_then(Value::as_object)
        .ok_or_else(|| "WEAPONRY_KNIFE_PROFILE_INVALID: facades must be an object".to_owned())?;
    let actual_names = facades.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected_names = FACADE_NAMES.into_iter().collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(
            "WEAPONRY_KNIFE_PROFILE_INVALID: facade names differ from the closed 11-name profile"
                .to_owned(),
        );
    }

    FACADE_NAMES
        .iter()
        .map(|name| {
            let value = facades
                .get(*name)
                .and_then(Value::as_object)
                .ok_or_else(|| format!("WEAPONRY_KNIFE_PROFILE_INVALID: {name} is not an object"))?;
            if value.get("facade_name").and_then(Value::as_str) != Some(*name) {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: {name} facade_name mismatch"
                ));
            }
            let read_operations = string_array(value, "read_tools", name)?;
            let write_operations = string_array(value, "write_tools", name)?;
            let operations = string_array(value, "underlying_operations", name)?;
            let read = read_operations.iter().cloned().collect::<BTreeSet<_>>();
            let write = write_operations.iter().cloned().collect::<BTreeSet<_>>();
            if !read.is_disjoint(&write) {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: {name} classifies an operation twice"
                ));
            }
            // Façade-native operations are declared separately in the profile
            // because they must never enter the compatibility raw manifest.
            // They still belong to the authoring_transaction underlying route.
            let native = if *name == "authoring_transaction" {
                profile
                    .get("native_operations")
                    .and_then(Value::as_object)
                    .map(|operations| operations.keys().cloned().collect::<BTreeSet<_>>())
                    .unwrap_or_default()
            } else {
                BTreeSet::new()
            };
            if !read.is_disjoint(&native) || !write.is_disjoint(&native) {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: {name} classifies a native operation twice"
                ));
            }
            let classified = read
                .union(&write)
                .chain(native.iter())
                .cloned()
                .collect::<BTreeSet<_>>();
            let declared = operations.iter().cloned().collect::<BTreeSet<_>>();
            if classified != declared || declared.len() != operations.len() {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: {name} operation classification differs from its allowlist"
                ));
            }
            let expected_hash = value
                .get("underlying_operation_allowlist_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_INVALID: {name} allowlist hash is missing"
                    )
                })?;
            if expected_hash.len() != 64
                || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: {name} allowlist hash is not a SHA-256"
                ));
            }
            Ok(FacadeSpec {
                name: (*name).to_owned(),
                read_operations,
                write_operations,
                operations,
                allowlist_sha256: expected_hash.to_owned(),
            })
        })
        .collect()
}

fn string_array(
    object: &Map<String, Value>,
    key: &str,
    facade_name: &str,
) -> Result<Vec<String>, String> {
    let values = object.get(key).and_then(Value::as_array).ok_or_else(|| {
        format!("WEAPONRY_KNIFE_PROFILE_INVALID: {facade_name}.{key} must be an array")
    })?;
    let result = values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                format!("WEAPONRY_KNIFE_PROFILE_INVALID: {facade_name}.{key} contains a non-string")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result.len() > 32 || result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(format!(
            "WEAPONRY_KNIFE_PROFILE_INVALID: {facade_name}.{key} is duplicated or exceeds 32 entries"
        ));
    }
    Ok(result)
}

fn parse_native_specs() -> Result<Vec<NativeSpec>, String> {
    let profile: Value = serde_json::from_str(PROFILE_JSON)
        .map_err(|error| format!("WEAPONRY_KNIFE_PROFILE_INVALID: {error}"))?;
    let native = profile
        .get("native_operations")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "WEAPONRY_KNIFE_PROFILE_INVALID: native_operations must be an object".to_owned()
        })?;
    let expected = [
        ("knife_curve_modifier_graph_get", true),
        ("knife_curve_modifier_graph_prepare", false),
        ("knife_curve_evaluated_mesh_get", true),
        ("knife_curve_evaluated_mesh_prepare", false),
    ];
    if native.len() != expected.len() {
        return Err(
            "WEAPONRY_KNIFE_PROFILE_INVALID: native operation set is not the closed knife slice"
                .to_owned(),
        );
    }
    expected
        .into_iter()
        .map(|(operation, read_only)| {
            let value = native.get(operation).and_then(Value::as_object).ok_or_else(|| {
                format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: native operation {operation} is missing"
                )
            })?;
            if value.get("operation_name").and_then(Value::as_str) != Some(operation)
                || value.get("classification").and_then(Value::as_str)
                    != Some(if read_only { "read" } else { "write" })
                || value.get("facade_name").and_then(Value::as_str)
                    != Some("authoring_transaction")
                || value.get("status").and_then(Value::as_str) != Some("native-development-only")
            {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: native operation {operation} metadata drifted"
                ));
            }
            let request_schema = value
                .get("request_schema")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_INVALID: native operation {operation} request schema missing"
                    )
                })?;
            let result_schema = value
                .get("result_schema")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_INVALID: native operation {operation} result schema missing"
                    )
                })?;
            let expected_request_schema = match operation {
                "knife_curve_modifier_graph_get" => "KnifeCurveModifierGraphGetRequest@1",
                "knife_curve_modifier_graph_prepare" => {
                    "KnifeCurveModifierGraphPrepareRequest@1"
                }
                "knife_curve_evaluated_mesh_get" => "KnifeCurveEvaluatedMeshGetRequest@1",
                "knife_curve_evaluated_mesh_prepare" => {
                    "KnifeCurveEvaluatedMeshPrepareRequest@1"
                }
                _ => unreachable!("native operation set is closed above"),
            };
            let expected_result_schema = match operation {
                "knife_curve_modifier_graph_get" | "knife_curve_modifier_graph_prepare" => {
                    "KnifeCurveModifierGraphResult@1"
                }
                "knife_curve_evaluated_mesh_get" | "knife_curve_evaluated_mesh_prepare" => {
                    "KnifeCurveEvaluatedMeshResult@1"
                }
                _ => unreachable!("native operation set is closed above"),
            };
            if request_schema != expected_request_schema || result_schema != expected_result_schema {
                return Err(format!(
                    "WEAPONRY_KNIFE_PROFILE_INVALID: native operation {operation} schema metadata drifted"
                ));
            }
            Ok(NativeSpec {
                operation: operation.to_owned(),
                read_only,
                facade_name: "authoring_transaction".to_owned(),
                request_schema: request_schema.to_owned(),
                result_schema: result_schema.to_owned(),
            })
        })
        .collect()
}

pub fn is_native_operation(operation: &str) -> bool {
    parse_native_specs()
        .map(|specs| specs.iter().any(|spec| spec.operation == operation))
        .unwrap_or(false)
}

pub fn native_operation_is_write(operation: &str) -> bool {
    parse_native_specs()
        .map(|specs| {
            specs
                .iter()
                .find(|spec| spec.operation == operation)
                .is_some_and(|spec| !spec.read_only)
        })
        .unwrap_or(false)
}

/// Return the write classification owned by the active knife profile.
///
/// This deliberately reads only the closed profile JSON and the four native
/// operation descriptors.  It must remain independent from the historical
/// compatibility registry so a default Knife request can be routed and
/// write-gated without constructing the 226-operation manifest.
pub fn is_write_operation(operation: &str) -> bool {
    if is_native_operation(operation) {
        return native_operation_is_write(operation);
    }
    parse_specs()
        .map(|specs| {
            specs.iter().any(|spec| {
                spec.write_operations
                    .iter()
                    .any(|candidate| candidate == operation)
            })
        })
        .unwrap_or(false)
}

fn native_specs_for_facade(facade_name: &str) -> Result<Vec<NativeSpec>, String> {
    Ok(parse_native_specs()?
        .into_iter()
        .filter(|spec| spec.facade_name == facade_name)
        .collect())
}

pub fn advertised_tools(
    profile: ToolProfile,
    _compatibility_tools: &[Value],
) -> Result<Vec<Value>, String> {
    if profile == ToolProfile::Compatibility {
        crate::compatibility_registry::ensure_enabled()?;
        return validate_compatibility_tools(_compatibility_tools)
            .map(|_| _compatibility_tools.to_vec());
    }
    // The active knife profile is deliberately independent from the legacy
    // registry.  `compatibility_tools` is retained in this API only so the
    // explicit replay profile and existing callers remain source-compatible;
    // the default path must not require callers to construct the 226-tool
    // manifest before projecting its eleven façades.
    active_tools()
}

fn validate_compatibility_tools(tools: &[Value]) -> Result<BTreeMap<String, Value>, String> {
    let mut by_name = BTreeMap::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "WEAPONRY_KNIFE_PROFILE_INVALID: legacy tool has no name".to_owned())?;
        if by_name.insert(name.to_owned(), tool.clone()).is_some() {
            return Err(format!(
                "WEAPONRY_KNIFE_PROFILE_INVALID: duplicate legacy tool {name}"
            ));
        }
    }
    Ok(by_name)
}

fn facade_tools(compatibility_tools: &[Value]) -> Result<Vec<Value>, String> {
    let by_name = validate_compatibility_tools(compatibility_tools)?;
    parse_specs()?
        .into_iter()
        .map(|spec| {
            let native_specs = native_specs_for_facade(&spec.name)?;
            let mut alternatives = Vec::with_capacity(spec.operations.len() + native_specs.len());
            for operation in &spec.operations {
                // Native operations are listed in the façade's complete
                // underlying allowlist for reachability/hash accounting, but
                // their definitions come from the native schema modules below
                // rather than the 226-operation compatibility manifest.
                if native_specs
                    .iter()
                    .any(|native| native.operation == *operation)
                {
                    continue;
                }
                let underlying = by_name.get(operation).ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_DRIFT: {} references missing operation {operation}",
                        spec.name
                    )
                })?;
                let actual_read_only = underlying
                    .pointer("/annotations/readOnlyHint")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        format!(
                            "WEAPONRY_KNIFE_PROFILE_DRIFT: operation {operation} lacks readOnlyHint"
                        )
                    })?;
                let declared_read_only = spec.read_operations.iter().any(|item| item == operation);
                let declared_write = spec.write_operations.iter().any(|item| item == operation);
                if actual_read_only != declared_read_only || declared_write == declared_read_only {
                    return Err(format!(
                        "WEAPONRY_KNIFE_PROFILE_DRIFT: {} misclassifies operation {operation}",
                        spec.name
                    ));
                }
                let request_schema = underlying.get("inputSchema").cloned().ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_DRIFT: operation {operation} lacks inputSchema"
                    )
                })?;
                alternatives.push(json!({
                    "type":"object",
                    "required":["operation","request"],
                    "properties":{
                        "operation":{"const":operation},
                        "request":request_schema
                    },
                    "additionalProperties":false
                }));
            }
            for native in native_specs {
                let request_schema = if crate::knife_curve_modifier_graph_tools::is_tool(
                    &native.operation,
                ) {
                    crate::knife_curve_modifier_graph_tools::input_schema(&native.operation)
                } else {
                    crate::knife_curve_evaluated_mesh_tools::input_schema(&native.operation)
                }
                .ok_or_else(|| {
                    format!(
                        "WEAPONRY_KNIFE_PROFILE_DRIFT: native operation {} lacks its closed request schema",
                        native.operation
                    )
                })?;
                alternatives.push(json!({
                    "type":"object",
                    "required":["operation","request"],
                    "properties":{
                        "operation":{"const":native.operation},
                        "request":request_schema
                    },
                    "additionalProperties":false
                }));
            }
            let read_only = spec.write_operations.is_empty();
            Ok(json!({
                "name":spec.name,
                "description":format!("Weaponry knife workflow façade. Select exactly one closed, allowlisted {} operation; the request is validated against that operation's original MCP schema.", spec.name),
                "inputSchema":{"oneOf":alternatives},
                "annotations":{
                    "readOnlyHint":read_only,
                    "destructiveHint":false,
                    "idempotentHint":false,
                    "openWorldHint":false,
                    "writeIntent":!read_only
                },
                "_meta":{"forgecad":{
                    "availability":"available",
                    "profileId":KNIFE_PROFILE_ID,
                    "boundedFacade":true,
                    "underlyingOperationAllowlistSha256":spec.allowlist_sha256
                }}
            }))
        })
        .collect()
}

/// Build only the active knife façades.  This path intentionally does not
/// ask the compatibility registry for its 226 raw definitions.  The active
/// profile owns its bounded request envelopes. Operations without a closed
/// active Contract request schema are advertised as unsatisfiable and fail
/// before routing; Runtime parsers are not treated as MCP schema metadata.
pub fn active_tools() -> Result<Vec<Value>, String> {
    let (operation_count, closed_request_schema_count) = request_schema_coverage()?;
    let schema_blocked_request_count = operation_count - closed_request_schema_count;
    parse_specs()?
        .into_iter()
        .map(|spec| {
            let native_specs = native_specs_for_facade(&spec.name)?;
            let mut alternatives = Vec::with_capacity(spec.operations.len());
            for operation in &spec.operations {
                let request_schema = active_operation_schema(operation)?;
                alternatives.push(json!({
                    "type":"object",
                    "required":["operation","request"],
                    "properties":{
                        "operation":{"const":operation},
                        "request":request_schema
                    },
                    "additionalProperties":false
                }));
            }
            // Native operations are included in the profile allowlist but
            // are not represented in `legacy_operations`; append their exact
            // closed request schemas here without loading the raw manifest.
            for native in native_specs {
                if spec.operations.iter().any(|operation| operation == &native.operation) {
                    continue;
                }
                let request_schema = active_operation_schema(&native.operation)?;
                alternatives.push(json!({
                    "type":"object",
                    "required":["operation","request"],
                    "properties":{
                        "operation":{"const":native.operation},
                        "request":request_schema
                    },
                    "additionalProperties":false
                }));
            }
            let read_only = spec.write_operations.is_empty();
            Ok(json!({
                "name":spec.name,
                "description":format!("Weaponry knife workflow façade. Select exactly one allowlisted {} operation. The façade envelope and operation allowlist are closed; {closed_request_schema_count}/{operation_count} operations have executable closed request schemas and the remaining {schema_blocked_request_count} fail closed until Contracts publishes their request schemas.", spec.name),
                "inputSchema":{"oneOf":alternatives},
                "annotations":{
                    "readOnlyHint":read_only,
                    "destructiveHint":false,
                    "idempotentHint":false,
                    "openWorldHint":false,
                    "writeIntent":!read_only
                },
                "_meta":{"forgecad":{
                    "availability":"available",
                    "profileId":KNIFE_PROFILE_ID,
                    "boundedFacade":true,
                    "facadeEnvelopeClosed":true,
                    "operationAllowlistClosed":true,
                    "requestSchemaClosureStatus":if schema_blocked_request_count == 0 {"COMPLETE"} else {"PARTIAL"},
                    "closedRequestSchemaCount":closed_request_schema_count,
                    "executableOperationCount":closed_request_schema_count,
                    "schemaBlockedRequestCount":schema_blocked_request_count,
                    "runtimeValidatedRequestSchemaCount":0,
                    "underlyingOperationAllowlistSha256":spec.allowlist_sha256,
                    "requestSchemaSource":"active-knife-registry"
                }}
            }))
        })
        .collect()
}

fn request_schema_coverage() -> Result<(usize, usize), String> {
    let specs = parse_specs()?;
    let native = parse_native_specs()?;
    let mut operations = specs
        .iter()
        .flat_map(|spec| spec.operations.iter().cloned())
        .collect::<BTreeSet<_>>();
    operations.extend(native.into_iter().map(|spec| spec.operation));
    let mut closed = 0;
    for operation in &operations {
        if crate::active_schema::is_closed(operation)? {
            closed += 1;
        }
    }
    Ok((operations.len(), closed))
}

/// Build the default Knife manifest summary without loading compatibility
/// schemas.  The legacy count and hashes are copied from the closed profile
/// as declared bindings; they are not runtime proof that the compatibility
/// registry is available or freshly rebuilt.
pub fn active_manifest_summary() -> Result<Value, String> {
    let facades = active_tools()?;
    let (operation_count, closed_request_schema_count) = request_schema_coverage()?;
    let schema_blocked_request_count = operation_count - closed_request_schema_count;
    let profile: Value = serde_json::from_str(PROFILE_JSON)
        .map_err(|error| format!("WEAPONRY_KNIFE_PROFILE_INVALID: {error}"))?;
    let specs = parse_specs()?;
    let mapping = specs
        .iter()
        .map(|spec| (spec.name.clone(), json!(spec.operations)))
        .collect::<Map<String, Value>>();
    let native_mapping = parse_native_specs()?
        .into_iter()
        .map(|spec| {
            (
                spec.operation.clone(),
                json!({
                    "operation_name": spec.operation,
                    "classification": if spec.read_only { "read" } else { "write" },
                    "facade_name": spec.facade_name,
                    "request_schema": spec.request_schema,
                    "result_schema": spec.result_schema,
                    "status": "native-development-only",
                }),
            )
        })
        .collect::<Map<String, Value>>();
    let legacy = profile
        .pointer("/compatibility_profile/legacy_manifest")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let legacy_count = legacy
        .get("total_count")
        .cloned()
        .unwrap_or_else(|| Value::from(226_u64));
    let mut summary = json!({
        "schema_version":"WeaponryKnifeToolManifestSummary@1",
        "default_profile_id":KNIFE_PROFILE_ID,
        "compatibility_profile_id":COMPATIBILITY_PROFILE_ID,
        "default_tool_count":facades.len(),
        "default_tool_names":facades.iter().filter_map(|tool| tool.get("name").cloned()).collect::<Vec<_>>(),
        "default_manifest_sha256":canonical_json_hash(&json!({"tools":facades})),
        "active_operation_count":operation_count,
        "closed_request_schema_count":closed_request_schema_count,
        "executable_operation_count":closed_request_schema_count,
        "schema_blocked_request_count":schema_blocked_request_count,
        "runtime_validated_request_schema_count":0,
        "request_schema_closure_status":if schema_blocked_request_count == 0 {"COMPLETE"} else {"PARTIAL"},
        "underlying_operation_allowlist_sha256":canonical_json_hash(&Value::Object(mapping)),
        "native_operation_allowlist_sha256":canonical_json_hash(&Value::Object(native_mapping)),
        "compatibility_tool_count":legacy_count,
        "compatibility_read_count":legacy.get("read_count").cloned().unwrap_or(Value::Null),
        "compatibility_write_count":legacy.get("write_count").cloned().unwrap_or(Value::Null),
        "compatibility_manifest_sha256":Value::Null,
        "compatibility_declared_read_manifest_sha256":legacy.get("read_manifest_sha256").cloned().unwrap_or(Value::Null),
        "compatibility_declared_write_enabled_manifest_sha256":legacy.get("write_enabled_manifest_sha256").cloned().unwrap_or(Value::Null),
        "compatibility_declared_summary_sha256":legacy.get("canonical_sha256").cloned().unwrap_or(Value::Null),
        "compatibility_profile_available":cfg!(feature = "legacy-compatibility-registry"),
        "compatibility_requires_explicit_profile":true
    });
    summary["canonical_sha256"] = Value::String(canonical_json_hash(&summary));
    Ok(summary)
}

/// Return the active knife request schema without constructing the legacy
/// compatibility manifest.  Most operation adapters perform a stricter
/// Runtime-side validation; this outer schema keeps the MCP envelope typed
/// and rejects the historically dangerous extra fields on no-argument calls.
pub fn active_operation_schema(operation: &str) -> Result<Value, String> {
    crate::active_schema::advertised_schema(operation)
}

/// Validate an operation request after the active façade has selected it.
/// Runtime parsers are not used as an MCP schema fallback: missing Contract
/// metadata returns a typed fail-closed error from `active_schema`.
pub(crate) fn validate_active_operation_request(
    operation: &str,
    request: &Value,
) -> Result<(), String> {
    crate::active_schema::validate(operation, request)
}

pub fn contains_operation(operation: &str) -> bool {
    parse_specs()
        .map(|specs| {
            specs
                .iter()
                .any(|spec| spec.operations.iter().any(|item| item == operation))
        })
        .unwrap_or(false)
}

pub fn compatibility_tool_count() -> usize {
    serde_json::from_str::<Value>(PROFILE_JSON)
        .ok()
        .and_then(|profile| {
            profile
                .pointer("/compatibility_profile/legacy_manifest/total_count")
                .and_then(Value::as_u64)
        })
        .unwrap_or(226) as usize
}

pub fn unwrap_facade_call(
    profile: ToolProfile,
    requested_name: &str,
    arguments: &Value,
) -> Result<(String, Value), String> {
    if profile == ToolProfile::Compatibility {
        crate::compatibility_registry::ensure_enabled()?;
        if is_native_operation(requested_name) {
            return Err(format!(
                "WEAPONRY_KNIFE_PROFILE_TOOL_HIDDEN: {requested_name} is façade-native and is not part of the 226-operation compatibility replay"
            ));
        }
        return Ok((requested_name.to_owned(), arguments.clone()));
    }
    if !FACADE_NAMES.contains(&requested_name) {
        return Err(format!(
            "WEAPONRY_KNIFE_PROFILE_TOOL_HIDDEN: {requested_name} is available only in the explicit compatibility profile"
        ));
    }
    let object = arguments.as_object().ok_or_else(|| {
        "WEAPONRY_KNIFE_PROFILE_INVALID: façade arguments must be an object".to_owned()
    })?;
    if object.len() != 2 || !object.contains_key("operation") || !object.contains_key("request") {
        return Err(
            "WEAPONRY_KNIFE_PROFILE_INVALID: façade arguments require only operation and request"
                .to_owned(),
        );
    }
    let operation = object
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| "WEAPONRY_KNIFE_PROFILE_INVALID: operation must be a string".to_owned())?;
    let request = object
        .get("request")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| "WEAPONRY_KNIFE_PROFILE_INVALID: request must be an object".to_owned())?;
    let spec = parse_specs()?
        .into_iter()
        .find(|spec| spec.name == requested_name)
        .ok_or_else(|| "WEAPONRY_KNIFE_PROFILE_INVALID: façade is not declared".to_owned())?;
    let native_allowed = requested_name == "authoring_transaction"
        && parse_native_specs()?
            .iter()
            .any(|native| native.operation == operation);
    if !spec.operations.iter().any(|allowed| allowed == operation) && !native_allowed {
        return Err(format!(
            "WEAPONRY_KNIFE_PROFILE_ROUTE_DENIED: {operation} is not allowed through {requested_name}"
        ));
    }
    Ok((operation.to_owned(), request))
}

pub fn manifest_summary(compatibility_tools: &[Value]) -> Result<Value, String> {
    crate::compatibility_registry::ensure_enabled()?;
    let facades = facade_tools(compatibility_tools)?;
    let compatibility = validate_compatibility_tools(compatibility_tools)?;
    let specs = parse_specs()?;
    let mapping = specs
        .iter()
        .map(|spec| (spec.name.clone(), json!(spec.operations)))
        .collect::<Map<String, Value>>();
    let native_specs = parse_native_specs()?;
    let native_mapping = native_specs
        .iter()
        .map(|spec| {
            (
                spec.operation.clone(),
                json!({
                    "operation_name": spec.operation,
                    "classification": if spec.read_only { "read" } else { "write" },
                    "facade_name": spec.facade_name,
                    "request_schema": spec.request_schema,
                    "result_schema": spec.result_schema,
                    "status": "native-development-only",
                }),
            )
        })
        .collect::<Map<String, Value>>();
    let mut summary = json!({
        "schema_version":"WeaponryKnifeToolManifestSummary@1",
        "default_profile_id":KNIFE_PROFILE_ID,
        "compatibility_profile_id":COMPATIBILITY_PROFILE_ID,
        "default_tool_count":facades.len(),
        "default_tool_names":facades.iter().filter_map(|tool| tool.get("name").cloned()).collect::<Vec<_>>(),
        "default_manifest_sha256":canonical_json_hash(&json!({"tools":facades})),
        "underlying_operation_allowlist_sha256":canonical_json_hash(&Value::Object(mapping)),
        "native_operation_allowlist_sha256":canonical_json_hash(&Value::Object(native_mapping)),
        "compatibility_tool_count":compatibility.len(),
        "compatibility_manifest_sha256":canonical_json_hash(&json!({"tools":compatibility_tools})),
        "compatibility_requires_explicit_profile":true
    });
    summary["canonical_sha256"] = Value::String(canonical_json_hash(&summary));
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_environment_is_closed() {
        assert_eq!(ToolProfile::Knife.id(), KNIFE_PROFILE_ID);
        assert_eq!(ToolProfile::Compatibility.id(), COMPATIBILITY_PROFILE_ID);
        assert_eq!(FACADE_NAMES.len(), 11);
        assert!(parse_specs().is_ok());
    }

    #[test]
    fn active_profile_builds_only_the_eleven_closed_allowlist_facades() {
        let tools = active_tools().expect("active knife registry is valid");
        assert_eq!(tools.len(), FACADE_NAMES.len());
        let names = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(names, FACADE_NAMES);
        assert!(tools.iter().all(|tool| {
            tool.pointer("/_meta/forgecad/requestSchemaSource")
                .and_then(Value::as_str)
                == Some("active-knife-registry")
        }));
        assert!(tools.iter().all(|tool| {
            tool.pointer("/_meta/forgecad/requestSchemaClosureStatus")
                .and_then(Value::as_str)
                == Some("COMPLETE")
        }));
        assert!(!tools
            .iter()
            .any(|tool| { tool.get("name").and_then(Value::as_str) == Some("project_create") }));
    }

    #[test]
    fn active_summary_is_independent_from_the_legacy_manifest() {
        let summary = active_manifest_summary().expect("active knife summary is valid");
        assert_eq!(summary["default_tool_count"], 11);
        assert_eq!(
            summary["default_tool_names"].as_array().map(Vec::len),
            Some(11)
        );
        assert_eq!(summary["compatibility_manifest_sha256"], Value::Null);
        assert_eq!(summary["compatibility_requires_explicit_profile"], true);
        assert_eq!(summary["active_operation_count"], 125);
        assert_eq!(summary["closed_request_schema_count"], 125);
        assert_eq!(summary["executable_operation_count"], 125);
        assert_eq!(summary["schema_blocked_request_count"], 0);
        assert_eq!(summary["runtime_validated_request_schema_count"], 0);
        assert_eq!(summary["request_schema_closure_status"], "COMPLETE");
        assert!(summary["canonical_sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64));
    }

    #[test]
    fn every_active_operation_has_a_closed_package_request_schema() {
        let specs = parse_specs().expect("active façade profile parses");
        let native = parse_native_specs().expect("native active profile parses");
        let mut operations = specs
            .iter()
            .flat_map(|spec| spec.operations.iter().cloned())
            .collect::<BTreeSet<_>>();
        operations.extend(native.into_iter().map(|spec| spec.operation));
        assert_eq!(operations.len(), 125);

        for operation in operations {
            assert!(
                crate::active_schema::is_closed(&operation)
                    .unwrap_or_else(|error| panic!("{operation} schema is invalid: {error}")),
                "{operation} must resolve to a root-closed package schema"
            );
            let advertised = active_operation_schema(&operation)
                .unwrap_or_else(|error| panic!("{operation} cannot be advertised: {error}"));
            assert_eq!(
                advertised.get("additionalProperties"),
                Some(&Value::Bool(false)),
                "{operation} advertised schema must remain closed"
            );
        }
    }

    #[test]
    fn knife_profile_owns_native_write_classification() {
        assert!(!is_write_operation("knife_curve_modifier_graph_get"));
        assert!(is_write_operation("knife_curve_modifier_graph_prepare"));
        assert!(!is_write_operation("knife_curve_evaluated_mesh_get"));
        assert!(is_write_operation("knife_curve_evaluated_mesh_prepare"));
        assert!(is_write_operation("project_create"));
        assert!(!is_write_operation("runtime_status"));
    }

    #[test]
    fn facade_unwrap_is_exact_and_cross_facade_routes_fail() {
        let request = json!({
            "operation":"runtime_status",
            "request":{}
        });
        assert_eq!(
            unwrap_facade_call(ToolProfile::Knife, "weapon_preflight", &request).unwrap(),
            ("runtime_status".to_owned(), json!({}))
        );
        assert!(unwrap_facade_call(ToolProfile::Knife, "approval", &request).is_err());
        let mut extra = request;
        extra["path"] = json!("forbidden");
        assert!(unwrap_facade_call(ToolProfile::Knife, "weapon_preflight", &extra).is_err());
        assert!(unwrap_facade_call(ToolProfile::Knife, "runtime_status", &json!({})).is_err());
    }
}
