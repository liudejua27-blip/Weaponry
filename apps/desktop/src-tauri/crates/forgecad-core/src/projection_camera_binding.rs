//! Rust-owned deterministic camera bindings for reference-to-UV projection.
//!
//! The workbench may render the binding, but it does not invent camera
//! numbers. Rust derives one view from the exact compiler readback bounds,
//! the frozen turntable slot and the reviewed 38 degree perspective lens. The
//! binding is expressed in the original GLB metre coordinate system, so the
//! restricted geometry worker can use `world_to_clip_row_major` directly for
//! UV rasterization. The desktop's presentation-only centering and uniform
//! scaling do not change this projective relation.

use serde::{Deserialize, Serialize};

use crate::{
    semantic_sha256, CoreError, CoreResult, GeometryInvariantBinding, TURN_TABLE_EIGHT_VIEW_IDS,
};

pub const PROJECTION_CAMERA_BINDING_SCHEMA_VERSION: &str = "ProjectionCameraBinding@1";
pub const PROJECTION_CAMERA_BINDING_ALGORITHM_ID: &str = "forgecad.turntable_projection_camera";
pub const PROJECTION_CAMERA_BINDING_ALGORITHM_VERSION: &str = "1";
pub const PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES: u32 = 38_000;
pub const PROJECTION_CAMERA_FRAME_TARGET_NDC: f64 = 0.84;
pub const GEOMETRY_PROJECTION_CAMERA_BINDING_SCHEMA_VERSION: &str =
    "GeometryProjectionCameraBinding@1";

/// A camera derived from compiler-proven asset dimensions. The matrix uses
/// the explicit row-major world-to-clip convention consumed by
/// `ReferenceCameraUvRasterBake@2`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionCameraBinding {
    pub schema_version: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub candidate_glb_sha256: String,
    pub view_id: String,
    pub vertical_fov_millidegrees: u32,
    pub frame_target_ndc_millionths: u32,
    /// Final compiler readback dimensions in the original GLB metre space.
    pub source_bounds_meters: [f64; 3],
    pub camera_position_meters: [f64; 3],
    pub camera_target_meters: [f64; 3],
    pub near_meters: f64,
    pub far_meters: f64,
    pub world_to_clip_row_major: [f64; 16],
    pub binding_sha256: String,
}

impl ProjectionCameraBinding {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != PROJECTION_CAMERA_BINDING_SCHEMA_VERSION
            || self.algorithm_id != PROJECTION_CAMERA_BINDING_ALGORITHM_ID
            || self.algorithm_version != PROJECTION_CAMERA_BINDING_ALGORITHM_VERSION
            || !is_sha256(&self.candidate_glb_sha256)
            || !is_turntable_view(&self.view_id)
            || self.vertical_fov_millidegrees != PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES
            || self.frame_target_ndc_millionths
                != (PROJECTION_CAMERA_FRAME_TARGET_NDC * 1_000_000.0) as u32
            || !positive_finite(&self.source_bounds_meters)
            || !finite(&self.camera_position_meters)
            || !finite(&self.camera_target_meters)
            || !self.near_meters.is_finite()
            || !self.far_meters.is_finite()
            || self.near_meters <= 0.0
            || self.far_meters <= self.near_meters
            || !self
                .world_to_clip_row_major
                .iter()
                .all(|value| value.is_finite())
        {
            return Err(invalid(
                "PROJECTION_CAMERA_BINDING_INVALID",
                "Projection camera binding identity, dimensions, lens, or matrix is invalid.",
            ));
        }
        let expected = derive_projection_camera_binding(
            &self.candidate_glb_sha256,
            &self.view_id,
            self.source_bounds_meters,
        )?;
        if self != &expected {
            return Err(invalid(
                "PROJECTION_CAMERA_BINDING_DRIFT",
                "Projection camera binding does not match the Rust-owned deterministic camera derivation.",
            ));
        }
        Ok(())
    }
}

