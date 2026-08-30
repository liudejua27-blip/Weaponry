//! Runtime-owned durable index for the typed FPS foundation importer.
//!
//! The importer deliberately has a different persistence boundary from the
//! candidate-bound AuthoringMesh records.  A foundation asset is an
//! allowlisted, offline source projection; until every Part can be materialized
//! as a bounded AuthoringMesh revision it must not be presented as candidate
//! topology or as a production asset.  This module therefore stores only a
//! hash-only link to compact CAS children and keeps the materialization state
//! explicit.

use forgecad_contracts::{CasObjectRecord, is_opaque_id, is_sha256};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Store, StoreError};

pub const RECORD_SCHEMA_VERSION: &str = "WeaponFoundationImportRecord@1";
pub const LINK_SCHEMA_VERSION: &str = "WeaponFoundationImportLink@1";
pub const RESULT_SCHEMA_VERSION: &str = "WeaponFoundationAssetResult@1";
pub const TABLE: &str = "weapon_foundation_imports";

pub const TOPOLOGY_OBJECT_KIND: &str = "forgecad-foundation-topology";
pub const SOCKET_MAP_OBJECT_KIND: &str = "forgecad-foundation-socket-map";
pub const RIG_MAP_OBJECT_KIND: &str = "forgecad-foundation-rig-map";
pub const PRESENTATION_PACKAGE_OBJECT_KIND: &str = "forgecad-fps-presentation-package";
pub const RESULT_OBJECT_KIND: &str = "forgecad-foundation-import-result";
pub const LINK_OBJECT_KIND: &str = "forgecad-foundation-import-link";
pub const JSON_MIME: &str = "application/json";
pub const TOPOLOGY_MIME: &str = "application/json";
pub const MAX_JSON_BYTES: u64 = 1_048_576;
pub const MAX_TOPOLOGY_BYTES: u64 = 16 * 1024 * 1024;
pub const MATERIALIZATION_PENDING: &str = "AUTHORING_MESH_MATERIALIZATION_PENDING";
pub const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
pub const FOUNDATION_PACK_ID: &str = "forgecad-fps-production-foundation";
pub const FOUNDATION_PACK_VERSION: &str = "0.1.0-proposal";
pub const FOUNDATION_MANIFEST_SHA256: &str =
    "cc7dccca305a1d9bbaf1df80e78e9cab6b2ee39f12de7ffc88d5cf52194330cb";

/// The only source assets admitted by the evaluation foundation pack.  Keep
/// this table in Store as a second line of defence: a caller cannot turn an
/// arbitrary CAS hash into a foundation import by crafting a Runtime record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowlistedAsset {
    pub asset_id: &'static str,
    pub asset_sha256: &'static str,
    pub source_format: &'static str,
    pub asset_role: &'static str,
}

