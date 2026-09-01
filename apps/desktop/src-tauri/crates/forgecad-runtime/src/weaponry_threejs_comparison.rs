//! Runtime-owned comparison for the packaged Three.js knife preview.
//!
//! This is deliberately a small, deterministic bridge between the existing
//! preview receipt and an authorized ReferenceEvidence object.  It does not
//! invoke a browser, infer a camera, or ask a model to judge likeness.  The
//! FRONT `semantic-id` pass is decoded and restricted to the two editable
//! blade parts, so guard, grip, ornament and material changes cannot hide in
//! the first correction range.

use super::{
    boundary_f1, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    sdf_chamfer_px, stable_visual_metric, Runtime, RuntimeError,
};
use forgecad_store::{
    WeaponryThreeJsComparisonCommit, WeaponryThreeJsComparisonStoreRecord,
    WeaponryThreeJsPreviewStoreRecord, WEAPONRY_THREEJS_COMPARISON_AOV_ID,
    WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS, WEAPONRY_THREEJS_COMPARISON_EDITABLE_PART_IDS,
    WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS, WEAPONRY_THREEJS_COMPARISON_FROZEN_PART_IDS,
    WEAPONRY_THREEJS_COMPARISON_HANDEDNESS_TRANSFORM, WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS,
    WEAPONRY_THREEJS_COMPARISON_METRIC_POLICY, WEAPONRY_THREEJS_COMPARISON_OPERATION,
    WEAPONRY_THREEJS_COMPARISON_RECEIPT_KIND, WEAPONRY_THREEJS_COMPARISON_RECEIPT_MIME,
    WEAPONRY_THREEJS_COMPARISON_RECEIPT_SCHEMA, WEAPONRY_THREEJS_COMPARISON_RECORD_SCHEMA,
    WEAPONRY_THREEJS_COMPARISON_STATUS, WEAPONRY_THREEJS_COMPARISON_VIEW_ID,
    WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS, WEAPONRY_THREEJS_PREVIEW_AOV_MIME,
};
use image::{imageops, GenericImageView, Rgba, RgbaImage};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const PREPARE_OPERATION: &str = "weaponry_threejs_knife_comparison_prepare";
pub(crate) const GET_OPERATION: &str = "weaponry_threejs_knife_comparison_get";
const PREPARE_SCHEMA: &str = "WeaponryThreeJsKnifeComparisonPrepareRequest@1";
const GET_SCHEMA: &str = "WeaponryThreeJsKnifeComparisonGetRequest@1";
const RESULT_SCHEMA: &str = "WeaponryThreeJsKnifeComparisonResult@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const INPUT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const RESULT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const FIXED_VIEW_ID: &str = WEAPONRY_THREEJS_COMPARISON_VIEW_ID;
const FIXED_AOV_ID: &str = WEAPONRY_THREEJS_COMPARISON_AOV_ID;
const METRIC_POLICY: &str = WEAPONRY_THREEJS_COMPARISON_METRIC_POLICY;
const HANDEDNESS_TRANSFORM: &str = WEAPONRY_THREEJS_COMPARISON_HANDEDNESS_TRANSFORM;
const NORMALIZATION_POLICY: &str = "aspect-preserving-foreground-bbox-fit@1:512x512:8px-margin";
const EDITABLE_PARTS: [&str; 2] = ["blade-body", "cutting-edge"];
const FIXED_REFERENCE_CROP: Crop = Crop {
    x: 10,
    y: 20,
    width: 650,
    height: 200,
};

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "preview_execution_id",
    "preview_program_sha256",
    "preview_program_object_sha256",
    "preview_worker_cohort_sha256",
    "preview_receipt_sha256",
    "preview_receipt_object_sha256",
    "preview_aov_sha256",
    "preview_aov_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "view_id",
    "reference_crop",
    "semantic_part_ids",
    "editable_part_ids",
    "frozen_part_ids",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "comparison_id",
    "comparison_sha256",
    "comparison_object_sha256",
    "preview_execution_id",
    "preview_program_sha256",
    "preview_program_object_sha256",
    "preview_worker_cohort_sha256",
    "preview_receipt_sha256",
    "preview_receipt_object_sha256",
    "preview_aov_sha256",
    "preview_aov_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "view_id",
    "reference_crop",
    "semantic_part_ids",
    "editable_part_ids",
    "frozen_part_ids",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPONRY_THREEJS_COMPARISON_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    request: &'a Value,
    fields: &[&str],
    schema: &str,
    operation: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = fields.iter().copied().collect();
    if actual != expected {
        return Err(invalid(format!(
            "{operation} request fields are not closed"
        )));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(schema)
        || object.get("operation").and_then(Value::as_str) != Some(operation)
    {
        return Err(invalid("schema_version or operation differs"));
    }
    Ok(object)
}

