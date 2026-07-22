use serde_json::{json, Map, Value};

use crate::{semantic_sha256, CoreError, CoreResult};

use super::{
    ids::stable_id,
    transform::{
        connector_frame, euler_xyz_from_rotation, inverse_rigid, multiply, rigid_rotation,
        rotation_matrix_from_euler, transform_matrix, transform_point, Matrix4,
    },
    ComponentRecipeInstanceProvenance, ComponentRecipeRef, EditableComponentRecipe,
    ExpandedComponentCandidate, ExpandedComponentInstance, RecipeExpansionPolicy,
    RecipeInstantiationRequest, RecipeRegistry, RecipeValidator,
};

/// Deterministic, read-only Recipe compiler.  It emits a transient candidate
/// only; its caller remains responsible for C104 locks plus preview→confirm.
pub struct RecipeExpander;

impl RecipeExpander {
    pub fn expand(
        registry: &RecipeRegistry,
        request: &RecipeInstantiationRequest,
        policy: &RecipeExpansionPolicy,
    ) -> CoreResult<ExpandedComponentCandidate> {
        Self::expand_with_root_transform(registry, request, policy, None)
    }

    /// Expands one reviewed standalone attachment in a caller-owned frame.
    /// The frame is baked into the candidate's ShapeProgram and AssemblyGraph
    /// before its candidate hash is calculated; the worker therefore never
    /// receives a mutable post-hash placement instruction.
    pub fn expand_with_root_transform(
        registry: &RecipeRegistry,
        request: &RecipeInstantiationRequest,
        policy: &RecipeExpansionPolicy,
        root_transform: Option<&super::RecipeTransform>,
    ) -> CoreResult<ExpandedComponentCandidate> {
        RecipeValidator::validate_registry(registry)?;
        RecipeValidator::validate_request(registry, request, policy)?;

        let request_sha256 = request_identity_sha256(request)?;
        let root = registry
            .recipe(&request.recipe.recipe_id)
            .ok_or_else(|| CoreError::not_found("reviewed Component Recipe"))?;
        let mut state = ExpansionState::default();
        expand_recipe(
            registry,
            request,
            policy,
            root,
            "root".into(),
            None,
            None,
            root_transform
                .map(transform_matrix)
                .transpose()?
                .unwrap_or(transform_matrix(&root.root_local_transform)?),
            &request_sha256,
            0,
            &mut state,
        )?;
        if state.instances.is_empty() {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_EXPANSION_EMPTY",
                "A reviewed Component Recipe must expand at least one instance.",
            ));
        }
        let root_instance = &state.instances[0];
        let expanded_shape_program = build_shape_program(&state, &request_sha256)?;
        let expanded_assembly_graph = build_assembly_graph(&state, &request_sha256)?;
        let candidate_id = stable_id(
            "recipecandidate",
            &json!({
                "request_sha256": request_sha256,
                "registry_sha256": registry.registry_sha256(),
                "instances": state.provenance,
                "shape_program": expanded_shape_program,
            }),
        )?;
        let mut candidate = ExpandedComponentCandidate {
            schema_version: "ComponentRecipeCandidate@1".into(),
            candidate_id,
            request_id: request.request_id.clone(),
            context_mode: request.context_mode.clone(),
            project_id: request.project_id.clone(),
            base_asset_version_id: request.base_asset_version_id.clone(),
            snapshot_revision: request.snapshot_revision,
            recipe: request.recipe.clone(),
            target_part_id: request.target_part_id.clone(),
            instance_path: root_instance.instance_path.clone(),
            changeset_id: None,
            expanded_shape_program,
            expanded_assembly_graph,
            component_recipe_instances: state.provenance,
            registry_sha256: registry.registry_sha256().into(),
            candidate_sha256: String::new(),
            status: "expanded".into(),
            quality_profile: "production_concept".into(),
            non_functional_only: true,
            instances: state.instances,
        };
        candidate.candidate_sha256 = candidate_hash(&candidate)?;
        Ok(candidate)
    }

    /// The explicit hash scope used by ComponentRecipeCandidate@1: canonical
    /// candidate JSON with `candidate_sha256` blank and transient diagnostics
    /// omitted.  Golden checks use this rather than trusting a fixture field.
    pub fn candidate_sha256(candidate: &ExpandedComponentCandidate) -> CoreResult<String> {
        candidate_hash(candidate)
    }

    /// Re-identifies an already-expanded candidate after Rust Core has baked
    /// an active-asset placement anchor into the reviewed geometry.  Active
    /// candidates are deliberately contextual (their target Part, parent
    /// anchor and translation belong to an immutable base Version), so the
    /// pre-placement candidate id/hash must never be reused for the placed
    /// ShapeProgram that is sent to the restricted geometry executor.
    pub fn reidentify(
        candidate: &mut ExpandedComponentCandidate,
        request: &RecipeInstantiationRequest,
    ) -> CoreResult<()> {
        let request_sha256 = request_identity_sha256(request)?;
        candidate.candidate_id = stable_id(
            "recipecandidate",
            &json!({
                "request_sha256": request_sha256,
                "registry_sha256": candidate.registry_sha256,
                "instances": candidate.component_recipe_instances,
                "shape_program": candidate.expanded_shape_program,
            }),
        )?;
        candidate.candidate_sha256 = candidate_hash(candidate)?;
        Ok(())
    }
}

