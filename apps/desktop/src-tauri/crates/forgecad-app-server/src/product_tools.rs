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
use forgecad_core::GameAssetDeliveryRequest;

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

    /// Binds explicit user delivery intent. The request has no part/socket
    /// IDs; the native executor derives those from the exact compiled source.
    fn bind_execution_game_asset_delivery_request(
        &self,
        _execution_id: &str,
        _turn_id: &str,
        _request: GameAssetDeliveryRequest,
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
                    "author_universal_asset" => "UniversalAuthorOutcome@1 any subject. IDs: robotic-arm, procedural.generic_visual_exterior_v1, procedural.generic_hard_surface_v1. Use generic_visual_exterior for organic/soft exteriors; no deformable is not quality_limited. Parts, requirements and plan rows use exact matching IDs; one plan row/part; include macro, meso, micro. Geometry excludes sphere/mesh/script; use ForgeVisualGeometryProgram@2, visual_ program_id, one output/part. Never arm/C111 for another subject.".into(),
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
                    ("author_universal_asset", _) => compact_universal_author_input_schema(),
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

    /// The open-category exterior route uses the same bounded VP204 patch
    /// language, but it must not be described to the Provider as a hard-
    /// surface repair. Keeping the projection separate prevents a failed
    /// visual comparison for a vehicle, building, furniture or other subject
    /// from nudging the next author turn back toward the hard-surface/robotic
    /// vocabulary.
    pub fn universal_visual_exterior_repair_provider_definition(&self) -> ProviderToolDefinition {
        let mut definition = self
            .provider_definitions_for_mode(ProviderToolInputMode::InitialSynthesis)
            .into_iter()
            .find(|definition| definition.name == "patch_forge_visual_program")
            .expect("code-owned registry must expose the visual patch tool");
        definition.input_schema = compact_universal_hard_surface_repair_patch_schema();
        definition.description = "Repair the current UAS@2 open-category visual exterior source with exactly one bounded ForgeVisualGeometryPatch@1. Preserve the identified subject, silhouette and part semantics; use only the Rust-projected source hash and stable node/material IDs. Do not turn the subject into a robotic arm, author a new object, replace a graph, call an arm tool, or add code, paths, URLs, dimensions, or unknown fields.".into();
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
                    "Complete ShapeProgram@1. box/wedge/cylinder/capsule/profile/loft/sweep use inputs=[]; extrude/revolve require one earlier profile; mirror requires one earlier mesh and non-zero axis; array requires one earlier mesh, count>=2, spacing>0 and non-zero axis; radial_array requires one earlier mesh, count>=2, radius>0, 0<angle<=2*pi and non-zero axis; bevel_approx/surface_panel/groove require one earlier box or bevel_approx; groove uses one axial face, bounded face_size, in-plane position and depth<=25% of its source normal extent; shell requires one earlier box and bounded positive thickness; lattice_deform requires one earlier mesh plus exactly eight non-zero bounded 2x2x2 corner offsets; local_mesh_patch requires one earlier mesh, normalized patch_center/radius and non-zero bounded patch_offset; union/subtract require 2-8 earlier non-profile meshes and boolean depth is at most 8.".into(),
        ),
    );
    shape_program
}

