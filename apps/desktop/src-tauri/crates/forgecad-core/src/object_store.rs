use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    filesystem_permissions::{
        create_private_file, ensure_existing_regular_file, ensure_private_directory_tree,
        ensure_private_file, ensure_private_file_if_present,
    },
    CoreError, CoreResult,
};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static PROMOTE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoredObject {
    pub sha256: String,
    pub relative_path: String,
    pub extension: String,
    pub byte_size: u64,
}

#[derive(Debug)]
pub struct StagedObject {
    stored: StoredObject,
    staging_path: Option<PathBuf>,
    final_path: PathBuf,
    pending_root: PathBuf,
}

impl StagedObject {
    pub fn metadata(&self) -> &StoredObject {
        &self.stored
    }

    pub fn promote(mut self) -> CoreResult<PromotedObject> {
        // Cloned repositories may stage the same SHA concurrently. Serialize
        // only the short journal/rename window so one call creates the final
        // object and every other call verifies and deduplicates it.
        let _promote_guard = PROMOTE_LOCK.lock().map_err(|_| {
            CoreError::conflict(
                "CONTENT_OBJECT_PROMOTE_LOCK_POISONED",
                "Content object promotion lock is unavailable.",
            )
        })?;
        let staging_path = self.staging_path.take().ok_or_else(|| {
            CoreError::conflict(
                "CONTENT_OBJECT_STAGE_CONSUMED",
                "The staged content object was already consumed.",
            )
        })?;
        if let Some(parent) = self.final_path.parent() {
            ensure_private_directory_tree(parent)?;
        }

        let final_exists = match fs::symlink_metadata(&self.final_path) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        let (newly_created, pending_path) = if final_exists {
            ensure_private_file(&self.final_path)?;
            verify_file(&self.final_path, &self.stored.sha256, self.stored.byte_size)?;
            fs::remove_file(&staging_path)?;
            (false, None)
        } else {
            // The durable journal is created before rename. On a crash, startup
            // can distinguish Rust-created orphan files from historical files
            // that predate the core object index.
            let pending_path = self.pending_root.join(format!(
                "{}.{}.pending.json",
                self.stored.sha256, self.stored.extension
            ));
            let mut pending = create_private_file(&pending_path)?;
            let journal = serde_json::to_vec(&self.stored).map_err(|_| {
                CoreError::invalid_data(
                    "CONTENT_OBJECT_JOURNAL_INVALID",
                    "Pending object journal could not be serialized.",
                )
            })?;
            pending.write_all(&journal)?;
            pending.sync_all()?;
            sync_directory(&self.pending_root)?;

            fs::rename(&staging_path, &self.final_path)?;
            ensure_private_file(&self.final_path)?;
            OpenOptions::new()
                .read(true)
                .open(&self.final_path)?
                .sync_all()?;
            if let Some(parent) = self.final_path.parent() {
                sync_directory(parent)?;
            }
            (true, Some(pending_path))
        };

        Ok(PromotedObject {
            stored: self.stored.clone(),
            final_path: self.final_path.clone(),
            newly_created,
            pending_path,
            pending_root: self.pending_root.clone(),
        })
    }
}

impl Drop for StagedObject {
    fn drop(&mut self) {
        if let Some(path) = self.staging_path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Debug)]
pub struct PromotedObject {
    stored: StoredObject,
    final_path: PathBuf,
    newly_created: bool,
    pending_path: Option<PathBuf>,
    pending_root: PathBuf,
}

impl PromotedObject {
    pub fn metadata(&self) -> &StoredObject {
        &self.stored
    }

    pub fn newly_created(&self) -> bool {
        self.newly_created
    }