pub const ALLOWLISTED_ASSETS: &[AllowlistedAsset] = &[
    AllowlistedAsset {
        asset_id: "kenney-blaster-a",
        asset_sha256: "d6d6fe0ec5baf21d7717449220799d45b95d2d663ace7b22612b255dc1a8b308",
        source_format: "glb",
        asset_role: "silhouette-family-reference",
    },
    AllowlistedAsset {
        asset_id: "kenney-blaster-r",
        asset_sha256: "ab0b5f51ecff405238727626d558414750a9946d5e255cd1dc8bf81eebe1f1e2",
        source_format: "glb",
        asset_role: "silhouette-family-reference",
    },
    AllowlistedAsset {
        asset_id: "quaternius-body-ar-1",
        asset_sha256: "7d0854e5aee2453efd47cf3a80495cc96b1da8f6476a590d68488beacd86c97c",
        source_format: "gltf",
        asset_role: "modular-body-reference",
    },
    AllowlistedAsset {
        asset_id: "quaternius-barrel-ar-1",
        asset_sha256: "be9309266e448a755c5c35853c82d79fc68633e9e6655f68122f93b55a3a091c",
        source_format: "glb",
        asset_role: "modular-barrel-reference",
    },
    AllowlistedAsset {
        asset_id: "quaternius-grip-ar-1",
        asset_sha256: "81e4c666a5891762012d0e76f24abfe09b0ec6fb4c508f9fbf1b67630fc33564",
        source_format: "gltf",
        asset_role: "modular-grip-reference",
    },
    AllowlistedAsset {
        asset_id: "quaternius-magazine-ar",
        asset_sha256: "e7cc5615a292cacec7d8a6b487b0b8f233a712e643210e622c7c846cfd3677af",
        source_format: "gltf",
        asset_role: "modular-magazine-reference",
    },
    AllowlistedAsset {
        asset_id: "quaternius-stock-2",
        asset_sha256: "e2d9d4ac7da3c9a079d6cb60d30fb22d946aa63b212df118bc803af5df3480ef",
        source_format: "gltf",
        asset_role: "modular-stock-reference",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-weapon-west",
        asset_sha256: "0d80dd2118c884172a856455968be14eadc97f041d27d52bfa75fedb708fa486",
        source_format: "glb",
        asset_role: "rigged-weapon-semantic-source",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-weapon-east",
        asset_sha256: "8b3d3f90afbff9a699c3e2a14574ff0c5a687f1faf6dc4b51ba4a5629ea07783",
        source_format: "glb",
        asset_role: "rigged-weapon-semantic-source",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-optic-holographic",
        asset_sha256: "b94436754da9b7550f85c72adefb857090cb8299b3bc8d99cc339ad47f166987",
        source_format: "glb",
        asset_role: "attachment-reference",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-optic-scope",
        asset_sha256: "a3c20e2c59273abd280f1a5ddb4f3ec104ca46fac3c06cbd2cf81664ee7cf195",
        source_format: "glb",
        asset_role: "attachment-reference",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-muzzle-device",
        asset_sha256: "0b7a598791ae01f212efd1df9817227b6514c52287c870bc72639dfff83897a4",
        source_format: "glb",
        asset_role: "attachment-reference",
    },
    AllowlistedAsset {
        asset_id: "pichuliru-foregrip",
        asset_sha256: "0980af948e14a6023c2e0752bbf2cdc4ed3c57a44c21adb484b6b73322f4fbb8",
        source_format: "glb",
        asset_role: "attachment-reference",
    },
    AllowlistedAsset {
        asset_id: "lightning-low-pbr",
        asset_sha256: "3f84f2b0d011ebfb142de7f7d9cfa7d57a59451a815b834b4f33603256c8f911",
        source_format: "glb",
        asset_role: "high-low-bake-pbr-animation-benchmark",
    },
    AllowlistedAsset {
        asset_id: "wrad-arms",
        asset_sha256: "580efbb0852bf0b41f82dd3e17eafec86b3d2a48f4a7acaa7e64d60e850f565d",
        source_format: "glb",
        asset_role: "first-person-armature-source",
    },
];

pub fn allowlisted_asset(asset_id: &str) -> Option<AllowlistedAsset> {
    ALLOWLISTED_ASSETS
        .iter()
        .copied()
        .find(|asset| asset.asset_id == asset_id)
}

