//! Active Knife request-schema resolution and validation.
//!
//! The default MCP binary must validate a façade's selected operation before
//! it reaches a Runtime service.  This module deliberately has no dependency
//! on the compatibility registry (or any Runtime parser).  Its resolver is a
//! seam: the currently checked-in package schemas are embedded here, while a
//! future `forgecad-contracts` active-schema index can implement the same
//! trait without changing the façade/router call path.

use forgecad_runtime::is_opaque_id;
use serde_json::{json, Map, Value};

const MAX_SCHEMA_NODES: usize = 4096;
const MAX_SCHEMA_DEPTH: usize = 64;

macro_rules! embedded_schema {
    ($file:literal) => {
        include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/schemas/",
            $file
        ))
    };
}

/// Resolver contract used by the active MCP path.
///
/// `None` is intentionally different from an empty/open schema: it means the
/// central active registry has no consumable metadata for the operation.  The
/// caller must fail closed in that case; it must never ask Runtime to infer
/// the request shape.
pub(crate) trait ActiveRequestSchemaResolver {
    fn resolve(&self, operation: &str) -> Result<Option<Value>, String>;
}

/// Compile-time package-schema resolver used until Contracts exposes its
/// active operation index.  The table is keyed by schema filenames, rather
/// than duplicating the 125-operation façade allowlist.  Operations with no
/// exact package contract remain unavailable.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EmbeddedActiveRequestSchemaResolver;

impl ActiveRequestSchemaResolver for EmbeddedActiveRequestSchemaResolver {
    fn resolve(&self, operation: &str) -> Result<Option<Value>, String> {
        let file = schema_file_for_operation(operation);
        let Some(source) = EMBEDDED_SCHEMA_DOCUMENTS
            .iter()
            .find_map(|(name, source)| (*name == file).then_some(*source))
        else {
            return Ok(None);
        };
        let schema: Value = serde_json::from_str(source).map_err(|error| {
            format!(
                "WEAPONRY_ACTIVE_SCHEMA_INVALID: {operation} embedded schema is not valid JSON: {error}"
            )
        })?;
        materialize_schema(schema, operation).map(Some)
    }
}

/// Construct the resolver explicitly so callers/tests can later swap in the
/// Contracts-owned index without changing `tools/call` or façade code.
pub(crate) fn default_resolver() -> EmbeddedActiveRequestSchemaResolver {
    EmbeddedActiveRequestSchemaResolver
}

/// Resolve a schema for advertisement.  An operation whose schema is absent
/// or not root-closed is represented by a schema that rejects every practical
/// request.  It is never advertised as `additionalProperties: true`.
pub(crate) fn advertised_schema(operation: &str) -> Result<Value, String> {
    let resolver = default_resolver();
    let Some(schema) = resolver.resolve(operation)? else {
        return Ok(blocked_schema(
            operation,
            "no active Contract request schema",
        ));
    };
    if !root_is_closed(&schema) {
        return Ok(blocked_schema(
            operation,
            "active Contract request schema is not root-closed",
        ));
    }
    validate_schema_shape(operation, &schema)?;
    Ok(schema)
}

/// Return whether the currently embedded active metadata has a closed root.
/// This is used for honest coverage accounting and does not treat a blocked
/// advertisement schema as a closed contract.
pub(crate) fn is_closed(operation: &str) -> Result<bool, String> {
    let resolver = default_resolver();
    let Some(schema) = resolver.resolve(operation)? else {
        return Ok(false);
    };
    if !root_is_closed(&schema) {
        return Ok(false);
    }
    validate_schema_shape(operation, &schema)?;
    Ok(true)
}

/// Validate one already-unwrapped façade request against active metadata.
/// This is the MCP boundary; Runtime remains responsible for domain semantics
/// only after this function succeeds.
pub(crate) fn validate(operation: &str, request: &Value) -> Result<(), String> {
    let resolver = default_resolver();
    validate_with_resolver(&resolver, operation, request)
}

/// Validate the Runtime result for an operation whose output Contract is
/// explicitly closed at the MCP boundary.  Result validation is deliberately
/// opt-in per operation: operations without a checked-in result contract
/// retain their existing compatibility behavior and are not silently
/// assigned a guessed schema.
pub(crate) fn validate_closed_result(operation: &str, value: &Value) -> Result<(), String> {
    let Some(filename) = result_schema_file_for_operation(operation) else {
        return Ok(());
    };
    let source = embedded_document(filename).ok_or_else(|| {
        format!(
            "WEAPONRY_ACTIVE_RESULT_SCHEMA_UNAVAILABLE: {operation} result schema {filename} is not embedded"
        )
    })?;
    let schema: Value = serde_json::from_str(source).map_err(|error| {
        format!(
            "WEAPONRY_ACTIVE_RESULT_SCHEMA_INVALID: {operation} result schema is not valid JSON: {error}"
        )
    })?;
    let schema = materialize_schema(schema, operation)
        .map_err(|error| format!("WEAPONRY_ACTIVE_RESULT_SCHEMA_INVALID: {operation} {error}"))?;
    if !root_is_closed(&schema) {
        return Err(format!(
            "WEAPONRY_ACTIVE_RESULT_NOT_CLOSED: {operation} result schema must declare additionalProperties=false"
        ));
    }
    validate_schema_shape(operation, &schema)
        .map_err(|error| format!("WEAPONRY_ACTIVE_RESULT_SCHEMA_INVALID: {operation} {error}"))?;
    let mut budget = ValidationBudget::default();
    validate_value(&schema, &schema, value, "$", &mut budget)
        .map_err(|detail| format!("WEAPONRY_ACTIVE_RESULT_INVALID: {operation} {detail}"))
}

/// Validate through an injected resolver.  Focused tests and a future
/// Contracts-owned registry use this seam; no compatibility handler is
/// consulted.
pub(crate) fn validate_with_resolver(
    resolver: &dyn ActiveRequestSchemaResolver,
    operation: &str,
    request: &Value,
) -> Result<(), String> {
    let schema = resolver.resolve(operation)?.ok_or_else(|| {
        format!(
            "WEAPONRY_ACTIVE_SCHEMA_UNAVAILABLE: {operation} has no closed active Contract request schema; Runtime parser is not an MCP schema source"
        )
    })?;
    if !root_is_closed(&schema) {
        return Err(format!(
            "WEAPONRY_ACTIVE_SCHEMA_NOT_CLOSED: {operation} active request schema must declare additionalProperties=false"
        ));
    }
    validate_schema_shape(operation, &schema)?;
    let mut budget = ValidationBudget::default();
    validate_value(&schema, &schema, request, "$", &mut budget)
        .map_err(|detail| format!("WEAPONRY_ACTIVE_REQUEST_INVALID: {operation} {detail}"))
}

fn root_is_closed(schema: &Value) -> bool {
    schema
        .as_object()
        .and_then(|object| object.get("additionalProperties"))
        == Some(&Value::Bool(false))
}

fn blocked_schema(operation: &str, reason: &str) -> Value {
    // This is a valid closed JSON Schema, but no ordinary caller can satisfy
    // it.  The validator still returns a typed unavailable/not-closed error,
    // so schema consumers cannot mistake the blocker for a real contract.
    json!({
        "type":"object",
        "required":["__forgecad_active_schema_unavailable__"],
        "properties":{"__forgecad_active_schema_unavailable__":{"const":"never"}},
        "additionalProperties":false,
        "description":format!("{operation}: active request schema unavailable ({reason})")
    })
}

fn schema_file_for_operation(operation: &str) -> String {
    let file_stem = match operation {
        // These operation names are retained as compatibility-safe aliases
        // in the active profile; their contract filenames carry the durable
        // materialization record name.
        "game_weapon_glb_socket_get" => "game-weapon-glb-socket-materialization-get",
        "game_weapon_glb_socket_prepare" => "game-weapon-glb-socket-materialization-prepare",
        "cross_view_promotion_confirm" => "cross-view-promotion",
        "authoring_mesh_v2_candidate_materialize" => {
            "authoring-mesh-v2-candidate-materialize-prepare"
        }
        _ => return format!("{}-request.schema.json", operation.replace('_', "-")),
    };
    format!("{file_stem}-request.schema.json")
}

fn result_schema_file_for_operation(operation: &str) -> Option<&'static str> {
    match operation {
        "reference_compare_prepare" => Some("reference-comparison-prepare-result.schema.json"),
        "high_artifact_reference_compare_prepare" => {
            Some("high-artifact-reference-comparison-prepare-result.schema.json")
        }
        "knife_pass_state_prepare" | "knife_pass_state_get" => {
            Some("knife-pass-state-result.schema.json")
        }
        "authoring_mesh_v2_high_bridge_prepare" | "authoring_mesh_v2_high_bridge_get" => {
            Some("authoring-mesh-v2-high-bridge-result.schema.json")
        }
        "authoring_mesh_v2_high_artifact_prepare" | "authoring_mesh_v2_high_artifact_get" => {
            Some("authoring-mesh-v2-high-artifact-result.schema.json")
        }
        "production_knife_uv_bake_v2_prepare" | "production_knife_uv_bake_v2_get" => {
            Some("production-knife-uv-bake-v2-result.schema.json")
        }
        _ => None,
    }
}

