use super::{
    camera_identity_hash, canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id,
    is_sha256, render_glb_with_runtime_worker_identity, render_worker, sha256_hex,
    strict_glb_inspection, strict_integrity_value, validate_artifact_readback_v2_output,
    validate_camera_calibration, validate_quality_report_v2_output,
    validate_reference_comparison_report, validate_render_set_v2_output,
    verify_output_canonical_hash, visible_view_gate_checks, Runtime, RuntimeError,
};
use serde_json::{json, Map, Value};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_JSON_OBJECT_BYTES: u64 = 1024 * 1024;
const MAX_REFERENCE_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_AOV_PNG_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REPLAY_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const REPLAY_POLICY: &str = "fixed-worker-nine-aov-byte-replay-read-only@1";
const AOV_PASSES: [&str; 9] = [
    "beauty",
    "silhouette",
    "depth",
    "normal",
    "ao",
    "part-id",
    "material-id",
    "wireframe",
    "uv-stretch",
];
const LIMITATIONS: [&str; 8] = [
    "CURRENT_SOURCE_COHORT_ONLY",
    "HISTORICAL_RECEIPTS_NOT_REPAIRED",
    "STRUCTURAL_INTEGRITY_DOES_NOT_PROVE_VISUAL_QUALITY",
    "NO_HUMAN_REVIEW_INFERENCE",
    "NO_CYCLES_EEVEE_OR_OCIO_PARITY",
    "SINGLE_VIEW_ONLY",
    "CAMERA_FIT_PROVENANCE_NOT_ASSERTED",
    "DOWNSTREAM_HUMAN_PBR_EXPORT_RESTART_HQ360_NOT_EVALUATED",
];
const REPLAY_LIMITATIONS: [&str; 9] = [
    "CURRENT_SOURCE_RENDER_WORKER_ONLY",
    "SAME_RENDER_WORKER_COHORT_REQUIRED",
    "AUTHORED_APPEARANCE_PROGRAM_NOT_DURABLY_BOUND",
    "BYTE_EXACT_REPLAY_DOES_NOT_PROVE_CROSS_PLATFORM_DETERMINISM",
    "STRUCTURAL_REPLAY_DOES_NOT_PROVE_VISUAL_QUALITY",
    "NO_HUMAN_REVIEW_INFERENCE",
    "NO_CYCLES_EEVEE_OR_OCIO_PARITY",
    "NO_CAS_SQLITE_CANDIDATE_OR_VERSION_WRITE",
    "DOWNSTREAM_PACKAGE_LIVE_PBR_EXPORT_RESTART_HQ360_NOT_EVALUATED",
];

#[derive(Debug, Clone)]
struct IntegrityRequest {
    project_id: String,
    candidate_id: String,
    artifact_sha256: String,
    artifact_readback_object_sha256: String,
    program_sha256: String,
    reference_id: String,
    reference_sha256: String,
    camera_hash: String,
    camera_object_sha256: String,
    render_set_object_sha256: String,
    comparison_report_object_sha256: String,
    quality_report_object_sha256: String,
    request_sha256: String,
}

#[derive(Debug, Clone)]
struct ReplayRequest {
    candidate_state_sha256: String,
    integrity_request: Value,
    request_sha256: String,
}

impl Runtime {
    /// Deeply re-read one exact, current candidate-bound render evidence
    /// cohort. This is a read-only integrity projection: it never repairs a
    /// historical receipt and never promotes structural integrity to visual
    /// or human acceptance.
    pub fn render_evidence_integrity_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let request = parse_request(&request)?;
        let candidate = self
            .candidate(&request.candidate_id)?
            .ok_or_else(|| integrity_error("candidate is unavailable"))?;
        if candidate.project_id != request.project_id {
            return Err(binding_error("candidate project differs"));
        }
        let candidate_artifact = candidate
            .prepared_object_sha256
            .as_deref()
            .or(candidate.manifest_hash.as_deref())
            .ok_or_else(|| binding_error("candidate artifact is unavailable"))?;
        if candidate_artifact != request.artifact_sha256
            || candidate
                .prepared_object_sha256
                .as_deref()
                .zip(candidate.manifest_hash.as_deref())
                .is_some_and(|(prepared, manifest)| prepared != manifest)
        {
            return Err(binding_error("candidate artifact differs"));
        }

        let geometry = self
            .store
            .get_geometry_candidate_evidence(&request.candidate_id)?
            .ok_or_else(|| binding_error("geometry evidence is unavailable"))?;
        if geometry.project_id != request.project_id
            || geometry.artifact_object_sha256 != request.artifact_sha256
            || geometry.geometry_program_sha256 != request.program_sha256
            || geometry.artifact_readback_object_sha256 != request.artifact_readback_object_sha256
        {
            return Err(binding_error("geometry evidence differs"));
        }

        let reference = self
            .reference(&request.reference_id)?
            .ok_or_else(|| integrity_error("reference is unavailable"))?;
        if reference.project_id != request.project_id
            || reference.object_sha256 != request.reference_sha256
        {
            return Err(binding_error("reference differs"));
        }
        let reference_bytes = read_binary_binding(
            self,
            &request.reference_sha256,
            Some((&reference.mime, reference.size_bytes)),
            "reference",
            MAX_REFERENCE_SOURCE_BYTES,
        )?;
        let artifact_bytes = read_binary_binding(
            self,
            &request.artifact_sha256,
            None,
            "artifact",
            MAX_REPLAY_ARTIFACT_BYTES,
        )?;

