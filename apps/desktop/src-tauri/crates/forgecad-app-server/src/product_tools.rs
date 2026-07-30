//! Code-owned Product Tool registry and restricted executor boundary.
//!
//! K002/U002 can invoke only these seventeen code-owned understanding,
//! planning, visual-source
//! and candidate tools. Permanent Product writes remain outside the registry
//! and require an explicit approval path owned by product core.

mod native_executor;

pub use native_executor::*;

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

use forgecad_app_server_protocol::{
    ProductToolApprovalPolicy, ProductToolExecutionRequest, ProductToolExecutionResult,
    ProductToolExecutionStatus, ValidatedProductToolPayload,
    PRODUCT_TOOL_EXECUTION_REQUEST_SCHEMA_VERSION, PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    canonical::{canonical_json, sha256_hex},
    CancellationToken, ProviderToolCall, ProviderToolDefinition,
};

pub const MAX_PRODUCT_TOOL_CALLS: u32 = 20;
/// Exact size of the reviewed ForgeCAD v1 Product Tool manifest.
///
/// Provider adapters use this bound so adding a code-owned tool cannot leave
/// the transport on a stale, smaller limit that rejects the native registry
/// before any network request is made.
pub const PRODUCT_TOOL_DEFINITION_COUNT: usize = 17;
const K002_FIXTURE_TOOL_DEFINITION_COUNT: usize = 16;

/// Provider input is deliberately narrower for a confirmed robotic-arm
/// continuation than for an initial synthesis.  This is a presentation/input
/// mode only: every accepted call is normalized and then validated against the
/// immutable Product Tool registry before it can reach the native executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderToolInputMode {
    InitialSynthesis,
    ArmContinuationDelta,
}

pub type ProductToolPortFuture = Pin<
    Box<
        dyn Future<Output = Result<ProductToolExecutionResult, ProductToolPortError>>
            + Send
            + 'static,
    >,
>;
pub type ProductToolCancelFuture =
    Pin<Box<dyn Future<Output = Result<bool, ProductToolPortError>> + Send + 'static>>;

