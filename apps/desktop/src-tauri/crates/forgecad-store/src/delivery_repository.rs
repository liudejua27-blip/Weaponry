//! Physical Store boundary for the game-delivery aggregate.
//!
//! DeliveryRepository borrows an existing Store. It does not own a SQLite
//! connection, migrations, or a CAS root. The first extracted slice is the
//! immutable GameAssetDeliveryLinkRecord: its record/get/list methods perform
//! the same candidate binding, CAS validation, idempotent replay and
//! reachability updates that the compatibility Store methods historically
//! performed.
//!
//! The table bootstrap remains in Store::migrate, which is the sole migration
//! owner. Other delivery families (approval/version/export and socket
//! sidecars) remain compatibility implementations until their own extraction
//! atoms; keeping those shims explicit avoids implying that this small
//! repository has already become the complete Delivery domain.

use forgecad_contracts::{is_opaque_id, is_sha256, GameAssetDeliveryLinkRecord};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;

use super::{
    canonical_json_hash, mark_reachable_in_transaction, Store, StoreError,
    GAME_ASSET_DELIVERY_JSON_MIME, MAX_GAME_ASSET_DELIVERY_JSON_BYTES,
};

/// Borrowed repository for the immutable game-delivery link aggregate.
///
/// The lifetime ties this view to the owning Store. Constructing it is
/// side-effect free and cannot create a second database, migration sequence,
/// or CAS root.
#[derive(Clone, Copy)]
pub struct DeliveryRepository<'store> {
    store: &'store Store,
}

/// Compatibility name for callers that identify this first slice by its
/// aggregate rather than by the Store domain.
pub type GameDeliveryRepository<'store> = DeliveryRepository<'store>;

const GAME_DELIVERY_LINK_SELECT: &str = "SELECT project_id, lod0_candidate_id, lod1_candidate_id, lod2_candidate_id, lod0_artifact_sha256, lod1_artifact_sha256, lod2_artifact_sha256, request_sha256, lod_receipt_object_sha256, collision_proxy_object_sha256, readiness_object_sha256, delivery_manifest_object_sha256, animation_artifact_sha256, materialization_status, canonical_sha256, created_at FROM game_asset_delivery_links";

fn validate_game_asset_delivery_link(link: &GameAssetDeliveryLinkRecord) -> Result<(), StoreError> {
    let mut canonical = link.clone();
    canonical.canonical_sha256.clear();
    let expected_canonical = canonical_json_hash(
        &serde_json::to_value(&canonical)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
    );
    if link.schema_version != "GameAssetDeliveryLink@1"
        || !is_opaque_id(&link.project_id)
        || link.lod_candidate_ids.len() != 3
        || link
            .lod_candidate_ids
            .iter()
            .any(|value| !is_opaque_id(value))
        || link.lod_candidate_ids.iter().collect::<HashSet<_>>().len() != 3
        || link.lod_artifact_sha256s.len() != 3
        || link
            .lod_artifact_sha256s
            .iter()
            .any(|value| !is_sha256(value))
        || link
            .lod_artifact_sha256s
            .iter()
            .collect::<HashSet<_>>()
            .len()
            != 3
        || !is_sha256(&link.request_sha256)
        || !is_sha256(&link.lod_receipt_object_sha256)
        || !is_sha256(&link.collision_proxy_object_sha256)
        || !is_sha256(&link.readiness_object_sha256)
        || !is_sha256(&link.delivery_manifest_object_sha256)
        || link
            .animation_artifact_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || link.materialization_status != "runtime-owned-durable-game-delivery-link"
        || link.canonical_sha256 != expected_canonical
        || link.created_at.is_empty()
        || link.created_at.len() > 128
    {
        return Err(StoreError::InvalidData(
            "game asset delivery link is malformed".to_owned(),
        ));
    }
    Ok(())
}

