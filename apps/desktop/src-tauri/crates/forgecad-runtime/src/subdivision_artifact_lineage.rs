//! Candidate/artifact-bound Subdivision root-lineage projection and sidecar.
//!
//! The persisted GeometryProgram and strict ArtifactReadback remain the
//! durable truth. The read-only projection replays that exact program through
//! the fixed Geometry Worker and requires byte-for-byte equality with the CAS
//! GLB before mapping evaluated quads to source-primitive-local triangles. An
//! explicit write-opt-in prepare may then materialize the already-validated
//! projection as an immutable CAS sidecar and a separate durable lookup link.

use super::{
    canonical_json_bytes, canonical_json_hash, compile_geometry_with_runtime_worker, exact_object,
    is_opaque_id, is_sha256, now_string, sha256_hex, strict_glb_inspection,
    validate_worker_metadata, verify_output_canonical_hash, Runtime, RuntimeError,
    MAX_DERIVED_JSON_BYTES, MAX_GEOMETRY_ARTIFACT_BYTES,
};
use forgecad_contracts::SubdivisionArtifactLineageLinkRecord;
use serde_json::{json, Map, Value};

const REQUEST_SCHEMA: &str = "SubdivisionArtifactLineageRequest@1";
const RESULT_SCHEMA: &str = "SubdivisionArtifactLineageProjection@1";
const SIDECAR_REQUEST_SCHEMA: &str = "SubdivisionArtifactLineageSidecarRequest@1";
const SIDECAR_SCHEMA: &str = "SubdivisionArtifactLineageSidecar@1";
const SIDECAR_LINK_SCHEMA: &str = "SubdivisionArtifactLineageLink@1";
const MAX_LINEAGE_ELEMENTS: u64 = 25_000;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const LIMITATIONS: [&str; 8] = [
    "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
    "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
    "SOURCE_PRIMITIVE_LOCAL_TRIANGLE_IDS_ONLY",
    "NO_GLTF_VERTEX_EDGE_OR_CORNER_IDENTITY",
    "NO_CROSS_VERSION_ELEMENT_ID_STABILITY",
    "PROJECTION_NOT_PERSISTED_AS_A_CAS_SIDECAR",
    "DETERMINISTIC_FULL_GLB_BYTE_REPLAY_REQUIRED",
    "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
];
const SIDECAR_LIMITATIONS: [&str; 8] = [
    "REGULAR_RECTANGULAR_OPEN_QUAD_GRID_ONLY",
    "INTEGER_EDGE_SHARPNESS_LEVELS_1_TO_2_ONLY",
    "SOURCE_PRIMITIVE_LOCAL_TRIANGLE_IDS_ONLY",
    "NO_GLTF_VERTEX_EDGE_OR_CORNER_IDENTITY",
    "NO_CROSS_VERSION_ELEMENT_ID_STABILITY",
    "IMMUTABLE_CAS_SIDECAR_NO_CROSS_VERSION_STABILITY",
    "DETERMINISTIC_FULL_GLB_BYTE_REPLAY_REQUIRED",
    "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
];

