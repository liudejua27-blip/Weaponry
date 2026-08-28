//! Runtime-owned launcher for the fixed geometry-worker sibling.
//!
//! The Runtime never accepts a Worker path, environment, command line or
//! script from MCP input. On macOS it resolves a same-directory (or Cargo test
//! `deps/..`) sibling and starts it with `posix_spawn` using
//! `POSIX_SPAWN_CLOEXEC_DEFAULT`; only the explicit stdin/stdout/stderr pipes
//! survive into the child. The Worker receives exactly one bounded request.
//! Geometry stays inside the declared maximum of 10 seconds; the closed
//! first-party 2K texture derivation has a separate fixed 120 second product
//! ceiling because its lossless PNG work is not caller-authored geometry.

use forgecad_contracts::{build_cohort_sha256, is_sha256};
use forgecad_worker_protocol::{
    validate_native_high_glb_materialize_payload, validate_native_high_glb_materialize_result,
    validate_response, WorkerRequest, WorkerResponse, MAX_WORKER_REQUEST_BYTES,
    MAX_WORKER_RESPONSE_BYTES, MAX_WORKER_STDERR_BYTES, NATIVE_HIGH_GLB_MATERIALIZE_ENTRY,
    NATIVE_HIGH_GLB_MATERIALIZE_OPERATION, WORKER_PROTOCOL,
};
use serde_json::{json, Value};
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const GEOMETRY_WORKER_BINARY: &str = "forgecad-geometry-worker";
const HIGH_WORKER_BINARY: &str = "forgecad-high-worker";
const WORKER_WALL_TIMEOUT: Duration = Duration::from_secs(10);
const FICTIONAL_ENERGY_WEAPON_2K_WALL_TIMEOUT: Duration = Duration::from_secs(120);
// Darwin's `wait4(2)` reports `ru_maxrss` in bytes. This is deliberately a
// post-hoc acceptance gate: it prevents an over-budget Worker result from
// being parsed or persisted, but it does not claim to stop a running process
// from briefly exceeding the budget. The latter remains a separate OS-level
// resource-control question.
const ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

/// Per-request limits derived from the closed V2 budget when it is present.
///
/// The Worker still starts with a product-wide 512 MiB allocator guard before
/// it can read any request byte. On Darwin, peak RSS remains a post-hoc
/// acceptance check rather than an OS-preventive memory cap. What *is*
/// enforceable here is that a V2 program requesting a smaller runtime or
/// accepted-result memory budget cannot be accepted under the broader product
/// ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExecutionBudget {
    wall_timeout: Duration,
    accepted_peak_rss_budget_bytes: u64,
}

const DEFAULT_EXECUTION_BUDGET: ExecutionBudget = ExecutionBudget {
    wall_timeout: WORKER_WALL_TIMEOUT,
    accepted_peak_rss_budget_bytes: ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum GeometryWorkerError {
    #[error("GEOMETRY_WORKER_UNAVAILABLE")]
    Unavailable,
    #[error("GEOMETRY_WORKER_PROTOCOL")]
    Protocol,
    #[error("GEOMETRY_WORKER_TIMEOUT")]
    Timeout,
    #[error("GEOMETRY_WORKER_CRASHED")]
    Crashed,
    #[cfg_attr(
        not(any(test, feature = "test-geometry-worker-fallback")),
        allow(dead_code)
    )]
    #[error("GEOMETRY_WORKER_REJECTED")]
    Rejected,
    /// A closed Worker error envelope survived the resource, protocol and
    /// cohort gates. Keep the machine code and bounded safe message for the
    /// Runtime caller; raw Worker stderr and arbitrary payload bytes never
    /// cross this boundary.
    #[error("GEOMETRY_WORKER_REJECTED: {code}: {message}")]
    RejectedWithDetails { code: String, message: String },
    #[error("GEOMETRY_WORKER_RUSAGE_UNAVAILABLE")]
    RusageUnavailable,
    #[error("GEOMETRY_WORKER_PEAK_RSS_BUDGET_EXCEEDED")]
    PeakRssBudgetExceeded,
}

fn protocol_at(_stage: &'static str) -> GeometryWorkerError {
    #[cfg(test)]
    eprintln!("FORGECAD_GEOMETRY_WORKER_PROTOCOL_STAGE={_stage}");
    GeometryWorkerError::Protocol
}

/// The generic sibling launcher returns the typed result together with the
/// identity reported by the child. Callers that only need the payload should
/// keep using `execute_sibling_worker`; evidence-producing adapters can retain
/// the cohort without duplicating process or protocol code.
#[derive(Debug, Clone)]
pub(crate) struct SiblingWorkerResult {
    pub result: Value,
    pub build_cohort_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GeometryArtifact {
    pub glb: Vec<u8>,
    pub build_cohort_sha256: Option<String>,
    pub part_ids: Vec<String>,
    pub triangle_count: u64,
    pub program_sha256: String,
    pub uv_status: String,
    pub tangent_status: String,
    pub material_zone_ids: Vec<String>,
}

pub(crate) fn compile_geometry(
    geometry_program: &Value,
    appearance_program: Option<&Value>,
) -> Result<GeometryArtifact, GeometryWorkerError> {
    let execution_budget = execution_budget_for_compile(geometry_program, appearance_program);
    // The Worker protocol is closed even when no appearance program is used;
    // send an explicit null so legacy GeometryProgram@1 calls and V2 geometry
    // calls take the same typed path instead of failing at the protocol gate.
    let payload = json!({
        "geometry_program":geometry_program,
        "appearance_program":appearance_program.cloned().unwrap_or(Value::Null),
    });
    let expected_program_sha256 = required_sha256(geometry_program.get("canonical_sha256"))?;
    let worker = execute_sibling_worker_with_metadata_and_budget(
        GEOMETRY_WORKER_BINARY,
        "compile_geometry",
        payload,
        execution_budget,
    )?;
    let object = worker
        .result
        .as_object()
        .ok_or_else(|| protocol_at("compile.result_object"))?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "mime",
            "glb_base64",
            "part_ids",
            "triangle_count",
            "program_sha256",
            "uv_status",
            "tangent_status",
            "material_zone_ids",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryWorkerResult@1")
        || object.get("mime").and_then(Value::as_str) != Some("model/gltf-binary")
    {
        return Err(protocol_at("compile.result_schema_or_mime"));
    }
    let program_sha256 = required_sha256(object.get("program_sha256"))?;
    if program_sha256 != expected_program_sha256 {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = object
        .get("glb_base64")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| protocol_at("compile.glb_base64"))?;
    let glb = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encoded.as_bytes(),
    )
    .map_err(|_| protocol_at("compile.glb_decode"))?;
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err(protocol_at("compile.glb_size"));
    }
    let part_ids = strict_identifier_array(object.get("part_ids"))?;
    let material_zone_ids = strict_identifier_array(object.get("material_zone_ids"))?;
    let triangle_count = object
        .get("triangle_count")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| protocol_at("compile.triangle_count"))?;
    let uv_status = status(object.get("uv_status"))?;
    let tangent_status = status(object.get("tangent_status"))?;
    Ok(GeometryArtifact {
        glb,
        build_cohort_sha256: worker.build_cohort_sha256,
        part_ids,
        triangle_count,
        program_sha256,
        uv_status,
        tangent_status,
        material_zone_ids,
    })
}

