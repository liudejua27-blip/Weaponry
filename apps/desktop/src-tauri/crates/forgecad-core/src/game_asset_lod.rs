use meshopt::{simplify_with_attributes_and_locks_decoder, SimplifyOptions};

use crate::{CoreError, CoreResult};

/// The maximum normalized geometric error permitted for the deterministic
/// game-asset LOD simplifier. A delivery compiler must reject a tier rather
/// than silently accept a looser approximation.
pub const GAME_ASSET_LOD_TARGET_ERROR: f32 = 0.02;

/// Minimal surface data required to keep simplification aware of visible
/// normal and UV discontinuities. This is an internal compiler value, never a
/// Provider-authored mesh format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameAssetLodVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv0: [f32; 2],
}

/// A simplified index buffer still referencing the immutable source vertices.
/// The later GLB delivery writer owns buffer/accessor construction, while this
/// Core operator owns deterministic topology reduction and its error bound.
#[derive(Debug, Clone, PartialEq)]
pub struct GameAssetLodMesh {
    pub indices: Vec<u32>,
    pub triangle_count: u32,
    pub simplification_error: f32,
}

/// Builds one bounded local LOD from a triangle list. The routine intentionally
/// does not compact or mutate vertices: all PBR attributes remain in the same
/// source buffer, making it safe for a later GLB writer to preserve material
/// zones and ForgeCAD face provenance.
pub fn simplify_game_asset_lod(
    vertices: &[GameAssetLodVertex],
    indices: &[u32],
    target_triangle_count: u32,
) -> CoreResult<GameAssetLodMesh> {
    simplify_game_asset_lod_with_error_limit(
        vertices,
        indices,
        target_triangle_count,
        GAME_ASSET_LOD_TARGET_ERROR,
        SimplifyOptions::Permissive,
        1.0,
    )
}

/// Like [`simplify_game_asset_lod`], but measures the 2% error gate against
/// the enclosing asset extent instead of each independently material-bound
/// primitive.  This prevents tiny screws and panel inserts from receiving a
/// physically microscopic error allowance while retaining the same normalized
/// quality contract in the resulting readback.
pub fn simplify_game_asset_lod_with_global_error(
    vertices: &[GameAssetLodVertex],
    indices: &[u32],
    target_triangle_count: u32,
    global_extent: f32,
) -> CoreResult<GameAssetLodMesh> {
    if !global_extent.is_finite() || global_extent <= f32::EPSILON {
        return Err(CoreError::invalid_data(
            "GAME_ASSET_LOD_INPUT_INVALID",
            "Game asset LOD simplification requires a finite positive asset extent.",
        ));
    }
    simplify_game_asset_lod_with_error_limit(
        vertices,
        indices,
        target_triangle_count,
        GAME_ASSET_LOD_TARGET_ERROR * global_extent,
        SimplifyOptions::Permissive | SimplifyOptions::ErrorAbsolute,
        global_extent,
    )
}

fn simplify_game_asset_lod_with_error_limit(
    vertices: &[GameAssetLodVertex],
    indices: &[u32],
    target_triangle_count: u32,
    error_limit: f32,
    options: SimplifyOptions,
    error_normalizer: f32,
) -> CoreResult<GameAssetLodMesh> {
    if vertices.len() < 3
        || indices.is_empty()
        || indices.len() % 3 != 0
        || target_triangle_count == 0
        || vertices.iter().any(|vertex| {
            vertex
                .position
                .iter()
                .chain(vertex.normal.iter())
                .chain(vertex.uv0.iter())
                .any(|value| !value.is_finite())
        })
        || indices
            .iter()
            .any(|index| *index as usize >= vertices.len())
    {
        return Err(CoreError::invalid_data(
            "GAME_ASSET_LOD_INPUT_INVALID",
            "Game asset LOD simplification requires finite indexed triangle surface data.",
        ));
    }
    let source_triangle_count = (indices.len() / 3) as u32;
    if target_triangle_count >= source_triangle_count {
        return Ok(GameAssetLodMesh {
            indices: indices.to_vec(),
            triangle_count: source_triangle_count,
            simplification_error: 0.0,
        });
    }

    let mut attributes = Vec::with_capacity(vertices.len() * 5);
    for vertex in vertices {
        attributes.extend_from_slice(&vertex.normal);
        attributes.extend_from_slice(&vertex.uv0);
    }
    let target_index_count = target_triangle_count as usize * 3;
    let mut reported_error = 0.0;
    let simplified = simplify_with_attributes_and_locks_decoder(
        indices,
        &vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>(),
        &attributes,
        &[0.5, 0.5, 0.5, 0.05, 0.05],
        std::mem::size_of::<[f32; 5]>(),
        &vec![false; vertices.len()],
        target_index_count,
        error_limit,
        // ForgeCAD's production GLB readback already rejects non-manifold
        // primitive surfaces.  `Permissive` therefore permits a bounded
        // collapse across split normal/UV seams while the weighted attributes
        // and geometric error limit keep visible shading and UV distortion in
        // the compiler objective. Locking every split seam made real dense
        // hard-surface production meshes unsimplifiable.
        options,
        Some(&mut reported_error),
    );
    if simplified.is_empty()
        || simplified.len() % 3 != 0
        || simplified.len() > target_index_count
        || simplified
            .iter()
            .any(|index| *index as usize >= vertices.len())
        || !reported_error.is_finite()
        || reported_error > error_limit
    {
        return Err(CoreError::invalid_data(
            "GAME_ASSET_LOD_SIMPLIFICATION_FAILED",
            "Local game asset simplification could not meet its triangle or error budget.",
        ));
    }
    Ok(GameAssetLodMesh {
        triangle_count: (simplified.len() / 3) as u32,
        indices: simplified,
        simplification_error: reported_error / error_normalizer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planar_grid(resolution: u32) -> (Vec<GameAssetLodVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        for y in 0..=resolution {
            for x in 0..=resolution {
                vertices.push(GameAssetLodVertex {
                    position: [
                        x as f32 / resolution as f32,
                        y as f32 / resolution as f32,
                        0.0,
                    ],
                    normal: [0.0, 0.0, 1.0],
                    uv0: [x as f32 / resolution as f32, y as f32 / resolution as f32],
                });
            }
        }
        let mut indices = Vec::new();
        for y in 0..resolution {
            for x in 0..resolution {
                let stride = resolution + 1;
                let a = y * stride + x;
                let b = a + 1;
                let c = a + stride;
                let d = c + 1;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        (vertices, indices)
    }

    #[test]
    fn lod_simplification_is_bounded_and_deterministic() {
        let (vertices, indices) = planar_grid(20);
        let first = simplify_game_asset_lod(&vertices, &indices, 160).unwrap();
        let second = simplify_game_asset_lod(&vertices, &indices, 160).unwrap();
        assert_eq!(first, second);
        assert!(first.triangle_count <= 160);
        assert!(first.triangle_count < (indices.len() / 3) as u32);
        assert!(first.simplification_error <= GAME_ASSET_LOD_TARGET_ERROR);
    }

    #[test]
    fn lod_simplification_rejects_invalid_triangles() {
        let (vertices, _) = planar_grid(2);
        assert_eq!(
            simplify_game_asset_lod(&vertices, &[0, 1, 99], 1)
                .unwrap_err()
                .code(),
            "GAME_ASSET_LOD_INPUT_INVALID"
        );
    }
}