/// Derive one immutable turntable camera from an exact GLB digest and final
/// compiler bounds. Bounds are dimensions, not browser-measured boxes: the
/// workbench centres the candidate before capturing and uniform display scale
/// cancels out of the projection.
pub fn derive_projection_camera_binding(
    candidate_glb_sha256: &str,
    view_id: &str,
    source_bounds_meters: [f64; 3],
) -> CoreResult<ProjectionCameraBinding> {
    if !is_sha256(candidate_glb_sha256)
        || !is_turntable_view(view_id)
        || !positive_finite(&source_bounds_meters)
    {
        return Err(invalid(
            "PROJECTION_CAMERA_BINDING_INPUT_INVALID",
            "Projection camera derivation requires an exact GLB, frozen turntable view and finite positive source bounds.",
        ));
    }
    let target = [0.0, 0.0, 0.0];
    let direction = normalize(direction_for_view(view_id));
    let half = [
        source_bounds_meters[0] * 0.5,
        source_bounds_meters[1] * 0.5,
        source_bounds_meters[2] * 0.5,
    ];
    let corners = box_corners(half);
    let backward = direction;
    let right = normalize(cross([0.0, 1.0, 0.0], backward));
    let up = cross(backward, right);
    let fov_radians = f64::from(PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES) / 1_000.0
        * std::f64::consts::PI
        / 180.0;
    let tangent = (fov_radians * 0.5).tan();
    let mut distance = 1.0_f64;
    for corner in corners {
        let width_distance =
            dot(corner, right).abs() / (PROJECTION_CAMERA_FRAME_TARGET_NDC * tangent);
        let height_distance =
            dot(corner, up).abs() / (PROJECTION_CAMERA_FRAME_TARGET_NDC * tangent);
        distance = distance.max(dot(corner, backward) + width_distance.max(height_distance));
    }
    distance *= 1.01;
    let position = scale(backward, distance);
    let near = (distance / 1_000.0).max(0.001);
    let far = (distance * 20.0).max(100.0);
    let matrix = world_to_clip(position, target, near, far, fov_radians)?;
    let mut binding = ProjectionCameraBinding {
        schema_version: PROJECTION_CAMERA_BINDING_SCHEMA_VERSION.into(),
        algorithm_id: PROJECTION_CAMERA_BINDING_ALGORITHM_ID.into(),
        algorithm_version: PROJECTION_CAMERA_BINDING_ALGORITHM_VERSION.into(),
        candidate_glb_sha256: candidate_glb_sha256.into(),
        view_id: view_id.into(),
        vertical_fov_millidegrees: PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES,
        frame_target_ndc_millionths: (PROJECTION_CAMERA_FRAME_TARGET_NDC * 1_000_000.0) as u32,
        source_bounds_meters,
        camera_position_meters: position,
        camera_target_meters: target,
        near_meters: near,
        far_meters: far,
        world_to_clip_row_major: matrix,
        binding_sha256: String::new(),
    };
    binding.binding_sha256 = semantic_sha256(&binding_without_sha(&binding))?;
    Ok(binding)
}

/// Camera lineage for a two-stage appearance compile.  Unlike the legacy
/// candidate binding, this deliberately names the geometry invariant rather
/// than a final GLB: PBR pixels may change without moving any vertex or UV.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeometryProjectionCameraBinding {
    pub schema_version: String,
    pub geometry_binding_sha256: String,
    pub view_id: String,
    pub source_bounds_meters: [f64; 3],
    pub camera_position_meters: [f64; 3],
    pub camera_target_meters: [f64; 3],
    pub near_meters: f64,
    pub far_meters: f64,
    pub world_to_clip_row_major: [f64; 16],
    pub binding_sha256: String,
}

pub fn derive_geometry_projection_camera_binding(
    geometry: &GeometryInvariantBinding,
    view_id: &str,
) -> CoreResult<GeometryProjectionCameraBinding> {
    geometry.validate()?;
    // Reuse the exact reviewed framing mathematics. The legacy function's
    // digest is only an identity input and never reads GLB bytes; this value
    // is discarded immediately so the public binding cannot misstate it.
    let framed = derive_projection_camera_binding(
        &geometry.binding_sha256,
        view_id,
        geometry.bounds_meters,
    )?;
    let mut binding = GeometryProjectionCameraBinding {
        schema_version: GEOMETRY_PROJECTION_CAMERA_BINDING_SCHEMA_VERSION.into(),
        geometry_binding_sha256: geometry.binding_sha256.clone(),
        view_id: framed.view_id,
        source_bounds_meters: framed.source_bounds_meters,
        camera_position_meters: framed.camera_position_meters,
        camera_target_meters: framed.camera_target_meters,
        near_meters: framed.near_meters,
        far_meters: framed.far_meters,
        world_to_clip_row_major: framed.world_to_clip_row_major,
        binding_sha256: String::new(),
    };
    binding.binding_sha256 = semantic_sha256(&binding_without_geometry_sha(&binding))?;
    Ok(binding)
}

