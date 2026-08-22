//! Runtime-owned durable Appearance source lineage.
//!
//! AppearancePrepare currently leaves its source program and supporting
//! receipts discoverable only through the returned transport envelope and
//! embedded GLB metadata.  This module materializes that source truth as one
//! immutable CAS sidecar plus a compact SQLite Link.  The sidecar is strictly
//! candidate/project/cohort bound and carries the three authored LOD GLB
//! hashes, so a restart cannot silently fall back to a different source.

use super::{
    build_cohort_sha256, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    now_string, CasObject, Runtime, RuntimeError, MAX_GEOMETRY_ARTIFACT_BYTES,
};
use forgecad_contracts::AppearanceSourceLineageLinkRecord;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const REQUEST_SCHEMA: &str = "AppearanceSourceLineagePrepareRequest@1";
const GET_REQUEST_SCHEMA: &str = "AppearanceSourceLineageGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "AppearanceSourceLineagePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "AppearanceSourceLineageGetResult@1";
const SIDECAR_SCHEMA: &str = "AppearanceSourceLineageSidecar@1";
const LINK_SCHEMA: &str = "AppearanceSourceLineageLink@1";
const SOURCE_PROGRAM_KIND: &str = "appearance-source-program";
// Reuse the Runtime's canonical MaterialPack object kind. The same manifest
// bytes are already persisted by AppearancePrepare; assigning a second kind
// to the same content-addressed object would make CAS metadata conflict.
const MATERIAL_PACK_KIND: &str = "material-pack-manifest";
const TEXTURE_RECEIPT_KIND: &str = "texture-build-receipt";
const SURFACE_BAKE_RECEIPT_KIND: &str = "candidate-surface-bake-receipt";
const SIDECAR_KIND: &str = "appearance-source-lineage-sidecar";
const MAX_SOURCE_JSON_BYTES: u64 = 1024 * 1024;
const ENERGY_PACK_ID: &str = "forgecad-fictional-energy-weapon-2k";
#[cfg(test)]
const WORKER_FALLBACK_COHORT: &str = "forgecad-source-test-fallback-worker-cohort";