    /// Removes the pending marker only after SQLite has committed the object
    /// row/reference. If this call is interrupted, recovery sees the committed
    /// SHA and removes only the marker.
    pub(crate) fn finalize_commit(&mut self) -> CoreResult<()> {
        if let Some(path) = self.pending_path.take() {
            match fs::remove_file(path) {
                Ok(()) => sync_directory(&self.pending_root)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.newly_created = false;
        Ok(())
    }

    /// Removes a just-promoted file after the owning SQLite transaction fails.
    /// This is safe under the process-wide single-writer lease.
    pub(crate) fn cleanup_after_rollback(&mut self) {
        if self.newly_created {
            let _ = fs::remove_file(&self.final_path);
            self.newly_created = false;
        }
        if let Some(path) = self.pending_path.take() {
            let _ = fs::remove_file(path);
        }
        if let Some(parent) = self.final_path.parent() {
            let _ = sync_directory(parent);
        }
        let _ = sync_directory(&self.pending_root);
    }
}

#[derive(Debug, Clone)]
pub struct ContentAddressedObjectStore {
    root: PathBuf,
    staging_root: PathBuf,
    pending_root: PathBuf,
}

impl ContentAddressedObjectStore {
    pub fn new(library_root: impl AsRef<Path>) -> CoreResult<Self> {
        let library_root = library_root.as_ref();
        ensure_private_directory_tree(library_root)?;
        let library_root = library_root.canonicalize()?;
        let root = library_root.join("objects").join("sha256");
        let staging_root = library_root.join("objects").join(".staging");
        let pending_root = library_root.join("objects").join(".pending");
        ensure_private_directory_tree(&root)?;
        ensure_private_directory_tree(&staging_root)?;
        ensure_private_directory_tree(&pending_root)?;
        Ok(Self {
            root,
            staging_root,
            pending_root,
        })
    }

    pub fn stage(&self, bytes: &[u8], extension: &str) -> CoreResult<StagedObject> {
        let extension = validate_extension(extension)?;
        let sha256 = hex_sha256(bytes);
        let relative_path = object_relative_path(&sha256, &extension);
        let final_path = self.root.join(&relative_path);
        ensure_within(&self.root, &final_path)?;

        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging_path = self.staging_root.join(format!(
            "{}.{}.{}.stage",
            sha256,
            std::process::id(),
            sequence
        ));
        let mut file = create_private_file(&staging_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        sync_directory(&self.staging_root)?;
        verify_file(&staging_path, &sha256, bytes.len() as u64)?;

        Ok(StagedObject {
            stored: StoredObject {
                sha256,
                relative_path,
                extension,
                byte_size: bytes.len() as u64,
            },
            staging_path: Some(staging_path),
            final_path,
            pending_root: self.pending_root.clone(),
        })
    }

    pub fn read(&self, stored: &StoredObject) -> CoreResult<Vec<u8>> {
        validate_sha256(&stored.sha256)?;
        let extension = validate_extension(&stored.extension)?;
        let expected_relative = object_relative_path(&stored.sha256, &extension);
        if stored.relative_path != expected_relative {
            return Err(CoreError::invalid_data(
                "CONTENT_OBJECT_PATH_INVALID",
                "Content object path does not match its SHA-256 identity.",
            ));
        }
        let path = self.root.join(&stored.relative_path);
        ensure_within(&self.root, &path)?;
        ensure_existing_regular_file(&path)?;
        let bytes = fs::read(path)?;
        if bytes.len() as u64 != stored.byte_size || hex_sha256(&bytes) != stored.sha256 {
            return Err(CoreError::invalid_data(
                "CONTENT_OBJECT_CORRUPT",
                "Content-addressed object failed size or SHA-256 verification.",
            ));
        }
        Ok(bytes)
    }

    /// Verifies and adopts a historical Python CAS path without copying it.
    ///
    /// Python stored paths relative to the library (`objects/sha256/...`),
    /// while the Rust object table stores paths relative to the SHA root. Only
    /// that exact legacy layout is accepted; arbitrary library files can never
    /// be smuggled into the Rust object index.
    pub fn adopt_existing_legacy_object(
        &self,
        legacy_relative_path: &str,
        expected_sha256: &str,
        expected_byte_size: u64,
        extension: &str,
    ) -> CoreResult<StoredObject> {
        validate_sha256(expected_sha256)?;
        let extension = validate_extension(extension)?;
        let relative_path = object_relative_path(expected_sha256, &extension);
        let expected_legacy_path = format!("objects/sha256/{relative_path}");
        if legacy_relative_path != expected_legacy_path {
            return Err(CoreError::invalid_data(
                "LEGACY_OBJECT_PATH_INVALID",
                "Historical object path is not the exact content-addressed library path.",
            ));
        }
        let final_path = self.root.join(&relative_path);
        ensure_within(&self.root, &final_path)?;
        ensure_existing_regular_file(&final_path).map_err(|error| match error.code() {
            "LIBRARY_PATH_UNTRUSTED_TYPE" => CoreError::invalid_data(
                "LEGACY_OBJECT_PATH_UNTRUSTED_TYPE",
                "Historical content-addressed object is not a trusted regular file.",
            ),
            _ => CoreError::invalid_data(
                "LEGACY_OBJECT_MISSING",
                "Historical content-addressed object is missing or unavailable.",
            ),
        })?;
        verify_file(&final_path, expected_sha256, expected_byte_size).map_err(|_| {
            CoreError::invalid_data(
                "LEGACY_OBJECT_CORRUPT",
                "Historical content-addressed object failed size or SHA-256 verification.",
            )
        })?;
        Ok(StoredObject {
            sha256: expected_sha256.to_string(),
            relative_path,
            extension,
            byte_size: expected_byte_size,
        })
    }

    pub(crate) fn remove(&self, stored: &StoredObject) -> CoreResult<()> {
        validate_sha256(&stored.sha256)?;
        let extension = validate_extension(&stored.extension)?;
        let expected_relative = object_relative_path(&stored.sha256, &extension);
        if stored.relative_path != expected_relative {
            return Err(CoreError::invalid_data(
                "CONTENT_OBJECT_PATH_INVALID",
                "Content object deletion path does not match its SHA-256 identity.",
            ));
        }
        let path = self.root.join(&stored.relative_path);
        ensure_within(&self.root, &path)?;
        if ensure_private_file_if_present(&path)? {
            verify_file(&path, &stored.sha256, stored.byte_size)?;
        }
        match fs::remove_file(&path) {
            Ok(()) => {
                if let Some(parent) = path.parent() {
                    sync_directory(parent)?;
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Recovers only paths named by Rust pending journals. It never scans or
    /// deletes arbitrary unindexed final files, preserving legacy/Python data.
    pub(crate) fn recover_pending(
        &self,
        indexed_sha256: &BTreeSet<String>,
    ) -> CoreResult<Vec<String>> {
        ensure_private_directory_tree(&self.staging_root)?;
        ensure_private_directory_tree(&self.pending_root)?;
        for entry in fs::read_dir(&self.staging_root)? {
            let entry = entry?;
            let path = entry.path();
            ensure_existing_regular_file(&path)?;
            fs::remove_file(path)?;
        }
        sync_directory(&self.staging_root)?;

        let mut recovered = Vec::new();
        for entry in fs::read_dir(&self.pending_root)? {
            let entry = entry?;
            let entry_path = entry.path();
            ensure_existing_regular_file(&entry_path)?;
            let stored: StoredObject =
                serde_json::from_slice(&fs::read(&entry_path)?).map_err(|_| {
                    CoreError::invalid_data(
                        "CONTENT_OBJECT_JOURNAL_INVALID",
                        "Pending object recovery journal is invalid.",
                    )
                })?;
            validate_sha256(&stored.sha256)?;
            let expected =
                object_relative_path(&stored.sha256, &validate_extension(&stored.extension)?);
            if stored.relative_path != expected {
                return Err(CoreError::invalid_data(
                    "CONTENT_OBJECT_JOURNAL_INVALID",
                    "Pending object journal path does not match its SHA-256.",
                ));
            }
            let final_path = self.root.join(&stored.relative_path);
            ensure_within(&self.root, &final_path)?;
            if indexed_sha256.contains(&stored.sha256) {
                ensure_private_file(&final_path)?;
                verify_file(&final_path, &stored.sha256, stored.byte_size)?;
            } else {
                if ensure_private_file_if_present(&final_path)? {
                    verify_file(&final_path, &stored.sha256, stored.byte_size)?;
                    fs::remove_file(&final_path)?;
                    if let Some(parent) = final_path.parent() {
                        sync_directory(parent)?;
                    }
                }
                recovered.push(stored.sha256.clone());
            }
            fs::remove_file(entry_path)?;
        }
        sync_directory(&self.pending_root)?;
        Ok(recovered)
    }
}

fn validate_extension(value: &str) -> CoreResult<String> {
    let value = value.trim_start_matches('.').to_ascii_lowercase();
    if (1..=16).contains(&value.len())
        && value.is_ascii()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        Ok(value)
    } else {
        Err(CoreError::invalid_data(
            "CONTENT_OBJECT_EXTENSION_INVALID",
            "Content object extension must be bounded ASCII alphanumeric text.",
        ))
    }
}

fn validate_sha256(value: &str) -> CoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "CONTENT_OBJECT_SHA_INVALID",
            "Content object SHA-256 must be 64 lowercase hexadecimal characters.",
        ))
    }
}

fn object_relative_path(sha256: &str, extension: &str) -> String {
    format!(
        "{}/{}/{}.{}",
        &sha256[..2],
        &sha256[2..4],
        sha256,
        extension
    )
}

fn ensure_within(root: &Path, path: &Path) -> CoreResult<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "CONTENT_OBJECT_PATH_INVALID",
            "Content object path escaped the library root.",
        ))
    }
}