/// Registry identity is already sealed independently in every candidate and
/// instance provenance. Excluding this duplicated transport guard preserves
/// the frozen C105 candidate/request hashes while repository selection still
/// verifies the exact C105/C106 registry before expansion.
fn request_identity_sha256(request: &RecipeInstantiationRequest) -> CoreResult<String> {
    let mut value = serde_json::to_value(request).map_err(|error| {
        CoreError::invalid_data(
            "COMPONENT_RECIPE_REQUEST_INVALID",
            format!("Component Recipe request cannot be serialized: {error}"),
        )
    })?;
    value
        .as_object_mut()
        .expect("RecipeInstantiationRequest serializes as an object")
        .remove("recipe_registry_sha256");
    semantic_sha256(&value)
}

#[derive(Default)]
struct ExpansionState {
    instances: Vec<ExpandedComponentInstance>,
    provenance: Vec<ComponentRecipeInstanceProvenance>,
    operation_count: u32,
    profile_count: u32,
    section_count: u32,
    zone_count: u32,
    triangle_count: u64,
}

#[allow(clippy::too_many_arguments)]
fn expand_recipe(
    registry: &RecipeRegistry,
    request: &RecipeInstantiationRequest,
    policy: &RecipeExpansionPolicy,
    recipe: &EditableComponentRecipe,
    instance_path: String,
    parent_instance_id: Option<String>,
    parent_slot_id: Option<String>,
    world_transform: Matrix4,
    request_sha256: &str,
    depth: u32,
    state: &mut ExpansionState,
) -> CoreResult<()> {
    if depth > policy.max_depth {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_DEPTH_EXCEEDED",
            "Recipe child depth exceeds the C105 policy.",
        ));
    }
    charge_budget(policy, recipe, state)?;
    require_supported_template_transform(recipe, world_transform)?;
    let recipe_ref = recipe_ref(recipe)?;
    let instance_id = stable_id(
        "recipeinst",
        &json!({
            "request_sha256": request_sha256,
            "recipe": recipe_ref,
            "instance_path": instance_path,
        }),
    )?;
    let provenance = ComponentRecipeInstanceProvenance {
        schema_version: "ComponentRecipeInstanceProvenance@1".into(),
        instance_id: instance_id.clone(),
        instance_path: instance_path.clone(),
        recipe: recipe_ref,
        registry_sha256: registry.registry_sha256().into(),
        policy_version: "ComponentRecipePolicy@1".into(),
        domain_pack_id: request.domain_pack_id.clone(),
        parent_instance_id,
        parent_slot_id,
        source: recipe.source.clone(),
        license: recipe.license.clone(),
        review_state: recipe.review_state.clone(),
        quality_status: recipe.quality_status.clone(),
        non_functional_only: recipe.non_functional_only,
    };
    state.instances.push(ExpandedComponentInstance {
        instance_id,
        instance_path,
        component_role: recipe.component_role.clone(),
        world_transform,
        recipe: recipe.clone(),
        provenance: provenance.clone(),
    });
    state.provenance.push(provenance);

    let parent = state.instances.last().expect("just pushed").clone();
    let mut slots = recipe.child_slots.iter().collect::<Vec<_>>();
    slots.sort_by(|left, right| left.slot_id.cmp(&right.slot_id));
    for slot in slots {
        let explicitly_bound = request
            .slot_bindings
            .iter()
            .any(|binding| binding.slot_id == slot.slot_id);
        if !slot.required && !explicitly_bound {
            continue;
        }
        let child = registry
            .recipe(&slot.child_recipe_id)
            .ok_or_else(|| CoreError::not_found("reviewed child Component Recipe"))?;
        let connector = recipe
            .connectors
            .iter()
            .find(|connector| connector.connector_id == slot.parent_connector_id)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "COMPONENT_RECIPE_CONNECTOR_MISSING",
                    "Child slot parent connector is absent.",
                )
            })?;
        let connector_world = multiply(
            parent.world_transform,
            connector_frame(connector.position, connector.normal, connector.up)?,
        );
        let child_connector = child
            .connectors
            .iter()
            .find(|connector| connector.connector_id == slot.child_connector_id)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "COMPONENT_RECIPE_CONNECTOR_MISSING",
                    "Child slot connector is absent.",
                )
            })?;
        let child_world = multiply(
            multiply(
                multiply(
                    connector_world,
                    transform_matrix(&slot.parent_local_transform)?,
                ),
                inverse_rigid(connector_frame(
                    child_connector.position,
                    child_connector.normal,
                    child_connector.up,
                )?),
            ),
            transform_matrix(&child.root_local_transform)?,
        );
        for index in 0..slot.count {
            expand_recipe(
                registry,
                request,
                policy,
                child,
                format!("{}/{}[{index}]", parent.instance_path, slot.slot_id),
                Some(parent.instance_id.clone()),
                Some(slot.slot_id.clone()),
                child_world,
                request_sha256,
                depth + 1,
                state,
            )?;
        }
    }
    Ok(())
}