#[derive(Debug, Clone)]
struct LodBinding {
    level: u64,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_sha256: String,
    artifact_readback_sha256: String,
    artifact_readback_object_sha256: String,
    artifact_readback_object: Option<CasObject>,
    part_binding_inventory_sha256: String,
    part_ids: Vec<String>,
    source_node_ids: Vec<String>,
    part_bindings: Vec<Value>,
    geometry_program_sha256: String,
    appearance_program_sha256: String,
    material_pack_manifest_sha256: String,
    uv_binding_sha256: String,
    material_zone_ids: Vec<String>,
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "source_replay_worker_cohort_sha256",
            "appearance_program",
            "geometry_program_object_sha256",
            "material_pack_manifest_sha256",
            "texture_build_receipt_sha256",
            "candidate_surface_bake_receipt_sha256",
            "uv_binding_sha256",
            "lods",
            "canonical_sha256",
        ],
        REQUEST_SCHEMA,
    )?;
    if text(object, "schema_version")? != REQUEST_SCHEMA {
        return invalid("Appearance source lineage request schema differs");
    }
    verify_canonical(request, REQUEST_SCHEMA)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?.to_owned();
    let worker_cohort = sha(object, "source_replay_worker_cohort_sha256")?.to_owned();
    ensure_current_worker_cohort(&worker_cohort)?;
    let appearance_program = object
        .get("appearance_program")
        .ok_or_else(|| invalid_error("appearance_program is required"))?;
    validate_appearance_program(appearance_program, &project_id)?;
    let appearance_program_sha256 = sha_value(appearance_program, "canonical_sha256")?.to_owned();
    let geometry_program_object_sha256 = sha(object, "geometry_program_object_sha256")?.to_owned();
    let source_geometry_program_sha256 =
        sha_value(appearance_program, "geometry_program_sha256")?.to_owned();
    let pack_manifest_sha256 = sha(object, "material_pack_manifest_sha256")?.to_owned();
    let texture_receipt_object_sha256 = sha(object, "texture_build_receipt_sha256")?.to_owned();
    let surface_bake_receipt_object_sha256 = match object
        .get("candidate_surface_bake_receipt_sha256")
        .ok_or_else(|| invalid_error("candidate_surface_bake_receipt_sha256 is required"))?
    {
        Value::Null => None,
        Value::String(value) if is_sha256(value) => Some(value.clone()),
        _ => return invalid("candidate_surface_bake_receipt_sha256 must be a SHA-256 or null"),
    };
    let uv_binding_sha256 = sha(object, "uv_binding_sha256")?.to_owned();
    let lod_declarations = object
        .get("lods")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid_error("exactly three LOD declarations are required"))?;

    let pack_manifest = super::material_pack_manifest_by_id(ENERGY_PACK_ID).ok_or_else(|| {
        invalid_error("Appearance source MaterialPack is unavailable in this Runtime cohort")
    })?;
    if pack_manifest
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(pack_manifest_sha256.as_str())
    {
        return invalid("Appearance source MaterialPack manifest hash differs");
    }
    let texture_receipt = read_json_object(
        runtime,
        &texture_receipt_object_sha256,
        TEXTURE_RECEIPT_KIND,
    )?;
    validate_texture_receipt(&texture_receipt, &pack_manifest_sha256)?;

    ensure_geometry_program_object(runtime, &geometry_program_object_sha256)?;
    if let Some(source_geometry_evidence) = runtime
        .store
        .get_geometry_candidate_evidence(&candidate_id)?
    {
        if source_geometry_evidence.project_id != project_id
            || source_geometry_evidence.geometry_program_sha256 != source_geometry_program_sha256
            || source_geometry_evidence.geometry_program_object_sha256
                != geometry_program_object_sha256
        {
            return invalid("Appearance source GeometryProgram evidence binding differs");
        }
    }
    let surface_bake_receipt = if let Some(hash) = surface_bake_receipt_object_sha256.as_deref() {
        let value = read_json_object(runtime, hash, SURFACE_BAKE_RECEIPT_KIND)?;
        validate_surface_bake_receipt(
            &value,
            &project_id,
            &candidate_id,
            &candidate_state_sha256,
            &source_geometry_program_sha256,
            &appearance_program_sha256,
            &pack_manifest_sha256,
            &Value::String(texture_receipt_object_sha256.clone()),
            &uv_binding_sha256,
            appearance_program,
        )?;
        Some(value)
    } else {
        if appearance_program["schema_version"] == "AppearanceProgram@3" {
            return invalid("AppearanceProgram@3 requires a CandidateSurfaceBake receipt");
        }
        None
    };

    let program_material_zones = validate_appearance_program_material_zones(appearance_program)?;
    let mut lods = Vec::with_capacity(3);
    for (expected_level, declaration) in lod_declarations.iter().enumerate() {
        let level = exact_lod(declaration)?;
        if level.level != expected_level as u64 {
            return invalid("LOD declarations must be ordered 0, 1, 2");
        }
        let binding = validate_lod(
            runtime,
            &project_id,
            &level,
            &candidate_id,
            &candidate_state_sha256,
            &appearance_program_sha256,
            &pack_manifest_sha256,
            &uv_binding_sha256,
            &program_material_zones,
            &source_geometry_program_sha256,
            &geometry_program_object_sha256,
            true,
        )?;
        lods.push(binding);
    }

    let request_sha256 = canonical_json_hash(request);
    if let Some(existing) = runtime
        .store
        .get_appearance_source_lineage_link(&candidate_id, &appearance_program_sha256)?
    {
        if existing.project_id != project_id || existing.request_sha256 != request_sha256 {
            return invalid(
                "Appearance source lineage immutable key is already bound to a different request",
            );
        }
        return load_link(runtime, &existing, PREPARE_RESULT_SCHEMA, true);
    }

    let program_bytes = canonical_json_bytes(appearance_program)
        .map_err(|error| invalid_error(error.to_string()))?;
    if program_bytes.len() as u64 > MAX_SOURCE_JSON_BYTES {
        return invalid("Appearance source program exceeds the 1 MiB CAS budget");
    }
    let program_object = runtime.put_object(
        &program_bytes,
        None,
        "application/json",
        SOURCE_PROGRAM_KIND,
    )?;
    let manifest_bytes =
        canonical_json_bytes(&pack_manifest).map_err(|error| invalid_error(error.to_string()))?;
    let manifest_object = runtime.put_object(
        &manifest_bytes,
        None,
        "application/json",
        MATERIAL_PACK_KIND,
    )?;
    let sidecar = sidecar_value(
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &worker_cohort,
        appearance_program,
        &appearance_program_sha256,
        &program_material_zones,
        &pack_manifest_sha256,
        &texture_receipt_object_sha256,
        &texture_receipt,
        &uv_binding_sha256,
        surface_bake_receipt_object_sha256.as_deref(),
        surface_bake_receipt.as_ref(),
        &source_geometry_program_sha256,
        &geometry_program_object_sha256,
        &lods,
    )?;
    let sidecar_bytes =
        canonical_json_bytes(&sidecar).map_err(|error| invalid_error(error.to_string()))?;
    let sidecar_object =
        runtime.put_object(&sidecar_bytes, None, "application/json", SIDECAR_KIND)?;
    let mut link = link_record(
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        &worker_cohort,
        &appearance_program_sha256,
        &program_object.record.sha256,
        &pack_manifest_sha256,
        &manifest_object.record.sha256,
        &texture_receipt_object_sha256,
        texture_receipt
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_error("TextureBuild receipt canonical hash is missing"))?,
        &uv_binding_sha256,
        surface_bake_receipt_object_sha256.as_deref(),
        surface_bake_receipt.as_ref(),
        &geometry_program_object_sha256,
        &source_geometry_program_sha256,
        appearance_program,
        &pack_manifest,
        &lods,
        &request_sha256,
        &sidecar_object.record.sha256,
    )?;
    let committed = match runtime.store.record_appearance_source_lineage_link(&link) {
        Ok(value) => value,
        Err(commit_error) => {
            let mut rollback_error = None;
            let mut rollback_objects = vec![&sidecar_object, &manifest_object, &program_object];
            rollback_objects.extend(
                lods.iter()
                    .filter_map(|lod| lod.artifact_readback_object.as_ref()),
            );
            for object in rollback_objects {
                if let Err(error) = runtime
                    .store
                    .discard_new_temporary_appearance_source_lineage_object(object)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(invalid_error(format!(
                    "Appearance source lineage commit failed ({commit_error}); CAS rollback also failed ({rollback_error})"
                )));
            }
            return Err(commit_error.into());
        }
    };
    link = committed;
    Ok(json!({
        "schema_version":PREPARE_RESULT_SCHEMA,
        "sidecar_object_sha256":sidecar_object.record.sha256,
        "sidecar":sidecar,
        "durable_link":link,
        "runtime_write_performed":true,
        "candidate_confirmed":false,
        "export_performed":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "appearance_program_sha256",
            "canonical_sha256",
        ],
        GET_REQUEST_SCHEMA,
    )?;
    if text(object, "schema_version")? != GET_REQUEST_SCHEMA {
        return invalid("Appearance source lineage get schema differs");
    }
    verify_canonical(request, GET_REQUEST_SCHEMA)?;
    let project_id = identifier(object, "project_id")?;
    let candidate_id = identifier(object, "candidate_id")?;
    let appearance_program_sha256 = sha(object, "appearance_program_sha256")?;
    let link = runtime
        .store
        .get_appearance_source_lineage_link(candidate_id, appearance_program_sha256)?
        .ok_or_else(|| invalid_error("durable Appearance source lineage sidecar is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable Appearance source lineage belongs to another project");
    }
    load_link(runtime, &link, GET_RESULT_SCHEMA, false)
}