fn text(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be an opaque identifier")))
}

fn hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be a SHA-256")))
}

fn validate_header(
    request: &Value,
    object: &Map<String, Value>,
    read_only: bool,
) -> Result<(), RuntimeError> {
    if object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(INPUT_CANONICALIZATION)
        || (read_only
            && object
                .get("persistent_user_data_touched")
                .and_then(Value::as_bool)
                != Some(false))
    {
        return Err(invalid("request header or fixed policy differs"));
    }
    let supplied = hash(object, "input_sha256")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid("input_sha256 differs from canonical request"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn crop(object: &Map<String, Value>) -> Result<Crop, RuntimeError> {
    let value = object
        .get("reference_crop")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("reference_crop must be an object"))?;
    let read = |field: &str| {
        value
            .get(field)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid(format!("reference_crop.{field} must be a bounded integer")))
    };
    let result = Crop {
        x: read("x")?,
        y: read("y")?,
        width: read("width")?,
        height: read("height")?,
    };
    if result.width == 0 || result.height == 0 {
        return Err(invalid("reference_crop must be non-empty"));
    }
    if result.x != FIXED_REFERENCE_CROP.x
        || result.y != FIXED_REFERENCE_CROP.y
        || result.width != FIXED_REFERENCE_CROP.width
        || result.height != FIXED_REFERENCE_CROP.height
    {
        return Err(invalid(
            "reference_crop differs from the frozen Dragonfang FRONT crop",
        ));
    }
    Ok(result)
}

fn crop_value(crop: Crop) -> Value {
    json!({"x":crop.x,"y":crop.y,"width":crop.width,"height":crop.height})
}

fn part_ids(object: &Map<String, Value>) -> Result<BTreeMap<String, u32>, RuntimeError> {
    let value = object
        .get("semantic_part_ids")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("semantic_part_ids must be an object"))?;
    let keys: BTreeSet<&str> = value.keys().map(String::as_str).collect();
    if keys != EDITABLE_PARTS.into_iter().collect() {
        return Err(invalid(
            "semantic_part_ids must contain exactly the two blade parts",
        ));
    }
    let mut result = BTreeMap::new();
    for part in EDITABLE_PARTS {
        let id = value
            .get(part)
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id > 0 && *id <= 0x00ff_ffff)
            .ok_or_else(|| invalid(format!("semantic_part_ids.{part} is invalid")))?;
        result.insert(part.to_owned(), id);
    }
    if result.values().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(invalid("semantic_part_ids must be unique"));
    }
    Ok(result)
}

fn part_list(object: &Map<String, Value>, field: &str) -> Result<Vec<String>, RuntimeError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{field} must be an array")))?;
    let mut result = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("{field} contains an invalid part id")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let original = result.clone();
    result.sort();
    result.dedup();
    if result.len() != original.len() {
        return Err(invalid(format!("{field} contains duplicate parts")));
    }
    Ok(original)
}