/// Trusted generation-origin fact supplied by the native lifecycle, never by
/// a Provider tool argument. `DeepSeekNetworkAttempted` is bound only after a
/// successful DeepSeek preflight and can reach a Product Tool result only
/// after the Action Loop has received a Provider ToolCall. Offline fixtures
/// use the explicit deterministic origin instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationSourceKind {
    OfflineDeterministic,
    DeepseekNetworkAttempted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GenerationSourceBinding {
    pub provider_id: String,
    pub source_kind: GenerationSourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolPortErrorKind {
    Unavailable,
    InvalidResponse,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductToolPortError {
    pub code: String,
    pub kind: ProductToolPortErrorKind,
    pub message: String,
    pub recoverable: bool,
}

impl ProductToolPortError {
    pub fn cancelled() -> Self {
        Self {
            code: "PRODUCT_TOOL_CANCELLED".into(),
            kind: ProductToolPortErrorKind::Cancelled,
            message: "Product Tool execution was cancelled.".into(),
            recoverable: true,
        }
    }

    pub fn timeout() -> Self {
        Self {
            code: "PRODUCT_TOOL_TIMEOUT".into(),
            kind: ProductToolPortErrorKind::Timeout,
            message: "Product Tool execution exceeded its time limit.".into(),
            recoverable: true,
        }
    }

    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self {
            code: "PRODUCT_TOOL_RESPONSE_INVALID".into(),
            kind: ProductToolPortErrorKind::InvalidResponse,
            message: message.into(),
            recoverable: false,
        }
    }
}

/// Transitional executor boundary. Its wire request/result are the protocol
/// crate's sole DTOs; no duplicate adapter contract is created here.
pub trait ProductToolExecutorPort: Send + Sync + 'static {
    /// Reads the Rust-owned ActiveDesignSnapshot for the Project bound to a
    /// Turn.  This is deliberately a read-only capability: the Provider may
    /// see the current asset context, but it cannot supply a Project or
    /// Snapshot identity and it cannot write product state through this
    /// method.  Compatibility executors return `None` until K003 is present.
    fn read_active_design_snapshot(
        &self,
        _project_id: &str,
    ) -> Result<Option<Value>, ProductToolPortError> {
        Ok(None)
    }

    /// Reads one immutable ReferenceEvidence metadata record from the
    /// Rust-owned product store. The source object bytes and CAS location are
    /// deliberately not exposed across this port; native Turn startup only
    /// needs the sealed identity and deterministic observations in order to
    /// revalidate a client-supplied VisualEvidenceGraph before any mutation.
    fn read_reference_evidence(
        &self,
        _project_id: &str,
        _evidence_id: &str,
    ) -> Result<Option<Value>, ProductToolPortError> {
        Ok(None)
    }

    /// Binds an execution to the Project already owned by the native Thread
    /// lifecycle.  This is intentionally not part of
    /// `ProductToolExecutionRequest`: a model must never be able to supply or
    /// rebind product identity through tool arguments.
    fn bind_execution_project(
        &self,
        _execution_id: &str,
        _turn_id: &str,
        _project_id: Option<&str>,
    ) -> Result<(), ProductToolPortError> {
        Ok(())
    }

    /// Binds one already validated visual-evidence context to the execution.
    /// The Provider cannot supply this capability through tool arguments;
    /// author/patch may only provide per-claim dispositions, while Rust owns
    /// request/graph/evidence lineage and constructs the final binding.
    fn bind_execution_multimodal_context(
        &self,
        _execution_id: &str,
        _turn_id: &str,
        _context: crate::ValidatedMultimodalActionContext,
    ) -> Result<(), ProductToolPortError> {
        Ok(())
    }

    /// Binds the category-open request and its exact sealed evidence before
    /// the Provider can call author_universal_asset.
    fn bind_execution_universal_author_context(
        &self,
        _execution_id: &str,
        _turn_id: &str,
        _context: crate::ValidatedUniversalAuthorContext,
    ) -> Result<(), ProductToolPortError> {
        Ok(())
    }

    /// Binds the trusted execution-origin fact before the first Tool result.
    /// It is deliberately separate from the wire request so a model cannot
    /// claim an offline or Provider source for a generated asset.
    fn bind_execution_generation_source(
        &self,
        _execution_id: &str,
        _turn_id: &str,
        _source: GenerationSourceBinding,
    ) -> Result<(), ProductToolPortError> {
        Ok(())
    }

    fn execute(
        &self,
        request: ProductToolExecutionRequest,
        cancellation: CancellationToken,
    ) -> ProductToolPortFuture;

    fn cancel(
        &self,
        _cancellation_id: String,
        _cancellation_token: String,
    ) -> ProductToolCancelFuture {
        Box::pin(async { Ok(false) })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductToolDefinition {
    pub tool_id: String,
    pub name: String,
    pub description: String,
    pub approval_policy: ProductToolApprovalPolicy,
    pub input_schema: Value,
    pub input_schema_sha256: String,
    pub output_schema: Value,
    pub output_schema_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductToolRegistryFixture {
    schema_version: String,
    fixture_id: String,
    registry_schema_version: String,
    canonicalization: Value,
    tools: Vec<ProductToolFixtureEntry>,
    manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductToolFixtureEntry {
    tool_id: String,
    name: String,
    description: String,
    input_schema: Value,
    output_schema: Value,
    approval_policy: ProductToolApprovalPolicy,
    input_schema_sha256: String,
    output_schema_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolRegistryErrorKind {
    UnknownTool,
    DuplicateTool,
    ApprovalForbidden,
    InvalidSchema,
    InvalidArguments,
    InvalidResult,
    InvalidIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductToolRegistryError {
    pub code: String,
    pub kind: ProductToolRegistryErrorKind,
    pub message: String,
}

impl ProductToolRegistryError {
    fn new(code: &str, kind: ProductToolRegistryErrorKind, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProductToolRegistry {
    definitions: BTreeMap<String, ProductToolDefinition>,
    order: Vec<String>,
}

impl Default for ProductToolRegistry {
    fn default() -> Self {
        Self::forgecad_v1().expect("code-owned Product Tool registry must validate")
    }
}

impl ProductToolRegistry {
    pub fn forgecad_v1() -> Result<Self, ProductToolRegistryError> {
        let fixture: ProductToolRegistryFixture = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/k002-product-tool-registry.json"
        )))
        .map_err(|error| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_FIXTURE_INVALID",
                ProductToolRegistryErrorKind::InvalidSchema,
                format!("Shared Product Tool fixture is invalid: {error}"),
            )
        })?;
        validate_fixture_header_and_manifest(&fixture)?;

        let mut definitions = BTreeMap::new();
        let mut order = Vec::with_capacity(fixture.tools.len());
        for entry in fixture.tools {
            if schema_digest(&entry.input_schema) != entry.input_schema_sha256
                || schema_digest(&entry.output_schema) != entry.output_schema_sha256
            {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_FIXTURE_SCHEMA_HASH_MISMATCH",
                    ProductToolRegistryErrorKind::InvalidSchema,
                    "Shared Product Tool fixture contains a schema hash mismatch.",
                ));
            }
            let definition = ProductToolDefinition {
                tool_id: entry.tool_id,
                name: entry.name,
                description: entry.description,
                approval_policy: entry.approval_policy,
                input_schema: entry.input_schema,
                input_schema_sha256: entry.input_schema_sha256,
                output_schema: entry.output_schema,
                output_schema_sha256: entry.output_schema_sha256,
            };
            order.push(definition.name.clone());
            if definitions
                .insert(definition.name.clone(), definition)
                .is_some()
            {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_DUPLICATE",
                    ProductToolRegistryErrorKind::DuplicateTool,
                    "Product Tool names must be unique.",
                ));
            }
        }
        let universal = universal_author_tool_definition();
        order.insert(0, universal.name.clone());
        if definitions
            .insert(universal.name.clone(), universal)
            .is_some()
        {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_DUPLICATE",
                ProductToolRegistryErrorKind::DuplicateTool,
                "Universal author tool name is already occupied.",
            ));
        }
        let registry = Self { definitions, order };
        registry.validate_registry()?;
        Ok(registry)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ProductToolDefinition> {
        self.order
            .iter()
            .filter_map(|name| self.definitions.get(name))
    }

    pub fn provider_definitions(&self) -> Vec<ProviderToolDefinition> {
        self.provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
    }

    pub fn provider_definitions_for_mode(
        &self,
        input_mode: ProviderToolInputMode,
    ) -> Vec<ProviderToolDefinition> {
        self.definitions()
            .map(|definition| ProviderToolDefinition {
                name: definition.name.clone(),
                description: match definition.name.as_str() {
                    "author_universal_asset" => "Understand the actual requested subject without a category allowlist. Return exactly one UniversalAuthorOutcome@1 containing the Rust-sealed request, SubjectProfile@1, VisualFeatureContract@1 and RepresentationPlan@1. Use executable only for a code-owned available capability; otherwise return a typed limitation with no geometry. For procedural.generic_hard_surface_v1, executable_payload must be one ForgeVisualGeometryProgram@2 with domain generic_hard_surface, one output per declared subject part, and no mechanical-arm/C111 fallback.".into(),
                    "author_forge_visual_program" => "Author one compact ForgeVisualAuthoringIntent@1. Choose the robotic-arm visual architecture, silhouette language, materials, surface motifs, detail density and pose; Rust derives every ShapeProgram operation, Part, Material Zone, Surface Program and Detail binding.".into(),
                    "patch_forge_visual_program" => "Patch only the current ForgeVisualProgram revision. A replace_geometry_graph operation must carry a complete ShapeProgram@1, including schema_version, program_id, units, seed, triangle_budget, parameters, operations, outputs, and non_functional_only. Reuse earlier operation IDs for non-primitive inputs; radial_array is never inputs=[].".into(),
                    _ => definition.description.clone(),
                },
                // The registry schema remains the authoritative Rust-side
                // validation contract.  The Provider-facing schema is a
                // deliberately smaller projection so a model does not have
                // to reproduce internal `$defs` for assembly deltas and
                // legacy compatibility fields while planning a new asset.
                // `build_execution_request` still validates the exact full
                // registry schema before any Product Tool runs.
                input_schema: match (definition.name.as_str(), input_mode) {
                    ("author_forge_visual_program", _) => {
                        compact_forge_visual_program_author_schema()
                    }
                    ("patch_forge_visual_program", _) => compact_forge_visual_patch_schema(),
                    ("plan_complete_concept", ProviderToolInputMode::InitialSynthesis) => {
                        compact_plan_provider_schema()
                    }
                    ("plan_complete_concept", ProviderToolInputMode::ArmContinuationDelta) => {
                        compact_arm_continuation_provider_schema()
                    }
                    _ => definition.input_schema.clone(),
                },
            })
            .collect()
    }

    /// The post-comparison repair turn is intentionally narrower than an
    /// ordinary visual edit. Rust has already selected exact current rows in
    /// `visual_repair_target_projection`; replacing a graph, title, or token
    /// would discard the failed candidate's same-intent lineage.
    pub fn visual_repair_provider_definition(&self) -> ProviderToolDefinition {
        let mut definition = self
            .provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
            .into_iter()
            .find(|definition| definition.name == "patch_forge_visual_program")
            .expect("code-owned registry must expose the visual patch tool");
        definition.input_schema = compact_forge_visual_repair_patch_schema();
        definition.description = "Repair one Rust-projected current row only. Use exactly one or more typed local upsert operations: upsert_geometry_operation, upsert_material_binding, upsert_surface_binding, or upsert_detail_inventory_item. Do not replace geometry/material/surface/detail graphs, set title/tokens/export profile, inspect, author, or build. Reuse only IDs supplied by visual_repair_target_projection.".into();
        definition
    }

    /// The UAS@2 route deliberately shares the candidate-only Product Tool
    /// identity with the legacy patch tool, but exposes only VP204's bounded
    /// geometry patch language after a failed generic PBR comparison.
    pub fn universal_hard_surface_repair_provider_definition(&self) -> ProviderToolDefinition {
        let mut definition = self
            .provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
            .into_iter()
            .find(|definition| definition.name == "patch_forge_visual_program")
            .expect("code-owned registry must expose the visual patch tool");
        definition.input_schema = compact_universal_hard_surface_repair_patch_schema();
        definition.description = "Repair the current UAS@2 generic hard-surface source with exactly one bounded ForgeVisualGeometryPatch@1. Use only the Rust-projected source hash and stable node/material IDs. Do not author a new object, replace a graph, call an arm tool, or add code, paths, URLs, dimensions, or unknown fields.".into();
        definition
    }

    /// The ordinary edit projection is intentionally typed and incremental.
    /// A Provider may update projected rows, but it must not resend a complete
    /// geometry/material/surface graph. The full ForgeVisualPatch contract is
    /// still enforced by the Rust execution boundary.
    pub fn visual_incremental_edit_provider_definition(&self) -> ProviderToolDefinition {
        let mut definition = self
            .provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
            .into_iter()
            .find(|definition| definition.name == "patch_forge_visual_program")
            .expect("code-owned registry must expose the visual patch tool");
        let local_operations = definition
            .input_schema
            .pointer("/properties/patch/properties/operations/items/anyOf")
            .and_then(Value::as_array)
            .expect("compact visual patch schema must contain operation branches")
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>();
        definition.input_schema["properties"]["patch"]["properties"]["operations"]["items"] =
            json!({"anyOf":local_operations});
        definition.description = "Apply a bounded typed incremental edit to the inspected current ForgeVisualProgram. Use only local upsert operations and the exact revision/hash from Rust; never resend a complete graph or call a legacy planner.".into();
        definition
    }

    /// An ordinary edit only needs the Rust-owned revision/hash and row
    /// summaries. Keeping `full` out of this projection prevents the next
    /// Provider request from replaying a hundreds-of-lines source dump.
    pub fn visual_incremental_edit_inspect_provider_definition(&self) -> ProviderToolDefinition {
        let mut definition = self
            .provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
            .into_iter()
            .find(|definition| definition.name == "inspect_forge_visual_program")
            .expect("code-owned registry must expose the visual inspect tool");
        definition.input_schema = json!({
            "type":"object",
            "additionalProperties":false,
            "required":["view"],
            "properties":{"view":{"type":"string", "const":"summary"}}
        });
        definition.description = "Inspect only the compact Rust-owned summary of the current ForgeVisualProgram. Do not request the full source; the next action is one typed incremental patch using the returned revision/hash.".into();
        definition
    }

    pub fn definition(
        &self,
        name: &str,
    ) -> Result<&ProductToolDefinition, ProductToolRegistryError> {
        self.definitions.get(name).ok_or_else(|| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_UNKNOWN",
                ProductToolRegistryErrorKind::UnknownTool,
                "Provider requested a tool outside the code-owned registry.",
            )
        })
    }

    pub fn build_execution_request(
        &self,
        turn_id: &str,
        call: &ProviderToolCall,
        execution_id: &str,
        cancellation_id: &str,
        cancellation_token: &str,
    ) -> Result<ProductToolExecutionRequest, ProductToolRegistryError> {
        self.build_execution_request_for_mode(
            turn_id,
            call,
            execution_id,
            cancellation_id,
            cancellation_token,
            ProviderToolInputMode::InitialSynthesis,
        )
    }

    pub fn build_execution_request_for_mode(
        &self,
        turn_id: &str,
        call: &ProviderToolCall,
        execution_id: &str,
        cancellation_id: &str,
        cancellation_token: &str,
        input_mode: ProviderToolInputMode,
    ) -> Result<ProductToolExecutionRequest, ProductToolRegistryError> {
        let definition = self.definition(&call.name)?;
        if definition.approval_policy == ProductToolApprovalPolicy::UserConfirmationRequired {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_APPROVAL_PATH_FORBIDDEN",
                ProductToolRegistryErrorKind::ApprovalForbidden,
                "Permanent-write tools cannot run inside the K002 Action Loop.",
            ));
        }
        let supplied_arguments = value_to_btree_object(&call.arguments).ok_or_else(|| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_ARGUMENTS_NOT_OBJECT",
                ProductToolRegistryErrorKind::InvalidArguments,
                "Product Tool arguments must be a JSON object.",
            )
        })?;
        let mut arguments = if definition.name == "plan_complete_concept"
            && input_mode == ProviderToolInputMode::ArmContinuationDelta
        {
            normalize_arm_continuation_arguments(supplied_arguments)?
        } else {
            supplied_arguments
        };
        if definition.name == "author_forge_visual_program"
            && arguments.contains_key("authoring_intent")
        {
            let provider_definition = self
                .provider_definitions_for_mode(input_mode)
                .into_iter()
                .find(|candidate| candidate.name == definition.name)
                .expect("every registry tool must have a Provider projection");
            let provider_arguments =
                Value::Object(arguments.clone().into_iter().collect::<Map<_, _>>());
            validate_json_schema(&provider_definition.input_schema, &provider_arguments).map_err(
                |message| {
                    ProductToolRegistryError::new(
                        "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID",
                        ProductToolRegistryErrorKind::InvalidArguments,
                        message,
                    )
                },
            )?;
            let authoring_intent = arguments
                .remove("authoring_intent")
                .expect("guarded key exists");
            let program = forgecad_core::lower_forge_visual_authoring_intent(&authoring_intent)
                .map_err(|error| {
                    ProductToolRegistryError::new(
                        error.code(),
                        ProductToolRegistryErrorKind::InvalidArguments,
                        error.to_string(),
                    )
                })?;
            arguments.insert(
                "program".into(),
                serde_json::to_value(program).map_err(|_| {
                    ProductToolRegistryError::new(
                        "FORGE_VISUAL_AUTHORING_INTENT_LOWERING_FAILED",
                        ProductToolRegistryErrorKind::InvalidArguments,
                        "Rust could not serialize the lowered ForgeVisualProgram.",
                    )
                })?,
            );
            if let Some(dispositions) = arguments
                .get_mut("evidence_dispositions")
                .and_then(Value::as_array_mut)
            {
                for disposition in dispositions {
                    if let Some(row) = disposition.as_object_mut() {
                        row.insert("detail_ids".into(), Value::Array(Vec::new()));
                    }
                }
            }
        }
        // Inspection is read-only and `summary` is the least-privileged view.
        // Vision models occasionally add display-only flags after a long
        // convergence turn. The inspection contract intentionally exposes
        // only `view`, so discard those non-semantic extras before schema
        // validation and default the one allowed enum when omitted.
        if definition.name == "inspect_forge_visual_program" {
            arguments.retain(|key, _| key == "view");
            arguments
                .entry("view".into())
                .or_insert_with(|| Value::String("summary".into()));
        }
        let arguments_value = Value::Object(arguments.clone().into_iter().collect::<Map<_, _>>());
        // The visual authoring tools advertise a deliberately strict schema
        // projection to the Provider. Their persisted registry envelope is
        // intentionally shallow for compatibility, so validating only that
        // envelope would let an operation outside the advertised ShapeProgram
        // whitelist reach the native executor. Enforce the same projection at
        // the request boundary before sealing the full registry identity.
        if definition.name == "patch_forge_visual_program" {
            // Validate each self-contained schema separately. Wrapping the
            // two contracts in an outer `anyOf` would rebase the VP204
            // schema's root-local `#/$defs` references and reject valid
            // source-bound geometry patches before native execution.
            let forge_visual_error =
                validate_json_schema(&compact_forge_visual_patch_schema(), &arguments_value).err();
            let universal_error = validate_json_schema(
                &compact_universal_hard_surface_repair_patch_schema(),
                &arguments_value,
            )
            .err();
            if forge_visual_error.is_some() && universal_error.is_some() {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID",
                    ProductToolRegistryErrorKind::InvalidArguments,
                    universal_error
                        .or(forge_visual_error)
                        .unwrap_or_else(|| "patch does not match any reviewed contract".into()),
                ));
            }
        }
        validate_json_schema(&definition.input_schema, &arguments_value).map_err(|message| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID",
                ProductToolRegistryErrorKind::InvalidArguments,
                message,
            )
        })?;

        let idempotency_value = json!({
            "turn_id": turn_id,
            "call_id": call.call_id,
            "tool_id": definition.tool_id,
            "arguments": arguments,
        });
        let request = ProductToolExecutionRequest {
            schema_version: PRODUCT_TOOL_EXECUTION_REQUEST_SCHEMA_VERSION.into(),
            execution_id: execution_id.into(),
            turn_id: turn_id.into(),
            call_id: call.call_id.clone(),
            tool_id: definition.tool_id.clone(),
            tool_name: definition.name.clone(),
            registry_schema_version: PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION.into(),
            idempotency_key: sha256_hex(canonical_json(&idempotency_value).as_bytes()),
            validated_arguments: ValidatedProductToolPayload {
                schema_id: format!("{}:input", definition.tool_id),
                schema_sha256: definition.input_schema_sha256.clone(),
                value: arguments,
            },
            approval_policy: definition.approval_policy,
            cancellation_id: cancellation_id.into(),
            cancellation_token: cancellation_token.into(),
        };
        request.validate().map_err(|error| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_REQUEST_INVALID",
                ProductToolRegistryErrorKind::InvalidIdentity,
                error.message,
            )
        })?;
        Ok(request)
    }

    pub fn validate_result(
        &self,
        request: &ProductToolExecutionRequest,
        result: &ProductToolExecutionResult,
    ) -> Result<(), ProductToolRegistryError> {
        result.validate().map_err(|error| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_RESULT_INVALID",
                ProductToolRegistryErrorKind::InvalidResult,
                error.message,
            )
        })?;
        if result.execution_id != request.execution_id
            || result.turn_id != request.turn_id
            || result.call_id != request.call_id
            || result.tool_id != request.tool_id
            || result.cancellation_id != request.cancellation_id
        {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_RESULT_IDENTITY_MISMATCH",
                ProductToolRegistryErrorKind::InvalidIdentity,
                "Product Tool result identity does not match its request.",
            ));
        }
        if result.status == ProductToolExecutionStatus::Completed {
            let definition = self.definition(&request.tool_name)?;
            let output = result.validated_output.as_ref().ok_or_else(|| {
                ProductToolRegistryError::new(
                    "PRODUCT_TOOL_OUTPUT_MISSING",
                    ProductToolRegistryErrorKind::InvalidResult,
                    "Completed Product Tool result omitted validated output.",
                )
            })?;
            if output.schema_sha256 != definition.output_schema_sha256 {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_OUTPUT_SCHEMA_DIGEST_MISMATCH",
                    ProductToolRegistryErrorKind::InvalidResult,
                    "Product Tool output schema digest does not match the code-owned registry.",
                ));
            }
            let value = Value::Object(output.value.clone().into_iter().collect());
            validate_json_schema(&definition.output_schema, &value).map_err(|message| {
                ProductToolRegistryError::new(
                    "PRODUCT_TOOL_OUTPUT_SCHEMA_INVALID",
                    ProductToolRegistryErrorKind::InvalidResult,
                    message,
                )
            })?;
        }
        Ok(())
    }

    fn validate_registry(&self) -> Result<(), ProductToolRegistryError> {
        if self.definitions.len() != PRODUCT_TOOL_DEFINITION_COUNT {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_REGISTRY_INCOMPLETE",
                ProductToolRegistryErrorKind::InvalidSchema,
                "ForgeCAD Product Tool registry does not match the code-owned tool count.",
            ));
        }
        let mut ids = BTreeSet::new();
        for definition in self.definitions() {
            if definition.approval_policy == ProductToolApprovalPolicy::UserConfirmationRequired {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_APPROVAL_PATH_FORBIDDEN",
                    ProductToolRegistryErrorKind::ApprovalForbidden,
                    "The K002 registry cannot include permanent-write tools.",
                ));
            }
            if !ids.insert(definition.tool_id.as_str()) {
                return Err(ProductToolRegistryError::new(
                    "PRODUCT_TOOL_ID_DUPLICATE",
                    ProductToolRegistryErrorKind::DuplicateTool,
                    "Product Tool IDs must be unique.",
                ));
            }
            validate_schema_definition(&definition.input_schema).map_err(|message| {
                ProductToolRegistryError::new(
                    "PRODUCT_TOOL_INPUT_SCHEMA_INVALID",
                    ProductToolRegistryErrorKind::InvalidSchema,
                    message,
                )
            })?;
            validate_schema_definition(&definition.output_schema).map_err(|message| {
                ProductToolRegistryError::new(
                    "PRODUCT_TOOL_OUTPUT_SCHEMA_INVALID",
                    ProductToolRegistryErrorKind::InvalidSchema,
                    message,
                )
            })?;
        }
        Ok(())
    }
}