/// Store-local row.  The immutable public result remains in CAS; this row is
/// only a restart-safe lookup and reachability index.  It intentionally has no
/// project/candidate foreign key because the evaluation foundation is not a
/// candidate and cannot be promoted by this slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponFoundationImportRecord {
    pub schema_version: String,
    pub request_id: String,
    pub request_sha256: String,
    pub foundation_pack_id: String,
    pub foundation_pack_version: String,
    pub foundation_manifest_sha256: String,
    pub asset_id: String,
    pub asset_sha256: String,
    pub asset_role: String,
    pub source_format: String,
    pub coordinate_spec_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub result_object_sha256: String,
    pub link_object_sha256: String,
    pub authoring_mesh_materialization_status: String,
    pub socket_materialization_status: String,
    pub rig_materialization_status: String,
    pub presentation_materialization_status: String,
    pub import_status: String,
    pub quality_status: String,
    pub promotion_eligible: bool,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub human_review_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = 'WeaponFoundationImportRecord@1'),
            request_id TEXT PRIMARY KEY,
            request_sha256 TEXT NOT NULL,
            foundation_pack_id TEXT NOT NULL CHECK (foundation_pack_id = 'forgecad-fps-production-foundation'),
            foundation_pack_version TEXT NOT NULL CHECK (foundation_pack_version = '0.1.0-proposal'),
            foundation_manifest_sha256 TEXT NOT NULL,
            asset_id TEXT NOT NULL,
            asset_sha256 TEXT NOT NULL,
            asset_role TEXT NOT NULL,
            source_format TEXT NOT NULL CHECK (source_format IN ('glb', 'gltf')),
            coordinate_spec_sha256 TEXT NOT NULL,
            topology_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            socket_map_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            rig_map_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            fps_presentation_package_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            link_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            authoring_mesh_materialization_status TEXT NOT NULL CHECK (authoring_mesh_materialization_status = 'AUTHORING_MESH_MATERIALIZATION_PENDING'),
            socket_materialization_status TEXT NOT NULL,
            rig_materialization_status TEXT NOT NULL,
            presentation_materialization_status TEXT NOT NULL,
            import_status TEXT NOT NULL CHECK (import_status IN ('IMPORTED_DRAFT', 'REJECTED')),
            quality_status TEXT NOT NULL CHECK (quality_status = 'structural_only'),
            promotion_eligible INTEGER NOT NULL CHECK (promotion_eligible = 0),
            runtime_write_performed INTEGER NOT NULL CHECK (runtime_write_performed IN (0, 1)),
            candidate_confirmed INTEGER NOT NULL CHECK (candidate_confirmed = 0),
            version_created INTEGER NOT NULL CHECK (version_created = 0),
            export_performed INTEGER NOT NULL CHECK (export_performed = 0),
            actual_engine_roundtrip INTEGER NOT NULL CHECK (actual_engine_roundtrip = 0),
            human_review_status TEXT NOT NULL CHECK (human_review_status = 'NOT_RUN'),
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            UNIQUE (asset_id, request_sha256)
        );
        CREATE INDEX IF NOT EXISTS weapon_foundation_import_asset_idx
            ON {TABLE}(asset_id, created_at DESC, request_id ASC);
        CREATE INDEX IF NOT EXISTS weapon_foundation_import_object_idx
            ON {TABLE}(topology_object_sha256, socket_map_object_sha256,
                       rig_map_object_sha256, fps_presentation_package_object_sha256,
                       result_object_sha256, link_object_sha256);"
    ))?;
    Ok(())
}