fn load_link(
    runtime: &Runtime,
    link: &AppearanceSourceLineageLinkRecord,
    result_schema: &str,
    write_performed: bool,
) -> Result<Value, RuntimeError> {
    let program = read_json_object(
        runtime,
        &link.appearance_program_object_sha256,
        SOURCE_PROGRAM_KIND,
    )?;
    validate_appearance_program(&program, &link.project_id)?;
    if !appearance_program_hash_matches(&program, &link.appearance_program_sha256)? {
        return invalid("durable Appearance source program canonical hash differs");
    }
    if program.get("schema_version").and_then(Value::as_str)
        != Some(link.appearance_program_schema_version.as_str())
        || program
            .get("geometry_program_sha256")
            .and_then(Value::as_str)
            != Some(link.geometry_program_sha256.as_str())
        || program
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            != link.material_layer_stack_sha256.as_deref()
    {
        return invalid("durable Appearance source program schema or geometry binding differs");
    }
    ensure_geometry_program_object(runtime, &link.geometry_program_object_sha256)?;
    if program
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        != Some(link.material_pack_manifest_sha256.as_str())
    {
        return invalid("durable Appearance source program MaterialPack binding differs");
    }
    let zones = validate_appearance_program_material_zones(&program)?;
    let manifest = read_json_object(
        runtime,
        &link.material_pack_manifest_object_sha256,
        MATERIAL_PACK_KIND,
    )?;
    if manifest_canonical_hash(&manifest) != link.material_pack_manifest_sha256
        || manifest.get("canonical_sha256").and_then(Value::as_str)
            != Some(link.material_pack_manifest_sha256.as_str())
    {
        return invalid("durable Appearance source MaterialPack manifest binding differs");
    }
    let provenance_sha256 = material_pack_provenance_sha256(&manifest)?;
    if manifest.get("pack_id").and_then(Value::as_str) != Some(link.material_pack_id.as_str())
        || manifest.get("version").and_then(Value::as_str)
            != Some(link.material_pack_version.as_str())
        || manifest.get("license_spdx").and_then(Value::as_str)
            != Some(link.material_pack_license_spdx.as_str())
        || provenance_sha256 != link.material_pack_provenance_sha256
    {
        return invalid("durable MaterialPack identity or provenance differs");
    }
    let texture_receipt = read_json_object(
        runtime,
        &link.texture_build_receipt_object_sha256,
        TEXTURE_RECEIPT_KIND,
    )?;
    validate_texture_receipt(&texture_receipt, &link.material_pack_manifest_sha256)?;
    if texture_receipt
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(link.texture_build_receipt_sha256.as_str())
    {
        return invalid("durable TextureBuild receipt hash differs");
    }
    let surface_bake_receipt =
        if let Some(hash) = link.candidate_surface_bake_receipt_object_sha256.as_deref() {
            let value = read_json_object(runtime, hash, SURFACE_BAKE_RECEIPT_KIND)?;
            validate_surface_bake_receipt(
                &value,
                &link.project_id,
                &link.candidate_id,
                &link.candidate_state_sha256,
                &link.geometry_program_sha256,
                &link.appearance_program_sha256,
                &link.material_pack_manifest_sha256,
                &Value::String(link.texture_build_receipt_object_sha256.clone()),
                &link.uv_binding_sha256,
                &program,
            )?;
            if value.get("canonical_sha256").and_then(Value::as_str)
                != link.candidate_surface_bake_receipt_sha256.as_deref()
            {
                return invalid("durable surface-bake receipt hash differs");
            }
            Some(value)
        } else {
            if link.appearance_program_schema_version == "AppearanceProgram@3" {
                return invalid("durable AppearanceProgram@3 sidecar has no surface-bake receipt");
            }
            None
        };
    let lods = link
        .lod_candidate_ids
        .iter()
        .enumerate()
        .map(|(index, candidate_id)| {
            let level = LodDeclaration {
                level: index as u64,
                candidate_id: candidate_id.clone(),
                candidate_state_sha256: link.lod_candidate_state_sha256s[index].clone(),
                artifact_sha256: link.lod_artifact_sha256s[index].clone(),
                artifact_readback_sha256: link.lod_artifact_readback_sha256s[index].clone(),
                artifact_readback_object_sha256: link.lod_artifact_readback_object_sha256s[index]
                    .clone(),
                part_binding_inventory_sha256: link.lod_part_binding_inventory_sha256s[index]
                    .clone(),
            };
            validate_lod(
                runtime,
                &link.project_id,
                &level,
                &link.candidate_id,
                &link.candidate_state_sha256,
                &link.appearance_program_sha256,
                &link.material_pack_manifest_sha256,
                &link.uv_binding_sha256,
                &zones,
                &link.geometry_program_sha256,
                &link.geometry_program_object_sha256,
                false,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sidecar_bytes =
        runtime.cas_read_bounded(&link.sidecar_object_sha256, MAX_SOURCE_JSON_BYTES)?;
    let sidecar: Value = serde_json::from_slice(&sidecar_bytes).map_err(|error| {
        invalid_error(format!(
            "durable Appearance source sidecar is invalid JSON: {error}"
        ))
    })?;
    validate_sidecar(
        &sidecar,
        link,
        &program,
        &manifest,
        &texture_receipt,
        surface_bake_receipt.as_ref(),
        &lods,
    )?;
    if canonical_json_bytes(&sidecar).map_err(|error| invalid_error(error.to_string()))?
        != sidecar_bytes
    {
        return invalid("durable Appearance source sidecar bytes are not canonical");
    }
    Ok(json!({
        "schema_version":result_schema,
        "sidecar_object_sha256":link.sidecar_object_sha256,
        "sidecar":sidecar,
        "durable_link":link,
        "runtime_write_performed":write_performed,
        "candidate_confirmed":false,
        "export_performed":false,
        "quality_status":"structural_only"
    }))
}

/// Revalidate one already-loaded durable lineage without weakening the
/// public get contract. Candidate material-surface quality uses this after
/// Store has resolved the exact immutable link, so the 2K pack, program,
/// TextureBuild, SurfaceBake and all three LOD CAS objects are independently
/// reread before a quality report can be materialized or returned.
pub(super) fn validate_link(
    runtime: &Runtime,
    link: &AppearanceSourceLineageLinkRecord,
) -> Result<(), RuntimeError> {
    load_link(runtime, link, GET_RESULT_SCHEMA, false).map(|_| ())
}

fn validate_lod(
    runtime: &Runtime,
    project_id: &str,
    level: &LodDeclaration,
    source_candidate_id: &str,
    source_candidate_state_sha256: &str,
    appearance_program_sha256: &str,
    material_pack_manifest_sha256: &str,
    uv_binding_sha256: &str,
    expected_material_zones: &BTreeMap<String, String>,
    geometry_program_sha256: &str,
    geometry_program_object_sha256: &str,
    allow_readback_materialization: bool,
) -> Result<LodBinding, RuntimeError> {
    let candidate = runtime
        .candidate(&level.candidate_id)?
        .ok_or_else(|| invalid_error("Appearance source LOD candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != level.candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(level.artifact_sha256.as_str())
    {
        return invalid("Appearance source LOD candidate/project/state/artifact binding differs");
    }
    if level.level == 0
        && (level.candidate_id != source_candidate_id
            || level.candidate_state_sha256 != source_candidate_state_sha256)
    {
        return invalid("Appearance source LOD0 does not bind the source candidate");
    }
    let readback = runtime.artifact_readback_bounded(
        &level.artifact_sha256,
        &level.candidate_id,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    super::validate_artifact_readback_v2_output(&readback)?;
    let (artifact_readback_object_sha256, artifact_readback_object, readback_object) =
        if let Some(evidence) = runtime
            .store
            .get_geometry_candidate_evidence(&level.candidate_id)?
        {
            if evidence.project_id != project_id
                || (level.level == 0
                    && (evidence.geometry_program_sha256 != geometry_program_sha256
                        || evidence.geometry_program_object_sha256
                            != geometry_program_object_sha256))
                || evidence.artifact_readback_object_sha256.is_empty()
                || (!level.artifact_readback_object_sha256.is_empty()
                    && evidence.artifact_readback_object_sha256
                        != level.artifact_readback_object_sha256)
            {
                return invalid("Appearance source LOD geometry program evidence differs");
            }
            let object = read_json_object_any(
                runtime,
                &evidence.artifact_readback_object_sha256,
                &[
                    "geometry-artifact-readback-v2",
                    "appearance-v2-artifact-readback",
                ],
            )?;
            (evidence.artifact_readback_object_sha256, None, object)
        } else if !level.artifact_readback_object_sha256.is_empty() {
            let object = read_json_object_any(
                runtime,
                &level.artifact_readback_object_sha256,
                &[
                    "geometry-artifact-readback-v2",
                    "appearance-v2-artifact-readback",
                ],
            )?;
            (level.artifact_readback_object_sha256.clone(), None, object)
        } else if allow_readback_materialization {
            let bytes = canonical_json_bytes(&readback)
                .map_err(|error| invalid_error(error.to_string()))?;
            let object = runtime.put_object(
                &bytes,
                None,
                "application/json",
                "appearance-v2-artifact-readback",
            )?;
            (object.record.sha256.clone(), Some(object), readback.clone())
        } else {
            return invalid(
                "durable Appearance source LOD has no read-only ArtifactReadback object binding",
            );
        };
    if readback_object != readback {
        return invalid("Appearance source LOD ArtifactReadback object differs");
    }
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(level.artifact_readback_sha256.as_str())
        || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return invalid("Appearance source LOD strict readback binding differs");
    }
    let bytes = runtime.cas_read_bounded(&level.artifact_sha256, MAX_GEOMETRY_ARTIFACT_BYTES)?;
    let root = glb_root(&bytes)?;
    let forgecad = root
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_error("Appearance source GLB ForgeCAD metadata is missing"))?;
    let actual_geometry_program_sha256 = forgecad
        .get("program_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid_error("Appearance source GLB GeometryProgram hash is missing"))?;
    if level.level == 0 && actual_geometry_program_sha256 != geometry_program_sha256 {
        return invalid("Appearance source LOD GeometryProgram hash differs");
    }
    let actual_program_sha256 = forgecad
        .get("appearance_program_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid_error("Appearance source GLB AppearanceProgram hash is missing"))?;
    if level.level == 0 && actual_program_sha256 != appearance_program_sha256 {
        return invalid("Appearance source LOD AppearanceProgram hash differs");
    }
    let actual_pack_sha256 = forgecad
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid_error("Appearance source GLB MaterialPack hash is missing"))?;
    if actual_pack_sha256 != material_pack_manifest_sha256
        || forgecad.get("material_pack_id").and_then(Value::as_str) != Some(ENERGY_PACK_ID)
    {
        return invalid("Appearance source LOD MaterialPack binding differs");
    }
    let uv_atlas = forgecad
        .get("uv_atlas")
        .ok_or_else(|| invalid_error("Appearance source GLB UV atlas metadata is missing"))?;
    let actual_uv_binding_sha256 = canonical_json_hash(uv_atlas);
    // The request-level UV binding identifies the source (LOD0) Appearance
    // program. Derived LODs may have independently packed atlases; their
    // exact hashes are retained in the per-LOD sidecar inventory below.
    if level.level == 0 && actual_uv_binding_sha256 != uv_binding_sha256 {
        return invalid("Appearance source LOD UV binding differs");
    }
    let material_zones = glb_material_zones(&root)?;
    if &material_zones != expected_material_zones {
        return invalid("Appearance source LOD MaterialZone map differs");
    }
    let part_ids = string_array(&readback, "part_ids")?;
    let source_node_ids = string_array(&readback, "source_node_ids")?;
    let part_bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 512)
        .ok_or_else(|| invalid_error("Appearance source LOD Part binding inventory is invalid"))?
        .clone();
    let part_binding_inventory = json!({
        "part_ids":part_ids,
        "source_node_ids":source_node_ids,
        "material_zone_ids":readback["material_zone_ids"],
        "part_bindings":part_bindings
    });
    let part_binding_inventory_sha256 = canonical_json_hash(&part_binding_inventory);
    if !level.part_binding_inventory_sha256.is_empty()
        && level.part_binding_inventory_sha256 != part_binding_inventory_sha256
    {
        return invalid("Appearance source LOD Part binding inventory hash differs");
    }
    Ok(LodBinding {
        level: level.level,
        candidate_id: level.candidate_id.clone(),
        candidate_state_sha256: level.candidate_state_sha256.clone(),
        artifact_sha256: level.artifact_sha256.clone(),
        artifact_readback_sha256: level.artifact_readback_sha256.clone(),
        artifact_readback_object_sha256,
        artifact_readback_object,
        part_binding_inventory_sha256,
        part_ids,
        source_node_ids,
        part_bindings,
        geometry_program_sha256: actual_geometry_program_sha256.to_owned(),
        appearance_program_sha256: actual_program_sha256.to_owned(),
        material_pack_manifest_sha256: actual_pack_sha256.to_owned(),
        uv_binding_sha256: actual_uv_binding_sha256,
        material_zone_ids: material_zones.keys().cloned().collect(),
    })
}

fn sidecar_value(
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    worker_cohort: &str,
    appearance_program: &Value,
    appearance_program_sha256: &str,
    zones: &BTreeMap<String, String>,
    material_pack_manifest_sha256: &str,
    texture_receipt_object_sha256: &str,
    texture_receipt: &Value,
    uv_binding_sha256: &str,
    surface_bake_receipt_object_sha256: Option<&str>,
    surface_bake_receipt: Option<&Value>,
    geometry_program_sha256: &str,
    geometry_program_object_sha256: &str,
    lods: &[LodBinding],
) -> Result<Value, RuntimeError> {
    let manifest = super::material_pack_manifest_by_id(ENERGY_PACK_ID)
        .ok_or_else(|| invalid_error("MaterialPack manifest is unavailable"))?;
    let provenance_sha256 = material_pack_provenance_sha256(&manifest)?;
    let mut sidecar = json!({
        "schema_version":SIDECAR_SCHEMA,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "candidate_state_sha256":candidate_state_sha256,
        "source_replay_worker_cohort_sha256":worker_cohort,
        "appearance_program_schema_version":appearance_program["schema_version"],
        "appearance_program_sha256":appearance_program_sha256,
        "geometry_program_object_sha256":geometry_program_object_sha256,
        "geometry_program_sha256":geometry_program_sha256,
        "material_layer_stack_sha256":appearance_program.get("material_layer_stack_sha256"),
        "material_pack_id":ENERGY_PACK_ID,
        "material_pack_version":manifest["version"],
        "material_pack_license_spdx":manifest["license_spdx"],
        "material_pack_provenance_sha256":provenance_sha256,
        "material_pack_manifest_sha256":material_pack_manifest_sha256,
        "texture_build_receipt_object_sha256":texture_receipt_object_sha256,
        "texture_build_receipt_sha256":texture_receipt["canonical_sha256"],
        "candidate_surface_bake_receipt_object_sha256":surface_bake_receipt_object_sha256,
        "candidate_surface_bake_receipt_sha256":surface_bake_receipt.map(|value| value["canonical_sha256"].clone()),
        "uv_binding_sha256":uv_binding_sha256,
        "material_zones":zones.iter().map(|(zone, material)| json!({"zone_id":zone,"material_id":material})).collect::<Vec<_>>(),
        "lods":lods.iter().map(|lod| json!({
            "level":lod.level,
            "candidate_id":lod.candidate_id,
            "candidate_state_sha256":lod.candidate_state_sha256,
            "artifact_sha256":lod.artifact_sha256,
            "artifact_readback_sha256":lod.artifact_readback_sha256,
            "artifact_readback_object_sha256":lod.artifact_readback_object_sha256,
            "part_binding_inventory_sha256":lod.part_binding_inventory_sha256,
            "geometry_program_sha256":lod.geometry_program_sha256,
            "appearance_program_sha256":lod.appearance_program_sha256,
            "material_pack_manifest_sha256":lod.material_pack_manifest_sha256,
            "uv_binding_sha256":lod.uv_binding_sha256,
            "material_zone_ids":lod.material_zone_ids,
            "part_ids":lod.part_ids,
            "source_node_ids":lod.source_node_ids,
            "part_bindings":lod.part_bindings
        })).collect::<Vec<_>>(),
        "binding_policy":"candidate-project-cohort-appearance-program-material-pack-texture-build-uv-three-lod-glb@1",
        "materialization_status":"runtime-owned-durable-appearance-source-lineage",
        "candidate_confirmed":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "runtime_write_performed":true,
        "canonical_sha256":""
    });
    sidecar["canonical_sha256"] = Value::String(canonical_hash(&sidecar)?);
    Ok(sidecar)
}

fn link_record(
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    worker_cohort: &str,
    appearance_program_sha256: &str,
    appearance_program_object_sha256: &str,
    material_pack_manifest_sha256: &str,
    material_pack_manifest_object_sha256: &str,
    texture_build_receipt_object_sha256: &str,
    texture_build_receipt_sha256: &str,
    uv_binding_sha256: &str,
    candidate_surface_bake_receipt_object_sha256: Option<&str>,
    candidate_surface_bake_receipt: Option<&Value>,
    geometry_program_object_sha256: &str,
    geometry_program_sha256: &str,
    appearance_program: &Value,
    material_pack_manifest: &Value,
    lods: &[LodBinding],
    request_sha256: &str,
    sidecar_object_sha256: &str,
) -> Result<AppearanceSourceLineageLinkRecord, RuntimeError> {
    let mut link = AppearanceSourceLineageLinkRecord {
        schema_version: LINK_SCHEMA.to_owned(),
        project_id: project_id.to_owned(),
        candidate_id: candidate_id.to_owned(),
        candidate_state_sha256: candidate_state_sha256.to_owned(),
        source_replay_worker_cohort_sha256: worker_cohort.to_owned(),
        appearance_program_schema_version: appearance_program["schema_version"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        appearance_program_object_sha256: appearance_program_object_sha256.to_owned(),
        appearance_program_sha256: appearance_program_sha256.to_owned(),
        geometry_program_object_sha256: geometry_program_object_sha256.to_owned(),
        geometry_program_sha256: geometry_program_sha256.to_owned(),
        material_layer_stack_sha256: appearance_program
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        material_pack_id: material_pack_manifest["pack_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        material_pack_version: material_pack_manifest["version"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        material_pack_license_spdx: material_pack_manifest["license_spdx"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        material_pack_provenance_sha256: material_pack_provenance_sha256(material_pack_manifest)?,
        material_pack_manifest_object_sha256: material_pack_manifest_object_sha256.to_owned(),
        material_pack_manifest_sha256: material_pack_manifest_sha256.to_owned(),
        texture_build_receipt_object_sha256: texture_build_receipt_object_sha256.to_owned(),
        texture_build_receipt_sha256: texture_build_receipt_sha256.to_owned(),
        candidate_surface_bake_receipt_object_sha256: candidate_surface_bake_receipt_object_sha256
            .map(str::to_owned),
        candidate_surface_bake_receipt_sha256: candidate_surface_bake_receipt
            .and_then(|value| value.get("canonical_sha256"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        uv_binding_sha256: uv_binding_sha256.to_owned(),
        lod_candidate_ids: lods.iter().map(|lod| lod.candidate_id.clone()).collect(),
        lod_candidate_state_sha256s: lods
            .iter()
            .map(|lod| lod.candidate_state_sha256.clone())
            .collect(),
        lod_artifact_sha256s: lods.iter().map(|lod| lod.artifact_sha256.clone()).collect(),
        lod_artifact_readback_sha256s: lods
            .iter()
            .map(|lod| lod.artifact_readback_sha256.clone())
            .collect(),
        lod_artifact_readback_object_sha256s: lods
            .iter()
            .map(|lod| lod.artifact_readback_object_sha256.clone())
            .collect(),
        lod_part_binding_inventory_sha256s: lods
            .iter()
            .map(|lod| lod.part_binding_inventory_sha256.clone())
            .collect(),
        request_sha256: request_sha256.to_owned(),
        sidecar_object_sha256: sidecar_object_sha256.to_owned(),
        materialization_status: "runtime-owned-durable-appearance-source-lineage".to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    link.canonical_sha256 = canonical_hash(
        &serde_json::to_value(&link).map_err(|error| invalid_error(error.to_string()))?,
    )?;
    Ok(link)
}

fn validate_sidecar(
    sidecar: &Value,
    link: &AppearanceSourceLineageLinkRecord,
    program: &Value,
    manifest: &Value,
    texture_receipt: &Value,
    surface_bake_receipt: Option<&Value>,
    lods: &[LodBinding],
) -> Result<(), RuntimeError> {
    let object = sidecar
        .as_object()
        .ok_or_else(|| invalid_error("Appearance source sidecar must be an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(SIDECAR_SCHEMA)
        || object.get("project_id").and_then(Value::as_str) != Some(link.project_id.as_str())
        || object.get("candidate_id").and_then(Value::as_str) != Some(link.candidate_id.as_str())
        || object.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(link.candidate_state_sha256.as_str())
        || object
            .get("source_replay_worker_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.source_replay_worker_cohort_sha256.as_str())
        || object
            .get("appearance_program_sha256")
            .and_then(Value::as_str)
            != Some(link.appearance_program_sha256.as_str())
        || object
            .get("appearance_program_schema_version")
            .and_then(Value::as_str)
            != Some(link.appearance_program_schema_version.as_str())
        || object
            .get("geometry_program_object_sha256")
            .and_then(Value::as_str)
            != Some(link.geometry_program_object_sha256.as_str())
        || object
            .get("geometry_program_sha256")
            .and_then(Value::as_str)
            != Some(link.geometry_program_sha256.as_str())
        || object.get("material_layer_stack_sha256")
            != Some(
                &link
                    .material_layer_stack_sha256
                    .as_ref()
                    .map_or(Value::Null, |value| Value::String(value.clone())),
            )
        || object
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            != Some(link.material_pack_manifest_sha256.as_str())
        || object
            .get("texture_build_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(link.texture_build_receipt_object_sha256.as_str())
        || object
            .get("texture_build_receipt_sha256")
            .and_then(Value::as_str)
            != Some(link.texture_build_receipt_sha256.as_str())
        || object.get("uv_binding_sha256").and_then(Value::as_str)
            != Some(link.uv_binding_sha256.as_str())
        || object.get("material_pack_id").and_then(Value::as_str) != Some(ENERGY_PACK_ID)
        || object.get("material_pack_version").and_then(Value::as_str)
            != Some(link.material_pack_version.as_str())
        || object
            .get("material_pack_license_spdx")
            .and_then(Value::as_str)
            != Some(link.material_pack_license_spdx.as_str())
        || object
            .get("material_pack_provenance_sha256")
            .and_then(Value::as_str)
            != Some(link.material_pack_provenance_sha256.as_str())
        || object.get("candidate_surface_bake_receipt_object_sha256")
            != Some(
                &link
                    .candidate_surface_bake_receipt_object_sha256
                    .as_ref()
                    .map_or(Value::Null, |value| Value::String(value.clone())),
            )
        || object.get("candidate_surface_bake_receipt_sha256")
            != Some(
                &link
                    .candidate_surface_bake_receipt_sha256
                    .as_ref()
                    .map_or(Value::Null, |value| Value::String(value.clone())),
            )
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("runtime-owned-durable-appearance-source-lineage")
    {
        return invalid("durable Appearance source sidecar binding differs");
    }
    if canonical_hash(sidecar)?
        != object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return invalid("durable Appearance source sidecar canonical hash differs");
    }
    if !appearance_program_hash_matches(program, &link.appearance_program_sha256)?
        || manifest_canonical_hash(manifest) != link.material_pack_manifest_sha256
        || texture_receipt
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.texture_build_receipt_sha256.as_str())
    {
        return invalid("durable Appearance source sidecar source object hash differs");
    }
    match (
        surface_bake_receipt,
        link.candidate_surface_bake_receipt_sha256.as_deref(),
    ) {
        (Some(receipt), Some(hash))
            if receipt.get("canonical_sha256").and_then(Value::as_str) == Some(hash) => {}
        (None, None) => {}
        _ => return invalid("durable Appearance source sidecar surface-bake binding differs"),
    }
    let sidecar_lods = object
        .get("lods")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_error("durable Appearance source sidecar LOD inventory is missing")
        })?;
    if sidecar_lods.len() != 3 || sidecar_lods.len() != lods.len() {
        return invalid("durable Appearance source sidecar LOD inventory is incomplete");
    }
    for (sidecar_lod, lod) in sidecar_lods.iter().zip(lods) {
        for (key, expected) in [
            ("level", Value::from(lod.level)),
            ("candidate_id", Value::String(lod.candidate_id.clone())),
            (
                "candidate_state_sha256",
                Value::String(lod.candidate_state_sha256.clone()),
            ),
            (
                "artifact_sha256",
                Value::String(lod.artifact_sha256.clone()),
            ),
            (
                "artifact_readback_sha256",
                Value::String(lod.artifact_readback_sha256.clone()),
            ),
            (
                "artifact_readback_object_sha256",
                Value::String(lod.artifact_readback_object_sha256.clone()),
            ),
            (
                "part_binding_inventory_sha256",
                Value::String(lod.part_binding_inventory_sha256.clone()),
            ),
            (
                "geometry_program_sha256",
                Value::String(lod.geometry_program_sha256.clone()),
            ),
            (
                "appearance_program_sha256",
                Value::String(lod.appearance_program_sha256.clone()),
            ),
            (
                "material_pack_manifest_sha256",
                Value::String(lod.material_pack_manifest_sha256.clone()),
            ),
            (
                "uv_binding_sha256",
                Value::String(lod.uv_binding_sha256.clone()),
            ),
        ] {
            if sidecar_lod.get(key) != Some(&expected) {
                return invalid("durable Appearance source sidecar LOD binding differs");
            }
        }
        for (key, expected) in [
            ("part_ids", Value::from(lod.part_ids.clone())),
            ("source_node_ids", Value::from(lod.source_node_ids.clone())),
            ("part_bindings", Value::from(lod.part_bindings.clone())),
        ] {
            if sidecar_lod.get(key) != Some(&expected) {
                return invalid("durable Appearance source sidecar LOD inventory differs");
            }
        }
    }
    Ok(())
}

fn validate_appearance_program(program: &Value, project_id: &str) -> Result<(), RuntimeError> {
    let object = program
        .as_object()
        .ok_or_else(|| invalid_error("Appearance source program must be an object"))?;
    let schema = object
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error("Appearance source program schema_version is missing"))?;
    let is_v2 = schema == "AppearanceProgram@2";
    let is_v3 = schema == "AppearanceProgram@3";
    if !is_v2 && !is_v3 {
        return invalid("Appearance source program schema is unsupported");
    }
    let expected_len = if is_v3 { 9 } else { 7 };
    if object.len() != expected_len
        || object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "schema_version"
                    | "project_id"
                    | "geometry_program_sha256"
                    | "material_pack_id"
                    | "material_pack_manifest_sha256"
                    | "material_zones"
                    | "canonical_sha256"
            ) && !(is_v3
                && matches!(
                    key.as_str(),
                    "material_layer_stack" | "material_layer_stack_sha256"
                ))
        })
        || object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || object.get("material_pack_id").and_then(Value::as_str) != Some(ENERGY_PACK_ID)
        || !object
            .get("geometry_program_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        || !object
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        || !object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
    {
        return invalid("Appearance source program fields are not closed or typed");
    }
    if is_v3
        && !object
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
    {
        return invalid("Appearance source program layer-stack hash is invalid");
    }
    if is_v3 {
        let layer_stack = object
            .get("material_layer_stack")
            .ok_or_else(|| invalid_error("Appearance source MaterialLayerStack is missing"))?;
        if layer_stack
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            != object
                .get("material_pack_manifest_sha256")
                .and_then(Value::as_str)
            || !appearance_program_hash_matches(
                layer_stack,
                object
                    .get("material_layer_stack_sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )?
        {
            return invalid("Appearance source MaterialLayerStack binding differs");
        }
    }
    if !appearance_program_hash_matches(
        program,
        object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )? {
        return invalid("Appearance source program canonical hash differs");
    }
    Ok(())
}

fn validate_appearance_program_material_zones(
    program: &Value,
) -> Result<BTreeMap<String, String>, RuntimeError> {
    let zones = program
        .get("material_zones")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 64)
        .ok_or_else(|| invalid_error("Appearance source material zones are invalid"))?;
    let mut result = BTreeMap::new();
    for zone in zones {
        let object = zone
            .as_object()
            .ok_or_else(|| invalid_error("Appearance source material zone is invalid"))?;
        if object.len() != 4
            || object.keys().any(|key| {
                !["zone_id", "part_ids", "material_id", "texture_set_id"].contains(&key.as_str())
            })
        {
            return invalid("Appearance source material zone fields are not closed");
        }
        let zone_id = object
            .get("zone_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid_error("Appearance source MaterialZone ID is invalid"))?;
        let material_id = object
            .get("material_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid_error("Appearance source material ID is invalid"))?;
        let part_ids = object
            .get("part_ids")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| invalid_error("Appearance source MaterialZone Part IDs are invalid"))?;
        if part_ids
            .iter()
            .any(|value| value.as_str().filter(|id| is_opaque_id(id)).is_none())
            || result
                .insert(zone_id.to_owned(), material_id.to_owned())
                .is_some()
        {
            return invalid("Appearance source MaterialZone IDs are duplicated or invalid");
        }
    }
    Ok(result)
}

fn validate_texture_receipt(
    receipt: &Value,
    material_pack_manifest_sha256: &str,
) -> Result<(), RuntimeError> {
    let schema = receipt
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error("TextureBuild receipt schema is missing"))?;
    if !matches!(schema, "TextureBuildReceipt@1" | "TextureBuildReceipt@2")
        || !receipt
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        || !receipt
            .get("pack_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value == ENERGY_PACK_ID)
        || receipt.get("external_uri").and_then(Value::as_bool) != Some(false)
        || receipt.get("network_at_runtime").and_then(Value::as_bool) != Some(false)
    {
        return invalid("TextureBuild receipt policy is invalid");
    }
    if schema == "TextureBuildReceipt@2"
        && receipt
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            != Some(material_pack_manifest_sha256)
    {
        return invalid("TextureBuild receipt MaterialPack binding differs");
    }
    if canonical_hash(receipt)?
        != receipt
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return invalid("TextureBuild receipt canonical hash differs");
    }
    Ok(())
}

fn material_pack_provenance_sha256(manifest: &Value) -> Result<String, RuntimeError> {
    let object = manifest
        .as_object()
        .ok_or_else(|| invalid_error("MaterialPack manifest must be an object"))?;
    Ok(canonical_json_hash(&json!({
        "pack_id":object.get("pack_id"),
        "version":object.get("version"),
        "publisher":object.get("publisher"),
        "license_spdx":object.get("license_spdx"),
        "source_assets":object.get("source_assets"),
        "source_textures":object.get("source_textures"),
        "derived_outputs":object.get("derived_outputs"),
        "texture_recipe":object.get("texture_recipe")
    })))
}

fn manifest_canonical_hash(manifest: &Value) -> String {
    let mut value = manifest.clone();
    if let Some(object) = value.as_object_mut() {
        object.remove("canonical_sha256");
    }
    canonical_json_hash(&value)
}

fn validate_surface_bake_receipt(
    receipt: &Value,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    geometry_program_sha256: &str,
    appearance_program_sha256: &str,
    material_pack_manifest_sha256: &str,
    texture_receipt_sha256: &Value,
    uv_binding_sha256: &str,
    appearance_program: &Value,
) -> Result<(), RuntimeError> {
    super::validate_candidate_surface_bake_receipt_output(receipt)?;
    let expected_texture = texture_receipt_sha256
        .as_str()
        .ok_or_else(|| invalid_error("TextureBuild receipt canonical hash is invalid"))?;
    let expected_layer_stack = appearance_program
        .get("material_layer_stack_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error("AppearanceProgram MaterialLayerStack hash is missing"))?;
    if receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt.get("source_candidate_id") != Some(&Value::Null)
        || receipt.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || receipt
            .get("candidate_canonical_sha256")
            .and_then(Value::as_str)
            != Some(candidate_state_sha256)
        || receipt
            .get("geometry_program_sha256")
            .and_then(Value::as_str)
            != Some(geometry_program_sha256)
        || receipt
            .get("appearance_program_sha256")
            .and_then(Value::as_str)
            != Some(appearance_program_sha256)
        || receipt.get("material_pack_id").and_then(Value::as_str) != Some(ENERGY_PACK_ID)
        || receipt
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            != Some(material_pack_manifest_sha256)
        || receipt
            .get("input_texture_receipt_sha256")
            .and_then(Value::as_str)
            != Some(expected_texture)
        || receipt.get("uv_binding_sha256").and_then(Value::as_str) != Some(uv_binding_sha256)
        || receipt
            .get("material_layer_stack_sha256")
            .and_then(Value::as_str)
            != Some(expected_layer_stack)
    {
        return invalid("CandidateSurfaceBake receipt binding differs");
    }
    Ok(())
}

fn glb_material_zones(root: &Value) -> Result<BTreeMap<String, String>, RuntimeError> {
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 64)
        .ok_or_else(|| invalid_error("Appearance source GLB materials are invalid"))?;
    let mut result = BTreeMap::new();
    for material in materials {
        let object = material
            .as_object()
            .ok_or_else(|| invalid_error("Appearance source GLB material is invalid"))?;
        let zone_id = object
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid_error("Appearance source GLB MaterialZone name is invalid"))?;
        let metadata = object
            .get("extras")
            .and_then(|value| value.get("forgecad"))
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_error("Appearance source GLB material metadata is missing"))?;
        let material_id = metadata
            .get("material_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid_error("Appearance source GLB stable material ID is invalid"))?;
        if result
            .insert(zone_id.to_owned(), material_id.to_owned())
            .is_some()
        {
            return invalid("Appearance source GLB MaterialZone names are duplicated");
        }
    }
    Ok(result)
}

fn glb_root(bytes: &[u8]) -> Result<Value, RuntimeError> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" || &bytes[16..20] != b"JSON" {
        return invalid("Appearance source GLB header is invalid");
    }
    let json_len = u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .map_err(|_| invalid_error("Appearance source GLB JSON length is invalid"))?,
    ) as usize;
    let json_end = 20usize
        .checked_add(json_len)
        .ok_or_else(|| invalid_error("Appearance source GLB JSON length overflowed"))?;
    if json_end > bytes.len() {
        return invalid("Appearance source GLB JSON chunk is truncated");
    }
    serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| invalid_error(format!("Appearance source GLB JSON is invalid: {error}")))
}

fn read_json_object(
    runtime: &Runtime,
    sha256: &str,
    expected_kind: &str,
) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid_error("Appearance source CAS object is unavailable"))?;
    if record.mime != "application/json"
        || record.kind != expected_kind
        || record.size_bytes == 0
        || record.size_bytes > MAX_SOURCE_JSON_BYTES
    {
        return invalid("Appearance source CAS object kind or size differs");
    }
    let bytes = runtime.cas_read_bounded(sha256, MAX_SOURCE_JSON_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        invalid_error(format!("Appearance source CAS JSON is invalid: {error}"))
    })?;
    if canonical_json_bytes(&value).map_err(|error| invalid_error(error.to_string()))? != bytes {
        return invalid("Appearance source CAS JSON is not canonical");
    }
    Ok(value)
}

