//! Deterministic identity primitives for a future durable AuthoringMesh
//! revision ledger.
//!
//! Original IDs deliberately exclude candidate, program, evaluated artifact
//! and readback hashes. Those hashes bind a revision's evidence, but they are
//! not the identity root. Evaluated geometry never enters these preimages.

use super::{canonical_json_hash, is_opaque_id, is_sha256, RuntimeError};
use serde_json::json;
use std::collections::BTreeSet;

pub(super) const IDENTITY_LINEAGE_SCHEMA_VERSION: &str = "AuthoringMeshIdentityLineage@1";
const MAX_PARENT_IDS: usize = 8;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_IDENTITY_INVALID: {}",
        message.into()
    ))
}

fn checked_identifier<'a>(value: &'a str, field: &str) -> Result<&'a str, RuntimeError> {
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an identifier")));
    }
    Ok(value)
}

fn checked_sha<'a>(value: &'a str, field: &str) -> Result<&'a str, RuntimeError> {
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn opaque_id(prefix: &str, preimage: serde_json::Value) -> String {
    let hash = canonical_json_hash(&preimage);
    format!("{prefix}-{}", &hash[..56])
}

pub(super) fn lineage_id(
    project_id: &str,
    authoring_node_id: &str,
    part_id: &str,
    genesis_source_mesh_sha256: &str,
) -> Result<String, RuntimeError> {
    checked_identifier(project_id, "project_id")?;
    checked_identifier(authoring_node_id, "authoring_node_id")?;
    checked_identifier(part_id, "part_id")?;
    checked_sha(genesis_source_mesh_sha256, "genesis_source_mesh_sha256")?;
    Ok(opaque_id(
        "amlineage",
        json!({
            "schema_version": IDENTITY_LINEAGE_SCHEMA_VERSION,
            "project_id": project_id,
            "authoring_node_id": authoring_node_id,
            "part_id": part_id,
            "genesis_source_mesh_sha256": genesis_source_mesh_sha256,
            "identity_policy": "runtime-owned-authored-ids-with-monotonic-tombstones@1"
        }),
    ))
}

pub(super) fn authored_identity_id(
    lineage_id: &str,
    element_kind: &str,
    source_element_id: &str,
) -> Result<String, RuntimeError> {
    checked_identifier(lineage_id, "lineage_id")?;
    checked_identifier(source_element_id, "source_element_id")?;
    if !matches!(element_kind, "vertex" | "edge" | "face" | "loop") {
        return Err(invalid(
            "authored stable identity kind must be vertex, edge, face or loop",
        ));
    }
    Ok(opaque_id(
        match element_kind {
            "vertex" => "v",
            "edge" => "e",
            "face" => "f",
            _ => "loop",
        },
        json!({
            "schema_version": IDENTITY_LINEAGE_SCHEMA_VERSION,
            "lineage_id": lineage_id,
            "element_kind": element_kind,
            "source_element_id": source_element_id,
            "origin": "authored"
        }),
    ))
}

pub(super) fn generated_identity_id(
    lineage_id: &str,
    element_kind: &str,
    operation_lineage_sha256: &str,
    parent_identity_ids: &[String],
    role: &str,
    ordinal: usize,
) -> Result<String, RuntimeError> {
    checked_identifier(lineage_id, "lineage_id")?;
    checked_identifier(role, "role")?;
    checked_sha(operation_lineage_sha256, "operation_lineage_sha256")?;
    if !matches!(
        element_kind,
        "vertex" | "edge" | "half-edge" | "corner" | "face" | "loop" | "ring" | "boundary"
    ) {
        return Err(invalid("element_kind is unsupported"));
    }
    if parent_identity_ids.len() > MAX_PARENT_IDS || ordinal > 32767 {
        return Err(invalid("generated identity budget exceeded"));
    }
    let mut parents = parent_identity_ids.to_vec();
    parents.sort();
    parents.dedup();
    if parents.len() != parent_identity_ids.len()
        || parents.iter().any(|parent| !is_opaque_id(parent))
    {
        return Err(invalid(
            "parent_identity_ids must be unique valid identifiers",
        ));
    }
    Ok(opaque_id(
        "amop",
        json!({
            "schema_version": IDENTITY_LINEAGE_SCHEMA_VERSION,
            "lineage_id": lineage_id,
            "element_kind": element_kind,
            "operation_lineage_sha256": operation_lineage_sha256,
            "parent_identity_ids": parents,
            "role": role,
            "ordinal": ordinal
        }),
    ))
}