fn compact_shape_program_provider_schema() -> Value {
    let mut shape_program: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/schemas/shape-program.schema.json"
    )))
    .expect("shipped ShapeProgram provider schema must parse");
    // Product Tool instance validation accepts only self-contained code-owned
    // schemas. The persisted ShapeProgram's optional profile_inputs point to
    // separate contract files and require content hashes a Provider must not
    // invent. Omit only that optional authoring surface while exposing the
    // complete bounded primitive/operation/output contract used by this first
    // multimodal production path.
    let shape_object = shape_program
        .as_object_mut()
        .expect("shipped ShapeProgram schema must be an object");
    shape_object.remove("$schema");
    shape_object.remove("$id");
    shape_object.remove("title");
    shape_object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .expect("shipped ShapeProgram schema must contain properties")
        .remove("profile_inputs");
    shape_object.insert(
        "description".into(),
        Value::String(
            "Complete ShapeProgram@1. box/wedge/cylinder/capsule/profile/loft/sweep use inputs=[]; extrude/revolve require one earlier profile; mirror requires one earlier mesh and non-zero axis; array requires one earlier mesh, count>=2, spacing>0 and non-zero axis; radial_array requires one earlier mesh, count>=2, radius>0, 0<angle<=2*pi and non-zero axis; bevel_approx/surface_panel require one earlier box or bevel_approx; lattice_deform requires one earlier mesh plus exactly eight non-zero bounded 2x2x2 corner offsets; union/subtract require 2-8 earlier non-profile meshes and boolean depth is at most 8.".into(),
        ),
    );
    shape_program
}

fn universal_author_tool_definition() -> ProductToolDefinition {
    let input_schema = json!({
        "type": "object",
        "properties": {
            "outcome": {"type": "object"},
            "legacy_evidence_dispositions": {
                "type": "array",
                "maxItems": 256,
                "items": {
                    "type": "object",
                    "required": ["claim_id", "disposition", "reason"],
                    "properties": {
                        "claim_id": {"type": "string"},
                        "disposition": {"enum": ["bound", "unresolved", "evaluation_only"]},
                        "reason": {"type": "string"}
                    },
                    "additionalProperties": false
                }
            }
        },
        "required": ["outcome"],
        "additionalProperties": false
    });
    let output_schema = json!({
        "type": "object",
        "required": ["schema_version", "outcome"],
        "properties": {
            "schema_version": {"enum": ["UniversalAuthorOutcome@1"]},
            "outcome": {"enum": ["executable", "limitation", "clarification_required"]}
        },
        "additionalProperties": true
    });
    ProductToolDefinition {
        tool_id: "forgecad.universal_asset.author.v1".into(),
        name: "author_universal_asset".into(),
        description: "Understand any subject, bind visual acceptance requirements and select only code-owned representation capabilities. Unsupported execution returns a typed limitation without geometry side effects.".into(),
        approval_policy: ProductToolApprovalPolicy::CandidateOnly,
        input_schema_sha256: schema_digest(&input_schema),
        output_schema_sha256: schema_digest(&output_schema),
        input_schema,
        output_schema,
    }
}

fn compact_forge_visual_program_author_schema() -> Value {
    let mut arm_intent = compact_plan_provider_schema()
        .pointer("/properties/plan/properties/arm_design_intent/anyOf/0")
        .cloned()
        .expect("compact plan schema must expose ArmDesignIntent");
    arm_intent["required"] = json!([
        "schema_version",
        "domain_pack_id",
        "architecture",
        "joint_language",
        "link_language",
        "base_language",
        "wrist_language",
        "end_effector_language",
        "cable_language",
        "surface_language",
        "material_palette",
        "detail_density",
        "pose",
        "proportion_profile",
        "style_keywords",
        "source",
        "visual_only"
    ]);
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["authoring_intent"],
        "properties":{
            "authoring_intent":{
                "type":"object",
                "additionalProperties":false,
                "required":["schema_version","authoring_id","title","arm_design_intent"],
                "properties":{
                    "schema_version":{"type":"string","const":"ForgeVisualAuthoringIntent@1"},
                    "authoring_id":{"type":"string","pattern":"^[A-Za-z0-9_:-]+$","maxLength":96},
                    "title":{"type":"string","minLength":1,"maxLength":160},
                    "arm_design_intent":arm_intent
                }
            },
            "evidence_dispositions":compact_authoring_intent_claim_dispositions_schema()
        }
    })
}

fn compact_authoring_intent_claim_dispositions_schema() -> Value {
    json!({
        "type":"array",
        "minItems":1,
        "maxItems":256,
        "items":{
            "type":"object",
            "additionalProperties":false,
            "required":["claim_id","disposition","reason"],
            "properties":{
                "claim_id":{"type":"string","pattern":"^vclaim_[A-Za-z0-9_.:-]+$","maxLength":160},
                "disposition":{"type":"string","enum":["bound","unresolved","evaluation_only"]},
                "reason":{"type":"string","minLength":1,"maxLength":320}
            }
        }
    })
}

