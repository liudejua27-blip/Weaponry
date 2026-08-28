//! Runtime-owned materialization of one typed foundation import into a
//! Part-bounded AuthoringMesh@2 genesis set.
//!
//! The caller can only name an already persisted foundation import and its
//! CAS roots. Runtime replays the closed embedded importer, derives all mesh
//! identities, writes immutable revision objects, and commits every Part plus
//! one compact descriptor in a single Store transaction. No candidate,
//! version, export, path, URL, source bytes, or script is accepted here.

use forgecad_contracts::{
    AuthoringMeshId, AuthoringMeshLineageId, AuthoringMeshRevision,
    AuthoringMeshV2FoundationSourceBinding, WeaponFoundationAuthoringMaterializationDescriptor,
    WeaponFoundationAuthoringMaterializationDescriptorPartRevision,
    WeaponFoundationAuthoringMaterializationGetRequest,
    WeaponFoundationAuthoringMaterializationGetResult,
    WeaponFoundationAuthoringMaterializationPartSummary,
    WeaponFoundationAuthoringMaterializationPrepareRequest,
    WeaponFoundationAuthoringMaterializationPrepareResult,
    WeaponFoundationAuthoringMaterializationRecord,
    AUTHORING_MESH_V2_FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_REQUEST_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_RESULT_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_LIMITATIONS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_PARTS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_REQUEST_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_RESULT_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RECORD_SCHEMA_VERSION,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RESPONSE_SHAPE,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_SOURCE_ONLY,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STATUS,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STORAGE_POLICY,
    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use forgecad_store::{
    foundation_authoring_mesh_v2_materialization as materialization_store,
    FoundationAuthoringMeshV2MaterializationBatch,
    FoundationAuthoringMeshV2MaterializationDescriptor,
    FoundationAuthoringMeshV2MaterializationRecord, FoundationAuthoringMeshV2PartRevision,
    FoundationAuthoringMeshV2RevisionInput,
};
use serde_json::{json, Value};

use super::authoring_mesh_v2::{AuthoringMeshV2GenesisInput, AuthoringMeshV2Revision};
use super::{
    authoring_mesh_v2_durable, now_string, weapon_foundation_import, weapon_foundation_runtime,
    Runtime, RuntimeError,
};

const BINDING_POLICY: &str = "foundation-import-part-to-authoring-mesh-v2-source@1";
const MAX_FOUNDATION_REVISION_BYTES: u64 = 64 * 1024 * 1024;
const JSON_MIME: &str = "application/json";

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REJECTED: {}",
        message.into()
    ))
}

fn canonical_hash_without<T: serde::Serialize>(
    value: &T,
    field: &str,
) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(value)
        .map_err(|error| invalid(format!("canonical serialization failed: {error}")))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical payload is not an object"))?;
    object.insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}

fn checked_response<T: serde::Serialize>(value: &T) -> Result<(), RuntimeError> {
    let bytes = canonical_json_bytes(
        &serde_json::to_value(value)
            .map_err(|error| invalid(format!("response serialization failed: {error}")))?,
    )
    .map_err(|error| invalid(format!("response canonicalization failed: {error}")))?;
    if bytes.is_empty()
        || bytes.len() as u64 >= WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES
    {
        return Err(invalid("public response must remain strictly below 1 MiB"));
    }
    Ok(())
}

