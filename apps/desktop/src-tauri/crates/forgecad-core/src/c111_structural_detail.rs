use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    semantic_sha256, CoreError, CoreResult, ForgeVisualProgram, SurfaceAdornmentProgram,
    SurfaceLayerProgram,
};

pub const C111_STRUCTURAL_DETAIL_SCHEMA_VERSION: &str = "C111StructuralDetailContract@1";
const REQUIRED_TEXTURE_ROLES: [&str; 5] = [
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
];
const GRIPPER_FINGERS: [&str; 3] = ["a", "b", "c"];
const MIN_RUBBER_CABLE_OUTPUTS: usize = 2;
const MIN_CABLE_CLAMP_OUTPUTS: usize = 2;

#[derive(Debug)]
struct GeometryOperationFact {
    operation_id: String,
    operation_kind: String,
    inputs: Vec<String>,
    part_role: Option<String>,
    material_id: Option<String>,
    zone_id: Option<String>,
}

#[derive(Debug)]
struct GeometryOutputFact {
    operation_id: String,
    output_kind: String,
    owner_part_id: String,
}

#[derive(Debug)]
struct GeometryFacts {
    operations: BTreeMap<String, GeometryOperationFact>,
    outputs: BTreeMap<String, GeometryOutputFact>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct C111StructuralDetailLineage {
    pub detail_class: String,
    pub part_ids: Vec<String>,
    pub geometry_output_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub surface_program_ids: Vec<String>,
    pub required_texture_roles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct C111StructuralDetailContract {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub surface_layer_program_id: String,
    pub surface_layer_program_sha256: String,
    pub surface_layer_lowering_sha256: String,
    pub lineages: Vec<C111StructuralDetailLineage>,
}

impl C111StructuralDetailContract {
    pub fn validate(
        &self,
        program: &ForgeVisualProgram,
        surface_layer: &SurfaceLayerProgram,
    ) -> CoreResult<()> {
        surface_layer.validate()?;
        let lowering = surface_layer.lower()?;
        if self.schema_version != C111_STRUCTURAL_DETAIL_SCHEMA_VERSION
            || self.source_program_sha256 != semantic_sha256(program)?
            || self.surface_layer_program_id != surface_layer.program_id
            || self.surface_layer_program_sha256 != semantic_sha256(surface_layer)?
            || self.surface_layer_lowering_sha256 != semantic_sha256(&lowering)?
            || surface_layer.target_zone_id != "zone_arm_link_armor"
            || surface_layer.material_zone_id != "zone_arm_link_armor"
            || surface_layer.target_part_role != "link_armor"
            || !surface_layer
                .decal_layers
                .iter()
                .any(|layer| layer.motif == "warning_stripe" && layer.text_token == "CAUTION")
            || !surface_layer
                .decal_layers
                .iter()
                .any(|layer| layer.motif == "panel_label" && layer.text_token == "A-01")
            || !surface_layer
                .roughness_masks
                .iter()
                .any(|mask| mask.motif == "edge_wear")
            || !surface_layer
                .roughness_masks
                .iter()
                .any(|mask| mask.motif == "linear_brush")
            || !surface_layer
                .emissive_masks
                .iter()
                .any(|mask| mask.motif == "double_flowline")
        {
            return Err(missing("C111 structural detail source lineage is invalid."));
        }
        let required_classes = BTreeSet::from([
            "service_panel",
            "joint_stack",
            "auxiliary_linkage",
            "cable_clamps",
            "gripper_hinges",
            "decal",
            "wear",
        ]);
        let actual_classes = self
            .lineages
            .iter()
            .map(|lineage| lineage.detail_class.as_str())
            .collect::<BTreeSet<_>>();
        if actual_classes != required_classes
            || self.lineages.len() != required_classes.len()
            || self.lineages.iter().any(|lineage| {
                lineage.part_ids.is_empty()
                    || lineage.material_zone_ids.is_empty()
                    || (lineage.geometry_output_ids.is_empty()
                        && lineage.surface_program_ids.is_empty())
                    || lineage.part_ids.iter().collect::<BTreeSet<_>>().len()
                        != lineage.part_ids.len()
                    || lineage
                        .geometry_output_ids
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != lineage.geometry_output_ids.len()
                    || lineage
                        .material_zone_ids
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != lineage.material_zone_ids.len()
                    || lineage
                        .surface_program_ids
                        .iter()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != lineage.surface_program_ids.len()
                    || lineage.required_texture_roles != REQUIRED_TEXTURE_ROLES.map(str::to_owned)
            })
        {
            return Err(missing(
                "C111 structural detail contract is missing a required detail class or exact lineage.",
            ));
        }

        let parts = program
            .parts
            .iter()
            .map(|part| (part.part_id.as_str(), part))
            .collect::<BTreeMap<_, _>>();
        let mut surface_ids = program
            .surface_graph
            .iter()
            .map(|surface| surface.surface_program_id.clone())
            .collect::<BTreeSet<_>>();
        surface_ids.extend(
            lowering
                .adornments
                .iter()
                .map(|surface| surface.program_id.clone()),
        );
        for lineage in &self.lineages {
            for part_id in &lineage.part_ids {
                parts
                    .get(part_id.as_str())
                    .ok_or_else(|| missing("C111 structural detail references a missing Part."))?;
            }
            if lineage.geometry_output_ids.iter().any(|output_id| {
                !lineage.part_ids.iter().any(|part_id| {
                    parts.get(part_id.as_str()).is_some_and(|part| {
                        part.geometry_output_ids
                            .iter()
                            .any(|value| value == output_id)
                    })
                })
            }) || lineage.material_zone_ids.iter().any(|zone_id| {
                !lineage.part_ids.iter().any(|part_id| {
                    parts.get(part_id.as_str()).is_some_and(|part| {
                        part.material_zone_ids.iter().any(|value| value == zone_id)
                    })
                })
            }) {
                return Err(missing(
                    "C111 structural detail output or Material Zone is not owned by its exact Part set.",
                ));
            }
            if lineage
                .surface_program_ids
                .iter()
                .any(|program_id| !surface_ids.contains(program_id))
            {
                return Err(missing(
                    "C111 structural detail references a missing surface program.",
                ));
            }
        }
        let geometry = GeometryFacts::from_program(program)?;
        let lineages = self
            .lineages
            .iter()
            .map(|lineage| (lineage.detail_class.as_str(), lineage))
            .collect::<BTreeMap<_, _>>();
        validate_cable_detail(
            lineages
                .get("cable_clamps")
                .copied()
                .ok_or_else(|| missing("C111 cable detail lineage is missing."))?,
            program,
            &geometry,
        )?;
        validate_gripper_detail(
            lineages
                .get("gripper_hinges")
                .copied()
                .ok_or_else(|| missing("C111 gripper detail lineage is missing."))?,
            program,
            &geometry,
        )?;
        Ok(())
    }
}

pub fn build_c111_structural_detail_contract(
    program: &ForgeVisualProgram,
    surface_programs: &[SurfaceAdornmentProgram],
    surface_layer: &SurfaceLayerProgram,
) -> CoreResult<C111StructuralDetailContract> {
    surface_layer.validate()?;
    let surface_layer_lowering = surface_layer.lower()?;
    let output_owner =
        program
            .parts
            .iter()
            .try_fold(BTreeMap::new(), |mut owners, part| -> CoreResult<_> {
                for output_id in &part.geometry_output_ids {
                    if owners
                        .insert(output_id.as_str(), part.part_id.as_str())
                        .is_some()
                    {
                        return Err(missing(
                            "C111 geometry outputs must have exactly one Part owner.",
                        ));
                    }
                }
                Ok(owners)
            })?;
    let program_ids = surface_programs
        .iter()
        .map(|surface| surface.program_id.as_str())
        .collect::<BTreeSet<_>>();
    let geometry = GeometryFacts::from_program(program)?;

    let geometry_lineage = |detail_class: &str,
                            output_suffixes: &[&str],
                            zone_ids: &[&str]|
     -> CoreResult<C111StructuralDetailLineage> {
        let mut output_ids = Vec::new();
        let mut part_ids = BTreeSet::new();
        for suffix in output_suffixes {
            let matches = output_owner
                .iter()
                .filter(|(output_id, _)| output_id.ends_with(suffix))
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(missing(format!(
                    "C111 detail class {detail_class} is missing geometry output suffix {suffix}."
                )));
            }
            for (output_id, owner) in matches {
                output_ids.push((*output_id).to_owned());
                part_ids.insert((*owner).to_owned());
            }
        }
        output_ids.sort();
        output_ids.dedup();
        Ok(C111StructuralDetailLineage {
            detail_class: detail_class.to_owned(),
            part_ids: part_ids.into_iter().collect(),
            geometry_output_ids: output_ids,
            material_zone_ids: zone_ids.iter().map(|value| (*value).to_owned()).collect(),
            surface_program_ids: Vec::new(),
            required_texture_roles: REQUIRED_TEXTURE_ROLES.map(str::to_owned).to_vec(),
        })
    };

    let surface_lineage =
        |detail_class: &str, zone_id: &str| -> CoreResult<C111StructuralDetailLineage> {
            let surface = surface_layer_lowering
                .adornments
                .iter()
                .find(|surface| surface.target_zone_id == zone_id)
                .ok_or_else(|| {
                    missing(format!(
                    "C111 detail class {detail_class} is missing a surface program on {zone_id}."
                ))
                })?;
            if !program_ids.contains(surface.program_id.as_str()) {
                return Err(missing(format!(
                    "C111 detail class {detail_class} has an invalid surface program identity."
                )));
            }
            let part = program
                .parts
                .iter()
                .find(|part| {
                    part.part_id == surface_layer.target_part_id
                        && part.material_zone_ids.iter().any(|value| value == zone_id)
                })
                .ok_or_else(|| missing("C111 Design Surface has no exact Part/Zone binding."))?;
            if surface_layer.material_zone_id != zone_id {
                return Err(missing(
                    "C111 surface detail is bound to the wrong Material Zone.",
                ));
            }
            Ok(C111StructuralDetailLineage {
                detail_class: detail_class.to_owned(),
                part_ids: vec![part.part_id.clone()],
                geometry_output_ids: Vec::new(),
                material_zone_ids: vec![zone_id.to_owned()],
                surface_program_ids: vec![surface.program_id.clone()],
                required_texture_roles: REQUIRED_TEXTURE_ROLES.map(str::to_owned).to_vec(),
            })
        };

    let rubber_cable_outputs = geometry
        .outputs
        .iter()
        .filter(|(_, output)| {
            geometry
                .operations
                .get(&output.operation_id)
                .is_some_and(|operation| {
                    operation.zone_id.as_deref() == Some("zone_arm_cable")
                        && operation.material_id.as_deref() == Some("mat_rubber")
                })
        })
        .collect::<Vec<_>>();
    if rubber_cable_outputs.len() < MIN_RUBBER_CABLE_OUTPUTS {
        return Err(missing(format!(
            "C111 cable harness requires at least {MIN_RUBBER_CABLE_OUTPUTS} distinct rubber cable outputs."
        )));
    }
    let clamp_outputs = geometry
        .outputs
        .iter()
        .filter(|(_, output)| {
            geometry
                .operations
                .get(&output.operation_id)
                .is_some_and(|operation| {
                    operation.operation_id.contains("_cable_clamp_")
                        && operation.zone_id.as_deref() == Some("zone_arm_cable_clamp_trim")
                })
        })
        .collect::<Vec<_>>();
    if clamp_outputs.len() < MIN_CABLE_CLAMP_OUTPUTS {
        return Err(missing(format!(
            "C111 cable harness requires at least {MIN_CABLE_CLAMP_OUTPUTS} distinct clamp outputs."
        )));
    }
    let mut cable_lineage = C111StructuralDetailLineage {
        detail_class: "cable_clamps".to_owned(),
        part_ids: Vec::new(),
        geometry_output_ids: Vec::new(),
        material_zone_ids: vec![
            "zone_arm_cable".to_owned(),
            "zone_arm_cable_clamp_trim".to_owned(),
        ],
        surface_program_ids: Vec::new(),
        required_texture_roles: REQUIRED_TEXTURE_ROLES.map(str::to_owned).to_vec(),
    };
    for (output_id, output) in rubber_cable_outputs.into_iter().chain(clamp_outputs) {
        cable_lineage.geometry_output_ids.push(output_id.clone());
        cable_lineage.part_ids.push(output.owner_part_id.clone());
    }
    cable_lineage.geometry_output_ids.sort();
    cable_lineage.geometry_output_ids.dedup();
    cable_lineage.part_ids.sort();
    cable_lineage.part_ids.dedup();

    let mut lineages = vec![
        geometry_lineage(
            "service_panel",
            &["_plinth_guard_array", "_plinth_service_panel"],
            &["zone_arm_base_paint", "zone_arm_base_service"],
        )?,
        geometry_lineage(
            "joint_stack",
            &[
                "_joint_inner_bearing",
                "_joint_outer_ring_secondary",
                "_joint_lower_guard",
            ],
            &["zone_arm_joint_shell", "zone_arm_joint_cap"],
        )?,
        geometry_lineage(
            "auxiliary_linkage",
            &["_link_upper_tension_rod", "_link_frame_cross_brace"],
            &["zone_arm_link_rail"],
        )?,
        cable_lineage,
        geometry_lineage(
            "gripper_hinges",
            &[
                "_gripper_knuckle_a",
                "_gripper_distal_knuckle_a",
                "_gripper_finger_a_armor",
                "_gripper_finger_a_pad",
                "_gripper_knuckle_b",
                "_gripper_distal_knuckle_b",
                "_gripper_finger_b_armor",
                "_gripper_finger_b_pad",
                "_gripper_knuckle_c",
                "_gripper_distal_knuckle_c",
                "_gripper_finger_c_armor",
                "_gripper_finger_c_panel",
            ],
            &[
                "zone_arm_gripper_paint",
                "zone_arm_gripper_knuckle",
                "zone_arm_gripper_contact",
            ],
        )?,
        surface_lineage("decal", "zone_arm_link_armor")?,
        surface_lineage("wear", "zone_arm_link_armor")?,
    ];
    lineages.sort_by(|left, right| left.detail_class.cmp(&right.detail_class));
    let contract = C111StructuralDetailContract {
        schema_version: C111_STRUCTURAL_DETAIL_SCHEMA_VERSION.to_owned(),
        source_program_sha256: semantic_sha256(program)?,
        surface_layer_program_id: surface_layer.program_id.clone(),
        surface_layer_program_sha256: semantic_sha256(surface_layer)?,
        surface_layer_lowering_sha256: semantic_sha256(&surface_layer_lowering)?,
        lineages,
    };
    contract.validate(program, surface_layer)?;
    Ok(contract)
}

impl GeometryFacts {
    fn from_program(program: &ForgeVisualProgram) -> CoreResult<Self> {
        let raw_operations = program.geometry_graph["operations"]
            .as_array()
            .ok_or_else(|| missing("C111 geometry graph operations are missing."))?;
        let mut operations = BTreeMap::new();
        for raw_operation in raw_operations {
            let operation_id = required_string(raw_operation, "operation_id")?;
            let operation_kind = required_string(raw_operation, "op")?;
            let inputs = raw_operation["inputs"]
                .as_array()
                .ok_or_else(|| missing("C111 geometry operation inputs are missing."))?
                .iter()
                .map(|input| {
                    input
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| missing("C111 geometry operation input is invalid."))
                })
                .collect::<CoreResult<Vec<_>>>()?;
            let args = raw_operation["args"].as_object();
            let fact = GeometryOperationFact {
                operation_id: operation_id.clone(),
                operation_kind,
                inputs,
                part_role: args
                    .and_then(|value| value.get("part_role"))
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                material_id: args
                    .and_then(|value| value.get("material_id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                zone_id: args
                    .and_then(|value| value.get("zone_id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
            };
            if operations.insert(operation_id, fact).is_some() {
                return Err(missing(
                    "C111 geometry operation identifiers are not unique.",
                ));
            }
        }

        let owner_by_output = program.parts.iter().try_fold(
            BTreeMap::new(),
            |mut owners, part| -> CoreResult<_> {
                for output_id in &part.geometry_output_ids {
                    if owners
                        .insert(output_id.as_str(), part.part_id.as_str())
                        .is_some()
                    {
                        return Err(missing(
                            "C111 geometry outputs must have exactly one Part owner.",
                        ));
                    }
                }
                Ok(owners)
            },
        )?;
        let raw_outputs = program.geometry_graph["outputs"]
            .as_array()
            .ok_or_else(|| missing("C111 geometry graph outputs are missing."))?;
        let mut outputs = BTreeMap::new();
        for raw_output in raw_outputs {
            let output_id = required_string(raw_output, "output_id")?;
            let operation_id = required_string(raw_output, "operation_id")?;
            if !operations.contains_key(&operation_id) {
                return Err(missing(
                    "C111 geometry output references a missing operation.",
                ));
            }
            let fact = GeometryOutputFact {
                operation_id,
                output_kind: required_string(raw_output, "kind")?,
                owner_part_id: owner_by_output
                    .get(output_id.as_str())
                    .copied()
                    .ok_or_else(|| missing("C111 geometry output has no exact Part owner."))?
                    .to_owned(),
            };
            if outputs.insert(output_id, fact).is_some() {
                return Err(missing("C111 geometry output identifiers are not unique."));
            }
        }
        Ok(Self {
            operations,
            outputs,
        })
    }
}

fn validate_cable_detail(
    lineage: &C111StructuralDetailLineage,
    program: &ForgeVisualProgram,
    geometry: &GeometryFacts,
) -> CoreResult<()> {
    require_exact_zones(lineage, &["zone_arm_cable", "zone_arm_cable_clamp_trim"])?;
    let mut owner_ids = BTreeSet::new();
    let mut rubber_operation_ids = BTreeSet::new();
    let mut clamp_operation_ids = BTreeSet::new();
    for output_id in &lineage.geometry_output_ids {
        let (output, operation) = output_operation(output_id, geometry)?;
        owner_ids.insert(output.owner_part_id.as_str());
        if operation.zone_id.as_deref() == Some("zone_arm_cable")
            && operation.material_id.as_deref() == Some("mat_rubber")
        {
            require_exported_mesh_binding(
                output_id,
                output,
                operation,
                program,
                "cable_harness",
                "visual_detail",
                "zone_arm_cable",
                "mat_rubber",
            )?;
            if operation.operation_kind != "sweep" {
                return Err(missing(
                    "C111 rubber cable output must lower from sweep geometry.",
                ));
            }
            rubber_operation_ids.insert(operation.operation_id.as_str());
        } else if operation.operation_id.contains("_cable_clamp_")
            && operation.zone_id.as_deref() == Some("zone_arm_cable_clamp_trim")
        {
            require_exported_mesh_binding(
                output_id,
                output,
                operation,
                program,
                "cable_harness",
                "visual_detail",
                "zone_arm_cable_clamp_trim",
                "mat_aluminum",
            )?;
            if operation.operation_kind != "surface_panel" || operation.inputs.len() != 1 {
                return Err(missing(
                    "C111 clamp output must be a surface panel adjacent to one clamp base operation.",
                ));
            }
            let base = geometry
                .operations
                .get(&operation.inputs[0])
                .ok_or_else(|| missing("C111 clamp base operation is missing."))?;
            if base.part_role != operation.part_role
                || base.zone_id.as_deref() != Some("zone_arm_cable_clamp_trim")
                || base.material_id.as_deref() != Some("mat_aluminum")
            {
                return Err(missing(
                    "C111 clamp panel and base must share the exact role, Material Zone, and material.",
                ));
            }
            clamp_operation_ids.insert(operation.operation_id.as_str());
        } else {
            return Err(missing(
                "C111 cable lineage contains an output without rubber-cable or clamp semantics.",
            ));
        }
    }
    if rubber_operation_ids.len() < MIN_RUBBER_CABLE_OUTPUTS {
        return Err(missing(format!(
            "C111 cable harness requires at least {MIN_RUBBER_CABLE_OUTPUTS} distinct rubber cable outputs."
        )));
    }
    if clamp_operation_ids.len() < MIN_CABLE_CLAMP_OUTPUTS {
        return Err(missing(format!(
            "C111 cable harness requires at least {MIN_CABLE_CLAMP_OUTPUTS} distinct clamp outputs."
        )));
    }
    require_exact_output_owners(lineage, owner_ids, program, "cable_harness")
}

fn validate_gripper_detail(
    lineage: &C111StructuralDetailLineage,
    program: &ForgeVisualProgram,
    geometry: &GeometryFacts,
) -> CoreResult<()> {
    require_exact_zones(
        lineage,
        &[
            "zone_arm_gripper_paint",
            "zone_arm_gripper_knuckle",
            "zone_arm_gripper_contact",
        ],
    )?;
    if lineage.geometry_output_ids.len() != GRIPPER_FINGERS.len() * 4 {
        return Err(missing(
            "C111 gripper lineage must contain exactly four structural outputs for each of fingers A/B/C.",
        ));
    }

    let mut owner_ids = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    for finger in GRIPPER_FINGERS {
        let requirements = [
            (
                format!("_gripper_knuckle_{finger}"),
                "zone_arm_gripper_knuckle",
                "mat_graphite",
                "hinge",
            ),
            (
                format!("_gripper_distal_knuckle_{finger}"),
                "zone_arm_gripper_knuckle",
                "mat_graphite",
                "distal hinge",
            ),
            (
                format!("_gripper_finger_{finger}_armor"),
                "zone_arm_gripper_paint",
                "mat_automotive_paint",
                "armor",
            ),
            (
                if finger == "c" {
                    "_gripper_finger_c_panel".to_owned()
                } else {
                    format!("_gripper_finger_{finger}_pad")
                },
                "zone_arm_gripper_contact",
                "mat_rubber",
                "contact",
            ),
        ];
        for (suffix, zone_id, material_id, detail_name) in requirements {
            let output_id = exact_lineage_output(lineage, &suffix)?;
            let (output, operation) = output_operation(output_id, geometry)?;
            require_exported_mesh_binding(
                output_id,
                output,
                operation,
                program,
                "end_effector_form",
                "end_effector_form",
                zone_id,
                material_id,
            )?;
            if !operation_ids.insert(operation.operation_id.as_str()) {
                return Err(missing(
                    "C111 gripper structural outputs must reference distinct operations.",
                ));
            }
            owner_ids.insert(output.owner_part_id.as_str());

            if detail_name == "armor" {
                let expected_input_suffix = format!("_gripper_finger_{finger}_core");
                require_operation_input(operation, geometry, &expected_input_suffix)?;
            } else if detail_name == "contact" {
                let expected_operation_suffix = if finger == "c" {
                    "_gripper_finger_c_panel".to_owned()
                } else {
                    format!("_gripper_finger_{finger}_pad")
                };
                if operation.operation_kind != "surface_panel"
                    || !operation.operation_id.ends_with(&expected_operation_suffix)
                {
                    return Err(missing(format!(
                        "C111 finger {finger} contact output is not bound to its contact panel operation."
                    )));
                }
                let expected_input_suffix = format!("_gripper_finger_{finger}");
                require_operation_input(operation, geometry, &expected_input_suffix)?;
            }
        }
    }
    require_exact_output_owners(lineage, owner_ids, program, "end_effector_form")
}

fn required_string(value: &serde_json::Value, field: &str) -> CoreResult<String> {
    value[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| missing(format!("C111 geometry {field} is missing or invalid.")))
}

fn exact_lineage_output<'a>(
    lineage: &'a C111StructuralDetailLineage,
    suffix: &str,
) -> CoreResult<&'a str> {
    let matches = lineage
        .geometry_output_ids
        .iter()
        .filter(|output_id| output_id.ends_with(suffix))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(missing(format!(
            "C111 structural lineage requires exactly one output suffix {suffix}."
        )));
    }
    Ok(matches[0].as_str())
}

fn output_operation<'a>(
    output_id: &str,
    geometry: &'a GeometryFacts,
) -> CoreResult<(&'a GeometryOutputFact, &'a GeometryOperationFact)> {
    let output = geometry
        .outputs
        .get(output_id)
        .ok_or_else(|| missing("C111 structural output is absent from the geometry graph."))?;
    let operation = geometry
        .operations
        .get(&output.operation_id)
        .ok_or_else(|| missing("C111 structural output operation is missing."))?;
    Ok((output, operation))
}