fn compact_forge_visual_patch_schema() -> Value {
    let shape_program = compact_shape_program_provider_schema();
    // Keep the Provider projection deliberately small, but do not turn the
    // new repair-only operations into untyped blobs.  The execution boundary
    // validates the complete shared schema; this projection still needs to
    // tell the Provider the exact stable IDs and row shapes it may address.
    let shape_operation = shape_program
        .pointer("/properties/operations/items")
        .cloned()
        .expect("compact ShapeProgram schema must include operation items");
    let mut schema = json!({
        "type":"object",
        "additionalProperties":false,
        "required":["patch"],
        "properties":{
            "patch":{
                "type":"object",
                "additionalProperties":false,
                "required":[
                    "schema_version", "patch_id", "expected_revision",
                    "expected_source_sha256", "preserve_geometry",
                    "preserve_material_surface", "operations"
                ],
                "properties":{
                    "schema_version":{"type":"string", "const":"ForgeVisualPatch@1"},
                    "patch_id":{"type":"string", "minLength":1, "maxLength":128},
                    "expected_revision":{"type":"integer", "minimum":1},
                    "expected_source_sha256":{"type":"string", "minLength":64, "maxLength":64},
                    "preserve_geometry":{"type":"boolean"},
                    "preserve_material_surface":{"type":"boolean"},
                    "operations":{
                        "type":"array", "minItems":1, "maxItems":32,
                        "items":{
                            "type":"object",
                            "required":["op"],
                            "properties":{
                                "op":{"enum":[
                                    "set_title", "upsert_design_token", "remove_design_token",
                                    "replace_parts", "replace_geometry_graph",
                                    "replace_assembly_graph", "replace_material_graph",
                                    "replace_surface_graph", "replace_detail_inventory",
                                    "set_export_profile"
                                ]},
                                "title":{"type":"string", "minLength":1, "maxLength":160},
                                "token":{"type":"object"},
                                "token_id":{"type":"string", "minLength":1, "maxLength":128},
                                "parts":{"type":"array", "minItems":1, "maxItems":256, "items":{"type":"object"}},
                                "geometry_graph":shape_program,
                                "assembly_graph":{"type":"object"},
                                "material_graph":{"type":"array", "minItems":1, "maxItems":2048, "items":{"type":"object"}},
                                "surface_graph":{"type":"array", "maxItems":2048, "items":{"type":"object"}},
                                "detail_inventory":{"type":"array", "minItems":1, "maxItems":512, "items":{"type":"object"}},
                                "export_profile":{"enum":["interactive_preview", "production_concept"]}
                            }
                        }
                    }
                }
            },
            "evidence_dispositions":compact_visual_claim_dispositions_schema()
        }
    });

    // The existing branch keeps the broader legacy replace operations for
    // initial/non-repair edits.  Each local upsert is a separate
    // `additionalProperties:false` branch so a repair cannot smuggle an
    // arbitrary graph replacement or an unknown field through the compact
    // Provider contract.
    let legacy_operation = schema
        .pointer("/properties/patch/properties/operations/items")
        .cloned()
        .expect("compact ForgeVisualPatch schema must include legacy operations");
    let mut operation_kind_projection = legacy_operation.clone();
    operation_kind_projection["properties"]["op"]["enum"]
        .as_array_mut()
        .expect("legacy patch operation kinds must be an enum")
        .extend([
            json!("upsert_geometry_operation"),
            json!("upsert_material_binding"),
            json!("upsert_surface_binding"),
            json!("upsert_detail_inventory_item"),
        ]);
    schema["properties"]["patch"]["properties"]["operations"]["items"] = json!({
        "required": operation_kind_projection["required"].clone(),
        "properties": operation_kind_projection["properties"].clone(),
        "anyOf":[
            legacy_operation,
            {
                "type":"object",
                "additionalProperties":false,
                "required":["op", "operation_id", "operation"],
                "properties":{
                    "op":{"const":"upsert_geometry_operation"},
                    "operation_id":{"type":"string", "pattern":"^op_[a-z0-9_\\-]+$"},
                    "operation":shape_operation
                }
            },
            {
                "type":"object",
                "additionalProperties":false,
                "required":["op", "binding"],
                "properties":{
                    "op":{"const":"upsert_material_binding"},
                    "binding":{
                        "type":"object",
                        "additionalProperties":false,
                        "required":["part_id", "material_zone_id", "material_id"],
                        "properties":{
                            "part_id":{"type":"string", "pattern":"^part_[A-Za-z0-9_\\-]+$"},
                            "material_zone_id":{"type":"string", "pattern":"^zone_[A-Za-z0-9_\\-]+$"},
                            "material_id":{"type":"string", "pattern":"^mat_[A-Za-z0-9_\\-]+$"}
                        }
                    }
                }
            },
            {
                "type":"object",
                "additionalProperties":false,
                "required":["op", "binding"],
                "properties":{
                    "op":{"const":"upsert_surface_binding"},
                    "binding":{
                        "type":"object",
                        "additionalProperties":false,
                        "required":["surface_program_id", "part_id", "material_zone_id"],
                        "properties":{
                            "surface_program_id":{"type":"string", "pattern":"^surface_[A-Za-z0-9_\\-]+$"},
                            "part_id":{"type":"string", "pattern":"^part_[A-Za-z0-9_\\-]+$"},
                            "material_zone_id":{"type":"string", "pattern":"^zone_[A-Za-z0-9_\\-]+$"}
                        }
                    }
                }
            },
            {
                "type":"object",
                "additionalProperties":false,
                "required":["op", "detail"],
                "properties":{
                    "op":{"const":"upsert_detail_inventory_item"},
                    "detail":{
                        "type":"object",
                        "additionalProperties":false,
                        "required":["detail_id", "level", "description", "critical", "status", "bindings"],
                        "properties":{
                            "detail_id":{"type":"string", "pattern":"^detail_[A-Za-z0-9_\\-]+$"},
                            "level":{"enum":["macro", "meso", "micro"]},
                            "description":{"type":"string", "minLength":1, "maxLength":240},
                            "critical":{"type":"boolean"},
                            "status":{"enum":["bound", "unresolved"]},
                            "bindings":{
                                "type":"array", "maxItems":128,
                                "items":{
                                    "type":"object",
                                    "additionalProperties":false,
                                    "required":["kind", "part_id", "target_id"],
                                    "properties":{
                                        "kind":{"enum":["geometry_output", "material_zone", "surface_program"]},
                                        "part_id":{"type":"string", "pattern":"^part_[A-Za-z0-9_\\-]+$"},
                                        "target_id":{"type":"string", "minLength":1, "maxLength":128}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ]
    });
    schema
}

fn compact_universal_hard_surface_repair_patch_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["patch"],
        "properties":{"patch":{
            "type":"object",
            "additionalProperties":false,
            "required":["schema_version","patch_id","expected_source_sha256","operations"],
            "properties":{
                "schema_version":{"const":"ForgeVisualGeometryPatch@1"},
                "patch_id":{"type":"string","minLength":7,"maxLength":96},
                "expected_source_sha256":{"type":"string","minLength":64,"maxLength":64},
                "operations":{"type":"array","minItems":1,"maxItems":8,"items":{
                    "type":"object","additionalProperties":false,"required":["op"],
                    "properties":{
                        "op":{"enum":["set_node_position","set_extrude_height","set_revolve_angle","set_loft_axis_length","set_sweep_profile_scale","set_array","set_material_base"]},
                        "node_id":{"type":"string","minLength":6,"maxLength":96},
                        "material_id":{"type":"string","minLength":5,"maxLength":96},
                        "base_material_id":{"type":"string","minLength":5,"maxLength":96},
                        "position":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"number","minimum":-100000,"maximum":100000}},
                        "height":{"type":"number","exclusiveMinimum":0,"maximum":100000},
                        "angle":{"type":"number","exclusiveMinimum":0,"maximum":6.283185307179586},
                        "axis_length":{"type":"number","exclusiveMinimum":0,"maximum":100000},
                        "profile_scale":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","exclusiveMinimum":0,"maximum":100000}},
                        "count":{"type":"integer","minimum":2,"maximum":64},
                        "spacing":{"type":"number","exclusiveMinimum":0,"maximum":100000}
                    }
                }}
            }
        }}
    })
}

/// Provider schema for a failed visual-convergence repair. Unlike the normal
/// patch schema, it deliberately excludes every legacy whole-graph and
/// presentation operation. Rust's native executor additionally checks that
/// these typed rows are among the current projection targets.
fn compact_forge_visual_repair_patch_schema() -> Value {
    let mut schema = compact_forge_visual_patch_schema();
    let local_operations = schema
        .pointer("/properties/patch/properties/operations/items/anyOf")
        .and_then(Value::as_array)
        .expect("compact visual patch schema must contain local operation branches")
        .iter()
        // Branch zero is the legacy edit projection. The remaining four are
        // the strict local upsert variants introduced for convergence repair.
        .skip(1)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        local_operations.len(),
        4,
        "visual repair must expose exactly four local upsert variants"
    );
    schema["properties"]["patch"]["properties"]["operations"]["items"] =
        json!({"anyOf":local_operations});
    // Keep the Provider contract identical to the native executor contract.
    // A high-detail material/surface repair regularly needs more than eight
    // projected rows, while 32 remains a deterministic bounded envelope.
    schema["properties"]["patch"]["properties"]["operations"]["maxItems"] = json!(32);
    // A local repair may touch either geometry or material/surface rows. These
    // flags are permissions, not assertions that a domain is changed, so the
    // repair envelope must permit both domains and let the typed operations
    // determine the actual changed-domain set.
    schema["properties"]["patch"]["properties"]["preserve_geometry"] =
        json!({"type":"boolean", "const":false});
    schema["properties"]["patch"]["properties"]["preserve_material_surface"] =
        json!({"type":"boolean", "const":false});
    // A005 surface programs use the stable `adorn_` prefix while older rows
    // use `surface_`; both are Rust-owned projected IDs.
    if let Some(branches) = schema
        .pointer_mut("/properties/patch/properties/operations/items/anyOf")
        .and_then(Value::as_array_mut)
    {
        for branch in branches {
            if branch
                .pointer("/properties/op/const")
                .and_then(Value::as_str)
                == Some("upsert_surface_binding")
            {
                branch["properties"]["binding"]["properties"]["surface_program_id"]["pattern"] =
                    json!("^(surface|adorn)_[A-Za-z0-9_\\-]+$");
            }
        }
    }
    let required = schema["required"]
        .as_array_mut()
        .expect("compact patch envelope must have required fields");
    required.push(Value::String("evidence_dispositions".into()));
    schema
}

fn compact_visual_claim_dispositions_schema() -> Value {
    json!({
        "type":"array",
        "minItems":1,
        "maxItems":256,
        "items":{
            "type":"object",
            "additionalProperties":false,
            "required":["claim_id","disposition","detail_ids","reason"],
            "properties":{
                "claim_id":{"type":"string","pattern":"^vclaim_[A-Za-z0-9_.:-]+$","maxLength":160},
                "disposition":{"enum":["bound","unresolved","evaluation_only"]},
                "detail_ids":{
                    "type":"array",
                    "maxItems":64,
                    "items":{"type":"string","pattern":"^detail_[A-Za-z0-9_.:-]+$","maxLength":160}
                },
                "reason":{"type":"string","minLength":1,"maxLength":320}
            }
        }
    })
}

fn compact_plan_provider_schema() -> Value {
    // An initial synthesis has no confirmed asset to bind.  Its Provider
    // projection therefore cannot advertise AssemblyDelta or a continuation
    // template.  The full Rust registry remains the execution validator, and
    // existing assets use compact_arm_continuation_provider_schema instead.
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["plan"],
        "properties": {
            "plan": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "plan_id", "domain_pack_id", "brief", "spec", "directions", "provider_id",
                    "arm_design_intent"
                ],
                "properties": {
                    "schema_version": {"type": "string", "const": "MechanicalConceptPlan@1"},
                    "plan_id": {"type": "string", "pattern": "^plan_[a-z0-9_\\-]+$"},
                    "domain_pack_id": {"type": "string", "pattern": "^pack_[a-z0-9_\\-]+$"},
                    "brief": {"type": "string", "minLength": 1, "maxLength": 2000},
                    "generation_stage": {"type": "string", "const": "blockout"},
                    "spec": {"type": "object"},
                    "directions": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 1,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": [
                                "direction_id", "title", "summary", "silhouette",
                                "primary_part_roles", "material_direction"
                            ],
                            "properties": {
                                "direction_id": {"type": "string", "pattern": "^direction_[a-z0-9_\\-]+$"},
                                "title": {"type": "string", "minLength": 1, "maxLength": 80},
                                "summary": {"type": "string", "minLength": 1, "maxLength": 500},
                                "silhouette": {
                                    "type": "string",
                                    "enum": ["compact", "balanced", "extended", "organic", "industrial"]
                                },
                                "primary_part_roles": {
                                    "type": "array", "minItems": 2, "maxItems": 16,
                                    "items": {"type": "string"}
                                },
                                "material_direction": {"type": "string", "minLength": 1, "maxLength": 160}
                            }
                        }
                    },
                    "provider_id": {"type": "string", "minLength": 1, "maxLength": 120},
                    "model": {"anyOf": [{"type": "string", "maxLength": 160}, {"type": "null"}]},
                    "arm_design_intent": {
                        "anyOf": [
                            {
                                "type": "object",
                                "additionalProperties": false,
                                "required": [
                                    "architecture", "joint_language", "link_language", "base_language",
                                    "wrist_language", "end_effector_language", "cable_language",
                                    "surface_language", "material_palette", "detail_density", "pose",
                                    "proportion_profile"
                                ],
                                "properties": {
                                    "schema_version": {"type": "string", "const": "ArmDesignIntent@1"},
                                    "domain_pack_id": {"type": "string", "const": "pack_robotic_arm_concept"},
                                    "architecture": {"type": "string", "enum": ["serial_chain", "parallel_link", "scara", "gantry", "delta", "cantilever"]},
                                    "joint_language": {"type": "string", "enum": ["armored_bearing", "exposed_ring", "gimbal_shell", "capsule_joint", "bellows_joint"]},
                                    "link_language": {"type": "string", "enum": ["closed_shell", "twin_rail", "open_truss", "tapered_loft", "tube_frame"]},
                                    "base_language": {"type": "string", "enum": ["round_turntable", "hex_platform", "floating_pedestal", "industrial_deck", "compact_puck"]},
                                    "wrist_language": {"type": "string", "enum": ["layered_wrist", "gimbal_wrist", "cylindrical_wrist", "fork_wrist"]},
                                    "end_effector_language": {"type": "string", "enum": ["parallel_gripper", "adaptive_claw", "precision_tool", "sensor_probe", "soft_pad_gripper"]},
                                    "cable_language": {"type": "string", "enum": ["internal_routing", "braided_external", "armored_harness", "minimal_cable"]},
                                    "surface_language": {"type": "array", "minItems": 1, "maxItems": 6, "items": {"type": "string", "enum": ["panel_seams", "flowline", "chevron_relief", "hex_microgrid", "engraved_ribs", "fastener_bands"]}},
                                    "material_palette": {"type": "string", "enum": ["graphite_blue", "white_aluminum", "industrial_yellow", "warm_copper", "monochrome_technical"]},
                                    "detail_density": {"type": "string", "enum": ["light", "medium", "dense"]},
                                    "pose": {"type": "string", "enum": ["neutral", "grounded", "elevated", "extended", "folded"]},
                                    "proportion_profile": {"type": "string", "enum": ["compact", "balanced", "long_reach", "heavy_base", "slender"]},
                                    "style_keywords": {"type": "array", "maxItems": 12, "items": {"type": "string"}},
                                    "source": {"type": "string", "enum": ["user_brief", "reference_evidence", "agent_inferred"]},
                                    "visual_only": {"type": "boolean", "const": true}
                                }
                            },
                            {"type": "null"}
                        ]
                    },
                    "shape_program_ready": {"type": "boolean"}
                }
            }
        }
    })
}

