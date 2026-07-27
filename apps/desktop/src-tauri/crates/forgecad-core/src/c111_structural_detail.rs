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
    let output_owner = program
        .parts
        .iter()
        .flat_map(|part| {
            part.geometry_output_ids
                .iter()
                .map(move |output_id| (output_id.as_str(), part.part_id.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let program_ids = surface_programs
        .iter()
        .map(|surface| surface.program_id.as_str())
        .collect::<BTreeSet<_>>();

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
        geometry_lineage(
            "cable_clamps",
            &["_cable_clamp_bridge"],
            &["zone_arm_cable_clamp_trim"],
        )?,
        geometry_lineage(
            "gripper_hinges",
            &[
                "_gripper_knuckle_a",
                "_gripper_distal_knuckle_a",
                "_gripper_finger_a_pad",
            ],
            &["zone_arm_gripper_knuckle", "zone_arm_gripper_contact"],
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

fn missing(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("C111_STRUCTURAL_DETAIL_MISSING", message)
}
