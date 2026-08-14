//! Runtime-owned typed adapter for the isolated Render Worker.
//!
//! Geometry Worker compilation ends at a persisted-model GLB.  This module
//! owns the Runtime-side Render Worker protocol projection after that point:
//! it accepts only bounded GLB bytes and typed cameras, validates the fixed
//! response shape, and returns transient or nine-AOV render passes.  It does
//! not write Runtime state and it never accepts a GeometryProgram.

use super::geometry_worker::{self, GeometryWorkerError};
use serde_json::{json, Value};

const RENDER_WORKER_BINARY: &str = "forgecad-render-worker";

/// Launch the fixed Render Worker sibling through the generic Runtime
/// transport seam. This module owns the Render Worker identity; Geometry
/// Worker owns neither this binary nor this protocol.
fn execute_render_worker(operation: &str, payload: Value) -> Result<Value, GeometryWorkerError> {
    geometry_worker::execute_sibling_worker(RENDER_WORKER_BINARY, operation, payload)
}

#[derive(Debug, Clone)]
pub(crate) struct RenderPass {
    pub pass: String,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn render_fixed_glb(glb: &[u8]) -> Result<Vec<RenderPass>, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = execute_render_worker("render_fixed", json!({"glb_base64":encoded}))?;
    let object = strict_object(&result)?;
    require_exact_keys(object, &["schema_version", "passes"])?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerResult@1") {
        return Err(GeometryWorkerError::Protocol);
    }
    let values = object
        .get("passes")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 16)
        .ok_or(GeometryWorkerError::Protocol)?;
    let mut passes = Vec::with_capacity(values.len());
    for value in values {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        let pass_name = pass
            .get("pass")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 64)
            .ok_or(GeometryWorkerError::Protocol)?;
        if pass.get("mime").and_then(Value::as_str) != Some("image/png") {
            return Err(GeometryWorkerError::Protocol);
        }
        let width = pass
            .get("width")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value <= 4096)
            .ok_or(GeometryWorkerError::Protocol)?;
        let height = pass
            .get("height")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0 && *value <= 4096)
            .ok_or(GeometryWorkerError::Protocol)?;
        let png = decode_png(pass.get("png_base64"))?;
        passes.push(RenderPass {
            pass: pass_name.to_owned(),
            png,
            width,
            height,
        });
    }
    Ok(passes)
}

pub(crate) fn render_glb(
    glb: &[u8],
    camera: &Value,
) -> Result<Vec<RenderPass>, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err(GeometryWorkerError::Protocol);
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result =
        execute_render_worker("render_glb", json!({"glb_base64":encoded,"camera":camera}))?;
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "passes",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerResult@2")
        || object.get("width").and_then(Value::as_u64) != Some(512)
        || object.get("height").and_then(Value::as_u64) != Some(512)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let values = object
        .get("passes")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 9)
        .ok_or(GeometryWorkerError::Protocol)?;
    let expected = [
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
    let mut passes = Vec::with_capacity(9);
    for (value, expected_name) in values.iter().zip(expected) {
        let pass = strict_object(value)?;
        require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(GeometryWorkerError::Protocol);
        }
        let png = decode_png(pass.get("png_base64"))?;
        passes.push(RenderPass {
            pass: expected_name.to_owned(),
            png,
            width: 512,
            height: 512,
        });
    }
    Ok(passes)
}

pub(crate) fn render_glb_fit_batch(
    glb: &[u8],
    cameras: &[Value],
) -> Result<Vec<Vec<RenderPass>>, GeometryWorkerError> {
    render_glb_fit_batch_at_resolution(glb, cameras, 128)
}