        let evidence = self
            .store
            .get_visual_evidence(&request.candidate_id)?
            .ok_or_else(|| integrity_error("visual evidence is unavailable"))?;
        if evidence.project_id != request.project_id
            || evidence.reference_id != request.reference_id
            || evidence.render_set_object_sha256 != request.render_set_object_sha256
            || evidence.comparison_report_object_sha256.as_deref()
                != Some(request.comparison_report_object_sha256.as_str())
            || evidence.quality_report_object_sha256 != request.quality_report_object_sha256
        {
            return Err(binding_error("visual evidence pointers differ"));
        }

        let artifact_readback = read_json_object(
            self,
            &request.artifact_readback_object_sha256,
            "ArtifactReadback@2",
        )?;
        validate_artifact_readback_v2_output(&artifact_readback)?;
        if artifact_readback.get("artifact_id").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
            || artifact_readback
                .get("candidate_id")
                .and_then(Value::as_str)
                != Some(request.candidate_id.as_str())
            || artifact_readback
                .get("object_sha256")
                .and_then(Value::as_str)
                != Some(request.artifact_sha256.as_str())
            || artifact_readback
                .get("program_sha256")
                .and_then(Value::as_str)
                != Some(request.program_sha256.as_str())
        {
            return Err(binding_error("artifact readback lineage differs"));
        }

        let render_set = read_json_object(self, &request.render_set_object_sha256, "RenderSet@2")?;
        validate_render_set_v2_output(&render_set)?;
        let comparison = read_json_object(
            self,
            &request.comparison_report_object_sha256,
            "ReferenceComparisonReport@1",
        )?;
        validate_reference_comparison_report(&comparison)?;
        let quality = read_json_object(
            self,
            &request.quality_report_object_sha256,
            "QualityReport@2",
        )?;
        validate_quality_report_v2_output(&quality)?;
        let camera = read_json_object(self, &request.camera_object_sha256, "CameraCalibration@1")?;
        validate_camera_calibration(&camera)?;

        validate_lineage(&request, &render_set, &comparison, &quality, &camera)?;
        validate_quality_derivation(&comparison, &quality)?;

        let pass_artifacts = render_set
            .get("pass_artifacts")
            .and_then(Value::as_object)
            .ok_or_else(|| integrity_error("RenderSet pass artifacts are unavailable"))?;
        let mut aov_artifacts = Vec::with_capacity(AOV_PASSES.len());
        for pass in AOV_PASSES {
            let artifact = pass_artifacts
                .get(pass)
                .and_then(Value::as_object)
                .ok_or_else(|| integrity_error("RenderSet AOV artifact is unavailable"))?;
            let sha256 = required_sha(artifact, "sha256")?;
            let declared_size = artifact
                .get("size_bytes")
                .and_then(Value::as_u64)
                .ok_or_else(|| integrity_error("AOV size is invalid"))?;
            ensure_aov_read_budget(declared_size, false)?;
            let bytes = self
                .cas_read_bounded(&sha256, MAX_AOV_PNG_BYTES)
                .map_err(|_| integrity_error("AOV CAS read failed within the 16 MiB budget"))?;
            if sha256_hex(&bytes) != sha256 || bytes.len() as u64 != declared_size {
                return Err(binding_error("AOV CAS hash or byte size differs"));
            }
            render_worker::validate_png_rgba8_bytes(&bytes, 512, 512)
                .map_err(|_| integrity_error("AOV is not an exact 512x512 RGBA8 PNG"))?;
            let color_space = artifact
                .get("color_space")
                .and_then(Value::as_str)
                .ok_or_else(|| integrity_error("AOV color space is invalid"))?;
            if color_space != if pass == "beauty" { "srgb" } else { "data" } {
                return Err(binding_error("AOV color-space semantic differs"));
            }
            aov_artifacts.push(json!({
                "pass":pass,
                "cas_object_sha256":sha256,
                "bytes_sha256":sha256_hex(&bytes),
                "size_bytes":declared_size,
                "width":512,
                "height":512,
                "mime":"image/png",
                "channels":"rgba8",
                "color_space":color_space,
                "cas_hash_verified":true,
                "png_decode_verified":true
            }));
        }

