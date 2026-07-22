//! Code-owned Product Tool registry and restricted executor boundary.
//!
//! K002 can invoke only these thirteen A004-compatible planning/candidate
//! tools. Permanent Product writes remain outside the registry and require an
//! explicit approval path owned by product core.

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

pub const MAX_PRODUCT_TOOL_CALLS: u32 = 12;

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
        self.definitions()
            .map(|definition| ProviderToolDefinition {
                name: definition.name.clone(),
                description: definition.description.clone(),
                // The registry schema remains the authoritative Rust-side
                // validation contract.  The Provider-facing schema is a
                // deliberately smaller projection so a model does not have
                // to reproduce internal `$defs` for assembly deltas and
                // legacy compatibility fields while planning a new asset.
                // `build_execution_request` still validates the exact full
                // registry schema before any Product Tool runs.
                input_schema: if definition.name == "plan_complete_concept" {
                    compact_plan_provider_schema()
                } else {
                    definition.input_schema.clone()
                },
            })
            .collect()
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
        let definition = self.definition(&call.name)?;
        if definition.approval_policy == ProductToolApprovalPolicy::UserConfirmationRequired {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_APPROVAL_PATH_FORBIDDEN",
                ProductToolRegistryErrorKind::ApprovalForbidden,
                "Permanent-write tools cannot run inside the K002 Action Loop.",
            ));
        }
        let arguments = value_to_btree_object(&call.arguments).ok_or_else(|| {
            ProductToolRegistryError::new(
                "PRODUCT_TOOL_ARGUMENTS_NOT_OBJECT",
                ProductToolRegistryErrorKind::InvalidArguments,
                "Product Tool arguments must be a JSON object.",
            )
        })?;
        validate_json_schema(
            &definition.input_schema,
            &Value::Object(arguments.clone().into_iter().collect::<Map<_, _>>()),
        )
        .map_err(|message| {
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
        if self.definitions.len() != 13 {
            return Err(ProductToolRegistryError::new(
                "PRODUCT_TOOL_REGISTRY_INCOMPLETE",
                ProductToolRegistryErrorKind::InvalidSchema,
                "ForgeCAD Product Tool registry must contain exactly thirteen tools.",
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

fn compact_plan_provider_schema() -> Value {
    let delta_transform = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["position", "rotation", "scale"],
        "properties": {
            "position": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number"}},
            "rotation": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number"}},
            "scale": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number"}}
        }
    });
    let delta_pose = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["rotation", "translation"],
        "properties": {
            "rotation": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number"}},
            "translation": {"type": "array", "minItems": 3, "maxItems": 3, "items": {"type": "number"}}
        }
    });
    let delta_add = json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "op", "operation_id", "new_part_id", "parent_part_id", "parent_connector_id",
            "child_connector_id", "recipe_id", "slot_id", "transform"
        ],
        "properties": {
            "op": {"const": "add_reviewed_recipe"},
            "operation_id": {"type": "string", "pattern": "^[A-Za-z0-9_:-]+$"},
            "new_part_id": {"type": "string", "pattern": "^part_[A-Za-z0-9_:-]+$"},
            "parent_part_id": {"type": "string", "minLength": 1},
            "parent_connector_id": {"type": "string", "minLength": 1},
            "child_connector_id": {"type": "string", "minLength": 1},
            "recipe_id": {"type": "string", "enum": [
                "recipe_c106_arm_turntable", "recipe_c106_arm_joint_housing", "recipe_c106_arm_link_armor",
                "recipe_c106_arm_cable_harness", "recipe_c106_arm_gripper", "recipe_c106_arm_surface_trim",
                "recipe_c110c_arm_sensor_pod", "recipe_c110d_arm_actuator_cover", "recipe_c110d_arm_cable_guide",
                "recipe_c110d_arm_wrist_tool_mount", "recipe_c110g_parallel_rail", "recipe_c110g_parallel_carriage",
                "recipe_c110g_parallel_link", "recipe_c110g_parallel_end_effector"
            ]},
            "slot_id": {"type": "string", "enum": [
                "slot_arm_sensor_pod", "slot_arm_guard_rail", "slot_arm_tool_changer", "slot_arm_camera_boom",
                "slot_c110g_parallel_rail", "slot_c110g_parallel_carriage", "slot_c110g_parallel_link", "slot_c110g_parallel_tool"
            ]},
            "transform": delta_transform
        }
    });
    let delta_replace = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "operation_id", "part_id", "recipe_id"],
        "properties": {
            "op": {"const": "replace_reviewed_recipe"},
            "operation_id": {"type": "string", "pattern": "^[A-Za-z0-9_:-]+$"},
            "part_id": {"type": "string", "minLength": 1},
            "recipe_id": {"type": "string", "enum": [
                "recipe_c106_arm_turntable", "recipe_c106_arm_joint_housing", "recipe_c106_arm_link_armor",
                "recipe_c106_arm_cable_harness", "recipe_c106_arm_gripper", "recipe_c106_arm_surface_trim",
                "recipe_c110c_arm_sensor_pod", "recipe_c110d_arm_actuator_cover", "recipe_c110d_arm_cable_guide",
                "recipe_c110d_arm_wrist_tool_mount", "recipe_c110g_parallel_rail", "recipe_c110g_parallel_carriage",
                "recipe_c110g_parallel_link", "recipe_c110g_parallel_end_effector"
            ]}
        }
    });
    let delta_transform_part = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "operation_id", "part_id", "transform"],
        "properties": {
            "op": {"const": "set_part_transform"},
            "operation_id": {"type": "string", "pattern": "^[A-Za-z0-9_:-]+$"},
            "part_id": {"type": "string", "minLength": 1},
            "transform": delta_transform
        }
    });
    let delta_pose_part = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "operation_id", "part_id", "joint_id", "pose"],
        "properties": {
            "op": {"const": "set_joint_pose"},
            "operation_id": {"type": "string", "pattern": "^[A-Za-z0-9_:-]+$"},
            "part_id": {"type": "string", "minLength": 1},
            "joint_id": {"type": "string", "minLength": 1},
            "pose": delta_pose
        }
    });
    let delta_snap = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["op", "operation_id", "part_id", "target_part_id", "target_connector_id", "connector_id"],
        "properties": {
            "op": {"const": "snap_part_to_connector"},
            "operation_id": {"type": "string", "pattern": "^[A-Za-z0-9_:-]+$"},
            "part_id": {"type": "string", "minLength": 1},
            "target_part_id": {"type": "string", "minLength": 1},
            "target_connector_id": {"type": "string", "minLength": 1},
            "connector_id": {"type": "string", "minLength": 1}
        }
    });
    let assembly_delta = json!({
        "anyOf": [
            {"type": "null"},
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["schema_version", "domain_pack_id", "base_asset_version_id", "summary", "operations", "visual_only"],
                "properties": {
                    "schema_version": {"const": "AssemblyDeltaProgram@1"},
                    "domain_pack_id": {"const": "pack_robotic_arm_concept"},
                    "base_asset_version_id": {"type": "string", "pattern": "^assetver_[A-Za-z0-9_:-]+$"},
                    "summary": {"type": "string", "minLength": 1, "maxLength": 2000},
                    "operations": {
                        "type": "array", "minItems": 1, "maxItems": 8,
                        "items": {"anyOf": [delta_add, delta_replace, delta_transform_part, delta_pose_part, delta_snap]}
                    },
                    "visual_only": {"const": true}
                }
            }
        ]
    });
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
                    "assembly_delta": assembly_delta,
                    "shape_program_ready": {"type": "boolean"}
                }
            }
        }
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
        || fixture.tools.len() != 13
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
            return Err("Value is outside the code-owned enum.".into());
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
                return Err("String violates a code-owned stable pattern.".into());
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
    let prefix = match pattern {
        "^direction_[a-z0-9_\\-]+$" => "direction_",
        "^plan_[a-z0-9_\\-]+$" => "plan_",
        "^pack_[a-z0-9_\\-]+$" => "pack_",
        "^attempt_[a-z0-9_\\-]+$" => "attempt_",
        "^gate_[a-z0-9_\\-]+$" => "gate_",
        _ => return false,
    };
    value.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_exactly_thirteen_code_owned_non_permanent_tools() {
        let registry = ProductToolRegistry::default();
        assert_eq!(registry.definitions().count(), 13);
        assert!(registry.definitions().all(|definition| {
            definition.approval_policy != ProductToolApprovalPolicy::UserConfirmationRequired
        }));
        assert!(registry.definition("compile_readback_candidate").is_ok());
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
        // The delta projection is intentionally more explicit than the old
        // opaque `{}` placeholder, but it must remain smaller than the full
        // registry contract and below a bounded Provider request size.
        assert!(provider_bytes < 20_000);
        assert!(full_bytes > 10_000);
        assert!(provider_bytes < full_bytes);
        assert_ne!(provider.input_schema, full.input_schema);
        assert!(provider
            .input_schema
            .pointer("/properties/plan/properties/arm_design_intent")
            .is_some());
        assert_eq!(
            provider
                .input_schema
                .pointer("/properties/plan/properties/assembly_delta/anyOf/1/properties/operations/items/anyOf/0/properties/recipe_id/enum/0")
                .and_then(Value::as_str),
            Some("recipe_c106_arm_turntable")
        );
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
                "486efc390e8e51a2147cf6e189c4bf2424d1193eaa10b488a2773afd6820dd53",
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