fn validate_part_scope(
    design: &Value,
    semantic_ids: &BTreeMap<String, u32>,
    editable: &[String],
    frozen: &[String],
) -> Result<(), RuntimeError> {
    let parts = design
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("durable design has no parts"))?;
    let mut all = parts
        .iter()
        .map(|part| {
            part.get("part_id")
                .and_then(Value::as_str)
                .filter(|id| is_opaque_id(id))
                .map(str::to_owned)
                .ok_or_else(|| invalid("durable design contains an invalid part id"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    all.sort();
    all.dedup();
    if all.len() != parts.len() || !editable.iter().all(|part| all.contains(part)) {
        return Err(invalid(
            "editable part scope is not a subset of the durable design",
        ));
    }
    let expected_frozen = all
        .iter()
        .filter(|part| !editable.contains(part))
        .cloned()
        .collect::<Vec<_>>();
    let mut actual_frozen = frozen.to_vec();
    actual_frozen.sort();
    if actual_frozen != expected_frozen {
        return Err(invalid(
            "frozen_part_ids must be the exact complement of editable parts",
        ));
    }
    let sorted_parts = parts
        .iter()
        .filter_map(|part| part.get("part_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    for (index, part) in sorted_parts.iter().copied().enumerate() {
        if let Some(expected) = semantic_ids.get(part) {
            let derived = u32::try_from(
                sorted_parts
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| *value < &part)
                    .count()
                    + 1,
            )
            .map_err(|_| invalid("semantic id exceeds bounded range"))?;
            if *expected != derived {
                return Err(invalid(format!(
                    "semantic_part_ids.{part} does not match sorted durable design order"
                )));
            }
        }
        let _ = index;
    }
    Ok(())
}

fn read_preview_aov(
    runtime: &Runtime,
    record: &WeaponryThreeJsPreviewStoreRecord,
    receipt: &Value,
    view_id: &str,
    aov_id: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let view = receipt
        .get("views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("view_id").and_then(Value::as_str) == Some(view_id))
        })
        .ok_or_else(|| invalid("preview receipt does not contain the requested view"))?;
    let pass = view
        .get("passes")
        .and_then(Value::as_array)
        .and_then(|passes| {
            passes
                .iter()
                .find(|pass| pass.get("aov_id").and_then(Value::as_str) == Some(aov_id))
        })
        .ok_or_else(|| invalid("preview receipt does not contain the requested AOV"))?;
    let object_hash = pass
        .get("object_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("preview AOV object hash is invalid"))?;
    if pass.get("mime").and_then(Value::as_str) != Some(WEAPONRY_THREEJS_PREVIEW_AOV_MIME)
        || pass.get("sha256").and_then(Value::as_str) != Some(object_hash)
    {
        return Err(invalid(
            "preview AOV identity differs from the fixed PNG contract",
        ));
    }
    let (object, bytes) = runtime
        .store
        .read_weaponry_threejs_preview_aov_exact(
            &record.project_id,
            &record.execution_id,
            view_id,
            aov_id,
            object_hash,
        )?
        .ok_or_else(|| invalid("preview AOV CAS object is not registered"))?;
    if pass.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
        || object.sha256 != object_hash
    {
        return Err(invalid("preview AOV bytes do not match the receipt"));
    }
    let image = image::load_from_memory(&bytes)
        .map_err(|error| invalid(format!("preview AOV PNG is invalid: {error}")))?;
    if image.dimensions() != (512, 512) {
        return Err(invalid("preview AOV dimensions differ from fixed 512x512"));
    }
    Ok(bytes)
}

fn semantic_mask(bytes: &[u8], ids: &BTreeMap<String, u32>) -> Result<Vec<bool>, RuntimeError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| invalid(format!("semantic-id PNG is invalid: {error}")))?
        .to_rgba8();
    let allowed = ids.values().copied().collect::<BTreeSet<_>>();
    let mask = image
        .pixels()
        .map(|pixel| {
            let [red, green, blue, _] = pixel.0;
            allowed.contains(&u32::from_be_bytes([0, red, green, blue]))
        })
        .collect::<Vec<_>>();
    if !mask.iter().any(|value| *value) {
        return Err(invalid(
            "editable blade parts are not visible in FRONT semantic-id pass",
        ));
    }
    // Handedness is applied by the caller before this common bbox fit.  Do
    // not normalize here: fitting before the fixed mirror can introduce a
    // one-pixel parity shift for odd-width silhouettes.
    Ok(mask)
}

fn normalize_foreground_bbox(mask: &[bool]) -> Vec<bool> {
    let Some((min_x, min_y, max_x, max_y)) = mask_bbox(mask) else {
        return mask.to_vec();
    };
    let source_width = max_x - min_x + 1;
    let source_height = max_y - min_y + 1;
    let target_extent = 512_u32.saturating_sub(16);
    let (width, height) = if source_width >= source_height {
        (
            target_extent,
            ((source_height as u64 * target_extent as u64 + source_width as u64 / 2)
                / source_width as u64)
                .max(1) as u32,
        )
    } else {
        (
            ((source_width as u64 * target_extent as u64 + source_height as u64 / 2)
                / source_height as u64)
                .max(1) as u32,
            target_extent,
        )
    };
    let mut source = RgbaImage::from_pixel(source_width, source_height, Rgba([0, 0, 0, 255]));
    for y in 0..source_height {
        for x in 0..source_width {
            if mask[(min_y + y) as usize * 512 + (min_x + x) as usize] {
                source.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }
    let resized = imageops::resize(&source, width, height, imageops::FilterType::Nearest);
    let mut canvas = vec![false; 512 * 512];
    let offset_x = (512 - width) / 2;
    let offset_y = (512 - height) / 2;
    for y in 0..height {
        for x in 0..width {
            if resized.get_pixel(x, y).0[0] > 0 {
                canvas[(offset_y + y) as usize * 512 + (offset_x + x) as usize] = true;
            }
        }
    }
    canvas
}

fn mirror_mask_x(mask: &[bool]) -> Vec<bool> {
    let mut mirrored = vec![false; mask.len()];
    for y in 0..512usize {
        for x in 0..512usize {
            mirrored[y * 512 + (511 - x)] = mask[y * 512 + x];
        }
    }
    mirrored
}

fn mask_bbox(mask: &[bool]) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = 512;
    let mut min_y = 512;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut seen = false;
    for (index, value) in mask.iter().enumerate() {
        if !*value {
            continue;
        }
        let x = (index % 512) as u32;
        let y = (index / 512) as u32;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        seen = true;
    }
    seen.then_some((min_x, min_y, max_x, max_y))
}

fn crop_reference_mask(bytes: &[u8], crop: Crop) -> Result<Vec<bool>, RuntimeError> {
    let source = image::load_from_memory(bytes)
        .map_err(|error| invalid(format!("ReferenceEvidence PNG is invalid: {error}")))?;
    let (width, height) = source.dimensions();
    if crop.x > width
        || crop.y > height
        || crop.width > width.saturating_sub(crop.x)
        || crop.height > height.saturating_sub(crop.y)
    {
        return Err(invalid(
            "reference_crop exceeds ReferenceEvidence dimensions",
        ));
    }
    let crop = source
        .crop_imm(crop.x, crop.y, crop.width, crop.height)
        .to_rgba8();
    let mut mask = vec![false; (crop.width() * crop.height()) as usize];
    let mut border = Vec::new();
    for x in 0..crop.width() {
        border.push(crop.get_pixel(x, 0).0);
        border.push(crop.get_pixel(x, crop.height() - 1).0);
    }
    for y in 0..crop.height() {
        border.push(crop.get_pixel(0, y).0);
        border.push(crop.get_pixel(crop.width() - 1, y).0);
    }
    border.sort_by_key(|pixel| (pixel[0], pixel[1], pixel[2]));
    let median = border[border.len() / 2];
    for (index, pixel) in crop.pixels().enumerate() {
        let [red, green, blue, alpha] = pixel.0;
        let distance = ((red as i32 - median[0] as i32).pow(2)
            + (green as i32 - median[1] as i32).pow(2)
            + (blue as i32 - median[2] as i32).pow(2))
        .isqrt();
        let max_channel = red.max(green).max(blue);
        let min_channel = red.min(green).min(blue);
        let mean = u16::from(red) + u16::from(green) + u16::from(blue);
        mask[index] = alpha > 0
            && ((distance >= 34 && max_channel >= 45 && max_channel - min_channel >= 12)
                || mean >= 255);
    }
    let component = largest_component(&mask, crop.width() as usize, crop.height() as usize);
    let mut row_filled = vec![false; mask.len()];
    for y in 0..crop.height() as usize {
        let xs = (0..crop.width() as usize)
            .filter(|x| component[y * crop.width() as usize + x])
            .collect::<Vec<_>>();
        if let (Some(min_x), Some(max_x)) = (xs.first(), xs.last()) {
            for x in *min_x..=*max_x {
                row_filled[y * crop.width() as usize + x] = true;
            }
        }
    }
    if !row_filled.iter().any(|value| *value) {
        return Err(invalid(
            "frozen Dragonfang reference crop has no foreground",
        ));
    }
    Ok(normalize_variable_mask(
        &row_filled,
        crop.width() as usize,
        crop.height() as usize,
    ))
}

fn largest_component(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut visited = vec![false; mask.len()];
    let mut largest = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        visited[start] = true;
        while let Some(index) = stack.pop() {
            component.push(index);
            let x = index % width;
            let y = index / width;
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x + 1, y.wrapping_sub(1)),
                (x.wrapping_sub(1), y + 1),
                (x + 1, y + 1),
            ] {
                if nx < width && ny < height {
                    let next = ny * width + nx;
                    if mask[next] && !visited[next] {
                        visited[next] = true;
                        stack.push(next);
                    }
                }
            }
        }
        if component.len() > largest.len() {
            largest = component;
        }
    }
    let mut result = vec![false; mask.len()];
    for index in largest {
        result[index] = true;
    }
    result
}