fn universal_author_tool_definition() -> ProductToolDefinition {
    let input_schema = universal_author_input_schema();
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

/// Build the Provider-facing universal author schema from the same checked-in
/// contract documents used by the TypeScript registry.  The native executor
/// still performs the authoritative Rust deserialization and semantic
/// validation; this projection only prevents a model from having to guess
/// the required SubjectProfile/FeatureContract/RepresentationPlan envelope.
///
/// The public documents use external `$ref` and `oneOf`, while the compact
/// Product Tool schema validator intentionally accepts only local refs and
/// `anyOf`.  Inline the small contract graph once at startup and normalize
/// those two transport details without creating a second contract.
fn universal_author_input_schema() -> Value {
    let document = concept_schema_document("universal-author-outcome.schema.json");
    let mut outcome = inline_concept_schema(&document, Some(&document));
    // The sealed request is Rust-owned and the native executor binds these
    // derived lineage values after the Provider's subject/feature/plan has
    // been parsed.  Requiring a model to reproduce Rust's canonical JSON
    // hashes at this earlier transport boundary rejects an otherwise valid
    // author result before the executor can perform that binding.  Keep the
    // checked-in contract strict; relax only this request-envelope projection
    // and let UniversalAuthorOutcome::validate remain authoritative later.
    relax_universal_author_lineage_fields(&mut outcome);
    json!({
        "type": "object",
        "properties": {
            "outcome": outcome,
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
    })
}

fn relax_universal_author_lineage_fields(schema: &mut Value) {
    const DERIVED_LINEAGE_FIELDS: [&str; 4] = [
        "request_sha256",
        "subject_profile_sha256",
        "visual_feature_contract_sha256",
        "capability_manifest_sha256",
    ];

    let Some(schema_object) = schema.as_object_mut() else {
        return;
    };

    if let Some(properties) = schema_object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        for (name, child) in properties.iter_mut() {
            if DERIVED_LINEAGE_FIELDS.contains(&name.as_str()) {
                *child = json!({
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 128,
                    "description": "Rust binds this lineage hash after exact request reproduction; do not rely on Provider-supplied value for identity."
                });
            } else {
                relax_universal_author_lineage_fields(child);
            }
        }
    }

    if let Some(branches) = schema_object
        .get_mut("anyOf")
        .and_then(Value::as_array_mut)
    {
        for branch in branches {
            relax_universal_author_lineage_fields(branch);
        }
    }

    if let Some(items) = schema_object.get_mut("items") {
        relax_universal_author_lineage_fields(items);
    }
}

/// The checked-in UniversalAuthorOutcome schema is intentionally complete and
/// remains the Rust execution contract.  Its external references expand into
/// three repeated contract trees when converted to a provider tool schema,
/// however.  That made a single image author request consume almost the whole
/// model context before DeepSeek could emit the required result.  This
/// provider projection keeps every field and cross-contract identity visible,
/// but removes repeated annotations and schema-only constraints; the native
/// executor still validates the exact shared schema and semantic hashes after
/// the call.  It is therefore a transport projection, not a second product
/// contract.
fn compact_universal_author_input_schema() -> Value {
    let sha256 = || json!({
        "type": "string",
        "pattern": "^[a-f0-9]{64}$",
        "maxLength": 64
    });
    // Rust derives these contract-lineage values after the sealed request is
    // reproduced.  Keep the Provider projection permissive here because the
    // model cannot be expected to reproduce Rust's canonical JSON hash.
    let derived_lineage = || json!({
        "type": "string",
        "minLength": 1,
        "maxLength": 128
    });
    let id = |max_length: u64| json!({
        "type": "string",
        "minLength": 1,
        "maxLength": max_length
    });
    let text = |max_length: u64| json!({
        "type": "string",
        "minLength": 1,
        "maxLength": max_length
    });

    let reference_input = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["evidence_id", "evidence_sha256", "role"],
        "properties": {
            "evidence_id": id(160),
            "evidence_sha256": sha256(),
            "role": text(80),
            "view_hint": {"type": ["string", "null"], "maxLength": 80}
        }
    });
    let active_asset = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["asset_version_id", "snapshot_revision", "source_sha256", "readback_sha256"],
        "properties": {
            "asset_version_id": id(160),
            "snapshot_revision": {"type": "integer", "minimum": 0},
            "source_sha256": sha256(),
            "readback_sha256": sha256()
        }
    });
    let selection = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["part_ids", "material_zone_ids"],
        "properties": {
            "part_ids": {"type": "array", "maxItems": 256, "items": id(160)},
            "material_zone_ids": {"type": "array", "maxItems": 256, "items": id(160)}
        }
    });
    let locks = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["preserve_geometry", "preserve_material_surface", "locked_part_ids", "locked_material_zone_ids"],
        "properties": {
            "preserve_geometry": {"type": "boolean"},
            "preserve_material_surface": {"type": "boolean"},
            "locked_part_ids": {"type": "array", "maxItems": 256, "items": id(160)},
            "locked_material_zone_ids": {"type": "array", "maxItems": 256, "items": id(160)}
        }
    });
    let request = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "request_id", "project_id", "turn_id", "instruction", "input_mode", "reference_inputs", "selection", "locks", "capability_manifest_sha256"],
        "properties": {
            "schema_version": {"const": "UniversalAuthorRequest@1"},
            "request_id": id(160),
            "project_id": id(160),
            "turn_id": id(160),
            "instruction": text(200000),
            "input_mode": {"enum": ["text", "single_image", "multiview", "active_asset", "mixed"]},
            "reference_inputs": {"type": "array", "maxItems": 12, "items": reference_input},
            "active_asset": {"anyOf": [{"type": "null"}, active_asset]},
            "selection": selection,
            "locks": locks,
            "capability_manifest_sha256": sha256()
        }
    });

    let part = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["part_id", "label", "semantic_role", "traits", "uncertainty_bps"],
        "properties": {
            "part_id": id(160),
            "parent_part_id": {"type": ["string", "null"], "maxLength": 160},
            "label": text(240),
            "semantic_role": text(160),
            "traits": {"type": "array", "maxItems": 64, "items": text(120)},
            "uncertainty_bps": {"type": "integer", "minimum": 0, "maximum": 10000}
        }
    });
    let subject_feature = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["feature_id", "part_id", "level", "description"],
        "properties": {
            "feature_id": id(160),
            "part_id": id(160),
            "level": {"enum": ["macro", "meso", "micro"]},
            "description": text(1200)
        }
    });
    let material = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["material_id", "label", "part_ids", "appearance_traits"],
        "properties": {
            "material_id": id(160),
            "label": text(240),
            "part_ids": {"type": "array", "maxItems": 256, "items": id(160)},
            "appearance_traits": {"type": "array", "maxItems": 64, "items": text(120)}
        }
    });
    let subject_profile = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "profile_id", "request_sha256", "identity_label", "category", "category_tags", "silhouette", "negative_space", "pose", "visible_views", "occlusions", "uncertainties", "parts", "features", "materials"],
        "properties": {
            "schema_version": {"const": "SubjectProfile@1"},
            "profile_id": id(160),
            "request_sha256": derived_lineage(),
            "identity_label": text(240),
            "category": text(240),
            "category_tags": {"type": "array", "maxItems": 64, "items": text(120)},
            "silhouette": text(1200),
            "negative_space": text(1200),
            "pose": text(1200),
            "visible_views": {"type": "array", "maxItems": 32, "items": text(80)},
            "occlusions": {"type": "array", "maxItems": 64, "items": text(320)},
            "uncertainties": {"type": "array", "maxItems": 64, "items": text(320)},
            "parts": {"type": "array", "minItems": 1, "maxItems": 256, "items": part},
            "features": {"type": "array", "minItems": 3, "maxItems": 512, "items": subject_feature},
            "materials": {"type": "array", "maxItems": 128, "items": material}
        }
    });

    let mut evidence_id = id(160);
    evidence_id["description"] = json!(
        "Copy byte-for-byte from request.reference_inputs[].evidence_id or the reference_evidence_ledger; never invent image_1/reference_1."
    );
    let region = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["evidence_id"],
        "properties": {
            "evidence_id": evidence_id,
            "view_id": {"type": ["string", "null"], "maxLength": 80},
            "region_per_mille": {"type": ["array", "null"], "minItems": 4, "maxItems": 4, "items": {"type": "integer", "minimum": 0, "maximum": 1000}}
        }
    });
    let requirement = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["feature_id", "level", "description", "salience_bps", "evidence_status", "evidence_regions", "affected_part_ids", "channels", "minimum_acceptance_views"],
        "properties": {
            "feature_id": id(160),
            "level": {"enum": ["macro", "meso", "micro"]},
            "description": text(1200),
            "salience_bps": {"type": "integer", "minimum": 0, "maximum": 10000},
            "evidence_status": {
                "enum": ["observed", "inferred", "hidden", "conflicting"],
                "description": "observed requires at least one evidence_regions entry; use inferred/hidden/conflicting when the reference does not visibly prove the feature."
            },
            "evidence_regions": {
                "type": "array",
                "maxItems": 32,
                "items": region,
                "description": "Required for observed features and must use exact sealed evidence IDs. Keep empty for inferred, hidden or conflicting details."
            },
            "affected_part_ids": {"type": "array", "minItems": 1, "maxItems": 256, "items": id(160)},
            "channels": {"type": "array", "minItems": 1, "maxItems": 7, "items": {"enum": ["geometry", "normal", "base_color", "roughness", "metallic", "emissive", "opacity"]}},
            "minimum_acceptance_views": {"type": "array", "minItems": 1, "maxItems": 16, "items": text(80)}
        }
    });
    let visual_feature_contract = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "contract_id", "request_sha256", "subject_profile_sha256", "requirements"],
        "properties": {
            "schema_version": {"const": "VisualFeatureContract@1"},
            "contract_id": id(160),
            "request_sha256": derived_lineage(),
            "subject_profile_sha256": derived_lineage(),
            "requirements": {"type": "array", "minItems": 1, "maxItems": 512, "items": requirement}
        }
    });

    let plan_part = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["part_id", "representation", "capability_id", "covered_feature_ids", "rationale"],
        "properties": {
            "part_id": id(160),
            "representation": {"enum": ["procedural", "deformable", "mesh_seed", "hybrid"]},
            "capability_id": id(160),
            "covered_feature_ids": {
                "type": "array",
                "maxItems": 512,
                "items": id(160),
                "description": "Use only feature IDs declared in VisualFeatureContract, and only when that feature's affected_part_ids contains this exact part_id. Empty is allowed."
            },
            "rationale": text(1200)
        }
    });
    let representation_plan = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "plan_id", "request_sha256", "subject_profile_sha256", "visual_feature_contract_sha256", "capability_manifest_sha256", "parts"],
        "properties": {
            "schema_version": {"const": "RepresentationPlan@1"},
            "plan_id": id(160),
            "request_sha256": derived_lineage(),
            "subject_profile_sha256": derived_lineage(),
            "visual_feature_contract_sha256": derived_lineage(),
            "capability_manifest_sha256": sha256(),
            "parts": {
                "type": "array",
                "minItems": 1,
                "maxItems": 256,
                "items": plan_part,
                "description": "Exactly one plan row per SubjectProfile part_id; never duplicate a part_id."
            }
        }
    });
    let limitation = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "code", "message", "affected_part_ids", "missing_capability_ids", "suggested_views", "retryable"],
        "properties": {
            "schema_version": {"const": "RepresentationLimitation@1"},
            "code": {"enum": ["needs_more_views", "representation_unavailable", "quality_limited", "provider_unavailable"]},
            "message": text(1200),
            "affected_part_ids": {"type": "array", "maxItems": 256, "items": id(160)},
            "missing_capability_ids": {"type": "array", "maxItems": 64, "items": id(160)},
            "suggested_views": {"type": "array", "maxItems": 16, "items": text(80)},
            "retryable": {"type": "boolean"}
        }
    });

    let base = |outcome: &str| {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["outcome", "schema_version", "request", "subject_profile", "visual_feature_contract", "representation_plan"],
            "properties": {
                "outcome": {"const": outcome},
                "schema_version": {"const": "UniversalAuthorOutcome@1"},
                "request": request.clone(),
                "subject_profile": subject_profile.clone(),
                "visual_feature_contract": visual_feature_contract.clone(),
                "representation_plan": representation_plan.clone()
            }
        })
    };
    // Do not advertise executable_payload as an arbitrary object.  The native
    // lowering boundary accepts either the reviewed generic geometry source
    // or the legacy robotic-arm authoring intent; an open object projection
    // made DeepSeek invent a `parts` wrapper that could never reach lowering.
    let geometry_material = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["material_id", "base_material_id"],
        "properties": {
            "material_id": {"type": "string", "pattern": "^mat_[a-z0-9_-]+$", "maxLength": 160},
            "base_material_id": {"enum": [
                "mat_graphite", "mat_aluminum", "mat_composite", "mat_dark_glass",
                "mat_clear_glass", "mat_emissive_blue", "mat_rubber", "mat_automotive_paint"
            ]}
        }
    });
    let geometry_budget = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "max_profiles", "max_section_sets", "max_nodes", "max_parts", "max_materials", "max_outputs", "max_operations", "triangle_budget"],
        "properties": {
            "schema_version": {"const": "GeometryProgramBudget@1"},
            "max_profiles": {"type": "integer", "minimum": 1, "maximum": 32},
            "max_section_sets": {"type": "integer", "minimum": 0, "maximum": 16},
            "max_nodes": {"type": "integer", "minimum": 1, "maximum": 256},
            "max_parts": {"type": "integer", "minimum": 1, "maximum": 128},
            "max_materials": {"type": "integer", "minimum": 1, "maximum": 64},
            "max_outputs": {"type": "integer", "minimum": 1, "maximum": 128},
            "max_operations": {"type": "integer", "minimum": 1, "maximum": 256},
            "triangle_budget": {"type": "integer", "minimum": 100, "maximum": 100000}
        }
    });
    let geometry_node_id = json!({
        "type": "string",
        "pattern": "^node_[a-z0-9_-]+$",
        "maxLength": 160
    });
    let point2 = json!({
        "type": "array",
        "minItems": 2,
        "maxItems": 2,
        "items": {"type": "number", "minimum": -1, "maximum": 1}
    });
    let point3 = json!({
        "type": "array",
        "minItems": 3,
        "maxItems": 3,
        "items": {"type": "number", "minimum": -100000, "maximum": 100000}
    });
    let rotation3 = json!({
        "type": "array",
        "minItems": 3,
        "maxItems": 3,
        "items": {"type": "number", "minimum": -4, "maximum": 4}
    });
    let positive = json!({
        "type": "number",
        "minimum": 0.000001,
        "maximum": 100000
    });
    let size3 = json!({
        "type": "array",
        "minItems": 3,
        "maxItems": 3,
        "items": positive.clone()
    });
    let scale2 = json!({
        "type": "array",
        "minItems": 2,
        "maxItems": 2,
        "items": positive.clone()
    });
    let axis = json!({"enum": ["x", "y", "z"]});
    let face_axis = json!({"enum": [
        "positive_x", "negative_x", "positive_y", "negative_y", "positive_z", "negative_z"
    ]});
    let profile = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["profile_id", "points", "resample_count"],
        "properties": {
            "profile_id": {"type": "string", "pattern": "^profile_[a-z0-9_-]+$", "maxLength": 160},
            "points": {"type": "array", "minItems": 3, "maxItems": 32, "items": point2},
            "resample_count": {"type": "integer", "minimum": 8, "maximum": 256}
        }
    });
    let section = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["section_id", "position", "profile_id", "scale", "twist_degrees", "cap_policy"],
        "properties": {
            "section_id": {"type": "string", "pattern": "^section_[a-z0-9_-]+$", "maxLength": 160},
            "position": {"type": "number", "minimum": -1, "maximum": 1},
            "profile_id": {"type": "string", "pattern": "^profile_[a-z0-9_-]+$", "maxLength": 160},
            "scale": {"type": "number", "minimum": 0.25, "maximum": 4},
            "twist_degrees": {"type": "number", "minimum": -45, "maximum": 45},
            "cap_policy": {"enum": ["none", "start", "end"]}
        }
    });
    let section_set = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["section_set_id", "main_axis", "sections"],
        "properties": {
            "section_set_id": {"type": "string", "pattern": "^sectionset_[a-z0-9_-]+$", "maxLength": 160},
            "main_axis": axis.clone(),
            "sections": {"type": "array", "minItems": 2, "maxItems": 12, "items": section}
        }
    });
    // Keep this projection compact, but expose every reviewed VP203 operation.
    // The native Rust validator remains authoritative for references, graph
    // fan-out, geometry semantics and budgets.
    let node_properties = |kind: &str, fields: Value| {
        let mut properties = fields.as_object().cloned().unwrap_or_default();
        properties.insert("kind".into(), json!({"const": kind}));
        properties.insert("node_id".into(), geometry_node_id.clone());
        json!({
            "type": "object",
            "required": fields.get("_required").cloned().unwrap_or_else(|| json!(["kind", "node_id"])),
            "properties": properties
        })
    };
    // `_required` is schema-construction metadata and must never be accepted
    // in a Provider payload. Remove it after constructing each branch.
    let branch = |kind: &str, required: &[&str], fields: Value| {
        let mut schema = node_properties(kind, fields);
        schema["required"] = json!(required);
        schema
    };
    let node_branch = |kind: &str, required: &[&str]| {
        json!({
            "type": "object",
            "required": required,
            "properties": {
                "kind": {"const": kind},
                "node_id": geometry_node_id.clone()
            }
        })
    };
    let mut node_branches = vec![
        node_branch("box", &["kind", "node_id", "size", "position"]),
        node_branch("cylinder", &["kind", "node_id", "radius", "height", "axis", "position"]),
        node_branch("capsule", &["kind", "node_id", "radius", "height", "axis", "position"]),
        node_branch("wedge", &["kind", "node_id", "size", "position"]),
        branch("extrude", &["kind", "node_id", "profile_id", "profile_scale", "height", "position", "cap_start", "cap_end"], json!({
            "profile_id": {"type": "string", "pattern": "^profile_[a-z0-9_-]+$"},
            "profile_scale": scale2.clone(), "height": positive.clone(), "position": point3.clone(),
            "rotation": rotation3.clone(), "cap_start": {"type": "boolean"}, "cap_end": {"type": "boolean"}
        })),
        branch("revolve", &["kind", "node_id", "profile_id", "profile_scale", "angle", "radial_segments", "position"], json!({
            "profile_id": {"type": "string", "pattern": "^profile_[a-z0-9_-]+$"},
            "profile_scale": scale2.clone(), "angle": {"type": "number", "minimum": 0.000001, "maximum": 6.283185307179586},
            "radial_segments": {"type": "integer", "minimum": 8, "maximum": 64}, "position": point3.clone(), "rotation": rotation3.clone()
        })),
        branch("loft", &["kind", "node_id", "section_set_id", "cross_section_scale", "axis_length", "position"], json!({
            "section_set_id": {"type": "string", "pattern": "^sectionset_[a-z0-9_-]+$"},
            "cross_section_scale": scale2.clone(), "axis_length": positive.clone(), "position": point3.clone(), "rotation": rotation3.clone()
        })),
        branch("sweep", &["kind", "node_id", "profile_id", "profile_scale", "path_points", "path_closed", "path_twist_degrees", "cap_start", "cap_end", "position"], json!({
            "profile_id": {"type": "string", "pattern": "^profile_[a-z0-9_-]+$"}, "profile_scale": scale2.clone(),
            "path_points": {"type": "array", "minItems": 2, "maxItems": 32, "items": point3.clone()},
            "path_closed": {"type": "boolean"}, "path_twist_degrees": {"type": "number", "minimum": -90, "maximum": 90},
            "cap_start": {"type": "boolean"}, "cap_end": {"type": "boolean"}, "position": point3.clone(), "rotation": rotation3.clone()
        })),
        branch("mirror", &["kind", "node_id", "input_node_id", "axis"], json!({"input_node_id": geometry_node_id.clone(), "axis": axis.clone()})),
        branch("array", &["kind", "node_id", "input_node_id", "axis", "count", "spacing"], json!({
            "input_node_id": geometry_node_id.clone(), "axis": axis.clone(), "count": {"type": "integer", "minimum": 2, "maximum": 64}, "spacing": positive.clone()
        })),
        branch("radial_array", &["kind", "node_id", "input_node_id", "axis", "count", "radius", "angle"], json!({
            "input_node_id": geometry_node_id.clone(), "axis": axis.clone(), "count": {"type": "integer", "minimum": 2, "maximum": 64},
            "radius": positive.clone(), "angle": {"type": "number", "minimum": 0.000001, "maximum": 6.283185307179586}
        })),
        branch("bevel_approx", &["kind", "node_id", "input_node_id", "radius", "segments"], json!({"input_node_id": geometry_node_id.clone(), "radius": positive.clone(), "segments": {"type": "integer", "minimum": 1, "maximum": 3}})),
        branch("surface_panel", &["kind", "node_id", "input_node_id", "size", "position", "axis"], json!({"input_node_id": geometry_node_id.clone(), "size": size3.clone(), "position": point3.clone(), "axis": face_axis.clone()})),
        branch("groove", &["kind", "node_id", "input_node_id", "face_size", "position", "axis", "depth"], json!({"input_node_id": geometry_node_id.clone(), "face_size": {"type": "array", "minItems": 2, "maxItems": 2, "items": positive.clone()}, "position": point3.clone(), "axis": face_axis.clone(), "depth": positive.clone()})),
        branch("shell", &["kind", "node_id", "input_node_id", "thickness"], json!({"input_node_id": geometry_node_id.clone(), "thickness": positive.clone()})),
        branch("lattice_deform", &["kind", "node_id", "input_node_id", "corner_offsets"], json!({"input_node_id": geometry_node_id.clone(), "corner_offsets": {"type": "array", "minItems": 8, "maxItems": 8, "items": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number", "minimum": -0.25, "maximum": 0.25}}}})),
        branch("local_mesh_patch", &["kind", "node_id", "input_node_id", "patch_center", "patch_radius", "patch_offset"], json!({"input_node_id": geometry_node_id.clone(), "patch_center": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number", "minimum": 0, "maximum": 1}}, "patch_radius": {"type": "number", "minimum": 0.05, "maximum": 0.4}, "patch_offset": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number", "minimum": -0.2, "maximum": 0.2}}})),
    ];
    node_branches.extend([
        node_branch("union", &["kind", "node_id", "input_node_ids"]),
        node_branch("subtract", &["kind", "node_id", "input_node_ids"]),
        node_branch("part", &["kind", "node_id", "input_node_id", "part_id", "role"]),
        node_branch("material_zone", &["kind", "node_id", "input_node_id", "zone_id", "material_id"]),
    ]);
    let geometry_node = json!({
        "type": "object",
        "required": ["kind", "node_id"],
        "properties": {
            "kind": {"enum": [
                "box", "cylinder", "capsule", "wedge", "extrude", "revolve", "loft", "sweep",
                "mirror", "array", "radial_array", "bevel_approx", "surface_panel", "groove",
                "shell", "lattice_deform", "local_mesh_patch", "union", "subtract", "part", "material_zone"
            ]},
            "node_id": geometry_node_id
        },
        "anyOf": node_branches,
            "description": "Use exact kind-specific fields and compose freely within the reviewed budget. Profiles use profile_id=profile_, points=[[x,y],...], resample_count; section_sets use sectionset_, main_axis and 2-12 sections. box/wedge use size=[x,y,z],position=[x,y,z]; cylinder/capsule use radius,height,axis='x'|'y'|'z',position; extrude uses profile_id,profile_scale,height,position,cap_start,cap_end; revolve uses profile_id,profile_scale,angle,radial_segments,position; loft uses section_set_id,cross_section_scale,axis_length,position; sweep uses profile_id,profile_scale,path_points,path_closed,path_twist_degrees,position,cap_start,cap_end. mirror uses input_node_id,axis; array uses input_node_id,axis,count,spacing; radial_array uses input_node_id,axis,count,radius,angle. bevel_approx uses input_node_id,radius,segments; surface_panel uses input_node_id,size=[x,y,z],position=[x,y,z],axis face; groove uses input_node_id,face_size=[x,y],position,axis face,depth; shell uses input_node_id,thickness; lattice_deform uses input_node_id and exactly eight corner_offsets; local_mesh_patch uses input_node_id, normalized patch_center,patch_radius,patch_offset. part uses input_node_id,part_id,role; material_zone uses input_node_id,zone_id,material_id; union/subtract use 2-8 input_node_ids. All references use node_ IDs and every output graph must be disjoint."
    });
    let geometry_output = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["output_id", "node_id"],
        "properties": {
            "output_id": {"type": "string", "pattern": "^output_[a-z0-9_-]+$", "maxLength": 160},
            "node_id": {"type": "string", "pattern": "^node_[a-z0-9_-]+$", "maxLength": 160}
        }
    });
    let geometry_payload = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "program_id", "domain", "units", "seed", "materials", "profiles", "section_sets", "nodes", "outputs", "budgets"],
        "properties": {
            "schema_version": {"const": "ForgeVisualGeometryProgram@2"},
            "program_id": {"type": "string", "pattern": "^visual_[a-z0-9_-]+$", "maxLength": 96},
            "domain": text(96),
            "units": {"const": "millimeter"},
            "seed": {"type": "integer", "minimum": 0, "maximum": 2147483647},
            "materials": {"type": "array", "minItems": 1, "maxItems": 64, "items": geometry_material},
            "profiles": {"type": "array", "maxItems": 32, "items": profile},
            "section_sets": {"type": "array", "maxItems": 16, "items": section_set},
            "nodes": {"type": "array", "minItems": 1, "maxItems": 256, "items": geometry_node},
            "outputs": {"type": "array", "minItems": 1, "maxItems": 128, "items": geometry_output},
            "budgets": geometry_budget
        }
    });
    let arm_authoring_payload = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "authoring_id", "title", "arm_design_intent"],
        "properties": {
            "schema_version": {"const": "ForgeVisualAuthoringIntent@1"},
            "authoring_id": text(96),
            "title": text(160),
            "arm_design_intent": {"type": "object"}
        }
    });
    let mut executable = base("executable");
    executable["required"] = json!(["outcome", "schema_version", "request", "subject_profile", "visual_feature_contract", "representation_plan", "executable_payload"]);
    executable["properties"]["executable_payload"] = json!({"anyOf": [geometry_payload, arm_authoring_payload]});
    let mut limited = base("limitation");
    limited["required"] = json!(["outcome", "schema_version", "request", "subject_profile", "visual_feature_contract", "representation_plan", "limitation"]);
    limited["properties"]["limitation"] = limitation;
    let clarification = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["outcome", "schema_version", "request", "reason", "questions"],
        "properties": {
            "outcome": {"const": "clarification_required"},
            "schema_version": {"const": "UniversalAuthorOutcome@1"},
            "request": request,
            "reason": text(1200),
            "questions": {"type": "array", "minItems": 1, "maxItems": 3, "items": text(320)}
        }
    });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["outcome"],
        "properties": {
            // Evidence dispositions belong to the legacy visual-program
            // author tool.  Advertising them on the universal author
            // projection caused the model to place them inside the nested
            // outcome object, where every UniversalAuthorOutcome variant
            // correctly rejects unknown fields.
            "outcome": {"anyOf": [executable, limited, clarification]}
        }
    })
}