pub(crate) fn geometry_program_hash(draft: &Value) -> Result<Value, GeometryWorkerError> {
    let result = execute(
        "geometry_program_hash",
        json!({"geometry_program_draft":draft}),
        DEFAULT_EXECUTION_BUDGET,
    )?;
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "geometry_program_schema_version",
            "canonical_sha256",
            "operator_catalog_sha256",
            "validation_status",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgramHashResult@1")
        || object
            .get("geometry_program_schema_version")
            .and_then(Value::as_str)
            != Some("GeometryProgram@2")
        || object.get("validation_status").and_then(Value::as_str) != Some("passed")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let canonical_sha256 = required_sha256(object.get("canonical_sha256"))?;
    let operator_catalog_sha256 = required_sha256(object.get("operator_catalog_sha256"))?;
    Ok(json!({
        "schema_version":"GeometryProgramHashResult@1",
        "geometry_program_schema_version":"GeometryProgram@2",
        "canonical_sha256":canonical_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "validation_status":"passed"
    }))
}

/// Run the fixed, bounded Low-retopology source operation.
///
/// The caller supplies only the closed Worker payload.  Binary identity,
/// command-line entrypoint, timeout and resource gates remain Runtime-owned.
/// The returned projection has not been persisted and cannot advance a
/// production stage by itself.
pub(crate) fn production_weapon_low_retopology(
    payload: &Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata(
        GEOMETRY_WORKER_BINARY,
        forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION,
        payload.clone(),
    )
}

/// Run the fixed topology-correspondent Cage-offset source operation.
///
/// As with Low retopology, this is a transient Worker projection until a
/// Runtime-owned CAS reservation and Store transaction commit the bundle.
pub(crate) fn production_weapon_cage_offset(
    payload: &Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata(
        GEOMETRY_WORKER_BINARY,
        forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION,
        payload.clone(),
    )
}

/// Run the fixed 2K High/Low/Cage geometric-bake operation.
///
/// The request cannot select a binary, entrypoint, timeout, environment or
/// resource budget.  The longer wall-clock allowance is reserved only for
/// this closed first-party 2K operation; its result is still transient until
/// Runtime independently validates it and commits the owned CAS roots.
pub(crate) fn production_weapon_geometric_bake_2k(
    payload: &Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata_and_budget(
        GEOMETRY_WORKER_BINARY,
        forgecad_worker_protocol::PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION,
        payload.clone(),
        ExecutionBudget {
            wall_timeout: FICTIONAL_ENERGY_WEAPON_2K_WALL_TIMEOUT,
            accepted_peak_rss_budget_bytes: ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES,
        },
    )
}

/// Run the fixed Native High sibling as a transient structural projection.
///
/// This transport does not admit the returned JSON into CAS, create or
/// retarget a candidate, advance ProductionStage, or materialize a GLB. Those
/// remain a later Runtime-owned producer transaction after strict artifact
/// readback exists. The sibling binary, entrypoint, timeout, memory acceptance
/// gate and same-build cohort check are not caller controlled.
pub(crate) fn production_weapon_native_high(
    payload: &Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata(
        HIGH_WORKER_BINARY,
        forgecad_worker_protocol::NATIVE_HIGH_WORKER_OPERATION,
        payload.clone(),
    )
}

/// Run the fixed Native High → embedded GLB sibling as a transient structural
/// projection. The wrapper validates the closed request/result shape but does
/// not write CAS/SQLite, alter a candidate, or advance a Runtime stage.
pub(crate) fn production_weapon_native_high_glb_materialize(
    payload: &Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    validate_native_high_glb_materialize_payload(payload)
        .map_err(|_| GeometryWorkerError::Protocol)?;
    let result = execute_sibling_worker_with_metadata(
        HIGH_WORKER_BINARY,
        NATIVE_HIGH_GLB_MATERIALIZE_OPERATION,
        payload.clone(),
    )?;
    validate_native_high_glb_materialize_result(&result.result)
        .map_err(|_| GeometryWorkerError::Protocol)?;
    Ok(result)
}

pub(crate) fn boolean_operand_lineage(
    geometry_program: &Value,
    boolean_node_id: &str,
    max_lineage_runs: u64,
) -> Result<Value, GeometryWorkerError> {
    let result = execute(
        "boolean_operand_lineage",
        json!({
            "geometry_program":geometry_program,
            "boolean_node_id":boolean_node_id,
            "max_lineage_runs":max_lineage_runs
        }),
        execution_budget_for_geometry_program(geometry_program),
    )?;
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "program_sha256",
            "operator_catalog_sha256",
            "boolean_node_id",
            "operation",
            "operands",
            "output_triangle_count",
            "lineage_run_count",
            "lineage_runs",
            "lineage_sha256",
            "lineage_kind",
            "materialization_status",
            "runtime_write_performed",
            "limitations",
            "canonical_sha256",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("BooleanOperandLineage@1")
        || object.get("boolean_node_id").and_then(Value::as_str) != Some(boolean_node_id)
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("lineage_kind").and_then(Value::as_str)
            != Some("evaluated-face-with-operand-run")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    required_sha256(object.get("program_sha256"))?;
    required_sha256(object.get("operator_catalog_sha256"))?;
    required_sha256(object.get("lineage_sha256"))?;
    required_sha256(object.get("canonical_sha256"))?;
    Ok(result)
}

pub(crate) fn subdivision_topology_lineage(
    geometry_program: &Value,
    subdivision_node_id: &str,
    max_lineage_elements: u64,
) -> Result<Value, GeometryWorkerError> {
    let result = execute(
        "subdivision_topology_lineage",
        json!({
            "geometry_program":geometry_program,
            "subdivision_node_id":subdivision_node_id,
            "max_lineage_elements":max_lineage_elements
        }),
        execution_budget_for_geometry_program(geometry_program),
    )?;
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "program_sha256",
            "operator_catalog_sha256",
            "subdivision_node_id",
            "lineage_kind",
            "lineage_space",
            "id_scope",
            "complete",
            "completeness_scope",
            "cross_version_stable",
            "artifact_binding_status",
            "max_lineage_elements",
            "lineage_element_count",
            "lineage",
            "lineage_sha256",
            "materialization_status",
            "runtime_write_performed",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("SubdivisionTopologyLineage@1")
        || object.get("subdivision_node_id").and_then(Value::as_str) != Some(subdivision_node_id)
        || object.get("complete") != Some(&Value::Bool(true))
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("lineage_space").and_then(Value::as_str) != Some("evaluated-quad-topology@1")
        || object.get("lineage_kind").and_then(Value::as_str)
            != Some("control-root-to-evaluated-quad-topology@1")
        || object.get("cross_version_stable") != Some(&Value::Bool(false))
    {
        return Err(GeometryWorkerError::Protocol);
    }
    required_sha256(object.get("program_sha256"))?;
    required_sha256(object.get("operator_catalog_sha256"))?;
    required_sha256(object.get("lineage_sha256"))?;
    required_sha256(object.get("canonical_sha256"))?;
    Ok(result)
}

fn execute(
    operation: &str,
    payload: Value,
    execution_budget: ExecutionBudget,
) -> Result<Value, GeometryWorkerError> {
    execute_sibling_worker_with_budget(GEOMETRY_WORKER_BINARY, operation, payload, execution_budget)
}

