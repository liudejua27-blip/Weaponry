//! Runtime-owned Evaluation reference comparison and visual review family.
//!
//! This module owns the complete typed implementation for reference comparison,
//! render-pass readback, visual-evidence projection, Codex review, and human
//! visual review.  All durable writes continue through the parent Runtime's
//! Store/CAS boundary; this is a physical extraction, not a second writer.

use base64::Engine;
use forgecad_store::{
    AuthoringMeshV2HighArtifactStoreRecord, AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{
    build_cohort_sha256, calibrate_default_camera, calibrate_default_camera_height_only,
    camera_fit_cache_key, camera_fit_score, canonical_json_bytes, canonical_json_hash,
    compare_masks_with_parts, decode_binary_mask, default_camera_calibration, mask_to_png,
    now_string, project_reference_mask_to_view, reference_annotation_readiness, reference_mask_png,
    render_glb_with_runtime_worker_identity, render_worker_binding_status, required_value_id,
    required_value_sha, sha256_hex, strict_glb_inspection, validate_camera_calibration,
    validate_human_review_receipt, validate_id, validate_quality_report_v2_output,
    validate_reference_comparison_report, validate_reference_view_spec,
    validate_render_set_v2_output, validate_request_keys, validate_visual_review_report,
    visible_view_gate_checks, visible_view_gate_passes, visible_view_threshold_policy_sha256,
    CasObject, Runtime, RuntimeError, VisualEvidenceRecord, VISIBLE_VIEW_THRESHOLD_REVISION,
    VISIBLE_VIEW_THRESHOLD_SOURCE,
};

fn resolve_reference_view_id(
    request: &Map<String, Value>,
    view_spec: &Value,
) -> Result<String, RuntimeError> {
    let view_spec_id = required_value_id(view_spec.get("view_id"), "view_spec.view_id")?;
    let requested_view_id = request
        .get("view_id")
        .map(|value| required_value_id(Some(value), "view_id"))
        .transpose()?;
    if requested_view_id.is_some_and(|requested| requested != view_spec_id) {
        return Err(RuntimeError::InvalidInput(
            "REFERENCE_VIEW_BINDING_MISMATCH: top-level view_id differs from view_spec.view_id"
                .to_owned(),
        ));
    }
    Ok(view_spec_id.to_owned())
}

const HIGH_ARTIFACT_FIXED_CAMERA_ORTHO_SCALE_M: f64 = 6.0;

/// Resolve the five closed Dragonfang orthographic review conventions.
///
/// Weapon authoring uses X/Y as the broadside plane and Z as thickness.  The
/// generic camera-rig names describe the camera position rather than the
/// reference-sheet label, so the explicit mapping is:
/// front=-Z, top=+Y, bottom=-Y, left=-X, right=+X.  This is deliberately a
/// closed lookup; callers still cannot inject an arbitrary camera or silently
/// change handedness between correction rounds.
fn high_artifact_fixed_camera(
    view_spec: &Value,
    requested_camera: Option<&Value>,
) -> Result<Value, RuntimeError> {
    let source_view = view_spec
        .get("source_view")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_CAMERA_CONVENTION_UNSUPPORTED: source_view is missing".to_owned(),
            )
        })?;
    let camera_kind = match source_view {
        "front" => "left",
        "top" => "top",
        "bottom" => "bottom",
        "left" => "back",
        "right" => "front",
        _ => return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_CAMERA_CONVENTION_UNSUPPORTED: source_view must be front, top, bottom, left, or right"
                .to_owned(),
        )),
    };
    let fixed = crate::multiview::camera_rig::inferred_weapon_camera(
        camera_kind,
        HIGH_ARTIFACT_FIXED_CAMERA_ORTHO_SCALE_M,
    )
    .map_err(|error| {
        RuntimeError::InvalidInput(format!(
            "HIGH_ARTIFACT_CAMERA_CONVENTION_UNSUPPORTED: {error}"
        ))
    })?;
    let fixed_hash = fixed
        .get("camera_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_CAMERA_CONVENTION_UNSUPPORTED: fixed camera hash is unavailable"
                    .to_owned(),
            )
        })?;
    match requested_camera {
        Some(value)
            if value.get("schema_version").and_then(Value::as_str)
                == Some("CameraCalibrationRef@1") => Err(RuntimeError::InvalidInput(
            "CAMERA_CALIBRATION_INVALID: High artifact comparison requires an explicit camera calibration, not a candidate fit reference"
                .to_owned(),
        )),
        Some(value) => {
            validate_camera_calibration(value)?;
            if value.get("camera_hash").and_then(Value::as_str) != Some(fixed_hash) {
                return Err(RuntimeError::InvalidInput(
                    "HIGH_ARTIFACT_CAMERA_CONVENTION_MISMATCH: explicit camera is not the fixed camera for source_view"
                        .to_owned(),
                ));
            }
            Ok(value.clone())
        }
        None => Ok(fixed),
    }
}

/// Expose the existing closed Dragonfang camera convention to adjacent
/// Runtime-owned delivery projections.  The delivery lane intentionally
/// supplies only a source-view label; arbitrary camera payloads remain
/// rejected by `high_artifact_fixed_camera` above.
pub(crate) fn high_artifact_fixed_camera_for_source_view(
    source_view: &str,
) -> Result<Value, RuntimeError> {
    high_artifact_fixed_camera(&json!({"source_view": source_view}), None)
}