impl Runtime {
    pub fn subdivision_artifact_lineage_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = exact_object(
            &request,
            &[
                "schema_version",
                "project_id",
                "candidate_id",
                "artifact_id",
                "artifact_readback_sha256",
                "subdivision_node_id",
                "max_lineage_elements",
                "canonical_sha256",
            ],
            REQUEST_SCHEMA,
        )?;
        if object.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA) {
            return Err(artifact_lineage_error("request schema_version differs"));
        }
        verify_output_canonical_hash(&request, REQUEST_SCHEMA)?;
        let project_id = required_id(object, "project_id")?;
        let candidate_id = required_id(object, "candidate_id")?;
        let artifact_id = required_hash(object, "artifact_id")?;
        let artifact_readback_sha256 = required_hash(object, "artifact_readback_sha256")?;
        let subdivision_node_id = required_id(object, "subdivision_node_id")?;
        let max_lineage_elements = object
            .get("max_lineage_elements")
            .and_then(Value::as_u64)
            .filter(|value| (1..=MAX_LINEAGE_ELEMENTS).contains(value))
            .ok_or_else(|| artifact_lineage_error("max_lineage_elements is outside 1..25000"))?;

        let candidate = self
            .candidate(&candidate_id)?
            .ok_or_else(|| artifact_lineage_error("candidate is unavailable"))?;
        if candidate.project_id != project_id {
            return Err(artifact_lineage_error(
                "candidate belongs to another project",
            ));
        }
        if candidate.prepared_object_sha256.as_deref() != Some(artifact_id.as_str()) {
            return Err(artifact_lineage_error(
                "candidate prepared artifact differs",
            ));
        }
        let evidence = self
            .store
            .get_geometry_candidate_evidence(&candidate_id)?
            .ok_or_else(|| artifact_lineage_error("durable V2 geometry evidence is unavailable"))?;
        if evidence.project_id != project_id || evidence.artifact_object_sha256 != artifact_id {
            return Err(artifact_lineage_error(
                "durable geometry evidence binding differs",
            ));
        }

        let artifact_bytes = self.cas_read_bounded(&artifact_id, MAX_GEOMETRY_ARTIFACT_BYTES)?;
        let inspection = strict_glb_inspection(&artifact_bytes)?;
        self.revalidate_v2_geometry_evidence(&candidate, &inspection, &evidence)?;
        let readback = self.artifact_readback(&artifact_id, &candidate_id)?;
        if readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(artifact_readback_sha256.as_str())
        {
            return Err(artifact_lineage_error(
                "caller ArtifactReadback canonical hash differs",
            ));
        }

        let mut program: Value = serde_json::from_slice(&self.cas_read_bounded(
            &evidence.geometry_program_object_sha256,
            MAX_DERIVED_JSON_BYTES,
        )?)
        .map_err(|_| artifact_lineage_error("persisted GeometryProgram draft is not JSON"))?;
        let program_object = program
            .as_object_mut()
            .ok_or_else(|| artifact_lineage_error("persisted GeometryProgram draft is invalid"))?;
        if program_object.contains_key("canonical_sha256") {
            return Err(artifact_lineage_error(
                "persisted GeometryProgram draft unexpectedly contains canonical_sha256",
            ));
        }
        program_object.insert(
            "canonical_sha256".to_owned(),
            Value::String(evidence.geometry_program_sha256.clone()),
        );

        let mut preview_request = json!({
            "schema_version":"SubdivisionTopologyLineageRequest@1",
            "geometry_program":program,
            "subdivision_node_id":subdivision_node_id,
            "max_lineage_elements":max_lineage_elements,
            "canonical_sha256":""
        });
        preview_request["canonical_sha256"] = Value::String(canonical_json_hash(&preview_request));
        let preview = self.subdivision_topology_lineage_preview(preview_request)?;

        // This replay is the decisive artifact boundary: a matching program
        // hash or matching triangle count is insufficient. The entire GLB,
        // including primitive order, accessors, BIN and extras, must match.
        let replay = compile_geometry_with_runtime_worker(&program, None).map_err(|error| {
            artifact_lineage_error(&format!(
                "deterministic GeometryProgram replay failed: {error}"
            ))
        })?;
        let replay_inspection = strict_glb_inspection(&replay.glb)?;
        validate_worker_metadata(&replay, &replay_inspection)?;
        let replay_sha256 = sha256_hex(&replay.glb);
        if replay_sha256 != artifact_id || replay.glb != artifact_bytes {
            return Err(artifact_lineage_error(
                "deterministic full GLB byte replay differs from the candidate artifact",
            ));
        }

        let bindings = readback
            .get("part_bindings")
            .and_then(Value::as_array)
            .ok_or_else(|| artifact_lineage_error("ArtifactReadback part bindings are invalid"))?;
        let matching = bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| {
                binding.get("source_node_id").and_then(Value::as_str)
                    == Some(subdivision_node_id.as_str())
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(artifact_lineage_error(
                "subdivision node must be one direct, unambiguous artifact source primitive",
            ));
        }
        let (source_primitive_ordinal, binding) = matching[0];
        let part_id = binding
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| artifact_lineage_error("artifact Part binding is invalid"))?;
        let material_zone_id = binding
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| artifact_lineage_error("artifact MaterialZone binding is invalid"))?;
        if binding.get("solid").and_then(Value::as_bool) != Some(false) {
            return Err(artifact_lineage_error(
                "subd-cage@2 artifact source must remain an open surface",
            ));
        }
        let source_triangle_count = binding
            .get("triangle_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| artifact_lineage_error("artifact source triangle count is invalid"))?;
        let root_lineage = preview
            .get("lineage")
            .cloned()
            .ok_or_else(|| artifact_lineage_error("validated root lineage is unavailable"))?;
        let lineage_element_count = preview
            .get("lineage_element_count")
            .and_then(Value::as_u64)
            .filter(|value| *value <= max_lineage_elements)
            .ok_or_else(|| artifact_lineage_error("lineage element budget binding is invalid"))?;
        let evaluated_triangle_count = root_lineage["evaluated_counts"]["triangle_count"]
            .as_u64()
            .ok_or_else(|| artifact_lineage_error("evaluated triangle count is invalid"))?;
        if source_triangle_count != evaluated_triangle_count {
            return Err(artifact_lineage_error(
                "artifact source triangle count differs from evaluated Subdivision topology",
            ));
        }

        let control_ranges = root_lineage
            .get("control_quad_descendant_ranges")
            .and_then(Value::as_array)
            .ok_or_else(|| artifact_lineage_error("control quad ranges are invalid"))?;
        let artifact_ranges = control_ranges
            .iter()
            .enumerate()
            .map(|(control_quad_id, range)| {
                json!({
                    "control_quad_id":control_quad_id,
                    "artifact_triangle_start":range["evaluated_triangle_start"],
                    "artifact_triangle_count":range["evaluated_triangle_count"]
                })
            })
            .collect::<Vec<_>>();
        let mut artifact_binding = json!({
            "binding_method":"deterministic-full-glb-byte-replay-and-source-primitive-triangle-order@1",
            "artifact_id":artifact_id,
            "recompiled_artifact_sha256":replay_sha256,
            "source_primitive_ordinal":source_primitive_ordinal,
            "source_triangle_count":source_triangle_count,
            "artifact_triangle_domain":"source-primitive-local-triangle-index@1",
            "quad_triangulation":"0-1-2_0-2-3",
            "evaluated_quad_to_artifact_triangles_policy":"quad-q-maps-to-local-triangles-2q-and-2q-plus-1@1",
            "control_quad_artifact_triangle_ranges":artifact_ranges,
            "mapping_complete":true,
            "canonical_sha256":""
        });
        artifact_binding["canonical_sha256"] =
            Value::String(canonical_json_hash(&artifact_binding));
        let lineage_sha256 = canonical_json_hash(&root_lineage);
        let artifact_binding_sha256 = artifact_binding["canonical_sha256"].clone();
        let mut result = json!({
            "schema_version":RESULT_SCHEMA,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "artifact_id":artifact_id,
            "artifact_readback_sha256":artifact_readback_sha256,
            "artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
            "geometry_candidate_evidence_sha256":evidence.canonical_sha256,
            "program_sha256":evidence.geometry_program_sha256,
            "geometry_program_object_sha256":evidence.geometry_program_object_sha256,
            "operator_catalog_sha256":evidence.operator_catalog_sha256,
            "readback_config_sha256":evidence.readback_config_sha256,
            "subdivision_node_id":subdivision_node_id,
            "part_id":part_id,
            "material_zone_id":material_zone_id,
            "solid":false,
            "lineage_kind":"control-root-to-evaluated-quad-topology@1",
            "lineage_space":"evaluated-quad-topology-to-source-primitive-triangles@1",
            "max_lineage_elements":max_lineage_elements,
            "lineage_element_count":lineage_element_count,
            "root_lineage":root_lineage,
            "lineage_sha256":lineage_sha256,
            "artifact_binding":artifact_binding,
            "artifact_binding_sha256":artifact_binding_sha256,
            "complete":true,
            "completeness_scope":"all-root-mappings-and-source-primitive-local-triangle-ranges",
            "cross_version_stable":false,
            "materialization_status":"read-only-reconstructed-projection-not-persisted-sidecar",
            "runtime_write_performed":false,
            "quality_status":"structural_only",
            "limitations":LIMITATIONS,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_projection(&result)?;
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| artifact_lineage_error(&error.to_string()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(artifact_lineage_error(
                "complete projection exceeds the 1 MiB response budget",
            ));
        }
        Ok(result)
    }

    /// Explicitly materialize a previously reconstructable lineage projection.
    /// This is the only write path for the sidecar; reads never backfill CAS or
    /// SQLite state.
    pub fn subdivision_artifact_lineage_prepare(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let request_binding = validate_sidecar_request(&request)?;
        if let Some(existing) = self.store.get_subdivision_artifact_lineage_link(
            &request_binding.candidate_id,
            &request_binding.subdivision_node_id,
        )? {
            if existing.request_sha256 != request_binding.request_sha256 {
                return Err(sidecar_error(
                    "an immutable sidecar link already exists for a different request",
                ));
            }
            return self.load_subdivision_artifact_lineage_link(&request, existing);
        }

        let projection_request = projection_request_from_sidecar_request(&request)?;
        let projection = self.subdivision_artifact_lineage_get(projection_request)?;
        let sidecar = sidecar_from_projection(&projection)?;
        let sidecar_bytes =
            canonical_json_bytes(&sidecar).map_err(|error| sidecar_error(&error.to_string()))?;
        if sidecar_bytes.len() > MAX_RESPONSE_BYTES {
            return Err(sidecar_error(
                "complete sidecar exceeds the 1 MiB object budget",
            ));
        }
        let sidecar_object = self.put_object(
            &sidecar_bytes,
            None,
            "application/json",
            "subdivision-artifact-lineage-sidecar",
        )?;
        let link = sidecar_link_value(
            &request_binding.request_sha256,
            &sidecar_object.record.sha256,
            &sidecar,
        )?;
        let record = link_record_from_value(&link, &now_string())?;
        if let Err(commit_error) = self.store.record_subdivision_artifact_lineage_link(&record) {
            if let Err(rollback_error) = self
                .store
                .discard_new_temporary_subdivision_sidecar(&sidecar_object)
            {
                return Err(sidecar_error(&format!(
                    "sidecar link commit failed ({commit_error}); temporary CAS rollback also failed ({rollback_error})"
                )));
            }
            return Err(commit_error.into());
        }
        self.load_subdivision_artifact_lineage_link(&request, record)
    }

    /// Read an already materialized sidecar. This path verifies the current
    /// candidate/evidence/CAS bindings but never runs the Geometry Worker and
    /// never writes product state.
    pub fn subdivision_artifact_lineage_sidecar_get(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let request_binding = validate_sidecar_request(&request)?;
        let record = self
            .store
            .get_subdivision_artifact_lineage_link(
                &request_binding.candidate_id,
                &request_binding.subdivision_node_id,
            )?
            .ok_or_else(|| sidecar_error("durable sidecar link is unavailable"))?;
        if record.request_sha256 != request_binding.request_sha256 {
            return Err(sidecar_error(
                "caller request differs from the immutable sidecar request",
            ));
        }
        self.load_subdivision_artifact_lineage_link(&request, record)
    }

    fn load_subdivision_artifact_lineage_link(
        &self,
        request: &Value,
        record: SubdivisionArtifactLineageLinkRecord,
    ) -> Result<Value, RuntimeError> {
        let request_binding = validate_sidecar_request(request)?;
        validate_record_request_binding(&record, &request_binding)?;

        let candidate = self
            .candidate(&record.candidate_id)?
            .ok_or_else(|| sidecar_error("candidate is unavailable"))?;
        if candidate.project_id != record.project_id
            || candidate.prepared_object_sha256.as_deref() != Some(record.artifact_id.as_str())
        {
            return Err(sidecar_error("durable candidate binding differs"));
        }
        let evidence = self
            .store
            .get_geometry_candidate_evidence(&record.candidate_id)?
            .ok_or_else(|| sidecar_error("durable V2 geometry evidence is unavailable"))?;
        if evidence.project_id != record.project_id
            || evidence.artifact_object_sha256 != record.artifact_id
            || evidence.canonical_sha256 != record.geometry_candidate_evidence_sha256
        {
            return Err(sidecar_error("durable geometry evidence binding differs"));
        }
        let artifact_bytes =
            self.cas_read_bounded(&record.artifact_id, MAX_GEOMETRY_ARTIFACT_BYTES)?;
        let inspection = strict_glb_inspection(&artifact_bytes)?;
        self.revalidate_v2_geometry_evidence(&candidate, &inspection, &evidence)?;
        let readback = self.artifact_readback(&record.artifact_id, &record.candidate_id)?;
        if readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.artifact_readback_sha256.as_str())
        {
            return Err(sidecar_error("durable ArtifactReadback binding differs"));
        }

        let sidecar_bytes =
            self.cas_read_bounded(&record.sidecar_object_sha256, MAX_DERIVED_JSON_BYTES)?;
        if sidecar_bytes.len() > MAX_RESPONSE_BYTES {
            return Err(sidecar_error("persisted sidecar exceeds the 1 MiB budget"));
        }
        let sidecar: Value = serde_json::from_slice(&sidecar_bytes)
            .map_err(|_| sidecar_error("persisted sidecar is not canonical JSON"))?;
        validate_sidecar(&sidecar)?;
        if canonical_json_bytes(&sidecar).map_err(|error| sidecar_error(&error.to_string()))?
            != sidecar_bytes
        {
            return Err(sidecar_error("persisted sidecar bytes are not canonical"));
        }
        validate_record_sidecar_binding(&record, &sidecar)?;
        let link = sidecar_link_value(
            &record.request_sha256,
            &record.sidecar_object_sha256,
            &sidecar,
        )?;
        if link.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.canonical_sha256.as_str())
        {
            return Err(sidecar_error("durable link canonical hash differs"));
        }
        let bytes =
            canonical_json_bytes(&link).map_err(|error| sidecar_error(&error.to_string()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(sidecar_error(
                "complete sidecar link exceeds the 1 MiB response budget",
            ));
        }
        Ok(link)
    }
}