impl GeometryProjectionCameraBinding {
    pub fn validate(&self, geometry: &GeometryInvariantBinding) -> CoreResult<()> {
        let expected = derive_geometry_projection_camera_binding(geometry, &self.view_id)?;
        if self != &expected {
            return Err(invalid(
                "GEOMETRY_PROJECTION_CAMERA_BINDING_DRIFT",
                "Geometry projection camera binding does not match the exact invariant geometry facts.",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct GeometryBindingWithoutSha<'a> {
    schema_version: &'a str,
    geometry_binding_sha256: &'a str,
    view_id: &'a str,
    source_bounds_meters: [f64; 3],
    camera_position_meters: [f64; 3],
    camera_target_meters: [f64; 3],
    near_meters: f64,
    far_meters: f64,
    world_to_clip_row_major: [f64; 16],
}

fn binding_without_geometry_sha(
    binding: &GeometryProjectionCameraBinding,
) -> GeometryBindingWithoutSha<'_> {
    GeometryBindingWithoutSha {
        schema_version: &binding.schema_version,
        geometry_binding_sha256: &binding.geometry_binding_sha256,
        view_id: &binding.view_id,
        source_bounds_meters: binding.source_bounds_meters,
        camera_position_meters: binding.camera_position_meters,
        camera_target_meters: binding.camera_target_meters,
        near_meters: binding.near_meters,
        far_meters: binding.far_meters,
        world_to_clip_row_major: binding.world_to_clip_row_major,
    }
}

fn binding_without_sha(binding: &ProjectionCameraBinding) -> BindingWithoutSha<'_> {
    BindingWithoutSha {
        schema_version: &binding.schema_version,
        algorithm_id: &binding.algorithm_id,
        algorithm_version: &binding.algorithm_version,
        candidate_glb_sha256: &binding.candidate_glb_sha256,
        view_id: &binding.view_id,
        vertical_fov_millidegrees: binding.vertical_fov_millidegrees,
        frame_target_ndc_millionths: binding.frame_target_ndc_millionths,
        source_bounds_meters: binding.source_bounds_meters,
        camera_position_meters: binding.camera_position_meters,
        camera_target_meters: binding.camera_target_meters,
        near_meters: binding.near_meters,
        far_meters: binding.far_meters,
        world_to_clip_row_major: binding.world_to_clip_row_major,
    }
}

#[derive(Serialize)]
struct BindingWithoutSha<'a> {
    schema_version: &'a str,
    algorithm_id: &'a str,
    algorithm_version: &'a str,
    candidate_glb_sha256: &'a str,
    view_id: &'a str,
    vertical_fov_millidegrees: u32,
    frame_target_ndc_millionths: u32,
    source_bounds_meters: [f64; 3],
    camera_position_meters: [f64; 3],
    camera_target_meters: [f64; 3],
    near_meters: f64,
    far_meters: f64,
    world_to_clip_row_major: [f64; 16],
}

fn world_to_clip(
    position: [f64; 3],
    target: [f64; 3],
    near: f64,
    far: f64,
    fov_radians: f64,
) -> CoreResult<[f64; 16]> {
    let backward = normalize(subtract(position, target));
    let right = normalize(cross([0.0, 1.0, 0.0], backward));
    let up = cross(backward, right);
    if !finite(&right) || !finite(&up) {
        return Err(invalid(
            "PROJECTION_CAMERA_BINDING_VIEW_INVALID",
            "Projection camera direction is parallel to the fixed up vector.",
        ));
    }
    let view = [
        right[0],
        right[1],
        right[2],
        -dot(right, position),
        up[0],
        up[1],
        up[2],
        -dot(up, position),
        backward[0],
        backward[1],
        backward[2],
        -dot(backward, position),
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    let scale = 1.0 / (fov_radians * 0.5).tan();
    let depth_a = (far + near) / (near - far);
    let depth_b = (2.0 * far * near) / (near - far);
    let projection = [
        scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, depth_a, depth_b, 0.0, 0.0, -1.0, 0.0,
    ];
    Ok(multiply(projection, view))
}

fn multiply(left: [f64; 16], right: [f64; 16]) -> [f64; 16] {
    let mut result = [0.0; 16];
    for row in 0..4 {
        for column in 0..4 {
            result[row * 4 + column] = (0..4)
                .map(|inner| left[row * 4 + inner] * right[inner * 4 + column])
                .sum();
        }
    }
    result
}

fn direction_for_view(view_id: &str) -> [f64; 3] {
    match view_id {
        "turntable_000" => [0.0, 0.12, 1.0],
        "turntable_045" => [0.707, 0.18, 0.707],
        "turntable_090" => [1.0, 0.12, 0.0],
        "turntable_135" => [0.707, 0.18, -0.707],
        "turntable_180" => [0.0, 0.12, -1.0],
        "turntable_225" => [-0.707, 0.18, -0.707],
        "turntable_270" => [-1.0, 0.12, 0.0],
        "turntable_315" => [-0.707, 0.18, 0.707],
        _ => [0.0, 0.0, 0.0],
    }
}

fn box_corners(half: [f64; 3]) -> [[f64; 3]; 8] {
    [
        [-half[0], -half[1], -half[2]],
        [-half[0], -half[1], half[2]],
        [-half[0], half[1], -half[2]],
        [-half[0], half[1], half[2]],
        [half[0], -half[1], -half[2]],
        [half[0], -half[1], half[2]],
        [half[0], half[1], -half[2]],
        [half[0], half[1], half[2]],
    ]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f64; 3], scalar: f64) -> [f64; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn normalize(value: [f64; 3]) -> [f64; 3] {
    let length = dot(value, value).sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return [f64::NAN; 3];
    }
    scale(value, 1.0 / length)
}

fn positive_finite(values: &[f64; 3]) -> bool {
    values.iter().all(|value| value.is_finite() && *value > 0.0)
}

fn finite(values: &[f64; 3]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn is_turntable_view(value: &str) -> bool {
    TURN_TABLE_EIGHT_VIEW_IDS.contains(&value)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn invalid(code: &'static str, message: &'static str) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLB: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn derives_repeatable_binding_in_original_glb_space() {
        let binding = derive_projection_camera_binding(GLB, "turntable_045", [1.2, 0.8, 2.4])
            .expect("reviewed input derives one camera");
        binding.validate().expect("derived binding validates");
        assert_eq!(binding.camera_target_meters, [0.0, 0.0, 0.0]);
        assert!(binding
            .camera_position_meters
            .iter()
            .any(|value| value.abs() > 0.01));
        assert!(binding
            .world_to_clip_row_major
            .iter()
            .all(|value| value.is_finite()));
        assert_eq!(
            binding.binding_sha256,
            derive_projection_camera_binding(GLB, "turntable_045", [1.2, 0.8, 2.4])
                .expect("same input")
                .binding_sha256,
        );
    }

    #[test]
    fn rejects_mutated_matrix_and_non_turntable_view() {
        let mut binding = derive_projection_camera_binding(GLB, "turntable_000", [1.0, 1.0, 1.0])
            .expect("valid fixture");
        binding.world_to_clip_row_major[0] += 0.001;
        assert_eq!(
            binding
                .validate()
                .expect_err("matrix drift must fail")
                .code(),
            "PROJECTION_CAMERA_BINDING_DRIFT"
        );
        assert_eq!(
            derive_projection_camera_binding(GLB, "front", [1.0, 1.0, 1.0])
                .expect_err("non-evidence view must fail")
                .code(),
            "PROJECTION_CAMERA_BINDING_INPUT_INVALID"
        );
    }

    #[test]
    fn uniform_presentation_scale_does_not_change_clip_coordinates() {
        let binding = derive_projection_camera_binding(GLB, "turntable_000", [1.0, 0.5, 2.0])
            .expect("binding");
        let point = [0.25, 0.1, -0.6, 1.0];
        let clip = transform(binding.world_to_clip_row_major, point);
        let presentation_scale = 350.0;
        let scaled_position = scale(binding.camera_position_meters, presentation_scale);
        let scaled_matrix = world_to_clip(
            scaled_position,
            [0.0, 0.0, 0.0],
            binding.near_meters * presentation_scale,
            binding.far_meters * presentation_scale,
            f64::from(PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES) / 1_000.0 * std::f64::consts::PI
                / 180.0,
        )
        .expect("scaled projection");
        let scaled_clip = transform(
            scaled_matrix,
            [
                point[0] * presentation_scale,
                point[1] * presentation_scale,
                point[2] * presentation_scale,
                1.0,
            ],
        );
        assert!(clip
            .iter()
            .zip(scaled_clip.iter())
            .all(|(left, right)| { (left / clip[3] - right / scaled_clip[3]).abs() < 1e-10 }));
    }

    #[test]
    fn geometry_bound_camera_survives_appearance_changes_but_rejects_geometry_drift() {
        let geometry =
            crate::derive_geometry_invariant_binding(GLB, &"b".repeat(64), 240, [1.0, 0.5, 2.0])
                .expect("geometry invariant");
        let binding = derive_geometry_projection_camera_binding(&geometry, "turntable_000")
            .expect("geometry camera");
        binding
            .validate(&geometry)
            .expect("same geometry validates");
        let changed_appearance_glb = "f".repeat(64);
        assert_ne!(changed_appearance_glb, geometry.binding_sha256);
        binding
            .validate(&geometry)
            .expect("appearance GLB is not this lineage");
        let changed_geometry =
            crate::derive_geometry_invariant_binding(GLB, &"c".repeat(64), 240, [1.0, 0.5, 2.0])
                .expect("changed topology");
        assert_eq!(
            binding
                .validate(&changed_geometry)
                .expect_err("topology drift must fail")
                .code(),
            "GEOMETRY_PROJECTION_CAMERA_BINDING_DRIFT"
        );
    }

    fn transform(matrix: [f64; 16], point: [f64; 4]) -> [f64; 4] {
        [
            dot4(&matrix[0..4], point),
            dot4(&matrix[4..8], point),
            dot4(&matrix[8..12], point),
            dot4(&matrix[12..16], point),
        ]
    }

    fn dot4(left: &[f64], right: [f64; 4]) -> f64 {
        left.iter().zip(right).map(|(a, b)| a * b).sum()
    }
}