fn normalize_variable_mask(mask: &[bool], width: usize, height: usize) -> Vec<bool> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut seen = false;
    for (index, value) in mask.iter().enumerate() {
        if *value {
            let x = index % width;
            let y = index / width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            seen = true;
        }
    }
    if !seen {
        return vec![false; 512 * 512];
    }
    let source_width = max_x - min_x + 1;
    let source_height = max_y - min_y + 1;
    let target_extent = 512_u32.saturating_sub(16);
    let (target_width, target_height) = if source_width >= source_height {
        (
            target_extent,
            ((source_height as u64 * target_extent as u64 + source_width as u64 / 2)
                / source_width as u64)
                .max(1) as u32,
        )
    } else {
        (
            ((source_width as u64 * target_extent as u64 + source_height as u64 / 2)
                / source_height as u64)
                .max(1) as u32,
            target_extent,
        )
    };
    let mut source = RgbaImage::from_pixel(
        source_width as u32,
        source_height as u32,
        Rgba([0, 0, 0, 255]),
    );
    for y in 0..source_height {
        for x in 0..source_width {
            if mask[(min_y + y) * width + min_x + x] {
                source.put_pixel(x as u32, y as u32, Rgba([255, 255, 255, 255]));
            }
        }
    }
    let resized = imageops::resize(
        &source,
        target_width,
        target_height,
        imageops::FilterType::Nearest,
    );
    let mut result = vec![false; 512 * 512];
    let offset_x = (512 - target_width) / 2;
    let offset_y = (512 - target_height) / 2;
    for y in 0..target_height {
        for x in 0..target_width {
            result[(offset_y + y) as usize * 512 + (offset_x + x) as usize] =
                resized.get_pixel(x, y).0[0] > 0;
        }
    }
    result
}