#[allow(clippy::too_many_arguments)]
fn require_exported_mesh_binding(
    output_id: &str,
    output: &GeometryOutputFact,
    operation: &GeometryOperationFact,
    program: &ForgeVisualProgram,
    owner_role: &str,
    operation_role: &str,
    zone_id: &str,
    material_id: &str,
) -> CoreResult<()> {
    let owner = program
        .parts
        .iter()
        .find(|part| part.part_id == output.owner_part_id)
        .ok_or_else(|| missing("C111 structural output owner Part is missing."))?;
    let material_bound = program.material_graph.iter().any(|binding| {
        binding.part_id == output.owner_part_id
            && binding.material_zone_id == zone_id
            && binding.material_id == material_id
    });
    if output.output_kind != "mesh"
        || owner.role != owner_role
        || operation.part_role.as_deref() != Some(operation_role)
        || operation.zone_id.as_deref() != Some(zone_id)
        || operation.material_id.as_deref() != Some(material_id)
        || !material_bound
    {
        return Err(missing(format!(
            "C111 structural output {output_id} lacks an exact exported mesh, Part role, Material Zone, or material binding."
        )));
    }
    Ok(())
}

fn require_operation_input(
    operation: &GeometryOperationFact,
    geometry: &GeometryFacts,
    expected_suffix: &str,
) -> CoreResult<()> {
    if operation.inputs.len() != 1 || !operation.inputs[0].ends_with(expected_suffix) {
        return Err(missing(format!(
            "C111 operation {} lacks exact adjacency input {expected_suffix}.",
            operation.operation_id
        )));
    }
    let input = geometry
        .operations
        .get(&operation.inputs[0])
        .ok_or_else(|| missing("C111 adjacency input operation is missing."))?;
    if input.part_role != operation.part_role {
        return Err(missing(
            "C111 adjacency input must share the exact operation Part role.",
        ));
    }
    Ok(())
}

fn require_exact_zones(lineage: &C111StructuralDetailLineage, expected: &[&str]) -> CoreResult<()> {
    let actual = lineage
        .material_zone_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected.iter().copied().collect::<BTreeSet<_>>() {
        return Err(missing(
            "C111 structural lineage does not contain its exact Material Zone set.",
        ));
    }
    Ok(())
}

fn require_exact_output_owners<'a>(
    lineage: &C111StructuralDetailLineage,
    owner_ids: BTreeSet<&'a str>,
    program: &'a ForgeVisualProgram,
    expected_role: &str,
) -> CoreResult<()> {
    let declared = lineage
        .part_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if owner_ids.len() != 1
        || owner_ids != declared
        || owner_ids.iter().any(|owner_id| {
            !program
                .parts
                .iter()
                .any(|part| part.part_id == *owner_id && part.role == expected_role)
        })
    {
        return Err(missing(
            "C111 adjacent structural outputs must share one exact Part owner and role.",
        ));
    }
    Ok(())
}

fn missing(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("C111_STRUCTURAL_DETAIL_MISSING", message)
}