/// The live edit prompt already carries the Rust-owned ActiveDesignSnapshot.
/// Asking a Provider to restate an entire initial-synthesis plan during an
/// edit made valid AssemblyDelta calls needlessly fragile.  This envelope is
/// intentionally only an input projection; it accepts no plan metadata or
/// extra Provider-controlled fields and is normalized below before the full
/// registry schema is checked.
fn compact_arm_continuation_provider_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["plan"],
        "properties": {
            "plan": {
                "type": "object",
                "additionalProperties": false,
                "required": ["continuation_template_id"],
                "properties": {
                    "continuation_template_id": {"const": "next_reviewed_attachment"}
                }
            }
        }
    })
}

fn normalize_arm_continuation_arguments(
    supplied_arguments: BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, ProductToolRegistryError> {
    let supplied_value = Value::Object(supplied_arguments.into_iter().collect::<Map<_, _>>());
    validate_json_schema(&compact_arm_continuation_provider_schema(), &supplied_value).map_err(
        |message| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID",
                ProductToolRegistryErrorKind::InvalidArguments,
                message,
            )
        },
    )?;
    let continuation_template_id = supplied_value
        .pointer("/plan/continuation_template_id")
        .and_then(Value::as_str)
        .expect("the compact continuation schema requires continuation_template_id");
    let plan_id = format!(
        "plan_continuation_{}",
        &sha256_hex(continuation_template_id.as_bytes())[..20]
    );
    value_to_btree_object(&json!({
        "plan": {
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": plan_id,
            "domain_pack_id": "pack_robotic_arm_concept",
            "brief": "Continue the current non-functional robotic-arm concept with one Rust-selected reviewed visual attachment.",
            "spec": {},
            "directions": [{
                "direction_id": "direction_current_arm",
                "title": "Current robotic-arm continuation",
                "summary": "One bounded visual-only edit to the current confirmed robotic-arm concept.",
                "silhouette": "industrial",
                "primary_part_roles": ["link_armor", "surface_trim"],
                "material_direction": "Preserve the current Rust-owned material zones and reviewed PBR exterior."
            }],
            "provider_id": "provider_compact_continuation",
            "continuation_template_id": continuation_template_id
        }
    }))
    .ok_or_else(|| {
        ProductToolRegistryError::new(
            "PRODUCT_TOOL_CONTINUATION_NORMALIZATION_FAILED",
            ProductToolRegistryErrorKind::InvalidArguments,
            "The compact continuation could not be normalized into the Product Tool contract.",
        )
    })
}

fn validate_fixture_header_and_manifest(
    fixture: &ProductToolRegistryFixture,
) -> Result<(), ProductToolRegistryError> {
    let canonicalization_valid = fixture
        .canonicalization
        .get("algorithm")
        .and_then(Value::as_str)
        == Some("sha256")
        && fixture
            .canonicalization
            .get("encoding")
            .and_then(Value::as_str)
            == Some("utf-8")
        && fixture
            .canonicalization
            .get("ensure_ascii")
            .and_then(Value::as_bool)
            == Some(false)
        && fixture
            .canonicalization
            .get("json_sort_keys")
            .and_then(Value::as_bool)
            == Some(true)
        && fixture
            .canonicalization
            .get("manifest_hash_scope")
            .and_then(Value::as_str)
            == Some("public_manifest_without_derived_hashes");
    if fixture.schema_version != "K002ProductToolRegistryFixture@1"
        || fixture.fixture_id != "k002_shared_a004_product_tool_registry"
        || fixture.registry_schema_version != PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION
        || fixture.tools.len() != K002_FIXTURE_TOOL_DEFINITION_COUNT
        || !canonicalization_valid
    {
        return Err(ProductToolRegistryError::new(
            "PRODUCT_TOOL_FIXTURE_HEADER_INVALID",
            ProductToolRegistryErrorKind::InvalidSchema,
            "Shared Product Tool fixture header or canonicalization contract is invalid.",
        ));
    }
    let public_tools = fixture
        .tools
        .iter()
        .map(|entry| {
            json!({
                "tool_id": entry.tool_id,
                "name": entry.name,
                "description": entry.description,
                "input_schema": entry.input_schema,
                "output_schema": entry.output_schema,
                "approval_policy": entry.approval_policy,
            })
        })
        .collect::<Vec<_>>();
    let actual_manifest_sha256 = sha256_hex(
        canonical_json(&json!({
            "schema_version": fixture.registry_schema_version,
            "tools": public_tools,
        }))
        .as_bytes(),
    );
    if actual_manifest_sha256 != fixture.manifest_sha256 {
        return Err(ProductToolRegistryError::new(
            "PRODUCT_TOOL_FIXTURE_MANIFEST_HASH_MISMATCH",
            ProductToolRegistryErrorKind::InvalidSchema,
            "Shared Product Tool fixture manifest hash does not match its public manifest.",
        ));
    }
    Ok(())
}

fn schema_digest(schema: &Value) -> String {
    sha256_hex(canonical_json(schema).as_bytes())
}

fn value_to_btree_object(value: &Value) -> Option<BTreeMap<String, Value>> {
    value.as_object().map(|map| {
        map.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    })
}

/// Structural validation for the fixed manifest. Runtime instance validation
/// below intentionally supports only the keywords used by this code-owned
/// A004 registry; it is not exposed as a general JSON Schema engine.
fn validate_schema_definition(schema: &Value) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| "Code-owned schema must be a JSON object.".to_string())?;
    if !object.contains_key("type")
        && !object.contains_key("enum")
        && !object.contains_key("$ref")
        && !object.contains_key("anyOf")
    {
        return Err("Code-owned schema must declare type, enum, anyOf, or a local ref.".into());
    }
    if let Some(types) = object.get("type") {
        let valid = types.as_str().map_or(false, is_supported_type)
            || types.as_array().is_some_and(|kinds| {
                !kinds.is_empty()
                    && kinds
                        .iter()
                        .all(|kind| kind.as_str().is_some_and(is_supported_type))
            });
        if !valid {
            return Err("Code-owned schema uses an unsupported type declaration.".into());
        }
    }
    if let Some(required) = object.get("required") {
        if !required
            .as_array()
            .is_some_and(|keys| keys.iter().all(Value::is_string))
        {
            return Err("Code-owned required must be an array of strings.".into());
        }
    }
    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| "Code-owned properties must be an object.".to_string())?;
        for child in properties.values() {
            validate_schema_definition(child)?;
        }
    }
    if let Some(definitions) = object.get("$defs") {
        let definitions = definitions
            .as_object()
            .ok_or_else(|| "Code-owned $defs must be an object.".to_string())?;
        for child in definitions.values() {
            validate_schema_definition(child)?;
        }
    }
    if let Some(branches) = object.get("anyOf") {
        let branches = branches
            .as_array()
            .filter(|branches| !branches.is_empty())
            .ok_or_else(|| "Code-owned anyOf must be a non-empty array.".to_string())?;
        for child in branches {
            validate_schema_definition(child)?;
        }
    }
    if let Some(items) = object.get("items") {
        validate_schema_definition(items)?;
    }
    Ok(())
}

fn is_supported_type(kind: &str) -> bool {
    matches!(
        kind,
        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
    )
}

fn validate_json_schema(schema: &Value, value: &Value) -> Result<(), String> {
    validate_json_schema_inner(schema, value, schema)
}