fn load_source(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<
    (
        WeaponryThreeJsPreviewStoreRecord,
        Value,
        Value,
        String,
        Vec<u8>,
        Vec<u8>,
    ),
    RuntimeError,
> {
    let project_id = text(object, "project_id")?;
    let execution_id = text(object, "preview_execution_id")?;
    let program_sha256 = hash(object, "preview_program_sha256")?;
    let program_object_sha256 = hash(object, "preview_program_object_sha256")?;
    let cohort = hash(object, "preview_worker_cohort_sha256")?;
    let receipt_sha256 = hash(object, "preview_receipt_sha256")?;
    let receipt_object_sha256 = hash(object, "preview_receipt_object_sha256")?;
    let requested_aov_sha256 = hash(object, "preview_aov_sha256")?;
    let requested_aov_object_sha256 = hash(object, "preview_aov_object_sha256")?;
    if object.get("view_id").and_then(Value::as_str) != Some(FIXED_VIEW_ID) {
        return Err(invalid("comparison only accepts the frozen FRONT view"));
    }
    let record = runtime
        .store
        .get_weaponry_threejs_preview_by_id(&project_id, &execution_id)?
        .ok_or_else(|| invalid("exact durable preview was not found"))?;
    // The existing preview exact lookup includes Worker-result identity and
    // runtime/dependency hashes.  Comparison requests bind only the receipt
    // and cohort, so verify those values after the lookup rather than accept
    // placeholders as a new source of truth.
    if record.preview_worker_cohort_sha256 != cohort
        || record.preview_receipt_sha256 != receipt_sha256
        || record.preview_receipt_object_sha256 != receipt_object_sha256
        || record.program_sha256 != program_sha256
        || record.program_object_sha256 != program_object_sha256
        || record.action != "preview"
    {
        return Err(invalid("preview source identity differs from request"));
    }
    let design = runtime.store.read_weaponry_threejs_program_json(
        &runtime
            .store
            .get_weaponry_threejs_design_exact(
                &project_id,
                &record.design_id,
                &program_sha256,
                &program_object_sha256,
            )?
            .ok_or_else(|| invalid("preview design source is unavailable"))?,
    )?;
    let receipt = runtime
        .store
        .read_weaponry_threejs_preview_receipt_json(&record)?;
    let aov_hash = receipt
        .get("views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views
                .iter()
                .find(|view| view.get("view_id").and_then(Value::as_str) == Some(FIXED_VIEW_ID))
        })
        .and_then(|view| view.get("passes").and_then(Value::as_array))
        .and_then(|passes| {
            passes
                .iter()
                .find(|pass| pass.get("aov_id").and_then(Value::as_str) == Some(FIXED_AOV_ID))
        })
        .and_then(|pass| pass.get("object_sha256").and_then(Value::as_str))
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("preview semantic-id object hash is missing"))?
        .to_owned();
    if requested_aov_sha256 != aov_hash || requested_aov_object_sha256 != aov_hash {
        return Err(invalid(
            "preview semantic-id AOV hash differs from its receipt",
        ));
    }
    let aov = read_preview_aov(runtime, &record, &receipt, FIXED_VIEW_ID, FIXED_AOV_ID)?;
    let reference_id = text(object, "reference_id")?;
    let reference_object_sha256 = hash(object, "reference_object_sha256")?;
    let reference_evidence_sha256 = hash(object, "reference_evidence_sha256")?;
    let reference = runtime
        .reference(&reference_id)?
        .ok_or_else(|| invalid("ReferenceEvidence is unavailable"))?;
    if reference.project_id != project_id
        || reference.object_sha256 != reference_object_sha256
        || reference.canonical_sha256 != reference_evidence_sha256
    {
        return Err(invalid("ReferenceEvidence identity differs from request"));
    }
    let reference_bytes = runtime.cas_read(&reference_object_sha256)?;
    Ok((record, design, receipt, aov_hash, aov, reference_bytes))
}