fn verify_file(path: &Path, expected_sha: &str, expected_size: u64) -> CoreResult<()> {
    ensure_existing_regular_file(path)?;
    let bytes = fs::read(path)?;
    if bytes.len() as u64 != expected_size || hex_sha256(&bytes) != expected_sha {
        return Err(CoreError::invalid_data(
            "CONTENT_OBJECT_CORRUPT",
            "Existing content-addressed bytes do not match their SHA-256 identity.",
        ));
    }
    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> CoreResult<()> {
    OpenOptions::new().read(true).open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> CoreResult<()> {
    // Directory handles are not portable through std::fs. The file is still
    // sync_all'ed and pending-journal recovery closes the rename window.
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn staged_object_promotes_deterministically_and_reads_verified_bytes() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let staged = store.stage(b"glb-bytes", ".GLB").unwrap();
        let expected = staged.metadata().clone();
        let mut promoted = staged.promote().unwrap();
        assert!(promoted.newly_created());
        promoted.finalize_commit().unwrap();
        assert_eq!(store.read(&expected).unwrap(), b"glb-bytes");

        let duplicate = store.stage(b"glb-bytes", "glb").unwrap().promote().unwrap();
        assert!(!duplicate.newly_created());
    }

    #[test]
    fn cancelled_stage_is_removed_and_corruption_is_rejected() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let stored = {
            let staged = store.stage(b"texture", "ktx2").unwrap();
            staged.metadata().clone()
        };
        assert!(!store.root.join(&stored.relative_path).exists());

        let promoted = store.stage(b"texture", "ktx2").unwrap().promote().unwrap();
        fs::write(&promoted.final_path, b"tampered").unwrap();
        assert_eq!(
            store.read(&stored).unwrap_err().code(),
            "CONTENT_OBJECT_CORRUPT"
        );
    }

    #[test]
    fn pending_journal_recovers_promote_before_database_commit() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let promoted = store.stage(b"orphan", "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        assert!(store.root.join(&stored.relative_path).exists());
        drop(promoted); // simulate crash before SQLite commit/finalize
        let recovered = store.recover_pending(&BTreeSet::new()).unwrap();
        assert_eq!(recovered, vec![stored.sha256]);
        assert!(!store.root.join(&stored.relative_path).exists());
    }

    #[test]
    fn committed_sha_keeps_final_and_recovery_only_clears_marker() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let promoted = store.stage(b"committed", "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        drop(promoted); // SQLite committed, process died before marker cleanup
        let recovered = store
            .recover_pending(&BTreeSet::from([stored.sha256.clone()]))
            .unwrap();
        assert!(recovered.is_empty());
        assert_eq!(store.read(&stored).unwrap(), b"committed");
    }

    #[test]
    fn traversal_extensions_are_rejected() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        assert_eq!(
            store.stage(b"x", "../glb").unwrap_err().code(),
            "CONTENT_OBJECT_EXTENSION_INVALID"
        );
    }

    #[test]
    fn deletion_revalidates_the_canonical_path_and_bytes_before_unlinking() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let mut promoted = store.stage(b"protected", "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();

        let mut escaped = stored.clone();
        escaped.relative_path = "../../outside.glb".into();
        assert_eq!(
            store.remove(&escaped).unwrap_err().code(),
            "CONTENT_OBJECT_PATH_INVALID"
        );
        assert_eq!(store.read(&stored).unwrap(), b"protected");

        fs::write(store.root.join(&stored.relative_path), b"tampered").unwrap();
        assert_eq!(
            store.remove(&stored).unwrap_err().code(),
            "CONTENT_OBJECT_CORRUPT"
        );
    }

    #[test]
    fn legacy_python_path_is_adopted_in_place_only_after_full_identity_check() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let mut promoted = store
            .stage(b"historical-glb", "glb")
            .unwrap()
            .promote()
            .unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let legacy_path = format!("objects/sha256/{}", stored.relative_path);
        assert_eq!(
            store
                .adopt_existing_legacy_object(
                    &legacy_path,
                    &stored.sha256,
                    stored.byte_size,
                    "glb",
                )
                .unwrap(),
            stored
        );
        assert_eq!(
            store
                .adopt_existing_legacy_object(
                    &stored.relative_path,
                    &stored.sha256,
                    stored.byte_size,
                    "glb",
                )
                .unwrap_err()
                .code(),
            "LEGACY_OBJECT_PATH_INVALID"
        );
        assert_eq!(
            store
                .adopt_existing_legacy_object(
                    &legacy_path,
                    &stored.sha256,
                    stored.byte_size + 1,
                    "glb",
                )
                .unwrap_err()
                .code(),
            "LEGACY_OBJECT_CORRUPT"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cas_directories_objects_and_atomic_replacements_are_private() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let mut promoted = store.stage(b"atomic", "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let object_path = store.root.join(&stored.relative_path);

        assert_eq!(
            fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(object_path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&object_path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let replacement = root.path().join("replacement");
        fs::write(&replacement, b"atomic").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o644)).unwrap();
        fs::rename(&replacement, &object_path).unwrap();
        assert_eq!(store.read(&stored).unwrap(), b"atomic");
        assert_eq!(
            fs::metadata(object_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn cas_target_and_pending_symlinks_fail_closed() {
        let root = tempdir().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let mut promoted = store.stage(b"symlink", "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let object_path = store.root.join(&stored.relative_path);
        let target = root.path().join("outside");
        fs::write(&target, b"symlink").unwrap();
        fs::remove_file(&object_path).unwrap();
        symlink(&target, &object_path).unwrap();
        assert_eq!(
            store.read(&stored).unwrap_err().code(),
            "LIBRARY_PATH_UNTRUSTED_TYPE"
        );

        let pending_link = store.pending_root.join("malicious.pending.json");
        symlink(&target, &pending_link).unwrap();
        assert_eq!(
            store.recover_pending(&BTreeSet::new()).unwrap_err().code(),
            "LIBRARY_PATH_UNTRUSTED_TYPE"
        );
    }
}