/// Shared Runtime launcher seam for typed sibling workers.
///
/// The caller owns the worker-specific binary identity and protocol projection
/// while this module owns only the fixed-process transport and its bounded
/// lifetime. Keeping this seam generic prevents Geometry Worker code from
/// importing Render Worker ownership and prevents Render Worker code from
/// reaching the GeometryProgram compiler.
pub(crate) fn execute_sibling_worker(
    worker_binary: &str,
    operation: &str,
    payload: Value,
) -> Result<Value, GeometryWorkerError> {
    execute_sibling_worker_with_metadata(worker_binary, operation, payload)
        .map(|response| response.result)
}

/// Launch one bounded sibling request and retain the child's authenticated
/// build identity for a Runtime-owned evidence envelope.
pub(crate) fn execute_sibling_worker_with_metadata(
    worker_binary: &str,
    operation: &str,
    payload: Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata_and_budget(
        worker_binary,
        operation,
        payload,
        DEFAULT_EXECUTION_BUDGET,
    )
}

/// Launch the closed first-party Hero UV producer through its dedicated
/// entrypoint.  Keeping this wrapper next to the generic launcher ensures the
/// Runtime cannot accidentally route the durable UV request through the
/// default Worker mode or bypass its cohort/resource gates.
pub(crate) fn production_weapon_hero_uv_layout(
    payload: Value,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    execute_sibling_worker_with_metadata(
        GEOMETRY_WORKER_BINARY,
        forgecad_worker_protocol::PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION,
        payload,
    )
}

fn execute_sibling_worker_with_budget(
    worker_binary: &str,
    operation: &str,
    payload: Value,
    execution_budget: ExecutionBudget,
) -> Result<Value, GeometryWorkerError> {
    execute_sibling_worker_with_metadata_and_budget(
        worker_binary,
        operation,
        payload,
        execution_budget,
    )
    .map(|response| response.result)
}

fn execute_sibling_worker_with_metadata_and_budget(
    worker_binary: &str,
    operation: &str,
    payload: Value,
    execution_budget: ExecutionBudget,
) -> Result<SiblingWorkerResult, GeometryWorkerError> {
    let request = WorkerRequest {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id: format!("forgecad-worker-{}", uuid::Uuid::new_v4().simple()),
        operation: operation.to_owned(),
        payload,
    };
    let input = serde_json::to_vec(&request).map_err(|_| protocol_at("spawn.request_serialize"))?;
    if input.is_empty() || input.len() > MAX_WORKER_REQUEST_BYTES {
        return Err(protocol_at("spawn.request_size"));
    }
    // The only extended profile is the closed first-party 2K surface bake.
    // Select a fixed sibling entry point so the Worker can install the
    // matching CPU rlimit before it reads request bytes. Generic geometry
    // remains on the ten-second entry point.
    let worker_args = if operation == forgecad_worker_protocol::NATIVE_HIGH_WORKER_OPERATION {
        [forgecad_worker_protocol::NATIVE_HIGH_WORKER_ENTRY]
    } else if operation == NATIVE_HIGH_GLB_MATERIALIZE_OPERATION {
        [NATIVE_HIGH_GLB_MATERIALIZE_ENTRY]
    } else if operation
        == forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION
    {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_ENTRY]
    } else if operation
        == forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION
    {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_ENTRY]
    } else if operation == forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_ENTRY]
    } else if operation == forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ENTRY]
    } else if operation == forgecad_worker_protocol::PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_HERO_UV_LAYOUT_ENTRY]
    } else if operation == forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION {
        [forgecad_worker_protocol::PRODUCTION_WEAPON_CAGE_OFFSET_ENTRY]
    } else if execution_budget.wall_timeout == FICTIONAL_ENERGY_WEAPON_2K_WALL_TIMEOUT {
        ["--isolated-once-2k"]
    } else {
        ["--isolated-once"]
    };
    let child = spawn_fixed_worker(
        worker_binary,
        &worker_args,
        input,
        execution_budget.wall_timeout,
    )?;
    // The peak-RSS gate is deliberately evaluated before the response is
    // decoded. A failed Worker response must not bypass the same resource
    // boundary as a successful result.
    accept_worker_resources(&child, execution_budget.accepted_peak_rss_budget_bytes)?;
    // The post-hoc peak-RSS gate above intentionally runs before parsing the
    // child output. A result that exceeded the budget therefore cannot become
    // an artifact, reach CAS, or create a candidate.
    let response =
        parse_worker_response(&child.stdout).map_err(|_| protocol_at("spawn.response_parse"))?;
    validate_response(&response, &request.request_id)
        .map_err(|_| protocol_at("spawn.response_validate"))?;
    if response.build_cohort_sha256 != build_cohort_sha256() {
        eprintln!(
            "FORGECAD_GEOMETRY_WORKER_PROTOCOL_STAGE=spawn.cohort expected={:?} actual={:?}",
            build_cohort_sha256(),
            response.build_cohort_sha256
        );
        return Err(protocol_at("spawn.cohort"));
    }
    let build_cohort_sha256 = response.build_cohort_sha256.clone();
    classify_completed_worker_response(&child, &response)?;
    Ok(SiblingWorkerResult {
        result: response
            .result
            .ok_or_else(|| protocol_at("spawn.result_missing"))?,
        build_cohort_sha256,
    })
}

/// Extract only limits that a V2 author has explicitly declared inside the
/// product maximum. Invalid V2 programs keep the safe product maximum here
/// and are subsequently rejected by the Worker’s authoritative validator;
/// this helper must not create a wider path for malformed input.
fn execution_budget_for_geometry_program(geometry_program: &Value) -> ExecutionBudget {
    if geometry_program
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("GeometryProgram@2")
    {
        return DEFAULT_EXECUTION_BUDGET;
    }
    let Some(budgets) = geometry_program.get("budgets").and_then(Value::as_object) else {
        return DEFAULT_EXECUTION_BUDGET;
    };
    let max_runtime_ms = budgets
        .get("max_runtime_ms")
        .and_then(Value::as_u64)
        .filter(|value| (1..=WORKER_WALL_TIMEOUT.as_millis() as u64).contains(value));
    let max_worker_memory_bytes = budgets
        .get("max_worker_memory_bytes")
        .and_then(Value::as_u64)
        .filter(|value| (1..=ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES).contains(value));
    ExecutionBudget {
        wall_timeout: max_runtime_ms
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_EXECUTION_BUDGET.wall_timeout),
        accepted_peak_rss_budget_bytes: max_worker_memory_bytes
            .unwrap_or(DEFAULT_EXECUTION_BUDGET.accepted_peak_rss_budget_bytes),
    }
}

fn execution_budget_for_compile(
    geometry_program: &Value,
    appearance_program: Option<&Value>,
) -> ExecutionBudget {
    let mut budget = execution_budget_for_geometry_program(geometry_program);
    if matches!(
        appearance_program
            .and_then(|value| value.get("schema_version"))
            .and_then(Value::as_str),
        Some("AppearanceProgram@2" | "AppearanceProgram@3")
    ) && appearance_program
        .and_then(|value| value.get("material_pack_id"))
        .and_then(Value::as_str)
        == Some("forgecad-fictional-energy-weapon-2k")
    {
        budget.wall_timeout = FICTIONAL_ENERGY_WEAPON_2K_WALL_TIMEOUT;
    }
    budget
}

fn strict_object(value: &Value) -> Result<&serde_json::Map<String, Value>, GeometryWorkerError> {
    value.as_object().ok_or(GeometryWorkerError::Protocol)
}