fn charge_budget(
    policy: &RecipeExpansionPolicy,
    recipe: &EditableComponentRecipe,
    state: &mut ExpansionState,
) -> CoreResult<()> {
    state.operation_count += recipe.shape_program_template["operations"]
        .as_array()
        .map_or(0, Vec::len) as u32;
    state.profile_count += recipe.shape_program_template["profile_inputs"]
        .as_array()
        .map_or(0, Vec::len) as u32;
    state.section_count += recipe.section_sets.len() as u32;
    state.zone_count += recipe.material_zones.len() as u32;
    state.triangle_count = state
        .triangle_count
        .saturating_add(recipe.triangle_estimate);
    if state.instances.len() as u32 >= policy.max_instances
        || state.operation_count > policy.max_operations
        || state.profile_count > policy.max_profiles
        || state.section_count > policy.max_sections
        || state.zone_count > policy.max_material_zones
        || state.triangle_count > policy.max_triangles
    {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_BUDGET_EXCEEDED",
            "Recipe expansion exceeds the bounded C105 policy.",
        ));
    }
    Ok(())
}

fn require_supported_template_transform(
    recipe: &EditableComponentRecipe,
    matrix: Matrix4,
) -> CoreResult<()> {
    let rotation = rigid_rotation(matrix)?;
    let has_rotation = (0..3).any(|row| {
        (0..3).any(|column| {
            (rotation[row][column] - if row == column { 1.0 } else { 0.0 }).abs() > 1e-9
        })
    });
    if has_rotation
        && recipe.shape_program_template["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|operation| {
                matches!(
                    operation.get("op").and_then(Value::as_str),
                    Some("mirror" | "array" | "radial_array" | "union" | "subtract")
                )
            })
    {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_TEMPLATE_TRANSFORM_OPERATION_UNSUPPORTED",
            "Static Recipe rotation is rejected for mirror, array and CSG templates until their local-frame propagation is independently verified.",
        ));
    }
    Ok(())
}