#[derive(Debug)]
struct SidecarRequestBinding {
    project_id: String,
    candidate_id: String,
    artifact_id: String,
    artifact_readback_sha256: String,
    subdivision_node_id: String,
    request_sha256: String,
}

fn validate_sidecar_request(value: &Value) -> Result<SidecarRequestBinding, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "subdivision_node_id",
            "max_lineage_elements",
            "canonical_sha256",
        ],
        SIDECAR_REQUEST_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(SIDECAR_REQUEST_SCHEMA) {
        return Err(sidecar_error("request schema_version differs"));
    }
    verify_output_canonical_hash(value, SIDECAR_REQUEST_SCHEMA)?;
    object
        .get("max_lineage_elements")
        .and_then(Value::as_u64)
        .filter(|value| (1..=MAX_LINEAGE_ELEMENTS).contains(value))
        .ok_or_else(|| sidecar_error("max_lineage_elements is outside 1..25000"))?;
    Ok(SidecarRequestBinding {
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        artifact_id: required_hash(object, "artifact_id")?,
        artifact_readback_sha256: required_hash(object, "artifact_readback_sha256")?,
        subdivision_node_id: required_id(object, "subdivision_node_id")?,
        request_sha256: required_hash(object, "canonical_sha256")?,
    })
}

fn projection_request_from_sidecar_request(request: &Value) -> Result<Value, RuntimeError> {
    validate_sidecar_request(request)?;
    let mut projection_request = request.clone();
    projection_request["schema_version"] = Value::String(REQUEST_SCHEMA.to_owned());
    projection_request["canonical_sha256"] = Value::String(String::new());
    projection_request["canonical_sha256"] =
        Value::String(canonical_json_hash(&projection_request));
    Ok(projection_request)
}

