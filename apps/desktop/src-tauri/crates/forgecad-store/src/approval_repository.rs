//! Physical Store boundary for the ApprovalLifecycle aggregate.
//!
//! Candidate confirmation/rejection, immutable version rows, and export
//! preparation/confirmation share one lifecycle contract and transaction
//! policy. Their implementations live in this module while the public
//! `Store` methods remain source-compatible for Runtime callers.
//!
//! This extraction deliberately borrows the existing Store connection and
//! CAS. `Store::migrate` remains the only migration owner and all existing
//! reachability updates stay inside the original SQLite transactions.

use super::*;

impl Store {
    pub fn insert_version(&self, version: &DesignAssetVersionRecord) -> Result<(), StoreError> {
        validate_version(version)?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO design_asset_versions (version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version.version_id,
                version.project_id,
                version.parent_version_id,
                version.candidate_id,
                version.manifest_hash,
                version.canonical_sha256,
                version.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_version(
        &self,
        version_id: &str,
    ) -> Result<Option<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at FROM design_asset_versions WHERE version_id = ?1",
                params![version_id],
                |row| {
                    Ok(DesignAssetVersionRecord {
                        schema_version: "DesignAssetVersion@1".to_owned(),
                        version_id: row.get(0)?,
                        project_id: row.get(1)?,
                        parent_version_id: row.get(2)?,
                        candidate_id: row.get(3)?,
                        manifest_hash: row.get(4)?,
                        canonical_sha256: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_versions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at FROM design_asset_versions WHERE (?1 IS NULL OR project_id = ?1) ORDER BY created_at DESC, version_id ASC",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(DesignAssetVersionRecord {
                schema_version: "DesignAssetVersion@1".to_owned(),
                version_id: row.get(0)?,
                project_id: row.get(1)?,
                parent_version_id: row.get(2)?,
                candidate_id: row.get(3)?,
                manifest_hash: row.get(4)?,
                canonical_sha256: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn latest_version_for_project(
        &self,
        project_id: &str,
    ) -> Result<Option<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT v.version_id, v.project_id, v.parent_version_id, v.candidate_id, v.manifest_hash, v.canonical_sha256, v.created_at FROM projects p JOIN snapshots s ON s.snapshot_id = p.head_snapshot_id JOIN design_asset_versions v ON v.candidate_id = s.candidate_id WHERE p.project_id = ?1 LIMIT 1",
                params![project_id],
                |row| {
                    Ok(DesignAssetVersionRecord {
                        schema_version: "DesignAssetVersion@1".to_owned(),
                        version_id: row.get(0)?,
                        project_id: row.get(1)?,
                        parent_version_id: row.get(2)?,
                        candidate_id: row.get(3)?,
                        manifest_hash: row.get(4)?,
                        canonical_sha256: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn confirm_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        self.confirm_candidate_with_tool(request, now, "candidate_confirm", None, None)
    }

    pub fn confirm_cross_view_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        request_sha256: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        if !is_sha256(request_sha256) {
            return Err(StoreError::InvalidData(
                "cross-view promotion request hash is invalid".to_owned(),
            ));
        }
        self.confirm_candidate_with_tool(
            request,
            now,
            "cross_view_promotion_confirm",
            None,
            Some(request_sha256),
        )
    }

    pub fn confirm_repair_apply_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        request_sha256: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        if !is_sha256(request_sha256) {
            return Err(StoreError::InvalidData(
                "repair apply request hash is invalid".to_owned(),
            ));
        }
        self.confirm_candidate_with_tool(
            request,
            now,
            "repair_apply_confirm",
            None,
            Some(request_sha256),
        )
    }

    pub fn restore_confirm(
        &self,
        request: &RestoreConfirmRequest,
        now: &str,
    ) -> Result<RestoreConfirmResult, StoreError> {
        validate_restore_confirm_request(request)?;
        let candidate_request = CandidateConfirmRequest {
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            base_version_id: request.base_version_id.clone(),
            prepared_object_id: request.prepared_object_id.clone(),
            prepared_object_sha256: request.prepared_object_sha256.clone(),
            quality_report_id: request.quality_report_id.clone(),
            approval_receipt_id: request.approval_receipt_id.clone(),
            approval_summary: request.approval_summary.clone(),
            approval_session_id: request.approval_session_id.clone(),
            approval_expires_at: request.approval_expires_at.clone(),
            idempotency_key: request.idempotency_key.clone(),
        };
        let result = self.confirm_candidate_with_tool(
            &candidate_request,
            now,
            "restore_confirm",
            Some(request.source_version_id.as_str()),
            Some(&canonical_json_hash(
                &serde_json::to_value(request)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )),
        )?;
        Ok(RestoreConfirmResult {
            schema_version: "RestoreConfirmResult@1".to_owned(),
            candidate_id: result.candidate_id,
            project_id: result.project_id,
            source_version_id: request.source_version_id.clone(),
            version_id: result.version_id,
            snapshot_id: result.snapshot_id,
            approval_receipt_id: result.approval_receipt_id,
            request_sha256: result.request_sha256,
            replayed: result.replayed,
        })
    }

    fn confirm_candidate_with_tool(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        approval_tool: &str,
        expected_source_version_id: Option<&str>,
        request_sha256_override: Option<&str>,
    ) -> Result<CandidateConfirmResult, StoreError> {
        validate_confirm_request(request)?;
        let request_value = serde_json::to_value(request)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let request_sha256 = request_sha256_override
            .map(str::to_owned)
            .unwrap_or_else(|| canonical_json_hash(&request_value));
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
        {
            if project_id != request.project_id
                || tool != approval_tool
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: CandidateConfirmResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }

        let project = transaction
            .query_row(
                "SELECT active_snapshot_revision, head_snapshot_id FROM projects WHERE project_id = ?1",
                params![request.project_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "project does not exist".to_owned(),
            })?;
        let candidate = read_candidate_for_transaction(&transaction, &request.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "candidate not found".to_owned(),
            })?;
        if candidate.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "candidate is outside the requested project".to_owned(),
            });
        }
        if expected_source_version_id.is_some() {
            if candidate.source_version_id.as_deref() != expected_source_version_id {
                return Err(StoreError::Contract {
                    code: "RESTORE_SOURCE_MISMATCH".to_owned(),
                    message: "restore candidate is not bound to the requested source version"
                        .to_owned(),
                });
            }
        } else if candidate.source_version_id.is_some() {
            return Err(StoreError::Contract {
                code: "CANDIDATE_OPERATION_MISMATCH".to_owned(),
                message: "restore candidate requires restore_confirm".to_owned(),
            });
        }
        if candidate.state != "reviewable" || !candidate.quality_hard_gate_passed {
            return Err(StoreError::Contract {
                code: "QUALITY_HARD_GATE_FAILED".to_owned(),
                message: "candidate is not reviewable with a passing hard quality gate".to_owned(),
            });
        }
        if candidate.base_version_id != request.base_version_id {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "confirm base does not match the prepared candidate".to_owned(),
            });
        }
        if candidate.prepared_object_id.as_deref() != Some(request.prepared_object_id.as_str())
            || candidate.prepared_object_sha256.as_deref()
                != Some(request.prepared_object_sha256.as_str())
            || candidate.quality_report_id.as_deref() != Some(request.quality_report_id.as_str())
        {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "prepared object or quality binding does not match the candidate"
                    .to_owned(),
            });
        }
        let current_head: Option<String> = transaction
            .query_row(
                "SELECT v.version_id FROM projects p JOIN snapshots s ON s.snapshot_id = p.head_snapshot_id JOIN design_asset_versions v ON v.candidate_id = s.candidate_id WHERE p.project_id = ?1 LIMIT 1",
                params![request.project_id],
                |row| row.get(0),
            )
            .optional()?;
        if current_head != request.base_version_id {
            return Err(StoreError::Contract {
                code: "STALE_BASE_VERSION".to_owned(),
                message: "project head changed after the candidate was prepared".to_owned(),
            });
        }
        let object_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM objects WHERE sha256 = ?1",
                params![request.prepared_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared CAS object is unavailable".to_owned(),
            });
        }
        // The caller supplies approval context, but the durable receipt ID is
        // always minted by Runtime inside this transaction.
        let approval_receipt_id = generated_approval_receipt_id();
        if is_expired(now, &request.approval_expires_at)? {
            let approval = approval_record(
                request.project_id.as_str(),
                approval_tool,
                approval_receipt_id.as_str(),
                candidate.base_version_id.as_deref(),
                request.prepared_object_id.as_str(),
                request.prepared_object_sha256.as_str(),
                Some(request.quality_report_id.as_str()),
                request.approval_summary.as_str(),
                "expired",
                request.approval_expires_at.as_str(),
                request.approval_session_id.as_str(),
                now,
            )?;
            insert_approval(&transaction, &approval)?;
            transaction.commit()?;
            return Err(StoreError::Contract {
                code: "APPROVAL_EXPIRED".to_owned(),
                message: "approval receipt expired before confirm".to_owned(),
            });
        }
        let approval = approval_record(
            request.project_id.as_str(),
            approval_tool,
            approval_receipt_id.as_str(),
            candidate.base_version_id.as_deref(),
            request.prepared_object_id.as_str(),
            request.prepared_object_sha256.as_str(),
            Some(request.quality_report_id.as_str()),
            request.approval_summary.as_str(),
            "approved",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        let marked_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![request.prepared_object_sha256],
        )?;
        if marked_reachable != 1 {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared CAS object disappeared before confirm".to_owned(),
            });
        }

        let manifest_hash = candidate
            .manifest_hash
            .clone()
            .or_else(|| candidate.prepared_object_sha256.clone())
            .ok_or_else(|| StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "candidate has no manifest hash".to_owned(),
            })?;
        if !is_sha256(&manifest_hash) {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "candidate manifest hash is invalid".to_owned(),
            });
        }
        let version_id = format!("version-{}", Uuid::new_v4().simple());
        let version_created_at = now.to_owned();
        let version_canonical_sha256 = canonical_json_hash(&serde_json::json!({
            "schema_version": "DesignAssetVersion@1",
            "version_id": version_id,
            "project_id": request.project_id,
            "parent_version_id": request.base_version_id,
            "candidate_id": request.candidate_id,
            "manifest_hash": manifest_hash,
            "created_at": version_created_at,
        }));
        transaction.execute(
            "INSERT INTO design_asset_versions (version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version_id,
                request.project_id,
                request.base_version_id,
                request.candidate_id,
                manifest_hash,
                version_canonical_sha256,
                version_created_at,
            ],
        )?;

        let snapshot_id = format!("snapshot-{}", Uuid::new_v4().simple());
        let snapshot_revision = project.0 + 1;
        let snapshot_canonical_sha256 = canonical_json_hash(&serde_json::json!({
            "schema_version": "ActiveDesignSnapshot@1",
            "snapshot_id": snapshot_id,
            "project_id": request.project_id,
            "parent_snapshot_id": project.1,
            "candidate_id": request.candidate_id,
            "revision": snapshot_revision,
            "status": "confirmed",
            "manifest_hash": manifest_hash,
            "created_at": now,
        }));
        transaction.execute(
            "INSERT INTO snapshots (snapshot_id, project_id, parent_snapshot_id, candidate_id, revision, status, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'confirmed', ?6, ?7, ?8)",
            params![
                snapshot_id,
                request.project_id,
                project.1,
                request.candidate_id,
                snapshot_revision,
                manifest_hash,
                snapshot_canonical_sha256,
                now,
            ],
        )?;
        let updated_project = transaction.execute(
            "UPDATE projects SET active_snapshot_revision = ?1, head_snapshot_id = ?2, updated_at = ?3 WHERE project_id = ?4 AND active_snapshot_revision = ?5",
            params![snapshot_revision, snapshot_id, now, request.project_id, project.0],
        )?;
        if updated_project != 1 {
            return Err(StoreError::Contract {
                code: "STALE_BASE_VERSION".to_owned(),
                message: "project head changed during confirm".to_owned(),
            });
        }
        transaction.execute(
            "UPDATE candidates SET state = 'confirmed', error_code = NULL, updated_at = ?1 WHERE candidate_id = ?2 AND state = 'reviewable'",
            params![now, request.candidate_id],
        )?;
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: match approval_tool {
                "restore_confirm" => "restore_confirmed",
                "repair_apply_confirm" => "repair_apply_confirmed",
                "cross_view_promotion_confirm" => "cross_view_candidate_confirmed",
                _ => "candidate_confirmed",
            }
            .to_owned(),
            object_id: Some(request.candidate_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "candidate_id": request.candidate_id,
                "version_id": version_id,
                "snapshot_id": snapshot_id,
                "approval_receipt_id": approval_receipt_id,
                "prepared_object_sha256": request.prepared_object_sha256,
                "quality_report_id": request.quality_report_id,
                "source_version_id": candidate.source_version_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = CandidateConfirmResult {
            schema_version: "CandidateConfirmResult@1".to_owned(),
            candidate_id: request.candidate_id.clone(),
            project_id: request.project_id.clone(),
            version_id,
            snapshot_id,
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            approval_tool,
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn prepare_export(
        &self,
        request: &ExportPrepareRequest,
        now: &str,
    ) -> Result<ExportPrepareResult, StoreError> {
        validate_export_prepare_request(request)?;
        let version =
            self.get_version(&request.version_id)?
                .ok_or_else(|| StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export version not found".to_owned(),
                })?;
        if version.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "export version is outside the requested project".to_owned(),
            });
        }
        let source_candidate =
            self.get_candidate(&version.candidate_id)?
                .ok_or_else(|| StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export source candidate not found".to_owned(),
                })?;
        if source_candidate.state != "confirmed" || !source_candidate.quality_hard_gate_passed {
            return Err(StoreError::Contract {
                code: "EXPORT_SOURCE_UNCONFIRMED".to_owned(),
                message: "export requires a confirmed quality-passing version".to_owned(),
            });
        }
        let source_object = self.get_object(&version.manifest_hash)?;
        if source_object.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "export source manifest object is unavailable".to_owned(),
            });
        }
        if request.format == "glb"
            && source_object
                .as_ref()
                .map(|object| object.mime != "model/gltf-binary")
                .unwrap_or(true)
        {
            return Err(StoreError::Contract {
                code: "EXPORT_FORMAT_UNAVAILABLE".to_owned(),
                message: "mvp-glb export requires a Runtime GLB artifact".to_owned(),
            });
        }
        let export_id = format!("export-{}", Uuid::new_v4().simple());
        let artifact_hashes = vec![version.manifest_hash.clone()];
        let output_kind = if request.format == "glb" {
            "mvp-glb"
        } else {
            "diagnostic-manifest"
        };
        let manifest_payload = serde_json::json!({
            "schema_version": "ExportPayload@1",
            "export_id": export_id,
            "project_id": request.project_id,
            "version_id": request.version_id,
            "format": request.format,
            "profile": request.profile,
            "artifact_hashes": artifact_hashes,
            "license_provenance": {
                "status": if request.format == "glb" { "procedural-mvp" } else { "diagnostic_fixture_unavailable" },
                "absolute_paths": false,
                "source": "runtime-contract-core"
            },
            "toolchain": output_kind
        });
        let manifest_bytes = serde_json::to_vec(&manifest_payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let manifest_object = self.put_object(
            &manifest_bytes,
            None,
            "application/json",
            "export-manifest",
            now,
        )?;
        let request_sha256 = canonical_json_hash(
            &serde_json::to_value(request)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        );
        let manifest = ExportManifestRecord {
            schema_version: "ExportManifest@1".to_owned(),
            export_id: export_id.clone(),
            project_id: request.project_id.clone(),
            version_id: request.version_id.clone(),
            format: request.format.clone(),
            profile: request.profile.clone(),
            manifest_sha256: manifest_object.record.sha256.clone(),
            artifact_hashes: vec![version.manifest_hash.clone()],
            state: "prepared".to_owned(),
            approval_receipt_id: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        validate_export_manifest(&manifest)?;
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: format!("job-{}", Uuid::new_v4().simple()),
            project_id: request.project_id.clone(),
            kind: "export_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: request_sha256.clone(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id: job.job_id.clone(),
            sequence: 1,
            kind: "export_prepared".to_owned(),
            payload: serde_json::json!({
                "export_id": export_id,
                "version_id": request.version_id,
                "manifest_sha256": manifest.manifest_sha256,
            }),
            created_at: now.to_owned(),
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "export_prepared".to_owned(),
            object_id: Some(manifest.export_id.clone()),
            request_sha256: Some(request_sha256),
            payload: serde_json::json!({
                "export_id": manifest.export_id,
                "version_id": manifest.version_id,
                "manifest_sha256": manifest.manifest_sha256,
            }),
            created_at: now.to_owned(),
        };
        validate_job(&job)?;
        validate_job_event(&event)?;
        validate_audit(&audit)?;
        let artifact_hashes_json = serde_json::to_string(&manifest.artifact_hashes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let event_payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO export_manifests (export_id, project_id, version_id, format, profile, manifest_sha256, artifact_hashes_json, state, approval_receipt_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![manifest.export_id, manifest.project_id, manifest.version_id, manifest.format, manifest.profile, manifest.manifest_sha256, artifact_hashes_json, manifest.state, manifest.approval_receipt_id, manifest.created_at, manifest.updated_at],
        )?;
        evaluation_repository::insert_job_and_event_in_transaction(
            &transaction,
            &job,
            &event,
            &event_payload,
            &job.updated_at,
        )?;
        insert_audit(&transaction, &audit)?;
        transaction.commit()?;
        drop(connection);
        let job_summary = self
            .get_job(&job.job_id)?
            .ok_or_else(|| StoreError::InvalidData("export job disappeared".to_owned()))?;
        Ok(ExportPrepareResult {
            schema_version: "ExportPrepareResult@1".to_owned(),
            manifest,
            job: job_summary,
        })
    }

    pub fn confirm_export(
        &self,
        request: &ExportConfirmRequest,
        now: &str,
    ) -> Result<ExportConfirmResult, StoreError> {
        validate_export_confirm_request(request)?;
        let request_sha256 = canonical_json_hash(
            &serde_json::to_value(request)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        );
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
            )
            .optional()?
        {
            if project_id != request.project_id
                || tool != "export_confirm"
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: ExportConfirmResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }
        let manifest =
            read_export_for_transaction(&transaction, &request.export_id)?.ok_or_else(|| {
                StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export manifest not found".to_owned(),
                }
            })?;
        if manifest.project_id != request.project_id
            || manifest.version_id != request.version_id
            || manifest.format != request.format
            || manifest.profile != request.profile
        {
            return Err(StoreError::Contract {
                code: "EXPORT_HASH_MISMATCH".to_owned(),
                message: "export request does not match prepared manifest".to_owned(),
            });
        }
        if manifest.state != "prepared" {
            return Err(StoreError::Contract {
                code: "EXPORT_STATE_INVALID".to_owned(),
                message: "export manifest is not awaiting confirmation".to_owned(),
            });
        }
        let object_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM objects WHERE sha256 = ?1",
                params![manifest.manifest_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared export manifest object is unavailable".to_owned(),
            });
        }
        let approval_receipt_id = generated_approval_receipt_id();
        if is_expired(now, &request.approval_expires_at)? {
            let approval = approval_record(
                request.project_id.as_str(),
                "export_confirm",
                approval_receipt_id.as_str(),
                Some(request.version_id.as_str()),
                request.export_id.as_str(),
                manifest.manifest_sha256.as_str(),
                None,
                request.approval_summary.as_str(),
                "expired",
                request.approval_expires_at.as_str(),
                request.approval_session_id.as_str(),
                now,
            )?;
            insert_approval(&transaction, &approval)?;
            transaction.commit()?;
            return Err(StoreError::Contract {
                code: "APPROVAL_EXPIRED".to_owned(),
                message: "approval receipt expired before export confirm".to_owned(),
            });
        }
        let approval = approval_record(
            request.project_id.as_str(),
            "export_confirm",
            approval_receipt_id.as_str(),
            Some(request.version_id.as_str()),
            request.export_id.as_str(),
            manifest.manifest_sha256.as_str(),
            None,
            request.approval_summary.as_str(),
            "approved",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        for artifact_hash in &manifest.artifact_hashes {
            let marked_reachable = transaction.execute(
                "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
                params![artifact_hash],
            )?;
            if marked_reachable != 1 {
                return Err(StoreError::Contract {
                    code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                    message: "export artifact disappeared before confirm".to_owned(),
                });
            }
        }
        let marked_manifest_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![manifest.manifest_sha256],
        )?;
        if marked_manifest_reachable != 1 {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared export manifest disappeared before confirm".to_owned(),
            });
        }
        transaction.execute(
            "UPDATE export_manifests SET state = 'confirmed', approval_receipt_id = ?1, updated_at = ?2 WHERE export_id = ?3 AND state = 'prepared'",
            params![approval_receipt_id, now, request.export_id],
        )?;
        let output_sha256 = if manifest.format == "glb" {
            manifest
                .artifact_hashes
                .first()
                .cloned()
                .unwrap_or_else(|| manifest.manifest_sha256.clone())
        } else {
            manifest.manifest_sha256.clone()
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "export_confirmed".to_owned(),
            object_id: Some(request.export_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "export_id": request.export_id,
                "version_id": request.version_id,
                "manifest_sha256": manifest.manifest_sha256,
                "output_sha256": output_sha256.clone(),
                "approval_receipt_id": approval_receipt_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = ExportConfirmResult {
            schema_version: "ExportConfirmResult@1".to_owned(),
            export_id: request.export_id.clone(),
            project_id: request.project_id.clone(),
            version_id: request.version_id.clone(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            output_sha256,
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            "export_confirm",
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn reject_candidate(
        &self,
        request: &CandidateRejectRequest,
        now: &str,
    ) -> Result<CandidateRejectResult, StoreError> {
        validate_reject_request(request)?;
        let request_value = serde_json::to_value(request)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let request_sha256 = canonical_json_hash(&request_value);
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
        {
            if project_id != request.project_id
                || tool != "candidate_reject"
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: CandidateRejectResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }
        let candidate = read_candidate_for_transaction(&transaction, &request.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "candidate not found".to_owned(),
            })?;
        if candidate.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "candidate is outside the requested project".to_owned(),
            });
        }
        if candidate.state == "confirmed" {
            return Err(StoreError::Contract {
                code: "CANDIDATE_ALREADY_CONFIRMED".to_owned(),
                message: "confirmed candidate cannot be rejected".to_owned(),
            });
        }
        let prepared_object_id =
            candidate
                .prepared_object_id
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "candidate has no prepared object".to_owned(),
                })?;
        let prepared_object_sha256 =
            candidate
                .prepared_object_sha256
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "candidate has no prepared object hash".to_owned(),
                })?;
        let approval_receipt_id = generated_approval_receipt_id();
        let approval = approval_record(
            request.project_id.as_str(),
            "candidate_reject",
            approval_receipt_id.as_str(),
            candidate.base_version_id.as_deref(),
            prepared_object_id.as_str(),
            prepared_object_sha256.as_str(),
            candidate.quality_report_id.as_deref(),
            request.approval_summary.as_str(),
            "rejected",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        transaction.execute(
            "UPDATE candidates SET state = 'rejected', error_code = NULL, updated_at = ?1 WHERE candidate_id = ?2",
            params![now, request.candidate_id],
        )?;
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "candidate_rejected".to_owned(),
            object_id: Some(request.candidate_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "candidate_id": request.candidate_id,
                "approval_receipt_id": approval_receipt_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = CandidateRejectResult {
            schema_version: "CandidateRejectResult@1".to_owned(),
            candidate_id: request.candidate_id.clone(),
            project_id: request.project_id.clone(),
            state: "rejected".to_owned(),
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            "candidate_reject",
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_lifecycle_rejects_invalid_requests_before_any_lifecycle_row() {
        let store = Store::memory().expect("store");
        let confirm = CandidateConfirmRequest {
            project_id: String::new(),
            candidate_id: String::new(),
            base_version_id: None,
            prepared_object_id: String::new(),
            prepared_object_sha256: String::new(),
            quality_report_id: String::new(),
            approval_receipt_id: String::new(),
            approval_summary: String::new(),
            approval_session_id: String::new(),
            approval_expires_at: String::new(),
            idempotency_key: String::new(),
        };
        let reject = CandidateRejectRequest {
            project_id: String::new(),
            candidate_id: String::new(),
            approval_receipt_id: String::new(),
            approval_summary: String::new(),
            approval_session_id: String::new(),
            approval_expires_at: String::new(),
            idempotency_key: String::new(),
        };
        let export_prepare = ExportPrepareRequest {
            project_id: String::new(),
            version_id: String::new(),
            format: String::new(),
            profile: String::new(),
            request: Value::Null,
        };
        let export_confirm = ExportConfirmRequest {
            project_id: String::new(),
            export_id: String::new(),
            version_id: String::new(),
            format: String::new(),
            profile: String::new(),
            approval_receipt_id: String::new(),
            approval_summary: String::new(),
            approval_session_id: String::new(),
            approval_expires_at: String::new(),
            idempotency_key: String::new(),
        };
        let invalid_version = DesignAssetVersionRecord {
            schema_version: String::new(),
            version_id: String::new(),
            project_id: String::new(),
            parent_version_id: None,
            candidate_id: String::new(),
            manifest_hash: String::new(),
            canonical_sha256: String::new(),
            created_at: String::new(),
        };

        assert!(matches!(
            store.confirm_candidate(&confirm, "now"),
            Err(StoreError::InvalidData(_))
        ));
        assert!(matches!(
            store.reject_candidate(&reject, "now"),
            Err(StoreError::InvalidData(_))
        ));
        assert!(matches!(
            store.prepare_export(&export_prepare, "now"),
            Err(StoreError::InvalidData(_))
        ));
        assert!(matches!(
            store.confirm_export(&export_confirm, "now"),
            Err(StoreError::InvalidData(_))
        ));
        assert!(matches!(
            store.insert_version(&invalid_version),
            Err(StoreError::InvalidData(_))
        ));
        assert!(store.list_versions(None).expect("versions").is_empty());
        assert!(store
            .get_object("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("CAS lookup")
            .is_none());
    }
}