/// Project the authorized front-panel crop into the square High review frame
/// without changing its pixel aspect ratio. `reference_mask_png` first fits
/// the complete source image into a 512px square canvas, while
/// `ReferenceViewSpec.image.crop` is expressed in original-image normalized
/// coordinates. The legacy projector treated those as the same coordinate
/// space and then stretched the rectangular crop to a square. Keep that
/// historical behavior outside this direct High path, but fail closed here
/// unless the fixed front convention is unrotated and fully bound.
fn high_artifact_reference_mask_to_fixed_view(
    source: &[bool],
    view_spec: &Value,
    reference_width: u32,
    reference_height: u32,
    source_is_aspect_fit_canvas: bool,
    require_non_empty: bool,
) -> Result<Vec<bool>, RuntimeError> {
    if source.len() != 512 * 512 || reference_width == 0 || reference_height == 0 {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: source mask or dimensions are invalid"
                .to_owned(),
        ));
    }
    let image = view_spec
        .get("image")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: view image is missing".to_owned(),
            )
        })?;
    if image.get("width").and_then(Value::as_u64) != Some(reference_width as u64)
        || image.get("height").and_then(Value::as_u64) != Some(reference_height as u64)
        || image
            .get("rotation_degrees")
            .and_then(Value::as_f64)
            .is_none_or(|rotation| rotation != 0.0)
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: fixed front dimensions or rotation drifted"
                .to_owned(),
        ));
    }
    let crop = image
        .get("crop")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: crop is missing".to_owned(),
            )
        })?;
    let number = |key: &str| -> Result<f64, RuntimeError> {
        crop.get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!(
                    "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: crop.{key} is invalid"
                ))
            })
    };
    let crop_x = number("x")?;
    let crop_y = number("y")?;
    let crop_width = number("width")?;
    let crop_height = number("height")?;
    if crop_x < 0.0
        || crop_y < 0.0
        || crop_width <= 0.0
        || crop_height <= 0.0
        || crop_x + crop_width > 1.0
        || crop_y + crop_height > 1.0
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: crop is outside the source image".to_owned(),
        ));
    }

    // Match aspect_fit_reference_rgba's integer rounding exactly before
    // translating original normalized crop coordinates into its canvas.
    let (fit_width, fit_height) = if reference_width >= reference_height {
        (
            512_u32,
            ((reference_height as u64 * 512 + reference_width as u64 / 2) / reference_width as u64)
                .clamp(1, 512) as u32,
        )
    } else {
        (
            ((reference_width as u64 * 512 + reference_height as u64 / 2) / reference_height as u64)
                .clamp(1, 512) as u32,
            512_u32,
        )
    };
    let (source_x, source_y, source_width, source_height) = if source_is_aspect_fit_canvas {
        let offset_x = (512 - fit_width) as f64 * 0.5;
        let offset_y = (512 - fit_height) as f64 * 0.5;
        (
            offset_x + crop_x * fit_width as f64,
            offset_y + crop_y * fit_height as f64,
            crop_width * fit_width as f64,
            crop_height * fit_height as f64,
        )
    } else {
        (
            crop_x * 512.0,
            crop_y * 512.0,
            crop_width * 512.0,
            crop_height * 512.0,
        )
    };
    let crop_pixel_aspect =
        crop_width * reference_width as f64 / (crop_height * reference_height as f64);
    if !crop_pixel_aspect.is_finite() || crop_pixel_aspect <= 0.0 {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_REFERENCE_FRAME_INVALID: crop pixel aspect is invalid".to_owned(),
        ));
    }
    let (content_width, content_height) = if crop_pixel_aspect >= 1.0 {
        (512.0, 512.0 / crop_pixel_aspect)
    } else {
        (512.0 * crop_pixel_aspect, 512.0)
    };
    let content_x = (512.0 - content_width) * 0.5;
    let content_y = (512.0 - content_height) * 0.5;
    let mut projected = vec![false; 512 * 512];
    for y in 0..512usize {
        let center_y = y as f64 + 0.5;
        if center_y < content_y || center_y >= content_y + content_height {
            continue;
        }
        for x in 0..512usize {
            let center_x = x as f64 + 0.5;
            if center_x < content_x || center_x >= content_x + content_width {
                continue;
            }
            let u = (center_x - content_x) / content_width;
            let v = (center_y - content_y) / content_height;
            let sample_x = (source_x + u * source_width).floor().clamp(0.0, 511.0) as usize;
            let sample_y = (source_y + v * source_height).floor().clamp(0.0, 511.0) as usize;
            projected[y * 512 + x] = source[sample_y * 512 + sample_x];
        }
    }
    if require_non_empty && !projected.iter().any(|value| *value) {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_REFERENCE_FRAME_EMPTY: fixed front crop contains no silhouette"
                .to_owned(),
        ));
    }
    Ok(projected)
}