fn require_exact_keys(
    value: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), GeometryWorkerError> {
    if value.len() != allowed.len()
        || allowed.iter().any(|key| !value.contains_key(*key))
        || value.keys().any(|key| !allowed.contains(&key.as_str()))
    {
        return Err(protocol_at("result.exact_keys"));
    }
    Ok(())
}

fn required_sha256(value: Option<&Value>) -> Result<String, GeometryWorkerError> {
    value
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| protocol_at("result.required_sha256"))
}

fn required_identifier(value: Option<&Value>) -> Result<String, GeometryWorkerError> {
    value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        .map(str::to_owned)
        .ok_or_else(|| protocol_at("result.required_identifier"))
}

fn strict_identifier_array(value: Option<&Value>) -> Result<Vec<String>, GeometryWorkerError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 512)
        .ok_or_else(|| protocol_at("result.identifier_array"))?;
    values
        .iter()
        .map(|value| required_identifier(Some(value)))
        .collect()
}

fn status(value: Option<&Value>) -> Result<String, GeometryWorkerError> {
    match value.and_then(Value::as_str) {
        Some("passed") => Ok("passed".to_owned()),
        Some("failed") => Ok("failed".to_owned()),
        _ => Err(protocol_at("result.status")),
    }
}

struct CollectedChild {
    exit_code: i32,
    /// `false` means wait4 observed a signal termination. An encoded
    /// `128 + signal` exit value is not sufficient here: a signalled child
    /// that happened to flush a partial/valid-looking error envelope must
    /// remain a crash, never a typed Worker rejection.
    exited_normally: bool,
    peak_rss_bytes: u64,
    stdout: Vec<u8>,
    #[allow(dead_code)]
    stderr: Vec<u8>,
}

#[cfg(target_os = "macos")]
fn spawn_fixed_worker(
    worker_binary: &str,
    args: &[&str],
    input: Vec<u8>,
    wall_timeout: Duration,
) -> Result<CollectedChild, GeometryWorkerError> {
    let worker = resolve_fixed_worker(worker_binary)?;
    let WorkerPipes {
        stdin_read,
        stdin_write,
        stdout_read,
        stdout_write,
        stderr_read,
        stderr_write,
    } = open_worker_pipes(pipe)?;

    let spawn = spawn_posix(
        &worker,
        args,
        stdin_read.as_raw_fd(),
        stdout_write.as_raw_fd(),
        stderr_write.as_raw_fd(),
    );
    // `OwnedFd` closes every descriptor on all error paths. After successful
    // spawn the three child ends are no longer needed in the Runtime.
    drop(stdin_read);
    drop(stdout_write);
    drop(stderr_write);
    let pid = spawn?;

    let stdin: std::fs::File = stdin_write.into();
    let stdout: std::fs::File = stdout_read.into();
    let stderr: std::fs::File = stderr_read.into();
    let writer = thread::spawn(move || {
        let mut stdin = stdin;
        let result = stdin.write_all(&input).and_then(|_| stdin.flush());
        drop(stdin); // EOF tells the one-shot Worker that the request is complete.
        result
    });
    let stdout_reader = read_bounded_in_thread(stdout, MAX_WORKER_RESPONSE_BYTES);
    let stderr_reader = read_bounded_in_thread(stderr, MAX_WORKER_STDERR_BYTES);

    let started = Instant::now();
    let exit = wait_for_exit(pid, started + wall_timeout)?;
    let _ = writer.join();
    let remaining = (started + wall_timeout).saturating_duration_since(Instant::now());
    let stdout = stdout_reader
        .recv_timeout(remaining)
        .map_err(|_| GeometryWorkerError::Timeout)??;
    let stderr = stderr_reader
        .recv_timeout(remaining)
        .map_err(|_| GeometryWorkerError::Timeout)??;
    Ok(CollectedChild {
        exit_code: exit_code(exit.status),
        exited_normally: exited_normally(exit.status),
        peak_rss_bytes: exit.peak_rss_bytes,
        stdout,
        stderr,
    })
}

#[cfg(not(target_os = "macos"))]
fn spawn_fixed_worker(
    _worker_binary: &str,
    _args: &[&str],
    _input: Vec<u8>,
    _wall_timeout: Duration,
) -> Result<CollectedChild, GeometryWorkerError> {
    Err(GeometryWorkerError::Unavailable)
}

#[cfg(target_os = "macos")]
fn resolve_fixed_worker(worker_binary: &str) -> Result<PathBuf, GeometryWorkerError> {
    let executable = std::env::current_exe().map_err(|_| GeometryWorkerError::Unavailable)?;
    let parent = executable
        .parent()
        .ok_or(GeometryWorkerError::Unavailable)?;
    let mut candidates = vec![parent.join(worker_binary)];
    if parent.file_name().is_some_and(|value| value == "deps") {
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join(worker_binary));
        }
    }
    for candidate in candidates {
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o111 != 0
            {
                return Ok(candidate);
            }
        }
    }
    Err(GeometryWorkerError::Unavailable)
}

#[cfg(target_os = "macos")]
struct WorkerPipes {
    stdin_read: OwnedFd,
    stdin_write: OwnedFd,
    stdout_read: OwnedFd,
    stdout_write: OwnedFd,
    stderr_read: OwnedFd,
    stderr_write: OwnedFd,
}

#[cfg(target_os = "macos")]
fn open_worker_pipes<F>(mut open_pipe: F) -> Result<WorkerPipes, GeometryWorkerError>
where
    F: FnMut() -> Result<(OwnedFd, OwnedFd), GeometryWorkerError>,
{
    // Each completed pair is owned immediately. If a later pipe creation
    // fails, Rust drops the earlier pairs before returning the typed error.
    let (stdin_read, stdin_write) = open_pipe()?;
    let (stdout_read, stdout_write) = open_pipe()?;
    let (stderr_read, stderr_write) = open_pipe()?;
    Ok(WorkerPipes {
        stdin_read,
        stdin_write,
        stdout_read,
        stdout_write,
        stderr_read,
        stderr_write,
    })
}

#[cfg(target_os = "macos")]
fn pipe() -> Result<(OwnedFd, OwnedFd), GeometryWorkerError> {
    let mut fds = [-1, -1];
    // SAFETY: `fds` is a valid two-element output buffer.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(GeometryWorkerError::Unavailable);
    }
    // SAFETY: successful `pipe(2)` initializes two distinct owned
    // descriptors. From this point RAII owns all cleanup paths.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