fn concept_schema_document(file_name: &str) -> Value {
    let source = match file_name {
        "common.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/common.schema.json"
        )),
        "universal-author-outcome.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/universal-author-outcome.schema.json"
        )),
        "universal-author-request.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/universal-author-request.schema.json"
        )),
        "subject-profile.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/subject-profile.schema.json"
        )),
        "visual-feature-contract.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/visual-feature-contract.schema.json"
        )),
        "representation-plan.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/representation-plan.schema.json"
        )),
        "representation-limitation.schema.json" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/representation-limitation.schema.json"
        )),
        _ => panic!("unsupported universal author schema document: {file_name}"),
    };
    serde_json::from_str(source).expect("checked-in concept schema must be valid JSON")
}

fn inline_concept_schema(node: &Value, scope: Option<&Value>) -> Value {
    let Some(object) = node.as_object() else {
        return node.clone();
    };
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let target = if let Some(name) = reference.strip_prefix("#/$defs/") {
            scope
                .and_then(|root| root.get("$defs"))
                .and_then(Value::as_object)
                .and_then(|definitions| definitions.get(name))
                .unwrap_or_else(|| panic!("missing local concept schema definition: {reference}"))
                .clone()
        } else {
            let (file_name, fragment) = reference
                .split_once('#')
                .unwrap_or((reference, ""));
            let document = concept_schema_document(file_name);
            if fragment.is_empty() {
                document.clone()
            } else if let Some(name) = fragment.strip_prefix("/$defs/") {
                document
                    .get("$defs")
                    .and_then(Value::as_object)
                    .and_then(|definitions| definitions.get(name))
                    .unwrap_or_else(|| panic!("missing external concept schema definition: {reference}"))
                    .clone()
            } else {
                panic!("unsupported concept schema reference: {reference}");
            }
        };
        let target_scope = if reference.starts_with("#/") {
            scope
        } else {
            let (file_name, _) = reference.split_once('#').unwrap_or((reference, ""));
            let document = concept_schema_document(file_name);
            // The external document owns the local `$defs` namespace used by
            // its inlined target and all descendants.
            return inline_concept_schema(&target, Some(&document));
        };
        return inline_concept_schema(&target, target_scope);
    }

    let mut result = Map::new();
    for (key, value) in object {
        // These annotations/negative constraints are useful in the public
        // documents but are intentionally outside the small runtime schema
        // validator used for Product Tool envelopes. Rust's typed outcome
        // validation remains authoritative after the call.
        if matches!(key.as_str(), "$schema" | "$id" | "title" | "format" | "not") {
            continue;
        }
        if key == "oneOf" {
            result.insert(
                "anyOf".into(),
                Value::Array(
                    value
                        .as_array()
                        .expect("concept schema oneOf must be an array")
                        .iter()
                        .map(|branch| inline_concept_schema(branch, scope))
                        .collect(),
                ),
            );
        } else {
            result.insert(key.clone(), inline_concept_schema(value, scope));
        }
    }
    Value::Object(result)
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
        && !object.contains_key("const")
    {
        return Err(format!(
            "Code-owned schema must declare type, enum, anyOf, const, or a local ref (keys: {}).",
            object.keys().cloned().collect::<Vec<_>>().join(",")
        ));
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
    validate_json_schema_inner_at(schema, value, schema, "$")
}

