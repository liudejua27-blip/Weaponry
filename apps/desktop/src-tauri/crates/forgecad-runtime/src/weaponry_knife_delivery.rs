//! Runtime-owned read-only inspection projection for the Dragonfang knife.
//!
//! This is deliberately an additive, internal seam. It does not add a
//! Contract or MCP method, does not create a candidate/version, and never
//! confirms or exports an asset. It resolves one already durable V2 High
//! artifact (or a future durable Low link), renders the same five closed
//! Dragonfang cameras, derives a blade-only mask from the Worker's semantic
//! part attribution, and returns an in-memory JSON inspection with the UE5.6
//! preflight boundary explicitly kept at `NOT_RUN`. The inspection is not
//! persisted and is not a whole-weapon assembly receipt.

use super::{
    canonical_json_bytes, canonical_json_hash, mask_to_png, native_high_glb_readback,
    render_worker, sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::{
    is_opaque_id, LowQuadDraftDurableRecord, LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
};
use forgecad_store::AuthoringMeshV2HighArtifactStoreRecord;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const REQUEST_SCHEMA: &str = "WeaponryKnifeDeliveryPrepareRequest@1";
const SIDECAR_SCHEMA: &str = "WeaponryKnifeDeliveryReadiness@1";
const FIXED_VIEW_POLICY: &str = "dragonfang-fixed-five-orthographic-cameras@1";
const FIXED_RENDER_RESOLUTION: u32 = 512;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const PNG_MIME: &str = "image/png";
const GLB_MIME: &str = "model/gltf-binary";
const HIGH_GLB_KIND: &str = "authoring-mesh-v2-high-artifact-glb@1";
const LOW_GLB_KIND: &str = LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND;
const BLADE_PART_IDS: [&str; 2] = ["blade-body", "cutting-edge"];
const FIXED_VIEWS: [(&str, &str); 5] = [
    ("view-front", "front"),
    ("view-top", "top"),
    ("view-bottom", "bottom"),
    ("view-left", "left"),
    ("view-right", "right"),
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPONRY_KNIFE_DELIVERY_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    role: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{role} must be an object")))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(format!(
            "{role} fields are not the closed delivery shape"
        )));
    }
    Ok(object)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, RuntimeError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} must be a non-empty string")))?;
    Ok(value)
}

fn required_opaque_id<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, RuntimeError> {
    let value = required_string(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque id")));
    }
    Ok(value)
}

#[derive(Debug)]
struct ResolvedSource {
    selector: Value,
    family: &'static str,
    artifact_id: String,
    glb_object_sha256: String,
    artifact_sha256: String,
    readback_object_sha256: String,
    readback_sha256: String,
    worker_build_cohort_sha256: Option<String>,
    glb: Vec<u8>,
    inspection: Value,
    part_ids: Vec<String>,
    source_metadata: Value,
}

fn load_source(
    runtime: &Runtime,
    project_id: &str,
    selector: &Value,
) -> Result<ResolvedSource, RuntimeError> {
    let selector_object = exact_object(selector, &["kind", "id"], "source_selector")?;
    let kind = required_string(selector_object, "kind")?;
    let id = required_opaque_id(selector_object, "id")?;
    match kind {
        "v2-high" => load_high_source(runtime, project_id, id, selector.clone()),
        "future-low" => load_low_source(runtime, project_id, id, selector.clone()),
        _ => Err(invalid(
            "source_selector.kind must be v2-high or future-low",
        )),
    }
}