/// Schema documents present in the checked-in Contracts package.  This is a
/// schema-source index, not a second operation allowlist; the active profile
/// remains the only source that decides which operation can be called.
const EMBEDDED_SCHEMA_DOCUMENTS: &[(&str, &str)] = &[
    (
        "capabilities-get-request.schema.json",
        embedded_schema!("capabilities-get-request.schema.json"),
    ),
    (
        "doctor-request.schema.json",
        embedded_schema!("doctor-request.schema.json"),
    ),
    (
        "project-list-request.schema.json",
        embedded_schema!("project-list-request.schema.json"),
    ),
    (
        "runtime-status-request.schema.json",
        embedded_schema!("runtime-status-request.schema.json"),
    ),
    (
        "selection-get-request.schema.json",
        embedded_schema!("selection-get-request.schema.json"),
    ),
    (
        "skill-get-request.schema.json",
        embedded_schema!("skill-get-request.schema.json"),
    ),
    (
        "version-diff-request.schema.json",
        embedded_schema!("version-diff-request.schema.json"),
    ),
    (
        "version-list-request.schema.json",
        embedded_schema!("version-list-request.schema.json"),
    ),
    (
        "appearance-source-lineage-get-request.schema.json",
        embedded_schema!("appearance-source-lineage-get-request.schema.json"),
    ),
    (
        "appearance-source-lineage-prepare-request.schema.json",
        embedded_schema!("appearance-source-lineage-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-edit-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-edit-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-get-request.schema.json",
        embedded_schema!("authoring-mesh-get-request.schema.json"),
    ),
    (
        "authoring-mesh-transaction-get-request.schema.json",
        embedded_schema!("authoring-mesh-transaction-get-request.schema.json"),
    ),
    (
        "authoring-mesh-transaction-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-transaction-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-candidate-materialize-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-v2-candidate-materialize-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-bridge-get-request.schema.json",
        embedded_schema!("authoring-mesh-v2-high-bridge-get-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-bridge-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-v2-high-bridge-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-bridge-result.schema.json",
        embedded_schema!("authoring-mesh-v2-high-bridge-result.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-bridge.schema.json",
        embedded_schema!("authoring-mesh-v2-high-bridge.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-artifact-get-request.schema.json",
        embedded_schema!("authoring-mesh-v2-high-artifact-get-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-artifact-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-v2-high-artifact-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-artifact-result.schema.json",
        embedded_schema!("authoring-mesh-v2-high-artifact-result.schema.json"),
    ),
    (
        "authoring-mesh-v2-high-artifact.schema.json",
        embedded_schema!("authoring-mesh-v2-high-artifact.schema.json"),
    ),
    (
        "candidate-confirm-request.schema.json",
        embedded_schema!("candidate-confirm-request.schema.json"),
    ),
    (
        "candidate-material-surface-quality-get-request.schema.json",
        embedded_schema!("candidate-material-surface-quality-get-request.schema.json"),
    ),
    (
        "candidate-material-surface-quality-prepare-request.schema.json",
        embedded_schema!("candidate-material-surface-quality-prepare-request.schema.json"),
    ),
    (
        "candidate-reject-request.schema.json",
        embedded_schema!("candidate-reject-request.schema.json"),
    ),
    (
        "candidate-topology-quality-get-request.schema.json",
        embedded_schema!("candidate-topology-quality-get-request.schema.json"),
    ),
    (
        "candidate-topology-quality-prepare-request.schema.json",
        embedded_schema!("candidate-topology-quality-prepare-request.schema.json"),
    ),
    (
        "export-confirm-request.schema.json",
        embedded_schema!("export-confirm-request.schema.json"),
    ),
    (
        "export-prepare-request.schema.json",
        embedded_schema!("export-prepare-request.schema.json"),
    ),
    (
        "fps-presentation-package-v2-candidate-get-request.schema.json",
        embedded_schema!("fps-presentation-package-v2-candidate-get-request.schema.json"),
    ),
    (
        "fps-presentation-package-v2-candidate-prepare-request.schema.json",
        embedded_schema!("fps-presentation-package-v2-candidate-prepare-request.schema.json"),
    ),
    (
        "fps-presentation-package-v2-get-request.schema.json",
        embedded_schema!("fps-presentation-package-v2-get-request.schema.json"),
    ),
    (
        "fps-presentation-package-v2-prepare-request.schema.json",
        embedded_schema!("fps-presentation-package-v2-prepare-request.schema.json"),
    ),
    (
        "game-asset-delivery-get-request.schema.json",
        embedded_schema!("game-asset-delivery-get-request.schema.json"),
    ),
    (
        "game-asset-delivery-prepare-request.schema.json",
        embedded_schema!("game-asset-delivery-prepare-request.schema.json"),
    ),
    (
        "game-asset-lod-derive-request.schema.json",
        embedded_schema!("game-asset-lod-derive-request.schema.json"),
    ),
    (
        "game-weapon-anchor-get-request.schema.json",
        embedded_schema!("game-weapon-anchor-get-request.schema.json"),
    ),
    (
        "game-weapon-anchor-prepare-request.schema.json",
        embedded_schema!("game-weapon-anchor-prepare-request.schema.json"),
    ),
    (
        "game-weapon-animated-glb-socket-transform-projection-get-request.schema.json",
        embedded_schema!(
            "game-weapon-animated-glb-socket-transform-projection-get-request.schema.json"
        ),
    ),
    (
        "game-weapon-animated-glb-socket-transform-projection-prepare-request.schema.json",
        embedded_schema!(
            "game-weapon-animated-glb-socket-transform-projection-prepare-request.schema.json"
        ),
    ),
    (
        "game-weapon-animated-glb-socket-transform-projection-v2-get-request.schema.json",
        embedded_schema!(
            "game-weapon-animated-glb-socket-transform-projection-v2-get-request.schema.json"
        ),
    ),
    (
        "game-weapon-animated-glb-socket-transform-projection-v2-prepare-request.schema.json",
        embedded_schema!(
            "game-weapon-animated-glb-socket-transform-projection-v2-prepare-request.schema.json"
        ),
    ),
    (
        "geometry-program-hash-request.schema.json",
        embedded_schema!("geometry-program-hash-request.schema.json"),
    ),
    (
        "hero-uv-durable-get-request.schema.json",
        embedded_schema!("hero-uv-durable-get-request.schema.json"),
    ),
    (
        "hero-uv-durable-prepare-request.schema.json",
        embedded_schema!("hero-uv-durable-prepare-request.schema.json"),
    ),
    (
        "knife-curve-evaluated-mesh-get-request.schema.json",
        embedded_schema!("knife-curve-evaluated-mesh-get-request.schema.json"),
    ),
    (
        "knife-curve-evaluated-mesh-prepare-request.schema.json",
        embedded_schema!("knife-curve-evaluated-mesh-prepare-request.schema.json"),
    ),
    (
        "knife-curve-modifier-graph-get-request.schema.json",
        embedded_schema!("knife-curve-modifier-graph-get-request.schema.json"),
    ),
    (
        "knife-curve-modifier-graph-prepare-request.schema.json",
        embedded_schema!("knife-curve-modifier-graph-prepare-request.schema.json"),
    ),
    (
        "knife-pass-state-get-request.schema.json",
        embedded_schema!("knife-pass-state-get-request.schema.json"),
    ),
    (
        "knife-pass-state-prepare-request.schema.json",
        embedded_schema!("knife-pass-state-prepare-request.schema.json"),
    ),
    (
        "knife-pass-state-result.schema.json",
        embedded_schema!("knife-pass-state-result.schema.json"),
    ),
    (
        "knife-pass-state.schema.json",
        embedded_schema!("knife-pass-state.schema.json"),
    ),
    (
        "low-quad-draft-durable-get-request.schema.json",
        embedded_schema!("low-quad-draft-durable-get-request.schema.json"),
    ),
    (
        "low-quad-draft-durable-prepare-request.schema.json",
        embedded_schema!("low-quad-draft-durable-prepare-request.schema.json"),
    ),
    (
        "production-knife-uv-bake-v2-get-request.schema.json",
        embedded_schema!("production-knife-uv-bake-v2-get-request.schema.json"),
    ),
    (
        "production-knife-uv-bake-v2-prepare-request.schema.json",
        embedded_schema!("production-knife-uv-bake-v2-prepare-request.schema.json"),
    ),
    (
        "production-knife-uv-bake-v2-result.schema.json",
        embedded_schema!("production-knife-uv-bake-v2-result.schema.json"),
    ),
    (
        "mechanical-animation-clip-get-request.schema.json",
        embedded_schema!("mechanical-animation-clip-get-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-prepare-request.schema.json",
        embedded_schema!("mechanical-animation-clip-prepare-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-v2-get-request.schema.json",
        embedded_schema!("mechanical-animation-clip-v2-get-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-v2-prepare-request.schema.json",
        embedded_schema!("mechanical-animation-clip-v2-prepare-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-v2-preview-request.schema.json",
        embedded_schema!("mechanical-animation-clip-v2-preview-request.schema.json"),
    ),
    (
        "mechanical-animation-glb-v2-get-request.schema.json",
        embedded_schema!("mechanical-animation-glb-v2-get-request.schema.json"),
    ),
    (
        "mechanical-animation-glb-v2-prepare-request.schema.json",
        embedded_schema!("mechanical-animation-glb-v2-prepare-request.schema.json"),
    ),
    (
        "production-stage-transition-get-request.schema.json",
        embedded_schema!("production-stage-transition-get-request.schema.json"),
    ),
    (
        "production-weapon-form-quality-get-request.schema.json",
        embedded_schema!("production-weapon-form-quality-get-request.schema.json"),
    ),
    (
        "production-weapon-form-quality-prepare-request.schema.json",
        embedded_schema!("production-weapon-form-quality-prepare-request.schema.json"),
    ),
    (
        "production-weapon-form-quality-v2-get-request.schema.json",
        embedded_schema!("production-weapon-form-quality-v2-get-request.schema.json"),
    ),
    (
        "production-weapon-form-quality-v2-preflight-get-request.schema.json",
        embedded_schema!("production-weapon-form-quality-v2-preflight-get-request.schema.json"),
    ),
    (
        "production-weapon-form-quality-v2-prepare-request.schema.json",
        embedded_schema!("production-weapon-form-quality-v2-prepare-request.schema.json"),
    ),
    (
        "production-weapon-formal-high-get-request.schema.json",
        embedded_schema!("production-weapon-formal-high-get-request.schema.json"),
    ),
    (
        "production-weapon-formal-high-prepare-request.schema.json",
        embedded_schema!("production-weapon-formal-high-prepare-request.schema.json"),
    ),
    (
        "production-weapon-high-low-bake-get-request.schema.json",
        embedded_schema!("production-weapon-high-low-bake-get-request.schema.json"),
    ),
    (
        "production-weapon-high-low-bake-preflight-get-request.schema.json",
        embedded_schema!("production-weapon-high-low-bake-preflight-get-request.schema.json"),
    ),
    (
        "production-weapon-high-low-bake-prepare-request.schema.json",
        embedded_schema!("production-weapon-high-low-bake-prepare-request.schema.json"),
    ),
    (
        "reference-import-request.schema.json",
        embedded_schema!("reference-import-request.schema.json"),
    ),
    (
        "repair-apply-confirm-request.schema.json",
        embedded_schema!("repair-apply-confirm-request.schema.json"),
    ),
    (
        "silhouette-rig-hash-request.schema.json",
        embedded_schema!("silhouette-rig-hash-request.schema.json"),
    ),
    (
        "game-weapon-glb-socket-materialization-get-request.schema.json",
        embedded_schema!("game-weapon-glb-socket-materialization-get-request.schema.json"),
    ),
    (
        "game-weapon-glb-socket-materialization-prepare-request.schema.json",
        embedded_schema!("game-weapon-glb-socket-materialization-prepare-request.schema.json"),
    ),
    (
        "cross-view-promotion-request.schema.json",
        embedded_schema!("cross-view-promotion-request.schema.json"),
    ),
    // WPN-ARCH-MCP-SCHEMA-002 package-owned active request roots. These are
    // extracted from the compiled compatibility wire shape, then registered
    // here only after their package files and root closure pass the checker.
    (
        "appearance-prepare-request.schema.json",
        embedded_schema!("appearance-prepare-request.schema.json"),
    ),
    (
        "artifact-readback-get-request.schema.json",
        embedded_schema!("artifact-readback-get-request.schema.json"),
    ),
    (
        "authoring-mesh-durable-get-request.schema.json",
        embedded_schema!("authoring-mesh-durable-get-request.schema.json"),
    ),
    (
        "authoring-mesh-durable-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-durable-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-identity-lineage-get-request.schema.json",
        embedded_schema!("authoring-mesh-identity-lineage-get-request.schema.json"),
    ),
    (
        "authoring-mesh-identity-lineage-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-identity-lineage-prepare-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-durable-get-request.schema.json",
        embedded_schema!("authoring-mesh-v2-durable-get-request.schema.json"),
    ),
    (
        "authoring-mesh-v2-durable-prepare-request.schema.json",
        embedded_schema!("authoring-mesh-v2-durable-prepare-request.schema.json"),
    ),
    (
        "production-weapon-authoring-mesh-v2-source-prepare-request.schema.json",
        embedded_schema!("production-weapon-authoring-mesh-v2-source-prepare-request.schema.json"),
    ),
    (
        "authoring-topology-get-request.schema.json",
        embedded_schema!("authoring-topology-get-request.schema.json"),
    ),
    (
        "candidate-get-request.schema.json",
        embedded_schema!("candidate-get-request.schema.json"),
    ),
    (
        "change-prepare-request.schema.json",
        embedded_schema!("change-prepare-request.schema.json"),
    ),
    (
        "checkpoint-get-request.schema.json",
        embedded_schema!("checkpoint-get-request.schema.json"),
    ),
    (
        "checkpoint-prepare-request.schema.json",
        embedded_schema!("checkpoint-prepare-request.schema.json"),
    ),
    (
        "checkpoint-restore-prepare-request.schema.json",
        embedded_schema!("checkpoint-restore-prepare-request.schema.json"),
    ),
    (
        "critic-report-get-request.schema.json",
        embedded_schema!("critic-report-get-request.schema.json"),
    ),
    (
        "design-action-run-prepare-request.schema.json",
        embedded_schema!("design-action-run-prepare-request.schema.json"),
    ),
    (
        "fps-presentation-package-v2-production-preflight-get-request.schema.json",
        embedded_schema!(
            "fps-presentation-package-v2-production-preflight-get-request.schema.json"
        ),
    ),
    (
        "game-weapon-animated-glb-socket-get-request.schema.json",
        embedded_schema!("game-weapon-animated-glb-socket-get-request.schema.json"),
    ),
    (
        "game-weapon-animated-glb-socket-prepare-request.schema.json",
        embedded_schema!("game-weapon-animated-glb-socket-prepare-request.schema.json"),
    ),
    (
        "geometry-prepare-request.schema.json",
        embedded_schema!("geometry-prepare-request.schema.json"),
    ),
    (
        "human-visual-review-submit-request.schema.json",
        embedded_schema!("human-visual-review-submit-request.schema.json"),
    ),
    (
        "job-cancel-request.schema.json",
        embedded_schema!("job-cancel-request.schema.json"),
    ),
    (
        "job-events-read-request.schema.json",
        embedded_schema!("job-events-read-request.schema.json"),
    ),
    (
        "job-get-request.schema.json",
        embedded_schema!("job-get-request.schema.json"),
    ),
    (
        "job-result-get-request.schema.json",
        embedded_schema!("job-result-get-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-preview-get-request.schema.json",
        embedded_schema!("mechanical-animation-clip-preview-get-request.schema.json"),
    ),
    (
        "optimization-job-get-request.schema.json",
        embedded_schema!("optimization-job-get-request.schema.json"),
    ),
    (
        "optimization-job-prepare-request.schema.json",
        embedded_schema!("optimization-job-prepare-request.schema.json"),
    ),
    (
        "optimization-job-resume-request.schema.json",
        embedded_schema!("optimization-job-resume-request.schema.json"),
    ),
    (
        "primary-form-repair-job-prepare-request.schema.json",
        embedded_schema!("primary-form-repair-job-prepare-request.schema.json"),
    ),
    (
        "production-weapon-retopology-cage-source-get-request.schema.json",
        embedded_schema!("production-weapon-retopology-cage-source-get-request.schema.json"),
    ),
    (
        "production-weapon-retopology-cage-source-prepare-request.schema.json",
        embedded_schema!("production-weapon-retopology-cage-source-prepare-request.schema.json"),
    ),
    (
        "project-create-request.schema.json",
        embedded_schema!("project-create-request.schema.json"),
    ),
    (
        "project-get-request.schema.json",
        embedded_schema!("project-get-request.schema.json"),
    ),
    (
        "quality-get-request.schema.json",
        embedded_schema!("quality-get-request.schema.json"),
    ),
    (
        "reference-compare-prepare-request.schema.json",
        embedded_schema!("reference-compare-prepare-request.schema.json"),
    ),
    (
        "high-artifact-reference-compare-prepare-request.schema.json",
        embedded_schema!("high-artifact-reference-compare-prepare-request.schema.json"),
    ),
    (
        "reference-comparison-prepare-result.schema.json",
        embedded_schema!("reference-comparison-prepare-result.schema.json"),
    ),
    (
        "high-artifact-reference-comparison-prepare-result.schema.json",
        embedded_schema!("high-artifact-reference-comparison-prepare-result.schema.json"),
    ),
    (
        "reference-get-request.schema.json",
        embedded_schema!("reference-get-request.schema.json"),
    ),
    (
        "knife-reference-intent-bundle-get-request.schema.json",
        embedded_schema!("knife-reference-intent-bundle-get-request.schema.json"),
    ),
    (
        "knife-reference-intent-bundle-prepare-request.schema.json",
        embedded_schema!("knife-reference-intent-bundle-prepare-request.schema.json"),
    ),
    (
        "knife-reference-intent-bundle.schema.json",
        embedded_schema!("knife-reference-intent-bundle.schema.json"),
    ),
    (
        "knife-reference-intent-bundle-result.schema.json",
        embedded_schema!("knife-reference-intent-bundle-result.schema.json"),
    ),
    (
        "knife-source-binding-get-request.schema.json",
        embedded_schema!("knife-source-binding-get-request.schema.json"),
    ),
    (
        "knife-source-binding-prepare-request.schema.json",
        embedded_schema!("knife-source-binding-prepare-request.schema.json"),
    ),
    (
        "knife-source-binding.schema.json",
        embedded_schema!("knife-source-binding.schema.json"),
    ),
    (
        "knife-source-binding-result.schema.json",
        embedded_schema!("knife-source-binding-result.schema.json"),
    ),
    (
        "weaponry-knife-production-brief-get-request.schema.json",
        embedded_schema!("weaponry-knife-production-brief-get-request.schema.json"),
    ),
    (
        "weaponry-knife-production-brief-prepare-request.schema.json",
        embedded_schema!("weaponry-knife-production-brief-prepare-request.schema.json"),
    ),
    (
        "weaponry-knife-production-brief.schema.json",
        embedded_schema!("weaponry-knife-production-brief.schema.json"),
    ),
    (
        "weaponry-knife-production-brief-result.schema.json",
        embedded_schema!("weaponry-knife-production-brief-result.schema.json"),
    ),
    (
        "reference-mask-prepare-request.schema.json",
        embedded_schema!("reference-mask-prepare-request.schema.json"),
    ),
    (
        "reference-mask-refine-prepare-request.schema.json",
        embedded_schema!("reference-mask-refine-prepare-request.schema.json"),
    ),
    (
        "render-evidence-integrity-get-request.schema.json",
        embedded_schema!("render-evidence-integrity-get-request.schema.json"),
    ),
    (
        "render-evidence-replay-get-request.schema.json",
        embedded_schema!("render-evidence-replay-get-request.schema.json"),
    ),
    (
        "render-pass-get-request.schema.json",
        embedded_schema!("render-pass-get-request.schema.json"),
    ),
    (
        "repair-apply-prepare-request.schema.json",
        embedded_schema!("repair-apply-prepare-request.schema.json"),
    ),
    (
        "repair-intent-run-prepare-request.schema.json",
        embedded_schema!("repair-intent-run-prepare-request.schema.json"),
    ),
    (
        "restore-confirm-request.schema.json",
        embedded_schema!("restore-confirm-request.schema.json"),
    ),
    (
        "restore-prepare-request.schema.json",
        embedded_schema!("restore-prepare-request.schema.json"),
    ),
    (
        "scene-observe-get-request.schema.json",
        embedded_schema!("scene-observe-get-request.schema.json"),
    ),
    (
        "session-create-or-resume-request.schema.json",
        embedded_schema!("session-create-or-resume-request.schema.json"),
    ),
    (
        "session-get-request.schema.json",
        embedded_schema!("session-get-request.schema.json"),
    ),
    (
        "silhouette-candidate-compare-request.schema.json",
        embedded_schema!("silhouette-candidate-compare-request.schema.json"),
    ),
    (
        "silhouette-evaluation-objective-prepare-request.schema.json",
        embedded_schema!("silhouette-evaluation-objective-prepare-request.schema.json"),
    ),
    (
        "silhouette-fit-prepare-request.schema.json",
        embedded_schema!("silhouette-fit-prepare-request.schema.json"),
    ),
    (
        "silhouette-part-error-get-request.schema.json",
        embedded_schema!("silhouette-part-error-get-request.schema.json"),
    ),
    (
        "silhouette-target-get-request.schema.json",
        embedded_schema!("silhouette-target-get-request.schema.json"),
    ),
    (
        "snapshot-get-request.schema.json",
        embedded_schema!("snapshot-get-request.schema.json"),
    ),
    (
        "visual-evidence-bundle-get-request.schema.json",
        embedded_schema!("visual-evidence-bundle-get-request.schema.json"),
    ),
    (
        "visual-review-submit-request.schema.json",
        embedded_schema!("visual-review-submit-request.schema.json"),
    ),
    // Transitive package-schema documents referenced by the active roots
    // above.  They are data dependencies, not additional active operations.
    (
        "authoring-mesh-edit-preview-request.schema.json",
        embedded_schema!("authoring-mesh-edit-preview-request.schema.json"),
    ),
    (
        "authoring-mesh-transaction.schema.json",
        embedded_schema!("authoring-mesh-transaction.schema.json"),
    ),
    (
        "geometry-program-v2.schema.json",
        embedded_schema!("geometry-program-v2.schema.json"),
    ),
    (
        "low-quad-draft-worker-request.schema.json",
        embedded_schema!("low-quad-draft-worker-request.schema.json"),
    ),
    (
        "mechanical-pose-sequence-preview-request.schema.json",
        embedded_schema!("mechanical-pose-sequence-preview-request.schema.json"),
    ),
    (
        "mechanical-animation-clip-v2.schema.json",
        embedded_schema!("mechanical-animation-clip-v2.schema.json"),
    ),
    (
        "mechanical-pose-action.schema.json",
        embedded_schema!("mechanical-pose-action.schema.json"),
    ),
    (
        "mechanical-rest-frame.schema.json",
        embedded_schema!("mechanical-rest-frame.schema.json"),
    ),
    (
        "production-weapon-form-quality.schema.json",
        embedded_schema!("production-weapon-form-quality.schema.json"),
    ),
    (
        "quality-report-v2.schema.json",
        embedded_schema!("quality-report-v2.schema.json"),
    ),
    (
        "reference-comparison-report.schema.json",
        embedded_schema!("reference-comparison-report.schema.json"),
    ),
    (
        "render-profile.schema.json",
        embedded_schema!("render-profile.schema.json"),
    ),
    (
        "render-set-v2.schema.json",
        embedded_schema!("render-set-v2.schema.json"),
    ),
    (
        "authoring-topology-request.schema.json",
        embedded_schema!("authoring-topology-request.schema.json"),
    ),
    (
        "camera-calibration.schema.json",
        embedded_schema!("camera-calibration.schema.json"),
    ),
    (
        "camera-calibration-v2.schema.json",
        embedded_schema!("camera-calibration-v2.schema.json"),
    ),
    (
        "camera-calibration-ref.schema.json",
        embedded_schema!("camera-calibration-ref.schema.json"),
    ),
    (
        "reference-view-spec.schema.json",
        embedded_schema!("reference-view-spec.schema.json"),
    ),
    (
        "silhouette-rig.schema.json",
        embedded_schema!("silhouette-rig.schema.json"),
    ),
];

/// Resolve package-local `$ref` documents before exposing/validating a root
/// schema.  Contracts currently publishes a few exact active roots that refer
/// to sibling schema documents by URL.  Keeping those documents in this
/// compile-time index makes the resolver exact without network access or a
/// compatibility-handler fallback.  Local references are expanded as well so
/// a referenced document keeps the `$defs` namespace it was authored with.
fn materialize_schema(schema: Value, operation: &str) -> Result<Value, String> {
    let mut stack = vec![format!("operation:{operation}")];
    materialize_schema_node(&schema, &schema, "$", &mut stack, 0)
        .map_err(|detail| format!("WEAPONRY_ACTIVE_SCHEMA_INVALID: {operation} {detail}"))
}

fn materialize_schema_node(
    schema: &Value,
    root: &Value,
    path: &str,
    stack: &mut Vec<String>,
    depth: usize,
) -> Result<Value, String> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(format!("{path} reference expansion exceeds bounded depth"));
    }
    let Some(object) = schema.as_object() else {
        let Some(array) = schema.as_array() else {
            return Ok(schema.clone());
        };
        return array
            .iter()
            .enumerate()
            .map(|(index, item)| {
                materialize_schema_node(item, root, &format!("{path}[{index}]"), stack, depth + 1)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    };

    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let stack_key = format!("{}:{reference}", root_pointer_identity(root));
        if stack.iter().any(|entry| entry == &stack_key) {
            return Err(format!("{path} contains a cyclic $ref {reference}"));
        }
        stack.push(stack_key);
        let expanded = if reference.starts_with('#') {
            let target = resolve_json_pointer(root, reference)?;
            materialize_schema_node(target, root, path, stack, depth + 1)
        } else {
            let (document_name, fragment) = external_reference_parts(reference)?;
            let source = embedded_document(document_name).ok_or_else(|| {
                format!("{path} external $ref {reference} is not an embedded package document")
            })?;
            let document: Value = serde_json::from_str(source).map_err(|error| {
                format!("{path} referenced schema {document_name} is not valid JSON: {error}")
            })?;
            let target = fragment
                .map(|fragment| resolve_json_pointer(&document, fragment))
                .transpose()?
                .unwrap_or(&document);
            materialize_schema_node(target, &document, path, stack, depth + 1)
        }?;
        stack.pop();
        let mut expanded = expanded;
        if let Some(description) = object.get("description") {
            if let Some(expanded_object) = expanded.as_object_mut() {
                expanded_object.insert("description".to_owned(), description.clone());
            }
        }
        return Ok(expanded);
    }

    object
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                materialize_schema_node(value, root, &format!("{path}.{key}"), stack, depth + 1)?,
            ))
        })
        .collect::<Result<Map<_, _>, String>>()
        .map(Value::Object)
}

// The pointer is used only to distinguish a local and external document while
// detecting recursive expansion.  A stable address is sufficient within one
// bounded expansion call and is never exposed in a schema or receipt.
fn root_pointer_identity(root: &Value) -> usize {
    root as *const Value as usize
}

fn embedded_document(name: &str) -> Option<&'static str> {
    EMBEDDED_SCHEMA_DOCUMENTS
        .iter()
        .find_map(|(candidate, source)| (*candidate == name).then_some(*source))
}

fn external_reference_parts(reference: &str) -> Result<(&str, Option<&str>), String> {
    let (document, fragment) = match reference.find('#') {
        Some(index) => (&reference[..index], Some(&reference[index..])),
        None => (reference, None),
    };
    let document = document
        .strip_prefix("https://forgecad.local/contracts/")
        .unwrap_or(document);
    if document.is_empty()
        || document.contains('/')
        || !document.ends_with(".schema.json")
        || !document
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(format!(
            "schema reference {reference} is outside the embedded Contracts namespace"
        ));
    }
    Ok((document, fragment))
}

fn resolve_json_pointer<'a>(root: &'a Value, pointer: &str) -> Result<&'a Value, String> {
    let pointer = pointer
        .strip_prefix('#')
        .ok_or_else(|| format!("schema reference {pointer} must be a local JSON pointer"))?;
    if pointer.is_empty() {
        return Ok(root);
    }
    let mut current = root;
    for token in pointer
        .strip_prefix('/')
        .ok_or_else(|| format!("schema reference #{pointer} is not a JSON pointer"))?
        .split('/')
    {
        let token = token.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(object) => object
                .get(&token)
                .ok_or_else(|| format!("schema reference #{pointer} target is missing"))?,
            Value::Array(array) => token
                .parse::<usize>()
                .ok()
                .and_then(|index| array.get(index))
                .ok_or_else(|| format!("schema reference #{pointer} target is missing"))?,
            _ => return Err(format!("schema reference #{pointer} crosses a scalar")),
        };
    }
    Ok(current)
}

