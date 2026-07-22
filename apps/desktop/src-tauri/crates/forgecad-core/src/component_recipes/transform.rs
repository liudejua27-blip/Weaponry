use crate::{CoreError, CoreResult};

use super::RecipeTransform;

pub(crate) type Matrix4 = [[f64; 4]; 4];

#[allow(dead_code)]
pub(crate) fn transform_matrix(transform: &RecipeTransform) -> CoreResult<Matrix4> {
    validate_transform(transform)?;
    let [rx, ry, rz] = transform.rotation;
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();
    // Column vectors, world = parent * local, Euler XYZ (Rz * Ry * Rx).
    let rotation = [
        [cy * cz, cz * sx * sy - cx * sz, sx * sz + cx * cz * sy],
        [cy * sz, cx * cz + sx * sy * sz, cx * sy * sz - cz * sx],
        [-sy, cy * sx, cx * cy],
    ];
    let mut matrix = identity();
    for row in 0..3 {
        for column in 0..3 {
            matrix[row][column] = rotation[row][column] * transform.scale[column];
        }
        matrix[row][3] = transform.position[row];
    }
    Ok(matrix)
}

pub(crate) fn connector_frame(
    position: [f64; 3],
    normal: [f64; 3],
    up: [f64; 3],
) -> CoreResult<Matrix4> {
    let normal = normalize(normal, "COMPONENT_RECIPE_CONNECTOR_NORMAL_INVALID")?;
    let up = normalize(up, "COMPONENT_RECIPE_CONNECTOR_UP_INVALID")?;
    if dot(normal, up).abs() > 1e-6 {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_CONNECTOR_FRAME_INVALID",
            "Connector normal and up must be orthogonal unit vectors.",
        ));
    }
    let right = normalize(
        cross(up, normal),
        "COMPONENT_RECIPE_CONNECTOR_FRAME_INVALID",
    )?;
    let mut matrix = identity();
    for row in 0..3 {
        matrix[row][0] = right[row];
        matrix[row][1] = up[row];
        matrix[row][2] = normal[row];
        matrix[row][3] = finite(position[row], "COMPONENT_RECIPE_CONNECTOR_POSITION_INVALID")?;
    }
    Ok(matrix)
}

#[allow(dead_code)]
pub(crate) fn multiply(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            result[row][column] = (0..4)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    result
}

/// Return the rigid rotation carried by a world matrix.  Recipe placement is
/// deliberately stricter than general scene transforms: the geometry worker
/// receives a static mesh, so a residual scale or shear would make the saved
/// AssemblyGraph disagree with the baked GLB.
pub(crate) fn rigid_rotation(matrix: Matrix4) -> CoreResult<[[f64; 3]; 3]> {
    if matrix[3] != [0.0, 0.0, 0.0, 1.0] {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_TEMPLATE_TRANSFORM_INVALID",
            "Recipe world transforms must be affine matrices.",
        ));
    }
    let mut rotation = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            rotation[row][column] = finite(
                matrix[row][column],
                "COMPONENT_RECIPE_TEMPLATE_TRANSFORM_INVALID",
            )?;
        }
    }
    let determinant = dot(rotation[0], cross(rotation[1], rotation[2]));
    if (determinant - 1.0).abs() > 1e-8
        || (0..3).any(|row| (dot(rotation[row], rotation[row]) - 1.0).abs() > 1e-8)
        || (0..3)
            .any(|row| (row + 1..3).any(|column| dot(rotation[row], rotation[column]).abs() > 1e-8))
    {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_TEMPLATE_SCALE_UNSUPPORTED",
            "Static Recipe mesh baking accepts rigid rotation only; scale or shear is rejected.",
        ));
    }
    Ok(rotation)
}

pub(crate) fn transform_point(matrix: Matrix4, point: [f64; 3]) -> CoreResult<[f64; 3]> {
    let _ = rigid_rotation(matrix)?;
    Ok(std::array::from_fn(|row| {
        (0..3)
            .map(|column| matrix[row][column] * point[column])
            .sum::<f64>()
            + matrix[row][3]
    }))
}