fn metrics(reference: &[bool], model: &[bool]) -> (Value, Value) {
    let intersection = reference
        .iter()
        .zip(model)
        .filter(|(left, right)| **left && **right)
        .count();
    let union = reference
        .iter()
        .zip(model)
        .filter(|(left, right)| **left || **right)
        .count();
    let reference_area = reference.iter().filter(|value| **value).count();
    let model_area = model.iter().filter(|value| **value).count();
    let iou = stable_visual_metric(if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    });
    let boundary = stable_visual_metric(boundary_f1(reference, model, 4));
    let chamfer_px = stable_visual_metric(sdf_chamfer_px(reference, model));
    let chamfer = stable_visual_metric((chamfer_px / 512.0).clamp(0.0, 1.0));
    let value = json!({
        "silhouette_iou": iou,
        "boundary_f1_4px": boundary,
        "symmetric_chamfer_px": chamfer_px,
        "symmetric_chamfer_normalized": chamfer,
        "reference_area_fraction": stable_visual_metric(reference_area as f64 / 512.0_f64.powi(2)),
        "model_area_fraction": stable_visual_metric(model_area as f64 / 512.0_f64.powi(2)),
    });
    let store = json!({
        "silhouette_iou_milli": (iou * 1_000_000.0).round() as u64,
        "boundary_f1_milli": (boundary * 1_000_000.0).round() as u64,
        "sdf_chamfer_milli": (chamfer * 1_000_000.0).round() as u64,
    });
    (value, store)
}

struct ComparisonComputation {
    preview: WeaponryThreeJsPreviewStoreRecord,
    aov_sha256: String,
    store_metrics: Value,
}