fn validate_json_schema_inner_at(
    schema: &Value,
    value: &Value,
    root: &Value,
    path: &str,
) -> Result<(), String> {
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
        return validate_json_schema_inner_at(target, value, root, path);
    }
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        let mut branch_errors = Vec::new();
        if !branches.iter().enumerate().any(|(index, branch)| {
            match validate_json_schema_inner_at(branch, value, root, path) {
                Ok(()) => true,
                Err(error) => {
                    branch_errors.push(format!("{index}: {error}"));
                    false
                }
            }
        }) {
            return Err(format!(
                "Value does not match any code-owned anyOf branch at {path}: {}.",
                branch_errors.join(" | ")
            ));
        }
    }
    if let Some(expected) = schema.get("const") {
        if expected != value {
            return Err(format!("Value does not match the code-owned constant at {path}."));
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
                "Value at {path} must have one of the code-owned JSON types: {}.",
                expected_types.join(", ")
            ));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return Err(format!(
                "Value at {path} is outside the code-owned enum {}.",
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
            return Err(format!("String at {path} violates code-owned length bounds."));
        }
        if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
            if !matches_known_pattern(pattern, text) {
                return Err(format!(
                    "String at {path} violates the code-owned stable pattern {pattern}."
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
            return Err(format!("Number at {path} violates code-owned bounds."));
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
                    return Err(format!("Required property {key} is missing at {path}."));
                }
            }
        }
        if schema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
            for key in object.keys() {
                if !properties.contains_key(key) {
                    return Err(format!("Property {key} is not allowed at {path}."));
                }
            }
        }
        for (key, child) in object {
            if let Some(child_schema) = properties.get(key) {
                let child_path = format!("{path}.{key}");
                validate_json_schema_inner_at(child_schema, child, root, &child_path)?;
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
            let minimum = schema.get("minItems").and_then(Value::as_u64);
            let maximum = schema.get("maxItems").and_then(Value::as_u64);
            return Err(format!(
                "Array at {path} violates code-owned item bounds (actual={}, min={minimum:?}, max={maximum:?}).",
                array.len()
            ));
        }
        if let Some(items) = schema.get("items") {
            for (index, child) in array.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                validate_json_schema_inner_at(items, child, root, &child_path)?;
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
    if pattern == "^[a-f0-9]{64}$" {
        return value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    }
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
    fn universal_author_provider_schema_exposes_the_three_typed_contracts() {
        let registry = ProductToolRegistry::default();
        let provider = registry
            .provider_definitions()
            .into_iter()
            .find(|definition| definition.name == "author_universal_asset")
            .unwrap();
        let outcome = provider
            .input_schema
            .pointer("/properties/outcome")
            .expect("universal author must expose its outcome schema");
        assert_eq!(outcome.get("anyOf").and_then(Value::as_array).map(Vec::len), Some(3));
        for path in [
            "/properties/outcome/anyOf/0/properties/request",
            "/properties/outcome/anyOf/0/properties/subject_profile",
            "/properties/outcome/anyOf/0/properties/visual_feature_contract",
            "/properties/outcome/anyOf/0/properties/representation_plan",
        ] {
            assert!(provider.input_schema.pointer(path).is_some(), "missing {path}");
        }
        let serialized = serde_json::to_string(&provider.input_schema).unwrap();
        assert!(!serialized.contains("universal-author-request.schema.json"));
        assert!(!serialized.contains("\"oneOf\""));
        assert!(provider
            .input_schema
            .pointer("/properties/legacy_evidence_dispositions")
            .is_none());
        assert!(serialized.contains("SubjectProfile@1"));
        assert!(serialized.contains("VisualFeatureContract@1"));
        assert!(serialized.contains("RepresentationPlan@1"));
        assert!(serialized.len() < 40_000, "provider schema grew too large: {} bytes", serialized.len());
    }

    #[test]
    fn universal_author_provider_description_stays_inside_deepseek_bound() {
        let registry = ProductToolRegistry::default();
        let provider = registry
            .provider_definitions()
            .into_iter()
            .find(|definition| definition.name == "author_universal_asset")
            .unwrap();
        assert!(provider.description.len() <= 500);
        assert!(provider.description.contains("generic_visual_exterior"));
        assert!(provider.description.contains("macro, meso, micro"));
    }

    #[test]
    fn universal_author_provider_schema_exposes_profile_driven_geometry_operations() {
        let registry = ProductToolRegistry::default();
        let provider = registry
            .provider_definitions()
            .into_iter()
            .find(|definition| definition.name == "author_universal_asset")
            .unwrap();
        let geometry = provider
            .input_schema
            .pointer("/properties/outcome/anyOf/0/properties/executable_payload/anyOf/0")
            .expect("executable universal author must expose geometry payload");

        for kind in [
            "extrude",
            "revolve",
            "loft",
            "sweep",
            "mirror",
            "array",
            "radial_array",
            "lattice_deform",
            "local_mesh_patch",
        ] {
            assert!(
                geometry.to_string().contains(&format!("\"const\":\"{kind}\"")),
                "provider schema must advertise the {kind} operation"
            );
        }
        assert!(geometry
            .pointer("/properties/profiles/items/properties/profile_id")
            .is_some());
        assert!(geometry
            .pointer("/properties/section_sets/items/properties/section_set_id")
            .is_some());
    }

    #[test]
    fn universal_author_transport_schema_defers_derived_lineage_hashes_to_rust() {
        let registry = ProductToolRegistry::default();
        let schema = &registry.definition("author_universal_asset").unwrap().input_schema;
        for path in [
            "/properties/outcome/anyOf/0/properties/subject_profile/properties/request_sha256",
            "/properties/outcome/anyOf/0/properties/visual_feature_contract/properties/request_sha256",
            "/properties/outcome/anyOf/0/properties/representation_plan/properties/request_sha256",
            "/properties/outcome/anyOf/0/properties/representation_plan/properties/capability_manifest_sha256",
        ] {
            let field = schema.pointer(path).expect("lineage field must remain in schema");
            assert!(
                field.get("pattern").is_none(),
                "transport schema must not require Provider to reproduce {path}"
            );
            assert_eq!(field.get("type").and_then(Value::as_str), Some("string"));
            assert_eq!(field.get("minLength").and_then(Value::as_u64), Some(1));
        }
    }

    #[test]
    fn provider_schema_validator_accepts_only_lowercase_sha256_values() {
        let valid = "0123456789abcdef".repeat(4);
        let invalid_length = valid[..63].to_owned();
        let invalid_upper = format!("{}A", &valid[..63]);
        let invalid_non_hex = format!("{}G", &valid[..63]);
        assert!(matches_known_pattern("^[a-f0-9]{64}$", &valid));
        assert!(!matches_known_pattern("^[a-f0-9]{64}$", &invalid_length));
        assert!(!matches_known_pattern("^[a-f0-9]{64}$", &invalid_non_hex));
        assert!(!matches_known_pattern("^[a-f0-9]{64}$", &invalid_upper));
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
                "010ab5eafdce651f3f06b841cb980365dfbd141607ca990cc253a2d7a8fcfa8d",
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