fn read_json_object_any(
    runtime: &Runtime,
    sha256: &str,
    expected_kinds: &[&str],
) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid_error("Appearance source CAS object is unavailable"))?;
    if record.mime != "application/json"
        || !expected_kinds.contains(&record.kind.as_str())
        || record.size_bytes == 0
        || record.size_bytes > MAX_SOURCE_JSON_BYTES
    {
        return invalid("Appearance source CAS object kind or size differs");
    }
    let bytes = runtime.cas_read_bounded(sha256, MAX_SOURCE_JSON_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        invalid_error(format!("Appearance source CAS JSON is invalid: {error}"))
    })?;
    if canonical_json_bytes(&value).map_err(|error| invalid_error(error.to_string()))? != bytes {
        return invalid("Appearance source CAS JSON is not canonical");
    }
    Ok(value)
}

fn ensure_geometry_program_object(runtime: &Runtime, sha256: &str) -> Result<(), RuntimeError> {
    let record = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid_error("Appearance source GeometryProgram object is unavailable"))?;
    if record.mime != "application/json"
        || record.kind != "geometry-program-v2"
        || record.size_bytes == 0
        || record.size_bytes > MAX_SOURCE_JSON_BYTES
    {
        return invalid("Appearance source GeometryProgram object kind or size differs");
    }
    let _ = runtime.cas_read_bounded(sha256, MAX_SOURCE_JSON_BYTES)?;
    Ok(())
}

