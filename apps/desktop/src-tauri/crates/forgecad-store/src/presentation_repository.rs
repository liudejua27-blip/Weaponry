//! Physical Store boundary for the first-person presentation animation aggregate.
//!
//! MechanicalAnimationClip@1 is a durable candidate-bound presentation
//! artifact. This repository borrows Store so the Store-owned SQLite
//! connection, migration owner, CAS root, reachability policy, and
//! cross-aggregate transaction semantics remain singular. The methods on
//! Store below are compatibility shims; the implementation lives here.

use forgecad_contracts::{is_opaque_id, MechanicalAnimationClipLinkRecord};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::fs;

use super::{
    mechanical_animation_clip_link_from_row, same_mechanical_animation_clip_link,
    validate_mechanical_animation_clip_link, CasObject, Store, StoreError,
    MAX_MECHANICAL_ANIMATION_CLIPS_PER_CANDIDATE, MAX_MECHANICAL_ANIMATION_CLIP_BYTES,
    MECHANICAL_ANIMATION_CLIP_KIND, MECHANICAL_ANIMATION_CLIP_MIME,
};

/// Borrowed repository for the coherent MechanicalAnimationClip@1 aggregate.
#[derive(Clone, Copy)]
pub struct PresentationRepository<'store> {
    store: &'store Store,
}

#[cfg(test)]
mod tests {
    use super::PresentationRepository;
    use crate::{Store, StoreError};
    use forgecad_contracts::MechanicalAnimationClipLinkRecord;

    fn malformed_clip_link() -> MechanicalAnimationClipLinkRecord {
        MechanicalAnimationClipLinkRecord {
            schema_version: String::new(),
            project_id: String::new(),
            candidate_id: String::new(),
            artifact_id: String::new(),
            artifact_readback_sha256: String::new(),
            geometry_candidate_evidence_sha256: String::new(),
            program_sha256: String::new(),
            operator_catalog_sha256: String::new(),
            readback_config_sha256: String::new(),
            clip_id: String::new(),
            request_sha256: String::new(),
            clip_object_sha256: String::new(),
            clip_sha256: String::new(),
            rest_frame_sha256: String::new(),
            pose_action_sha256: String::new(),
            source_replay_worker_cohort_sha256: String::new(),
            materialization_status: String::new(),
            canonical_sha256: String::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn borrowed_repository_and_store_shim_fail_closed_before_sqlite_write() {
        let store = Store::memory().expect("store");
        let link = malformed_clip_link();
        let repository: PresentationRepository<'_> = store.presentation_repository();
        assert!(matches!(
            repository.record_mechanical_animation_clip_link(&link),
            Err(StoreError::InvalidData(_))
        ));
        assert!(matches!(
            store.record_mechanical_animation_clip_link(&link),
            Err(StoreError::InvalidData(_))
        ));
    }
}

impl<'store> PresentationRepository<'store> {
    pub(crate) fn new(store: &'store Store) -> Self {
        Self { store }
    }

    /// Create the aggregate table and index inside Store's one migration
    /// transaction. No independent connection or migration state is allowed.
    pub(crate) fn ensure_schema(transaction: &rusqlite::Transaction<'_>) -> Result<(), StoreError> {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS mechanical_animation_clip_links (
                 project_id TEXT NOT NULL REFERENCES projects(project_id),
                 candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
                 artifact_id TEXT NOT NULL REFERENCES objects(sha256),
                 artifact_readback_sha256 TEXT NOT NULL,
                 geometry_candidate_evidence_sha256 TEXT NOT NULL,
                 program_sha256 TEXT NOT NULL,
                 operator_catalog_sha256 TEXT NOT NULL,
                 readback_config_sha256 TEXT NOT NULL,
                 clip_id TEXT NOT NULL,
                 request_sha256 TEXT NOT NULL,
                 clip_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
                 clip_sha256 TEXT NOT NULL,
                 rest_frame_sha256 TEXT NOT NULL,
                 pose_action_sha256 TEXT NOT NULL,
                 source_replay_worker_cohort_sha256 TEXT NOT NULL,
                 materialization_status TEXT NOT NULL CHECK (materialization_status = 'runtime-owned-immutable-cas-clip'),
                 canonical_sha256 TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 PRIMARY KEY (candidate_id, clip_id)
             );
             CREATE INDEX IF NOT EXISTS mechanical_animation_clip_links_project_idx
                 ON mechanical_animation_clip_links(project_id, candidate_id, clip_id);",
        )?;
        Ok(())
    }

