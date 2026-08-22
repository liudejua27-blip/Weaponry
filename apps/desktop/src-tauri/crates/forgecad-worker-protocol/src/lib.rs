use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const WORKER_PROTOCOL: &str = "forgecad-worker-protocol@1";
pub const MATERIAL_PACK_ID: &str = "forgecad-hard-surface-robot";
const MATERIAL_PACK_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../..//packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json"
));
/// The isolated Worker accepts a single bounded JSON request. Geometry
/// programs are small, but `render_glb` carries the self-contained candidate
/// (including up to 64 MiB of embedded PNGs) as base64. Keep the framing
/// bounded while leaving room for that documented product maximum.
pub const MAX_WORKER_REQUEST_BYTES: usize = 96 * 1024 * 1024;
/// A 64 MiB GLB is base64 encoded in the internal response, so the response
/// envelope needs modest headroom while still being decisively bounded.
pub const MAX_WORKER_RESPONSE_BYTES: usize = 96 * 1024 * 1024;
pub const MAX_WORKER_STDERR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequest {
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResponse {
    pub protocol: String,
    pub request_id: String,
    /// The Runtime and the fixed sibling worker must be built in the same
    /// development cohort when a cohort is present. `null` is valid for
    /// ordinary source builds that intentionally omit a cohort.
    pub build_cohort_sha256: Option<String>,
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<WorkerError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerError {
    pub code: String,
    pub message: String,
}

/// The catalog is shared protocol data rather than a Runtime-owned mirror of
/// executable Worker state. The Worker validates against this closed value and
/// the Runtime exposes the exact same canonical JSON through its read path.
///
/// The Boolean operator is product-owned and is compiled through the fixed
/// Manifold C API bridge in the isolated Geometry Worker.  Only the bounded
/// union/difference/intersection same-Part slice is active in P0; no arbitrary script or
/// plugin can add an operator at runtime.
pub fn operator_catalog() -> Value {
    let mut catalog = json!({
        "schema_version":"OperatorCatalog@1",
        "catalog_id":"forgecad-mcp010d-hard-surface",
        "geometry_program_schema_version":"GeometryProgram@2",
        "operators":[
            {"operator_id":"forgecad.geometry.primitive@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"GeometryPrimitiveParameters@2","part_output_required":true,"supported_shapes":["box","cylinder","ellipsoid","sphere"]},
            {"operator_id":"forgecad.geometry.profile-extrude@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileExtrudeParameters@1","part_output_required":true,"supported_shapes":["profile-extrude"]},
            {"operator_id":"forgecad.geometry.profile-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileLoftParameters@1","part_output_required":true,"supported_shapes":["profile-loft"]},
            {"operator_id":"forgecad.geometry.profile-loft@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileLoftParameters@2","part_output_required":true,"supported_shapes":["profile-loft-v2"]},
            {"operator_id":"forgecad.geometry.multi-loop-profile-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"MultiLoopProfileLoftParameters@1","part_output_required":true,"supported_shapes":["multi-loop-profile-loft"]},
            {"operator_id":"forgecad.geometry.longitudinal-section-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"LongitudinalSectionLoftParameters@1","part_output_required":true,"supported_shapes":["longitudinal-section-loft"]},
            {"operator_id":"forgecad.geometry.subd-cage@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SubdCageParameters@1","part_output_required":true,"supported_shapes":["subd-cage"]},
            {"operator_id":"forgecad.geometry.surface-patch@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SurfacePatchParameters@1","part_output_required":true,"supported_shapes":["surface-patch"]},
            {"operator_id":"forgecad.geometry.surface-shell@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SurfaceShellParameters@1","part_output_required":true,"supported_shapes":["surface-shell"]},
            {"operator_id":"forgecad.geometry.revolve@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"RevolveParameters@1","part_output_required":true,"supported_shapes":["revolve"]},
            {"operator_id":"forgecad.geometry.tube-sweep@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"TubeSweepParameters@1","part_output_required":true,"supported_shapes":["tube-sweep"]},
            {"operator_id":"forgecad.geometry.transform@2","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"TransformParameters@2","part_output_required":true,"supported_shapes":["transform"]},
            {"operator_id":"forgecad.geometry.mirror@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"MirrorParameters@1","part_output_required":true,"supported_shapes":["mirror"]},
            {"operator_id":"forgecad.geometry.array@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"ArrayParameters@1","part_output_required":true,"supported_shapes":["array"]},
            {"operator_id":"forgecad.geometry.panel@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"PanelParameters@1","part_output_required":true,"supported_shapes":["panel"]},
            {"operator_id":"forgecad.geometry.vent-array@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"VentArrayParameters@1","part_output_required":true,"supported_shapes":["vent-array"]},
            {"operator_id":"forgecad.geometry.joint-stack@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"JointStackParameters@1","part_output_required":true,"supported_shapes":["joint-stack"]},
            {"operator_id":"forgecad.geometry.part-output@1","status":"active","input_arity":{"min":1,"max":64},"output_kind":"triangle-mesh","parameter_schema":"PartOutputParameters@1","part_output_required":true,"supported_shapes":["part-output"]},
            {"operator_id":"forgecad.geometry.boolean@1","status":"active","input_arity":{"min":2,"max":2},"output_kind":"triangle-mesh","parameter_schema":"BooleanParameters@1","part_output_required":true,"supported_shapes":["union","difference","intersection"]}
        ],
        "canonical_sha256":""
    });
    let mut without_hash = catalog
        .as_object()
        .expect("operator catalog is an object")
        .clone();
    without_hash.remove("canonical_sha256");
    catalog["canonical_sha256"] = Value::String(canonical_hash(&Value::Object(without_hash)));
    catalog
}

pub fn operator_catalog_sha256() -> String {
    operator_catalog()["canonical_sha256"]
        .as_str()
        .expect("operator catalog has a canonical hash")
        .to_owned()
}

/// Runtime-owned read-only material manifest.  The manifest is compiled into
/// the same source cohort as the Worker; it is never fetched from a URL or
/// resolved through a user path.  The checked-in canonical field is verified
/// against the same canonical JSON function used by the Worker.
pub fn material_pack_manifest() -> Value {
    let manifest: Value = serde_json::from_str(MATERIAL_PACK_MANIFEST_JSON)
        .expect("ForgeCAD material pack manifest is valid JSON");
    let expected = manifest
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .expect("ForgeCAD material pack manifest has a canonical hash");
    let mut without_hash = manifest
        .as_object()
        .expect("ForgeCAD material pack manifest is an object")
        .clone();
    without_hash.remove("canonical_sha256");
    let actual = canonical_hash(&Value::Object(without_hash));
    assert_eq!(
        expected, actual,
        "ForgeCAD material pack manifest hash drifted"
    );
    assert_eq!(
        manifest.get("pack_id").and_then(Value::as_str),
        Some(MATERIAL_PACK_ID)
    );
    manifest
}

pub fn material_pack_manifest_sha256() -> String {
    material_pack_manifest()["canonical_sha256"]
        .as_str()
        .expect("ForgeCAD material pack manifest hash")
        .to_owned()
}

pub fn build_cohort_sha256() -> Option<String> {
    option_env!("FORGECAD_BUILD_COHORT_SHA256")
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
}

pub fn validate_request(request: &WorkerRequest) -> Result<(), String> {
    if request.protocol != WORKER_PROTOCOL {
        return Err("worker protocol version is invalid".to_owned());
    }
    if !is_opaque_id(&request.request_id) {
        return Err("worker request_id is invalid".to_owned());
    }
    if !matches!(
        request.operation.as_str(),
        "compile_geometry"
            | "render_fixed"
            | "render_glb"
            | "render_glb_fit_batch"
            | "geometry_program_hash"
    ) {
        return Err("worker operation is not allowlisted".to_owned());
    }
    if !request.payload.is_object() {
        return Err("worker payload must be an object".to_owned());
    }
    Ok(())
}

pub fn validate_response(
    response: &WorkerResponse,
    expected_request_id: &str,
) -> Result<(), String> {
    if response.protocol != WORKER_PROTOCOL {
        return Err("worker response protocol is invalid".to_owned());
    }
    if response.request_id != expected_request_id || !is_opaque_id(&response.request_id) {
        return Err("worker response request_id is invalid".to_owned());
    }
    if response.ok {
        if response.result.is_none() || response.error.is_some() {
            return Err("successful worker response has an invalid result envelope".to_owned());
        }
    } else {
        let error = response
            .error
            .as_ref()
            .ok_or_else(|| "failed worker response lacks an error".to_owned())?;
        if response.result.is_some()
            || !is_opaque_id(&error.code)
            || error.message.is_empty()
            || error.message.len() > 512
        {
            return Err("failed worker response has an invalid error envelope".to_owned());
        }
    }
    if response
        .build_cohort_sha256
        .as_deref()
        .is_some_and(|value| !is_sha256(value))
    {
        return Err("worker response cohort is invalid".to_owned());
    }
    Ok(())
}

impl WorkerResponse {
    pub fn unavailable(request_id: String, worker: &str) -> Self {
        Self {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: false,
            result: None,
            error: Some(WorkerError {
                code: "CAPABILITY_UNAVAILABLE".to_owned(),
                message: format!("{worker} worker is not enabled in MCP001"),
            }),
        }
    }
}

fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn canonical_hash(value: &Value) -> String {
    let mut bytes = Vec::new();
    write_canonical(value, &mut bytes);
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            serde_json::to_writer(&mut *output, value).expect("string serializes")
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(value, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).expect("object key serializes");
                output.push(b':');
                write_canonical(&values[*key], output);
            }
            output.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_hash_is_stable() {
        let catalog = operator_catalog();
        assert_eq!(catalog["canonical_sha256"], operator_catalog_sha256());
        assert_eq!(operator_catalog(), catalog);
    }

    #[test]
    fn strict_request_and_response_envelopes_reject_drift() {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "request-1".to_owned(),
            operation: "compile_geometry".to_owned(),
            payload: json!({"geometry_program":{}}),
        };
        assert!(validate_request(&request).is_ok());
        let response = WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            build_cohort_sha256: None,
            ok: true,
            result: Some(json!({"schema_version":"GeometryWorkerResult@1"})),
            error: None,
        };
        assert!(validate_response(&response, &request.request_id).is_ok());
        let mut invalid = response;
        invalid.error = Some(WorkerError {
            code: "BAD".to_owned(),
            message: "drift".to_owned(),
        });
        assert!(validate_response(&invalid, "request-1").is_err());
    }
}