fn load_high_source(
    runtime: &Runtime,
    project_id: &str,
    artifact_id: &str,
    selector: Value,
) -> Result<ResolvedSource, RuntimeError> {
    let record = runtime
        .store
        .get_authoring_mesh_v2_high_artifact_by_id(project_id, artifact_id)?
        .ok_or_else(|| invalid("durable V2 High artifact is unavailable"))?;
    let glb = runtime.cas_read_bounded(&record.high_artifact_object_sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&glb) != record.high_artifact_object_sha256
        || record.high_artifact_sha256 != record.high_artifact_object_sha256
    {
        return Err(invalid("V2 High GLB semantic/object hash binding differs"));
    }
    let inspection = native_high_glb_readback::inspect_authoring_mesh_v2_high_glb(&glb)
        .map_err(|error| invalid(format!("V2 High strict GLB readback failed: {error}")))?;
    let part_ids = inspection
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("V2 High readback part inventory is missing"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("V2 High readback part inventory contains an invalid id"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ResolvedSource {
        selector,
        family: "v2-high",
        artifact_id: record.artifact_id.clone(),
        glb_object_sha256: record.high_artifact_object_sha256.clone(),
        artifact_sha256: record.high_artifact_sha256.clone(),
        readback_object_sha256: record.high_artifact_readback_object_sha256.clone(),
        readback_sha256: record.high_artifact_readback_sha256.clone(),
        worker_build_cohort_sha256: Some(record.high_worker_build_cohort_sha256.clone()),
        glb,
        inspection,
        part_ids,
        source_metadata: high_source_metadata(&record),
    })
}

fn load_low_source(
    runtime: &Runtime,
    project_id: &str,
    link_id: &str,
    selector: Value,
) -> Result<ResolvedSource, RuntimeError> {
    let record = runtime
        .store
        .get_low_quad_draft_durable_by_link_id(link_id)?
        .ok_or_else(|| invalid("future Low durable link is unavailable"))?;
    if record.project_id != project_id {
        return Err(invalid("future Low link is outside the target project"));
    }
    let glb = runtime.cas_read_bounded(&record.artifact_object_sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&glb) != record.artifact_object_sha256
        || record.artifact_sha256 != record.artifact_object_sha256
    {
        return Err(invalid(
            "future Low GLB semantic/object hash binding differs",
        ));
    }
    let strict = super::strict_glb_inspection(&glb)?;
    if !strict.hard_gate_passed {
        return Err(invalid(format!(
            "future Low strict GLB readback failed: {}",
            strict.failure_codes.join(",")
        )));
    }
    let part_ids = strict.part_ids.clone();
    let mut inspection = strict.report_value();
    inspection["schema_version"] = Value::String("RuntimeStrictGlbIntegrity@1".to_owned());
    inspection["part_ids"] = json!(strict.part_ids);
    inspection["source_node_ids"] = json!(strict.source_node_ids);
    inspection["material_zone_ids"] = json!(strict.material_zone_ids);
    inspection["triangle_count"] = Value::from(strict.triangle_count);
    inspection["hard_gate_passed"] = Value::Bool(strict.hard_gate_passed);
    Ok(ResolvedSource {
        selector,
        family: "future-low",
        artifact_id: record.link_id.clone(),
        glb_object_sha256: record.artifact_object_sha256.clone(),
        artifact_sha256: record.artifact_sha256.clone(),
        readback_object_sha256: record.readback_object_sha256.clone(),
        readback_sha256: record.readback_sha256.clone(),
        worker_build_cohort_sha256: Some(record.worker_build_cohort_sha256.clone()),
        glb,
        inspection,
        part_ids,
        source_metadata: low_source_metadata(&record),
    })
}

fn high_source_metadata(record: &AuthoringMeshV2HighArtifactStoreRecord) -> Value {
    json!({
        "artifact_id": record.artifact_id,
        "bridge_id": record.bridge_id,
        "revision_id": record.revision_id,
        "materialized_candidate_id": record.materialized_candidate_id,
        "high_result_sha256": record.high_result_sha256,
        "high_worker_algorithm_sha256": record.high_worker_algorithm_sha256,
        "high_worker_build_cohort_sha256": record.high_worker_build_cohort_sha256,
        "materialization_status": record.materialization_status,
        "structural_status": record.structural_status,
        "visual_status": record.visual_status,
    })
}

fn low_source_metadata(record: &LowQuadDraftDurableRecord) -> Value {
    json!({
        "link_id": record.link_id,
        "candidate_id": record.candidate_id,
        "source_high_artifact_id": record.source_high_artifact_id,
        "worker_build_cohort_sha256": record.worker_build_cohort_sha256,
        "materialization_status": record.materialization_status,
    })
}

#[derive(Debug)]
struct ViewRender {
    view_id: &'static str,
    source_view: &'static str,
    camera: Value,
    render: render_worker::RenderWorkerRender,
    mask: Vec<bool>,
    selected_triangle_count: usize,
    selected_source_count: usize,
    source_table_sha256: String,
    triangle_ids_sha256: String,
}

fn blade_mask_from_attribution(
    attribution: &render_worker::RenderWorkerRasterAttribution,
    available_part_ids: &BTreeSet<String>,
) -> Result<(Vec<bool>, usize, usize), RuntimeError> {
    if attribution.width != FIXED_RENDER_RESOLUTION
        || attribution.height != FIXED_RENDER_RESOLUTION
        || attribution.raster_width != 1024
        || attribution.raster_height != 1024
        || attribution.triangle_ids_le.len()
            != (FIXED_RENDER_RESOLUTION * FIXED_RENDER_RESOLUTION * 4) as usize
    {
        return Err(invalid("part-id attribution dimensions are not 512x512"));
    }
    let selected_sources = attribution
        .sources
        .iter()
        .filter(|source| available_part_ids.contains(&source.semantic_part_id))
        .collect::<Vec<_>>();
    if selected_sources.is_empty() {
        return Err(invalid(
            "the source GLB has no blade-body or cutting-edge attribution",
        ));
    }
    let selected_triangles = selected_sources
        .iter()
        .map(|source| source.triangle_index)
        .collect::<BTreeSet<_>>();
    let mask = attribution
        .triangle_ids_le
        .chunks_exact(4)
        .map(|bytes| {
            let triangle = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            selected_triangles.contains(&triangle)
        })
        .collect::<Vec<_>>();
    Ok((mask, selected_triangles.len(), selected_sources.len()))
}

fn render_fixed_views(source: &ResolvedSource) -> Result<Vec<ViewRender>, RuntimeError> {
    let available_part_ids = source
        .part_ids
        .iter()
        .filter(|part_id| BLADE_PART_IDS.contains(&part_id.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    if available_part_ids.is_empty() {
        return Err(invalid("source GLB has no blade part inventory"));
    }
    FIXED_VIEWS
        .iter()
        .map(|(view_id, source_view)| {
            let camera = super::runtime_services::evaluation_service::reference_comparison::
                high_artifact_fixed_camera_for_source_view(source_view)?;
            let render = render_worker::render_glb_with_worker_identity(&source.glb, &camera)
                .map_err(|error| invalid(format!("fixed {source_view} render failed: {error}")))?;
            if render.passes.len() != 9 {
                return Err(invalid(format!(
                    "fixed {source_view} render returned {} AOVs, expected 9",
                    render.passes.len()
                )));
            }
            let attribution = render_worker::render_glb_raster_attribution(&source.glb, &camera)
                .map_err(|error| {
                    invalid(format!(
                        "fixed {source_view} part-id attribution failed: {error}"
                    ))
                })?;
            let (mask, selected_triangle_count, selected_source_count) =
                blade_mask_from_attribution(&attribution, &available_part_ids)?;
            Ok(ViewRender {
                view_id,
                source_view,
                camera,
                render,
                mask,
                selected_triangle_count,
                selected_source_count,
                source_table_sha256: attribution.source_table_sha256,
                triangle_ids_sha256: attribution.triangle_ids_sha256,
            })
        })
        .collect()
}

fn glb_external_uri_count(glb: &[u8]) -> Result<u64, RuntimeError> {
    if glb.len() < 20 || glb.get(..4) != Some(b"glTF") {
        return Err(invalid("GLB header is unavailable for UE5.6 preflight"));
    }
    let json_length = u32::from_le_bytes(
        glb[12..16]
            .try_into()
            .map_err(|_| invalid("GLB JSON length is invalid"))?,
    ) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| invalid("GLB JSON length overflows"))?;
    if json_end > glb.len() || glb.get(16..20) != Some(b"JSON") {
        return Err(invalid("GLB JSON chunk is invalid"));
    }
    let root: Value = serde_json::from_slice(&glb[20..json_end])
        .map_err(|error| invalid(format!("GLB JSON cannot be decoded: {error}")))?;
    fn walk(value: &Value, count: &mut u64) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if key == "uri" && child.as_str().is_some_and(|uri| !uri.is_empty()) {
                        *count = count.saturating_add(1);
                    }
                    walk(child, count);
                }
            }
            Value::Array(values) => values.iter().for_each(|child| walk(child, count)),
            _ => {}
        }
    }
    let mut count = 0;
    walk(&root, &mut count);
    Ok(count)
}