#[derive(Debug, Clone, Copy)]
struct ValidationBudget {
    nodes: usize,
}

impl Default for ValidationBudget {
    fn default() -> Self {
        Self {
            nodes: MAX_SCHEMA_NODES,
        }
    }
}

impl ValidationBudget {
    fn consume(&mut self, depth: usize) -> Result<(), String> {
        if depth > MAX_SCHEMA_DEPTH || self.nodes == 0 {
            return Err("schema exceeds bounded validation budget".to_owned());
        }
        self.nodes -= 1;
        Ok(())
    }
}

fn validate_schema_shape(operation: &str, schema: &Value) -> Result<(), String> {
    let mut budget = ValidationBudget::default();
    validate_schema_shape_inner(schema, schema, "$", 0, &mut budget)
        .map_err(|detail| format!("WEAPONRY_ACTIVE_SCHEMA_INVALID: {operation} {detail}"))
}

fn validate_schema_shape_inner(
    schema: &Value,
    root: &Value,
    path: &str,
    depth: usize,
    budget: &mut ValidationBudget,
) -> Result<(), String> {
    budget.consume(depth)?;
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path} schema must be an object"))?;
    if let Some(reference) = object.get("$ref") {
        if object
            .keys()
            .any(|key| key != "$ref" && key != "description")
        {
            return Err(format!("{path} $ref cannot have sibling keywords"));
        }
        let reference = reference
            .as_str()
            .ok_or_else(|| format!("{path} $ref must be a string"))?;
        let target = resolve_local_ref(root, reference)?;
        return validate_schema_shape_inner(target, root, path, depth + 1, budget);
    }
    const ALLOWED: &[&str] = &[
        "$schema",
        "$id",
        "$comment",
        "$defs",
        "title",
        "type",
        "required",
        "properties",
        "additionalProperties",
        "oneOf",
        "anyOf",
        "allOf",
        "not",
        "if",
        "then",
        "else",
        "const",
        "enum",
        "minLength",
        "maxLength",
        "pattern",
        "minimum",
        "exclusiveMinimum",
        "maximum",
        "exclusiveMaximum",
        "finite",
        "maxProperties",
        "minProperties",
        "prefixItems",
        "items",
        "minItems",
        "maxItems",
        "uniqueItems",
        "description",
    ];
    if let Some(key) = object.keys().find(|key| !ALLOWED.contains(&key.as_str())) {
        return Err(format!("{path} contains unsupported keyword {key}"));
    }
    if let Some(kind) = object.get("type") {
        validate_type_shape(kind, path)?;
    }
    if let Some(finite) = object.get("finite") {
        if finite != &Value::Bool(true) {
            return Err(format!("{path}.finite must be true"));
        }
    }
    if let Some(additional) = object.get("additionalProperties") {
        if !additional.is_boolean() && !additional.is_object() {
            return Err(format!(
                "{path}.additionalProperties must be boolean or schema"
            ));
        }
        if let Some(additional_schema) = additional.as_object() {
            validate_schema_shape_inner(
                &Value::Object(additional_schema.clone()),
                root,
                &format!("{path}.additionalProperties"),
                depth + 1,
                budget,
            )?;
        }
    }
    if let Some(required) = object.get("required") {
        let entries = required
            .as_array()
            .ok_or_else(|| format!("{path}.required must be an array"))?;
        for entry in entries {
            if entry.as_str().is_none() {
                return Err(format!("{path}.required contains a non-string"));
            }
        }
    }
    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| format!("{path}.properties must be an object"))?;
        for (name, property) in properties {
            validate_schema_shape_inner(
                property,
                root,
                &format!("{path}.properties.{name}"),
                depth + 1,
                budget,
            )?;
        }
    }
    let min_properties = object
        .get("minProperties")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("{path}.minProperties must be a non-negative integer"))
        })
        .transpose()?;
    let max_properties = object
        .get("maxProperties")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("{path}.maxProperties must be a non-negative integer"))
        })
        .transpose()?;
    if min_properties
        .zip(max_properties)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(format!(
            "{path}.minProperties must not exceed maxProperties"
        ));
    }
    for keyword in ["oneOf", "anyOf", "allOf"] {
        if let Some(alternatives) = object.get(keyword) {
            let alternatives = alternatives
                .as_array()
                .filter(|items| !items.is_empty())
                .ok_or_else(|| format!("{path}.{keyword} must be a non-empty array"))?;
            for (index, alternative) in alternatives.iter().enumerate() {
                validate_schema_shape_inner(
                    alternative,
                    root,
                    &format!("{path}.{keyword}[{index}]"),
                    depth + 1,
                    budget,
                )?;
            }
        }
    }
    if let Some(not_schema) = object.get("not") {
        validate_schema_shape_inner(not_schema, root, &format!("{path}.not"), depth + 1, budget)?;
    }
    if let Some(if_schema) = object.get("if") {
        validate_schema_shape_inner(if_schema, root, &format!("{path}.if"), depth + 1, budget)?;
    }
    if let Some(then_schema) = object.get("then") {
        validate_schema_shape_inner(
            then_schema,
            root,
            &format!("{path}.then"),
            depth + 1,
            budget,
        )?;
    }
    if let Some(else_schema) = object.get("else") {
        validate_schema_shape_inner(
            else_schema,
            root,
            &format!("{path}.else"),
            depth + 1,
            budget,
        )?;
    }
    if let Some(prefix_items) = object.get("prefixItems") {
        let prefix_items = prefix_items
            .as_array()
            .ok_or_else(|| format!("{path}.prefixItems must be an array"))?;
        for (index, item) in prefix_items.iter().enumerate() {
            validate_schema_shape_inner(
                item,
                root,
                &format!("{path}.prefixItems[{index}]"),
                depth + 1,
                budget,
            )?;
        }
    }
    if let Some(items) = object.get("items") {
        if !items.is_boolean() {
            validate_schema_shape_inner(items, root, &format!("{path}.items"), depth + 1, budget)?;
        }
    }
    if let Some(definitions) = object.get("$defs") {
        let definitions = definitions
            .as_object()
            .ok_or_else(|| format!("{path}.$defs must be an object"))?;
        for (name, definition) in definitions {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                return Err(format!("{path}.$defs contains an invalid name"));
            }
            validate_schema_shape_inner(
                definition,
                root,
                &format!("{path}.$defs.{name}"),
                depth + 1,
                budget,
            )?;
        }
    }
    Ok(())
}