pub(crate) fn euler_xyz_from_rotation(rotation: [[f64; 3]; 3]) -> [f64; 3] {
    // Inverse of transform_matrix's Rz * Ry * Rx convention.  The gimbal-lock
    // branch fixes Z to zero, which is deterministic and represents the same
    // rigid frame.
    let y = (-rotation[2][0]).clamp(-1.0, 1.0).asin();
    let cosine_y = y.cos();
    let (x, z) = if cosine_y.abs() > 1e-8 {
        (
            rotation[2][1].atan2(rotation[2][2]),
            rotation[1][0].atan2(rotation[0][0]),
        )
    } else {
        ((-rotation[1][2]).atan2(rotation[1][1]), 0.0)
    };
    [canonical_angle(x), canonical_angle(y), canonical_angle(z)]
}

pub(crate) fn rotation_matrix_from_euler(rotation: [f64; 3]) -> Matrix4 {
    transform_matrix(&RecipeTransform {
        position: [0.0; 3],
        rotation,
        scale: [1.0; 3],
    })
    .expect("finite bounded rotation supplied by Recipe expander")
}

fn canonical_angle(value: f64) -> f64 {
    if value.abs() < 1e-12 {
        0.0
    } else {
        value
    }
}

/// Inverse of a connector frame. Connector frames are validated orthonormal
/// and therefore rigid; accepting a scaled/non-rigid inverse would hide shear.
pub(crate) fn inverse_rigid(matrix: Matrix4) -> Matrix4 {
    let mut inverse = identity();
    for row in 0..3 {
        for column in 0..3 {
            inverse[row][column] = matrix[column][row];
        }
    }
    for row in 0..3 {
        inverse[row][3] = -(0..3)
            .map(|column| inverse[row][column] * matrix[column][3])
            .sum::<f64>();
    }
    inverse
}

pub(crate) fn identity() -> Matrix4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub(crate) fn validate_transform(transform: &RecipeTransform) -> CoreResult<()> {
    for position in transform.position {
        let position = finite(position, "COMPONENT_RECIPE_TRANSFORM_INVALID")?;
        if position.abs() > 100_000.0 {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_TRANSFORM_OUT_OF_RANGE",
                "Recipe position is outside the lightweight concept range.",
            ));
        }
    }
    let rotation = transform.rotation;
    for value in rotation {
        let value = finite(value, "COMPONENT_RECIPE_TRANSFORM_INVALID")?;
        if value.abs() > 36_000.0 {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_TRANSFORM_OUT_OF_RANGE",
                "Recipe rotation is outside the bounded concept range.",
            ));
        }
    }
    for value in transform.scale {
        let value = finite(value, "COMPONENT_RECIPE_TRANSFORM_INVALID")?;
        if !(0.1..=10.0).contains(&value) {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_TRANSFORM_OUT_OF_RANGE",
                "Recipe scale must be finite, positive and bounded.",
            ));
        }
    }
    let non_uniform = (transform.scale[0] - transform.scale[1]).abs() > 1e-9
        || (transform.scale[1] - transform.scale[2]).abs() > 1e-9;
    let rotated = transform.rotation.iter().any(|value| value.abs() > 1e-9);
    if non_uniform && rotated {
        return Err(CoreError::invalid_data(
            "COMPONENT_RECIPE_SHEAR_UNSUPPORTED",
            "C105 v1 rejects non-uniform scale combined with rotation because it can produce shear.",
        ));
    }
    Ok(())
}

fn finite(value: f64, code: &'static str) -> CoreResult<f64> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| CoreError::invalid_data(code, "Recipe numeric values must be finite."))
}

fn normalize(value: [f64; 3], code: &'static str) -> CoreResult<[f64; 3]> {
    let magnitude = value.iter().try_fold(0.0, |sum, item| {
        let item = finite(*item, code)?;
        Ok::<_, CoreError>(sum + item * item)
    })?;
    let magnitude = magnitude.sqrt();
    if !magnitude.is_finite() || (magnitude - 1.0).abs() > 1e-6 {
        return Err(CoreError::invalid_data(
            code,
            "Connector vectors must be finite and already unit length.",
        ));
    }
    Ok(value)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_recipe_transform() {
        let transform = RecipeTransform {
            position: [f64::NAN, 0.0, 0.0],
            ..RecipeTransform::default()
        };
        assert_eq!(
            validate_transform(&transform).unwrap_err().code(),
            "COMPONENT_RECIPE_TRANSFORM_INVALID"
        );
    }
}