fn sidecar_from_projection(projection: &Value) -> Result<Value, RuntimeError> {
    validate_projection(projection)?;
    let mut sidecar = projection.clone();
    let object = sidecar
        .as_object_mut()
        .ok_or_else(|| sidecar_error("projection is not an object"))?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(SIDECAR_SCHEMA.to_owned()),
    );
    object.remove("runtime_write_performed");
    object.insert(
        "materialization_status".to_owned(),
        Value::String("runtime-owned-immutable-cas-sidecar".to_owned()),
    );
    object.insert("limitations".to_owned(), json!(SIDECAR_LIMITATIONS));
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let canonical_sha256 = canonical_json_hash(&sidecar);
    sidecar["canonical_sha256"] = Value::String(canonical_sha256);
    validate_sidecar(&sidecar)?;
    Ok(sidecar)
}

fn validate_sidecar(value: &Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| sidecar_error("sidecar is not an object"))?;
    let mut keys = vec![
        "schema_version",
        "project_id",
        "candidate_id",
        "artifact_id",
        "artifact_readback_sha256",
        "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256",
        "program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "subdivision_node_id",
        "part_id",
        "material_zone_id",
        "solid",
        "lineage_kind",
        "lineage_space",
        "max_lineage_elements",
        "lineage_element_count",
        "root_lineage",
        "lineage_sha256",
        "artifact_binding",
        "artifact_binding_sha256",
        "complete",
        "completeness_scope",
        "cross_version_stable",
        "materialization_status",
        "quality_status",
        "limitations",
        "canonical_sha256",
    ];
    keys.sort_unstable();
    let mut actual = object.keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();
    if actual != keys {
        return Err(sidecar_error("sidecar keys differ"));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(SIDECAR_SCHEMA)
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("runtime-owned-immutable-cas-sidecar")
        || object.get("limitations") != Some(&json!(SIDECAR_LIMITATIONS))
    {
        return Err(sidecar_error("sidecar constants differ"));
    }
    verify_output_canonical_hash(value, SIDECAR_SCHEMA)?;

    // Reuse the already exhaustive projection validator after restoring the
    // two projection-only truth markers. This keeps the sidecar from becoming
    // a weaker parallel interpretation of the same lineage payload.
    let mut projection = value.clone();
    projection["schema_version"] = Value::String(RESULT_SCHEMA.to_owned());
    projection["materialization_status"] =
        Value::String("read-only-reconstructed-projection-not-persisted-sidecar".to_owned());
    projection["runtime_write_performed"] = Value::Bool(false);
    projection["limitations"] = json!(LIMITATIONS);
    projection["canonical_sha256"] = Value::String(String::new());
    projection["canonical_sha256"] = Value::String(canonical_json_hash(&projection));
    validate_projection(&projection)
}

fn sidecar_link_value(
    request_sha256: &str,
    sidecar_object_sha256: &str,
    sidecar: &Value,
) -> Result<Value, RuntimeError> {
    validate_sidecar(sidecar)?;
    if !is_sha256(request_sha256) || !is_sha256(sidecar_object_sha256) {
        return Err(sidecar_error("link hashes are invalid"));
    }
    let mut link = json!({
        "schema_version":SIDECAR_LINK_SCHEMA,
        "project_id":sidecar["project_id"],
        "candidate_id":sidecar["candidate_id"],
        "artifact_id":sidecar["artifact_id"],
        "artifact_readback_sha256":sidecar["artifact_readback_sha256"],
        "geometry_candidate_evidence_sha256":sidecar["geometry_candidate_evidence_sha256"],
        "subdivision_node_id":sidecar["subdivision_node_id"],
        "request_sha256":request_sha256,
        "sidecar_object_sha256":sidecar_object_sha256,
        "lineage_sha256":sidecar["lineage_sha256"],
        "artifact_binding_sha256":sidecar["artifact_binding_sha256"],
        "materialization_status":"runtime-owned-immutable-cas-sidecar",
        "sidecar":sidecar,
        "canonical_sha256":""
    });
    link["canonical_sha256"] = Value::String(canonical_json_hash(&link));
    validate_sidecar_link(&link)?;
    Ok(link)
}

fn validate_sidecar_link(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "geometry_candidate_evidence_sha256",
            "subdivision_node_id",
            "request_sha256",
            "sidecar_object_sha256",
            "lineage_sha256",
            "artifact_binding_sha256",
            "materialization_status",
            "sidecar",
            "canonical_sha256",
        ],
        SIDECAR_LINK_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(SIDECAR_LINK_SCHEMA)
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("runtime-owned-immutable-cas-sidecar")
    {
        return Err(sidecar_error("link constants differ"));
    }
    for key in [
        "artifact_id",
        "artifact_readback_sha256",
        "geometry_candidate_evidence_sha256",
        "request_sha256",
        "sidecar_object_sha256",
        "lineage_sha256",
        "artifact_binding_sha256",
    ] {
        required_hash(object, key)?;
    }
    for key in ["project_id", "candidate_id", "subdivision_node_id"] {
        required_id(object, key)?;
    }
    let sidecar = object
        .get("sidecar")
        .ok_or_else(|| sidecar_error("link sidecar is missing"))?;
    validate_sidecar(sidecar)?;
    for key in [
        "project_id",
        "candidate_id",
        "artifact_id",
        "artifact_readback_sha256",
        "geometry_candidate_evidence_sha256",
        "subdivision_node_id",
        "lineage_sha256",
        "artifact_binding_sha256",
        "materialization_status",
    ] {
        if object.get(key) != sidecar.get(key) {
            return Err(sidecar_error(&format!("link {key} binding differs")));
        }
    }
    verify_output_canonical_hash(value, SIDECAR_LINK_SCHEMA)
}

fn link_record_from_value(
    link: &Value,
    created_at: &str,
) -> Result<SubdivisionArtifactLineageLinkRecord, RuntimeError> {
    validate_sidecar_link(link)?;
    let object = link
        .as_object()
        .ok_or_else(|| sidecar_error("link is not an object"))?;
    Ok(SubdivisionArtifactLineageLinkRecord {
        schema_version: SIDECAR_LINK_SCHEMA.to_owned(),
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        artifact_id: required_hash(object, "artifact_id")?,
        artifact_readback_sha256: required_hash(object, "artifact_readback_sha256")?,
        geometry_candidate_evidence_sha256: required_hash(
            object,
            "geometry_candidate_evidence_sha256",
        )?,
        subdivision_node_id: required_id(object, "subdivision_node_id")?,
        request_sha256: required_hash(object, "request_sha256")?,
        sidecar_object_sha256: required_hash(object, "sidecar_object_sha256")?,
        lineage_sha256: required_hash(object, "lineage_sha256")?,
        artifact_binding_sha256: required_hash(object, "artifact_binding_sha256")?,
        materialization_status: "runtime-owned-immutable-cas-sidecar".to_owned(),
        canonical_sha256: required_hash(object, "canonical_sha256")?,
        created_at: created_at.to_owned(),
    })
}