        let comparison_mask_sha256 = comparison
            .pointer("/mask/sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| integrity_error("comparison mask hash is invalid"))?;
        let comparison_mask = self
            .cas_read_bounded(comparison_mask_sha256, MAX_AOV_PNG_BYTES)
            .map_err(|_| {
                integrity_error("comparison mask CAS read failed within the 16 MiB budget")
            })?;
        if sha256_hex(&comparison_mask) != comparison_mask_sha256 {
            return Err(binding_error("comparison mask CAS hash differs"));
        }
        render_worker::validate_png_rgba8_bytes(&comparison_mask, 512, 512)
            .map_err(|_| integrity_error("comparison mask is not an exact 512x512 RGBA8 PNG"))?;

        let mut result = json!({
            "schema_version":"RenderEvidenceIntegrity@1",
            "projection_status":"projection/read-only",
            "read_only":true,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id,
            "artifact_sha256":request.artifact_sha256,
            "artifact_readback_object_sha256":request.artifact_readback_object_sha256,
            "program_sha256":request.program_sha256,
            "reference_id":request.reference_id,
            "reference_sha256":request.reference_sha256,
            "request_sha256":request.request_sha256,
            "camera_binding":{
                "camera_hash":request.camera_hash,
                "camera_object_sha256":request.camera_object_sha256,
                "camera_canonical_sha256":camera["canonical_sha256"],
                "render_set_camera_hash":render_set["camera_hash"],
                "comparison_camera_hash":comparison["camera_hash"],
                "identity_hash_verified":true,
                "object_hash_verified":true,
                "status":"same_camera_verified"
            },
            "object_hashes":{
                "artifact_readback":object_hash_row(&request.artifact_readback_object_sha256, &artifact_readback),
                "render_set":object_hash_row(&request.render_set_object_sha256, &render_set),
                "comparison_report":object_hash_row(&request.comparison_report_object_sha256, &comparison),
                "quality_report":object_hash_row(&request.quality_report_object_sha256, &quality)
            },
            "source_bytes_binding":{
                "artifact":artifact_bytes,
                "reference":reference_bytes
            },
            "render_profile_binding":{
                "renderer_hash":render_set["renderer_hash"],
                "render_profile_sha256":render_set["render_profile_sha256"],
                "aov_definition_sha256":render_set["aov_definition_sha256"],
                "color_pipeline_sha256":render_set["color_pipeline_sha256"],
                "id_palette_definition_sha256":render_set["id_palette_definition_sha256"],
                "render_worker_build_cohort_sha256":render_set["render_worker_build_cohort_sha256"],
                "render_worker_binding_status":render_set["render_worker_binding_status"]
            },
            "aov_artifacts":aov_artifacts,
            "comparison_mask_binding":{
                "mask_object_sha256":comparison_mask_sha256,
                "mask_bytes_sha256":sha256_hex(&comparison_mask),
                "width":512,
                "height":512,
                "mime":"image/png",
                "channels":"rgba8",
                "object_hash_verified":true,
                "png_decode_verified":true
            },
            "comparison_status":comparison["status"],
            "quality_gate_binding":{
                "visual_status":quality["visual_status"],
                "hard_gate_passed":quality["hard_gate_passed"],
                "threshold_revision":quality["threshold_revision"],
                "threshold_policy_sha256":quality["threshold_policy_sha256"],
                "threshold_source":quality["threshold_source"],
                "metric_gate_results":quality["metric_gate_results"]
            },
            "binding_status":"passed",
            "runtime_write_performed":false,
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "limitations":LIMITATIONS,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_output(&result)?;
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| integrity_error(&format!("output serialization failed: {error}")))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(integrity_error("response exceeds 1 MiB"));
        }
        Ok(result)
    }

    /// Re-run the fixed Render Worker twice against one exact integrity-bound
    /// artifact/camera and compare raw PNG plus decoded RGBA8 bytes with the
    /// persisted nine-AOV RenderSet. The replay is transient and read-only.
    pub fn render_evidence_replay_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let request = parse_replay_request(&request)?;
        let integrity = self.render_evidence_integrity_get(request.integrity_request.clone())?;
        let integrity_object = request
            .integrity_request
            .as_object()
            .ok_or_else(|| replay_invalid("integrity request is invalid"))?;
        let project_id = required_id(integrity_object, "project_id")?;
        let candidate_id = required_id(integrity_object, "candidate_id")?;
        let artifact_sha256 = required_sha(integrity_object, "artifact_sha256")?;
        let camera_hash = required_sha(integrity_object, "camera_hash")?;
        let camera_object_sha256 = required_sha(integrity_object, "camera_object_sha256")?;
        let render_set_object_sha256 = required_sha(integrity_object, "render_set_object_sha256")?;

        let candidate = self
            .candidate(&candidate_id)?
            .ok_or_else(|| replay_binding("candidate is unavailable"))?;
        if candidate.project_id != project_id
            || candidate.canonical_sha256 != request.candidate_state_sha256
        {
            return Err(replay_binding("candidate state differs"));
        }

        let artifact = self
            .cas_read_bounded(&artifact_sha256, MAX_REPLAY_ARTIFACT_BYTES)
            .map_err(|_| replay_invalid("artifact CAS read failed within the 64 MiB budget"))?;
        if artifact.is_empty() {
            return Err(replay_invalid(
                "artifact is outside the 64 MiB replay budget",
            ));
        }
        let artifact_readback_object_sha256 =
            required_sha(integrity_object, "artifact_readback_object_sha256")?;
        let program_sha256 = required_sha(integrity_object, "program_sha256")?;
        let artifact_readback =
            read_json_object(self, &artifact_readback_object_sha256, "ArtifactReadback@2")?;
        validate_artifact_readback_v2_output(&artifact_readback)?;
        validate_replay_strict_glb_readback(
            &artifact,
            &artifact_sha256,
            &candidate_id,
            &program_sha256,
            &artifact_readback,
        )?;
        let camera = read_json_object(self, &camera_object_sha256, "CameraCalibration@1")?;
        let render_set = read_json_object(self, &render_set_object_sha256, "RenderSet@2")?;
        validate_render_set_v2_output(&render_set)?;