fn string_array(value: &Value, key: &str) -> Result<Vec<String>, RuntimeError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 512)
        .ok_or_else(|| invalid_error(format!("Appearance source {key} inventory is invalid")))?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|id| is_opaque_id(id))
                .map(str::to_owned)
                .ok_or_else(|| invalid_error(format!("Appearance source {key} ID is invalid")))
        })
        .collect()
}

fn exact_lod(value: &Value) -> Result<LodDeclaration, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_error("Appearance source LOD declaration must be an object"))?;
    let required = [
        "level",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_sha256",
        "artifact_readback_sha256",
    ];
    if object.len() != required.len() || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return invalid("Appearance source LOD declaration fields are not closed");
    }
    Ok(LodDeclaration {
        level: object
            .get("level")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid_error("Appearance source LOD level is invalid"))?,
        candidate_id: identifier(object, "candidate_id")?.to_owned(),
        candidate_state_sha256: sha(object, "candidate_state_sha256")?.to_owned(),
        artifact_sha256: sha(object, "artifact_sha256")?.to_owned(),
        artifact_readback_sha256: sha(object, "artifact_readback_sha256")?.to_owned(),
        artifact_readback_object_sha256: String::new(),
        part_binding_inventory_sha256: String::new(),
    })
}

#[derive(Debug, Clone)]
struct LodDeclaration {
    level: u64,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_sha256: String,
    artifact_readback_sha256: String,
    artifact_readback_object_sha256: String,
    part_binding_inventory_sha256: String,
}