fn parse_prepare(
    value: &Value,
) -> Result<WeaponFoundationAuthoringMaterializationPrepareRequest, RuntimeError> {
    let request: WeaponFoundationAuthoringMaterializationPrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("prepare request is not closed: {error}")))?;
    if request.schema_version
        != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_REQUEST_SCHEMA_VERSION
        || !super::is_opaque_id(&request.project_id)
        || !super::is_opaque_id(&request.foundation_request_id)
        || !super::is_opaque_id(&request.idempotency_key)
        || !super::is_sha256(&request.foundation_request_sha256)
        || !super::is_sha256(&request.foundation_result_object_sha256)
        || !super::is_sha256(&request.topology_object_sha256)
        || !super::is_sha256(&request.socket_map_object_sha256)
        || !super::is_sha256(&request.rig_map_object_sha256)
        || !super::is_sha256(&request.fps_presentation_package_object_sha256)
        || !super::is_sha256(&request.input_sha256)
        || request.materialization_profile != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE
        || request.max_response_bytes
            != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES
        || request.runtime_write_performed
        || request.writer_policy != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY
        || request.canonicalization_policy
            != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY
    {
        return Err(invalid("prepare identity, policy, or hash is invalid"));
    }
    if canonical_hash_without(&request, "input_sha256")? != request.input_sha256 {
        return Err(invalid("input_sha256 does not bind the prepare request"));
    }
    Ok(request)
}

fn parse_get(
    value: &Value,
) -> Result<WeaponFoundationAuthoringMaterializationGetRequest, RuntimeError> {
    let request: WeaponFoundationAuthoringMaterializationGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("get request is not closed: {error}")))?;
    if request.schema_version
        != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_REQUEST_SCHEMA_VERSION
        || !super::is_opaque_id(&request.project_id)
        || !super::is_opaque_id(&request.materialization_id)
        || request
            .descriptor_sha256
            .as_deref()
            .is_some_and(|hash| !super::is_sha256(hash))
        || request.writer_policy != WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || !super::is_sha256(&request.input_sha256)
    {
        return Err(invalid("get identity, policy, or hash is invalid"));
    }
    if canonical_hash_without(&request, "input_sha256")? != request.input_sha256 {
        return Err(invalid("input_sha256 does not bind the get request"));
    }
    Ok(request)
}

fn short_id(prefix: &str, value: &Value) -> String {
    let hash = canonical_json_hash(value);
    format!("{prefix}-{}", &hash[..24])
}

fn material_zone_id(mesh: &weapon_foundation_import::FoundationMesh) -> String {
    short_id(
        "material-zone",
        &json!({"part_id":mesh.part_id,"face_material_indices":mesh.face_material_indices}),
    )
}