        let first = render_glb_with_runtime_worker_identity(&artifact, &camera)
            .map_err(replay_worker_error)?;
        let repeated = render_glb_with_runtime_worker_identity(&artifact, &camera)
            .map_err(replay_worker_error)?;
        let source_cohort = validate_replay_worker_binding(&render_set, &first, &repeated)?;
        let rows = build_aov_replay_rows(self, &render_set, &first.passes, &repeated.passes)?;

        let candidate_after = self
            .candidate(&candidate_id)?
            .ok_or_else(|| replay_binding("candidate disappeared during replay"))?;
        if candidate_after.canonical_sha256 != request.candidate_state_sha256 {
            return Err(replay_binding("candidate state changed during replay"));
        }

        let replayed_profile_sha256 = required_sha(
            first
                .render_profile
                .as_object()
                .ok_or_else(|| replay_binding("replayed RenderProfile is invalid"))?,
            "canonical_sha256",
        )?;
        let mut result = json!({
            "schema_version":"RenderEvidenceReplay@1",
            "projection_status":"transient-replay/read-only",
            "read_only":true,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":request.candidate_state_sha256,
            "artifact_sha256":artifact_sha256,
            "camera_hash":camera_hash,
            "source_render_set_object_sha256":render_set_object_sha256,
            "request_sha256":request.request_sha256,
            "integrity_request_sha256":integrity_object["canonical_sha256"],
            "integrity_result_sha256":integrity["canonical_sha256"],
            "replay_policy":REPLAY_POLICY,
            "appearance_binding_status":"artifact-embedded-materials-only",
            "temporary_materialization":"in-memory-only",
            "render_profile_binding":{
                "source_render_profile_sha256":render_set["render_profile_sha256"],
                "replayed_render_profile_sha256":replayed_profile_sha256,
                "aov_definition_sha256":render_set["aov_definition_sha256"],
                "color_pipeline_sha256":render_set["color_pipeline_sha256"],
                "id_palette_definition_sha256":render_set["id_palette_definition_sha256"],
                "profile_match":true
            },
            "worker_cohort_binding":{
                "source_render_worker_build_cohort_sha256":source_cohort,
                "first_replay_render_worker_build_cohort_sha256":first.build_cohort_sha256,
                "repeat_replay_render_worker_build_cohort_sha256":repeated.build_cohort_sha256,
                "status":"same_cohort_verified"
            },
            "aov_replay_rows":rows,
            "mismatched_passes":[],
            "replay_status":"repeat_byte_exact_match",
            "determinism_claim":"repeat_byte_exact_same_cohort",
            "binding_status":"passed",
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "limitations":REPLAY_LIMITATIONS,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_replay_output(&result)?;
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| replay_invalid(&format!("output serialization failed: {error}")))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(replay_invalid("response exceeds 1 MiB"));
        }
        Ok(result)
    }
}

fn validate_replay_strict_glb_readback(
    artifact: &[u8],
    artifact_sha256: &str,
    candidate_id: &str,
    program_sha256: &str,
    readback: &Value,
) -> Result<(), RuntimeError> {
    let inspection = strict_glb_inspection(artifact)
        .map_err(|error| replay_binding(&format!("strict GLB readback failed: {error}")))?;
    let part_bindings = inspection
        .part_bindings
        .iter()
        .map(|binding| {
            json!({
                "part_id":binding.part_id,
                "source_node_id":binding.source_node_id,
                "material_zone_id":binding.material_zone_id,
                "solid":binding.solid,
                "triangle_count":binding.triangle_count
            })
        })
        .collect::<Vec<_>>();
    if !inspection.hard_gate_passed
        || inspection.artifact_schema_version != "ArtifactReadback@2"
        || inspection.program_sha256 != program_sha256
        || readback.get("artifact_id").and_then(Value::as_str) != Some(artifact_sha256)
        || readback.get("object_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || readback.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || readback.get("program_sha256").and_then(Value::as_str) != Some(program_sha256)
        || readback.get("size_bytes").and_then(Value::as_u64) != Some(artifact.len() as u64)
        || readback.get("triangle_count").and_then(Value::as_u64) != Some(inspection.triangle_count)
        || readback.get("part_ids") != Some(&json!(inspection.part_ids))
        || readback.get("source_node_ids") != Some(&json!(inspection.source_node_ids))
        || readback.get("material_zone_ids") != Some(&json!(inspection.material_zone_ids))
        || readback.get("part_bindings") != Some(&json!(part_bindings))
        || readback
            .get("readback_config_sha256")
            .and_then(Value::as_str)
            != Some(inspection.readback_config_sha256.as_str())
        || readback.get("integrity") != Some(&strict_integrity_value(&inspection))
    {
        return Err(replay_binding(
            "current strict GLB readback differs from persisted ArtifactReadback@2",
        ));
    }
    Ok(())
}

fn parse_replay_request(value: &Value) -> Result<ReplayRequest, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "candidate_state_sha256",
            "integrity_request",
            "replay_policy",
            "canonical_sha256",
        ],
        "RenderEvidenceReplayRequest@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderEvidenceReplayRequest@1")
        || object.get("replay_policy").and_then(Value::as_str) != Some(REPLAY_POLICY)
    {
        return Err(replay_invalid("request constants differ"));
    }
    let candidate_state_sha256 = required_sha(object, "candidate_state_sha256")?;
    let integrity_request = object
        .get("integrity_request")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| replay_invalid("integrity_request is invalid"))?;
    let request_sha256 = required_sha(object, "canonical_sha256")?;
    verify_output_canonical_hash(value, "RenderEvidenceReplayRequest@1")?;
    Ok(ReplayRequest {
        candidate_state_sha256,
        integrity_request,
        request_sha256,
    })
}