    /// Atomically bind one canonical mechanical animation clip CAS object to
    /// the exact candidate and durable geometry evidence cohort. Reusing the
    /// same candidate/clip identity is idempotent only when every hash agrees.
    pub fn record_mechanical_animation_clip_link(
        &self,
        link: &MechanicalAnimationClipLinkRecord,
    ) -> Result<(), StoreError> {
        validate_mechanical_animation_clip_link(link)?;
        let clip_bytes = self
            .store
            .cas
            .read_verified_bounded(
                &link.clip_object_sha256,
                MAX_MECHANICAL_ANIMATION_CLIP_BYTES,
            )
            .map_err(StoreError::Cas)?;
        if clip_bytes.is_empty() {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_BUDGET_EXCEEDED".to_owned(),
                message: "mechanical animation clip CAS object is empty".to_owned(),
            });
        }

        let mut connection = self.store.lock_connection()?;
        let transaction = connection.transaction()?;
        let candidate: Option<(String, Option<String>)> = transaction
            .query_row(
                "SELECT project_id, prepared_object_sha256 FROM candidates WHERE candidate_id = ?1",
                params![link.candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let candidate = candidate.ok_or_else(|| StoreError::Contract {
            code: "MECHANICAL_ANIMATION_CLIP_CANDIDATE_NOT_FOUND".to_owned(),
            message: "candidate is unavailable for the mechanical animation clip".to_owned(),
        })?;
        if candidate.0 != link.project_id
            || candidate.1.as_deref() != Some(link.artifact_id.as_str())
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_CANDIDATE_BINDING_MISMATCH".to_owned(),
                message: "candidate project or prepared artifact differs from the clip".to_owned(),
            });
        }
        let evidence: Option<(String, String, String, String, String, String)> = transaction
            .query_row(
                "SELECT project_id, artifact_object_sha256, artifact_readback_object_sha256, canonical_sha256, geometry_program_sha256, operator_catalog_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
                params![link.candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()?;
        let evidence = evidence.ok_or_else(|| StoreError::Contract {
            code: "MECHANICAL_ANIMATION_CLIP_EVIDENCE_NOT_FOUND".to_owned(),
            message: "durable geometry evidence is unavailable for the clip".to_owned(),
        })?;
        if evidence.0 != link.project_id
            || evidence.1 != link.artifact_id
            || evidence.3 != link.geometry_candidate_evidence_sha256
            || evidence.4 != link.program_sha256
            || evidence.5 != link.operator_catalog_sha256
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_EVIDENCE_BINDING_MISMATCH".to_owned(),
                message: "geometry evidence differs from the clip link".to_owned(),
            });
        }
        let readback_bytes = self
            .store
            .cas
            .read_verified_bounded(&evidence.2, MAX_MECHANICAL_ANIMATION_CLIP_BYTES)
            .map_err(StoreError::Cas)?;
        let readback: Value = serde_json::from_slice(&readback_bytes).map_err(|error| {
            StoreError::InvalidData(format!("artifact readback is not valid JSON: {error}"))
        })?;
        if readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(link.artifact_readback_sha256.as_str())
            || readback
                .get("readback_config_sha256")
                .and_then(Value::as_str)
                != Some(link.readback_config_sha256.as_str())
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_READBACK_BINDING_MISMATCH".to_owned(),
                message: "ArtifactReadback differs from the clip link".to_owned(),
            });
        }
        let clip_object: Option<(String, String, i64)> = transaction
            .query_row(
                "SELECT kind, mime, size_bytes FROM objects WHERE sha256 = ?1",
                params![link.clip_object_sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let clip_object = clip_object.ok_or_else(|| StoreError::Contract {
            code: "MECHANICAL_ANIMATION_CLIP_OBJECT_UNAVAILABLE".to_owned(),
            message: "clip CAS object is not registered in Runtime storage".to_owned(),
        })?;
        if clip_object.0 != MECHANICAL_ANIMATION_CLIP_KIND
            || clip_object.1 != MECHANICAL_ANIMATION_CLIP_MIME
            || clip_object.2 <= 0
            || clip_object.2 as u64 > MAX_MECHANICAL_ANIMATION_CLIP_BYTES
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_OBJECT_INVALID".to_owned(),
                message: "clip CAS metadata is not the bounded JSON clip kind".to_owned(),
            });
        }
        let existing = transaction
            .query_row(
                "SELECT project_id, candidate_id, artifact_id, artifact_readback_sha256, geometry_candidate_evidence_sha256, program_sha256, operator_catalog_sha256, readback_config_sha256, clip_id, request_sha256, clip_object_sha256, clip_sha256, rest_frame_sha256, pose_action_sha256, source_replay_worker_cohort_sha256, materialization_status, canonical_sha256, created_at FROM mechanical_animation_clip_links WHERE candidate_id = ?1 AND clip_id = ?2",
                params![link.candidate_id, link.clip_id],
                mechanical_animation_clip_link_from_row,
            )
            .optional()?;
        if let Some(existing) = existing {
            if !same_mechanical_animation_clip_link(&existing, link) {
                return Err(StoreError::Contract {
                    code: "MECHANICAL_ANIMATION_CLIP_LINK_CONFLICT".to_owned(),
                    message: "candidate/clip is already bound to different content".to_owned(),
                });
            }
            transaction.execute(
                "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
                params![link.clip_object_sha256],
            )?;
            transaction.commit()?;
            return Ok(());
        }
        transaction.execute(
            "INSERT INTO mechanical_animation_clip_links (project_id, candidate_id, artifact_id, artifact_readback_sha256, geometry_candidate_evidence_sha256, program_sha256, operator_catalog_sha256, readback_config_sha256, clip_id, request_sha256, clip_object_sha256, clip_sha256, rest_frame_sha256, pose_action_sha256, source_replay_worker_cohort_sha256, materialization_status, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![link.project_id, link.candidate_id, link.artifact_id, link.artifact_readback_sha256, link.geometry_candidate_evidence_sha256, link.program_sha256, link.operator_catalog_sha256, link.readback_config_sha256, link.clip_id, link.request_sha256, link.clip_object_sha256, link.clip_sha256, link.rest_frame_sha256, link.pose_action_sha256, link.source_replay_worker_cohort_sha256, link.materialization_status, link.canonical_sha256, link.created_at],
        )?;
        let marked = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![link.clip_object_sha256],
        )?;
        if marked != 1 {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_OBJECT_UNAVAILABLE".to_owned(),
                message: "clip CAS metadata disappeared during link commit".to_owned(),
            });
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn get_mechanical_animation_clip_link(
        &self,
        candidate_id: &str,
        clip_id: &str,
    ) -> Result<Option<MechanicalAnimationClipLinkRecord>, StoreError> {
        if !is_opaque_id(candidate_id) || !is_opaque_id(clip_id) {
            return Err(StoreError::InvalidData(
                "mechanical animation clip identity is invalid".to_owned(),
            ));
        }
        let connection = self.store.lock_connection()?;
        let link = connection
            .query_row(
                "SELECT project_id, candidate_id, artifact_id, artifact_readback_sha256, geometry_candidate_evidence_sha256, program_sha256, operator_catalog_sha256, readback_config_sha256, clip_id, request_sha256, clip_object_sha256, clip_sha256, rest_frame_sha256, pose_action_sha256, source_replay_worker_cohort_sha256, materialization_status, canonical_sha256, created_at FROM mechanical_animation_clip_links WHERE candidate_id = ?1 AND clip_id = ?2",
                params![candidate_id, clip_id],
                mechanical_animation_clip_link_from_row,
            )
            .optional()?;
        drop(connection);
        let Some(link) = link else {
            return Ok(None);
        };
        validate_mechanical_animation_clip_link(&link)?;
        let candidate = self
            .store
            .get_candidate(&link.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_CANDIDATE_NOT_FOUND".to_owned(),
                message: "linked candidate is unavailable".to_owned(),
            })?;
        if candidate.project_id != link.project_id
            || candidate.prepared_object_sha256.as_deref() != Some(link.artifact_id.as_str())
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_CANDIDATE_BINDING_MISMATCH".to_owned(),
                message: "stored clip no longer matches the candidate".to_owned(),
            });
        }
        let evidence = self
            .store
            .get_geometry_candidate_evidence(&link.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_EVIDENCE_NOT_FOUND".to_owned(),
                message: "linked geometry evidence is unavailable".to_owned(),
            })?;
        if evidence.project_id != link.project_id
            || evidence.artifact_object_sha256 != link.artifact_id
            || evidence.canonical_sha256 != link.geometry_candidate_evidence_sha256
            || evidence.geometry_program_sha256 != link.program_sha256
            || evidence.operator_catalog_sha256 != link.operator_catalog_sha256
            || evidence.readback_config_sha256 != link.readback_config_sha256
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_EVIDENCE_BINDING_MISMATCH".to_owned(),
                message: "stored clip no longer matches durable geometry evidence".to_owned(),
            });
        }
        let object = self
            .store
            .get_object(&link.clip_object_sha256)?
            .ok_or_else(|| StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_OBJECT_UNAVAILABLE".to_owned(),
                message: "linked clip CAS object is unavailable".to_owned(),
            })?;
        if object.kind != MECHANICAL_ANIMATION_CLIP_KIND
            || object.mime != MECHANICAL_ANIMATION_CLIP_MIME
            || object.size_bytes == 0
            || object.size_bytes > MAX_MECHANICAL_ANIMATION_CLIP_BYTES
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_OBJECT_INVALID".to_owned(),
                message: "linked clip CAS metadata is invalid".to_owned(),
            });
        }
        self.store
            .cas
            .read_verified_bounded(
                &link.clip_object_sha256,
                MAX_MECHANICAL_ANIMATION_CLIP_BYTES,
            )
            .map_err(StoreError::Cas)?;
        Ok(Some(link))
    }

    /// List the bounded immutable clip links for one candidate. Each returned
    /// row is reloaded through the same fail-closed binding/CAS verification
    /// as a direct lookup so the Viewer never receives a stale SQLite-only
    /// projection.
    pub fn list_mechanical_animation_clip_links(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<MechanicalAnimationClipLinkRecord>, StoreError> {
        if !is_opaque_id(candidate_id) {
            return Err(StoreError::InvalidData(
                "mechanical animation clip candidate identity is invalid".to_owned(),
            ));
        }
        let connection = self.store.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT clip_id FROM mechanical_animation_clip_links WHERE candidate_id = ?1 ORDER BY created_at DESC, clip_id ASC LIMIT ?2",
        )?;
        let clip_ids = statement
            .query_map(
                params![
                    candidate_id,
                    (MAX_MECHANICAL_ANIMATION_CLIPS_PER_CANDIDATE + 1) as i64
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        if clip_ids.len() > MAX_MECHANICAL_ANIMATION_CLIPS_PER_CANDIDATE {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_INVENTORY_BUDGET_EXCEEDED".to_owned(),
                message: "candidate has more than 16 durable mechanical animation clips".to_owned(),
            });
        }
        clip_ids
            .into_iter()
            .map(|clip_id| {
                self.get_mechanical_animation_clip_link(candidate_id, &clip_id)?
                    .ok_or_else(|| StoreError::Contract {
                        code: "MECHANICAL_ANIMATION_CLIP_INVENTORY_CHANGED".to_owned(),
                        message: "clip inventory changed during verified readback".to_owned(),
                    })
            })
            .collect()
    }

    pub fn discard_new_temporary_mechanical_animation_clip(
        &self,
        object: &CasObject,
    ) -> Result<bool, StoreError> {
        if !object.created_new {
            return Ok(false);
        }
        let mut connection = self.store.lock_connection()?;
        let transaction = connection.transaction()?;
        let metadata: Option<(String, String, String)> = transaction
            .query_row(
                "SELECT mime, kind, reachability FROM objects WHERE sha256 = ?1",
                params![object.record.sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((mime, kind, reachability)) = metadata else {
            return Ok(false);
        };
        if mime != MECHANICAL_ANIMATION_CLIP_MIME
            || kind != MECHANICAL_ANIMATION_CLIP_KIND
            || reachability != "temporary"
        {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_ROLLBACK_DENIED".to_owned(),
                message: "only the current operation's unlinked temporary clip may be rolled back"
                    .to_owned(),
            });
        }
        let link_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM mechanical_animation_clip_links WHERE clip_object_sha256 = ?1",
            params![object.record.sha256],
            |row| row.get(0),
        )?;
        if link_count != 0 {
            return Err(StoreError::Contract {
                code: "MECHANICAL_ANIMATION_CLIP_ROLLBACK_DENIED".to_owned(),
                message: "linked clip CAS content cannot be rolled back".to_owned(),
            });
        }
        transaction.execute(
            "DELETE FROM objects WHERE sha256 = ?1 AND reachability = 'temporary'",
            params![object.record.sha256],
        )?;
        transaction.commit()?;
        match fs::remove_file(&object.path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(StoreError::Io(error)),
        }
    }
}
