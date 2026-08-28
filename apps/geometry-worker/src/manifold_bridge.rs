//! Rust-side ownership and validation for the product-owned Manifold bridge.
//!
//! The C++ layer never receives JSON or paths.  It receives only the already
//! validated typed mesh buffers from the GeometryProgram compiler and returns
//! a copied result.  This module turns that result back into the worker's
//! bounded `PrimitiveNodeMesh` representation and performs the final finite,
//! index, normal, and lineage-array checks.

use super::{cross3, finite3, normalize, subtract3, GeometryError, PrimitiveNodeMesh};
use std::collections::BTreeMap;
use std::os::raw::{c_int, c_void};

const BOOLEAN_MAX_VERTICES: usize = 750_000;

/// A copied, product-owned Boolean result for sibling evaluators.
///
/// The C ABI remains private to this crate.  This value intentionally carries
/// no Manifold handle and no borrowed pointer, so a caller cannot retain or
/// mutate third-party state after the bridge call returns.
#[derive(Debug, Clone)]
pub struct ManifoldBooleanOutput {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
    pub source_ids: Vec<u32>,
    pub face_ids: Vec<u64>,
    pub volume: f64,
    pub surface_area: f64,
    pub genus: i32,
}

/// Evaluate one bounded, typed Boolean through the accepted vendored Manifold
/// C ABI.  The caller supplies only finite indexed triangle buffers; no JSON,
/// path, callback, environment, or Runtime/CAS handle crosses this seam.
///
/// This helper is intentionally not used by the ordinary GeometryProgram
/// compiler path.  It exists so a sibling High evaluator can opt into the
/// same fixed C ABI behind an explicit Cargo feature while preserving the
/// existing Worker boundary and fail-closed behavior.
pub fn manifold_boolean_typed(
    left_positions: &[[f32; 3]],
    left_indices: &[[u32; 3]],
    right_positions: &[[f32; 3]],
    right_indices: &[[u32; 3]],
    operation: &str,
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<ManifoldBooleanOutput, String> {
    let left = PrimitiveNodeMesh {
        operator_id: "forgecad.module.manifold.left@1".to_owned(),
        lineage_source_node_ids: Vec::new(),
        positions: left_positions.to_vec(),
        normals: Vec::new(),
        indices: left_indices
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .collect(),
    };
    let right = PrimitiveNodeMesh {
        operator_id: "forgecad.module.manifold.right@1".to_owned(),
        lineage_source_node_ids: Vec::new(),
        positions: right_positions.to_vec(),
        normals: Vec::new(),
        indices: right_indices
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .collect(),
    };
    let result = execute_boolean(&left, &right, operation, max_triangles, max_runtime_ms)
        .map_err(|error| error.to_string())?;
    let indices = result
        .indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();
    if indices.len() != result.source_ids.len() || indices.len() != result.face_ids.len() {
        return Err("MANIFOLD_TYPED_RESULT_LINEAGE_LENGTH_MISMATCH".to_owned());
    }
    Ok(ManifoldBooleanOutput {
        positions: result.positions,
        indices,
        source_ids: result.source_ids,
        face_ids: result.face_ids,
        volume: result.volume,
        surface_area: result.surface_area,
        genus: result.genus,
    })
}

#[repr(C)]
struct ForgeCADBooleanOutputV1 {
    status: i32,
    manifold_error: i32,
    num_vertices: usize,
    num_triangles: usize,
    volume: f64,
    surface_area: f64,
    genus: i32,
    positions: *mut f64,
    indices: *mut u64,
    source_ids: *mut u32,
    face_ids: *mut u64,
}

extern "C" {
    fn forgecad_manifold_boolean_v1(
        operation: c_int,
        left_positions: *const f64,
        left_vertices: usize,
        left_indices: *const u64,
        left_triangles: usize,
        right_positions: *const f64,
        right_vertices: usize,
        right_indices: *const u64,
        right_triangles: usize,
        max_vertices: usize,
        max_triangles: usize,
        max_runtime_ms: u64,
        output: *mut ForgeCADBooleanOutputV1,
    ) -> c_int;
    fn forgecad_manifold_boolean_free_v1(output: *mut ForgeCADBooleanOutputV1);
}

#[derive(Debug, Clone)]
pub(super) struct BooleanMesh {
    pub(super) positions: Vec<[f32; 3]>,
    pub(super) indices: Vec<u32>,
    pub(super) normals: Vec<[f32; 3]>,
    pub(super) source_ids: Vec<u32>,
    pub(super) face_ids: Vec<u64>,
    pub(super) volume: f64,
    pub(super) surface_area: f64,
    pub(super) genus: i32,
}

pub(super) fn execute_boolean(
    left: &PrimitiveNodeMesh,
    right: &PrimitiveNodeMesh,
    operation: &str,
    max_triangles: u64,
    max_runtime_ms: u64,
) -> Result<BooleanMesh, GeometryError> {
    let operation_code = match operation {
        "union" => 0,
        "difference" => 1,
        "intersection" => 2,
        _ => {
            return Err(GeometryError::Invalid(
                "boolean operation is not active".to_owned(),
            ))
        }
    };
    let left = weld_mesh(left, "left")?;
    let right = weld_mesh(right, "right")?;
    let max_triangles = usize::try_from(max_triangles).map_err(|_| {
        GeometryError::Invalid("boolean triangle budget is not representable".to_owned())
    })?;
    let max_vertices = max_triangles
        .saturating_mul(3)
        .min(BOOLEAN_MAX_VERTICES)
        .max(3);
    if left.positions.len() > max_vertices || right.positions.len() > max_vertices {
        return Err(GeometryError::Invalid(
            "boolean input exceeds the bounded vertex budget".to_owned(),
        ));
    }

    let left_positions = flatten_positions(&left.positions);
    let right_positions = flatten_positions(&right.positions);
    let left_indices = left
        .indices
        .iter()
        .map(|index| *index as u64)
        .collect::<Vec<_>>();
    let right_indices = right
        .indices
        .iter()
        .map(|index| *index as u64)
        .collect::<Vec<_>>();
    let mut output = ForgeCADBooleanOutputV1 {
        status: 5,
        manifold_error: 0,
        num_vertices: 0,
        num_triangles: 0,
        volume: 0.0,
        surface_area: 0.0,
        genus: 0,
        positions: std::ptr::null_mut(),
        indices: std::ptr::null_mut(),
        source_ids: std::ptr::null_mut(),
        face_ids: std::ptr::null_mut(),
    };
    let status = unsafe {
        forgecad_manifold_boolean_v1(
            operation_code,
            left_positions.as_ptr(),
            left.positions.len(),
            left_indices.as_ptr(),
            left.indices.len() / 3,
            right_positions.as_ptr(),
            right.positions.len(),
            right_indices.as_ptr(),
            right.indices.len() / 3,
            max_vertices,
            max_triangles,
            max_runtime_ms,
            &mut output,
        )
    };
    if status != 0 || output.status != 0 {
        let error = output.manifold_error;
        unsafe { forgecad_manifold_boolean_free_v1(&mut output) };
        let message = match status.max(output.status) {
            1 => "boolean bridge rejected its typed input",
            2 => "Manifold rejected the Boolean input or result",
            3 => "Boolean result exceeded the declared budget",
            4 => "Boolean exceeded the declared runtime budget",
            _ => "Boolean bridge failed internally",
        };
        return Err(GeometryError::Invalid(format!(
            "{message} (status={status}, manifold_error={error})"
        )));
    }

    let result = (|| {
        if output.num_vertices < 3
            || output.num_vertices > max_vertices
            || output.num_triangles < 1
            || output.num_triangles > max_triangles
            || output.positions.is_null()
            || output.indices.is_null()
            || output.source_ids.is_null()
            || output.face_ids.is_null()
            || !output.volume.is_finite()
            || !output.surface_area.is_finite()
            || output.volume <= 0.0
        {
            return Err(GeometryError::Invalid(
                "Boolean result failed the strict bounded readback".to_owned(),
            ));
        }
        let positions = unsafe {
            std::slice::from_raw_parts(output.positions, output.num_vertices * 3)
                .chunks_exact(3)
                .map(|point| [point[0] as f32, point[1] as f32, point[2] as f32])
                .collect::<Vec<_>>()
        };
        if positions.iter().any(|point| !finite3(*point)) {
            return Err(GeometryError::Invalid(
                "Boolean result contains non-finite vertices".to_owned(),
            ));
        }
        let indices = unsafe {
            std::slice::from_raw_parts(output.indices, output.num_triangles * 3)
                .iter()
                .map(|index| {
                    u32::try_from(*index).map_err(|_| {
                        GeometryError::Invalid(
                            "Boolean result index does not fit the worker mesh".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if indices
            .iter()
            .any(|index| *index as usize >= positions.len())
        {
            return Err(GeometryError::Invalid(
                "Boolean result has an out-of-bounds index".to_owned(),
            ));
        }
        let source_ids =
            unsafe { std::slice::from_raw_parts(output.source_ids, output.num_triangles).to_vec() };
        if source_ids.iter().any(|source| *source > 1) {
            return Err(GeometryError::Invalid(
                "Boolean result lost operand source lineage".to_owned(),
            ));
        }
        let face_ids =
            unsafe { std::slice::from_raw_parts(output.face_ids, output.num_triangles).to_vec() };
        if face_ids.len() != output.num_triangles {
            return Err(GeometryError::Invalid(
                "Boolean result face lineage length is invalid".to_owned(),
            ));
        }
        let normals = compute_normals(&positions, &indices)?;
        Ok(BooleanMesh {
            positions,
            indices,
            normals,
            source_ids,
            face_ids,
            volume: output.volume,
            surface_area: output.surface_area,
            genus: output.genus,
        })
    })();
    unsafe { forgecad_manifold_boolean_free_v1(&mut output) };
    result
}

#[derive(Debug, Clone)]
struct WeldedMesh {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn weld_mesh(mesh: &PrimitiveNodeMesh, label: &str) -> Result<WeldedMesh, GeometryError> {
    if mesh.positions.len() < 3
        || mesh.indices.is_empty()
        || mesh.indices.len() % 3 != 0
        || mesh.positions.iter().any(|point| !finite3(*point))
        || mesh
            .indices
            .iter()
            .any(|index| *index as usize >= mesh.positions.len())
    {
        return Err(GeometryError::Invalid(format!(
            "boolean {label} input is not a finite indexed triangle mesh"
        )));
    }
    let mut unique = BTreeMap::<(u32, u32, u32), u32>::new();
    let mut positions = Vec::new();
    let mut remap = Vec::with_capacity(mesh.positions.len());
    for point in &mesh.positions {
        let key = (point[0].to_bits(), point[1].to_bits(), point[2].to_bits());
        let index = if let Some(index) = unique.get(&key) {
            *index
        } else {
            let index = u32::try_from(positions.len()).map_err(|_| {
                GeometryError::Invalid("boolean welded vertex index overflow".to_owned())
            })?;
            unique.insert(key, index);
            positions.push(*point);
            index
        };
        remap.push(index);
    }
    let indices = mesh
        .indices
        .iter()
        .map(|index| remap[*index as usize])
        .collect::<Vec<_>>();
    if positions.len() < 3
        || indices
            .iter()
            .any(|index| *index as usize >= positions.len())
    {
        return Err(GeometryError::Invalid(
            "boolean welded mesh has invalid topology".to_owned(),
        ));
    }
    Ok(WeldedMesh { positions, indices })
}

fn flatten_positions(positions: &[[f32; 3]]) -> Vec<f64> {
    positions
        .iter()
        .flat_map(|point| point.iter().copied().map(f64::from))
        .collect()
}

fn compute_normals(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let mut normals = vec![[0.0; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let face = normalize(cross3(
            subtract3(positions[b], positions[a]),
            subtract3(positions[c], positions[a]),
        ));
        if !finite3(face) || face == [0.0; 3] {
            return Err(GeometryError::Invalid(
                "Boolean result contains a degenerate triangle".to_owned(),
            ));
        }
        for index in [a, b, c] {
            normals[index][0] += face[0];
            normals[index][1] += face[1];
            normals[index][2] += face[2];
        }
    }
    for normal in &mut normals {
        *normal = normalize(*normal);
        if !finite3(*normal) || *normal == [0.0; 3] {
            return Err(GeometryError::Invalid(
                "Boolean result has an invalid vertex normal".to_owned(),
            ));
        }
    }
    Ok(normals)
}

#[allow(dead_code)]
fn _opaque_ffi_type_is_c_compatible(_: *mut c_void) {}
