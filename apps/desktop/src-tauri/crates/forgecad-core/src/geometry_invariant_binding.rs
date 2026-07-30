//! Geometry-only lineage for two-stage appearance compilation.
//!
//! A final GLB hash changes when PBR pixels change.  Camera projection must
//! instead bind the immutable geometry facts that the restricted compiler
//! read back before appearance baking.

use serde::{Deserialize, Serialize};

use crate::{semantic_sha256, CoreError, CoreResult};

pub const GEOMETRY_INVARIANT_BINDING_SCHEMA_VERSION: &str = "GeometryInvariantBinding@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GeometryInvariantBinding {
    pub schema_version: String,
    pub shape_program_sha256: String,
    pub topology_hash: String,
    pub triangle_count: u32,
    /// Raw compiler-space dimensions in metres, before any browser display scale.
    pub bounds_meters: [f64; 3],
    pub binding_sha256: String,
}

pub fn derive_geometry_invariant_binding(
    shape_program_sha256: &str,
    topology_hash: &str,
    triangle_count: u32,
    bounds_meters: [f64; 3],
) -> CoreResult<GeometryInvariantBinding> {
    if !sha256(shape_program_sha256)
        || !sha256(topology_hash)
        || triangle_count == 0
        || !bounds_meters
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
    {
        return Err(CoreError::invalid_data(
            "GEOMETRY_INVARIANT_BINDING_INPUT_INVALID",
            "Geometry invariant binding requires exact geometry hashes, non-zero triangles and positive finite metre bounds.",
        ));
    }
    let mut binding = GeometryInvariantBinding {
        schema_version: GEOMETRY_INVARIANT_BINDING_SCHEMA_VERSION.into(),
        shape_program_sha256: shape_program_sha256.into(),
        topology_hash: topology_hash.into(),
        triangle_count,
        bounds_meters,
        binding_sha256: String::new(),
    };
    binding.binding_sha256 = semantic_sha256(&BindingWithoutSha::from(&binding))?;
    Ok(binding)
}

impl GeometryInvariantBinding {
    pub fn validate(&self) -> CoreResult<()> {
        let expected = derive_geometry_invariant_binding(
            &self.shape_program_sha256,
            &self.topology_hash,
            self.triangle_count,
            self.bounds_meters,
        )?;
        if self != &expected {
            return Err(CoreError::conflict(
                "GEOMETRY_INVARIANT_BINDING_DRIFT",
                "Geometry invariant binding does not match the exact compiler geometry facts.",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct BindingWithoutSha<'a> {
    schema_version: &'a str,
    shape_program_sha256: &'a str,
    topology_hash: &'a str,
    triangle_count: u32,
    bounds_meters: [f64; 3],
}

impl<'a> From<&'a GeometryInvariantBinding> for BindingWithoutSha<'a> {
    fn from(value: &'a GeometryInvariantBinding) -> Self {
        Self {
            schema_version: &value.schema_version,
            shape_program_sha256: &value.shape_program_sha256,
            topology_hash: &value.topology_hash,
            triangle_count: value.triangle_count,
            bounds_meters: value.bounds_meters,
        }
    }
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_repeatable_and_rejects_geometry_drift() {
        let first = derive_geometry_invariant_binding(
            &"a".repeat(64),
            &"b".repeat(64),
            42,
            [1.0, 2.0, 3.0],
        )
        .unwrap();
        assert_eq!(
            first,
            derive_geometry_invariant_binding(
                &"a".repeat(64),
                &"b".repeat(64),
                42,
                [1.0, 2.0, 3.0]
            )
            .unwrap()
        );
        first.validate().unwrap();
        let mut drifted = first;
        drifted.bounds_meters[0] = 1.1;
        assert_eq!(
            drifted.validate().unwrap_err().code(),
            "GEOMETRY_INVARIANT_BINDING_DRIFT"
        );
    }
}