fn validate_value(
    schema: &Value,
    root: &Value,
    value: &Value,
    path: &str,
    budget: &mut ValidationBudget,
) -> Result<(), String> {
    budget.consume(path.matches('.').count())?;
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path} schema must be an object"))?;
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let target = resolve_local_ref(root, reference)?;
        return validate_value(target, root, value, path, budget);
    }
    if let Some(kind) = object.get("type") {
        if !value_matches_type(kind, value) {
            return Err(format!("at {path}: value has the wrong type"));
        }
    }
    if let Some(expected) = object.get("const") {
        if value != expected {
            return Err(format!("at {path}: const does not match"));
        }
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|candidate| candidate == value) {
            return Err(format!("at {path}: value is not in enum"));
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        let object_value = value
            .as_object()
            .ok_or_else(|| format!("at {path}: object is required"))?;
        for field in required.iter().filter_map(Value::as_str) {
            if !object_value.contains_key(field) {
                return Err(format!("at {path}: required field {field} is missing"));
            }
        }
    }
    if let Some(object_value) = value.as_object() {
        let property_count = object_value.len() as u64;
        if object
            .get("minProperties")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| property_count < minimum)
            || object
                .get("maxProperties")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| property_count > maximum)
        {
            return Err(format!(
                "at {path}: object property count constraint failed"
            ));
        }
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        let object_value = value
            .as_object()
            .ok_or_else(|| format!("at {path}: object is required for properties"))?;
        if object.get("additionalProperties") == Some(&Value::Bool(false))
            && object_value.keys().any(|key| !properties.contains_key(key))
        {
            return Err(format!("at {path}: unknown field"));
        }
        for (field, property_schema) in properties {
            if let Some(property_value) = object_value.get(field) {
                validate_value(
                    property_schema,
                    root,
                    property_value,
                    &format!("{path}.{field}"),
                    budget,
                )?;
            }
        }
    } else if object.get("additionalProperties") == Some(&Value::Bool(false))
        && value.as_object().is_none_or(|object| !object.is_empty())
    {
        return Err(format!("at {path}: object has no declared fields"));
    }
    if let Some(additional) = object
        .get("additionalProperties")
        .and_then(Value::as_object)
    {
        if let (Some(properties), Some(object_value)) = (
            object.get("properties").and_then(Value::as_object),
            value.as_object(),
        ) {
            for (field, property_value) in object_value {
                if !properties.contains_key(field) {
                    validate_value(
                        &Value::Object(additional.clone()),
                        root,
                        property_value,
                        &format!("{path}.{field}"),
                        budget,
                    )?;
                }
            }
        }
    }
    for (keyword, require_one) in [("allOf", false), ("oneOf", true), ("anyOf", false)] {
        let Some(alternatives) = object.get(keyword).and_then(Value::as_array) else {
            continue;
        };
        let mut matches = 0;
        for alternative in alternatives {
            let mut alternative_budget = *budget;
            if validate_value(alternative, root, value, path, &mut alternative_budget).is_ok() {
                matches += 1;
            }
        }
        if (require_one && matches != 1) || (!require_one && keyword == "anyOf" && matches == 0) {
            return Err(format!("at {path}: {keyword} alternatives do not match"));
        }
        if keyword == "allOf" {
            for alternative in alternatives {
                validate_value(alternative, root, value, path, budget)?;
            }
        }
    }
    if let Some(not_schema) = object.get("not") {
        let mut not_budget = *budget;
        if validate_value(not_schema, root, value, path, &mut not_budget).is_ok() {
            return Err(format!("at {path}: not schema matched"));
        }
    }
    if let Some(if_schema) = object.get("if") {
        let mut condition_budget = *budget;
        if validate_value(if_schema, root, value, path, &mut condition_budget).is_ok() {
            if let Some(then_schema) = object.get("then") {
                validate_value(then_schema, root, value, path, budget)?;
            }
        } else if let Some(else_schema) = object.get("else") {
            validate_value(else_schema, root, value, path, budget)?;
        }
    }
    if let Some(number) = value.as_f64() {
        for (keyword, valid) in [
            (
                "minimum",
                object
                    .get("minimum")
                    .and_then(Value::as_f64)
                    .map(|n| number >= n),
            ),
            (
                "exclusiveMinimum",
                object
                    .get("exclusiveMinimum")
                    .and_then(Value::as_f64)
                    .map(|n| number > n),
            ),
            (
                "maximum",
                object
                    .get("maximum")
                    .and_then(Value::as_f64)
                    .map(|n| number <= n),
            ),
            (
                "exclusiveMaximum",
                object
                    .get("exclusiveMaximum")
                    .and_then(Value::as_f64)
                    .map(|n| number < n),
            ),
        ] {
            if valid == Some(false) {
                return Err(format!("at {path}: {keyword} constraint failed"));
            }
        }
    }
    if let Some(string) = value.as_str() {
        let length = string.chars().count();
        if object
            .get("minLength")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| (length as u64) < minimum)
            || object
                .get("maxLength")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| (length as u64) > maximum)
        {
            return Err(format!("at {path}: string length constraint failed"));
        }
        if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
            if !matches_pattern(pattern, string) {
                return Err(format!("at {path}: string pattern constraint failed"));
            }
        }
    }
    if let Some(items) = object.get("items") {
        let values = value
            .as_array()
            .ok_or_else(|| format!("at {path}: array is required for items"))?;
        if object
            .get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| (values.len() as u64) < minimum)
            || object
                .get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| (values.len() as u64) > maximum)
        {
            return Err(format!("at {path}: array length constraint failed"));
        }
        if object.get("uniqueItems") == Some(&Value::Bool(true))
            && values
                .iter()
                .enumerate()
                .any(|(index, value)| values[index + 1..].iter().any(|other| other == value))
        {
            return Err(format!("at {path}: array items must be unique"));
        }
        if let Some(prefix_items) = object.get("prefixItems").and_then(Value::as_array) {
            for (index, item_schema) in prefix_items.iter().enumerate() {
                if let Some(item) = values.get(index) {
                    validate_value(item_schema, root, item, &format!("{path}[{index}]"), budget)?;
                }
            }
            if items == &Value::Bool(false) && values.len() > prefix_items.len() {
                return Err(format!("at {path}: array has items after prefixItems"));
            }
            if items == &Value::Bool(true) {
                return Ok(());
            }
            if let Some(item_schema) = items.as_object() {
                for (index, item) in values.iter().enumerate().skip(prefix_items.len()) {
                    validate_value(
                        &Value::Object(item_schema.clone()),
                        root,
                        item,
                        &format!("{path}[{index}]"),
                        budget,
                    )?;
                }
            }
        } else if items == &Value::Bool(false) {
            if !values.is_empty() {
                return Err(format!("at {path}: array items are forbidden"));
            }
        } else if items == &Value::Bool(true) {
            return Ok(());
        } else {
            for (index, item) in values.iter().enumerate() {
                validate_value(items, root, item, &format!("{path}[{index}]"), budget)?;
            }
        }
    }
    Ok(())
}

fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let name = reference
        .strip_prefix("#/$defs/")
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 64
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        .ok_or_else(|| "schema reference must be a bounded local #/$defs reference".to_owned())?;
    root.get("$defs")
        .and_then(Value::as_object)
        .and_then(|definitions| definitions.get(name))
        .ok_or_else(|| format!("schema reference #/$defs/{name} is missing"))
}

fn validate_type_shape(schema_type: &Value, path: &str) -> Result<(), String> {
    let valid = |kind: &str| {
        matches!(
            kind,
            "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
        )
    };
    match schema_type {
        Value::String(kind) if valid(kind) => Ok(()),
        Value::Array(kinds)
            if !kinds.is_empty() && kinds.iter().all(|kind| kind.as_str().is_some_and(valid)) =>
        {
            Ok(())
        }
        _ => Err(format!("{path}.type is invalid")),
    }
}

fn value_matches_type(schema_type: &Value, value: &Value) -> bool {
    let matches = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    match schema_type {
        Value::String(kind) => matches(kind),
        Value::Array(kinds) => kinds.iter().any(|kind| kind.as_str().is_some_and(matches)),
        _ => false,
    }
}

fn matches_pattern(pattern: &str, value: &str) -> bool {
    match pattern {
        "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]{1,6})?Z$" => {
            matches_utc_timestamp(value)
        }
        "^20[0-9]{2}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$" => {
            value.starts_with("20") && matches_utc_timestamp(value) && !value.contains('.')
        }
        "^[^\\r\\n]{1,512}$" => {
            let length = value.chars().count();
            (1..=512).contains(&length)
                && !value
                    .chars()
                    .any(|character| matches!(character, '\r' | '\n'))
        }
        "^[^\\u0000-\\u001f\\u007f]*$" => !value
            .chars()
            .any(|character| character <= '\u{001f}' || character == '\u{007f}'),
        "^[0-9a-f]{64}$" => {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        }
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$" => {
            is_opaque_id(value)
                && value
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
        }
        "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$" => {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
        }
        "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}:(front|back|left|right|front-three-quarter|rear-three-quarter|top|bottom|fps-hold|fps-inspect)$" => {
            let Some((reference_id, view)) = value.rsplit_once(':') else {
                return false;
            };
            is_opaque_id(reference_id)
                && reference_id
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && matches!(
                    view,
                    "front"
                        | "back"
                        | "left"
                        | "right"
                        | "front-three-quarter"
                        | "rear-three-quarter"
                        | "top"
                        | "bottom"
                        | "fps-hold"
                        | "fps-inspect"
                )
        }
        "^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}:(front|back|left|right|front-three-quarter|rear-three-quarter|top|bottom|fps-hold|fps-inspect)$" => {
            let Some((reference_id, view)) = value.rsplit_once(':') else {
                return false;
            };
            !reference_id.is_empty()
                && reference_id.len() <= 128
                && reference_id
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && reference_id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
                && matches!(
                    view,
                    "front"
                        | "back"
                        | "left"
                        | "right"
                        | "front-three-quarter"
                        | "rear-three-quarter"
                        | "top"
                        | "bottom"
                        | "fps-hold"
                        | "fps-inspect"
                )
        }
        "^[A-Za-z0-9_.-]{1,128}$" => {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        }
        "^[A-Za-z0-9_.@-]{1,128}$" => {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'@' | b'-')
                })
        }
        "^[A-Za-z0-9_.:-]{1,128}$" => {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
        }
        "^[A-Za-z0-9._:-]+$" => {
            !value.is_empty()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
        }
        "^[0-9]{1,10}$" => {
            !value.is_empty()
                && value.len() <= 10
                && value.bytes().all(|byte| byte.is_ascii_digit())
        }
        "^ProductionStageTransition@[0-9]+$" => value
            .strip_prefix("ProductionStageTransition@")
            .is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            }),
        _ => false,
    }
}