fn current_worker_cohort() -> Option<String> {
    build_cohort_sha256().or_else(|| {
        #[cfg(test)]
        {
            Some(super::sha256_hex(WORKER_FALLBACK_COHORT.as_bytes()))
        }
        #[cfg(not(test))]
        {
            None
        }
    })
}

fn ensure_current_worker_cohort(expected: &str) -> Result<(), RuntimeError> {
    if current_worker_cohort().as_deref() != Some(expected) {
        return invalid(
            "Appearance source Worker cohort is unavailable or differs from this Runtime",
        );
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    required: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_error(format!("{context} must be an object")))?;
    if object.len() != required.len()
        || required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !required.contains(&key.as_str()))
    {
        return invalid(format!("{context} has an unexpected field set"));
    }
    Ok(object)
}

fn verify_canonical(value: &Value, context: &str) -> Result<(), RuntimeError> {
    let actual = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid_error(format!("{context}.canonical_sha256 is invalid")))?;
    if canonical_hash(value)? != actual {
        return invalid(format!(
            "{context}.canonical_sha256 does not bind the payload"
        ));
    }
    Ok(())
}

fn canonical_hash(value: &Value) -> Result<String, RuntimeError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid_error("canonical hash value must be an object"))?;
    if !object.contains_key("canonical_sha256") {
        return invalid("canonical_sha256 is missing");
    }
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&preimage))
}