fn validate_json_schema_inner(schema: &Value, value: &Value, root: &Value) -> Result<(), String> {
    let schema = schema
        .as_object()
        .ok_or_else(|| "Code-owned schema must be a JSON object.".to_string())?;
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference
            .strip_prefix("#/$defs/")
            .ok_or_else(|| "Only local code-owned $defs references are supported.".to_string())?;
        let target = root
            .get("$defs")
            .and_then(Value::as_object)
            .and_then(|definitions| definitions.get(name))
            .ok_or_else(|| "Code-owned local schema reference is missing.".to_string())?;
        return validate_json_schema_inner(target, value, root);
    }
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        if !branches
            .iter()
            .any(|branch| validate_json_schema_inner(branch, value, root).is_ok())
        {
            return Err("Value does not match any code-owned anyOf branch.".into());
        }
    }
    if let Some(expected) = schema.get("const") {
        if expected != value {
            return Err("Value does not match the code-owned constant.".into());
        }
    }
    if let Some(expected) = schema.get("type") {
        let expected_types = if let Some(expected) = expected.as_str() {
            vec![expected]
        } else {
            expected
                .as_array()
                .ok_or_else(|| "Code-owned schema type declaration is invalid.".to_string())?
                .iter()
                .map(|kind| {
                    kind.as_str().ok_or_else(|| {
                        "Code-owned schema type array must contain strings.".to_string()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if !expected_types
            .iter()
            .any(|expected| value_matches_type(value, expected))
        {
            return Err(format!(
                "Value must have one of the code-owned JSON types: {}.",
                expected_types.join(", ")
            ));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return Err(format!(
                "Value is outside the code-owned enum {}.",
                serde_json::to_string(allowed)
                    .unwrap_or_else(|_| "<invalid-code-owned-enum>".into())
            ));
        }
    }
    if let Some(text) = value.as_str() {
        let count = text.chars().count() as u64;
        if schema
            .get("minLength")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| count < minimum)
            || schema
                .get("maxLength")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| count > maximum)
        {
            return Err("String violates code-owned length bounds.".into());
        }
        if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
            if !matches_known_pattern(pattern, text) {
                return Err(format!(
                    "String violates the code-owned stable pattern {pattern}."
                ));
            }
        }
    }
    if let Some(number) = value.as_f64() {
        if schema
            .get("minimum")
            .and_then(Value::as_f64)
            .is_some_and(|minimum| number < minimum)
            || schema
                .get("maximum")
                .and_then(Value::as_f64)
                .is_some_and(|maximum| number > maximum)
        {
            return Err("Number violates code-owned bounds.".into());
        }
    }
    if let Some(object) = value.as_object() {
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("Required property {key} is missing."));
                }
            }
        }
        if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            for key in object.keys() {
                if !properties.contains_key(key) {
                    return Err(format!("Property {key} is not allowed."));
                }
            }
        }
        for (key, child) in object {
            if let Some(child_schema) = properties.get(key) {
                validate_json_schema_inner(child_schema, child, root)?;
            }
        }
    }
    if let Some(array) = value.as_array() {
        if schema
            .get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|minimum| array.len() < minimum as usize)
            || schema
                .get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| array.len() > maximum as usize)
        {
            return Err("Array violates code-owned item bounds.".into());
        }
        if let Some(items) = schema.get("items") {
            for child in array {
                validate_json_schema_inner(items, child, root)?;
            }
        }
    }
    Ok(())
}