fn aov_sidecars(view: &ViewRender) -> Result<Vec<Value>, RuntimeError> {
    view.render
        .passes
        .iter()
        .map(|pass| {
            Ok(json!({
                "pass": pass.pass,
                "width": pass.width,
                "height": pass.height,
                "mime": PNG_MIME,
                "sha256": sha256_hex(&pass.png),
                "object_sha256": Value::Null,
                "cas_persistence": "NOT_PERFORMED_READ_ONLY_SEAM",
            }))
        })
        .collect()
}

fn view_sidecar(view: &ViewRender) -> Result<Value, RuntimeError> {
    let mask_png = mask_to_png(&view.mask)?;
    Ok(json!({
        "view_id": view.view_id,
        "source_view": view.source_view,
        "camera_hash": view.camera.get("camera_hash").cloned().unwrap_or(Value::Null),
        "camera": view.camera,
        "render_status": "RUNTIME_FIXED_VIEW_RENDERED",
        "render_profile": view.render.render_profile,
        "worker_build_cohort_sha256": view.render.build_cohort_sha256,
        "aov_passes": aov_sidecars(view)?,
        "blade_only_part_id_mask": {
            "projection": "raster-attribution-triangle-id-to-part-id@1",
            "width": FIXED_RENDER_RESOLUTION,
            "height": FIXED_RENDER_RESOLUTION,
            "mime": PNG_MIME,
            "mask_sha256": sha256_hex(&mask_png),
            "mask_object_sha256": Value::Null,
            "cas_persistence": "NOT_PERFORMED_READ_ONLY_SEAM",
            "selected_triangle_count": view.selected_triangle_count,
            "selected_source_count": view.selected_source_count,
            "source_table_sha256": view.source_table_sha256,
            "triangle_ids_sha256": view.triangle_ids_sha256,
        }
    }))
}

