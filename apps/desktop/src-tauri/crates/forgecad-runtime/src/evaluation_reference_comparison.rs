//! Runtime-owned Evaluation reference comparison and visual review family.
//!
//! This module owns the complete typed implementation for reference comparison,
//! render-pass readback, visual-evidence projection, Codex review, and human
//! visual review.  All durable writes continue through the parent Runtime's
//! Store/CAS boundary; this is a physical extraction, not a second writer.

use base64::Engine;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    calibrate_default_camera, calibrate_default_camera_height_only, camera_fit_cache_key,
    camera_fit_score, canonical_json_bytes, canonical_json_hash, compare_masks_with_parts,
    decode_binary_mask, default_camera_calibration, mask_to_png, now_string,
    project_reference_mask_to_view, reference_annotation_readiness, reference_mask_png,
    render_glb_with_runtime_worker_identity, render_worker_binding_status, required_value_id,
    required_value_sha, sha256_hex, strict_glb_inspection, validate_camera_calibration,
    validate_human_review_receipt, validate_id, validate_quality_report_v2_output,
    validate_reference_comparison_report, validate_reference_view_spec,
    validate_render_set_v2_output, validate_request_keys, validate_visual_review_report,
    visible_view_gate_checks, visible_view_gate_passes, visible_view_threshold_policy_sha256,
    CasObject, Runtime, RuntimeError, VisualEvidenceRecord, VISIBLE_VIEW_THRESHOLD_REVISION,
    VISIBLE_VIEW_THRESHOLD_SOURCE,
};

impl Runtime {
    pub fn prepare_reference_comparison(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let mut ignored_objects = Vec::new();
        self.prepare_reference_comparison_with_projection(
            project_id,
            request,
            true,
            None,
            None,
            &mut ignored_objects,
        )
    }

    /// Produce immutable comparison artifacts without replacing the mutable
    /// latest-view observation projection. Cross-view evidence owns and links
    /// these hashes directly, so historical DesignSession/FormEvidence
    /// bindings remain restart-readable after the comparison.
    pub(crate) fn prepare_reference_comparison_detached(
        &self,
        project_id: &str,
        request: Value,
        reservation: &forgecad_store::CasReservation,
        reserved_objects: &mut Vec<CasObject>,
    ) -> Result<Value, RuntimeError> {
        self.prepare_reference_comparison_with_projection(
            project_id,
            request,
            false,
            Some(reservation),
            None,
            reserved_objects,
        )
    }

    /// Fresh FormArt baseline variant of the detached comparison producer.
    /// Every derived CAS object is preclaimed by the Store-owned durable batch
    /// before bytes are installed, closing the process-crash ownership gap.
    pub(crate) fn prepare_reference_comparison_detached_form_art_batch(
        &self,
        project_id: &str,
        request: Value,
        batch: &forgecad_store::ProductionWeaponFormArtBaselineCasBatch,
        reserved_objects: &mut Vec<CasObject>,
    ) -> Result<Value, RuntimeError> {
        self.prepare_reference_comparison_with_projection(
            project_id,
            request,
            false,
            Some(batch.reservation()),
            Some(batch),
            reserved_objects,
        )
    }