fn recipe_ref(recipe: &EditableComponentRecipe) -> CoreResult<ComponentRecipeRef> {
    Ok(ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: recipe.recipe_id.clone(),
        version: recipe.version,
        recipe_sha256: RecipeValidator::recipe_sha256(recipe)?,
    })
}

fn build_shape_program(state: &ExpansionState, request_sha256: &str) -> CoreResult<Value> {
    let mut profile_inputs = Vec::new();
    let mut operations = Vec::new();
    let mut outputs = Vec::new();
    for instance in &state.instances {
        let template = instance
            .recipe
            .shape_program_template
            .as_object()
            .expect("validated");
        let suffix = instance.instance_id.trim_start_matches("recipeinst_");
        let mut input_ids = Map::new();
        for input in template
            .get("profile_inputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut input = input.clone();
            let old = input["input_id"].as_str().expect("validated").to_owned();
            let new = format!("profileinput_{suffix}_{old}");
            input["input_id"] = Value::String(new.clone());
            input_ids.insert(old, Value::String(new));
            profile_inputs.push(input);
        }
        let mut operation_ids = Map::new();
        for operation in template
            .get("operations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut operation = operation.clone();
            let old = operation["operation_id"]
                .as_str()
                .expect("validated")
                .to_owned();
            let new = format!("op_{suffix}_{}", old.trim_start_matches("op_"));
            operation["operation_id"] = Value::String(new.clone());
            operation_ids.insert(old, Value::String(new));
            if let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) {
                for key in ["profile_input_id", "section_set_input_id"] {
                    if let Some(old_input) = args.get(key).and_then(Value::as_str) {
                        let rewritten = input_ids.get(old_input).cloned().ok_or_else(|| CoreError::invalid_data(
                            "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                            "Template operation references a profile/section input not embedded by its reviewed Recipe.",
                        ))?;
                        args.insert(key.to_owned(), rewritten);
                    }
                }
            }
            let inputs = operation["inputs"]
                .as_array()
                .expect("validated")
                .iter()
                .map(|input| {
                    operation_ids
                        .get(input.as_str().expect("validated"))
                        .cloned()
                        .expect("ordered template op input")
                })
                .collect();
            operation["inputs"] = Value::Array(inputs);
            bake_static_transform(&mut operation, instance.world_transform)?;
            operations.push(operation);
        }
        for output in template
            .get("outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut output = output.clone();
            let old_output = output["output_id"].as_str().expect("validated");
            output["output_id"] = Value::String(format!(
                "output_{suffix}_{}",
                old_output.trim_start_matches("output_")
            ));
            let operation_id = output["operation_id"].as_str().expect("validated");
            output["operation_id"] = operation_ids
                .get(operation_id)
                .cloned()
                .expect("validated output operation");
            outputs.push(output);
        }
    }
    Ok(json!({
        "schema_version": "ShapeProgram@1",
        "program_id": format!("shape_recipe_{}", &request_sha256[..16]),
        "units": "millimeter",
        "seed": u32::from_str_radix(&request_sha256[..8], 16).unwrap_or(0) % 2_147_483_647,
        "triangle_budget": state.triangle_count.max(100).min(100_000),
        "parameters": [],
        "profile_inputs": profile_inputs,
        "operations": operations,
        "outputs": outputs,
        "non_functional_only": true,
    }))
}