fn seal_sidecar(mut sidecar: Value) -> Result<(Value, Vec<u8>), RuntimeError> {
    sidecar["canonical_sha256"] = Value::String(String::new());
    sidecar["canonical_sha256"] = Value::String(canonical_json_hash(&sidecar));
    let bytes = canonical_json_bytes(&sidecar).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(invalid("delivery readiness sidecar exceeds 8 MiB"));
    }
    Ok((sidecar, bytes))
}

/// Resolve and render one already durable Dragonfang High/Low source.
///
/// This is an internal, read-only inspection seam.  It intentionally does not
/// write CAS, create a Store link, or enter the public Delivery façade.  A
/// future write route must first add a dedicated Store aggregate carrying the
/// request key, component/assembly receipt, sidecar roots and replay/GC
/// reachability semantics; routing this function directly would violate the
/// existing GameAssetDeliveryLink contract.
pub(crate) fn inspect(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "source_selector"],
        "delivery request",
    )?;
    if required_string(object, "schema_version")? != REQUEST_SCHEMA {
        return Err(invalid("delivery request schema_version differs"));
    }
    let project_id = required_opaque_id(object, "project_id")?;
    let source = load_source(
        runtime,
        project_id,
        object.get("source_selector").expect("closed field"),
    )?;
    let views = render_fixed_views(&source)?;
    if views.len() != FIXED_VIEWS.len() {
        return Err(invalid("fixed camera render count differs"));
    }
    let external_uri_count = glb_external_uri_count(&source.glb)?;
    let available_blade_parts = source
        .part_ids
        .iter()
        .filter(|part_id| BLADE_PART_IDS.contains(&part_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut fixed_views = Vec::with_capacity(views.len());
    for view in &views {
        fixed_views.push(view_sidecar(view)?);
    }
    let sidecar = json!({
        "schema_version": SIDECAR_SCHEMA,
        "project_id": project_id,
        "delivery_policy": FIXED_VIEW_POLICY,
        "source_selector": source.selector,
        "source": {
            "family": source.family,
            "artifact_id": source.artifact_id,
            "artifact_sha256": source.artifact_sha256,
            "artifact_object_sha256": source.glb_object_sha256,
            "readback_sha256": source.readback_sha256,
            "readback_object_sha256": source.readback_object_sha256,
            "worker_build_cohort_sha256": source.worker_build_cohort_sha256,
            "structural_readback": source.inspection,
            "metadata": source.source_metadata,
            "glb_mime": GLB_MIME,
            "glb_kind": if source.family == "v2-high" { HIGH_GLB_KIND } else { LOW_GLB_KIND },
        },
        "blade_only_part_selection": {
            "allowed_part_ids": BLADE_PART_IDS,
            "selected_part_ids": available_blade_parts,
            "selection_status": if available_blade_parts.len() == BLADE_PART_IDS.len() { "READY" } else { "PARTIAL" },
            "mask_projection": "raster-attribution-triangle-id-to-part-id@1",
        },
        "assembly": {
            "asset_scope": "blade-components-only",
            "status": "NOT_MATERIALIZED",
            "foundation_required": "dragonfang-high-10-part",
            "foundation_and_v2_lineage_must_be_same_project": true,
            "replacement_part_ids": BLADE_PART_IDS,
            "runtime_owned_successor_required": true,
            "assembly_receipt_status": "NOT_ACCEPTED",
            "whole_weapon_claim": false,
        },
        "fixed_views": fixed_views,
        "ue5_6_readiness": {
            "profile": "unreal-engine-5.6-static-mesh-glb-preflight@1",
            "runtime_preflight_status": if external_uri_count == 0 { "PASS_EMBEDDED_GLB_CONTAINER_ONLY" } else { "BLOCKED_EXTERNAL_URI" },
            "status": "NOT_RUN",
            "gltf_version": "2.0",
            "container": "GLB",
            "embedded_resources_only": external_uri_count == 0,
            "external_uri_count": external_uri_count,
            "static_mesh_import": "NOT_RUN",
            "material_instance_binding": "NOT_RUN",
            "tangent_validation": "NOT_RUN",
            "lod_validation": "NOT_RUN",
            "collision_validation": "NOT_RUN",
            "socket_validation": "NOT_RUN",
            "actual_engine_roundtrip": false,
        },
        "material_status": "NOT_RUN",
        "visual_quality_status": "NOT_RUN",
        "human_review_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "export_status": "NOT_RUN",
        "commercial_status": "BLOCKED_PENDING_MATERIAL_HUMAN_ENGINE_EXPORT",
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "persistence_status": "READ_ONLY_TRANSIENT_INSPECTION",
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "limitations": [
            "no-material-authoring-or-bake",
            "no-visual-quality-promotion",
            "no-human-review",
            "no-ue5.6-engine-roundtrip",
            "no-export-or-commercial-release"
        ],
        "canonical_sha256": ""
    });
    let (sidecar, bytes) = seal_sidecar(sidecar)?;
    Ok(json!({
        "schema_version": "WeaponryKnifeDeliveryPrepareResult@1",
        "sidecar": sidecar,
        "sidecar_sha256": sha256_hex(&bytes),
        "sidecar_object_sha256": Value::Null,
        "sidecar_persistence": "NOT_PERFORMED_READ_ONLY_SEAM",
        "fixed_view_count": FIXED_VIEWS.len(),
        "render_status": "RUNTIME_FIXED_FIVE_VIEW_RENDERED",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "export_status": "NOT_RUN",
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false
    }))
}