fn result_value(
    request_kind: &str,
    status: &str,
    comparison_id: &str,
    record: &WeaponryThreeJsPreviewStoreRecord,
    object: &Map<String, Value>,
    metric: Value,
    aov_sha256: &str,
    comparison_record: Option<&WeaponryThreeJsComparisonStoreRecord>,
) -> Result<Value, RuntimeError> {
    let comparison_receipt_sha256 = comparison_record
        .map(|record| record.comparison_receipt_sha256.clone())
        .unwrap_or_default();
    let comparison_receipt_object_sha256 = comparison_record
        .map(|record| record.comparison_receipt_object_sha256.clone())
        .unwrap_or_default();
    let mut result = json!({
        "schema_version": RESULT_SCHEMA,
        "operation": if request_kind == "get" { GET_OPERATION } else { PREPARE_OPERATION },
        "request_kind": request_kind,
        "status": status,
        "comparison_id": comparison_id,
        "project_id": record.project_id,
        "preview_execution_id": record.execution_id,
        "preview_design_id": record.design_id,
        "preview_program_sha256": record.program_sha256,
        "preview_program_object_sha256": record.program_object_sha256,
        "preview_worker_cohort_sha256": record.preview_worker_cohort_sha256,
        "preview_receipt_sha256": record.preview_receipt_sha256,
        "preview_receipt_object_sha256": record.preview_receipt_object_sha256,
        "preview_aov_sha256": aov_sha256,
        "preview_aov_object_sha256": aov_sha256,
        "reference_id": object["reference_id"],
        "reference_object_sha256": object["reference_object_sha256"],
        "reference_evidence_sha256": object["reference_evidence_sha256"],
        "view_id": FIXED_VIEW_ID,
        "aov_id": FIXED_AOV_ID,
        "handedness_transform": HANDEDNESS_TRANSFORM,
        "reference_crop": object["reference_crop"],
        "semantic_part_ids": object["semantic_part_ids"],
        "editable_part_ids": object["editable_part_ids"],
        "frozen_part_ids": object["frozen_part_ids"],
        "metric_policy": METRIC_POLICY,
        "normalization_policy": NORMALIZATION_POLICY,
        "metrics": metric,
        "thresholds": {"silhouette_iou_min":0.90,"boundary_f1_4px_min":0.90,"symmetric_chamfer_px_max":4.0},
        "comparison_status": WEAPONRY_THREEJS_COMPARISON_STATUS,
        "quality_status": WEAPONRY_THREEJS_COMPARISON_STATUS,
        "visual_status": WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS,
        "parent_retained": true,
        "candidate_created": false,
        "version_created": false,
        "export_performed": false,
        "human_status": WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS,
        "engine_status": WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS,
        "commercial_status": WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS,
        "comparison_sha256": if comparison_receipt_sha256.is_empty() { Value::Null } else { Value::String(comparison_receipt_sha256) },
        "comparison_object_sha256": if comparison_receipt_object_sha256.is_empty() { Value::Null } else { Value::String(comparison_receipt_object_sha256) },
        "idempotency_key": object.get("idempotency_key").cloned().unwrap_or(Value::Null),
        "replayed": status == "replayed",
        "store_effect": if status == "replayed" || status == "found" { "not-touched" } else { "inserted" },
        "cas_effect": if status == "replayed" || status == "found" { "not-touched" } else { "inserted" },
        "runtime_write_performed": status == "stored",
        "persistent_user_data_touched": status == "stored",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": RESULT_CANONICALIZATION,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("comparison result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn compute(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<ComparisonComputation, RuntimeError> {
    let semantic_ids = part_ids(object)?;
    let editable = part_list(object, "editable_part_ids")?;
    let frozen = part_list(object, "frozen_part_ids")?;
    if editable != EDITABLE_PARTS.map(str::to_owned) {
        return Err(invalid(
            "editable_part_ids must be blade-body then cutting-edge",
        ));
    }
    let crop = crop(object)?;
    let (record, design, _receipt, aov_hash, aov, reference_bytes) = load_source(runtime, object)?;
    validate_part_scope(&design, &semantic_ids, &editable, &frozen)?;
    let reference = crop_reference_mask(&reference_bytes, crop)?;
    let model = normalize_foreground_bbox(&mirror_mask_x(&semantic_mask(&aov, &semantic_ids)?));
    let (_metrics, store_metrics) = metrics(&reference, &model);
    Ok(ComparisonComputation {
        preview: record,
        aov_sha256: aov_hash,
        store_metrics,
    })
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, PREPARE_SCHEMA, PREPARE_OPERATION)?;
    validate_header(request, object, false)?;
    let project_id = text(object, "project_id")?;
    let idempotency_key = text(object, "idempotency_key")?;
    if runtime.project(&project_id)?.is_none() {
        return Err(invalid("project does not exist"));
    }
    let request_sha256 = hash(object, "input_sha256")?;
    let comparison_id = format!("three-comparison-{}", &request_sha256[..40]);
    let computed = compute(runtime, object)?;
    let crop = crop(object)?;
    let semantic_part_ids = part_ids(object)?;
    let editable_part_ids = WEAPONRY_THREEJS_COMPARISON_EDITABLE_PART_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let frozen_part_ids = WEAPONRY_THREEJS_COMPARISON_FROZEN_PART_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut receipt = json!({
        "schema_version": WEAPONRY_THREEJS_COMPARISON_RECEIPT_SCHEMA,
        "operation": WEAPONRY_THREEJS_COMPARISON_OPERATION,
        "project_id": computed.preview.project_id,
        "comparison_id": comparison_id,
        "preview_execution_id": computed.preview.execution_id,
        "preview_receipt_sha256": computed.preview.preview_receipt_sha256,
        "preview_receipt_object_sha256": computed.preview.preview_receipt_object_sha256,
        "preview_worker_cohort_sha256": computed.preview.preview_worker_cohort_sha256,
        "view_id": FIXED_VIEW_ID,
        "aov_id": FIXED_AOV_ID,
        "handedness_transform": HANDEDNESS_TRANSFORM,
        "preview_aov_sha256": computed.aov_sha256,
        "preview_aov_object_sha256": computed.aov_sha256,
        "reference_id": object["reference_id"],
        "reference_object_sha256": object["reference_object_sha256"],
        "reference_evidence_sha256": object["reference_evidence_sha256"],
        "reference_crop": crop_value(crop),
        "semantic_part_ids": semantic_part_ids,
        "editable_part_ids": editable_part_ids,
        "frozen_part_ids": frozen_part_ids,
        "metric_policy": METRIC_POLICY,
        "metrics": computed.store_metrics,
        "comparison_status": WEAPONRY_THREEJS_COMPARISON_STATUS,
        "visual_status": WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS,
        "human_status": WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS,
        "engine_status": WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS,
        "commercial_status": WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS,
        "parent_retained": true,
        "canonical_sha256": ""
    });
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    let receipt_bytes =
        canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let receipt_cas = match runtime.store.put_object_reserved(
        &reservation,
        &receipt_bytes,
        None,
        WEAPONRY_THREEJS_COMPARISON_RECEIPT_MIME,
        WEAPONRY_THREEJS_COMPARISON_RECEIPT_KIND,
        &super::now_string(),
    ) {
        Ok(object) => object,
        Err(error) => return Err(error.into()),
    };
    let record = WeaponryThreeJsComparisonStoreRecord {
        schema_version: WEAPONRY_THREEJS_COMPARISON_RECORD_SCHEMA.to_owned(),
        project_id: computed.preview.project_id.clone(),
        comparison_id: comparison_id.clone(),
        preview_execution_id: computed.preview.execution_id.clone(),
        preview_receipt_sha256: computed.preview.preview_receipt_sha256.clone(),
        preview_receipt_object_sha256: computed.preview.preview_receipt_object_sha256.clone(),
        preview_worker_cohort_sha256: computed.preview.preview_worker_cohort_sha256.clone(),
        preview_view_id: FIXED_VIEW_ID.to_owned(),
        preview_aov_id: FIXED_AOV_ID.to_owned(),
        handedness_transform: HANDEDNESS_TRANSFORM.to_owned(),
        preview_aov_sha256: computed.aov_sha256.clone(),
        preview_aov_object_sha256: computed.aov_sha256.clone(),
        reference_id: text(object, "reference_id")?,
        reference_object_sha256: hash(object, "reference_object_sha256")?,
        reference_evidence_sha256: hash(object, "reference_evidence_sha256")?,
        reference_crop_x: u64::from(crop.x),
        reference_crop_y: u64::from(crop.y),
        reference_crop_width: u64::from(crop.width),
        reference_crop_height: u64::from(crop.height),
        semantic_part_ids: semantic_part_ids,
        editable_part_ids: editable_part_ids,
        frozen_part_ids: frozen_part_ids,
        metric_policy: METRIC_POLICY.to_owned(),
        metrics: computed.store_metrics.clone(),
        comparison_status: WEAPONRY_THREEJS_COMPARISON_STATUS.to_owned(),
        visual_status: WEAPONRY_THREEJS_COMPARISON_VISUAL_STATUS.to_owned(),
        human_status: WEAPONRY_THREEJS_COMPARISON_HUMAN_STATUS.to_owned(),
        engine_status: WEAPONRY_THREEJS_COMPARISON_ENGINE_STATUS.to_owned(),
        commercial_status: WEAPONRY_THREEJS_COMPARISON_COMMERCIAL_STATUS.to_owned(),
        parent_retained: true,
        request_sha256,
        idempotency_key,
        comparison_receipt_sha256: receipt["canonical_sha256"]
            .as_str()
            .expect("sealed receipt")
            .to_owned(),
        comparison_receipt_object_sha256: receipt_cas.record.sha256.clone(),
        created_at: super::now_string(),
    };
    let (stored, replayed) = match runtime
        .store
        .record_weaponry_threejs_comparison_with_replay(&WeaponryThreeJsComparisonCommit {
            record,
            receipt: receipt_cas.record.clone(),
        }) {
        Ok(value) => value,
        Err(error) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &receipt_cas, true);
            return Err(error.into());
        }
    };
    let _ = runtime
        .store
        .release_cas_reservation_object(&reservation, &receipt_cas, false);
    let stored_receipt = runtime
        .store
        .read_weaponry_threejs_comparison_receipt_json(&stored)?;
    if stored_receipt["metrics"] != computed.store_metrics {
        return Err(invalid(
            "stored comparison receipt metrics differ from the computed source metrics",
        ));
    }
    result_value(
        "prepare",
        if replayed { "replayed" } else { "stored" },
        &stored.comparison_id,
        &computed.preview,
        object,
        stored_receipt["metrics"].clone(),
        &stored.preview_aov_sha256,
        Some(&stored),
    )
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, GET_SCHEMA, GET_OPERATION)?;
    validate_header(request, object, true)?;
    let project_id = text(object, "project_id")?;
    let comparison_id = text(object, "comparison_id")?;
    let expected_sha = hash(object, "comparison_sha256")?;
    let expected_object = hash(object, "comparison_object_sha256")?;
    if runtime.project(&project_id)?.is_none() {
        return Err(invalid("project does not exist"));
    }
    let computed = compute(runtime, object)?;
    let stored = runtime
        .store
        .get_weaponry_threejs_comparison_exact(
            &project_id,
            &comparison_id,
            HANDEDNESS_TRANSFORM,
            &hash(object, "preview_receipt_sha256")?,
            &hash(object, "preview_receipt_object_sha256")?,
            &hash(object, "preview_aov_sha256")?,
            &hash(object, "preview_aov_object_sha256")?,
            &hash(object, "reference_object_sha256")?,
            &hash(object, "reference_evidence_sha256")?,
            &expected_sha,
            &expected_object,
            &hash(object, "preview_worker_cohort_sha256")?,
        )?
        .ok_or_else(|| invalid("exact durable comparison was not found"))?;
    let receipt = runtime
        .store
        .read_weaponry_threejs_comparison_receipt_json(&stored)?;
    if receipt["metrics"] != computed.store_metrics {
        return Err(invalid(
            "durable comparison metrics differ from deterministic source recomputation",
        ));
    }
    result_value(
        "get",
        "found",
        &stored.comparison_id,
        &computed.preview,
        object,
        receipt["metrics"].clone(),
        &stored.preview_aov_sha256,
        Some(&stored),
    )
}

impl Runtime {
    pub fn weaponry_threejs_knife_comparison_prepare(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }

    pub fn weaponry_threejs_knife_comparison_get(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        get(self, request)
    }
}