fn bake_static_transform(operation: &mut Value, matrix: Matrix4) -> CoreResult<()> {
    let world_rotation = rigid_rotation(matrix)?;
    let operation_name = operation
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    // Only source geometry receives the world bake.  Derived operations keep
    // their already-transformed inputs; applying it again would rotate bevels
    // and panels twice.  mirror/array/CSG were rejected above when rotated.
    if !matches!(
        operation_name.as_str(),
        "box" | "cylinder" | "capsule" | "wedge" | "extrude" | "revolve" | "loft" | "sweep"
    ) {
        return Ok(());
    }
    let args = operation
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .expect("validated");
    let local_position = vector3(
        args.get("position"),
        "Template position must be a finite three-number vector.",
    )?;
    args.insert(
        "position".into(),
        json!(transform_point(matrix, local_position)?),
    );
    let template_declared_rotation = args.contains_key("rotation");
    let local_rotation = vector3(
        args.get("rotation"),
        "Template rotation must be a finite three-number vector.",
    )?;
    let combined = multiply(
        rotation_matrix_from_euler(euler_xyz_from_rotation(world_rotation)),
        rotation_matrix_from_euler(local_rotation),
    );
    let combined_rotation = euler_xyz_from_rotation(rigid_rotation(combined)?);
    // Preserve C105 v1 canonical candidates byte-for-byte for identity frames:
    // the existing ShapeProgram contract makes omitted rotation identity.  A
    // non-identity baked frame (or an already explicit template rotation) is
    // persisted and therefore included in the candidate hash.
    if template_declared_rotation || combined_rotation.iter().any(|value| value.abs() > 1e-12) {
        args.insert("rotation".into(), json!(combined_rotation));
    }
    // C105's frozen identity-frame sweep candidates historically normalized
    // path-point JSON through ``value + 0.0``. Keep that canonical byte shape
    // for the existing golden without applying a rotated world's frame twice.
    if operation_name == "sweep"
        && combined_rotation.iter().all(|value| value.abs() <= 1e-12)
        && (0..3).all(|axis| matrix[axis][3].abs() <= 1e-12)
    {
        if let Some(points) = args.get_mut("path_points").and_then(Value::as_array_mut) {
            for point in points {
                let point = point.as_array_mut().ok_or_else(|| {
                    CoreError::invalid_data(
                        "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                        "Sweep path points must be finite three-number vectors.",
                    )
                })?;
                if point.len() != 3 {
                    return Err(CoreError::invalid_data(
                        "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                        "Sweep path points must be finite three-number vectors.",
                    ));
                }
                for value in point {
                    *value = json!(
                        value
                            .as_f64()
                            .filter(|value| value.is_finite())
                            .ok_or_else(|| CoreError::invalid_data(
                                "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
                                "Sweep path points must be finite three-number vectors."
                            ))?
                            + 0.0
                    );
                }
            }
        }
    }
    Ok(())
}

fn vector3(value: Option<&Value>, message: &'static str) -> CoreResult<[f64; 3]> {
    let Some(values) = value.and_then(Value::as_array) else {
        return Ok([0.0; 3]);
    };
    if values.len() != 3 {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE",
            message,
        ));
    }
    let mut result = [0.0; 3];
    for (index, item) in values.iter().enumerate() {
        result[index] = item
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                CoreError::invalid_data("COMPONENT_RECIPE_EXECUTION_TEMPLATE_INCOMPLETE", message)
            })?;
    }
    Ok(result)
}