/// AppearanceProgram and MaterialLayerStack predate this sidecar and their
/// existing worker fixtures hash the payload before adding the hash field.
/// Accept that established source hash as well as the Runtime blank-field
/// convention used by newer receipts, while still requiring one exact hash.
fn appearance_program_hash_matches(value: &Value, expected: &str) -> Result<bool, RuntimeError> {
    if !is_sha256(expected) {
        return Ok(false);
    }
    let mut source_preimage = value.clone();
    let source_object = source_preimage
        .as_object_mut()
        .ok_or_else(|| invalid_error("Appearance source hash value must be an object"))?;
    source_object.remove("canonical_sha256");
    if canonical_json_hash(&source_preimage) == expected {
        return Ok(true);
    }
    if value
        .as_object()
        .is_some_and(|object| object.contains_key("canonical_sha256"))
    {
        return Ok(canonical_hash(value)? == expected);
    }
    Ok(false)
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("{key} is required")))
}

fn identifier<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, key)?;
    if !is_opaque_id(value) {
        return invalid(format!("{key} is not an opaque identifier"));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, key)?;
    if !is_sha256(value) {
        return invalid(format!("{key} is not a SHA-256"));
    }
    Ok(value)
}

fn sha_value<'a>(value: &'a Value, key: &str) -> Result<&'a str, RuntimeError> {
    let value = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_error(format!("{key} is required")))?;
    if !is_sha256(value) {
        return invalid(format!("{key} is not a SHA-256"));
    }
    Ok(value)
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "APPEARANCE_SOURCE_LINEAGE_INVALID: {}",
        message.into()
    ))
}