pub(crate) fn render_glb_fit_batch_at_resolution(
    glb: &[u8],
    cameras: &[Value],
    resolution: u32,
) -> Result<Vec<Vec<RenderPass>>, GeometryWorkerError> {
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 || cameras.is_empty() || cameras.len() > 64 {
        return Err(GeometryWorkerError::Protocol);
    }
    if !matches!(resolution, 128 | 512) {
        return Err(GeometryWorkerError::Protocol);
    }
    #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
    let fallback = || {
        cameras
            .iter()
            .map(|camera| {
                forgecad_render_core::render_perspective_glb_fit_at_resolution(
                    glb, camera, resolution,
                )
                .map(|passes| {
                    passes
                        .into_iter()
                        .map(|pass| RenderPass {
                            pass: pass.pass,
                            png: pass.png,
                            width: pass.width,
                            height: pass.height,
                        })
                        .collect()
                })
                .map_err(|_| GeometryWorkerError::Rejected)
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, glb);
    let result = match execute_render_worker(
        "render_glb_fit_batch",
        json!({"glb_base64":encoded,"cameras":cameras,"resolution":resolution}),
    ) {
        Ok(result) => result,
        #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
        Err(GeometryWorkerError::Unavailable) => return fallback(),
        Err(error) => return Err(error),
    };
    let object = strict_object(&result)?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "width",
            "height",
            "renderer_revision",
            "renders",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("RenderWorkerFitBatchResult@1")
        || object.get("width").and_then(Value::as_u64) != Some(resolution as u64)
        || object.get("height").and_then(Value::as_u64) != Some(resolution as u64)
        || object.get("renderer_revision").and_then(Value::as_str) != Some("forgecad-renderer-2")
    {
        return Err(GeometryWorkerError::Protocol);
    }
    let renders = object
        .get("renders")
        .and_then(Value::as_array)
        .filter(|values| values.len() == cameras.len() && values.len() <= 64)
        .ok_or(GeometryWorkerError::Protocol)?;
    let expected = ["silhouette", "part-id"];
    let mut output = Vec::with_capacity(renders.len());
    for (index, render) in renders.iter().enumerate() {
        let render = strict_object(render)?;
        require_exact_keys(render, &["index", "passes"])?;
        if render.get("index").and_then(Value::as_u64) != Some(index as u64) {
            return Err(GeometryWorkerError::Protocol);
        }
        let values = render
            .get("passes")
            .and_then(Value::as_array)
            .filter(|values| values.len() == expected.len())
            .ok_or(GeometryWorkerError::Protocol)?;
        let mut passes = Vec::with_capacity(expected.len());
        for (value, expected_name) in values.iter().zip(expected) {
            let pass = strict_object(value)?;
            require_exact_keys(pass, &["pass", "mime", "width", "height", "png_base64"])?;
            if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
                || pass.get("mime").and_then(Value::as_str) != Some("image/png")
                || pass.get("width").and_then(Value::as_u64) != Some(resolution as u64)
                || pass.get("height").and_then(Value::as_u64) != Some(resolution as u64)
            {
                return Err(GeometryWorkerError::Protocol);
            }
            passes.push(RenderPass {
                pass: expected_name.to_owned(),
                png: decode_png(pass.get("png_base64"))?,
                width: resolution,
                height: resolution,
            });
        }
        output.push(passes);
    }
    Ok(output)
}

fn strict_object(value: &Value) -> Result<&serde_json::Map<String, Value>, GeometryWorkerError> {
    value.as_object().ok_or(GeometryWorkerError::Protocol)
}

fn require_exact_keys(
    value: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> Result<(), GeometryWorkerError> {
    if value.len() != allowed.len() || allowed.iter().any(|key| !value.contains_key(*key)) {
        return Err(GeometryWorkerError::Protocol);
    }
    Ok(())
}

fn decode_png(value: Option<&Value>) -> Result<Vec<u8>, GeometryWorkerError> {
    let encoded = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(GeometryWorkerError::Protocol)?;
    let png = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encoded.as_bytes(),
    )
    .map_err(|_| GeometryWorkerError::Protocol)?;
    if png.is_empty() || png.len() > 16 * 1024 * 1024 || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(GeometryWorkerError::Protocol);
    }
    Ok(png)
}

// This gate deliberately requires a source-built same-cohort Render Worker
// sibling. The GLB fixture is compiled locally only to isolate the render
// boundary; the nine AOVs and their determinism must come from the real
// Render Worker process, not the in-process render-core fallback.
#[cfg(all(test, target_os = "macos"))]
mod isolated_tests {
    use super::*;
    use serde_json::json;

    fn v1_fixture_program() -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@1",
            "project_id":"mcp010f-render-worker-isolation-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "nodes":[
                {"node_id":"torso","operator_id":"forgecad.geometry.primitive@1","part_id":"torso","parameters":{"shape":"box","size":[1.2,1.6,0.55],"position":[0.0,1.7,0.0],"material_zone_id":"zone-white-shell"}},
                {"node_id":"core","operator_id":"forgecad.geometry.primitive@1","part_id":"core","parameters":{"shape":"cylinder","size":[0.55,1.2,0.55],"position":[0.0,1.5,0.0],"material_zone_id":"zone-black-mechanical","segments":16}},
                {"node_id":"head","operator_id":"forgecad.geometry.primitive@1","part_id":"head","parameters":{"shape":"sphere","size":[0.85,0.9,0.85],"position":[0.0,2.75,0.0],"material_zone_id":"zone-white-shell","segments":16}}
            ],
            "budgets":{"max_nodes":16,"max_triangles":20000,"max_runtime_ms":1000}
        });
        program["canonical_sha256"] = Value::String(crate::canonical_json_hash(&program));
        program
    }

    fn fixed_camera() -> Value {
        json!({
            "schema_version":"CameraCalibration@1",
            "projection":"perspective",
            "transform":{"position_m":[4.0,3.0,6.0],"target_m":[0.0,1.5,0.0],"up":[0.0,1.0,0.0]},
            "fov_y_degrees":42.0,
            "near_m":0.05,
            "far_m":20.0,
            "resolution":{"width":512,"height":512},
            "coordinate_system":"right-handed-y-up-meter"
        })
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_render_worker_returns_deterministic_nine_aov_set() {
        let artifact = geometry_worker::compile_geometry_test_fallback(&v1_fixture_program(), None)
            .expect("fixture GLB");
        let camera = fixed_camera();
        let first = render_glb(&artifact.glb, &camera).expect("isolated render");
        assert_eq!(
            first
                .iter()
                .map(|pass| pass.pass.as_str())
                .collect::<Vec<_>>(),
            vec![
                "beauty",
                "silhouette",
                "depth",
                "normal",
                "ao",
                "part-id",
                "material-id",
                "wireframe",
                "uv-stretch"
            ]
        );
        assert!(first.iter().all(|pass| {
            pass.width == 512 && pass.height == 512 && pass.png.starts_with(b"\x89PNG\r\n\x1a\n")
        }));
        let second = render_glb(&artifact.glb, &camera).expect("repeat isolated render");
        assert_eq!(
            first.iter().map(|pass| &pass.png).collect::<Vec<_>>(),
            second.iter().map(|pass| &pass.png).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires source-built same-cohort forgecad-render-worker sibling"]
    fn isolated_render_worker_rejects_geometry_program_payload() {
        let error = execute_render_worker(
            "render_glb",
            json!({"geometry_program":v1_fixture_program()}),
        )
        .expect_err("Render Worker must reject compiler payloads");
        assert!(matches!(error, GeometryWorkerError::Rejected));
    }
}