#[cfg(target_os = "macos")]
fn spawn_posix(
    worker: &PathBuf,
    args: &[&str],
    stdin_read: RawFd,
    stdout_write: RawFd,
    stderr_write: RawFd,
) -> Result<libc::pid_t, GeometryWorkerError> {
    let worker = CString::new(worker.as_os_str().as_bytes())
        .map_err(|_| GeometryWorkerError::Unavailable)?;
    let mut argv_strings = vec![worker.clone()];
    for arg in args {
        argv_strings.push(CString::new(*arg).map_err(|_| GeometryWorkerError::Protocol)?);
    }
    let mut argv = argv_strings
        .iter_mut()
        .map(|arg| arg.as_ptr() as *mut libc::c_char)
        .collect::<Vec<_>>();
    argv.push(std::ptr::null_mut());
    // The Worker receives no parent secrets or caller-controlled environment.
    let mut environment = vec![CString::new("PATH=/usr/bin:/bin").expect("static environment")];
    let mut envp = environment
        .iter_mut()
        .map(|value| value.as_ptr() as *mut libc::c_char)
        .collect::<Vec<_>>();
    envp.push(std::ptr::null_mut());

    let mut actions: libc::posix_spawn_file_actions_t = std::ptr::null_mut();
    let mut attributes: libc::posix_spawnattr_t = std::ptr::null_mut();
    // SAFETY: Apple exposes these opaque values through init/destroy APIs.
    unsafe {
        if libc::posix_spawn_file_actions_init(&mut actions) != 0 {
            return Err(GeometryWorkerError::Unavailable);
        }
        if libc::posix_spawnattr_init(&mut attributes) != 0 {
            libc::posix_spawn_file_actions_destroy(&mut actions);
            return Err(GeometryWorkerError::Unavailable);
        }
        let result = (|| {
            add_dup_and_close(&mut actions, stdin_read, libc::STDIN_FILENO)?;
            add_dup_and_close(&mut actions, stdout_write, libc::STDOUT_FILENO)?;
            add_dup_and_close(&mut actions, stderr_write, libc::STDERR_FILENO)?;
            if libc::posix_spawnattr_setflags(
                &mut attributes,
                libc::POSIX_SPAWN_CLOEXEC_DEFAULT as libc::c_short,
            ) != 0
            {
                return Err(GeometryWorkerError::Unavailable);
            }
            let mut pid = 0;
            let status = libc::posix_spawn(
                &mut pid,
                worker.as_ptr(),
                &actions,
                &attributes,
                argv.as_mut_ptr(),
                envp.as_mut_ptr(),
            );
            if status != 0 {
                return Err(GeometryWorkerError::Unavailable);
            }
            Ok(pid)
        })();
        libc::posix_spawn_file_actions_destroy(&mut actions);
        libc::posix_spawnattr_destroy(&mut attributes);
        result
    }
}

#[cfg(target_os = "macos")]
unsafe fn add_dup_and_close(
    actions: &mut libc::posix_spawn_file_actions_t,
    from: RawFd,
    to: RawFd,
) -> Result<(), GeometryWorkerError> {
    if libc::posix_spawn_file_actions_adddup2(actions, from, to) != 0 {
        return Err(GeometryWorkerError::Unavailable);
    }
    if from != to && libc::posix_spawn_file_actions_addclose(actions, from) != 0 {
        return Err(GeometryWorkerError::Unavailable);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_bounded_in_thread(
    file: std::fs::File,
    limit: usize,
) -> mpsc::Receiver<Result<Vec<u8>, GeometryWorkerError>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut file = file;
        let mut output = Vec::new();
        let mut chunk = [0u8; 8192];
        let result = loop {
            match file.read(&mut chunk) {
                Ok(0) => break Ok(output),
                Ok(count) if output.len().saturating_add(count) <= limit => {
                    output.extend_from_slice(&chunk[..count]);
                }
                Ok(_) => break Err(GeometryWorkerError::Protocol),
                Err(_) => break Err(GeometryWorkerError::Protocol),
            }
        };
        let _ = sender.send(result);
    });
    receiver
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitFailureAction {
    Retry,
    ReturnCrashed,
    TerminateThenCrash,
    TerminateThenTimeout,
}

#[cfg(target_os = "macos")]
fn deadline_expired(now: Instant, deadline: Instant) -> bool {
    now >= deadline
}

#[cfg(target_os = "macos")]
fn wait_failure_action(errno: libc::c_int, now: Instant, deadline: Instant) -> WaitFailureAction {
    match errno {
        libc::EINTR if deadline_expired(now, deadline) => WaitFailureAction::TerminateThenTimeout,
        libc::EINTR => WaitFailureAction::Retry,
        libc::ECHILD => WaitFailureAction::ReturnCrashed,
        _ => WaitFailureAction::TerminateThenCrash,
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct WorkerExit {
    status: libc::c_int,
    peak_rss_bytes: u64,
}

#[cfg(target_os = "macos")]
fn wait_for_exit(pid: libc::pid_t, deadline: Instant) -> Result<WorkerExit, GeometryWorkerError> {
    loop {
        // Check before every wait attempt. In particular, a stream of signals
        // must not keep returning EINTR and defer the monotonic watchdog.
        if deadline_expired(Instant::now(), deadline) {
            terminate_and_reap(pid);
            return Err(GeometryWorkerError::Timeout);
        }
        let mut status = 0;
        // SAFETY: the child PID was returned by posix_spawn and is reaped only
        // here. Darwin documents `rusage.ru_maxrss` as bytes, unlike Linux's
        // KiB convention. `rusage` is only inspected when this call reaps the
        // child (that is, when it returns `pid`).
        let mut rusage = unsafe { std::mem::zeroed::<libc::rusage>() };
        let result = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut rusage) };
        if result == pid {
            return Ok(WorkerExit {
                status,
                peak_rss_bytes: darwin_peak_rss_bytes(rusage.ru_maxrss)?,
            });
        }
        if result == -1 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            // `ECHILD` means there is nothing this Runtime can reap. For any
            // other wait failure the spawned PID is still ours, so stop it
            // before returning a typed crash instead of orphaning work.
            match wait_failure_action(errno, Instant::now(), deadline) {
                WaitFailureAction::Retry => continue,
                WaitFailureAction::ReturnCrashed => return Err(GeometryWorkerError::Crashed),
                WaitFailureAction::TerminateThenCrash => {
                    terminate_and_reap(pid);
                    return Err(GeometryWorkerError::Crashed);
                }
                WaitFailureAction::TerminateThenTimeout => {
                    terminate_and_reap(pid);
                    return Err(GeometryWorkerError::Timeout);
                }
            }
        }
        if deadline_expired(Instant::now(), deadline) {
            terminate_and_reap(pid);
            return Err(GeometryWorkerError::Timeout);
        }
        thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(target_os = "macos")]
fn darwin_peak_rss_bytes(ru_maxrss: libc::c_long) -> Result<u64, GeometryWorkerError> {
    u64::try_from(ru_maxrss).map_err(|_| GeometryWorkerError::RusageUnavailable)
}

#[cfg(test)]
fn accept_completed_worker(
    child: &CollectedChild,
    accepted_peak_rss_budget_bytes: u64,
) -> Result<(), GeometryWorkerError> {
    accept_worker_resources(child, accepted_peak_rss_budget_bytes)?;
    if !child.exited_normally || child.exit_code != 0 {
        return Err(GeometryWorkerError::Crashed);
    }
    Ok(())
}

fn accept_worker_resources(
    child: &CollectedChild,
    accepted_peak_rss_budget_bytes: u64,
) -> Result<(), GeometryWorkerError> {
    if child.peak_rss_bytes > accepted_peak_rss_budget_bytes {
        return Err(GeometryWorkerError::PeakRssBudgetExceeded);
    }
    Ok(())
}

fn parse_worker_response(stdout: &[u8]) -> Result<WorkerResponse, GeometryWorkerError> {
    if stdout.is_empty() {
        // A one-shot Worker that exits without an envelope is indistinguish-
        // able from a crash at this boundary. Do not turn it into a generic
        // protocol rejection that would hide the process failure.
        return Err(GeometryWorkerError::Crashed);
    }
    serde_json::from_slice::<WorkerResponse>(stdout).map_err(|_| {
        #[cfg(test)]
        eprintln!(
            "FORGECAD_GEOMETRY_WORKER_PROTOCOL_STAGE=parse.response_json stdout_bytes={}",
            stdout.len()
        );
        GeometryWorkerError::Protocol
    })
}

