use serde_json::{json, Map, Value};

const READ_FIELDS: [&str; 3] = ["project_id", "candidate_id", "job_id"];
const PREPARE_FIELDS: [&str; 9] = [
    "project_id",
    "candidate_id",
    "intent",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];
const RESUME_FIELDS: [&str; 9] = [
    "project_id",
    "candidate_id",
    "job_id",
    "approved",
    "approval_receipt_id",
    "approval_summary",
    "approval_expires_at",
    "approval_session_id",
    "idempotency_key",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationTool {
    Get,
    Prepare,
    Resume,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub project_id: Option<String>,
    pub candidate_id: Option<String>,
    pub job_id: Option<String>,
}

impl OptimizationTool {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "optimization_job_get" => Self::Get,
            "optimization_job_prepare" => Self::Prepare,
            "optimization_job_resume" => Self::Resume,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "optimization_job_get",
            Self::Prepare => "optimization_job_prepare",
            Self::Resume => "optimization_job_resume",
        }
    }

    pub const fn is_write(self) -> bool {
        !matches!(self, Self::Get)
    }

    pub const fn runtime_method(self) -> &'static str {
        self.name()
    }
}

pub fn is_tool(name: &str) -> bool {
    OptimizationTool::from_name(name).is_some()
}

pub fn is_write_tool(name: &str) -> bool {
    OptimizationTool::from_name(name).is_some_and(OptimizationTool::is_write)
}

pub fn runtime_method(name: &str) -> Option<&'static str> {
    OptimizationTool::from_name(name).map(OptimizationTool::runtime_method)
}

pub fn unavailable_error(name: &str) -> String {
    format!("OPTIMIZATION_RUNTIME_METHOD_UNAVAILABLE: {name} requires Runtime method {name}")
}

pub fn read_tool_names() -> Vec<String> {
    vec![OptimizationTool::Get.name().to_owned()]
}

pub fn write_tool_names() -> Vec<String> {
    vec![
        OptimizationTool::Prepare.name().to_owned(),
        OptimizationTool::Resume.name().to_owned(),
    ]
}

pub fn read_tools() -> Vec<Value> {
    vec![tool_definition(OptimizationTool::Get)]
}

pub fn write_tools() -> Vec<Value> {
    vec![
        tool_definition(OptimizationTool::Prepare),
        tool_definition(OptimizationTool::Resume),
    ]
}

pub fn all_tools() -> Vec<Value> {
    read_tools().into_iter().chain(write_tools()).collect()
}

fn tool_definition(tool: OptimizationTool) -> Value {
    let (description, schema) = match tool {
        OptimizationTool::Get => (
            "Read one durable CADFit-style multi-fidelity OptimizationJob, its best-so-far checkpoint and proposal status.",
            read_schema(),
        ),
        OptimizationTool::Prepare => (
            "Start one approved, asynchronous, Runtime-owned multi-fidelity optimization over one typed Part; it never confirms or mutates a candidate version.",
            prepare_schema(),
        ),
        OptimizationTool::Resume => (
            "Recover an interrupted or cancelled OptimizationJob from its immutable intent and last CAS checkpoint; the Runtime reuses validated evaluations, continues only unfinished fidelity stages and never confirms a proposal.",
            resume_schema(),
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
            "writeIntent":tool.is_write(),
            "approvalRequired":tool.is_write()
        },
        "_meta":{"forgecad":{
            "availability":"available",
            "runtime_method":tool.runtime_method(),
            "requiresConfirmation":tool.is_write(),
            "transaction":"RuntimeJob+CAS+checkpoint",
            "definition_only":false
        }}
    })
}

fn read_schema() -> Value {
    object_schema(
        READ_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("job_id".to_owned(), id_property()),
        ]),
    )
}