fn validate_record_request_binding(
    record: &SubdivisionArtifactLineageLinkRecord,
    request: &SidecarRequestBinding,
) -> Result<(), RuntimeError> {
    if record.schema_version != SIDECAR_LINK_SCHEMA
        || record.project_id != request.project_id
        || record.candidate_id != request.candidate_id
        || record.artifact_id != request.artifact_id
        || record.artifact_readback_sha256 != request.artifact_readback_sha256
        || record.subdivision_node_id != request.subdivision_node_id
        || record.request_sha256 != request.request_sha256
        || record.materialization_status != "runtime-owned-immutable-cas-sidecar"
    {
        return Err(sidecar_error("durable link and caller request differ"));
    }
    Ok(())
}

fn validate_record_sidecar_binding(
    record: &SubdivisionArtifactLineageLinkRecord,
    sidecar: &Value,
) -> Result<(), RuntimeError> {
    if sidecar.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || sidecar.get("candidate_id").and_then(Value::as_str) != Some(record.candidate_id.as_str())
        || sidecar.get("artifact_id").and_then(Value::as_str) != Some(record.artifact_id.as_str())
        || sidecar
            .get("artifact_readback_sha256")
            .and_then(Value::as_str)
            != Some(record.artifact_readback_sha256.as_str())
        || sidecar
            .get("geometry_candidate_evidence_sha256")
            .and_then(Value::as_str)
            != Some(record.geometry_candidate_evidence_sha256.as_str())
        || sidecar.get("subdivision_node_id").and_then(Value::as_str)
            != Some(record.subdivision_node_id.as_str())
        || sidecar.get("lineage_sha256").and_then(Value::as_str)
            != Some(record.lineage_sha256.as_str())
        || sidecar
            .get("artifact_binding_sha256")
            .and_then(Value::as_str)
            != Some(record.artifact_binding_sha256.as_str())
    {
        return Err(sidecar_error("durable link and sidecar bindings differ"));
    }
    Ok(())
}

fn validate_projection(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "artifact_readback_object_sha256",
            "geometry_candidate_evidence_sha256",
            "program_sha256",
            "geometry_program_object_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "subdivision_node_id",
            "part_id",
            "material_zone_id",
            "solid",
            "lineage_kind",
            "lineage_space",
            "max_lineage_elements",
            "lineage_element_count",
            "root_lineage",
            "lineage_sha256",
            "artifact_binding",
            "artifact_binding_sha256",
            "complete",
            "completeness_scope",
            "cross_version_stable",
            "materialization_status",
            "runtime_write_performed",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        RESULT_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(RESULT_SCHEMA)
        || object.get("solid") != Some(&Value::Bool(false))
        || object.get("lineage_kind").and_then(Value::as_str)
            != Some("control-root-to-evaluated-quad-topology@1")
        || object.get("lineage_space").and_then(Value::as_str)
            != Some("evaluated-quad-topology-to-source-primitive-triangles@1")
        || object.get("complete") != Some(&Value::Bool(true))
        || object.get("completeness_scope").and_then(Value::as_str)
            != Some("all-root-mappings-and-source-primitive-local-triangle-ranges")
        || object.get("cross_version_stable") != Some(&Value::Bool(false))
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("read-only-reconstructed-projection-not-persisted-sidecar")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || object.get("limitations") != Some(&json!(LIMITATIONS))
    {
        return Err(artifact_lineage_error("projection constants differ"));
    }
    for key in [
        "artifact_id",
        "artifact_readback_sha256",
        "artifact_readback_object_sha256",
        "geometry_candidate_evidence_sha256",
        "program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "lineage_sha256",
        "artifact_binding_sha256",
    ] {
        required_hash(object, key)?;
    }
    for key in [
        "project_id",
        "candidate_id",
        "subdivision_node_id",
        "part_id",
        "material_zone_id",
    ] {
        required_id(object, key)?;
    }
    let max_lineage_elements = object
        .get("max_lineage_elements")
        .and_then(Value::as_u64)
        .filter(|value| (1..=MAX_LINEAGE_ELEMENTS).contains(value))
        .ok_or_else(|| artifact_lineage_error("projection lineage budget is invalid"))?;
    let lineage_element_count = object
        .get("lineage_element_count")
        .and_then(Value::as_u64)
        .filter(|value| *value <= max_lineage_elements)
        .ok_or_else(|| artifact_lineage_error("projection lineage count exceeds its budget"))?;
    let root_lineage = object
        .get("root_lineage")
        .ok_or_else(|| artifact_lineage_error("root_lineage is missing"))?;
    if object.get("lineage_sha256").and_then(Value::as_str)
        != Some(canonical_json_hash(root_lineage).as_str())
    {
        return Err(artifact_lineage_error("root lineage hash differs"));
    }
    let binding = exact_object(
        object
            .get("artifact_binding")
            .ok_or_else(|| artifact_lineage_error("artifact_binding is missing"))?,
        &[
            "binding_method",
            "artifact_id",
            "recompiled_artifact_sha256",
            "source_primitive_ordinal",
            "source_triangle_count",
            "artifact_triangle_domain",
            "quad_triangulation",
            "evaluated_quad_to_artifact_triangles_policy",
            "control_quad_artifact_triangle_ranges",
            "mapping_complete",
            "canonical_sha256",
        ],
        "SubdivisionArtifactLineageProjection@1.artifact_binding",
    )?;
    if binding.get("binding_method").and_then(Value::as_str)
        != Some("deterministic-full-glb-byte-replay-and-source-primitive-triangle-order@1")
        || binding.get("artifact_id") != object.get("artifact_id")
        || binding.get("recompiled_artifact_sha256") != object.get("artifact_id")
        || binding
            .get("artifact_triangle_domain")
            .and_then(Value::as_str)
            != Some("source-primitive-local-triangle-index@1")
        || binding.get("quad_triangulation").and_then(Value::as_str) != Some("0-1-2_0-2-3")
        || binding
            .get("evaluated_quad_to_artifact_triangles_policy")
            .and_then(Value::as_str)
            != Some("quad-q-maps-to-local-triangles-2q-and-2q-plus-1@1")
        || binding.get("mapping_complete") != Some(&Value::Bool(true))
    {
        return Err(artifact_lineage_error("artifact binding constants differ"));
    }
    let source_triangle_count = binding
        .get("source_triangle_count")
        .and_then(Value::as_u64)
        .filter(|value| (32..=7200).contains(value))
        .ok_or_else(|| artifact_lineage_error("source triangle count is invalid"))?;
    if root_lineage["evaluated_counts"]["triangle_count"].as_u64() != Some(source_triangle_count) {
        return Err(artifact_lineage_error(
            "root lineage and artifact triangle counts differ",
        ));
    }
    let root_ranges = root_lineage
        .get("control_quad_descendant_ranges")
        .and_then(Value::as_array)
        .ok_or_else(|| artifact_lineage_error("root lineage ranges are invalid"))?;
    let artifact_ranges = binding
        .get("control_quad_artifact_triangle_ranges")
        .and_then(Value::as_array)
        .filter(|ranges| ranges.len() == root_ranges.len())
        .ok_or_else(|| artifact_lineage_error("artifact triangle ranges are invalid"))?;
    let expected_triangles_per_control_quad = source_triangle_count
        .checked_div(root_ranges.len() as u64)
        .filter(|value| matches!(value, 8 | 32))
        .ok_or_else(|| artifact_lineage_error("control quad triangle partition is invalid"))?;
    for (control_quad_id, (artifact_range, root_range)) in
        artifact_ranges.iter().zip(root_ranges).enumerate()
    {
        let range = exact_object(
            artifact_range,
            &[
                "control_quad_id",
                "artifact_triangle_start",
                "artifact_triangle_count",
            ],
            "SubdivisionArtifactLineageProjection@1.triangle_range",
        )?;
        let expected_start = (control_quad_id as u64)
            .checked_mul(expected_triangles_per_control_quad)
            .ok_or_else(|| artifact_lineage_error("artifact triangle range overflow"))?;
        if range.get("control_quad_id").and_then(Value::as_u64) != Some(control_quad_id as u64)
            || range.get("artifact_triangle_start") != root_range.get("evaluated_triangle_start")
            || range.get("artifact_triangle_count") != root_range.get("evaluated_triangle_count")
            || range.get("artifact_triangle_start").and_then(Value::as_u64) != Some(expected_start)
            || range.get("artifact_triangle_count").and_then(Value::as_u64)
                != Some(expected_triangles_per_control_quad)
        {
            return Err(artifact_lineage_error("artifact triangle range differs"));
        }
    }
    if lineage_element_count
        != object["root_lineage"]["control_counts"]["vertex_count"]
            .as_u64()
            .and_then(|vertices| {
                object["root_lineage"]["control_counts"]["edge_count"]
                    .as_u64()
                    .and_then(|edges| vertices.checked_add(edges))
            })
            .and_then(|sum| {
                object["root_lineage"]["control_counts"]["quad_count"]
                    .as_u64()
                    .and_then(|quads| sum.checked_add(quads))
            })
            .and_then(|sum| {
                object["root_lineage"]["evaluated_counts"]["vertex_count"]
                    .as_u64()
                    .and_then(|vertices| sum.checked_add(vertices))
            })
            .and_then(|sum| {
                object["root_lineage"]["evaluated_counts"]["edge_count"]
                    .as_u64()
                    .and_then(|edges| sum.checked_add(edges))
            })
            .and_then(|sum| {
                object["root_lineage"]["evaluated_counts"]["quad_count"]
                    .as_u64()
                    .and_then(|quads| sum.checked_add(quads))
            })
            .and_then(|sum| sum.checked_add(source_triangle_count))
            .ok_or_else(|| artifact_lineage_error("lineage element count overflow"))?
    {
        return Err(artifact_lineage_error("lineage element count differs"));
    }
    verify_output_canonical_hash(
        object
            .get("artifact_binding")
            .expect("artifact binding was checked"),
        "SubdivisionArtifactLineageProjection@1.artifact_binding",
    )?;
    if object.get("artifact_binding_sha256") != binding.get("canonical_sha256") {
        return Err(artifact_lineage_error("artifact binding hash differs"));
    }
    verify_output_canonical_hash(value, RESULT_SCHEMA)
}