    fn prepare_reference_comparison_with_projection(
        &self,
        project_id: &str,
        request: Value,
        update_visual_evidence_projection: bool,
        reservation: Option<&forgecad_store::CasReservation>,
        form_art_batch: Option<&forgecad_store::ProductionWeaponFormArtBaselineCasBatch>,
        reserved_objects: &mut Vec<CasObject>,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("reference comparison request must be an object".to_owned())
        })?;
        validate_request_keys(
            object,
            &[
                "project_id",
                "candidate_id",
                "reference_id",
                "view_spec",
                "camera",
                "target_sha256",
                "view_id",
            ],
            "reference_compare_prepare",
        )?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let view_id = object
            .get("view_id")
            .map(|value| required_value_id(Some(value), "view_id"))
            .transpose()?
            .map(str::to_owned);
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: candidate not found".to_owned())
        })?;
        if candidate.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: candidate is outside the target project".to_owned(),
            ));
        }
        let reference = self.reference(reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput("NOT_FOUND: reference not found".to_owned())
        })?;
        if reference.project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_SCOPE_DENIED: reference is outside the target project".to_owned(),
            ));
        }
        let view_spec = object
            .get("view_spec")
            .ok_or_else(|| RuntimeError::InvalidInput("REFERENCE_VIEW_SPEC_REQUIRED".to_owned()))?;
        validate_reference_view_spec(view_spec, &reference)?;
        let explicit_camera = object.get("camera").is_some_and(|value| !value.is_null());
        let target_sha256 = object
            .get("target_sha256")
            .map(|value| required_value_sha(Some(value), "target_sha256"))
            .transpose()?
            .map(str::to_owned);
        if let Some(target_sha256) = target_sha256.as_deref() {
            let target = self.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str) != Some(reference_id) {
                return Err(RuntimeError::InvalidInput(
                    "REFERENCE_SCOPE_DENIED: silhouette target is bound to another reference"
                        .to_owned(),
                ));
            }
        }
        let mut reused_cached_camera_fit = false;
        let mut camera = match object.get("camera").filter(|value| !value.is_null()) {
            None => {
                let cached_camera = target_sha256.as_deref().and_then(|target_sha256| {
                    let cache_key = camera_fit_cache_key(project_id, candidate_id, target_sha256);
                    self.camera_fit_cache
                        .lock()
                        .ok()
                        .and_then(|cache| cache.get(&cache_key).cloned())
                        .and_then(|result| result.get("selected_camera").cloned())
                });
                if let Some(cached_camera) = cached_camera {
                    reused_cached_camera_fit = true;
                    cached_camera
                } else {
                    default_camera_calibration()
                }
            }
            Some(value)
                if value.get("schema_version").and_then(Value::as_str)
                    == Some("CameraCalibrationRef@1") =>
            {
                let target_sha256 = target_sha256.as_deref().ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "CAMERA_CALIBRATION_INVALID: CameraCalibrationRef@1 requires target_sha256"
                            .to_owned(),
                    )
                })?;
                self.resolve_silhouette_fit_camera(project_id, candidate_id, target_sha256, value)?
            }
            Some(value) => value.clone(),
        };
        validate_camera_calibration(&camera)?;
        let artifact_sha256 = candidate
            .manifest_hash
            .clone()
            .or(candidate.prepared_object_sha256.clone())
            .ok_or_else(|| {
                RuntimeError::InvalidInput("CANDIDATE_ARTIFACT_UNAVAILABLE".to_owned())
            })?;
        let glb = self.cas_read(&artifact_sha256)?;
        let inspection = strict_glb_inspection(&glb)?;
        if !inspection.hard_gate_passed {
            return Err(RuntimeError::InvalidInput(format!(
                "RENDER_REJECTED: strict GLB readback failed: {}",
                inspection.failure_codes.join(",")
            )));
        }
        let initial_render = render_glb_with_runtime_worker_identity(&glb, &camera)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let mut render_worker_cohort = initial_render.build_cohort_sha256.clone();
        let render_profile = initial_render.render_profile.clone();
        let mut render_passes = initial_render.passes;
        let (mut reference_mask, reference_mask_method, reference_mask_revision) =
            if let Some(target_sha256) = target_sha256.as_deref() {
                // A caller-supplied SilhouetteTarget is the reviewed contour
                // truth for this comparison. Falling back to a fresh
                // flood-fill here would make camera fitting and the final
                // quality gate evaluate different masks despite sharing one
                // target hash.
                let target = self.read_silhouette_target(target_sha256)?;
                (
                    self.target_mask(target_sha256, &target)?,
                    "silhouette-target",
                    "target-1",
                )
            } else {
                let reference_bytes = self.cas_read(&reference.object_sha256)?;
                (
                    reference_mask_png(&reference_bytes)?,
                    "local-border-flood-fill-morphology",
                    "mask-2",
                )
            };
        reference_mask.mask = project_reference_mask_to_view(
            &reference_mask.mask,
            view_spec,
            target_sha256.is_some(),
        )?;
        reference_mask.png = mask_to_png(&reference_mask.mask)?;
        if !explicit_camera && !reused_cached_camera_fit {
            let initial_silhouette = render_passes
                .iter()
                .find(|pass| pass.pass == "silhouette")
                .map(|pass| decode_binary_mask(&pass.png))
                .transpose()?;
            if let Some(initial_silhouette) = initial_silhouette {
                // Compare a small deterministic set of framing candidates and
                // keep the one with the best combined silhouette/boundary/
                // extent/centroid score. This prevents a height-only fit from
                // improving one metric while making the overall reference
                // comparison worse. Only the winning render is persisted.
                let mut best_camera = camera.clone();
                let mut best_passes = std::mem::take(&mut render_passes);
                let mut best_score = camera_fit_score(&reference_mask.mask, &initial_silhouette);
                for candidate in [
                    calibrate_default_camera_height_only(
                        &camera,
                        &reference_mask.mask,
                        &initial_silhouette,
                    ),
                    calibrate_default_camera(&camera, &reference_mask.mask, &initial_silhouette),
                ] {
                    if candidate == camera {
                        continue;
                    }
                    validate_camera_calibration(&candidate)?;
                    let candidate_render =
                        render_glb_with_runtime_worker_identity(&glb, &candidate).map_err(
                            |error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")),
                        )?;
                    let candidate_silhouette = candidate_render
                        .passes
                        .iter()
                        .find(|pass| pass.pass == "silhouette")
                        .map(|pass| decode_binary_mask(&pass.png))
                        .transpose()?
                        .ok_or_else(|| {
                            RuntimeError::InvalidInput(
                                "RENDER_REJECTED: calibrated silhouette pass missing".to_owned(),
                            )
                        })?;
                    let score = camera_fit_score(&reference_mask.mask, &candidate_silhouette);
                    if score > best_score {
                        best_score = score;
                        best_camera = candidate;
                        render_worker_cohort = candidate_render.build_cohort_sha256.clone();
                        best_passes = candidate_render.passes;
                    }
                }
                camera = best_camera;
                render_passes = best_passes;
            }
        }
        let camera_bytes = canonical_json_bytes(&camera)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let camera_object = self.put_reference_comparison_object(
            reservation,
            form_art_batch,
            reserved_objects,
            &camera_bytes,
            None,
            "application/json",
            "camera-calibration",
        )?;
        if render_passes.len() != 9
            || render_passes
                .iter()
                .any(|pass| pass.width != 512 || pass.height != 512)
        {
            return Err(RuntimeError::InvalidInput(
                "RENDER_REJECTED: fixed renderer did not return nine 512x512 passes".to_owned(),
            ));
        }
        let mut pass_artifacts = serde_json::Map::new();
        let mut pass_bytes = std::collections::HashMap::new();
        for pass in &render_passes {
            let stored = self.put_reference_comparison_object(
                reservation,
                form_art_batch,
                reserved_objects,
                &pass.png,
                None,
                "image/png",
                &format!("render-pass-{}", pass.pass),
            )?;
            pass_bytes.insert(pass.pass.clone(), pass.png.clone());
            pass_artifacts.insert(
                pass.pass.clone(),
                json!({
                    "sha256":stored.record.sha256,
                    "mime":"image/png",
                    "size_bytes":stored.record.size_bytes,
                    "width":512,
                    "height":512,
                    "channels":"rgba8",
                    "color_space":if pass.pass == "beauty" {"srgb"} else {"data"}
                }),
            );
        }
        let mut render_set = json!({
            "schema_version":"RenderSet@2",
            "render_set_id":format!("render-set-{}", &artifact_sha256[..32]),
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "camera_hash":camera["camera_hash"].clone(),
            "camera_object_sha256":camera_object.record.sha256.clone(),
            "renderer_hash":sha256_hex(b"forgecad-renderer-2"),
            "render_profile":render_profile.clone(),
            "render_profile_sha256":render_profile["canonical_sha256"].clone(),
            "aov_definition_sha256":render_profile["aov_definition_sha256"].clone(),
            "color_pipeline_sha256":render_profile["color_pipeline_sha256"].clone(),
            "id_palette_definition_sha256":render_profile["id_palette_definition_sha256"].clone(),
            "render_worker_build_cohort_sha256":render_worker_cohort.clone(),
            "render_worker_binding_status":render_worker_binding_status(render_worker_cohort.as_ref()),
            "width":512,
            "height":512,
            "passes":["beauty","silhouette","depth","normal","ao","part-id","material-id","wireframe","uv-stretch"],
            "pass_artifacts":pass_artifacts,
            "canonical_sha256":""
        });
        if let Some(view_id) = view_id.as_deref() {
            render_set["view_id"] = Value::String(view_id.to_owned());
        }
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        validate_render_set_v2_output(&render_set)?;
        let render_set_bytes = canonical_json_bytes(&render_set)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let render_set_object = self.put_reference_comparison_object(
            reservation,
            form_art_batch,
            reserved_objects,
            &render_set_bytes,
            None,
            "application/json",
            "render-set-v2",
        )?;
        let render_set_hash = render_set_object.record.sha256.clone();
        let mask_object = self.put_reference_comparison_object(
            reservation,
            form_art_batch,
            reserved_objects,
            &reference_mask.png,
            None,
            "image/png",
            // reference_mask_prepare may already have admitted these exact
            // deterministic bytes. Reuse its stable CAS kind so the later
            // compare stage remains idempotent instead of colliding on
            // metadata for the same content hash.
            "reference-silhouette-mask-v1",
        )?;
        let model_mask = pass_bytes.get("silhouette").ok_or_else(|| {
            RuntimeError::InvalidInput("RENDER_REJECTED: silhouette pass missing".to_owned())
        })?;
        let metrics = compare_masks_with_parts(
            &reference_mask.mask,
            &decode_binary_mask(model_mask)?,
            view_spec,
            pass_bytes
                .get("part-id")
                .map(|bytes| (bytes.as_slice(), inspection.part_ids.as_slice())),
        );
        let annotation_readiness = reference_annotation_readiness(
            self,
            project_id,
            candidate_id,
            reference_id,
            target_sha256.as_deref(),
            view_spec,
            &camera,
        )?;
        let visual_status = if annotation_readiness.benchmark_eligibility == "READY_PARTIAL_VIEW"
            && visible_view_gate_passes(&metrics)
        {
            "PARTIAL_VISIBLE_VIEW_PASS"
        } else {
            "QUALITY_TARGET_NOT_MET"
        };
        let mut comparison = json!({
            "schema_version":"ReferenceComparisonReport@1",
            "report_id":format!("comparison-{}", &render_set_hash[..32]),
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_hash,
            "camera_hash":camera["camera_hash"].clone(),
            "benchmark_eligibility":annotation_readiness.benchmark_eligibility,
            "mask":{"method":reference_mask_method,"revision":reference_mask_revision,"sha256":mask_object.record.sha256,"width":512,"height":512},
            "metrics":metrics,
            "status":visual_status,
            "canonical_sha256":""
        });
        if let Some(view_id) = view_id.as_deref() {
            comparison["view_id"] = Value::String(view_id.to_owned());
        }
        comparison["canonical_sha256"] = Value::String(canonical_json_hash(&comparison));
        validate_reference_comparison_report(&comparison)?;
        let comparison_object = self.put_reference_comparison_object(
            reservation,
            form_art_batch,
            reserved_objects,
            &canonical_json_bytes(&comparison)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "reference-comparison-report",
        )?;
        let comparison_hash = comparison_object.record.sha256.clone();
        let quality_id = format!("quality-c-{}", Uuid::new_v4().simple());
        let mut limitations = vec![
            "human_visual_review_not_run".to_owned(),
            "single_reference_view_only".to_owned(),
            "HQ_360_PASS_BLOCKED_REFERENCE_COVERAGE".to_owned(),
        ];
        limitations.push(format!(
            "benchmark_eligibility:{}",
            annotation_readiness.benchmark_eligibility
        ));
        limitations.extend(
            annotation_readiness
                .reasons
                .iter()
                .map(|reason| format!("reference_annotation:{reason}")),
        );
        let mut quality = json!({
            "schema_version":"QualityReport@2",
            "quality_report_id":quality_id,
            "candidate_id":candidate_id,
            "artifact_sha256":artifact_sha256,
            "program_sha256":inspection.program_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_hash,
            "comparison_report_hash":comparison_hash,
            "human_receipt_hash":Value::Null,
            "structural_status":"passed",
            "visual_status":visual_status,
            "hard_gate_passed":visual_status == "PARTIAL_VISIBLE_VIEW_PASS",
            "threshold_revision":VISIBLE_VIEW_THRESHOLD_REVISION,
            "threshold_policy_sha256":visible_view_threshold_policy_sha256(),
            "threshold_source":VISIBLE_VIEW_THRESHOLD_SOURCE,
            "metric_gate_results":visible_view_gate_checks(&metrics),
            "benchmark_eligibility":annotation_readiness.benchmark_eligibility,
            "limitations":limitations,
            "canonical_sha256":""
        });
        if let Some(view_id) = view_id.as_deref() {
            quality["view_id"] = Value::String(view_id.to_owned());
        }
        quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
        validate_quality_report_v2_output(&quality)?;
        let quality_object = self.put_reference_comparison_object(
            reservation,
            form_art_batch,
            reserved_objects,
            &canonical_json_bytes(&quality)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "quality-report-v2",
        )?;
        let now = now_string();
        if update_visual_evidence_projection {
            self.store.upsert_visual_evidence(&VisualEvidenceRecord {
                candidate_id: candidate_id.to_owned(),
                project_id: project_id.to_owned(),
                reference_id: reference_id.to_owned(),
                target_sha256: target_sha256.clone(),
                render_set_object_sha256: render_set_object.record.sha256.clone(),
                comparison_report_object_sha256: Some(comparison_object.record.sha256.clone()),
                visual_review_object_sha256: None,
                quality_report_object_sha256: quality_object.record.sha256.clone(),
                human_receipt_object_sha256: None,
                created_at: now.clone(),
                updated_at: now,
            })?;
            if let Some(view_id) = view_id {
                let now = now_string();
                self.store.upsert_visual_evidence_view(
                    &forgecad_store::VisualEvidenceViewRecord {
                        candidate_id: candidate_id.to_owned(),
                        project_id: project_id.to_owned(),
                        view_id,
                        reference_id: reference_id.to_owned(),
                        reference_sha256: reference.object_sha256.clone(),
                        camera_hash: camera["camera_hash"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        render_set_object_sha256: render_set_object.record.sha256.clone(),
                        comparison_report_object_sha256: Some(
                            comparison_object.record.sha256.clone(),
                        ),
                        quality_report_object_sha256: quality_object.record.sha256.clone(),
                        quality_status: visual_status.to_owned(),
                        created_at: now.clone(),
                        updated_at: now,
                    },
                )?;
            }
        }
        Ok(json!({
            "schema_version":"ReferenceComparisonPrepareResult@1",
            "candidate_id":candidate_id,
            "reference_id":reference_id,
            "camera":camera,
            "camera_object_sha256":camera_object.record.sha256,
            "render_set":render_set,
            "render_set_hash":render_set_object.record.sha256,
            "render_set_object_sha256":render_set_object.record.sha256,
            "comparison_report":comparison,
            "comparison_report_hash":comparison_object.record.sha256,
            "comparison_report_object_sha256":comparison_object.record.sha256,
            "quality_report":quality,
            "quality_report_object_sha256":quality_object.record.sha256
        }))
    }

    fn put_reference_comparison_object(
        &self,
        reservation: Option<&forgecad_store::CasReservation>,
        form_art_batch: Option<&forgecad_store::ProductionWeaponFormArtBaselineCasBatch>,
        reserved_objects: &mut Vec<CasObject>,
        bytes: &[u8],
        expected_sha256: Option<&str>,
        mime: &str,
        kind: &str,
    ) -> Result<CasObject, RuntimeError> {
        let object = match (form_art_batch, reservation) {
            (Some(batch), Some(_)) => self
                .store
                .put_production_weapon_form_art_baseline_cas_object(
                    batch,
                    bytes,
                    expected_sha256,
                    mime,
                    kind,
                    &now_string(),
                )?,
            (None, Some(reservation)) => self.store.put_object_reserved(
                reservation,
                bytes,
                expected_sha256,
                mime,
                kind,
                &now_string(),
            )?,
            (None, None) => self.put_object(bytes, expected_sha256, mime, kind)?,
            (Some(_), None) => {
                return Err(RuntimeError::InvalidInput(
                    "FORM_ART_CAS_BATCH_RESERVATION_MISSING".to_owned(),
                ))
            }
        };
        if reservation.is_some() && object.record.reachability == "temporary" {
            reserved_objects.push(object.clone());
        }
        Ok(object)
    }

    pub fn render_pass_get(
        &self,
        render_set_hash: &str,
        pass_name: &str,
    ) -> Result<Value, RuntimeError> {
        if !forgecad_contracts::is_sha256(render_set_hash) {
            return Err(RuntimeError::InvalidInput(
                "RENDER_PASS_INVALID: render_set_hash is invalid".to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(&self.cas_read(render_set_hash)?)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_PASS_INVALID: {error}")))?;
        validate_render_set_v2_output(&render_set)?;
        let artifact = render_set
            .get("pass_artifacts")
            .and_then(|value| value.get(pass_name))
            .ok_or_else(|| RuntimeError::InvalidInput("RENDER_PASS_NOT_FOUND".to_owned()))?;
        let pass_hash = artifact
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::InvalidInput("RENDER_PASS_INVALID: pass hash is missing".to_owned())
            })?;
        let png = self.cas_read(pass_hash)?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(RuntimeError::InvalidInput(
                "RENDER_PASS_INVALID: CAS bytes are not PNG".to_owned(),
            ));
        }
        Ok(
            json!({"schema_version":"RenderPassGet@1","render_set_hash":render_set_hash,"candidate_id":render_set["candidate_id"],"pass":pass_name,"mime":"image/png","width":512,"height":512,"sha256":pass_hash,"png_base64":base64::engine::general_purpose::STANDARD.encode(png)}),
        )
    }

    /// Return the candidate-bound visual evidence needed by the optional
    /// Viewer. Reports stay in CAS and are re-read/validated here; the
    /// projection contains no image bytes and performs no writes.
    pub fn visual_evidence(&self, candidate_id: &str) -> Result<Value, RuntimeError> {
        validate_id(candidate_id)?;
        let candidate = self.candidate(candidate_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: candidate not found".to_owned(),
            )
        })?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| RuntimeError::InvalidInput("VISUAL_EVIDENCE_UNAVAILABLE".to_owned()))?;
        let reference = self.reference(&evidence.reference_id)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: reference not found".to_owned(),
            )
        })?;
        if reference.project_id != candidate.project_id
            || evidence.project_id != candidate.project_id
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: project differs".to_owned(),
            ));
        }
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!("VISUAL_EVIDENCE_INVALID: RenderSet: {error}"))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if render_set.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
            || render_set.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: RenderSet candidate differs".to_owned(),
            ));
        }
        let candidate_artifact_sha256 = candidate
            .prepared_object_sha256
            .as_deref()
            .or(candidate.manifest_hash.as_deref())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_UNAVAILABLE: candidate artifact is missing".to_owned(),
                )
            })?;
        if !forgecad_contracts::is_sha256(candidate_artifact_sha256)
            || candidate
                .prepared_object_sha256
                .as_deref()
                .is_some_and(|hash| !forgecad_contracts::is_sha256(hash))
            || candidate
                .manifest_hash
                .as_deref()
                .is_some_and(|hash| !forgecad_contracts::is_sha256(hash))
            || candidate
                .prepared_object_sha256
                .as_deref()
                .zip(candidate.manifest_hash.as_deref())
                .is_some_and(|(prepared, manifest)| prepared != manifest)
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(candidate_artifact_sha256)
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: RenderSet artifact differs from candidate"
                    .to_owned(),
            ));
        }
        if let Some(target_sha256) = evidence.target_sha256.as_deref() {
            let target = self.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
                || target.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference.object_sha256.as_str())
            {
                return Err(RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_BINDING_MISMATCH: silhouette target differs from reference"
                        .to_owned(),
                ));
            }
        }
        let comparison_report = if let Some(hash) =
            evidence.comparison_report_object_sha256.as_deref()
        {
            let report: Value = serde_json::from_slice(&self.cas_read(hash)?).map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "VISUAL_EVIDENCE_INVALID: comparison report: {error}"
                ))
            })?;
            validate_reference_comparison_report(&report)?;
            if report.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
                || report.get("reference_id").and_then(Value::as_str)
                    != Some(evidence.reference_id.as_str())
                || report.get("artifact_sha256").and_then(Value::as_str)
                    != Some(candidate_artifact_sha256)
                || report.get("reference_sha256").and_then(Value::as_str)
                    != Some(reference.object_sha256.as_str())
                || report.get("render_set_hash").and_then(Value::as_str)
                    != Some(evidence.render_set_object_sha256.as_str())
                || report.get("camera_hash") != render_set.get("camera_hash")
            {
                return Err(RuntimeError::InvalidInput(
                    "VISUAL_EVIDENCE_BINDING_MISMATCH: comparison report lineage differs"
                        .to_owned(),
                ));
            }
            Some(report)
        } else {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_UNAVAILABLE: comparison report is missing".to_owned(),
            ));
        };
        let quality_report = self.quality(candidate_id, Some(&evidence.reference_id))?;
        validate_quality_report_v2_output(&quality_report)?;
        if quality_report.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
            || quality_report
                .get("artifact_sha256")
                .and_then(Value::as_str)
                != Some(candidate_artifact_sha256)
            || quality_report.get("reference_id").and_then(Value::as_str)
                != Some(evidence.reference_id.as_str())
            || quality_report
                .get("reference_sha256")
                .and_then(Value::as_str)
                != Some(reference.object_sha256.as_str())
            || quality_report
                .get("render_set_hash")
                .and_then(Value::as_str)
                != Some(evidence.render_set_object_sha256.as_str())
            || quality_report
                .get("comparison_report_hash")
                .and_then(Value::as_str)
                != evidence.comparison_report_object_sha256.as_deref()
        {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_EVIDENCE_BINDING_MISMATCH: QualityReport lineage differs".to_owned(),
            ));
        }
        Ok(json!({
            "schema_version":"ViewerVisualEvidence@1",
            "candidate_id":candidate_id,
            "project_id":evidence.project_id,
            "reference_id":evidence.reference_id,
            "target_sha256":evidence.target_sha256,
            "render_set_hash":evidence.render_set_object_sha256,
            "comparison_report_hash":evidence.comparison_report_object_sha256,
            "quality_report_hash":evidence.quality_report_object_sha256,
            "render_set":render_set,
            "comparison_report":comparison_report,
            "quality_report":quality_report
        }))
    }

    pub fn submit_visual_review(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("visual review request must be an object".to_owned())
        })?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_REVIEW_UNAVAILABLE: run reference_compare_prepare first".to_owned(),
                )
            })?;
        if evidence.reference_id != reference_id {
            return Err(RuntimeError::InvalidInput(
                "REFERENCE_BINDING_MISMATCH: review reference differs from candidate evidence"
                    .to_owned(),
            ));
        }
        let render_set_hash = required_value_sha(object.get("render_set_hash"), "render_set_hash")?;
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "VISUAL_REVIEW_UNAVAILABLE: RenderSet is invalid: {error}"
            ))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if evidence.render_set_object_sha256 != render_set_hash {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_REVIEW_BINDING_MISMATCH: RenderSet hash is not candidate-bound".to_owned(),
            ));
        }
        let comparison_hash = required_value_sha(
            object.get("comparison_report_hash"),
            "comparison_report_hash",
        )?;
        let comparison_object_sha = evidence
            .comparison_report_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "VISUAL_REVIEW_UNAVAILABLE: comparison report is missing".to_owned(),
                )
            })?;
        let comparison: Value = serde_json::from_slice(&self.cas_read(comparison_object_sha)?)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "VISUAL_REVIEW_UNAVAILABLE: comparison report is invalid: {error}"
                ))
            })?;
        validate_reference_comparison_report(&comparison)?;
        if evidence.comparison_report_object_sha256.as_deref() != Some(comparison_hash) {
            return Err(RuntimeError::InvalidInput(
                "VISUAL_REVIEW_BINDING_MISMATCH: comparison report is not candidate-bound"
                    .to_owned(),
            ));
        }
        let mut report = json!({"schema_version":"VisualReviewReport@1","review_id":format!("review-{}",Uuid::new_v4().simple()),"candidate_id":candidate_id,"reference_id":reference_id,"render_set_hash":render_set_hash,"comparison_report_hash":comparison_hash,"round":object.get("round").cloned().unwrap_or(Value::Null),"stage":object.get("stage").cloned().unwrap_or(Value::Null),"issues":object.get("issues").cloned().unwrap_or(Value::Array(Vec::new())),"status":object.get("status").cloned().unwrap_or(Value::String("submitted".to_owned())),"canonical_sha256":""});
        report["canonical_sha256"] = Value::String(canonical_json_hash(&report));
        validate_visual_review_report(&report)?;
        let report_object = self.put_object(
            &canonical_json_bytes(&report)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "visual-review-report",
        )?;
        let now = now_string();
        self.store.upsert_visual_evidence(&VisualEvidenceRecord {
            visual_review_object_sha256: Some(report_object.record.sha256.clone()),
            updated_at: now.clone(),
            ..evidence
        })?;
        Ok(
            json!({"schema_version":"VisualReviewSubmitResult@1","review":report,"review_object_sha256":report_object.record.sha256}),
        )
    }

    pub fn submit_human_visual_review(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput("human visual review request must be an object".to_owned())
        })?;
        let candidate_id = required_value_id(object.get("candidate_id"), "candidate_id")?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
        let evidence = self
            .store
            .get_visual_evidence(candidate_id)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_UNAVAILABLE: run reference_compare_prepare first".to_owned(),
                )
            })?;
        if evidence.reference_id != reference_id {
            return Err(RuntimeError::InvalidInput("REFERENCE_BINDING_MISMATCH: human review reference differs from candidate evidence".to_owned()));
        }
        let render_set_hash = required_value_sha(object.get("render_set_hash"), "render_set_hash")?;
        let comparison_hash = required_value_sha(
            object.get("comparison_report_hash"),
            "comparison_report_hash",
        )?;
        let render_set: Value = serde_json::from_slice(
            &self.cas_read(&evidence.render_set_object_sha256)?,
        )
        .map_err(|error| {
            RuntimeError::InvalidInput(format!(
                "HUMAN_REVIEW_UNAVAILABLE: RenderSet is invalid: {error}"
            ))
        })?;
        validate_render_set_v2_output(&render_set)?;
        if evidence.render_set_object_sha256 != render_set_hash {
            return Err(RuntimeError::InvalidInput(
                "HUMAN_REVIEW_BINDING_MISMATCH: RenderSet hash is not candidate-bound".to_owned(),
            ));
        }
        let comparison_object_sha = evidence
            .comparison_report_object_sha256
            .as_deref()
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HUMAN_REVIEW_UNAVAILABLE: comparison report is missing".to_owned(),
                )
            })?;
        let comparison: Value = serde_json::from_slice(&self.cas_read(comparison_object_sha)?)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "HUMAN_REVIEW_UNAVAILABLE: comparison report is invalid: {error}"
                ))
            })?;
        validate_reference_comparison_report(&comparison)?;
        if evidence.comparison_report_object_sha256.as_deref() != Some(comparison_hash) {
            return Err(RuntimeError::InvalidInput(
                "HUMAN_REVIEW_BINDING_MISMATCH: comparison report is not candidate-bound"
                    .to_owned(),
            ));
        }
        let scores = object
            .get("scores")
            .cloned()
            .ok_or_else(|| RuntimeError::InvalidInput("HUMAN_REVIEW_SCORES_REQUIRED".to_owned()))?;
        let mut receipt = json!({"schema_version":"HumanVisualReviewReceipt@1","receipt_id":format!("human-review-{}",Uuid::new_v4().simple()),"candidate_id":candidate_id,"reference_id":reference_id,"render_set_hash":render_set_hash,"comparison_report_hash":comparison_hash,"scores":scores,"approved":object.get("approved").cloned().unwrap_or(Value::Bool(false)),"recorded_at":now_string(),"canonical_sha256":""});
        receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
        validate_human_review_receipt(&receipt)?;
        let receipt_object = self.put_object(
            &canonical_json_bytes(&receipt)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "human-visual-review-receipt",
        )?;
        let mut quality: Value =
            serde_json::from_slice(&self.cas_read(&evidence.quality_report_object_sha256)?)
                .map_err(|error| {
                    RuntimeError::InvalidInput(format!("QUALITY_REPORT_INVALID: {error}"))
                })?;
        quality["human_receipt_hash"] = Value::String(receipt_object.record.sha256.clone());
        quality["canonical_sha256"] = Value::String(String::new());
        quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
        validate_quality_report_v2_output(&quality)?;
        let quality_object = self.put_object(
            &canonical_json_bytes(&quality)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "quality-report-v2",
        )?;
        let now = now_string();
        self.store.upsert_visual_evidence(&VisualEvidenceRecord {
            human_receipt_object_sha256: Some(receipt_object.record.sha256.clone()),
            quality_report_object_sha256: quality_object.record.sha256.clone(),
            updated_at: now,
            ..evidence
        })?;
        Ok(
            json!({"schema_version":"HumanVisualReviewSubmitResult@1","receipt":receipt,"receipt_object_sha256":receipt_object.record.sha256,"quality_report":quality,"quality_report_object_sha256":quality_object.record.sha256}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn reference_comparison_rejects_non_object_requests_at_the_domain_boundary() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .prepare_reference_comparison("project", Value::Null)
            .expect_err("null comparison request must fail closed");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: reference comparison request must be an object"
        );
    }

    #[test]
    fn render_pass_readback_rejects_invalid_hash_before_cas_access() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .render_pass_get("not-a-sha256", "beauty")
            .expect_err("invalid render-set hash must fail closed");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: RENDER_PASS_INVALID: render_set_hash is invalid"
        );
    }
}