fn prepare_schema() -> Value {
    object_schema(
        PREPARE_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            (
                "intent".to_owned(),
                json!({"oneOf":[intent_property(), intent_v2_property()]}),
            ),
            ("approved".to_owned(), json!({"const":true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

fn resume_schema() -> Value {
    object_schema(
        RESUME_FIELDS.to_vec(),
        Map::from_iter([
            ("project_id".to_owned(), id_property()),
            ("candidate_id".to_owned(), id_property()),
            ("job_id".to_owned(), id_property()),
            ("approved".to_owned(), json!({"const":true})),
            ("approval_receipt_id".to_owned(), id_property()),
            ("approval_summary".to_owned(), safe_text_property(512)),
            ("approval_expires_at".to_owned(), safe_text_property(64)),
            ("approval_session_id".to_owned(), id_property()),
            ("idempotency_key".to_owned(), id_property()),
        ]),
    )
}

pub(crate) fn intent_property() -> Value {
    json!({
        "type":"object",
        "required":["schema_version","intent_id","job_id","project_id","candidate_id","reference_id","reference_sha256","program_sha256","target_sha256","camera","camera_hash","part_id","stage","rig","fidelity","budget","objective","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"OptimizationIntent@1"},
            "intent_id":id_property(),
            "action_run_id":{"type":["string","null"],"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
            "job_id":id_property(),
            "project_id":id_property(),
            "candidate_id":id_property(),
            "reference_id":id_property(),
            "reference_sha256":sha_property(),
            "program_sha256":sha_property(),
            "target_sha256":sha_property(),
            "evaluation_objective_sha256":sha_property(),
            "camera":{"type":"object"},
            "camera_hash":sha_property(),
            "part_id":id_property(),
            "stage":{"enum":["primary-form","secondary-structure","tertiary-detail","uv-pbr","final-review"]},
            "rig":{"type":"object"},
            "fidelity":{"type":"object"},
            "budget":{"type":"object"},
            "objective":{"type":"object"},
            "residual":residual_property(),
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn intent_v2_property() -> Value {
    // The MCP declaration is intentionally bounded but not a second copy of
    // the full Runtime contract.  Runtime owns the authoritative V2
    // validation; this surface only rejects malformed top-level transport
    // shapes and keeps the Codex tool manifest deterministic.
    json!({
        "type":"object",
        "required":["schema_version","intent_id","job_id","project_id","candidate_id","reference_id","reference_sha256","program_sha256","camera_rig_sha256","camera_rig","views","part_id","target_part_ids","stage","rig","fidelity","budget","objective","canonical_sha256"],
        "properties":{
            "schema_version":{"const":"OptimizationIntent@2"},
            "intent_id":id_property(),
            "action_run_id":{"type":["string","null"],"pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"},
            "job_id":id_property(),
            "project_id":id_property(),
            "candidate_id":id_property(),
            "reference_id":id_property(),
            "reference_sha256":sha_property(),
            "program_sha256":sha_property(),
            "camera_rig_sha256":sha_property(),
            "camera_rig":{"type":"object"},
            "views":{
                "type":"array",
                "minItems":6,
                "maxItems":8,
                "items":{
                    "type":"object",
                    "required":["view_id","kind","target_sha256","camera","camera_hash","weight","primary"],
                    "properties":{
                        "view_id":id_property(),
                        "kind":{"enum":["left","right","top","bottom","front","back","front-three-quarter","rear-three-quarter"]},
                        "target_sha256":sha_property(),
                        "camera":{"type":"object"},
                        "camera_hash":sha_property(),
                        "weight":{"type":"number","exclusiveMinimum":0.0,"maximum":1.0},
                        "primary":{"type":"boolean"}
                    },
                    "additionalProperties":false
                }
            },
            "part_id":id_property(),
            "target_part_ids":{
                "type":"array",
                "minItems":1,
                "maxItems":64,
                "uniqueItems":true,
                "items":id_property()
            },
            "stage":{"enum":["primary-form","secondary-structure","tertiary-detail","uv-pbr","final-review"]},
            "rig":{"type":"object"},
            "fidelity":{"type":"object"},
            "budget":{"type":"object"},
            "objective":{"type":"object"},
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn residual_property() -> Value {
    json!({
        "type":"object",
        "required":[
            "schema_version",
            "part_id",
            "node_id",
            "operation",
            "parameters",
            "source_critic_report_sha256",
            "source_part_error_sha256",
            "canonical_sha256"
        ],
        "properties":{
            "schema_version":{"const":"OptimizationResidual@1"},
            "part_id":id_property(),
            "node_id":id_property(),
            "operation":{"enum":["union","difference","intersection"]},
            "parameters":primitive_parameters_property(),
            "source_critic_report_sha256":sha_property(),
            "source_part_error_sha256":sha_property(),
            "source_visual_surface_sha256":sha_property(),
            "canonical_sha256":sha_property()
        },
        "additionalProperties":false
    })
}

fn primitive_parameters_property() -> Value {
    json!({
        "oneOf":[
            {
                "type":"object",
                "required":["shape","size_m","position_m","rotation_rad"],
                "properties":{
                    "shape":{"const":"box"},
                    "size_m":vec3_property(0.0,10.0),
                    "position_m":vec3_property(-10.0,10.0),
                    "rotation_rad":vec3_property(-6.283185307179586,6.283185307179586)
                },
                "additionalProperties":false
            },
            {
                "type":"object",
                "required":["shape","radius_m","height_m","radial_segments","position_m","rotation_rad"],
                "properties":{
                    "shape":{"const":"cylinder"},
                    "radius_m":{"type":"number","minimum":0.0000001,"maximum":5.0},
                    "height_m":{"type":"number","minimum":0.0000001,"maximum":10.0},
                    "radial_segments":{"type":"integer","minimum":8,"maximum":64},
                    "position_m":vec3_property(-10.0,10.0),
                    "rotation_rad":vec3_property(-6.283185307179586,6.283185307179586)
                },
                "additionalProperties":false
            },
            {
                "type":"object",
                "required":["shape","radii_m","longitude_segments","latitude_segments","position_m","rotation_rad"],
                "properties":{
                    "shape":{"const":"ellipsoid"},
                    "radii_m":vec3_property(0.0,5.0),
                    "longitude_segments":{"type":"integer","minimum":8,"maximum":64},
                    "latitude_segments":{"type":"integer","minimum":4,"maximum":64},
                    "position_m":vec3_property(-10.0,10.0),
                    "rotation_rad":vec3_property(-6.283185307179586,6.283185307179586)
                },
                "additionalProperties":false
            },
            {
                "type":"object",
                "required":["shape","radius_m","longitude_segments","latitude_segments","position_m","rotation_rad"],
                "properties":{
                    "shape":{"const":"sphere"},
                    "radius_m":{"type":"number","minimum":0.0000001,"maximum":5.0},
                    "longitude_segments":{"type":"integer","minimum":8,"maximum":64},
                    "latitude_segments":{"type":"integer","minimum":4,"maximum":64},
                    "position_m":vec3_property(-10.0,10.0),
                    "rotation_rad":vec3_property(-6.283185307179586,6.283185307179586)
                },
                "additionalProperties":false
            }
        ]
    })
}

fn vec3_property(minimum: f64, maximum: f64) -> Value {
    json!({
        "type":"array",
        "minItems":3,
        "maxItems":3,
        "items":{"type":"number","minimum":minimum,"maximum":maximum}
    })
}

fn object_schema(required: Vec<&str>, properties: Map<String, Value>) -> Value {
    json!({"type":"object","required":required,"properties":properties,"additionalProperties":false})
}

fn id_property() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$"})
}

fn sha_property() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{64}$"})
}

fn safe_text_property(max_length: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max_length})
}

pub fn validate_call(name: &str, arguments: &Value, binding: &Binding) -> Result<(), String> {
    let Some(tool) = OptimizationTool::from_name(name) else {
        return Ok(());
    };
    let object = arguments
        .as_object()
        .ok_or_else(|| "OPTIMIZATION_INVALID_INPUT: arguments must be an object".to_owned())?;
    let allowed = match tool {
        OptimizationTool::Get => &READ_FIELDS[..],
        OptimizationTool::Prepare => &PREPARE_FIELDS[..],
        OptimizationTool::Resume => &RESUME_FIELDS[..],
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("OPTIMIZATION_INVALID_INPUT: unknown field".to_owned());
    }
    let project_id = required_id(object, "project_id")?;
    let candidate_id = required_id(object, "candidate_id")?;
    let job_id = match tool {
        OptimizationTool::Prepare => object
            .get("intent")
            .and_then(|intent| intent.get("job_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| "OPTIMIZATION_INVALID_INPUT: intent.job_id is required".to_owned())?,
        _ => required_id(object, "job_id")?,
    };
    for (expected, actual) in [
        (binding.project_id.as_deref(), project_id),
        (binding.candidate_id.as_deref(), candidate_id),
        (binding.job_id.as_deref(), job_id),
    ] {
        if expected.is_some_and(|expected| expected != actual) {
            return Err("OPTIMIZATION_SCOPE_MISMATCH: request is outside the bound job".to_owned());
        }
    }
    if tool.is_write() {
        if object.get("approved") != Some(&Value::Bool(true)) {
            return Err("OPTIMIZATION_APPROVAL_REQUIRED: approved=true is required".to_owned());
        }
        for key in [
            "approval_receipt_id",
            "approval_summary",
            "approval_expires_at",
            "approval_session_id",
            "idempotency_key",
        ] {
            if object.get(key).is_none() {
                return Err(format!("OPTIMIZATION_APPROVAL_REQUIRED: {key} is required"));
            }
        }
    }
    Ok(())
}

pub fn validate_response(name: &str, value: &Value, binding: &Binding) -> Result<(), String> {
    if !is_tool(name) {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "OPTIMIZATION_RESPONSE_INVALID: response must be an object".to_owned())?;
    let job = object
        .get("job")
        .and_then(Value::as_object)
        .ok_or_else(|| "OPTIMIZATION_RESPONSE_INVALID: job summary is missing".to_owned())?;
    let project_id = job
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "OPTIMIZATION_RESPONSE_INVALID: job.project_id is missing".to_owned())?;
    let job_id = job
        .get("job_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "OPTIMIZATION_RESPONSE_INVALID: job.job_id is missing".to_owned())?;
    for (expected, actual) in [
        (binding.project_id.as_deref(), project_id),
        (binding.job_id.as_deref(), job_id),
    ] {
        if expected.is_some_and(|expected| expected != actual) {
            return Err("OPTIMIZATION_RESPONSE_SCOPE_MISMATCH".to_owned());
        }
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("OptimizationJobResult@1") {
        return Err("OPTIMIZATION_RESPONSE_INVALID: schema_version".to_owned());
    }
    Ok(())
}

pub fn bind_response(name: &str, value: &Value, binding: &mut Binding) -> Result<(), String> {
    validate_response(name, value, binding)?;
    let job = value["job"]
        .as_object()
        .ok_or_else(|| "OPTIMIZATION_RESPONSE_INVALID: job summary is missing".to_owned())?;
    for (key, slot) in [
        ("project_id", &mut binding.project_id),
        ("job_id", &mut binding.job_id),
    ] {
        let actual = job[key]
            .as_str()
            .ok_or_else(|| format!("OPTIMIZATION_RESPONSE_INVALID: job.{key} is missing"))?;
        if slot.as_deref().is_some_and(|expected| expected != actual) {
            return Err("OPTIMIZATION_RESPONSE_SCOPE_MISMATCH".to_owned());
        }
        *slot = Some(actual.to_owned());
    }
    if let Some(candidate_id) = value
        .get("result")
        .and_then(Value::as_object)
        .and_then(|_| binding.candidate_id.clone())
    {
        binding.candidate_id = Some(candidate_id);
    }
    Ok(())
}

fn required_id<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("OPTIMIZATION_INVALID_INPUT: {key} is required"))?;
    if value.is_empty() {
        return Err(format!("OPTIMIZATION_INVALID_INPUT: {key} is empty"));
    }
    Ok(value)
}