fn matches_utc_timestamp(value: &str) -> bool {
    let (whole_seconds, fraction) = match value.strip_suffix('Z') {
        Some(value) => match value.split_once('.') {
            Some((whole_seconds, fraction)) => (whole_seconds, Some(fraction)),
            None => (value, None),
        },
        None => return false,
    };
    if whole_seconds.len() != 19 {
        return false;
    }
    let bytes = whole_seconds.as_bytes();
    if !bytes.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 => *byte == b'-',
        10 => *byte == b'T',
        13 | 16 => *byte == b':',
        _ => byte.is_ascii_digit(),
    }) {
        return false;
    }
    let parse = |range: std::ops::Range<usize>| {
        whole_seconds[range]
            .parse::<u32>()
            .expect("timestamp digits were validated")
    };
    let month = parse(5..7);
    let day = parse(8..10);
    let hour = parse(11..13);
    let minute = parse(14..16);
    let second = parse(17..19);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return false;
    }
    fraction.is_none_or(|fraction| {
        (1..=6).contains(&fraction.len()) && fraction.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_timestamp_pattern_is_bounded_and_fail_closed() {
        let pattern = "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]{1,6})?Z$";
        assert!(matches_pattern(pattern, "2026-08-30T12:34:56Z"));
        assert!(matches_pattern(pattern, "2026-08-30T12:34:56.123456Z"));
        assert!(!matches_pattern(pattern, "2026-08-30 12:34:56Z"));
        assert!(!matches_pattern(pattern, "2026-08-30T12:34:56.1234567Z"));
        assert!(!matches_pattern(pattern, "2026-13-30T12:34:56Z"));
        let pass_state_pattern = "^20[0-9]{2}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$";
        assert!(matches_pattern(pass_state_pattern, "2026-08-30T12:34:56Z"));
        assert!(!matches_pattern(pass_state_pattern, "1999-08-30T12:34:56Z"));
        assert!(!matches_pattern(
            pass_state_pattern,
            "2026-08-30T12:34:56.1Z"
        ));
        assert!(matches_pattern("^[^\\r\\n]{1,512}$", "尼泊尔-屠龙"));
        assert!(!matches_pattern("^[^\\r\\n]{1,512}$", "line\nbreak"));
        let control_free_pattern = "^[^\\u0000-\\u001f\\u007f]*$";
        assert!(matches_pattern(
            control_free_pattern,
            "An authorized primary reference view is required."
        ));
        assert!(!matches_pattern(control_free_pattern, "line\nbreak"));
        assert!(!matches_pattern(control_free_pattern, "bad\u{007f}text"));
        assert!(matches_pattern(
            "^[A-Za-z0-9_.@-]{1,128}$",
            "fixed-ggx-material-shading@1"
        ));
        assert!(!matches_pattern(
            "^[A-Za-z0-9_.@-]{1,128}$",
            "fixed shading@1"
        ));
    }

    #[test]
    fn conditional_else_and_reference_view_pattern_are_enforced() {
        let evidence_region_pattern = "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}:(front|back|left|right|front-three-quarter|rear-three-quarter|top|bottom|fps-hold|fps-inspect)$";
        let resolver = StubResolver(json!({
            "type":"object",
            "required":["topic","view","region"],
            "properties":{
                "topic":{"enum":["reference-view","other"]},
                "view":{"anyOf":[{"type":"string"},{"type":"null"}]},
                "region":{"pattern":evidence_region_pattern}
            },
            "allOf":[{
                "if":{"properties":{"topic":{"const":"reference-view"}},"required":["topic"]},
                "then":{"properties":{"view":{"type":"string"}}},
                "else":{"properties":{"view":{"const":null}}}
            }],
            "additionalProperties":false
        }));
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"topic":"reference-view","view":"front","region":"reference-1:front"})
        )
        .is_ok());
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"topic":"other","view":null,"region":"reference-1:rear-three-quarter"})
        )
        .is_ok());
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"topic":"other","view":"front","region":"reference-1:front"})
        )
        .is_err());
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"topic":"reference-view","view":"front","region":"reference-1:side"})
        )
        .is_err());
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"topic":"reference-view","view":"front","region":"reference:segment:front"})
        )
        .is_err());
    }

    #[derive(Debug)]
    struct StubResolver(Value);

    impl ActiveRequestSchemaResolver for StubResolver {
        fn resolve(&self, _operation: &str) -> Result<Option<Value>, String> {
            Ok(Some(self.0.clone()))
        }
    }

    #[test]
    fn embedded_schema_resolver_is_independent_from_compatibility_handlers() {
        let resolver = default_resolver();
        let schema = resolver
            .resolve("game_asset_delivery_get")
            .expect("schema resolves")
            .expect("embedded contract exists");
        assert_eq!(schema["additionalProperties"], false);
        assert!(resolver
            .resolve("unknown_runtime_operation")
            .unwrap()
            .is_none());
        assert!(validate("unknown_runtime_operation", &json!({}))
            .unwrap_err()
            .contains("WEAPONRY_ACTIVE_SCHEMA_UNAVAILABLE"));
    }

    #[test]
    fn closed_injected_schema_rejects_unknown_fields_and_wrong_types() {
        let resolver = StubResolver(json!({
            "type":"object",
            "required":["project_id"],
            "properties":{"project_id":{"type":"string"}},
            "additionalProperties":false
        }));
        assert!(validate_with_resolver(&resolver, "probe", &json!({"project_id":"p"})).is_ok());
        assert!(validate_with_resolver(
            &resolver,
            "probe",
            &json!({"project_id":"p","script":"x"})
        )
        .is_err());
        assert!(validate_with_resolver(&resolver, "probe", &json!({"project_id":7})).is_err());
    }

    #[test]
    fn object_property_count_constraints_are_enforced() {
        let resolver = StubResolver(json!({
            "type":"object",
            "minProperties":1,
            "maxProperties":2,
            "properties":{
                "a":{"type":"string"},
                "b":{"type":"string"},
                "c":{"type":"string"}
            },
            "additionalProperties":false
        }));
        assert!(validate_with_resolver(&resolver, "probe", &json!({})).is_err());
        assert!(validate_with_resolver(&resolver, "probe", &json!({"a":"x"})).is_ok());
        assert!(
            validate_with_resolver(&resolver, "probe", &json!({"a":"x","b":"y","c":"z"})).is_err()
        );
    }

    #[test]
    fn not_keyword_rejects_matching_values_and_preserves_bounded_validation() {
        let resolver = StubResolver(json!({
            "type":"object",
            "required":["label"],
            "properties":{
                "label":{
                    "type":"string",
                    "not":{"anyOf":[
                        {"pattern":"^(file|https?)://"},
                        {"const":"forbidden"}
                    ]}
                }
            },
            "additionalProperties":false
        }));
        assert!(validate_with_resolver(&resolver, "probe", &json!({"label":"safe"})).is_ok());
        assert!(validate_with_resolver(&resolver, "probe", &json!({"label":"forbidden"})).is_err());
    }

    #[test]
    fn missing_or_open_schema_fails_closed_without_runtime_fallback() {
        struct Missing;
        impl ActiveRequestSchemaResolver for Missing {
            fn resolve(&self, _operation: &str) -> Result<Option<Value>, String> {
                Ok(None)
            }
        }
        assert!(validate_with_resolver(&Missing, "probe", &json!({}))
            .unwrap_err()
            .contains("WEAPONRY_ACTIVE_SCHEMA_UNAVAILABLE"));

        let open = StubResolver(json!({"type":"object","additionalProperties":true}));
        assert!(validate_with_resolver(&open, "probe", &json!({}))
            .unwrap_err()
            .contains("WEAPONRY_ACTIVE_SCHEMA_NOT_CLOSED"));
    }

    #[test]
    fn native_ref_schema_is_bounded_and_rejects_extra_root_fields() {
        let resolver = default_resolver();
        let schema = resolver
            .resolve("knife_curve_modifier_graph_get")
            .expect("schema resolves")
            .expect("native contract exists");
        assert!(root_is_closed(&schema));
        let mut request = Map::new();
        request.insert(
            "schema_version".to_owned(),
            json!("KnifeCurveModifierGraphGetRequest@1"),
        );
        request.insert(
            "operation".to_owned(),
            json!("knife_curve_modifier_graph_get"),
        );
        request.insert("project_id".to_owned(), json!("project-1"));
        request.insert("source_candidate_id".to_owned(), json!("candidate-1"));
        request.insert(
            "source_candidate_state_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert("source_authoring_mesh_id".to_owned(), json!("mesh-1"));
        request.insert(
            "source_authoring_mesh_lineage_id".to_owned(),
            json!("lineage-1"),
        );
        request.insert(
            "source_authoring_mesh_revision_id".to_owned(),
            json!("revision-1"),
        );
        request.insert("source_authoring_mesh_revision_index".to_owned(), json!(0));
        request.insert(
            "source_authoring_mesh_revision_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "source_authoring_mesh_identity_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "curve_set_semantic_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "sample_set_semantic_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "modifier_graph_semantic_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "dependency_graph_semantic_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert(
            "recompute_plan_semantic_sha256".to_owned(),
            json!("a".repeat(64)),
        );
        request.insert("lookup_key_sha256".to_owned(), json!("a".repeat(64)));
        request.insert("idempotency_key".to_owned(), json!("request-1"));
        request.insert("max_response_bytes".to_owned(), json!(1048576));
        request.insert("runtime_write_performed".to_owned(), json!(false));
        request.insert(
            "writer_policy".to_owned(),
            json!("forgecad-runtime-only-state-writer@1"),
        );
        request.insert(
            "canonicalization_policy".to_owned(),
            json!("canonical-json-sha256-excluding-input-sha256@1"),
        );
        request.insert("input_sha256".to_owned(), json!("a".repeat(64)));
        assert!(validate(
            "knife_curve_modifier_graph_get",
            &Value::Object(request.clone())
        )
        .is_ok());
        request.insert("script".to_owned(), json!("forbidden"));
        assert!(validate("knife_curve_modifier_graph_get", &Value::Object(request)).is_err());
    }

    #[test]
    fn game_asset_delivery_prepare_accepts_the_closed_knife_projection_branch() {
        let request = json!({
            "schema_version": "WeaponryKnifeDeliveryPrepareRequest@1",
            "project_id": "dragonfang-project",
            "source_selector": {
                "kind": "v2-high",
                "id": "dragonfang-high-artifact"
            }
        });
        validate("game_asset_delivery_prepare", &request)
            .expect("Knife delivery projection uses the existing Delivery operation");

        let mut invalid = request;
        invalid
            .as_object_mut()
            .expect("projection request object")
            .insert("script".to_owned(), json!("forbidden"));
        assert!(validate("game_asset_delivery_prepare", &invalid)
            .expect_err("closed Delivery projection must reject unknown fields")
            .starts_with("WEAPONRY_ACTIVE_REQUEST_INVALID: game_asset_delivery_prepare"));
    }

    #[test]
    fn reference_compare_result_is_embedded_closed_and_fail_closed() {
        let filename = result_schema_file_for_operation("reference_compare_prepare")
            .expect("reference comparison result has a closed contract");
        let source = embedded_document(filename).expect("result schema is embedded");
        let schema: Value = serde_json::from_str(source).expect("result schema is valid JSON");
        let schema = materialize_schema(schema, "reference_compare_prepare")
            .expect("nested result contracts materialize");
        assert!(root_is_closed(&schema));
        assert_eq!(schema["properties"]["render_set"]["type"], "object");
        assert_eq!(schema["properties"]["comparison_report"]["type"], "object");
        assert_eq!(schema["properties"]["quality_report"]["type"], "object");
        validate_schema_shape("reference_compare_prepare", &schema)
            .expect("materialized result schema remains bounded");

        let error = validate_closed_result("reference_compare_prepare", &json!({}))
            .expect_err("an incomplete Runtime result must be rejected");
        assert!(error.starts_with("WEAPONRY_ACTIVE_RESULT_INVALID: reference_compare_prepare"));
        assert!(validate_closed_result("runtime_status", &json!({})).is_ok());
    }

    #[test]
    fn knife_pass_state_results_are_embedded_closed_and_fail_closed() {
        for operation in ["knife_pass_state_prepare", "knife_pass_state_get"] {
            let filename = result_schema_file_for_operation(operation)
                .expect("PassState result has a closed contract");
            let source = embedded_document(filename).expect("PassState result is embedded");
            let schema: Value = serde_json::from_str(source).expect("PassState result JSON");
            let schema = materialize_schema(schema, operation)
                .expect("PassState result nested Main schema materializes");
            assert!(root_is_closed(&schema));
            validate_schema_shape(operation, &schema)
                .expect("PassState result schema remains bounded");
            let error = validate_closed_result(operation, &json!({}))
                .expect_err("incomplete PassState result must be rejected");
            assert!(error.starts_with(&format!("WEAPONRY_ACTIVE_RESULT_INVALID: {operation}")));
        }
    }

    #[test]
    fn knife_pass_state_positive_main_passes_the_active_prepare_schema() {
        let pass_state: Value = serde_json::from_str(include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/fixtures/knife-pass-state/positive/",
            "dragonfang-pass-state.json"
        )))
        .expect("checked-in PassState fixture is valid JSON");
        let project_id = pass_state["project_id"]
            .as_str()
            .expect("fixture project_id");
        let request = json!({
            "schema_version":"KnifePassStatePrepareRequest@1",
            "operation":"knife_pass_state_prepare",
            "project_id":project_id,
            "pass_state":pass_state,
            "idempotency_key":"knife-pass-state-active-schema-fixture",
            "max_response_bytes":1048576,
            "runtime_write_performed":false,
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "canonicalization_policy":"canonical-json-sha256-excluding-input-sha256@1",
            "input_sha256":"0000000000000000000000000000000000000000000000000000000000000000"
        });
        validate("knife_pass_state_prepare", &request)
            .expect("the published positive PassState must pass the active MCP request schema");
    }

    #[test]
    fn authoring_mesh_v2_high_bridge_requests_and_result_are_embedded_closed() {
        let resolver = default_resolver();
        for operation in [
            "authoring_mesh_v2_high_bridge_prepare",
            "authoring_mesh_v2_high_bridge_get",
        ] {
            let schema = resolver
                .resolve(operation)
                .expect("High bridge request schema resolves")
                .expect("High bridge request schema is embedded");
            assert!(root_is_closed(&schema));
            validate_schema_shape(operation, &schema)
                .expect("High bridge request schema remains bounded");
            assert!(is_closed(operation).expect("High bridge closure check"));
        }

        let result: Value = serde_json::from_str(include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-bridge/positive/",
            "dragonfang-high-bridge-result-prepared.json"
        )))
        .expect("checked-in High bridge result fixture is valid JSON");
        validate_closed_result("authoring_mesh_v2_high_bridge_prepare", &result)
            .expect("the published structural High bridge result must pass");
        let mut invalid = result;
        invalid
            .as_object_mut()
            .expect("High bridge result object")
            .insert("unexpected".to_owned(), json!(true));
        assert!(
            validate_closed_result("authoring_mesh_v2_high_bridge_prepare", &invalid)
                .expect_err("High bridge result must reject unknown fields")
                .starts_with(
                    "WEAPONRY_ACTIVE_RESULT_INVALID: authoring_mesh_v2_high_bridge_prepare"
                )
        );
    }

    #[test]
    fn authoring_mesh_v2_high_artifact_requests_and_result_are_embedded_closed() {
        let resolver = default_resolver();
        for operation in [
            "authoring_mesh_v2_high_artifact_prepare",
            "authoring_mesh_v2_high_artifact_get",
        ] {
            let schema = resolver
                .resolve(operation)
                .expect("High artifact request schema resolves")
                .expect("High artifact request schema is embedded");
            assert!(root_is_closed(&schema));
            validate_schema_shape(operation, &schema)
                .expect("High artifact request schema remains bounded");
            assert!(is_closed(operation).expect("High artifact request closure check"));
        }

        let prepare: Value = serde_json::from_str(include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-artifact/positive/",
            "dragonfang-high-artifact-prepare-request.json"
        )))
        .expect("checked-in High artifact prepare fixture is valid JSON");
        validate("authoring_mesh_v2_high_artifact_prepare", &prepare)
            .expect("the published High artifact prepare request must pass");

        let get: Value = serde_json::from_str(include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-artifact/positive/",
            "dragonfang-high-artifact-get-request.json"
        )))
        .expect("checked-in High artifact get fixture is valid JSON");
        validate("authoring_mesh_v2_high_artifact_get", &get)
            .expect("the published High artifact get request must pass");

        let result: Value = serde_json::from_str(include_str!(concat!(
            "../../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-artifact/positive/",
            "dragonfang-high-artifact-result-prepared.json"
        )))
        .expect("checked-in High artifact result fixture is valid JSON");
        validate_closed_result("authoring_mesh_v2_high_artifact_prepare", &result)
            .expect("the published High artifact result must pass");
        let mut invalid = result;
        invalid
            .as_object_mut()
            .expect("High artifact result object")
            .insert("unexpected".to_owned(), json!(true));
        assert!(
            validate_closed_result("authoring_mesh_v2_high_artifact_prepare", &invalid)
                .expect_err("High artifact result must reject unknown fields")
                .starts_with(
                    "WEAPONRY_ACTIVE_RESULT_INVALID: authoring_mesh_v2_high_artifact_prepare"
                )
        );
    }

    #[test]
    fn high_artifact_reference_comparison_result_is_fully_closed() {
        let source =
            embedded_document("high-artifact-reference-comparison-prepare-result.schema.json")
                .expect("High artifact comparison result is embedded");
        let schema: Value =
            serde_json::from_str(source).expect("High artifact comparison result is valid JSON");
        let schema = materialize_schema(schema, "high_artifact_reference_compare_prepare")
            .expect("High artifact comparison nested contracts materialize");
        assert!(root_is_closed(&schema));
        let camera_alternatives = schema["properties"]["camera"]["oneOf"]
            .as_array()
            .expect("High artifact camera has closed calibration alternatives");
        assert_eq!(camera_alternatives.len(), 2);
        assert!(camera_alternatives
            .iter()
            .all(|alternative| root_is_closed(alternative)));
        for field in ["render_set", "comparison_report"] {
            assert_eq!(
                schema["properties"][field]["additionalProperties"], false,
                "{field} must remain a closed result object"
            );
        }
        validate_schema_shape("high_artifact_reference_compare_prepare", &schema)
            .expect("High artifact comparison result schema remains bounded");
        let error = validate_closed_result("high_artifact_reference_compare_prepare", &json!({}))
            .expect_err("incomplete High artifact comparison result must fail closed");
        assert!(error.starts_with(
            "WEAPONRY_ACTIVE_RESULT_INVALID: high_artifact_reference_compare_prepare"
        ));
    }
}