fn value_matches_type(value: &Value, expected: &str) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn matches_known_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "^[A-Za-z0-9_.:-]+$" {
        return !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
            });
    }
    if pattern == "^[A-Za-z0-9_:-]+$" {
        return !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'-'));
    }
    if pattern == "^[a-z][a-z0-9_\\-]{1,63}$" {
        let mut bytes = value.bytes();
        return bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
            && bytes.all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
            && (2..=64).contains(&value.len());
    }
    if pattern == "^(surface|adorn)_[A-Za-z0-9_\\-]+$" {
        return value
            .strip_prefix("surface_")
            .or_else(|| value.strip_prefix("adorn_"))
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'-')
                    })
            });
    }
    if pattern == "^vclaim_[A-Za-z0-9_.:-]+$" {
        return value.strip_prefix("vclaim_").is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
                })
        });
    }
    if pattern == "^detail_[A-Za-z0-9_.:-]+$" {
        return value.strip_prefix("detail_").is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
                })
        });
    }
    let prefix = match pattern {
        "^direction_[a-z0-9_\\-]+$" => "direction_",
        "^plan_[a-z0-9_\\-]+$" => "plan_",
        "^pack_[a-z0-9_\\-]+$" => "pack_",
        "^attempt_[a-z0-9_\\-]+$" => "attempt_",
        "^gate_[a-z0-9_\\-]+$" => "gate_",
        "^op_[a-z0-9_\\-]+$" => "op_",
        "^output_[a-z0-9_\\-]+$" => "output_",
        "^param_[a-z0-9_\\-]+$" => "param_",
        "^profileinput_[a-z0-9_\\-]+$" => "profileinput_",
        "^shape_[a-z0-9_\\-]+$" => "shape_",
        "^mat_[a-z0-9_\\-]+$" => "mat_",
        "^zone_[a-z0-9_\\-]+$" => "zone_",
        "^part_[A-Za-z0-9_:-]+$" => "part_",
        "^part_[A-Za-z0-9_\\-]+$" => "part_",
        "^zone_[A-Za-z0-9_\\-]+$" => "zone_",
        "^mat_[A-Za-z0-9_\\-]+$" => "mat_",
        "^surface_[A-Za-z0-9_\\-]+$" => "surface_",
        "^detail_[A-Za-z0-9_\\-]+$" => "detail_",
        "^assetver_[A-Za-z0-9_:-]+$" => "assetver_",
        _ => return false,
    };
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.bytes().all(|byte| {
                if matches!(
                    pattern,
                    "^part_[A-Za-z0-9_:-]+$"
                        | "^part_[A-Za-z0-9_\\-]+$"
                        | "^zone_[A-Za-z0-9_\\-]+$"
                        | "^mat_[A-Za-z0-9_\\-]+$"
                        | "^surface_[A-Za-z0-9_\\-]+$"
                        | "^detail_[A-Za-z0-9_\\-]+$"
                        | "^assetver_[A-Za-z0-9_:-]+$"
                ) {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'-')
                } else {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                }
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_exactly_seventeen_code_owned_non_permanent_tools() {
        let registry = ProductToolRegistry::default();
        assert_eq!(
            registry.definitions().count(),
            PRODUCT_TOOL_DEFINITION_COUNT
        );
        assert!(registry.definitions().all(|definition| {
            definition.approval_policy != ProductToolApprovalPolicy::UserConfirmationRequired
        }));
        assert!(registry.definition("compile_readback_candidate").is_ok());
        assert!(registry.definition("author_universal_asset").is_ok());
        assert!(registry.definition("inspect_forge_visual_program").is_ok());
        assert!(registry.definition("author_forge_visual_program").is_ok());
        assert!(registry.definition("patch_forge_visual_program").is_ok());
        assert!(registry.definition("arbitrary_shell").is_err());
    }

    #[test]
    fn provider_plan_schema_is_compact_but_rust_keeps_the_full_registry_contract() {
        let registry = ProductToolRegistry::default();
        let full = registry.definition("plan_complete_concept").unwrap();
        let provider = registry
            .provider_definitions()
            .into_iter()
            .find(|definition| definition.name == "plan_complete_concept")
            .unwrap();
        let provider_bytes = serde_json::to_vec(&provider.input_schema).unwrap().len();
        let full_bytes = serde_json::to_vec(&full.input_schema).unwrap().len();
        // The initial-synthesis projection must remain smaller than the full
        // registry contract and below a bounded Provider request size.
        assert!(provider_bytes < 20_000);
        assert!(full_bytes > 10_000);
        assert!(provider_bytes < full_bytes);
        assert_ne!(provider.input_schema, full.input_schema);
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/arm_design_intent")
            .is_some());
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/assembly_delta")
            .is_none());
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/continuation_template_id")
            .is_none());
        // The exact full schema is still used at the execution boundary.
        let valid = json!({
            "plan": {
                "plan_id": "plan_provider_schema",
                "domain_pack_id": "pack_robotic_arm_concept",
                "brief": "非功能性机械臂",
                "spec": {},
                "directions": [{
                    "direction_id": "direction_provider_schema",
                    "title": "机械臂",
                    "summary": "生产级概念资产",
                    "silhouette": "industrial",
                    "primary_part_roles": ["body_shell", "link_armor"],
                    "material_direction": "graphite blue"
                }],
                "provider_id": "deepseek"
            }
        });
        let call = ProviderToolCall {
            call_id: "provider_schema_call".into(),
            name: "plan_complete_concept".into(),
            arguments: valid,
        };
        assert!(registry
            .build_execution_request(
                "turn_provider_schema",
                &call,
                "execution_provider_schema",
                "cancel_provider_schema",
                "token_provider_schema"
            )
            .is_ok());
    }

    #[test]
    fn visual_incremental_edit_projection_excludes_complete_graph_replacements() {
        let registry = ProductToolRegistry::default();
        let definition = registry.visual_incremental_edit_provider_definition();
        assert_eq!(definition.name, "patch_forge_visual_program");
        assert!(definition.description.contains("typed incremental edit"));
        assert!(!definition
            .input_schema
            .to_string()
            .contains("replace_geometry_graph"));
        assert!(definition
            .input_schema
            .to_string()
            .contains("upsert_material_binding"));
        let inspect = registry.visual_incremental_edit_inspect_provider_definition();
        assert_eq!(
            inspect
                .input_schema
                .pointer("/properties/view/const")
                .and_then(Value::as_str),
            Some("summary")
        );
        assert!(inspect
            .description
            .contains("Do not request the full source"));
    }

    #[test]
    fn inspect_visual_program_defaults_an_empty_provider_payload_to_summary_view() {
        let registry = ProductToolRegistry::default();
        let call = ProviderToolCall {
            call_id: "inspect_default_summary".into(),
            name: "inspect_forge_visual_program".into(),
            arguments: json!({}),
        };

        let request = registry
            .build_execution_request_for_mode(
                "turn_inspect_default_summary",
                &call,
                "execution_inspect_default_summary",
                "cancel_inspect_default_summary",
                "token_inspect_default_summary",
                ProviderToolInputMode::InitialSynthesis,
            )
            .expect("the injected summary view must satisfy the inspection schema");

        assert_eq!(request.tool_name, "inspect_forge_visual_program");
        assert_eq!(
            request
                .validated_arguments
                .value
                .get("view")
                .and_then(Value::as_str),
            Some("summary")
        );
    }

    #[test]
    fn inspect_visual_program_drops_display_only_provider_flags_before_defaulting_view() {
        let registry = ProductToolRegistry::default();
        let call = ProviderToolCall {
            call_id: "inspect_drop_display_flag".into(),
            name: "inspect_forge_visual_program".into(),
            arguments: json!({"include_style_recipe": true}),
        };

        let request = registry
            .build_execution_request_for_mode(
                "turn_inspect_drop_display_flag",
                &call,
                "execution_inspect_drop_display_flag",
                "cancel_inspect_drop_display_flag",
                "token_inspect_drop_display_flag",
                ProviderToolInputMode::InitialSynthesis,
            )
            .expect("display-only flags must not make a read-only inspection fail");

        assert_eq!(
            request
                .validated_arguments
                .value
                .get("view")
                .and_then(Value::as_str),
            Some("summary")
        );
        assert_eq!(request.validated_arguments.value.len(), 1);
        assert!(!request
            .validated_arguments
            .value
            .contains_key("include_style_recipe"));
    }

    #[test]
    fn pv008_provider_receives_compact_authoring_intent_and_typed_patch_contracts() {
        let registry = ProductToolRegistry::default();
        let definitions = registry.provider_definitions();
        let author = definitions
            .iter()
            .find(|definition| definition.name == "author_forge_visual_program")
            .unwrap();
        assert_eq!(
            author
                .input_schema
                .pointer("/properties/authoring_intent/properties/schema_version/const")
                .and_then(Value::as_str),
            Some("ForgeVisualAuthoringIntent@1")
        );
        assert_eq!(
            author
                .input_schema
                .pointer("/properties/authoring_intent/properties/arm_design_intent/properties/domain_pack_id/const")
                .and_then(Value::as_str),
            Some("pack_robotic_arm_concept")
        );
        assert!(author
            .input_schema
            .pointer("/properties/authoring_intent/properties/arm_design_intent/properties/surface_language")
            .is_some());
        assert!(author.input_schema.pointer("/properties/program").is_none());
        assert!(author
            .input_schema
            .pointer("/properties/authoring_intent/properties/geometry_graph")
            .is_none());
        assert!(author
            .description
            .contains("Rust derives every ShapeProgram"));
        assert!(author
            .input_schema
            .pointer("/properties/evidence_dispositions/items/properties/disposition/enum")
            .is_some());
        assert!(author
            .input_schema
            .pointer("/properties/evidence_dispositions/items/properties/detail_ids")
            .is_none());

        let patch = definitions
            .iter()
            .find(|definition| definition.name == "patch_forge_visual_program")
            .unwrap();
        let operations = patch
            .input_schema
            .pointer("/properties/patch/properties/operations/items/properties/op/enum")
            .and_then(Value::as_array)
            .unwrap();
        assert!(operations.contains(&json!("replace_geometry_graph")));
        assert!(operations.contains(&json!("replace_material_graph")));
        assert!(operations.contains(&json!("replace_surface_graph")));
        assert!(patch
            .input_schema
            .pointer("/properties/patch/properties/expected_source_sha256")
            .is_some());
        assert!(patch
            .input_schema
            .pointer("/properties/evidence_dispositions/items/properties/detail_ids")
            .is_some());
        assert!(patch.description.contains("complete ShapeProgram@1"));
        assert!(patch
            .description
            .contains("radial_array is never inputs=[]"));
        assert_eq!(
            patch
                .input_schema
                .pointer(
                    "/properties/patch/properties/operations/items/properties/geometry_graph/properties/schema_version/const"
                )
                .and_then(Value::as_str),
            Some("ShapeProgram@1"),
            "replace_geometry_graph must no longer advertise an untyped object"
        );
    }

    #[test]
    fn visual_author_execution_defers_program_semantics_to_the_native_rust_validator() {
        let registry = ProductToolRegistry::default();
        let mut program = native_executor::reviewed_c111_draft_visual_program()
            .expect("the reviewed C111 visual program must remain available");
        program["geometry_graph"]
            .as_object_mut()
            .expect("the reviewed C111 geometry graph must be an object")
            .remove("profile_inputs");
        let valid_call = ProviderToolCall {
            call_id: "visual_reviewed_program".into(),
            name: "author_forge_visual_program".into(),
            arguments: json!({"program":program.clone()}),
        };
        registry
            .build_execution_request_for_mode(
                "turn_visual_reviewed_program",
                &valid_call,
                "execution_visual_reviewed_program",
                "cancel_visual_reviewed_program",
                "token_visual_reviewed_program",
                ProviderToolInputMode::InitialSynthesis,
            )
            .expect("the reviewed program must satisfy the advertised author schema");
        program["geometry_graph"]["operations"][0]["op"] = json!("arbitrary_script");
        let call = ProviderToolCall {
            call_id: "visual_unknown_operation".into(),
            name: "author_forge_visual_program".into(),
            arguments: json!({"program":program}),
        };

        let request = registry
            .build_execution_request_for_mode(
                "turn_visual_unknown_operation",
                &call,
                "execution_visual_unknown_operation",
                "cancel_visual_unknown_operation",
                "token_visual_unknown_operation",
                ProviderToolInputMode::InitialSynthesis,
            )
            .expect("the bounded program object must reach the native Rust validator");
        assert_eq!(
            request.validated_arguments.value["program"]["geometry_graph"]["operations"][0]["op"],
            json!("arbitrary_script")
        );
    }

    #[test]
    fn pv008_authoring_intent_is_lowered_before_the_native_tool_boundary() {
        let registry = ProductToolRegistry::default();
        let call = ProviderToolCall {
            call_id: "visual_provider_intent".into(),
            name: "author_forge_visual_program".into(),
            arguments: json!({
                "authoring_intent":{
                    "schema_version":"ForgeVisualAuthoringIntent@1",
                    "authoring_id":"authoring_deepsea_arm",
                    "title":"深海维修机械臂",
                    "arm_design_intent":{
                        "schema_version":"ArmDesignIntent@1",
                        "domain_pack_id":"pack_robotic_arm_concept",
                        "architecture":"serial_chain",
                        "joint_language":"exposed_ring",
                        "link_language":"open_truss",
                        "base_language":"industrial_deck",
                        "wrist_language":"fork_wrist",
                        "end_effector_language":"adaptive_claw",
                        "cable_language":"braided_external",
                        "surface_language":["panel_seams","flowline","fastener_bands"],
                        "material_palette":"graphite_blue",
                        "detail_density":"dense",
                        "pose":"extended",
                        "proportion_profile":"long_reach",
                        "style_keywords":["deep sea","industrial collectible"],
                        "source":"agent_inferred",
                        "visual_only":true
                    }
                }
            }),
        };
        let request = registry
            .build_execution_request_for_mode(
                "turn_visual_provider_intent",
                &call,
                "execution_visual_provider_intent",
                "cancel_visual_provider_intent",
                "token_visual_provider_intent",
                ProviderToolInputMode::InitialSynthesis,
            )
            .expect("compact Provider intent must lower to the full internal program");
        assert!(!request
            .validated_arguments
            .value
            .contains_key("authoring_intent"));
        assert!(request.validated_arguments.value["program"]["program_id"]
            .as_str()
            .unwrap()
            .starts_with("visualprog_provider_ir_"));
        let reviewed_output_count = forgecad_core::reviewed_c111_draft_visual_program()
            .unwrap()
            .geometry_graph["outputs"]
            .as_array()
            .unwrap()
            .len();
        assert!(reviewed_output_count >= 96);
        assert_eq!(
            request.validated_arguments.value["program"]["geometry_graph"]["outputs"]
                .as_array()
                .unwrap()
                .len(),
            reviewed_output_count,
            "Provider input normalization must retain the complete reviewed substrate"
        );
    }

    #[test]
    fn compact_visual_patch_schema_accepts_only_strict_local_upserts() {
        let schema = compact_forge_visual_patch_schema();
        let patch = |operation: Value| {
            json!({
                "patch": {
                    "schema_version":"ForgeVisualPatch@1",
                    "patch_id":"patch_local_row",
                    "expected_revision":2,
                    "expected_source_sha256":"a".repeat(64),
                    "preserve_geometry":false,
                    "preserve_material_surface":false,
                    "operations":[operation]
                }
            })
        };

        for operation in [
            json!({
                "op":"upsert_geometry_operation",
                "operation_id":"op_target",
                "operation":{
                    "operation_id":"op_target",
                    "op":"box",
                    "inputs":[],
                    "args":{"size":[10, 20, 30]}
                }
            }),
            json!({
                "op":"upsert_material_binding",
                "binding":{
                    "part_id":"part_target",
                    "material_zone_id":"zone_target",
                    "material_id":"mat_copper"
                }
            }),
            json!({
                "op":"upsert_surface_binding",
                "binding":{
                    "surface_program_id":"surface_target",
                    "part_id":"part_target",
                    "material_zone_id":"zone_target"
                }
            }),
            json!({
                "op":"upsert_detail_inventory_item",
                "detail":{
                    "detail_id":"detail_target",
                    "level":"meso",
                    "description":"Bound panel segmentation for the existing visual target.",
                    "critical":true,
                    "status":"bound",
                    "bindings":[{
                        "kind":"material_zone",
                        "part_id":"part_target",
                        "target_id":"zone_target"
                    }]
                }
            }),
        ] {
            let operation_name = operation
                .get("op")
                .and_then(Value::as_str)
                .unwrap()
                .to_owned();
            let validation = validate_json_schema(&schema, &patch(operation));
            assert!(
                validation.is_ok(),
                "{operation_name} must be advertised with its complete typed row shape: {validation:?}"
            );
        }

        let rejected = patch(json!({
            "op":"upsert_material_binding",
            "binding":{
                "part_id":"part_target",
                "material_zone_id":"zone_target",
                "material_id":"mat_copper",
                "arbitrary_graph_replacement":true
            }
        }));
        assert!(validate_json_schema(&schema, &rejected).is_err());
    }

    #[test]
    fn compact_visual_repair_patch_schema_rejects_legacy_replacements() {
        let schema = compact_forge_visual_repair_patch_schema();
        let patch = |operation: Value| {
            json!({
                "patch": {
                    "schema_version":"ForgeVisualPatch@1",
                    "patch_id":"patch_local_only",
                    "expected_revision":2,
                    "expected_source_sha256":"a".repeat(64),
                    "preserve_geometry":false,
                    "preserve_material_surface":false,
                    "operations":[operation]
                },
                "evidence_dispositions":[{
                    "claim_id":"vclaim_meso_target",
                    "disposition":"bound",
                    "detail_ids":["detail_target"],
                    "reason":"Repair the exact current row."
                }]
            })
        };
        assert!(validate_json_schema(
            &schema,
            &patch(json!({"op":"set_title", "title":"Do not admit presentation edits"}))
        )
        .is_err());
        assert!(validate_json_schema(
            &schema,
            &patch(json!({
                "op":"replace_geometry_graph",
                "geometry_graph":{}
            }))
        )
        .is_err());
        let too_many_rows = json!({
            "patch": {
                "schema_version":"ForgeVisualPatch@1",
                "patch_id":"patch_too_many_rows",
                "expected_revision":2,
                "expected_source_sha256":"a".repeat(64),
                "preserve_geometry":false,
                "preserve_material_surface":false,
                "operations":(0..33).map(|index| json!({
                    "op":"upsert_material_binding",
                    "binding":{
                        "part_id":format!("part_target_{index}"),
                        "material_zone_id":format!("zone_target_{index}"),
                        "material_id":"mat_copper"
                    }
                })).collect::<Vec<_>>()
            },
            "evidence_dispositions":[{
                "claim_id":"vclaim_meso_target",
                "disposition":"bound",
                "detail_ids":["detail_target"],
                "reason":"Repair the exact current row."
            }]
        });
        assert!(
            validate_json_schema(&schema, &too_many_rows).is_err(),
            "Provider repair schema must enforce the native 32-row limit"
        );
        let adorn_validation = validate_json_schema(
            &schema,
            &json!({
                "patch": {
                    "schema_version":"ForgeVisualPatch@1",
                    "patch_id":"patch_adorn_surface",
                    "expected_revision":2,
                    "expected_source_sha256":"a".repeat(64),
                    "preserve_geometry":false,
                    "preserve_material_surface":false,
                    "operations":[{
                        "op":"upsert_surface_binding",
                        "binding":{
                            "surface_program_id":"adorn_c111_link_groove",
                            "part_id":"part_target",
                            "material_zone_id":"zone_target"
                        }
                    }]
                },
                "evidence_dispositions":[{
                    "claim_id":"vclaim_meso_target",
                    "disposition":"bound",
                    "detail_ids":["detail_target"],
                    "reason":"Repair the exact current row."
                }]
            }),
        );
        assert!(
            adorn_validation.is_ok(),
            "A005 adorn IDs are valid Rust-projected surface targets: {adorn_validation:?}"
        );
        let mut contradictory_locks = patch(json!({
            "op":"upsert_material_binding",
            "binding":{
                "part_id":"part_target",
                "material_zone_id":"zone_target",
                "material_id":"mat_copper"
            }
        }));
        contradictory_locks["patch"]["preserve_material_surface"] = json!(true);
        assert!(validate_json_schema(&schema, &contradictory_locks).is_err());

        let repair_definition = ProductToolRegistry::default().visual_repair_provider_definition();
        assert_eq!(repair_definition.name, "patch_forge_visual_program");
        assert!(repair_definition
            .description
            .contains("upsert_geometry_operation"));
        assert!(!repair_definition
            .description
            .contains("replace_geometry_graph"));
    }

    #[test]
    fn arm_continuation_provider_schema_accepts_only_a_template_selector_and_normalizes_to_full_plan(
    ) {
        let registry = ProductToolRegistry::default();
        let provider = registry
            .provider_definitions_for_mode(ProviderToolInputMode::ArmContinuationDelta)
            .into_iter()
            .find(|definition| definition.name == "plan_complete_concept")
            .unwrap();
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/continuation_template_id")
            .is_some());
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/assembly_delta")
            .is_none());
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/arm_design_intent")
            .is_none());

        let compact_call = ProviderToolCall {
            call_id: "continuation_delta_call".into(),
            name: "plan_complete_concept".into(),
            arguments: json!({
                "plan": {
                    "continuation_template_id": "next_reviewed_attachment"
                }
            }),
        };
        let request = registry
            .build_execution_request_for_mode(
                "turn_continuation",
                &compact_call,
                "execution_continuation",
                "cancel_continuation",
                "token_continuation",
                ProviderToolInputMode::ArmContinuationDelta,
            )
            .unwrap();
        let plan = request
            .validated_arguments
            .value
            .get("plan")
            .and_then(Value::as_object)
            .unwrap();
        assert!(plan
            .get("plan_id")
            .and_then(Value::as_str)
            .is_some_and(|plan_id| plan_id.starts_with("plan_continuation_")));
        assert_eq!(
            plan.get("continuation_template_id").and_then(Value::as_str),
            Some("next_reviewed_attachment")
        );
        assert!(plan.get("arm_design_intent").is_none());
    }

    #[test]
    fn arm_continuation_rejects_unknown_envelope_fields_before_full_plan_normalization() {
        let registry = ProductToolRegistry::default();
        let call = ProviderToolCall {
            call_id: "continuation_invalid_call".into(),
            name: "plan_complete_concept".into(),
            arguments: json!({
                "plan": {
                    "continuation_template_id": "next_reviewed_attachment",
                    "brief": "Provider must not supply initial-plan metadata here."
                }
            }),
        };
        let error = registry
            .build_execution_request_for_mode(
                "turn_continuation_invalid",
                &call,
                "execution_continuation_invalid",
                "cancel_continuation_invalid",
                "token_continuation_invalid",
                ProviderToolInputMode::ArmContinuationDelta,
            )
            .unwrap_err();
        assert_eq!(error.code, "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID");
    }

    #[test]
    fn registry_schema_digests_match_python_a004_boundary_manifest() {
        let registry = ProductToolRegistry::default();
        let expected = [
            (
                "infer_product_domain",
                "36c2335632e1ac499f1db2437d9e6d792134aa0461340358ecdff5d99a8946da",
                "4858caa173037cdc49183332e3542a94f1be5ec5a386861d3fd148b615b038f9",
            ),
            (
                "research_approved_references",
                "251e933cd9ea7b630c9ba78cb45204a19af4612434eb44c8017d5c074870b5d0",
                "c5ecd34bb7aebad4501ccdfde9eb4d8f1cad368e8e23cd14a0c4d9c9bbfee3c0",
            ),
            (
                "select_style_recipe",
                "d47a87b36d4b4fdbd7d84db35649b10aff0a05b07ed3dbf15415a18b1e55c7e2",
                "b1ec14a13f3ad5976fb55fbe4e05f5d514cc360480e5cd4fb30d47d0bfefc899",
            ),
            (
                "author_profile_sketch",
                "c18b0316633302398a5a66b64525c85b6a6c410e28f625a4a8deecd35a1da6ec",
                "70f0af7aa89ddf179f3f9f6108757a503a4ad97e63cbce6240dd4031c5c5ca3f",
            ),
            (
                "validate_profile_sketch",
                "c18b0316633302398a5a66b64525c85b6a6c410e28f625a4a8deecd35a1da6ec",
                "70f0af7aa89ddf179f3f9f6108757a503a4ad97e63cbce6240dd4031c5c5ca3f",
            ),
            (
                "author_shape_program",
                "06edd48bb143ac0779a286911bb062547cee2ee13f1d0808938bdc386ba1be7f",
                "8a426d5559980c293cd9a35c470c82e80e769a358895e21e293e73953d747bb0",
            ),
            (
                "validate_shape_program",
                "06edd48bb143ac0779a286911bb062547cee2ee13f1d0808938bdc386ba1be7f",
                "8a426d5559980c293cd9a35c470c82e80e769a358895e21e293e73953d747bb0",
            ),
            (
                "plan_complete_concept",
                "2a7d15abd0aa07fbd157b111d03d1edef3cfd5082b85ebaf8bfc7c9953f42755",
                "680fb6a9db6a2b2c2ceaa72337e1bd5b90901c223ca51d4ecb02f5d219cf1101",
            ),
            (
                "build_candidate_geometry",
                "3f1df28ad9187cafb174157551fa73069833d97483dacf219a05c4088e6a0a2f",
                "bfe343df9e7aefbf2dd0de8998239fac8299d7929bab306f9fcd1edbfb5d6bf4",
            ),
            (
                "compile_readback_candidate",
                "d746974fa9afd5e951f76f9af38954b0ad7f436f2120dc974da65e5ee39f856f",
                "0174b9f9a227828a79dd8bf5661f81ac9398b6dcdd5395a6113ac94ae19a7db8",
            ),
            (
                "render_candidate_views",
                "d746974fa9afd5e951f76f9af38954b0ad7f436f2120dc974da65e5ee39f856f",
                "841547869d12018f914cca9afcf1876b89384dc2a344a429edb42ee1313e778f",
            ),
            (
                "evaluate_candidate",
                "d746974fa9afd5e951f76f9af38954b0ad7f436f2120dc974da65e5ee39f856f",
                "913210d8b4fbbf868f21280c1b8a8d6d933d1c2f94e5c1ec898f333e83ac56b7",
            ),
            (
                "prepare_candidate_preview",
                "d746974fa9afd5e951f76f9af38954b0ad7f436f2120dc974da65e5ee39f856f",
                "1f442df67f97e374449034eeb4c58a58325c2ca54cce227c972984f12bb5e1ce",
            ),
        ];
        for (name, input_sha, output_sha) in expected {
            let definition = registry.definition(name).unwrap();
            assert_eq!(definition.input_schema_sha256, input_sha, "{name} input");
            assert_eq!(definition.output_schema_sha256, output_sha, "{name} output");
        }
    }

    #[test]
    fn execution_request_uses_protocol_dto_and_rejects_schema_or_unknown_tools() {
        let registry = ProductToolRegistry::default();
        let valid = registry
            .build_execution_request(
                "turn_1",
                &ProviderToolCall {
                    call_id: "call_1".into(),
                    name: "compile_readback_candidate".into(),
                    arguments: json!({}),
                },
                "execution_1",
                "cancel_1",
                "cancel_token_1",
            )
            .unwrap();
        valid.validate().unwrap();
        let serialized = serde_json::to_string(&valid).unwrap();
        for forbidden in [
            "api_key",
            "database_path",
            "session_id",
            "reasoning_content",
        ] {
            assert!(!serialized.contains(forbidden));
        }

        for call in [
            ProviderToolCall {
                call_id: "call_2".into(),
                name: "compile_readback_candidate".into(),
                arguments: json!({"unknown": true}),
            },
            ProviderToolCall {
                call_id: "call_3".into(),
                name: "dynamic_plugin".into(),
                arguments: json!({}),
            },
        ] {
            assert!(registry
                .build_execution_request(
                    "turn_1",
                    &call,
                    "execution_2",
                    "cancel_2",
                    "cancel_token_2",
                )
                .is_err());
        }
    }

    #[test]
    fn result_must_match_protocol_identity_schema_digest_and_zero_side_effects() {
        let registry = ProductToolRegistry::default();
        let request = registry
            .build_execution_request(
                "turn_1",
                &ProviderToolCall {
                    call_id: "call_1".into(),
                    name: "compile_readback_candidate".into(),
                    arguments: json!({}),
                },
                "execution_1",
                "cancel_1",
                "cancel_token_1",
            )
            .unwrap();
        let definition = registry.definition(&request.tool_name).unwrap();
        let mut result = ProductToolExecutionResult {
            schema_version:
                forgecad_app_server_protocol::PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
            execution_id: request.execution_id.clone(),
            turn_id: request.turn_id.clone(),
            call_id: request.call_id.clone(),
            tool_id: request.tool_id.clone(),
            cancellation_id: request.cancellation_id.clone(),
            status: ProductToolExecutionStatus::Completed,
            validated_output: Some(ValidatedProductToolPayload {
                schema_id: format!("{}:output", request.tool_id),
                schema_sha256: definition.output_schema_sha256.clone(),
                value: BTreeMap::from([
                    ("triangle_count".into(), json!(1200)),
                    ("bounds_mm".into(), json!([100, 40, 30])),
                    ("mesh_count".into(), json!(2)),
                    ("primitive_count".into(), json!(3)),
                    ("material_count".into(), json!(2)),
                    (
                        "evidence_source".into(),
                        json!("geometry_compile_glb_readback"),
                    ),
                ]),
            }),
            failure_category: None,
            error_code: None,
            message: None,
            duration_ms: 12,
            permanent_side_effects: 0,
        };
        registry.validate_result(&request, &result).unwrap();
        result.permanent_side_effects = 1;
        assert!(registry.validate_result(&request, &result).is_err());
    }
}