fn required_id(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| artifact_lineage_error(&format!("{key} is invalid")))
}

fn required_hash(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| artifact_lineage_error(&format!("{key} is invalid")))
}

fn artifact_lineage_error(message: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("SUBDIVISION_ARTIFACT_LINEAGE_INVALID: {message}"))
}

fn sidecar_error(message: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "SUBDIVISION_ARTIFACT_LINEAGE_SIDECAR_INVALID: {message}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_catalog_sha256;
    use std::fs;
    use uuid::Uuid;

    fn draft(project_id: &str) -> Value {
        json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"8".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":128,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"cage","operator_id":"forgecad.geometry.subd-cage@2","inputs":[],
                "parameters":{
                    "shape":"subd-cage",
                    "control_points":[[-1.0,-1.0,0.0],[0.0,-1.0,0.0],[1.0,-1.0,0.0],[-1.0,0.0,0.0],[0.0,0.0,1.0],[1.0,0.0,0.0],[-1.0,1.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
                    "u_points":3,"v_points":3,"subdivision_levels":2,
                    "crease_method":"uniform-integer-level-decay@1",
                    "crease_edges":[{"vertex_a":3,"vertex_b":4,"sharpness_levels":2},{"vertex_a":4,"vertex_b":5,"sharpness_levels":2}],
                    "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"cage-part","input_node_ids":["cage"],"material_zone_id":"zone-shell","solid":false}]
        })
    }

    fn prepare(runtime: &Runtime, project_id: &str) -> Value {
        prepare_program(runtime, project_id, draft(project_id))
    }

    fn prepare_program(runtime: &Runtime, project_id: &str, mut program: Value) -> Value {
        let hash = runtime
            .geometry_program_hash(&json!({
                "schema_version":"GeometryProgramHashRequest@1",
                "geometry_program_draft":program
            }))
            .expect("program hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        runtime
            .prepare_geometry_candidate(
                project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("geometry prepare")
    }

    fn large_admitted_draft(project_id: &str) -> Value {
        let control_points = (0..14)
            .flat_map(|v| (0..14).map(move |u| json!([u as f64 * 0.1, v as f64 * 0.1, 0.0])))
            .collect::<Vec<_>>();
        json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"9".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":1,"max_triangles":5408,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{
                "node_id":"cage","operator_id":"forgecad.geometry.subd-cage@2","inputs":[],
                "parameters":{
                    "shape":"subd-cage","control_points":control_points,"u_points":14,"v_points":14,
                    "subdivision_levels":2,"crease_method":"uniform-integer-level-decay@1",
                    "crease_edges":[{"vertex_a":17,"vertex_b":18,"sharpness_levels":2}],
                    "position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{"part_id":"cage-part","input_node_ids":["cage"],"material_zone_id":"zone-shell","solid":false}]
        })
    }

    fn request(project_id: &str, prepared: &Value) -> Value {
        let mut value = json!({
            "schema_version":REQUEST_SCHEMA,
            "project_id":project_id,
            "candidate_id":prepared["candidate"]["candidate_id"],
            "artifact_id":prepared["artifact"]["artifact_id"],
            "artifact_readback_sha256":prepared["artifact"]["canonical_sha256"],
            "subdivision_node_id":"cage",
            "max_lineage_elements":25000,
            "canonical_sha256":""
        });
        value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
        value
    }

    fn sidecar_request(project_id: &str, prepared: &Value) -> Value {
        let mut value = json!({
            "schema_version":SIDECAR_REQUEST_SCHEMA,
            "project_id":project_id,
            "candidate_id":prepared["candidate"]["candidate_id"],
            "artifact_id":prepared["artifact"]["artifact_id"],
            "artifact_readback_sha256":prepared["artifact"]["canonical_sha256"],
            "subdivision_node_id":"cage",
            "max_lineage_elements":25000,
            "canonical_sha256":""
        });
        value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
        value
    }

    #[test]
    fn projection_is_exact_artifact_bound_and_read_only() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Subdivision artifact lineage", json!({"profile":"mvp"}))
            .expect("project");
        let prepared = prepare(&runtime, &project.project_id);
        let before = json!({
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions")
        });
        let projection = runtime
            .subdivision_artifact_lineage_get(request(&project.project_id, &prepared))
            .expect("artifact lineage");
        assert_eq!(projection["schema_version"], RESULT_SCHEMA);
        assert_eq!(
            projection["lineage_sha256"],
            canonical_json_hash(&projection["root_lineage"])
        );
        assert_eq!(
            projection["artifact_binding"]["artifact_id"],
            prepared["artifact"]["artifact_id"]
        );
        assert_eq!(
            projection["artifact_binding"]["recompiled_artifact_sha256"],
            prepared["artifact"]["artifact_id"]
        );
        assert_eq!(projection["artifact_binding"]["source_triangle_count"], 128);
        assert_eq!(projection["max_lineage_elements"], 25000);
        assert_eq!(projection["lineage_element_count"], 442);
        assert_eq!(
            projection["artifact_binding"]["source_primitive_ordinal"],
            0
        );
        assert_eq!(
            projection["artifact_binding"]["control_quad_artifact_triangle_ranges"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(projection["runtime_write_performed"], false);
        let after = json!({
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions")
        });
        assert_eq!(before, after);

        let mut forged = projection.clone();
        forged["artifact_binding"]["control_quad_artifact_triangle_ranges"][0]
            ["artifact_triangle_start"] = json!(2);
        forged["artifact_binding"]["canonical_sha256"] = json!("");
        forged["artifact_binding"]["canonical_sha256"] =
            Value::String(canonical_json_hash(&forged["artifact_binding"]));
        forged["artifact_binding_sha256"] = forged["artifact_binding"]["canonical_sha256"].clone();
        forged["canonical_sha256"] = json!("");
        forged["canonical_sha256"] = Value::String(canonical_json_hash(&forged));
        assert!(validate_projection(&forged).is_err());
    }

    #[test]
    fn projection_restarts_from_durable_evidence_and_rejects_stale_binding() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-subdivision-artifact-lineage-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let (project_id, prepared, request) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
            let project = runtime
                .create_project("Subdivision artifact restart", json!({"profile":"mvp"}))
                .expect("project");
            let prepared = prepare(&runtime, &project.project_id);
            let request = request(&project.project_id, &prepared);
            runtime
                .subdivision_artifact_lineage_get(request.clone())
                .expect("initial projection");
            (project.project_id, prepared, request)
        };
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen");
        let projection = reopened
            .subdivision_artifact_lineage_get(request.clone())
            .expect("restarted projection");
        assert_eq!(projection["project_id"], project_id);
        assert_eq!(
            projection["artifact_id"],
            prepared["artifact"]["artifact_id"]
        );

        let mut stale = request;
        stale["artifact_readback_sha256"] = json!("f".repeat(64));
        stale["canonical_sha256"] = json!("");
        stale["canonical_sha256"] = Value::String(canonical_json_hash(&stale));
        assert!(reopened.subdivision_artifact_lineage_get(stale).is_err());
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn projection_rejects_oversized_artifact_and_program_before_replay() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Subdivision artifact read bounds", json!({"profile":"mvp"}))
            .expect("project");
        let prepared = prepare(&runtime, &project.project_id);
        let request = request(&project.project_id, &prepared);
        let artifact_id = prepared["artifact"]["artifact_id"]
            .as_str()
            .expect("artifact id");
        let artifact_path = runtime
            .store
            .cas()
            .root()
            .join("objects")
            .join(&artifact_id[..2])
            .join(artifact_id);
        let artifact_bytes = fs::read(&artifact_path).expect("artifact bytes");
        fs::OpenOptions::new()
            .write(true)
            .open(&artifact_path)
            .expect("open artifact for oversize simulation")
            .set_len(MAX_GEOMETRY_ARTIFACT_BYTES + 1)
            .expect("grow artifact sparsely");
        let artifact_error = runtime
            .subdivision_artifact_lineage_get(request.clone())
            .expect_err("oversized GLB must fail before replay");
        assert!(artifact_error.to_string().contains("capacity"));
        fs::write(&artifact_path, artifact_bytes).expect("restore artifact");

        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(
                prepared["candidate"]["candidate_id"]
                    .as_str()
                    .expect("candidate id"),
            )
            .expect("geometry evidence lookup")
            .expect("geometry evidence");
        let program_sha256 = evidence.geometry_program_object_sha256;
        let program_path = runtime
            .store
            .cas()
            .root()
            .join("objects")
            .join(&program_sha256[..2])
            .join(&program_sha256);
        let program_bytes = fs::read(&program_path).expect("program bytes");
        fs::OpenOptions::new()
            .write(true)
            .open(&program_path)
            .expect("open program for oversize simulation")
            .set_len(MAX_DERIVED_JSON_BYTES + 1)
            .expect("grow program sparsely");
        let program_error = runtime
            .subdivision_artifact_lineage_get(request)
            .expect_err("oversized GeometryProgram must fail before replay");
        assert!(
            program_error.to_string().contains("capacity"),
            "unexpected program bound error: {program_error}"
        );
        fs::write(&program_path, program_bytes).expect("restore program");
    }

    #[test]
    fn large_admitted_artifact_projection_stays_complete_under_one_mib() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "Subdivision artifact maximum envelope",
                json!({"profile":"mvp"}),
            )
            .expect("project");
        let mut direct_program = large_admitted_draft(&project.project_id);
        let direct_hash = runtime
            .geometry_program_hash(&json!({
                "schema_version":"GeometryProgramHashRequest@1",
                "geometry_program_draft":direct_program
            }))
            .expect("maximum program hash");
        direct_program["canonical_sha256"] = direct_hash["canonical_sha256"].clone();
        crate::compile_geometry_program(&direct_program).expect("maximum direct compile");
        let prepared = prepare_program(
            &runtime,
            &project.project_id,
            large_admitted_draft(&project.project_id),
        );
        let projection = runtime
            .subdivision_artifact_lineage_get(request(&project.project_id, &prepared))
            .expect("maximum artifact projection");
        assert_eq!(projection["lineage_element_count"], 17_162);
        assert_eq!(
            projection["artifact_binding"]["source_triangle_count"],
            5_408
        );
        assert_eq!(
            projection["artifact_binding"]["control_quad_artifact_triangle_ranges"]
                .as_array()
                .expect("control quad ranges")
                .len(),
            169
        );
        let bytes = canonical_json_bytes(&projection).expect("canonical projection bytes");
        assert!(
            bytes.len() <= MAX_RESPONSE_BYTES - 4096,
            "large artifact projection leaves insufficient MCP envelope headroom at {} bytes",
            bytes.len()
        );
        let link = runtime
            .subdivision_artifact_lineage_prepare(sidecar_request(&project.project_id, &prepared))
            .expect("maximum durable sidecar");
        let link_bytes = canonical_json_bytes(&link).expect("canonical sidecar link bytes");
        assert!(
            link_bytes.len() <= MAX_RESPONSE_BYTES,
            "large durable sidecar link exceeds MCP envelope at {} bytes",
            link_bytes.len()
        );
    }

    #[test]
    fn sidecar_prepare_is_idempotent_and_get_survives_restart_without_backfill() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-subdivision-artifact-sidecar-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let (project_id, sidecar_request, prepared, first) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
            let project = runtime
                .create_project("Subdivision durable sidecar", json!({"profile":"mvp"}))
                .expect("project");
            let prepared = prepare(&runtime, &project.project_id);
            let request = sidecar_request(&project.project_id, &prepared);
            let before = json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            });
            let first = runtime
                .subdivision_artifact_lineage_prepare(request.clone())
                .expect("sidecar prepare");
            let repeated = runtime
                .subdivision_artifact_lineage_prepare(request.clone())
                .expect("idempotent prepare");
            assert_eq!(first, repeated);
            assert_eq!(first["schema_version"], SIDECAR_LINK_SCHEMA);
            assert_eq!(
                first["sidecar"]["materialization_status"],
                "runtime-owned-immutable-cas-sidecar"
            );
            assert_eq!(first["sidecar"]["quality_status"], "structural_only");
            let after = json!({
                "candidates":runtime.candidates(&project.project_id).expect("candidates"),
                "versions":runtime.versions(Some(&project.project_id)).expect("versions")
            });
            assert_eq!(before, after);
            (project.project_id, request, prepared, first)
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen");
        let before_get = json!({
            "candidates":reopened.candidates(&project_id).expect("candidates"),
            "versions":reopened.versions(Some(&project_id)).expect("versions")
        });
        let restarted = reopened
            .subdivision_artifact_lineage_sidecar_get(sidecar_request.clone())
            .expect("restarted sidecar get");
        assert_eq!(first, restarted);
        assert_eq!(
            restarted["artifact_id"],
            prepared["artifact"]["artifact_id"]
        );
        let after_get = json!({
            "candidates":reopened.candidates(&project_id).expect("candidates"),
            "versions":reopened.versions(Some(&project_id)).expect("versions")
        });
        assert_eq!(before_get, after_get);

        let mut different_request = sidecar_request;
        different_request["max_lineage_elements"] = json!(24999);
        different_request["canonical_sha256"] = json!("");
        different_request["canonical_sha256"] =
            Value::String(canonical_json_hash(&different_request));
        assert!(reopened
            .subdivision_artifact_lineage_prepare(different_request)
            .is_err());
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn sidecar_get_rejects_cross_candidate_and_corrupt_cas_bytes() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-subdivision-artifact-sidecar-negative-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let runtime = Runtime::open_with_cas(&database, &cas).expect("runtime");
        let project = runtime
            .create_project("Subdivision sidecar negatives", json!({"profile":"mvp"}))
            .expect("project");
        let prepared = prepare(&runtime, &project.project_id);
        let request = sidecar_request(&project.project_id, &prepared);
        let link = runtime
            .subdivision_artifact_lineage_prepare(request.clone())
            .expect("sidecar prepare");

        let other_project = runtime
            .create_project("Other project", json!({"profile":"mvp"}))
            .expect("other project");
        let other = prepare(&runtime, &other_project.project_id);
        let mut cross_candidate = request.clone();
        cross_candidate["candidate_id"] = other["candidate"]["candidate_id"].clone();
        cross_candidate["canonical_sha256"] = json!("");
        cross_candidate["canonical_sha256"] = Value::String(canonical_json_hash(&cross_candidate));
        assert!(runtime
            .subdivision_artifact_lineage_sidecar_get(cross_candidate)
            .is_err());

        let sidecar_sha = link["sidecar_object_sha256"].as_str().expect("sidecar sha");
        let object_path = runtime
            .store
            .cas()
            .root()
            .join("objects")
            .join(&sidecar_sha[..2])
            .join(sidecar_sha);
        let sidecar_bytes = fs::read(&object_path).expect("sidecar bytes");
        fs::OpenOptions::new()
            .write(true)
            .open(&object_path)
            .expect("open sidecar for oversize simulation")
            .set_len(MAX_RESPONSE_BYTES as u64 + 1)
            .expect("grow sidecar sparsely");
        let sidecar_error = runtime
            .subdivision_artifact_lineage_sidecar_get(request.clone())
            .expect_err("oversized sidecar must fail before JSON parsing");
        assert!(sidecar_error.to_string().contains("capacity"));
        fs::write(&object_path, sidecar_bytes).expect("restore sidecar");

        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(
                prepared["candidate"]["candidate_id"]
                    .as_str()
                    .expect("candidate id"),
            )
            .expect("geometry evidence lookup")
            .expect("geometry evidence");
        let readback_sha = evidence.artifact_readback_object_sha256;
        let readback_path = runtime
            .store
            .cas()
            .root()
            .join("objects")
            .join(&readback_sha[..2])
            .join(&readback_sha);
        let readback_bytes = fs::read(&readback_path).expect("readback bytes");
        fs::OpenOptions::new()
            .write(true)
            .open(&readback_path)
            .expect("open readback for oversize simulation")
            .set_len(MAX_RESPONSE_BYTES as u64 + 1)
            .expect("grow readback sparsely");
        let readback_error = runtime
            .subdivision_artifact_lineage_sidecar_get(request.clone())
            .expect_err("oversized ArtifactReadback JSON must fail before parsing");
        assert!(readback_error.to_string().contains("capacity"));
        fs::write(&readback_path, readback_bytes).expect("restore readback");

        fs::write(&object_path, b"tampered").expect("tamper isolated CAS");
        assert!(runtime
            .subdivision_artifact_lineage_sidecar_get(request)
            .is_err());
        drop(runtime);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