fn game_asset_delivery_link_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<GameAssetDeliveryLinkRecord> {
    Ok(GameAssetDeliveryLinkRecord {
        schema_version: "GameAssetDeliveryLink@1".to_owned(),
        project_id: row.get(0)?,
        lod_candidate_ids: vec![row.get(1)?, row.get(2)?, row.get(3)?],
        lod_artifact_sha256s: vec![row.get(4)?, row.get(5)?, row.get(6)?],
        request_sha256: row.get(7)?,
        lod_receipt_object_sha256: row.get(8)?,
        collision_proxy_object_sha256: row.get(9)?,
        readiness_object_sha256: row.get(10)?,
        delivery_manifest_object_sha256: row.get(11)?,
        animation_artifact_sha256: row.get(12)?,
        materialization_status: row.get(13)?,
        canonical_sha256: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn same_game_asset_delivery_link(
    left: &GameAssetDeliveryLinkRecord,
    right: &GameAssetDeliveryLinkRecord,
) -> bool {
    left.schema_version == right.schema_version
        && left.project_id == right.project_id
        && left.lod_candidate_ids == right.lod_candidate_ids
        && left.lod_artifact_sha256s == right.lod_artifact_sha256s
        && left.request_sha256 == right.request_sha256
        && left.lod_receipt_object_sha256 == right.lod_receipt_object_sha256
        && left.collision_proxy_object_sha256 == right.collision_proxy_object_sha256
        && left.readiness_object_sha256 == right.readiness_object_sha256
        && left.delivery_manifest_object_sha256 == right.delivery_manifest_object_sha256
        && left.animation_artifact_sha256 == right.animation_artifact_sha256
        && left.materialization_status == right.materialization_status
        && left.canonical_sha256 == right.canonical_sha256
}

fn game_asset_delivery_json_hashes(link: &GameAssetDeliveryLinkRecord) -> [&str; 4] {
    [
        &link.lod_receipt_object_sha256,
        &link.collision_proxy_object_sha256,
        &link.readiness_object_sha256,
        &link.delivery_manifest_object_sha256,
    ]
}

fn game_asset_delivery_reachable_hashes(link: &GameAssetDeliveryLinkRecord) -> Vec<String> {
    let mut hashes = link.lod_artifact_sha256s.clone();
    hashes.extend(
        game_asset_delivery_json_hashes(link)
            .into_iter()
            .map(str::to_owned),
    );
    if let Some(animation_sha256) = link.animation_artifact_sha256.clone() {
        hashes.push(animation_sha256);
    }
    hashes
}

fn validate_game_asset_delivery_bindings_in_transaction(
    connection: &Connection,
    link: &GameAssetDeliveryLinkRecord,
) -> Result<(), StoreError> {
    for index in 0..3 {
        let candidate: Option<(String, Option<String>)> = connection
            .query_row(
                "SELECT project_id, prepared_object_sha256 FROM candidates WHERE candidate_id = ?1",
                params![link.lod_candidate_ids[index]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if candidate.as_ref().map(|value| value.0.as_str()) != Some(link.project_id.as_str())
            || candidate.as_ref().and_then(|value| value.1.as_deref())
                != Some(link.lod_artifact_sha256s[index].as_str())
        {
            return Err(StoreError::Contract {
                code: "GAME_ASSET_DELIVERY_CANDIDATE_BINDING_MISMATCH".to_owned(),
                message: "game delivery candidate/project/artifact binding differs".to_owned(),
            });
        }
        let artifact: Option<(String, i64)> = connection
            .query_row(
                "SELECT mime, size_bytes FROM objects WHERE sha256 = ?1",
                params![link.lod_artifact_sha256s[index]],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if artifact.as_ref().map(|value| value.0.as_str()) != Some("model/gltf-binary")
            || artifact
                .as_ref()
                .is_none_or(|value| value.1 <= 0 || value.1 > 64 * 1024 * 1024)
        {
            return Err(StoreError::Contract {
                code: "GAME_ASSET_DELIVERY_ARTIFACT_INVALID".to_owned(),
                message: "game delivery LOD artifact metadata is invalid".to_owned(),
            });
        }
    }
    let expected_json = [
        (&link.lod_receipt_object_sha256, "game-lod-set-receipt"),
        (&link.collision_proxy_object_sha256, "collision-proxy-set"),
        (
            &link.readiness_object_sha256,
            "game-engine-import-readiness",
        ),
        (
            &link.delivery_manifest_object_sha256,
            "game-asset-delivery-manifest",
        ),
    ];
    for (sha256, expected_kind) in expected_json {
        let metadata: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT mime, kind, size_bytes FROM objects WHERE sha256 = ?1",
                params![sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if metadata.as_ref().map(|value| value.0.as_str()) != Some(GAME_ASSET_DELIVERY_JSON_MIME)
            || metadata.as_ref().map(|value| value.1.as_str()) != Some(expected_kind)
            || metadata.as_ref().is_none_or(|value| {
                value.2 <= 0 || value.2 as u64 > MAX_GAME_ASSET_DELIVERY_JSON_BYTES
            })
        {
            return Err(StoreError::Contract {
                code: "GAME_ASSET_DELIVERY_OBJECT_INVALID".to_owned(),
                message: "game delivery sidecar metadata is invalid".to_owned(),
            });
        }
    }
    if let Some(animation_sha256) = link.animation_artifact_sha256.as_deref() {
        let metadata: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT mime, kind, size_bytes FROM objects WHERE sha256 = ?1",
                params![animation_sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if metadata.as_ref().map(|value| value.0.as_str()) != Some("model/gltf-binary")
            || metadata.as_ref().map(|value| value.1.as_str()) != Some("mechanical-animation-glb")
            || metadata
                .as_ref()
                .is_none_or(|value| value.2 <= 0 || value.2 > 64 * 1024 * 1024)
        {
            return Err(StoreError::Contract {
                code: "GAME_ASSET_DELIVERY_ANIMATION_INVALID".to_owned(),
                message: "game delivery animation artifact metadata is invalid".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_delivery_objects(
    store: &Store,
    link: &GameAssetDeliveryLinkRecord,
) -> Result<(), StoreError> {
    for sha256 in game_asset_delivery_json_hashes(link) {
        let bytes = store
            .cas
            .read_verified_bounded(sha256, MAX_GAME_ASSET_DELIVERY_JSON_BYTES)
            .map_err(StoreError::Cas)?;
        if bytes.is_empty() {
            return Err(StoreError::Contract {
                code: "GAME_ASSET_DELIVERY_OBJECT_INVALID".to_owned(),
                message: "game delivery JSON CAS object is empty".to_owned(),
            });
        }
    }
    if let Some(animation_sha256) = link.animation_artifact_sha256.as_deref() {
        store
            .cas
            .read_verified_bounded(animation_sha256, 64 * 1024 * 1024)
            .map_err(StoreError::Cas)?;
    }
    Ok(())
}

impl<'store> DeliveryRepository<'store> {
    pub(crate) fn new(store: &'store Store) -> Self {
        Self { store }
    }

    /// Commit one immutable game-delivery link and all of its CAS roots in a
    /// single Store transaction. An existing delivery manifest is an exact
    /// idempotent replay; the same key with different bindings is a conflict.
    pub fn record_game_asset_delivery_link(
        &self,
        link: &GameAssetDeliveryLinkRecord,
    ) -> Result<GameAssetDeliveryLinkRecord, StoreError> {
        validate_game_asset_delivery_link(link)?;
        verify_delivery_objects(self.store, link)?;

        let mut connection = self.store.lock_connection()?;
        let transaction = connection.transaction()?;
        validate_game_asset_delivery_bindings_in_transaction(&transaction, link)?;
        let existing = transaction
            .query_row(
                &format!("{GAME_DELIVERY_LINK_SELECT} WHERE delivery_manifest_object_sha256 = ?1"),
                params![link.delivery_manifest_object_sha256],
                game_asset_delivery_link_from_row,
            )
            .optional()?;
        if let Some(existing) = existing {
            if !same_game_asset_delivery_link(&existing, link) {
                return Err(StoreError::Contract {
                    code: "GAME_ASSET_DELIVERY_LINK_CONFLICT".to_owned(),
                    message: "delivery manifest is already bound to a different cohort".to_owned(),
                });
            }
            mark_reachable_in_transaction(
                &transaction,
                &game_asset_delivery_reachable_hashes(&existing),
            )?;
            transaction.commit()?;
            return Ok(existing);
        }
        transaction.execute(
            "INSERT INTO game_asset_delivery_links (delivery_manifest_object_sha256, project_id, lod0_candidate_id, lod1_candidate_id, lod2_candidate_id, lod0_artifact_sha256, lod1_artifact_sha256, lod2_artifact_sha256, request_sha256, lod_receipt_object_sha256, collision_proxy_object_sha256, readiness_object_sha256, animation_artifact_sha256, materialization_status, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                link.delivery_manifest_object_sha256,
                link.project_id,
                link.lod_candidate_ids[0],
                link.lod_candidate_ids[1],
                link.lod_candidate_ids[2],
                link.lod_artifact_sha256s[0],
                link.lod_artifact_sha256s[1],
                link.lod_artifact_sha256s[2],
                link.request_sha256,
                link.lod_receipt_object_sha256,
                link.collision_proxy_object_sha256,
                link.readiness_object_sha256,
                link.animation_artifact_sha256,
                link.materialization_status,
                link.canonical_sha256,
                link.created_at,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &game_asset_delivery_reachable_hashes(link))?;
        transaction.commit()?;
        Ok(link.clone())
    }

    /// Read one delivery link by its manifest CAS hash and revalidate every
    /// candidate, object metadata and embedded CAS sidecar on each read.
    pub fn get_game_asset_delivery_link(
        &self,
        delivery_manifest_object_sha256: &str,
    ) -> Result<Option<GameAssetDeliveryLinkRecord>, StoreError> {
        if !is_sha256(delivery_manifest_object_sha256) {
            return Err(StoreError::InvalidData(
                "game delivery manifest hash is invalid".to_owned(),
            ));
        }
        let connection = self.store.lock_connection()?;
        let link = connection
            .query_row(
                &format!("{GAME_DELIVERY_LINK_SELECT} WHERE delivery_manifest_object_sha256 = ?1"),
                params![delivery_manifest_object_sha256],
                game_asset_delivery_link_from_row,
            )
            .optional()?;
        let Some(link) = link else {
            return Ok(None);
        };
        validate_game_asset_delivery_link(&link)?;
        validate_game_asset_delivery_bindings_in_transaction(&connection, &link)?;
        drop(connection);
        verify_delivery_objects(self.store, &link)?;
        Ok(Some(link))
    }

    /// List all immutable delivery links for a project in deterministic newest
    /// first order. Every row receives the same strict validation as get; a
    /// malformed or cross-project row fails closed instead of being hidden by
    /// the projection.
    pub fn list_game_asset_delivery_links(
        &self,
        project_id: &str,
    ) -> Result<Vec<GameAssetDeliveryLinkRecord>, StoreError> {
        if !is_opaque_id(project_id) {
            return Err(StoreError::InvalidData(
                "game delivery project id is invalid".to_owned(),
            ));
        }
        let connection = self.store.lock_connection()?;
        let mut statement = connection.prepare(&format!(
            "{GAME_DELIVERY_LINK_SELECT} WHERE project_id = ?1 ORDER BY created_at DESC, delivery_manifest_object_sha256 ASC"
        ))?;
        let rows = statement.query_map(params![project_id], game_asset_delivery_link_from_row)?;
        let links = rows.collect::<Result<Vec<_>, _>>()?;
        for link in &links {
            validate_game_asset_delivery_link(link)?;
            validate_game_asset_delivery_bindings_in_transaction(&connection, link)?;
        }
        drop(statement);
        drop(connection);
        for link in &links {
            verify_delivery_objects(self.store, link)?;
        }
        Ok(links)
    }
}

impl Store {
    /// Borrow the first physical Delivery repository from this Store.
    ///
    /// This constructor only creates a typed view; Store::migrate remains the
    /// sole migration owner and the Store remains the sole CAS/SQLite owner.
    pub fn delivery_repository(&self) -> DeliveryRepository<'_> {
        DeliveryRepository::new(self)
    }

    /// Compatibility shim for callers still using the Store root. New
    /// Delivery code should use delivery_repository() directly.
    pub fn list_game_asset_delivery_links(
        &self,
        project_id: &str,
    ) -> Result<Vec<GameAssetDeliveryLinkRecord>, StoreError> {
        self.delivery_repository()
            .list_game_asset_delivery_links(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateRecord, ProjectRecord};
    use forgecad_core::{canonical_json_hash, sha256_hex};
    use serde_json::json;

    fn sha(seed: &str) -> String {
        sha256_hex(seed.as_bytes())
    }

    fn project(store: &Store, project_id: &str) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: project_id.to_owned(),
                name: "Delivery repository test".to_owned(),
                policy: json!({"scope":"test"}),
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn link_fixture() -> (Store, GameAssetDeliveryLinkRecord, Vec<String>) {
        let store = Store::memory().expect("store");
        let project_id = "project-delivery-repository";
        project(&store, project_id);
        let mut artifact_hashes = Vec::new();
        for (index, candidate_id) in [
            "candidate-delivery-lod0",
            "candidate-delivery-lod1",
            "candidate-delivery-lod2",
        ]
        .into_iter()
        .enumerate()
        {
            let artifact = store
                .put_object(
                    format!("glb-{index}").as_bytes(),
                    None,
                    "model/gltf-binary",
                    "fixture-glb",
                    "1",
                )
                .expect("artifact")
                .record;
            artifact_hashes.push(artifact.sha256.clone());
            store
                .insert_candidate(&CandidateRecord {
                    schema_version: "Candidate@1".to_owned(),
                    candidate_id: candidate_id.to_owned(),
                    project_id: project_id.to_owned(),
                    base_version_id: None,
                    source_version_id: None,
                    prepared_object_id: None,
                    prepared_object_sha256: Some(artifact.sha256),
                    state: "prepared".to_owned(),
                    request_sha256: sha(&format!("request-{index}")),
                    manifest_hash: None,
                    quality_report_id: None,
                    quality_hard_gate_passed: false,
                    canonical_sha256: sha(&format!("candidate-{index}")),
                    error_code: None,
                    created_at: format!("{}", index + 1),
                    updated_at: format!("{}", index + 1),
                })
                .expect("candidate");
        }
        let sidecar = |kind: &str| {
            store
                .put_object(
                    kind.as_bytes(),
                    None,
                    GAME_ASSET_DELIVERY_JSON_MIME,
                    kind,
                    "1",
                )
                .expect("sidecar")
                .record
                .sha256
        };
        let mut link = GameAssetDeliveryLinkRecord {
            schema_version: "GameAssetDeliveryLink@1".to_owned(),
            project_id: project_id.to_owned(),
            lod_candidate_ids: vec![
                "candidate-delivery-lod0".to_owned(),
                "candidate-delivery-lod1".to_owned(),
                "candidate-delivery-lod2".to_owned(),
            ],
            lod_artifact_sha256s: artifact_hashes.clone(),
            request_sha256: sha("delivery-request"),
            lod_receipt_object_sha256: sidecar("game-lod-set-receipt"),
            collision_proxy_object_sha256: sidecar("collision-proxy-set"),
            readiness_object_sha256: sidecar("game-engine-import-readiness"),
            delivery_manifest_object_sha256: sidecar("game-asset-delivery-manifest"),
            animation_artifact_sha256: None,
            materialization_status: "runtime-owned-durable-game-delivery-link".to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        link.canonical_sha256 = {
            let mut canonical = link.clone();
            canonical.canonical_sha256.clear();
            canonical_json_hash(&serde_json::to_value(canonical).expect("canonical"))
        };
        (store, link, artifact_hashes)
    }

    #[test]
    fn borrowed_repository_commits_replays_lists_and_preserves_cas_reachability() {
        let (store, link, artifact_hashes) = link_fixture();
        let repository = store.delivery_repository();
        assert!(std::ptr::eq(repository.store, &store));
        let committed = repository
            .record_game_asset_delivery_link(&link)
            .expect("commit");
        assert_eq!(committed, link);
        for hash in artifact_hashes.iter().chain(
            [
                &link.lod_receipt_object_sha256,
                &link.collision_proxy_object_sha256,
                &link.readiness_object_sha256,
                &link.delivery_manifest_object_sha256,
            ]
            .iter()
            .copied(),
        ) {
            assert_eq!(
                store
                    .get_object(hash)
                    .expect("object")
                    .unwrap()
                    .reachability,
                "reachable"
            );
        }
        let replay = repository
            .record_game_asset_delivery_link(&link)
            .expect("replay");
        assert_eq!(replay, link);
        let root_replay = store
            .record_game_asset_delivery_link(&link)
            .expect("root compatibility replay");
        assert_eq!(root_replay, link);
        let listed = repository
            .list_game_asset_delivery_links(&link.project_id)
            .expect("list");
        assert_eq!(listed, vec![link.clone()]);
        assert_eq!(
            store
                .get_game_asset_delivery_link(&link.delivery_manifest_object_sha256)
                .expect("get")
                .as_ref(),
            Some(&link)
        );
        assert_eq!(
            store
                .list_game_asset_delivery_links(&link.project_id)
                .expect("shim list"),
            vec![link]
        );
    }

    #[test]
    fn borrowed_repository_rejects_manifest_conflict_without_mutating_row() {
        let (store, link, _) = link_fixture();
        let repository = store.delivery_repository();
        repository
            .record_game_asset_delivery_link(&link)
            .expect("commit");
        let mut conflict = link.clone();
        // Keep the request digest distinct so this is a real same-manifest
        // content conflict rather than an exact replay.
        conflict.request_sha256 = sha("x-request");
        let mut canonical = conflict.clone();
        canonical.canonical_sha256.clear();
        conflict.canonical_sha256 = canonical_json_hash(&serde_json::to_value(canonical).unwrap());
        let error = repository
            .record_game_asset_delivery_link(&conflict)
            .expect_err("conflict");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. } if code == "GAME_ASSET_DELIVERY_LINK_CONFLICT"
        ));
        assert_eq!(
            repository
                .get_game_asset_delivery_link(&link.delivery_manifest_object_sha256)
                .expect("get")
                .unwrap(),
            link
        );
    }

    #[test]
    fn list_fails_closed_for_invalid_project_identity() {
        let store = Store::memory().expect("store");
        assert!(matches!(
            store
                .delivery_repository()
                .list_game_asset_delivery_links(""),
            Err(StoreError::InvalidData(_))
        ));
    }
}