fn stable_positions(positions: &[[f64; 3]]) -> Result<Vec<[f64; 3]>, RuntimeError> {
    let quantize = |value: f64| -> Result<f64, RuntimeError> {
        format!("{value:.9}")
            .parse::<f64>()
            .map_err(|error| invalid(format!("position quantization failed: {error}")))
    };
    let mut current = positions
        .iter()
        .map(|position| {
            Ok([
                quantize(position[0])?,
                quantize(position[1])?,
                quantize(position[2])?,
            ])
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    for _ in 0..8 {
        let bytes = canonical_json_bytes(
            &serde_json::to_value(&current).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let parsed: Vec<[f64; 3]> = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("position normalization failed: {error}")))?;
        let next = parsed
            .iter()
            .map(|position| {
                Ok([
                    quantize(position[0])?,
                    quantize(position[1])?,
                    quantize(position[2])?,
                ])
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let next_bytes = canonical_json_bytes(
            &serde_json::to_value(&next).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?;
        if next_bytes == bytes {
            return Ok(next);
        }
        current = next;
    }
    Err(invalid(
        "foundation positions did not reach a bounded JSON fixed point",
    ))
}

fn prepare_hash_from_aggregate(
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
) -> Result<String, RuntimeError> {
    let mut request = WeaponFoundationAuthoringMaterializationPrepareRequest {
        schema_version: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_REQUEST_SCHEMA_VERSION
            .to_owned(),
        project_id: aggregate.project_id.clone(),
        foundation_request_id: aggregate.foundation_request_id.clone(),
        foundation_request_sha256: aggregate.foundation_request_sha256.clone(),
        foundation_result_object_sha256: aggregate.foundation_result_object_sha256.clone(),
        topology_object_sha256: aggregate.foundation_topology_object_sha256.clone(),
        socket_map_object_sha256: aggregate.foundation_socket_map_object_sha256.clone(),
        rig_map_object_sha256: aggregate.foundation_rig_map_object_sha256.clone(),
        fps_presentation_package_object_sha256: aggregate
            .foundation_fps_presentation_package_object_sha256
            .clone(),
        materialization_profile: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE.to_owned(),
        idempotency_key: aggregate.idempotency_key.clone(),
        max_response_bytes: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES,
        runtime_write_performed: false,
        writer_policy: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY.to_owned(),
        canonicalization_policy:
            WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY.to_owned(),
        input_sha256: String::new(),
    };
    request.input_sha256 = canonical_hash_without(&request, "input_sha256")?;
    Ok(request.input_sha256)
}

fn public_descriptor(
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> WeaponFoundationAuthoringMaterializationDescriptor {
    WeaponFoundationAuthoringMaterializationDescriptor {
        schema_version: descriptor.schema_version.clone(),
        project_id: descriptor.project_id.clone(),
        foundation_request_id: descriptor.foundation_request_id.clone(),
        foundation_request_sha256: descriptor.foundation_request_sha256.clone(),
        foundation_result_object_sha256: descriptor.foundation_result_object_sha256.clone(),
        foundation_topology_object_sha256: descriptor.foundation_topology_object_sha256.clone(),
        foundation_socket_map_object_sha256: descriptor.foundation_socket_map_object_sha256.clone(),
        foundation_rig_map_object_sha256: descriptor.foundation_rig_map_object_sha256.clone(),
        foundation_fps_presentation_package_object_sha256: descriptor
            .foundation_fps_presentation_package_object_sha256
            .clone(),
        part_revisions: descriptor
            .part_revisions
            .iter()
            .map(
                |part| WeaponFoundationAuthoringMaterializationDescriptorPartRevision {
                    part_id: part.part_id.clone(),
                    mesh_id: part.mesh_id.clone(),
                    lineage_id: part.lineage_id.clone(),
                    revision_id: part.revision_id.clone(),
                    idempotency_key: part.idempotency_key.clone(),
                    revision_object_sha256: part.revision_object_sha256.clone(),
                    revision_sha256: part.revision_sha256.clone(),
                    vertex_count: part.vertex_count,
                    face_count: part.face_count,
                },
            )
            .collect(),
        part_revision_summary_sha256: descriptor.part_revision_summary_sha256.clone(),
        part_count: descriptor.part_count,
        vertex_count: descriptor.vertex_count,
        face_count: descriptor.face_count,
        status: descriptor.status.clone(),
        canonical_sha256: descriptor.canonical_sha256.clone(),
    }
}

fn read_parts(
    runtime: &Runtime,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> Result<Vec<WeaponFoundationAuthoringMaterializationPartSummary>, RuntimeError> {
    let mut parts = Vec::with_capacity(descriptor.part_revisions.len());
    for part in &descriptor.part_revisions {
        let bytes = runtime
            .cas_read_bounded(&part.revision_object_sha256, MAX_FOUNDATION_REVISION_BYTES)?;
        let revision: AuthoringMeshRevision = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("revision CAS JSON is invalid: {error}")))?;
        let kernel = AuthoringMeshV2Revision::from_record(revision.clone())?;
        let record = kernel.record();
        let binding = record
            .foundation_source_binding
            .as_ref()
            .ok_or_else(|| invalid("materialized revision lost foundation provenance"))?;
        if record.mesh_id.0 != part.mesh_id
            || record.lineage_id.0 != part.lineage_id
            || record.revision_id.0 != part.revision_id
            || record.canonical_sha256 != part.revision_sha256
            || binding.part_id != part.part_id
        {
            return Err(invalid("descriptor and revision Part binding differ"));
        }
        let original = &record.original;
        let to_u32 = |value: usize, label: &str| {
            u32::try_from(value).map_err(|_| invalid(format!("{label} count exceeds u32")))
        };
        parts.push(WeaponFoundationAuthoringMaterializationPartSummary {
            part_id: part.part_id.clone(),
            material_zone_id: binding.material_zone_id.clone(),
            source_part_topology_sha256: binding.source_part_topology_sha256.clone(),
            authoring_mesh_id: part.mesh_id.clone(),
            authoring_mesh_object_sha256: part.revision_object_sha256.clone(),
            authoring_mesh_sha256: original.canonical_sha256.clone(),
            authoring_mesh_lineage_id: part.lineage_id.clone(),
            authoring_mesh_lineage_sha256: sha256_hex(part.lineage_id.as_bytes()),
            authoring_mesh_revision_id: part.revision_id.clone(),
            authoring_mesh_revision_sha256: part.revision_sha256.clone(),
            source_binding_sha256: binding.canonical_sha256.clone(),
            vertex_count: to_u32(original.vertices.len(), "vertex")?,
            edge_count: to_u32(original.edges.len(), "edge")?,
            half_edge_count: to_u32(original.half_edges.len(), "half-edge")?,
            corner_count: to_u32(original.corners.len(), "corner")?,
            face_count: to_u32(original.faces.len(), "face")?,
            loop_count: to_u32(original.loops.len(), "loop")?,
            ring_count: to_u32(original.rings.len(), "ring")?,
            source_triangle_count: to_u32(original.faces.len(), "source triangle")?,
            sanitized_triangle_count: to_u32(original.faces.len(), "sanitized triangle")?,
        });
    }
    Ok(parts)
}

fn public_record(
    runtime: &Runtime,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
) -> Result<WeaponFoundationAuthoringMaterializationRecord, RuntimeError> {
    let foundation = runtime
        .store
        .get_weapon_foundation_import(
            &aggregate.foundation_request_id,
            Some(&aggregate.foundation_request_sha256),
        )?
        .ok_or_else(|| invalid("foundation import disappeared after materialization"))?;
    let parts = read_parts(runtime, descriptor)?;
    let mut record = WeaponFoundationAuthoringMaterializationRecord {
        schema_version: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RECORD_SCHEMA_VERSION
            .to_owned(),
        record_id: short_id(
            "foundation-authoring-record",
            &json!({"project_id":aggregate.project_id,"foundation_request_sha256":aggregate.foundation_request_sha256,"descriptor_object_sha256":aggregate.descriptor_object_sha256}),
        ),
        materialization_id: aggregate.idempotency_key.clone(),
        project_id: aggregate.project_id.clone(),
        descriptor_object_sha256: aggregate.descriptor_object_sha256.clone(),
        descriptor_sha256: aggregate.descriptor_canonical_sha256.clone(),
        foundation_request_id: aggregate.foundation_request_id.clone(),
        foundation_request_sha256: aggregate.foundation_request_sha256.clone(),
        foundation_result_object_sha256: aggregate.foundation_result_object_sha256.clone(),
        topology_object_sha256: aggregate.foundation_topology_object_sha256.clone(),
        socket_map_object_sha256: aggregate.foundation_socket_map_object_sha256.clone(),
        rig_map_object_sha256: aggregate.foundation_rig_map_object_sha256.clone(),
        fps_presentation_package_object_sha256: aggregate
            .foundation_fps_presentation_package_object_sha256
            .clone(),
        source_asset_id: foundation.asset_id,
        source_asset_sha256: foundation.asset_sha256,
        source_asset_role: foundation.asset_role,
        materialization_profile: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE.to_owned(),
        source_only: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_SOURCE_ONLY,
        part_count: u32::try_from(parts.len()).map_err(|_| invalid("Part count exceeds u32"))?,
        parts,
        materialization_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STATUS.to_owned(),
        quality_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS.to_owned(),
        review_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS.to_owned(),
        storage_policy: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STORAGE_POLICY.to_owned(),
        writer_policy: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY.to_owned(),
        runtime_write_performed: true,
        persistent_user_data_touched: true,
        request_input_sha256: prepare_hash_from_aggregate(aggregate)?,
        idempotency_key: aggregate.idempotency_key.clone(),
        limitations: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_LIMITATIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        canonicalization_policy:
            WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY.to_owned(),
        canonical_sha256: String::new(),
        created_at: aggregate.created_at.clone(),
    };
    record.canonical_sha256 = canonical_hash_without(&record, "canonical_sha256")?;
    Ok(record)
}

fn result_from_persisted(
    runtime: &Runtime,
    aggregate: &FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: &FoundationAuthoringMeshV2MaterializationDescriptor,
    request_input_sha256: String,
    replayed: bool,
    write_performed: bool,
    get: bool,
) -> Result<Value, RuntimeError> {
    let public_descriptor = public_descriptor(descriptor);
    let record = public_record(runtime, aggregate, descriptor)?;
    if get {
        let mut result = WeaponFoundationAuthoringMaterializationGetResult {
            schema_version: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_GET_RESULT_SCHEMA_VERSION
                .to_owned(),
            project_id: aggregate.project_id.clone(),
            materialization_id: aggregate.idempotency_key.clone(),
            descriptor_object_sha256: aggregate.descriptor_object_sha256.clone(),
            descriptor_sha256: descriptor.canonical_sha256.clone(),
            descriptor: public_descriptor,
            record_sha256: record.canonical_sha256.clone(),
            record,
            request_input_sha256,
            max_response_bytes: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES,
            materialization_profile: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE.to_owned(),
            source_only: true,
            materialization_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STATUS.to_owned(),
            quality_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS.to_owned(),
            review_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS.to_owned(),
            response_shape: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RESPONSE_SHAPE.to_owned(),
            replayed,
            restart_hash_verified: true,
            runtime_write_performed: write_performed,
            persistent_user_data_touched: write_performed,
            writer_policy: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY.to_owned(),
            limitations: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_LIMITATIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            canonicalization_policy:
                WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        result.canonical_sha256 = canonical_hash_without(&result, "canonical_sha256")?;
        checked_response(&result)?;
        serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
    } else {
        let mut result = WeaponFoundationAuthoringMaterializationPrepareResult {
            schema_version:
                WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PREPARE_RESULT_SCHEMA_VERSION.to_owned(),
            project_id: aggregate.project_id.clone(),
            materialization_id: aggregate.idempotency_key.clone(),
            descriptor_object_sha256: aggregate.descriptor_object_sha256.clone(),
            descriptor_sha256: descriptor.canonical_sha256.clone(),
            descriptor: public_descriptor,
            record_sha256: record.canonical_sha256.clone(),
            record,
            request_input_sha256,
            idempotency_key: aggregate.idempotency_key.clone(),
            max_response_bytes: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_RESPONSE_BYTES,
            materialization_profile: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE.to_owned(),
            source_only: true,
            materialization_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_STATUS.to_owned(),
            quality_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS.to_owned(),
            review_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS.to_owned(),
            response_shape: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_RESPONSE_SHAPE.to_owned(),
            replayed,
            restart_hash_verified: true,
            runtime_write_performed: write_performed,
            persistent_user_data_touched: write_performed,
            writer_policy: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_WRITER_POLICY.to_owned(),
            limitations: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_LIMITATIONS
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            canonicalization_policy:
                WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        result.canonical_sha256 = canonical_hash_without(&result, "canonical_sha256")?;
        checked_response(&result)?;
        serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
    }
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    let foundation = runtime
        .store
        .get_weapon_foundation_import(
            &request.foundation_request_id,
            Some(&request.foundation_request_sha256),
        )?
        .ok_or_else(|| invalid("foundation import is not durable"))?;
    if foundation.result_object_sha256 != request.foundation_result_object_sha256
        || foundation.topology_object_sha256 != request.topology_object_sha256
        || foundation.socket_map_object_sha256 != request.socket_map_object_sha256
        || foundation.rig_map_object_sha256 != request.rig_map_object_sha256
        || foundation.fps_presentation_package_object_sha256
            != request.fps_presentation_package_object_sha256
    {
        return Err(invalid(
            "prepare hashes differ from the immutable foundation import",
        ));
    }
    let asset = weapon_foundation_runtime::builtin_asset(&foundation.asset_id)
        .ok_or_else(|| invalid("foundation asset has no closed Runtime importer"))?;
    let source_bytes = weapon_foundation_runtime::builtin_asset_bytes(asset);
    if sha256_hex(source_bytes) != foundation.asset_sha256 {
        return Err(invalid("embedded foundation source hash differs"));
    }
    let imported = weapon_foundation_import::import_builtin_weapon_foundation(asset, source_bytes)
        .map_err(|error| invalid(format!("foundation replay failed: {error}")))?;
    if imported.meshes.is_empty()
        || imported.meshes.len() > WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_MAX_PARTS as usize
    {
        return Err(invalid(
            "foundation Part count is outside materialization bounds",
        ));
    }

    let materialization_id = request.idempotency_key.clone();
    let record_id = short_id(
        "foundation-authoring-record",
        &json!({"project_id":request.project_id,"request_input_sha256":request.input_sha256}),
    );
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved = Vec::new();
    let work = (|| -> Result<_, RuntimeError> {
        let mut revision_inputs = Vec::new();
        let mut descriptor_parts = Vec::new();
        for mesh in &imported.meshes {
            let positions_m = stable_positions(&mesh.positions_m)?;
            let lineage_id = short_id(
                "foundation-lineage",
                &json!({"project_id":request.project_id,"foundation_request_sha256":request.foundation_request_sha256,"part_id":mesh.part_id,"topology_sha256":mesh.topology.topology_sha256}),
            );
            let faces = mesh
                .faces
                .iter()
                .map(|face| face.iter().map(|index| *index as usize).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            let provisional = AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
                mesh_id: AuthoringMeshId(mesh.mesh_id.clone()),
                lineage_id: AuthoringMeshLineageId(lineage_id.clone()),
                positions_m: positions_m.clone(),
                faces: faces.clone(),
                evaluated: None,
                source_binding: None,
                foundation_source_binding: None,
            })?;
            let mut binding = AuthoringMeshV2FoundationSourceBinding {
                schema_version: AUTHORING_MESH_V2_FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION
                    .to_owned(),
                project_id: request.project_id.clone(),
                materialization_id: materialization_id.clone(),
                record_id: record_id.clone(),
                foundation_request_id: request.foundation_request_id.clone(),
                foundation_request_sha256: request.foundation_request_sha256.clone(),
                foundation_result_object_sha256: request.foundation_result_object_sha256.clone(),
                topology_object_sha256: request.topology_object_sha256.clone(),
                socket_map_object_sha256: request.socket_map_object_sha256.clone(),
                rig_map_object_sha256: request.rig_map_object_sha256.clone(),
                fps_presentation_package_object_sha256: request
                    .fps_presentation_package_object_sha256
                    .clone(),
                source_asset_id: foundation.asset_id.clone(),
                source_asset_sha256: foundation.asset_sha256.clone(),
                source_asset_role: foundation.asset_role.clone(),
                part_id: mesh.part_id.clone(),
                material_zone_id: material_zone_id(mesh),
                source_part_topology_sha256: mesh.topology.topology_sha256.clone(),
                authoring_mesh_id: mesh.mesh_id.clone(),
                authoring_mesh_lineage_id: lineage_id.clone(),
                authoring_mesh_revision_id: provisional.record().revision_id.0.clone(),
                binding_policy: BINDING_POLICY.to_owned(),
                materialization_profile: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_PROFILE
                    .to_owned(),
                source_only: true,
                quality_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_QUALITY_STATUS
                    .to_owned(),
                review_status: WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_REVIEW_STATUS.to_owned(),
                canonicalization_policy:
                    WEAPON_FOUNDATION_AUTHORING_MATERIALIZATION_CANONICALIZATION_POLICY.to_owned(),
                canonical_sha256: String::new(),
            };
            binding.canonical_sha256 = canonical_hash_without(&binding, "canonical_sha256")?;
            let revision = AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
                mesh_id: AuthoringMeshId(mesh.mesh_id.clone()),
                lineage_id: AuthoringMeshLineageId(lineage_id),
                positions_m,
                faces,
                evaluated: None,
                source_binding: None,
                foundation_source_binding: Some(binding),
            })?;
            let revision = revision.record().clone();
            if revision.revision_id != provisional.record().revision_id {
                return Err(invalid(
                    "foundation provenance changed the topology revision identity",
                ));
            }
            let revision_bytes = canonical_json_bytes(
                &serde_json::to_value(&revision).map_err(|error| invalid(error.to_string()))?,
            )
            .map_err(|error| invalid(error.to_string()))?;
            if revision_bytes.is_empty()
                || revision_bytes.len() as u64 > MAX_FOUNDATION_REVISION_BYTES
            {
                return Err(invalid(
                    "foundation revision exceeds the 64 MiB internal bound",
                ));
            }
            let object = runtime.store.put_object_reserved(
                &reservation,
                &revision_bytes,
                None,
                JSON_MIME,
                forgecad_store::AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                &now_string(),
            )?;
            reserved.push(object.clone());
            let part_idempotency = short_id(
                "foundation-part",
                &json!({"materialization_id":materialization_id,"part_id":mesh.part_id}),
            );
            let durable = authoring_mesh_v2_durable::durable_record_for(
                &request.project_id,
                &revision,
                &object.record.sha256,
                &request.input_sha256,
                &part_idempotency,
            )?;
            let descriptor_part = FoundationAuthoringMeshV2PartRevision {
                part_id: mesh.part_id.clone(),
                mesh_id: revision.mesh_id.0.clone(),
                lineage_id: revision.lineage_id.0.clone(),
                revision_id: revision.revision_id.0.clone(),
                idempotency_key: part_idempotency,
                revision_object_sha256: object.record.sha256.clone(),
                revision_sha256: revision.canonical_sha256.clone(),
                vertex_count: revision.original.vertices.len() as u64,
                face_count: revision.original.faces.len() as u64,
            };
            revision_inputs.push(FoundationAuthoringMeshV2RevisionInput {
                part_id: mesh.part_id.clone(),
                record: durable,
                revision,
                object: object.record.clone(),
            });
            descriptor_parts.push(descriptor_part);
        }
        descriptor_parts.sort_by(|left, right| left.part_id.cmp(&right.part_id));
        revision_inputs.sort_by(|left, right| left.part_id.cmp(&right.part_id));
        let vertex_count = descriptor_parts
            .iter()
            .map(|part| part.vertex_count)
            .sum::<u64>();
        let face_count = descriptor_parts
            .iter()
            .map(|part| part.face_count)
            .sum::<u64>();
        let part_summary_hash = canonical_json_hash(
            &serde_json::to_value(&descriptor_parts).map_err(|error| invalid(error.to_string()))?,
        );
        let mut descriptor = FoundationAuthoringMeshV2MaterializationDescriptor {
            schema_version: materialization_store::DESCRIPTOR_SCHEMA_VERSION.to_owned(),
            project_id: request.project_id.clone(),
            foundation_request_id: request.foundation_request_id.clone(),
            foundation_request_sha256: request.foundation_request_sha256.clone(),
            foundation_result_object_sha256: request.foundation_result_object_sha256.clone(),
            foundation_topology_object_sha256: request.topology_object_sha256.clone(),
            foundation_socket_map_object_sha256: request.socket_map_object_sha256.clone(),
            foundation_rig_map_object_sha256: request.rig_map_object_sha256.clone(),
            foundation_fps_presentation_package_object_sha256: request
                .fps_presentation_package_object_sha256
                .clone(),
            part_revisions: descriptor_parts,
            part_revision_summary_sha256: part_summary_hash,
            part_count: revision_inputs.len() as u64,
            vertex_count,
            face_count,
            status: materialization_store::STATUS.to_owned(),
            canonical_sha256: String::new(),
        };
        descriptor.canonical_sha256 = canonical_hash_without(&descriptor, "canonical_sha256")?;
        let descriptor_bytes = canonical_json_bytes(
            &serde_json::to_value(&descriptor).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let descriptor_object = runtime.store.put_object_reserved(
            &reservation,
            &descriptor_bytes,
            None,
            materialization_store::JSON_MIME,
            materialization_store::DESCRIPTOR_OBJECT_KIND,
            &now_string(),
        )?;
        reserved.push(descriptor_object.clone());
        let mut aggregate = FoundationAuthoringMeshV2MaterializationRecord {
            schema_version: materialization_store::RECORD_SCHEMA_VERSION.to_owned(),
            project_id: request.project_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            foundation_request_id: request.foundation_request_id.clone(),
            foundation_request_sha256: request.foundation_request_sha256.clone(),
            foundation_result_object_sha256: request.foundation_result_object_sha256.clone(),
            foundation_topology_object_sha256: request.topology_object_sha256.clone(),
            foundation_socket_map_object_sha256: request.socket_map_object_sha256.clone(),
            foundation_rig_map_object_sha256: request.rig_map_object_sha256.clone(),
            foundation_fps_presentation_package_object_sha256: request
                .fps_presentation_package_object_sha256
                .clone(),
            descriptor_object_sha256: descriptor_object.record.sha256.clone(),
            descriptor_canonical_sha256: descriptor.canonical_sha256.clone(),
            part_revision_summary_sha256: descriptor.part_revision_summary_sha256.clone(),
            part_count: descriptor.part_count,
            vertex_count,
            face_count,
            status: materialization_store::STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        aggregate.canonical_sha256 = canonical_hash_without(&aggregate, "canonical_sha256")?;
        let batch = FoundationAuthoringMeshV2MaterializationBatch {
            record: aggregate,
            descriptor: descriptor.clone(),
            descriptor_object: descriptor_object.record.clone(),
            revisions: revision_inputs,
        };
        for item in &batch.revisions {
            let expected = canonical_json_bytes(
                &serde_json::to_value(&item.revision)
                    .map_err(|error| invalid(error.to_string()))?,
            )
            .map_err(|error| invalid(error.to_string()))?;
            if sha256_hex(&expected) != item.object.sha256 {
                return Err(invalid(format!(
                    "revision changed before atomic Store commit (cas={}, typed={})",
                    item.object.sha256,
                    sha256_hex(&expected)
                )));
            }
        }
        let (stored, replayed) = runtime
            .store
            .record_foundation_authoring_mesh_v2_materialization_with_replay(&batch)?;
        Ok((stored, descriptor, replayed))
    })();
    match work {
        Ok((aggregate, descriptor, replayed)) => {
            for object in &reserved {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            result_from_persisted(
                runtime,
                &aggregate,
                &descriptor,
                request.input_sha256,
                replayed,
                !replayed,
                false,
            )
        }
        Err(error) => {
            for object in &reserved {
                let _ = runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, true);
            }
            Err(error)
        }
    }
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let aggregate = runtime
        .store
        .get_foundation_authoring_mesh_v2_materialization(
            &request.project_id,
            &request.materialization_id,
        )?
        .ok_or_else(|| invalid("materialization is not durable"))?;
    let descriptor_bytes = runtime.cas_read_bounded(
        &aggregate.descriptor_object_sha256,
        materialization_store::MAX_JSON_BYTES,
    )?;
    let descriptor: FoundationAuthoringMeshV2MaterializationDescriptor =
        serde_json::from_slice(&descriptor_bytes)
            .map_err(|error| invalid(format!("descriptor CAS JSON is invalid: {error}")))?;
    if request
        .descriptor_sha256
        .as_deref()
        .is_some_and(|hash| hash != descriptor.canonical_sha256)
    {
        return Err(invalid(
            "get descriptor hash differs from durable materialization",
        ));
    }
    result_from_persisted(
        runtime,
        &aggregate,
        &descriptor,
        request.input_sha256,
        true,
        false,
        true,
    )
}