const HIGH_ARTIFACT_RENDER_SET_KIND: &str = "high-artifact-render-set-v1";
const HIGH_ARTIFACT_RENDER_SET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "high_artifact_id",
    "high_artifact_sha256",
    "high_artifact_object_sha256",
    "high_artifact_readback_sha256",
    "high_artifact_readback_object_sha256",
    "high_artifact_receipt_sha256",
    "high_artifact_receipt_object_sha256",
    "high_bridge_id",
    "high_bridge_sha256",
    "high_bridge_object_sha256",
    "revision_id",
    "revision_sha256",
    "revision_object_sha256",
    "high_result_sha256",
    "high_result_object_sha256",
    "high_readback_sha256",
    "high_readback_object_sha256",
    "high_worker_algorithm_sha256",
    "high_worker_build_cohort_sha256",
    "reference_id",
    "view_id",
    "camera_hash",
    "camera_object_sha256",
    "renderer_hash",
    "render_profile",
    "render_profile_sha256",
    "aov_definition_sha256",
    "color_pipeline_sha256",
    "id_palette_definition_sha256",
    "render_worker_build_cohort_sha256",
    "render_worker_binding_status",
    "width",
    "height",
    "passes",
    "pass_artifacts",
    "canonical_sha256",
];

const FIXED_RENDER_PASSES: [&str; 9] = [
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

/// Validate and resolve the source of a direct High render set.  This is a
/// separate selector from the legacy Candidate RenderSet path: a High GLB is
/// usable only when its complete semantic/object/readback/receipt/bridge/
/// revision/worker-cohort identity is still present in Store/CAS.
fn select_high_artifact_render_source(
    runtime: &Runtime,
    render_set_hash: &str,
    render_set: &Value,
) -> Result<AuthoringMeshV2HighArtifactStoreRecord, RuntimeError> {
    let object = render_set.as_object().ok_or_else(|| {
        RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: render set must be an object".to_owned(),
        )
    })?;
    validate_request_keys(
        object,
        HIGH_ARTIFACT_RENDER_SET_FIELDS,
        "HighArtifactRenderSet@1",
    )?;
    if object.len() != HIGH_ARTIFACT_RENDER_SET_FIELDS.len()
        || object.get("schema_version").and_then(Value::as_str) != Some("HighArtifactRenderSet@1")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: fixed render constants drifted".to_owned(),
        ));
    }
    let project_id = required_value_id(object.get("project_id"), "project_id")?;
    let artifact_id = required_value_id(object.get("high_artifact_id"), "high_artifact_id")?;
    let artifact_sha256 =
        required_value_sha(object.get("high_artifact_sha256"), "high_artifact_sha256")?;
    let artifact_object_sha256 = required_value_sha(
        object.get("high_artifact_object_sha256"),
        "high_artifact_object_sha256",
    )?;
    let artifact_readback_sha256 = required_value_sha(
        object.get("high_artifact_readback_sha256"),
        "high_artifact_readback_sha256",
    )?;
    let artifact_readback_object_sha256 = required_value_sha(
        object.get("high_artifact_readback_object_sha256"),
        "high_artifact_readback_object_sha256",
    )?;
    let receipt_sha256 = required_value_sha(
        object.get("high_artifact_receipt_sha256"),
        "high_artifact_receipt_sha256",
    )?;
    let receipt_object_sha256 = required_value_sha(
        object.get("high_artifact_receipt_object_sha256"),
        "high_artifact_receipt_object_sha256",
    )?;
    let bridge_id = required_value_id(object.get("high_bridge_id"), "high_bridge_id")?;
    let bridge_sha256 = required_value_sha(object.get("high_bridge_sha256"), "high_bridge_sha256")?;
    let bridge_object_sha256 = required_value_sha(
        object.get("high_bridge_object_sha256"),
        "high_bridge_object_sha256",
    )?;
    let revision_id = required_value_id(object.get("revision_id"), "revision_id")?;
    let revision_sha256 = required_value_sha(object.get("revision_sha256"), "revision_sha256")?;
    let revision_object_sha256 = required_value_sha(
        object.get("revision_object_sha256"),
        "revision_object_sha256",
    )?;
    let high_result_sha256 =
        required_value_sha(object.get("high_result_sha256"), "high_result_sha256")?;
    let high_result_object_sha256 = required_value_sha(
        object.get("high_result_object_sha256"),
        "high_result_object_sha256",
    )?;
    let high_readback_sha256 =
        required_value_sha(object.get("high_readback_sha256"), "high_readback_sha256")?;
    let high_readback_object_sha256 = required_value_sha(
        object.get("high_readback_object_sha256"),
        "high_readback_object_sha256",
    )?;
    let high_worker_algorithm_sha256 = required_value_sha(
        object.get("high_worker_algorithm_sha256"),
        "high_worker_algorithm_sha256",
    )?;
    let high_worker_build_cohort_sha256 = required_value_sha(
        object.get("high_worker_build_cohort_sha256"),
        "high_worker_build_cohort_sha256",
    )?;
    let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
    let view_id = required_value_id(object.get("view_id"), "view_id")?;

    for key in [
        "camera_hash",
        "camera_object_sha256",
        "renderer_hash",
        "render_profile_sha256",
        "aov_definition_sha256",
        "color_pipeline_sha256",
        "id_palette_definition_sha256",
        "canonical_sha256",
    ] {
        required_value_sha(object.get(key), key)?;
    }
    let expected_profile = forgecad_worker_protocol::render_profile();
    let expected_renderer_hash = sha256_hex(b"forgecad-renderer-2");
    if object.get("render_profile") != Some(&expected_profile)
        || object.get("render_profile_sha256") != expected_profile.get("canonical_sha256")
        || object.get("aov_definition_sha256") != expected_profile.get("aov_definition_sha256")
        || object.get("color_pipeline_sha256") != expected_profile.get("color_pipeline_sha256")
        || object.get("id_palette_definition_sha256")
            != expected_profile.get("id_palette_definition_sha256")
        || object.get("renderer_hash").and_then(Value::as_str)
            != Some(expected_renderer_hash.as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: renderer profile lineage drifted".to_owned(),
        ));
    }
    let render_worker_status = object
        .get("render_worker_binding_status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: render worker status is missing".to_owned(),
            )
        })?;
    match render_worker_status {
        "same_cohort_verified" => {
            let render_cohort = required_value_sha(
                object.get("render_worker_build_cohort_sha256"),
                "render_worker_build_cohort_sha256",
            )?;
            if build_cohort_sha256().as_deref() != Some(render_cohort) {
                return Err(RuntimeError::InvalidInput(
                    "HIGH_ARTIFACT_RENDER_SET_INVALID: render worker cohort differs from Runtime"
                        .to_owned(),
                ));
            }
        }
        "cohort_unavailable"
            if object
                .get("render_worker_build_cohort_sha256")
                .is_some_and(Value::is_null) => {}
        _ => {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: render worker cohort/status pair is invalid"
                    .to_owned(),
            ));
        }
    }
    let passes = object
        .get("passes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass order is missing".to_owned(),
            )
        })?;
    if passes.len() != FIXED_RENDER_PASSES.len()
        || passes.iter().map(Value::as_str).collect::<Option<Vec<_>>>()
            != Some(FIXED_RENDER_PASSES.to_vec())
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: pass order is not fixed".to_owned(),
        ));
    }
    let pass_artifacts = object
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass artifacts are missing".to_owned(),
            )
        })?;
    if pass_artifacts.len() != FIXED_RENDER_PASSES.len()
        || FIXED_RENDER_PASSES
            .iter()
            .any(|pass| !pass_artifacts.contains_key(*pass))
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: pass artifacts are incomplete".to_owned(),
        ));
    }
    for pass in FIXED_RENDER_PASSES {
        let artifact = pass_artifacts
            .get(pass)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HIGH_ARTIFACT_RENDER_SET_INVALID: pass artifact is not an object".to_owned(),
                )
            })?;
        validate_request_keys(
            artifact,
            &[
                "sha256",
                "mime",
                "size_bytes",
                "width",
                "height",
                "channels",
                "color_space",
            ],
            "HighArtifactRenderSet@1.pass_artifact",
        )?;
        required_value_sha(artifact.get("sha256"), "pass_artifact.sha256")?;
        if artifact.len() != 7
            || artifact.get("mime").and_then(Value::as_str) != Some("image/png")
            || artifact
                .get("size_bytes")
                .and_then(Value::as_u64)
                .is_none_or(|size| size == 0)
            || artifact.get("width").and_then(Value::as_u64) != Some(512)
            || artifact.get("height").and_then(Value::as_u64) != Some(512)
            || artifact.get("channels").and_then(Value::as_str) != Some("rgba8")
            || artifact.get("color_space").and_then(Value::as_str)
                != Some(if pass == "beauty" { "srgb" } else { "data" })
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass PNG metadata is invalid".to_owned(),
            ));
        }
    }
    let mut canonical = render_set.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical)
        != object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: canonical hash does not bind payload".to_owned(),
        ));
    }
    let render_set_bytes = canonical_json_bytes(render_set)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let render_object = runtime.store.get_object(render_set_hash)?.ok_or_else(|| {
        RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: render set CAS object is missing".to_owned(),
        )
    })?;
    if render_object.sha256 != render_set_hash
        || render_object.mime != "application/json"
        || render_object.kind != HIGH_ARTIFACT_RENDER_SET_KIND
        || render_object.size_bytes != render_set_bytes.len() as u64
        || sha256_hex(&render_set_bytes) != render_set_hash
    {
        return Err(RuntimeError::InvalidInput(
            "HIGH_ARTIFACT_RENDER_SET_INVALID: render set CAS metadata differs".to_owned(),
        ));
    }
    for pass in FIXED_RENDER_PASSES {
        let artifact = pass_artifacts
            .get(pass)
            .and_then(Value::as_object)
            .expect("pass artifact checked above");
        let pass_hash = required_value_sha(artifact.get("sha256"), "pass_artifact.sha256")?;
        let pass_object = runtime.store.get_object(pass_hash)?.ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass PNG CAS object is missing".to_owned(),
            )
        })?;
        if pass_object.sha256 != pass_hash
            || pass_object.mime != "image/png"
            || pass_object.kind != format!("render-pass-{pass}")
            || pass_object.size_bytes
                != artifact
                    .get("size_bytes")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass PNG CAS metadata differs".to_owned(),
            ));
        }
        let pass_bytes = runtime.cas_read(pass_hash)?;
        if pass_bytes.len() as u64 != pass_object.size_bytes
            || sha256_hex(&pass_bytes) != pass_hash
            || !pass_bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_RENDER_SET_INVALID: pass PNG bytes differ".to_owned(),
            ));
        }
    }
    runtime
        .store
        .get_authoring_mesh_v2_high_artifact_exact(
            project_id,
            artifact_id,
            artifact_sha256,
            artifact_object_sha256,
            artifact_readback_sha256,
            artifact_readback_object_sha256,
            receipt_sha256,
            receipt_object_sha256,
            bridge_id,
            bridge_sha256,
            bridge_object_sha256,
            revision_id,
            revision_sha256,
            revision_object_sha256,
            high_result_sha256,
            high_result_object_sha256,
            high_readback_sha256,
            high_readback_object_sha256,
            high_worker_algorithm_sha256,
            high_worker_build_cohort_sha256,
        )?
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_SELECTOR_NOT_FOUND: exact High artifact lineage is unavailable"
                    .to_owned(),
            )
        })
}

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

    /// Render a direct V2 High artifact without pretending that the High row
    /// is a Candidate.  The ordinary reference comparison operation is
    /// intentionally candidate-bound because its RenderSet is consumed by
    /// the candidate visual-evidence projection.  High artifacts have a
    /// different lineage and therefore use this closed, direct comparison
    /// seam.  Every durable High identity is supplied by the caller and is
    /// checked against the Store row before the GLB reaches the renderer.
    pub fn prepare_high_artifact_reference_comparison(
        &self,
        project_id: &str,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        validate_id(project_id)?;
        let object = request.as_object().ok_or_else(|| {
            RuntimeError::InvalidInput(
                "High artifact reference comparison request must be an object".to_owned(),
            )
        })?;
        validate_request_keys(
            object,
            &[
                "project_id",
                "high_artifact_id",
                "high_artifact_sha256",
                "high_artifact_object_sha256",
                "high_artifact_readback_sha256",
                "high_artifact_readback_object_sha256",
                "high_artifact_receipt_sha256",
                "high_artifact_receipt_object_sha256",
                "high_bridge_id",
                "high_bridge_sha256",
                "high_bridge_object_sha256",
                "revision_id",
                "revision_sha256",
                "revision_object_sha256",
                "high_result_sha256",
                "high_result_object_sha256",
                "high_readback_sha256",
                "high_readback_object_sha256",
                "high_worker_algorithm_sha256",
                "high_worker_build_cohort_sha256",
                "reference_id",
                "view_spec",
                "camera",
                "target_sha256",
                "view_id",
            ],
            "high_artifact_reference_compare_prepare",
        )?;
        let request_project_id = required_value_id(object.get("project_id"), "project_id")?;
        if request_project_id != project_id {
            return Err(RuntimeError::InvalidInput(
                "PROJECT_SCOPE_DENIED: request project differs from route project".to_owned(),
            ));
        }
        let high_artifact_id =
            required_value_id(object.get("high_artifact_id"), "high_artifact_id")?;
        let high_artifact_sha256 =
            required_value_sha(object.get("high_artifact_sha256"), "high_artifact_sha256")?;
        let high_artifact_object_sha256 = required_value_sha(
            object.get("high_artifact_object_sha256"),
            "high_artifact_object_sha256",
        )?;
        let high_artifact_readback_sha256 = required_value_sha(
            object.get("high_artifact_readback_sha256"),
            "high_artifact_readback_sha256",
        )?;
        let high_artifact_readback_object_sha256 = required_value_sha(
            object.get("high_artifact_readback_object_sha256"),
            "high_artifact_readback_object_sha256",
        )?;
        let high_artifact_receipt_sha256 = required_value_sha(
            object.get("high_artifact_receipt_sha256"),
            "high_artifact_receipt_sha256",
        )?;
        let high_artifact_receipt_object_sha256 = required_value_sha(
            object.get("high_artifact_receipt_object_sha256"),
            "high_artifact_receipt_object_sha256",
        )?;
        let high_bridge_id = required_value_id(object.get("high_bridge_id"), "high_bridge_id")?;
        let high_bridge_sha256 =
            required_value_sha(object.get("high_bridge_sha256"), "high_bridge_sha256")?;
        let high_bridge_object_sha256 = required_value_sha(
            object.get("high_bridge_object_sha256"),
            "high_bridge_object_sha256",
        )?;
        let revision_id = required_value_id(object.get("revision_id"), "revision_id")?;
        let revision_sha256 = required_value_sha(object.get("revision_sha256"), "revision_sha256")?;
        let revision_object_sha256 = required_value_sha(
            object.get("revision_object_sha256"),
            "revision_object_sha256",
        )?;
        let high_result_sha256 =
            required_value_sha(object.get("high_result_sha256"), "high_result_sha256")?;
        let high_result_object_sha256 = required_value_sha(
            object.get("high_result_object_sha256"),
            "high_result_object_sha256",
        )?;
        let high_readback_sha256 =
            required_value_sha(object.get("high_readback_sha256"), "high_readback_sha256")?;
        let high_readback_object_sha256 = required_value_sha(
            object.get("high_readback_object_sha256"),
            "high_readback_object_sha256",
        )?;
        let high_worker_algorithm_sha256 = required_value_sha(
            object.get("high_worker_algorithm_sha256"),
            "high_worker_algorithm_sha256",
        )?;
        let high_worker_build_cohort_sha256 = required_value_sha(
            object.get("high_worker_build_cohort_sha256"),
            "high_worker_build_cohort_sha256",
        )?;
        let record = self
            .store
            .get_authoring_mesh_v2_high_artifact_exact(
                project_id,
                high_artifact_id,
                high_artifact_sha256,
                high_artifact_object_sha256,
                high_artifact_readback_sha256,
                high_artifact_readback_object_sha256,
                high_artifact_receipt_sha256,
                high_artifact_receipt_object_sha256,
                high_bridge_id,
                high_bridge_sha256,
                high_bridge_object_sha256,
                revision_id,
                revision_sha256,
                revision_object_sha256,
                high_result_sha256,
                high_result_object_sha256,
                high_readback_sha256,
                high_readback_object_sha256,
                high_worker_algorithm_sha256,
                high_worker_build_cohort_sha256,
            )?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "NOT_FOUND: exact High artifact identity is not present in Store".to_owned(),
                )
            })?;
        let glb_object = self
            .store
            .get_object(&record.high_artifact_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HIGH_ARTIFACT_CAS_MISSING: GLB object is not registered".to_owned(),
                )
            })?;
        if glb_object.sha256 != record.high_artifact_object_sha256
            || glb_object.mime != AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_MIME
            || glb_object.kind != AUTHORING_MESH_V2_HIGH_ARTIFACT_GLB_OBJECT_KIND
            || glb_object.size_bytes != record.high_artifact_size_bytes
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_CAS_METADATA_MISMATCH: GLB metadata differs from Store identity"
                    .to_owned(),
            ));
        }
        let glb = self.cas_read(&record.high_artifact_object_sha256)?;
        if glb.len() as u64 != record.high_artifact_size_bytes
            || sha256_hex(&glb) != record.high_artifact_sha256
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_CAS_HASH_MISMATCH: GLB bytes differ from Store identity".to_owned(),
            ));
        }
        let readback_object = self
            .store
            .get_object(&record.high_artifact_readback_object_sha256)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "HIGH_ARTIFACT_READBACK_CAS_MISSING: readback object is not registered"
                        .to_owned(),
                )
            })?;
        if readback_object.kind != AUTHORING_MESH_V2_HIGH_ARTIFACT_READBACK_OBJECT_KIND
            || readback_object.mime != "application/json"
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_READBACK_CAS_METADATA_MISMATCH".to_owned(),
            ));
        }
        let readback: Value =
            serde_json::from_slice(&self.cas_read(&record.high_artifact_readback_object_sha256)?)
                .map_err(|error| {
                RuntimeError::InvalidInput(format!("HIGH_ARTIFACT_READBACK_INVALID: {error}"))
            })?;
        if readback.get("high_artifact_sha256").and_then(Value::as_str)
            != Some(record.high_artifact_sha256.as_str())
        {
            return Err(RuntimeError::InvalidInput(
                "HIGH_ARTIFACT_READBACK_BINDING_MISMATCH".to_owned(),
            ));
        }
        // Direct V2 High artifacts use the V2 GLB policy: one evaluated base
        // primitive per materialized Part, zero legacy detail primitives, and
        // deterministic NORMAL/TEXCOORD_0 streams for the perspective
        // renderer.  Do not route this through the legacy NativeHigh
        // integrity parser, whose detail-layer rule would reject a valid V2
        // artifact before rendering.
        let inspection = crate::native_high_glb_readback::inspect_authoring_mesh_v2_high_glb(&glb)
            .map_err(|error| {
                RuntimeError::InvalidInput(format!(
                    "RENDER_REJECTED: strict V2 High GLB readback failed: {error}"
                ))
            })?;
        let inspection_part_ids = inspection
            .get("part_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(
                    "RENDER_REJECTED: strict V2 High GLB readback omitted Part inventory"
                        .to_owned(),
                )
            })?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    RuntimeError::InvalidInput(
                        "RENDER_REJECTED: strict V2 High GLB Part inventory is invalid".to_owned(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reference_id = required_value_id(object.get("reference_id"), "reference_id")?;
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
        let view_id = resolve_reference_view_id(object, view_spec)?;
        let target_sha256 = object
            .get("target_sha256")
            .map(|value| required_value_sha(Some(value), "target_sha256"))
            .transpose()?;
        if let Some(target_sha256) = target_sha256 {
            let target = self.read_silhouette_target(target_sha256)?;
            if target.get("reference_id").and_then(Value::as_str) != Some(reference_id) {
                return Err(RuntimeError::InvalidInput(
                    "REFERENCE_SCOPE_DENIED: silhouette target is bound to another reference"
                        .to_owned(),
                ));
            }
        }
        let camera = high_artifact_fixed_camera(
            view_spec,
            object.get("camera").filter(|value| !value.is_null()),
        )?;
        validate_camera_calibration(&camera)?;
        let initial_render = render_glb_with_runtime_worker_identity(&glb, &camera)
            .map_err(|error| RuntimeError::InvalidInput(format!("RENDER_REJECTED: {error}")))?;
        let render_worker_cohort = initial_render.build_cohort_sha256.clone();
        let render_profile = initial_render.render_profile.clone();
        let render_passes = initial_render.passes;
        if render_passes.len() != 9
            || render_passes
                .iter()
                .any(|pass| pass.width != 512 || pass.height != 512)
        {
            return Err(RuntimeError::InvalidInput(
                "RENDER_REJECTED: fixed renderer did not return nine 512x512 passes".to_owned(),
            ));
        }
        let camera_object = self.put_reference_comparison_object(
            None,
            None,
            &mut Vec::new(),
            &canonical_json_bytes(&camera)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "camera-calibration",
        )?;
        let mut pass_artifacts = Map::new();
        let mut pass_bytes = std::collections::HashMap::new();
        for pass in &render_passes {
            let stored = self.put_reference_comparison_object(
                None,
                None,
                &mut Vec::new(),
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
        let mut reference_mask = if let Some(target_sha256) = target_sha256 {
            let target = self.read_silhouette_target(target_sha256)?;
            self.target_mask(target_sha256, &target)?
        } else {
            reference_mask_png(&self.cas_read(&reference.object_sha256)?)?
        };
        reference_mask.mask = high_artifact_reference_mask_to_fixed_view(
            &reference_mask.mask,
            view_spec,
            reference.width,
            reference.height,
            target_sha256.is_none(),
            target_sha256.is_some(),
        )?;
        reference_mask.png = mask_to_png(&reference_mask.mask)?;
        let model_mask = pass_bytes.get("silhouette").ok_or_else(|| {
            RuntimeError::InvalidInput("RENDER_REJECTED: silhouette pass missing".to_owned())
        })?;
        let metrics = compare_masks_with_parts(
            &reference_mask.mask,
            &decode_binary_mask(model_mask)?,
            view_spec,
            pass_bytes
                .get("part-id")
                .map(|bytes| (bytes.as_slice(), inspection_part_ids.as_slice())),
        );
        let mask_object = self.put_reference_comparison_object(
            None,
            None,
            &mut Vec::new(),
            &reference_mask.png,
            None,
            "image/png",
            "reference-silhouette-mask-v1",
        )?;
        let mut render_set = json!({
            "schema_version":"HighArtifactRenderSet@1",
            "project_id":project_id,
            "high_artifact_id":record.artifact_id,
            "high_artifact_sha256":record.high_artifact_sha256,
            "high_artifact_object_sha256":record.high_artifact_object_sha256,
            "high_artifact_readback_sha256":record.high_artifact_readback_sha256,
            "high_artifact_readback_object_sha256":record.high_artifact_readback_object_sha256,
            "high_artifact_receipt_sha256":record.receipt_sha256,
            "high_artifact_receipt_object_sha256":record.receipt_object_sha256,
            "high_bridge_id":record.bridge_id,
            "high_bridge_sha256":record.bridge_sha256,
            "high_bridge_object_sha256":record.bridge_object_sha256,
            "revision_id":record.revision_id,
            "revision_sha256":record.revision_sha256,
            "revision_object_sha256":record.revision_object_sha256,
            "high_result_sha256":record.high_result_sha256,
            "high_result_object_sha256":record.high_result_object_sha256,
            "high_readback_sha256":record.high_readback_sha256,
            "high_readback_object_sha256":record.high_readback_object_sha256,
            "high_worker_algorithm_sha256":record.high_worker_algorithm_sha256,
            "high_worker_build_cohort_sha256":record.high_worker_build_cohort_sha256,
            "reference_id":reference_id,
            "view_id":view_id,
            "camera_hash":camera["camera_hash"].clone(),
            "camera_object_sha256":camera_object.record.sha256,
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
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        let render_set_object = self.put_reference_comparison_object(
            None,
            None,
            &mut Vec::new(),
            &canonical_json_bytes(&render_set)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "high-artifact-render-set-v1",
        )?;
        let status = if visible_view_gate_passes(&metrics) {
            "PARTIAL_VISIBLE_VIEW_PASS"
        } else {
            "QUALITY_TARGET_NOT_MET"
        };
        let mut comparison = json!({
            "schema_version":"HighArtifactReferenceComparisonReport@1",
            "report_id":format!("high-comparison-{}", &render_set_object.record.sha256[..32]),
            "project_id":project_id,
            "high_artifact_id":record.artifact_id,
            "high_artifact_sha256":record.high_artifact_sha256,
            "high_artifact_object_sha256":record.high_artifact_object_sha256,
            "high_artifact_readback_sha256":record.high_artifact_readback_sha256,
            "high_artifact_readback_object_sha256":record.high_artifact_readback_object_sha256,
            "high_artifact_receipt_sha256":record.receipt_sha256,
            "high_artifact_receipt_object_sha256":record.receipt_object_sha256,
            "high_bridge_id":record.bridge_id,
            "high_bridge_sha256":record.bridge_sha256,
            "high_bridge_object_sha256":record.bridge_object_sha256,
            "revision_id":record.revision_id,
            "revision_sha256":record.revision_sha256,
            "revision_object_sha256":record.revision_object_sha256,
            "high_result_sha256":record.high_result_sha256,
            "high_result_object_sha256":record.high_result_object_sha256,
            "high_readback_sha256":record.high_readback_sha256,
            "high_readback_object_sha256":record.high_readback_object_sha256,
            "high_worker_algorithm_sha256":record.high_worker_algorithm_sha256,
            "high_worker_build_cohort_sha256":record.high_worker_build_cohort_sha256,
            "reference_id":reference_id,
            "reference_sha256":reference.object_sha256,
            "render_set_hash":render_set_object.record.sha256,
            "camera_hash":camera["camera_hash"].clone(),
            "benchmark_eligibility":"DIRECT_HIGH_ARTIFACT_COMPARE",
            "mask":{"method":"direct-reference-mask","revision":"mask-2","sha256":mask_object.record.sha256,"width":512,"height":512},
            "metrics":metrics,
            "status":status,
            "limitations":["candidate_visual_evidence_projection_not_updated","human_visual_review_not_run","commercial_quality_not_proven"],
            "canonical_sha256":""
        });
        comparison["canonical_sha256"] = Value::String(canonical_json_hash(&comparison));
        let comparison_object = self.put_reference_comparison_object(
            None,
            None,
            &mut Vec::new(),
            &canonical_json_bytes(&comparison)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?,
            None,
            "application/json",
            "high-artifact-reference-comparison-v1",
        )?;
        Ok(json!({
            "schema_version":"HighArtifactReferenceComparisonPrepareResult@1",
            "project_id":project_id,
            "high_artifact_id":record.artifact_id,
            "high_artifact_sha256":record.high_artifact_sha256,
            "high_artifact_object_sha256":record.high_artifact_object_sha256,
            "high_artifact_readback_sha256":record.high_artifact_readback_sha256,
            "high_artifact_readback_object_sha256":record.high_artifact_readback_object_sha256,
            "high_artifact_receipt_sha256":record.receipt_sha256,
            "high_artifact_receipt_object_sha256":record.receipt_object_sha256,
            "reference_id":reference_id,
            "view_id":view_id,
            "camera":camera,
            "camera_object_sha256":camera_object.record.sha256,
            "render_set":render_set,
            "render_set_hash":render_set_object.record.sha256,
            "render_set_object_sha256":render_set_object.record.sha256,
            "comparison_report":comparison,
            "comparison_report_hash":comparison_object.record.sha256,
            "comparison_report_object_sha256":comparison_object.record.sha256,
            "high_artifact_status":record.structural_status,
            "visual_status":status,
            "human_status":"NOT_RUN",
            "engine_status":"NOT_RUN",
            "candidate_visual_evidence_projection":"NOT_UPDATED"
        }))
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
        // ReferenceViewSpec is the closed public request authority for the
        // view identity.  The historical top-level field remains accepted by
        // Runtime for compatibility, but it may not introduce a second,
        // drifting value.  Always propagating this resolved identity also
        // keeps RenderSet, comparison, quality and durable PassState aligned
        // when the default Knife MCP schema omits the legacy field.
        let view_id = Some(resolve_reference_view_id(object, view_spec)?);
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
        if render_set.get("schema_version").and_then(Value::as_str)
            == Some("HighArtifactRenderSet@1")
        {
            // The High selector validates the complete artifact lineage and
            // every fixed-render CAS object before this read is allowed.  It
            // intentionally does not pass through the legacy Candidate
            // RenderSet validator below.
            let _record = select_high_artifact_render_source(self, render_set_hash, &render_set)?;
            if !FIXED_RENDER_PASSES.contains(&pass_name) {
                return Err(RuntimeError::InvalidInput(
                    "RENDER_PASS_NOT_FOUND".to_owned(),
                ));
            }
            let artifact = render_set
                .get("pass_artifacts")
                .and_then(|value| value.get(pass_name))
                .ok_or_else(|| RuntimeError::InvalidInput("RENDER_PASS_NOT_FOUND".to_owned()))?;
            let pass_hash = required_value_sha(artifact.get("sha256"), "pass_artifact.sha256")?;
            let png = self.cas_read(pass_hash)?;
            return Ok(json!({
                "schema_version":"RenderPassGet@1",
                "render_set_hash":render_set_hash,
                "high_artifact_id":render_set["high_artifact_id"],
                "high_artifact_sha256":render_set["high_artifact_sha256"],
                "high_artifact_object_sha256":render_set["high_artifact_object_sha256"],
                "high_artifact_readback_sha256":render_set["high_artifact_readback_sha256"],
                "high_artifact_readback_object_sha256":render_set["high_artifact_readback_object_sha256"],
                "high_artifact_receipt_sha256":render_set["high_artifact_receipt_sha256"],
                "high_artifact_receipt_object_sha256":render_set["high_artifact_receipt_object_sha256"],
                "pass":pass_name,
                "mime":"image/png",
                "width":512,
                "height":512,
                "sha256":pass_hash,
                "png_base64":base64::engine::general_purpose::STANDARD.encode(png)
            }));
        }
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

    #[test]
    fn reference_view_identity_comes_from_the_closed_view_spec() {
        let request = json!({"project_id":"project"});
        let view_spec = json!({"view_id":"dragonfang-front-panel-001"});
        assert_eq!(
            resolve_reference_view_id(request.as_object().expect("request"), &view_spec)
                .expect("view id"),
            "dragonfang-front-panel-001"
        );

        let compatible = json!({"view_id":"dragonfang-front-panel-001"});
        assert_eq!(
            resolve_reference_view_id(compatible.as_object().expect("request"), &view_spec)
                .expect("compatible legacy view id"),
            "dragonfang-front-panel-001"
        );
    }

    #[test]
    fn legacy_top_level_view_identity_cannot_drift_from_the_view_spec() {
        let request = json!({"view_id":"another-view"});
        let view_spec = json!({"view_id":"dragonfang-front-panel-001"});
        let error = resolve_reference_view_id(request.as_object().expect("request"), &view_spec)
            .expect_err("drift must fail closed");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: REFERENCE_VIEW_BINDING_MISMATCH: top-level view_id differs from view_spec.view_id"
        );
    }
}