fn validate_replay_worker_binding(
    render_set: &Value,
    first: &super::RuntimeRenderResult,
    repeated: &super::RuntimeRenderResult,
) -> Result<String, RuntimeError> {
    if render_set
        .get("render_worker_binding_status")
        .and_then(Value::as_str)
        != Some("same_cohort_verified")
    {
        return Err(replay_cohort_unavailable());
    }
    let source_cohort = render_set
        .get("render_worker_build_cohort_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(replay_cohort_unavailable)?
        .to_owned();
    if first.build_cohort_sha256.as_deref() != Some(source_cohort.as_str())
        || repeated.build_cohort_sha256.as_deref() != Some(source_cohort.as_str())
    {
        return Err(replay_binding("Render Worker cohort differs"));
    }
    if first.render_profile != render_set["render_profile"]
        || repeated.render_profile != render_set["render_profile"]
        || first.render_profile != repeated.render_profile
    {
        return Err(replay_binding("RenderProfile differs"));
    }
    Ok(source_cohort)
}

fn build_aov_replay_rows(
    runtime: &Runtime,
    render_set: &Value,
    first: &[render_worker::RenderPass],
    repeated: &[render_worker::RenderPass],
) -> Result<Vec<Value>, RuntimeError> {
    if first.len() != AOV_PASSES.len() || repeated.len() != AOV_PASSES.len() {
        return Err(replay_binding("replayed AOV count differs"));
    }
    let artifacts = render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| replay_binding("source AOV inventory is invalid"))?;
    let mut rows = Vec::with_capacity(AOV_PASSES.len());
    for ((pass_name, first_pass), repeat_pass) in AOV_PASSES.into_iter().zip(first).zip(repeated) {
        if first_pass.pass != pass_name
            || repeat_pass.pass != pass_name
            || first_pass.width != 512
            || first_pass.height != 512
            || repeat_pass.width != 512
            || repeat_pass.height != 512
        {
            return Err(replay_binding("replayed AOV order or dimensions differ"));
        }
        let source = artifacts
            .get(pass_name)
            .and_then(Value::as_object)
            .ok_or_else(|| replay_binding("source AOV is unavailable"))?;
        let source_sha256 = required_sha(source, "sha256")?;
        let source_size = source
            .get("size_bytes")
            .and_then(Value::as_u64)
            .ok_or_else(|| replay_binding("source AOV size is invalid"))?;
        ensure_aov_read_budget(source_size, true)?;
        let color_space = source
            .get("color_space")
            .and_then(Value::as_str)
            .ok_or_else(|| replay_binding("source AOV color space is invalid"))?;
        let source_png = runtime
            .cas_read_bounded(&source_sha256, MAX_AOV_PNG_BYTES)
            .map_err(|_| replay_invalid("source AOV CAS read failed within the 16 MiB budget"))?;
        if source_png.len() as u64 != source_size || sha256_hex(&source_png) != source_sha256 {
            return Err(replay_binding("source AOV CAS bytes differ"));
        }
        let source_pixels = render_worker::decode_png_rgba8_pixels(&source_png, 512, 512)
            .map_err(|_| replay_png_invalid())?;
        let first_pixels = render_worker::decode_png_rgba8_pixels(&first_pass.png, 512, 512)
            .map_err(|_| replay_png_invalid())?;
        let repeat_pixels = render_worker::decode_png_rgba8_pixels(&repeat_pass.png, 512, 512)
            .map_err(|_| replay_png_invalid())?;
        let byte_exact = source_png == first_pass.png;
        let repeat_byte_exact = first_pass.png == repeat_pass.png;
        let pixel_exact = source_pixels == first_pixels && first_pixels == repeat_pixels;
        if !byte_exact || !repeat_byte_exact || !pixel_exact {
            return Err(replay_aov_mismatch(pass_name));
        }
        rows.push(json!({
            "pass":pass_name,
            "source_cas_object_sha256":source_sha256,
            "source_bytes_sha256":sha256_hex(&source_png),
            "source_pixel_sha256":sha256_hex(&source_pixels),
            "first_replay_bytes_sha256":sha256_hex(&first_pass.png),
            "first_replay_pixel_sha256":sha256_hex(&first_pixels),
            "repeat_replay_bytes_sha256":sha256_hex(&repeat_pass.png),
            "repeat_replay_pixel_sha256":sha256_hex(&repeat_pixels),
            "source_size_bytes":source_size,
            "first_replay_size_bytes":first_pass.png.len(),
            "repeat_replay_size_bytes":repeat_pass.png.len(),
            "width":512,
            "height":512,
            "mime":"image/png",
            "channels":"rgba8",
            "color_space":color_space,
            "source_cas_verified":true,
            "first_replay_png_decode_verified":true,
            "repeat_replay_png_decode_verified":true,
            "byte_exact":true,
            "pixel_exact":true,
            "repeat_byte_exact":true
        }));
    }
    Ok(rows)
}