fn record_value(record: &WeaponFoundationImportRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

pub fn canonical_record_sha256(
    record: &WeaponFoundationImportRecord,
) -> Result<String, StoreError> {
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn record_json(record: &WeaponFoundationImportRecord) -> Result<String, StoreError> {
    let value = record_value(record)?;
    let bytes =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(contract(
            "WEAPON_FOUNDATION_RECORD_TOO_LARGE",
            "foundation import Store record exceeds the bounded JSON limit",
        ));
    }
    String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &WeaponFoundationImportRecord) -> Result<(), StoreError> {
    let Some(asset) = allowlisted_asset(&record.asset_id) else {
        return Err(contract(
            "WEAPON_FOUNDATION_ASSET_NOT_ALLOWLISTED",
            "foundation asset_id is not in the closed evaluation allowlist",
        ));
    };
    if record.schema_version != RECORD_SCHEMA_VERSION
        || !is_opaque_id(&record.request_id)
        || !is_sha256(&record.request_sha256)
        || record.foundation_pack_id != FOUNDATION_PACK_ID
        || record.foundation_pack_version != FOUNDATION_PACK_VERSION
        || record.foundation_manifest_sha256 != FOUNDATION_MANIFEST_SHA256
        || record.asset_sha256 != asset.asset_sha256
        || record.asset_role != asset.asset_role
        || record.source_format != asset.source_format
        || !is_sha256(&record.coordinate_spec_sha256)
        || !is_sha256(&record.topology_object_sha256)
        || !is_sha256(&record.socket_map_object_sha256)
        || !is_sha256(&record.rig_map_object_sha256)
        || !is_sha256(&record.fps_presentation_package_object_sha256)
        || !is_sha256(&record.result_object_sha256)
        || !is_sha256(&record.link_object_sha256)
        || record.authoring_mesh_materialization_status != MATERIALIZATION_PENDING
        || record.quality_status != "structural_only"
        || record.promotion_eligible
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.actual_engine_roundtrip
        || record.human_review_status != "NOT_RUN"
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "WEAPON_FOUNDATION_RECORD_INVALID",
            "foundation import identity, status or binding is malformed",
        ));
    }
    if canonical_record_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "WEAPON_FOUNDATION_RECORD_CANONICAL_MISMATCH",
            "foundation import Store record canonical hash differs",
        ));
    }
    Ok(())
}

fn expected_roots(record: &WeaponFoundationImportRecord) -> [(&str, &str, &str, u64); 6] {
    [
        (
            record.topology_object_sha256.as_str(),
            TOPOLOGY_MIME,
            TOPOLOGY_OBJECT_KIND,
            MAX_TOPOLOGY_BYTES,
        ),
        (
            record.socket_map_object_sha256.as_str(),
            JSON_MIME,
            SOCKET_MAP_OBJECT_KIND,
            MAX_JSON_BYTES,
        ),
        (
            record.rig_map_object_sha256.as_str(),
            JSON_MIME,
            RIG_MAP_OBJECT_KIND,
            MAX_JSON_BYTES,
        ),
        (
            record.fps_presentation_package_object_sha256.as_str(),
            JSON_MIME,
            PRESENTATION_PACKAGE_OBJECT_KIND,
            MAX_JSON_BYTES,
        ),
        (
            record.result_object_sha256.as_str(),
            JSON_MIME,
            RESULT_OBJECT_KIND,
            MAX_JSON_BYTES,
        ),
        (
            record.link_object_sha256.as_str(),
            JSON_MIME,
            LINK_OBJECT_KIND,
            MAX_JSON_BYTES,
        ),
    ]
}

fn validate_registered_root(
    store: &Store,
    transaction: &Transaction<'_>,
    hash: &str,
    mime: &str,
    kind: &str,
    max_bytes: u64,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let row: Option<(i64, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![hash],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size, actual_mime, actual_kind, reachability)) = row else {
        return Err(contract(
            "WEAPON_FOUNDATION_CAS_OBJECT_MISSING",
            "foundation import references a missing CAS object",
        ));
    };
    if size <= 0
        || u64::try_from(size).unwrap_or(u64::MAX) > max_bytes
        || actual_mime != mime
        || actual_kind != kind
        || (require_reachable && reachability != "reachable")
        || (!require_reachable && !matches!(reachability.as_str(), "temporary" | "reachable"))
    {
        return Err(contract(
            "WEAPON_FOUNDATION_CAS_METADATA_INVALID",
            "foundation import CAS metadata is outside the closed binding",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(hash, max_bytes)
        .map_err(StoreError::Cas)?;
    if bytes.len() as i64 != size || sha256_hex(&bytes) != hash {
        return Err(contract(
            "WEAPON_FOUNDATION_CAS_HASH_MISMATCH",
            "foundation import CAS bytes do not match their registered hash",
        ));
    }
    if mime == JSON_MIME {
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            contract(
                "WEAPON_FOUNDATION_CAS_JSON_INVALID",
                format!("foundation import JSON CAS is invalid: {error}"),
            )
        })?;
        let canonical = canonical_json_bytes(&value)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if canonical != bytes {
            let first_difference = canonical
                .iter()
                .zip(bytes.iter())
                .position(|(left, right)| left != right)
                .unwrap_or_else(|| canonical.len().min(bytes.len()));
            return Err(contract(
                "WEAPON_FOUNDATION_CAS_JSON_NON_CANONICAL",
                format!(
                    "foundation import JSON CAS object {kind} ({hash}) must use canonical JSON bytes; first_difference={first_difference}, stored_bytes={}, canonical_bytes={}",
                    bytes.len(),
                    canonical.len()
                ),
            ));
        }
    }
    Ok(())
}