fn build_assembly_graph(state: &ExpansionState, request_sha256: &str) -> CoreResult<Value> {
    let parts = state.instances.iter().map(|instance| {
        let parent = instance.provenance.parent_instance_id.as_ref().map(|id| format!("part_{}", id.trim_start_matches("recipeinst_")));
        let suffix = instance.instance_id.trim_start_matches("recipeinst_");
        let template_output = instance.recipe.shape_program_template["outputs"].as_array().and_then(|outputs| outputs.first()).expect("validated template output");
        let template_operation_id = template_output["operation_id"].as_str().expect("validated template output");
        let template_output_id = template_output["output_id"].as_str().expect("validated template output");
        let mut part = json!({
            "part_id": format!("part_{}", instance.instance_id.trim_start_matches("recipeinst_")),
            // This explicit durable mapping is required after C102 replacement
            // remaps the recipe root to a stable project Part identity.
            "recipe_instance_id": instance.instance_id,
            "role": instance.component_role,
            "parent_part_id": parent,
            "geometry_source": "shape_program",
            "operation_id": format!("op_{suffix}_{}", template_operation_id.trim_start_matches("op_")),
            "output_id": format!("output_{suffix}_{}", template_output_id.trim_start_matches("output_")),
            // The GLB is statically baked, but AssemblyGraph still records the
            // same world frame plus local connector/pivot facts. Consumers can
            // therefore reconstruct a connector's world pose without guessing
            // or deriving a second transform from the mesh.
            "transform": {"position": [instance.world_transform[0][3], instance.world_transform[1][3], instance.world_transform[2][3]], "rotation": euler_xyz_from_rotation(rigid_rotation(instance.world_transform).expect("validated rigid Recipe transform")), "scale": [1.0, 1.0, 1.0]},
            // Connector `up` and the part pivot are persistent AssemblyGraph
            // facts, not transient placement helpers: dropping either loses
            // connector roll/pivot semantics after an asset restart.
            "connectors": instance.recipe.connectors.iter().map(|connector| json!({"connector_id": connector.connector_id, "kind": connector.kind, "position": connector.position, "normal": connector.normal, "up": connector.up})).collect::<Vec<_>>(),
            "pivot": {"position": instance.recipe.pivot.position, "normal": instance.recipe.pivot.normal, "up": instance.recipe.pivot.up},
            "joints": [],
            "material_zones": instance.recipe.material_zones.iter().filter_map(|zone| zone.get("zone_id").cloned()).collect::<Vec<_>>(),
            "material_zone_ids": instance.recipe.material_zones.iter().filter_map(|zone| zone.get("zone_id").cloned()).collect::<Vec<_>>(),
            "editable_parameters": instance.recipe.parameter_bindings.iter().filter_map(|binding| binding.get("parameter_id").cloned()).collect::<Vec<_>>(),
            "editable_parameter_bindings": instance.recipe.parameter_bindings,
            "locked": false,
            "provenance": "agent_generated"
        });
        // Existing C105 Recipes predate C106 design-surface slots.  Omitting
        // an empty optional field preserves their canonical candidate and
        // AssemblyGraph hashes exactly; a reviewed C106 slot is projected only
        // when the Recipe explicitly declares one.
        if !instance.recipe.surface_adornment_slots.is_empty() {
            part.as_object_mut()
                .expect("Recipe AssemblyGraph part is an object")
                .insert(
                    "surface_adornment_slots".into(),
                    serde_json::to_value(&instance.recipe.surface_adornment_slots)
                        .expect("closed surface adornment slot serializes"),
                );
        }
        part
    }).collect::<Vec<_>>();
    let mut connections = Vec::new();
    for child in state.instances.iter().skip(1) {
        let parent_id = child
            .provenance
            .parent_instance_id
            .as_deref()
            .expect("non-root has parent");
        let slot_id = child
            .provenance
            .parent_slot_id
            .as_deref()
            .expect("non-root has parent slot");
        let parent = state
            .instances
            .iter()
            .find(|instance| instance.instance_id == parent_id)
            .expect("expanded parent");
        let slot = parent
            .recipe
            .child_slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .expect("validated slot");
        connections.push(json!({
            "connection_id": stable_id("conn", &json!({"parent": parent.instance_id, "slot": slot.slot_id, "child": child.instance_id}))?,
            "from_part_id": format!("part_{}", parent.instance_id.trim_start_matches("recipeinst_")),
            "from_connector_id": slot.parent_connector_id,
            "to_part_id": format!("part_{}", child.instance_id.trim_start_matches("recipeinst_")),
            "to_connector_id": slot.child_connector_id,
            "status": "connected",
        }));
    }
    Ok(json!({
        "schema_version": "AssemblyGraph@1",
        "graph_id": format!("asset_recipe_graph_{}", &request_sha256[..16]),
        "concept_id": format!("asset_recipe_{}", &request_sha256[..16]),
        "root_part_id": format!("part_{}", state.instances[0].instance_id.trim_start_matches("recipeinst_")),
        "parts": parts,
        "connections": connections,
        "component_recipe_instances": state.provenance,
    }))
}

fn candidate_hash(candidate: &ExpandedComponentCandidate) -> CoreResult<String> {
    let mut value = serde_json::to_value(candidate).map_err(|error| {
        CoreError::invalid_data("COMPONENT_RECIPE_CANDIDATE_INVALID", error.to_string())
    })?;
    value["candidate_sha256"] = Value::String(String::new());
    semantic_sha256(&value)
}