fn validate_replay_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "projection_status",
            "read_only",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_sha256",
            "camera_hash",
            "source_render_set_object_sha256",
            "request_sha256",
            "integrity_request_sha256",
            "integrity_result_sha256",
            "replay_policy",
            "appearance_binding_status",
            "temporary_materialization",
            "render_profile_binding",
            "worker_cohort_binding",
            "aov_replay_rows",
            "mismatched_passes",
            "replay_status",
            "determinism_claim",
            "binding_status",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "max_response_bytes",
            "limitations",
            "canonical_sha256",
        ],
        "RenderEvidenceReplay@1",
    )?;
    let rows = object
        .get("aov_replay_rows")
        .and_then(Value::as_array)
        .ok_or_else(|| replay_invalid("output AOV rows are invalid"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderEvidenceReplay@1")
        || object.get("projection_status").and_then(Value::as_str)
            != Some("transient-replay/read-only")
        || object.get("read_only") != Some(&Value::Bool(true))
        || object.get("replay_policy").and_then(Value::as_str) != Some(REPLAY_POLICY)
        || object.get("replay_status").and_then(Value::as_str) != Some("repeat_byte_exact_match")
        || object.get("determinism_claim").and_then(Value::as_str)
            != Some("repeat_byte_exact_same_cohort")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("persistent_user_data_touched") != Some(&Value::Bool(false))
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
        || object.get("mismatched_passes") != Some(&json!([]))
        || rows.len() != AOV_PASSES.len()
        || rows.iter().zip(AOV_PASSES).any(|(row, pass)| {
            row.get("pass").and_then(Value::as_str) != Some(pass)
                || row.get("byte_exact") != Some(&Value::Bool(true))
                || row.get("pixel_exact") != Some(&Value::Bool(true))
                || row.get("repeat_byte_exact") != Some(&Value::Bool(true))
        })
        || object.get("limitations") != Some(&json!(REPLAY_LIMITATIONS))
    {
        return Err(replay_invalid("output constants or replay rows differ"));
    }
    verify_output_canonical_hash(value, "RenderEvidenceReplay@1")
}

fn parse_request(value: &Value) -> Result<IntegrityRequest, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_sha256",
            "artifact_readback_object_sha256",
            "program_sha256",
            "reference_id",
            "reference_sha256",
            "camera_hash",
            "camera_object_sha256",
            "render_set_object_sha256",
            "comparison_report_object_sha256",
            "quality_report_object_sha256",
            "canonical_sha256",
        ],
        "RenderEvidenceIntegrityRequest@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some("RenderEvidenceIntegrityRequest@1")
    {
        return Err(integrity_error("request schema_version differs"));
    }
    let request_sha256 = required_sha(object, "canonical_sha256")?;
    verify_output_canonical_hash(value, "RenderEvidenceIntegrityRequest@1")?;
    Ok(IntegrityRequest {
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        artifact_sha256: required_sha(object, "artifact_sha256")?,
        artifact_readback_object_sha256: required_sha(object, "artifact_readback_object_sha256")?,
        program_sha256: required_sha(object, "program_sha256")?,
        reference_id: required_id(object, "reference_id")?,
        reference_sha256: required_sha(object, "reference_sha256")?,
        camera_hash: required_sha(object, "camera_hash")?,
        camera_object_sha256: required_sha(object, "camera_object_sha256")?,
        render_set_object_sha256: required_sha(object, "render_set_object_sha256")?,
        comparison_report_object_sha256: required_sha(object, "comparison_report_object_sha256")?,
        quality_report_object_sha256: required_sha(object, "quality_report_object_sha256")?,
        request_sha256,
    })
}