pub(super) fn validate_active_and_tombstone_ids(
    active_identity_ids: &[String],
    tombstone_identity_ids: &[String],
) -> Result<(), RuntimeError> {
    if active_identity_ids.len() > 32768 || tombstone_identity_ids.len() > 32768 {
        return Err(invalid("identity or tombstone budget exceeded"));
    }
    let active = active_identity_ids.iter().collect::<BTreeSet<_>>();
    let tombstones = tombstone_identity_ids.iter().collect::<BTreeSet<_>>();
    if active.len() != active_identity_ids.len()
        || tombstones.len() != tombstone_identity_ids.len()
        || active_identity_ids.iter().any(|value| !is_opaque_id(value))
        || tombstone_identity_ids
            .iter()
            .any(|value| !is_opaque_id(value))
    {
        return Err(invalid("identity IDs must be unique valid identifiers"));
    }
    if active
        .iter()
        .any(|identity_id| tombstones.contains(identity_id))
    {
        return Err(invalid("a tombstoned identity cannot be reused"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_ids_ignore_candidate_program_and_evaluated_artifact_revisions() {
        let genesis_mesh_sha256 = "a".repeat(64);
        let lineage = lineage_id(
            "project-authoring",
            "receiver-authoring",
            "receiver-shell",
            &genesis_mesh_sha256,
        )
        .expect("lineage");
        let revision_a = json!({
            "candidate_id":"candidate-a",
            "program_sha256":"b".repeat(64),
            "artifact_sha256":"c".repeat(64)
        });
        let revision_b = json!({
            "candidate_id":"candidate-b",
            "program_sha256":"d".repeat(64),
            "artifact_sha256":"e".repeat(64)
        });
        assert_ne!(revision_a, revision_b);
        for (kind, source_id) in [
            ("vertex", "v-receiver-001"),
            ("edge", "e-receiver-001"),
            ("face", "f-receiver-001"),
            ("loop", "l-receiver-001"),
        ] {
            let first = authored_identity_id(&lineage, kind, source_id).expect("first ID");
            let second = authored_identity_id(&lineage, kind, source_id).expect("second ID");
            assert_eq!(first, second);
            assert!(is_opaque_id(&first));
        }
    }

    #[test]
    fn operation_ids_are_parent_order_independent_and_tombstones_are_monotonic() {
        let lineage = lineage_id(
            "project-authoring",
            "receiver-authoring",
            "receiver-shell",
            &"a".repeat(64),
        )
        .expect("lineage");
        let left = authored_identity_id(&lineage, "edge", "edge-left").expect("left");
        let right = authored_identity_id(&lineage, "edge", "edge-right").expect("right");
        let operation = "b".repeat(64);
        let forward = generated_identity_id(
            &lineage,
            "vertex",
            &operation,
            &[left.clone(), right.clone()],
            "split-midpoint",
            0,
        )
        .expect("forward");
        let reversed = generated_identity_id(
            &lineage,
            "vertex",
            &operation,
            &[right.clone(), left.clone()],
            "split-midpoint",
            0,
        )
        .expect("reversed");
        assert_eq!(forward, reversed);
        validate_active_and_tombstone_ids(std::slice::from_ref(&forward), &[left.clone()])
            .expect("disjoint active and tombstone sets");
        assert!(validate_active_and_tombstone_ids(&[left.clone()], &[left]).is_err());
        assert!(generated_identity_id(
            &lineage,
            "vertex",
            &operation,
            &[right.clone(), right],
            "split-midpoint",
            0
        )
        .is_err());
    }
}