fn bounded_worker_error_message(message: &str) -> String {
    // Worker errors are intended to be short semantic diagnostics. Do not
    // forward path-like/control-bearing text across the Runtime error
    // boundary, even though the envelope itself is already size-bounded.
    if message.contains('/')
        || message.contains('\\')
        || message.chars().any(char::is_control)
        || !message.is_ascii()
    {
        return "worker rejected request".to_owned();
    }
    message.chars().take(512).collect()
}

fn classify_completed_worker_response(
    child: &CollectedChild,
    response: &WorkerResponse,
) -> Result<(), GeometryWorkerError> {
    if !child.exited_normally {
        return Err(GeometryWorkerError::Crashed);
    }
    if !response.ok {
        // The fixed Worker intentionally exits non-zero for a typed,
        // validated rejection. Only that closed error envelope is allowed
        // to turn the process status into `Rejected`.
        let error = response
            .error
            .as_ref()
            .ok_or_else(|| protocol_at("classify.failed_missing_error"))?;
        return if child.exit_code != 0 {
            Err(GeometryWorkerError::RejectedWithDetails {
                code: error.code.clone(),
                message: bounded_worker_error_message(&error.message),
            })
        } else {
            // A failed envelope with a successful process status violates the
            // one-shot Worker contract. Keep it fail-closed as protocol drift
            // instead of treating it as a genuine Worker rejection.
            #[cfg(test)]
            eprintln!(
                "FORGECAD_GEOMETRY_WORKER_PROTOCOL_STAGE=classify.failed_exit_zero code={} message={}",
                error.code,
                bounded_worker_error_message(&error.message)
            );
            Err(GeometryWorkerError::Protocol)
        };
    }
    if child.exit_code != 0 {
        return Err(GeometryWorkerError::Crashed);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn terminate_and_reap(pid: libc::pid_t) {
    // SAFETY: the child PID belongs to this Runtime invocation.
    let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    loop {
        let mut status = 0;
        // SAFETY: blocking reap of the child we just terminated.
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result == pid {
            return;
        }
        if result == -1 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            if errno != libc::EINTR {
                return;
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn exit_code(status: libc::c_int) -> i32 {
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        128 + libc::WTERMSIG(status)
    }
}

#[cfg(target_os = "macos")]
fn exited_normally(status: libc::c_int) -> bool {
    libc::WIFEXITED(status)
}

#[cfg(test)]
pub(crate) fn test_worker_mode(args: &[&str]) -> Result<(i32, Vec<u8>), GeometryWorkerError> {
    let child = spawn_fixed_worker(
        GEOMETRY_WORKER_BINARY,
        args,
        Vec::new(),
        DEFAULT_EXECUTION_BUDGET.wall_timeout,
    )?;
    Ok((child.exit_code, child.stdout))
}

#[cfg(test)]
pub(crate) fn test_worker_raw(
    args: &[&str],
    input: Vec<u8>,
) -> Result<(i32, Vec<u8>), GeometryWorkerError> {
    let child = spawn_fixed_worker(
        GEOMETRY_WORKER_BINARY,
        args,
        input,
        DEFAULT_EXECUTION_BUDGET.wall_timeout,
    )?;
    Ok((child.exit_code, child.stdout))
}

#[cfg(test)]
fn test_worker_mode_with_timeout(
    args: &[&str],
    wall_timeout: Duration,
) -> Result<(i32, Vec<u8>), GeometryWorkerError> {
    let child = spawn_fixed_worker(GEOMETRY_WORKER_BINARY, args, Vec::new(), wall_timeout)?;
    Ok((child.exit_code, child.stdout))
}

/// Unit tests that exercise historical V1/MCP007–009 transaction behavior can
/// still run in a bare `cargo test` target where Cargo has not built a sibling
/// binary. A consumer test (such as the MCP authenticated-IPC test) may opt
/// into the same internal-only feature. This fallback is absent from Runtime
/// product builds; dedicated isolation gates require the real sibling and do
/// not use it.
#[cfg(any(test, feature = "test-geometry-worker-fallback"))]
pub(crate) fn compile_geometry_test_fallback(
    geometry_program: &Value,
    appearance_program: Option<&Value>,
) -> Result<GeometryArtifact, GeometryWorkerError> {
    let artifact = forgecad_geometry_worker::compile_geometry_program_with_appearance(
        geometry_program,
        appearance_program,
    )
    .map_err(|error| GeometryWorkerError::RejectedWithDetails {
        code: "GEOMETRY_COMPILE_REJECTED".to_owned(),
        message: error.to_string(),
    })?;
    Ok(GeometryArtifact {
        glb: artifact.glb,
        // MCP transport tests explicitly opt into this in-process compiler
        // and therefore do not have a sibling Worker identity to persist.
        // Keep the same fixed test-only cohort already used by Runtime's
        // appearance fixtures so durable V1 clip validation can still prove
        // that both fallback replays came from one deterministic cohort. A
        // real compile-time cohort always wins, and production builds cannot
        // compile this fallback at all.
        build_cohort_sha256: Some(
            build_cohort_sha256().unwrap_or_else(|| {
                crate::sha256_hex(b"forgecad-source-test-fallback-worker-cohort")
            }),
        ),
        part_ids: artifact.part_ids,
        triangle_count: artifact.triangle_count,
        program_sha256: artifact.program_sha256,
        uv_status: artifact.uv_status,
        tangent_status: artifact.tangent_status,
        material_zone_ids: artifact.material_zone_ids,
    })
}

#[cfg(any(test, feature = "test-geometry-worker-fallback"))]
pub(crate) fn boolean_operand_lineage_test_fallback(
    geometry_program: &Value,
    boolean_node_id: &str,
    max_lineage_runs: u64,
) -> Result<Value, GeometryWorkerError> {
    let max_lineage_runs =
        usize::try_from(max_lineage_runs).map_err(|_| GeometryWorkerError::Rejected)?;
    forgecad_geometry_worker::boolean_operand_lineage_preview(
        geometry_program,
        boolean_node_id,
        max_lineage_runs,
    )
    .map_err(|_| GeometryWorkerError::Rejected)
}

#[cfg(any(test, feature = "test-geometry-worker-fallback"))]
pub(crate) fn subdivision_topology_lineage_test_fallback(
    geometry_program: &Value,
    subdivision_node_id: &str,
    max_lineage_elements: u64,
) -> Result<Value, GeometryWorkerError> {
    let max_lineage_elements =
        usize::try_from(max_lineage_elements).map_err(|_| GeometryWorkerError::Rejected)?;
    forgecad_geometry_worker::subdivision_topology_lineage_preview(
        geometry_program,
        subdivision_node_id,
        max_lineage_elements,
    )
    .map_err(|_| GeometryWorkerError::Rejected)
}

// These gates deliberately require a source-built sibling executable. Normal
// `cargo test -p forgecad-runtime` remains useful for legacy MCP007–009 unit
// coverage without manufacturing a Worker binary; `script/test_mcp010b.sh`
// builds the sibling first and explicitly runs these ignored tests.
#[cfg(all(test, target_os = "macos"))]
mod isolated_tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs::File;
    use std::os::fd::AsRawFd;

    fn v1_fixture_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":"mcp010b-isolation-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.2,1.6,0.55],"position":[0.0,1.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.55,1.2,0.55],"position":[0.0,1.5,0.0],"material_zone_id":"zone-black-mechanical","segments":16}},
                {"node_id":"head","operator_id":"forgecad.geometry.primitive@1","part_id":"head","parameters":{"shape":"sphere","size":[0.85,0.9,0.85],"position":[0.0,2.75,0.0],"material_zone_id":"zone-white-shell","segments":16}}
            ],
            "budgets":{"max_nodes":16,"max_triangles":20000,"max_runtime_ms":1000}
        });
        program["canonical_sha256"] = Value::String(crate::canonical_json_hash(&program));
        program
    }

    fn v2_fixture_program(max_runtime_ms: u64, max_worker_memory_bytes: u64) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"mcp010b-isolation-v2-fixture",
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":forgecad_geometry_worker::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":1000,
                "max_glb_bytes":1048576,
                "max_worker_memory_bytes":max_worker_memory_bytes,
                "max_runtime_ms":max_runtime_ms
            },
            "nodes":[{
                "node_id":"shell",
                "operator_id":"forgecad.geometry.primitive@2",
                "inputs":[],
                "parameters":{
                    "shape":"box",
                    "size_m":[1.0,1.0,1.0],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"shell",
                "input_node_ids":["shell"],
                "material_zone_id":"zone-white-shell",
                "solid":true
            }]
        });
        let canonical_sha256 = forgecad_geometry_worker::geometry_program_v2_draft_hash(&program)
            .expect("fixture V2 draft must be valid");
        program["canonical_sha256"] = Value::String(canonical_sha256);
        program
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_is_deterministic_for_five_real_compiles() {
        let program = v1_fixture_program();
        let first = compile_geometry(&program, None).expect("first isolated compile");
        for _ in 0..4 {
            let next = compile_geometry(&program, None).expect("repeat isolated compile");
            assert_eq!(next.glb, first.glb);
            assert_eq!(next.part_ids, first.part_ids);
            assert_eq!(next.material_zone_ids, first.material_zone_ids);
            assert_eq!(next.triangle_count, first.triangle_count);
            assert_eq!(next.program_sha256, first.program_sha256);
            assert_eq!(next.uv_status, first.uv_status);
            assert_eq!(next.tangent_status, first.tangent_status);
        }
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_rejects_malformed_one_shot_request() {
        let (exit_code, stdout) = test_worker_raw(&["--isolated-once"], b"{not-json".to_vec())
            .expect("launch malformed worker request");
        assert_eq!(exit_code, 1);
        let response: WorkerResponse = serde_json::from_slice(&stdout).expect("error response");
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("WORKER_PROTOCOL")
        );
        assert!(validate_response(&response, "invalid-request").is_ok());
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_timeout_is_monotonic_and_reaped() {
        let started = Instant::now();
        let error = test_worker_mode(&["--isolated-test-sleep"])
            .expect_err("sleeping worker must hit Runtime wall timeout");
        let elapsed = started.elapsed();
        assert!(matches!(error, GeometryWorkerError::Timeout));
        assert!(elapsed >= Duration::from_secs(9));
        assert!(elapsed < Duration::from_secs(12));
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_honors_a_lower_runtime_budget() {
        let started = Instant::now();
        let error =
            test_worker_mode_with_timeout(&["--isolated-test-sleep"], Duration::from_millis(75))
                .expect_err("sleeping worker must respect the tighter parent deadline");
        assert!(matches!(error, GeometryWorkerError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_rejects_an_accepted_result_above_the_program_memory_budget() {
        // The fixed Worker itself starts under the product-wide guard. A V2
        // program may request a tighter accepted-result peak budget, which
        // must be checked before its output can reach CAS/candidate state.
        let program = v2_fixture_program(10_000, 1);
        assert!(matches!(
            compile_geometry(&program, None),
            Err(GeometryWorkerError::PeakRssBudgetExceeded)
        ));
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn isolated_worker_crash_is_distinct_from_typed_rejection() {
        let (exit_code, stdout) =
            test_worker_mode(&["--isolated-test-crash"]).expect("launch crash fixture");
        assert_eq!(exit_code, 73);
        assert!(stdout.is_empty());
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn posix_spawn_closes_unlisted_runtime_file_descriptors() {
        let probe = File::open("/dev/null").expect("open test descriptor");
        let descriptor = probe.as_raw_fd();
        // Clear CLOEXEC on the parent descriptor: POSIX_SPAWN_CLOEXEC_DEFAULT
        // must still withhold it because it is not one of the three explicit
        // Worker stdio pipes.
        unsafe {
            let flags = libc::fcntl(descriptor, libc::F_GETFD);
            assert!(flags >= 0);
            assert_eq!(
                libc::fcntl(descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC),
                0
            );
        }
        let (exit_code, stdout) =
            test_worker_mode(&["--isolated-test-fd-probe", &descriptor.to_string()])
                .expect("launch fd isolation fixture");
        assert_eq!(exit_code, 0);
        assert_eq!(
            serde_json::from_slice::<Value>(&stdout).expect("fd probe JSON")["fd_inherited"],
            false
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn worker_reports_darwin_memory_limit_truthfully() {
        let (exit_code, stdout) =
            test_worker_mode(&["--isolated-test-limits"]).expect("launch limits fixture");
        assert_eq!(exit_code, 0);
        let limits: Value = serde_json::from_slice(&stdout).expect("limits JSON");
        assert!(limits["cpu_seconds"]
            .as_u64()
            .is_some_and(|seconds| seconds > 0 && seconds <= 10));
        assert_eq!(limits["core_bytes"], 0);
        assert_eq!(limits["tracked_allocator_limit_bytes"], 512 * 1024 * 1024);
        for (applied, value) in [
            ("address_space_rlimit_applied", "address_space_bytes"),
            ("data_rlimit_applied", "data_bytes"),
        ] {
            if limits[applied] == true {
                assert!(limits[value]
                    .as_u64()
                    .is_some_and(|bytes| bytes <= 512 * 1024 * 1024));
            } else {
                assert!(limits[value].is_null());
            }
        }

        let (exit_code, stdout) = test_worker_mode(&["--isolated-test-allocator-limit"])
            .expect("launch allocator fixture");
        assert_eq!(exit_code, 0);
        let allocator: Value = serde_json::from_slice(&stdout).expect("allocator JSON");
        assert_eq!(allocator["actual_allocation_bytes"], 32 * 1024 * 1024);
        assert_eq!(
            allocator["tracked_allocator_limit_bytes"],
            512 * 1024 * 1024
        );
        assert_eq!(allocator["allocator_rejected_limit_reservation"], true);
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-geometry-worker sibling"]
    fn worker_never_widens_an_inherited_soft_cpu_limit() {
        let (exit_code, stdout) = test_worker_mode(&["--isolated-test-inherited-soft-cpu"])
            .expect("launch inherited CPU limit fixture");
        assert_eq!(exit_code, 0);
        assert!(
            serde_json::from_slice::<Value>(&stdout).expect("inherited CPU limit JSON")
                ["cpu_seconds"]
                .as_u64()
                .is_some_and(|seconds| seconds > 0 && seconds <= 1)
        );
    }

    #[test]
    #[ignore = "run serially by the fixed-sibling isolation gate"]
    fn failed_later_pipe_creation_releases_earlier_owned_descriptors() {
        let (read, write) = pipe().expect("first pipe");
        let read_fd = read.as_raw_fd();
        let write_fd = write.as_raw_fd();
        let mut first_pair = Some((read, write));
        let mut calls = 0;

        let result = open_worker_pipes(|| {
            calls += 1;
            if calls == 1 {
                Ok(first_pair.take().expect("one injected first pair"))
            } else {
                Err(GeometryWorkerError::Unavailable)
            }
        });
        assert!(matches!(result, Err(GeometryWorkerError::Unavailable)));
        assert_eq!(calls, 2);

        // The serial isolation gate prevents another test from reusing these
        // descriptor numbers between the RAII drop and this direct check.
        for descriptor in [read_fd, write_fd] {
            assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EBADF)
            );
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod wait_error_tests {
    use super::*;

    #[test]
    fn interrupted_wait_is_retried_without_abandoning_the_child() {
        let now = Instant::now();
        let future = now + Duration::from_secs(1);
        assert_eq!(
            wait_failure_action(libc::EINTR, now, future),
            WaitFailureAction::Retry
        );
        // Inject an EINTR observed after the monotonic deadline. It must
        // choose the kill/reap timeout path rather than keep retrying.
        assert_eq!(
            wait_failure_action(libc::EINTR, now, now),
            WaitFailureAction::TerminateThenTimeout
        );
        assert_eq!(
            wait_failure_action(libc::ECHILD, now, future),
            WaitFailureAction::ReturnCrashed
        );
        assert_eq!(
            wait_failure_action(libc::EINVAL, now, future),
            WaitFailureAction::TerminateThenCrash
        );
    }

    #[test]
    fn darwin_wait4_rusage_is_interpreted_as_bytes_without_large_allocation() {
        // This is an injected `wait4` observation, not a process that tries
        // to allocate near the budget. Darwin documents ru_maxrss in bytes.
        assert_eq!(
            darwin_peak_rss_bytes(12_345_678).expect("non-negative Darwin RSS"),
            12_345_678
        );
        assert!(matches!(
            darwin_peak_rss_bytes(-1),
            Err(GeometryWorkerError::RusageUnavailable)
        ));
    }

    #[test]
    fn accepted_worker_peak_rss_is_fail_closed_before_response_parse() {
        let at_budget = CollectedChild {
            exit_code: 0,
            exited_normally: true,
            peak_rss_bytes: ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES,
            stdout: b"this is deliberately not JSON".to_vec(),
            stderr: Vec::new(),
        };
        assert!(accept_completed_worker(&at_budget, ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES).is_ok());

        assert!(matches!(
            accept_completed_worker(
                &at_budget,
                ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES.saturating_sub(1)
            ),
            Err(GeometryWorkerError::PeakRssBudgetExceeded)
        ));

        // Simulate a `wait4` rusage observation just one byte above budget.
        // No test process allocates the budget-sized memory region.
        let over_budget = CollectedChild {
            peak_rss_bytes: ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES + 1,
            ..at_budget
        };
        assert!(matches!(
            accept_completed_worker(&over_budget, ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES),
            Err(GeometryWorkerError::PeakRssBudgetExceeded)
        ));

        let crashed = CollectedChild {
            exit_code: 73,
            exited_normally: true,
            peak_rss_bytes: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert!(matches!(
            accept_completed_worker(&crashed, ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES),
            Err(GeometryWorkerError::Crashed)
        ));
    }

    #[test]
    fn typed_worker_failure_with_nonzero_normal_exit_is_rejected_with_bounded_details() {
        let child = CollectedChild {
            exit_code: 1,
            exited_normally: true,
            peak_rss_bytes: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let response = WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "request-1".to_owned(),
            build_cohort_sha256: None,
            ok: false,
            result: None,
            error: Some(forgecad_worker_protocol::WorkerError {
                code: "RENDER_REJECTED".to_owned(),
                message: "render input rejected".to_owned(),
            }),
        };
        let error = classify_completed_worker_response(&child, &response)
            .expect_err("closed nonzero failure response must reject");
        assert_eq!(
            error.to_string(),
            "GEOMETRY_WORKER_REJECTED: RENDER_REJECTED: render input rejected"
        );

        let unsafe_message = "read /Users/example/private.glb";
        assert_eq!(
            bounded_worker_error_message(unsafe_message),
            "worker rejected request"
        );
    }

    #[test]
    fn signaled_worker_and_empty_stdout_remain_crashed() {
        let signaled = CollectedChild {
            exit_code: 137,
            exited_normally: false,
            peak_rss_bytes: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let response = WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "request-1".to_owned(),
            build_cohort_sha256: None,
            ok: false,
            result: None,
            error: Some(forgecad_worker_protocol::WorkerError {
                code: "RENDER_REJECTED".to_owned(),
                message: "render input rejected".to_owned(),
            }),
        };
        assert!(matches!(
            classify_completed_worker_response(&signaled, &response),
            Err(GeometryWorkerError::Crashed)
        ));
        assert!(matches!(
            parse_worker_response(&[],),
            Err(GeometryWorkerError::Crashed)
        ));
        assert!(matches!(
            parse_worker_response(b"not-json"),
            Err(GeometryWorkerError::Protocol)
        ));
    }

    #[test]
    fn v2_execution_budget_honors_tighter_declared_limits() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "budgets":{
                "max_runtime_ms":73,
                "max_worker_memory_bytes":123_456
            }
        });
        assert_eq!(
            execution_budget_for_geometry_program(&program),
            ExecutionBudget {
                wall_timeout: Duration::from_millis(73),
                accepted_peak_rss_budget_bytes: 123_456,
            }
        );
    }

    #[test]
    fn invalid_or_legacy_programs_cannot_widen_the_product_budget() {
        let legacy = json!({"schema_version":"GeometryProgram@1"});
        assert_eq!(
            execution_budget_for_geometry_program(&legacy),
            DEFAULT_EXECUTION_BUDGET
        );
        let invalid_v2 = json!({
            "schema_version":"GeometryProgram@2",
            "budgets":{
                "max_runtime_ms":10_001,
                "max_worker_memory_bytes":ACCEPTED_WORKER_PEAK_RSS_BUDGET_BYTES + 1
            }
        });
        assert_eq!(
            execution_budget_for_geometry_program(&invalid_v2),
            DEFAULT_EXECUTION_BUDGET
        );
    }

    #[test]
    fn only_the_closed_2k_pack_gets_the_separate_texture_build_budget() {
        let program = json!({
            "schema_version":"GeometryProgram@2",
            "budgets":{"max_runtime_ms":10_000,"max_worker_memory_bytes":123_456}
        });
        let appearance = json!({
            "schema_version":"AppearanceProgram@2",
            "material_pack_id":"forgecad-fictional-energy-weapon-2k"
        });
        assert_eq!(
            execution_budget_for_compile(&program, Some(&appearance)),
            ExecutionBudget {
                wall_timeout: Duration::from_secs(120),
                accepted_peak_rss_budget_bytes: 123_456,
            }
        );
        let mut surface_bake_appearance = appearance.clone();
        surface_bake_appearance["schema_version"] = Value::String("AppearanceProgram@3".to_owned());
        assert_eq!(
            execution_budget_for_compile(&program, Some(&surface_bake_appearance)).wall_timeout,
            Duration::from_secs(120)
        );
        let mut arbitrary = appearance;
        arbitrary["material_pack_id"] = Value::String("caller-pack".to_owned());
        assert_eq!(
            execution_budget_for_compile(&program, Some(&arbitrary)).wall_timeout,
            Duration::from_secs(10)
        );
    }
}