fn validate_result_and_link(
    store: &Store,
    transaction: &Transaction<'_>,
    record: &WeaponFoundationImportRecord,
    require_reachable: bool,
) -> Result<(), StoreError> {
    for (hash, mime, kind, max_bytes) in expected_roots(record) {
        validate_registered_root(
            store,
            transaction,
            hash,
            mime,
            kind,
            max_bytes,
            require_reachable,
        )?;
    }
    let result_bytes = store
        .cas
        .read_verified_bounded(&record.result_object_sha256, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    let result: Value = serde_json::from_slice(&result_bytes).map_err(|error| {
        contract(
            "WEAPON_FOUNDATION_RESULT_INVALID",
            format!("foundation result JSON is invalid: {error}"),
        )
    })?;
    if result.get("schema_version").and_then(Value::as_str) != Some(RESULT_SCHEMA_VERSION)
        || result.get("request_id").and_then(Value::as_str) != Some(record.request_id.as_str())
        || result.get("request_sha256").and_then(Value::as_str)
            != Some(record.request_sha256.as_str())
        || result.get("asset_id").and_then(Value::as_str) != Some(record.asset_id.as_str())
        || result.get("asset_sha256").and_then(Value::as_str) != Some(record.asset_sha256.as_str())
        || result.get("topology_object_sha256").and_then(Value::as_str)
            != Some(record.topology_object_sha256.as_str())
        || result
            .get("socket_map_object_sha256")
            .and_then(Value::as_str)
            != Some(record.socket_map_object_sha256.as_str())
        || result.get("rig_map_object_sha256").and_then(Value::as_str)
            != Some(record.rig_map_object_sha256.as_str())
        || result
            .get("fps_presentation_package_object_sha256")
            .and_then(Value::as_str)
            != Some(record.fps_presentation_package_object_sha256.as_str())
        || result
            .get("authoring_mesh_materialization_status")
            .and_then(Value::as_str)
            != Some(MATERIALIZATION_PENDING)
        || result.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || result.get("promotion_eligible").and_then(Value::as_bool) != Some(false)
        || result.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || result.get("version_created").and_then(Value::as_bool) != Some(false)
        || result.get("export_performed").and_then(Value::as_bool) != Some(false)
        || result
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || result.get("human_review_status").and_then(Value::as_str) != Some("NOT_RUN")
    {
        return Err(contract(
            "WEAPON_FOUNDATION_RESULT_BINDING_MISMATCH",
            "foundation result does not match its Store link",
        ));
    }
    let mut result_without_hash = result.clone();
    result_without_hash["canonical_sha256"] = Value::String(String::new());
    if result.get("canonical_sha256").and_then(Value::as_str)
        != Some(canonical_json_hash(&result_without_hash).as_str())
    {
        return Err(contract(
            "WEAPON_FOUNDATION_RESULT_CANONICAL_MISMATCH",
            "foundation result canonical hash differs",
        ));
    }

    let link_bytes = store
        .cas
        .read_verified_bounded(&record.link_object_sha256, MAX_JSON_BYTES)
        .map_err(StoreError::Cas)?;
    let link: Value = serde_json::from_slice(&link_bytes).map_err(|error| {
        contract(
            "WEAPON_FOUNDATION_LINK_INVALID",
            format!("foundation link JSON is invalid: {error}"),
        )
    })?;
    if link.get("schema_version").and_then(Value::as_str) != Some(LINK_SCHEMA_VERSION)
        || link.get("request_id").and_then(Value::as_str) != Some(record.request_id.as_str())
        || link.get("request_sha256").and_then(Value::as_str)
            != Some(record.request_sha256.as_str())
        || link.get("asset_id").and_then(Value::as_str) != Some(record.asset_id.as_str())
        || link.get("result_object_sha256").and_then(Value::as_str)
            != Some(record.result_object_sha256.as_str())
        || link
            .get("authoring_mesh_materialization_status")
            .and_then(Value::as_str)
            != Some(MATERIALIZATION_PENDING)
        || link.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
    {
        return Err(contract(
            "WEAPON_FOUNDATION_LINK_BINDING_MISMATCH",
            "foundation link does not match its Store row",
        ));
    }
    let mut link_without_hash = link.clone();
    link_without_hash["canonical_sha256"] = Value::String(String::new());
    if link.get("canonical_sha256").and_then(Value::as_str)
        != Some(canonical_json_hash(&link_without_hash).as_str())
    {
        return Err(contract(
            "WEAPON_FOUNDATION_LINK_CANONICAL_MISMATCH",
            "foundation link canonical hash differs",
        ));
    }
    Ok(())
}

fn read_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WeaponFoundationImportRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

impl Store {
    /// Create the immutable Store link and promote all six CAS roots in one
    /// SQLite transaction.  A replay with the same request_id and request
    /// hash returns the original row without changing reachability or JSON.
    pub fn record_weapon_foundation_import(
        &self,
        record: &WeaponFoundationImportRecord,
        owned_objects: &[CasObjectRecord],
    ) -> Result<(WeaponFoundationImportRecord, bool), StoreError> {
        validate_record(record)?;
        let payload = record_json(record)?;
        let roots = expected_roots(record);
        if owned_objects.len() != roots.len()
            || owned_objects
                .iter()
                .map(|object| object.sha256.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != roots.len()
            || roots
                .iter()
                .any(|(hash, _, _, _)| !owned_objects.iter().any(|object| object.sha256 == *hash))
        {
            return Err(contract(
                "WEAPON_FOUNDATION_CAS_ROOT_SET_INVALID",
                "foundation import must own exactly its six declared CAS roots",
            ));
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT record_json FROM weapon_foundation_imports WHERE request_id = ?1",
                params![record.request_id],
                read_row,
            )
            .optional()?
        {
            validate_record(&existing)?;
            if existing.request_sha256 != record.request_sha256 {
                return Err(contract(
                    "IDEMPOTENCY_KEY_REUSED",
                    "foundation request_id is bound to a different request hash",
                ));
            }
            validate_result_and_link(self, &transaction, &existing, true)?;
            transaction.rollback()?;
            return Ok((existing, true));
        }
        for (hash, mime, kind, max_bytes) in roots {
            validate_registered_root(self, &transaction, hash, mime, kind, max_bytes, false)?;
        }
        transaction.execute(
            "INSERT INTO weapon_foundation_imports (schema_version, request_id, request_sha256, foundation_pack_id, foundation_pack_version, foundation_manifest_sha256, asset_id, asset_sha256, asset_role, source_format, coordinate_spec_sha256, topology_object_sha256, socket_map_object_sha256, rig_map_object_sha256, fps_presentation_package_object_sha256, result_object_sha256, link_object_sha256, authoring_mesh_materialization_status, socket_materialization_status, rig_materialization_status, presentation_materialization_status, import_status, quality_status, promotion_eligible, runtime_write_performed, candidate_confirmed, version_created, export_performed, actual_engine_roundtrip, human_review_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)",
            params![
                record.schema_version,
                record.request_id,
                record.request_sha256,
                record.foundation_pack_id,
                record.foundation_pack_version,
                record.foundation_manifest_sha256,
                record.asset_id,
                record.asset_sha256,
                record.asset_role,
                record.source_format,
                record.coordinate_spec_sha256,
                record.topology_object_sha256,
                record.socket_map_object_sha256,
                record.rig_map_object_sha256,
                record.fps_presentation_package_object_sha256,
                record.result_object_sha256,
                record.link_object_sha256,
                record.authoring_mesh_materialization_status,
                record.socket_materialization_status,
                record.rig_materialization_status,
                record.presentation_materialization_status,
                record.import_status,
                record.quality_status,
                record.promotion_eligible,
                record.runtime_write_performed,
                record.candidate_confirmed,
                record.version_created,
                record.export_performed,
                record.actual_engine_roundtrip,
                record.human_review_status,
                record.canonical_sha256,
                record.created_at,
                payload,
            ],
        )?;
        for (hash, _, _, _) in expected_roots(record) {
            let updated = transaction.execute(
                "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
                params![hash],
            )?;
            if updated != 1 {
                return Err(contract(
                    "WEAPON_FOUNDATION_CAS_OBJECT_UNAVAILABLE",
                    "foundation import could not promote a CAS root",
                ));
            }
        }
        transaction.commit()?;
        Ok((record.clone(), false))
    }

    /// Read and re-verify a foundation import after a Runtime restart.  This
    /// path never repairs reachability and never returns source bytes.
    pub fn get_weapon_foundation_import(
        &self,
        request_id: &str,
        expected_request_sha256: Option<&str>,
    ) -> Result<Option<WeaponFoundationImportRecord>, StoreError> {
        if !is_opaque_id(request_id) || expected_request_sha256.is_some_and(|hash| !is_sha256(hash))
        {
            return Err(StoreError::InvalidData(
                "foundation import lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = transaction
            .query_row(
                "SELECT record_json FROM weapon_foundation_imports WHERE request_id = ?1",
                params![request_id],
                read_row,
            )
            .optional()?
        else {
            transaction.rollback()?;
            return Ok(None);
        };
        validate_record(&record)?;
        if expected_request_sha256.is_some_and(|hash| hash != record.request_sha256) {
            return Err(contract(
                "FOUNDATION_REQUEST_HASH_MISMATCH",
                "foundation lookup request hash differs from the durable row",
            ));
        }
        validate_result_and_link(self, &transaction, &record, true)?;
        transaction.rollback()?;
        Ok(Some(record))
    }

    pub fn get_weapon_foundation_import_by_result(
        &self,
        result_object_sha256: &str,
    ) -> Result<Option<WeaponFoundationImportRecord>, StoreError> {
        if !is_sha256(result_object_sha256) {
            return Err(StoreError::InvalidData(
                "foundation result object hash is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let record = transaction
            .query_row(
                "SELECT record_json FROM weapon_foundation_imports WHERE result_object_sha256 = ?1",
                params![result_object_sha256],
                read_row,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.rollback()?;
            return Ok(None);
        };
        validate_record(&record)?;
        validate_result_and_link(self, &transaction, &record, true)?;
        transaction.rollback()?;
        Ok(Some(record))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_is_closed_and_hash_bound() {
        assert_eq!(
            allowlisted_asset("pichuliru-weapon-west")
                .unwrap()
                .source_format,
            "glb"
        );
        assert!(allowlisted_asset("arbitrary-path.glb").is_none());
        assert!(
            ALLOWLISTED_ASSETS
                .iter()
                .all(|asset| is_sha256(asset.asset_sha256))
        );
    }
}