fn validate_lineage(
    request: &IntegrityRequest,
    render_set: &Value,
    comparison: &Value,
    quality: &Value,
    camera: &Value,
) -> Result<(), RuntimeError> {
    if render_set.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || render_set.get("program_sha256").and_then(Value::as_str)
            != Some(request.program_sha256.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(request.reference_id.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(request.camera_hash.as_str())
        || render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            != Some(request.camera_object_sha256.as_str())
    {
        return Err(binding_error("RenderSet lineage differs"));
    }
    if comparison.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || comparison.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || comparison.get("reference_id").and_then(Value::as_str)
            != Some(request.reference_id.as_str())
        || comparison.get("reference_sha256").and_then(Value::as_str)
            != Some(request.reference_sha256.as_str())
        || comparison.get("render_set_hash").and_then(Value::as_str)
            != Some(request.render_set_object_sha256.as_str())
        || comparison.get("camera_hash").and_then(Value::as_str)
            != Some(request.camera_hash.as_str())
    {
        return Err(binding_error("comparison lineage differs"));
    }
    if quality.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || quality.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || quality.get("program_sha256").and_then(Value::as_str)
            != Some(request.program_sha256.as_str())
        || quality.get("reference_id").and_then(Value::as_str)
            != Some(request.reference_id.as_str())
        || quality.get("reference_sha256").and_then(Value::as_str)
            != Some(request.reference_sha256.as_str())
        || quality.get("render_set_hash").and_then(Value::as_str)
            != Some(request.render_set_object_sha256.as_str())
        || quality
            .get("comparison_report_hash")
            .and_then(Value::as_str)
            != Some(request.comparison_report_object_sha256.as_str())
    {
        return Err(binding_error("quality lineage differs"));
    }
    if camera.get("camera_hash").and_then(Value::as_str) != Some(request.camera_hash.as_str())
        || camera_identity_hash(camera)? != request.camera_hash
    {
        return Err(binding_error("camera identity differs"));
    }
    Ok(())
}

fn validate_quality_derivation(comparison: &Value, quality: &Value) -> Result<(), RuntimeError> {
    let metrics = comparison
        .get("metrics")
        .ok_or_else(|| integrity_error("comparison metrics are unavailable"))?;
    let expected_gate_results = Value::Array(visible_view_gate_checks(metrics));
    let comparison_status = comparison
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| integrity_error("comparison status is unavailable"))?;
    if quality.get("metric_gate_results") != Some(&expected_gate_results)
        || quality.get("visual_status").and_then(Value::as_str) != Some(comparison_status)
        || quality.get("hard_gate_passed").and_then(Value::as_bool)
            != Some(comparison_status == "PARTIAL_VISIBLE_VIEW_PASS")
        || quality.get("benchmark_eligibility") != comparison.get("benchmark_eligibility")
    {
        return Err(binding_error(
            "quality metrics, status or benchmark eligibility are not derived from comparison",
        ));
    }
    Ok(())
}

fn read_json_object(
    runtime: &Runtime,
    object_sha256: &str,
    context: &str,
) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_object(object_sha256)?
        .ok_or_else(|| integrity_error(&format!("{context} CAS object is unavailable")))?;
    if record.mime != "application/json" || record.size_bytes > MAX_JSON_OBJECT_BYTES {
        return Err(integrity_error(&format!(
            "{context} CAS metadata or 1 MiB budget differs"
        )));
    }
    let bytes = runtime.cas_read(object_sha256)?;
    if sha256_hex(&bytes) != object_sha256 || bytes.len() as u64 != record.size_bytes {
        return Err(binding_error(&format!("{context} CAS object hash differs")));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| integrity_error(&format!("{context} is not JSON")))?;
    if !value.is_object() {
        return Err(integrity_error(&format!("{context} is not an object")));
    }
    Ok(value)
}

fn read_binary_binding(
    runtime: &Runtime,
    object_sha256: &str,
    expected_metadata: Option<(&str, u64)>,
    context: &str,
    max_bytes: u64,
) -> Result<Value, RuntimeError> {
    let record = runtime
        .store
        .get_object(object_sha256)?
        .ok_or_else(|| integrity_error(&format!("{context} CAS object is unavailable")))?;
    if let Some((expected_mime, expected_size)) = expected_metadata {
        if record.mime != expected_mime || record.size_bytes != expected_size {
            return Err(binding_error(&format!("{context} CAS metadata differs")));
        }
    }
    ensure_binary_read_budget(record.size_bytes, max_bytes, context)?;
    let bytes = runtime
        .cas_read_bounded(object_sha256, max_bytes)
        .map_err(|_| integrity_error(&format!("{context} CAS read failed within budget")))?;
    if sha256_hex(&bytes) != object_sha256 || bytes.len() as u64 != record.size_bytes {
        return Err(binding_error(&format!("{context} CAS bytes differ")));
    }
    Ok(json!({
        "cas_object_sha256":object_sha256,
        "bytes_sha256":sha256_hex(&bytes),
        "size_bytes":record.size_bytes,
        "mime":record.mime,
        "cas_hash_verified":true
    }))
}

fn object_hash_row(object_sha256: &str, value: &Value) -> Value {
    json!({
        "object_sha256":object_sha256,
        "canonical_sha256":value["canonical_sha256"],
        "object_hash_verified":true
    })
}

fn validate_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "projection_status",
            "read_only",
            "project_id",
            "candidate_id",
            "artifact_sha256",
            "artifact_readback_object_sha256",
            "program_sha256",
            "reference_id",
            "reference_sha256",
            "request_sha256",
            "camera_binding",
            "object_hashes",
            "source_bytes_binding",
            "render_profile_binding",
            "aov_artifacts",
            "comparison_mask_binding",
            "comparison_status",
            "quality_gate_binding",
            "binding_status",
            "runtime_write_performed",
            "max_response_bytes",
            "limitations",
            "canonical_sha256",
        ],
        "RenderEvidenceIntegrity@1",
    )?;
    let aovs = object
        .get("aov_artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| integrity_error("output AOV inventory is invalid"))?;
    let source_bytes = object
        .get("source_bytes_binding")
        .and_then(Value::as_object)
        .ok_or_else(|| integrity_error("output source byte bindings are invalid"))?;
    let source_bytes_valid = ["artifact", "reference"].iter().all(|key| {
        source_bytes.get(*key).is_some_and(|row| {
            row.get("cas_object_sha256") == row.get("bytes_sha256")
                && row.get("cas_hash_verified") == Some(&Value::Bool(true))
        })
    });
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderEvidenceIntegrity@1")
        || object.get("projection_status").and_then(Value::as_str) != Some("projection/read-only")
        || object.get("read_only") != Some(&Value::Bool(true))
        || object.get("binding_status").and_then(Value::as_str) != Some("passed")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
        || aovs.len() != AOV_PASSES.len()
        || aovs.iter().zip(AOV_PASSES).any(|(row, pass)| {
            row.get("pass").and_then(Value::as_str) != Some(pass)
                || row.get("cas_hash_verified") != Some(&Value::Bool(true))
                || row.get("png_decode_verified") != Some(&Value::Bool(true))
        })
        || !source_bytes_valid
        || object.get("limitations") != Some(&json!(LIMITATIONS))
    {
        return Err(integrity_error("output constants or AOV inventory differ"));
    }
    verify_output_canonical_hash(value, "RenderEvidenceIntegrity@1")
}

fn required_id(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| integrity_error(&format!("{key} is not an identifier")))
}

fn required_sha(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| integrity_error(&format!("{key} is not SHA-256")))
}

fn integrity_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_INTEGRITY_INVALID: {detail}"))
}

fn binding_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_BINDING_MISMATCH: {detail}"))
}

fn replay_invalid(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_REPLAY_INVALID: {detail}"))
}

fn replay_binding(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_REPLAY_BINDING_MISMATCH: {detail}"))
}

fn replay_cohort_unavailable() -> RuntimeError {
    RuntimeError::InvalidInput("RENDER_EVIDENCE_REPLAY_COHORT_UNAVAILABLE".to_owned())
}

fn replay_png_invalid() -> RuntimeError {
    RuntimeError::InvalidInput("RENDER_EVIDENCE_REPLAY_PNG_INVALID".to_owned())
}

fn replay_aov_mismatch(pass: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_REPLAY_AOV_BYTES_MISMATCH: {pass}"))
}

fn replay_worker_error(error: super::geometry_worker::GeometryWorkerError) -> RuntimeError {
    RuntimeError::InvalidInput(format!("RENDER_EVIDENCE_REPLAY_WORKER_FAILED: {error}"))
}

fn ensure_aov_read_budget(declared_size: u64, replay: bool) -> Result<(), RuntimeError> {
    if declared_size == 0 || declared_size > MAX_AOV_PNG_BYTES {
        return Err(if replay {
            RuntimeError::InvalidInput(
                "RENDER_EVIDENCE_REPLAY_INPUT_BUDGET_EXCEEDED: AOV PNG exceeds 16 MiB".to_owned(),
            )
        } else {
            integrity_error("AOV PNG exceeds the 16 MiB read budget")
        });
    }
    Ok(())
}

fn ensure_binary_read_budget(
    declared_size: u64,
    max_bytes: u64,
    context: &str,
) -> Result<(), RuntimeError> {
    if declared_size == 0 || declared_size > max_bytes {
        return Err(integrity_error(&format!(
            "{context} exceeds the bounded CAS read budget"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comparison_and_quality() -> (Value, Value) {
        let metrics = json!({
            "silhouette_iou":0.1,
            "boundary_f1_4px":0.1,
            "bbox_edge_error":0.9,
            "centroid_error":0.9,
            "landmark_coverage":0.1,
            "landmark_nme":0.9,
            "region_median_iou":0.1,
            "critical_region_min_iou":0.1
        });
        let comparison = json!({
            "metrics":metrics,
            "status":"QUALITY_TARGET_NOT_MET",
            "benchmark_eligibility":"READY_PARTIAL_VIEW"
        });
        let quality = json!({
            "metric_gate_results":visible_view_gate_checks(&comparison["metrics"]),
            "visual_status":"QUALITY_TARGET_NOT_MET",
            "hard_gate_passed":false,
            "benchmark_eligibility":"READY_PARTIAL_VIEW"
        });
        (comparison, quality)
    }

    #[test]
    fn quality_derivation_rejects_metric_status_and_eligibility_drift() {
        let (comparison, quality) = comparison_and_quality();
        validate_quality_derivation(&comparison, &quality).expect("derived quality");

        let mut metric_drift = quality.clone();
        metric_drift["metric_gate_results"][0]["observed"] = json!(0.2);
        assert!(validate_quality_derivation(&comparison, &metric_drift).is_err());

        let mut status_drift = quality.clone();
        status_drift["visual_status"] = json!("PARTIAL_VISIBLE_VIEW_PASS");
        status_drift["hard_gate_passed"] = json!(true);
        assert!(validate_quality_derivation(&comparison, &status_drift).is_err());

        let mut eligibility_drift = quality;
        eligibility_drift["benchmark_eligibility"] = json!("BLOCKED_USER_CONFIRMATION_REQUIRED");
        assert!(validate_quality_derivation(&comparison, &eligibility_drift).is_err());
    }

    #[test]
    fn replay_aov_budget_is_rejected_before_any_cas_read() {
        let error = ensure_aov_read_budget(MAX_AOV_PNG_BYTES + 1, true)
            .expect_err("oversized AOV must fail closed");
        assert!(error
            .to_string()
            .contains("RENDER_EVIDENCE_REPLAY_INPUT_BUDGET_EXCEEDED"));
        ensure_aov_read_budget(MAX_AOV_PNG_BYTES, true).expect("bounded AOV");
    }

    #[test]
    fn integrity_binary_budget_is_rejected_before_any_cas_read() {
        let error = ensure_binary_read_budget(
            MAX_REFERENCE_SOURCE_BYTES + 1,
            MAX_REFERENCE_SOURCE_BYTES,
            "reference",
        )
        .expect_err("oversized reference must fail closed");
        assert!(error
            .to_string()
            .contains("reference exceeds the bounded CAS read budget"));
        ensure_binary_read_budget(
            MAX_REPLAY_ARTIFACT_BYTES,
            MAX_REPLAY_ARTIFACT_BYTES,
            "artifact",
        )
        .expect("bounded artifact");
    }
}
