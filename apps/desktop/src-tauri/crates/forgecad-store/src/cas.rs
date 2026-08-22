use forgecad_contracts::CasObjectRecord;
use forgecad_core::sha256_hex;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const HASH_LENGTH: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum CasError {
    #[error("CAS I/O failure")]
    Io(#[from] std::io::Error),
    #[error("CAS hash is invalid")]
    InvalidHash,
    #[error("CAS expected hash does not match content")]
    HashMismatch { expected: String, actual: String },
    #[error("CAS object is corrupt")]
    Corrupt,
    #[error("CAS object exceeds configured capacity")]
    CapacityExceeded,
    #[error("CAS root must not be a symlink")]
    UnsafeRoot,
    #[error("CAS put lock is poisoned")]
    PutLockPoisoned,
}

#[derive(Debug, Clone)]
pub struct CasObject {
    pub record: CasObjectRecord,
    pub path: PathBuf,
    /// True only when this `put` call installed the content-addressed file.
    /// This is not exclusive ownership: higher-level concurrent operations
    /// must use their Store reservation token before deciding whether a
    /// temporary object may be cleaned up.
    pub created_new: bool,
}

#[derive(Debug, Clone)]
pub struct CasStore {
    root: PathBuf,
    max_object_bytes: Option<u64>,
    put_lock: Arc<Mutex<()>>,
}

impl CasStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, CasError> {
        Self::with_max_object_bytes(root, None)
    }

    pub fn with_max_object_bytes(
        root: impl AsRef<Path>,
        max_object_bytes: Option<u64>,
    ) -> Result<Self, CasError> {
        let root = root.as_ref().to_path_buf();
        ensure_directory(&root)?;
        ensure_directory(&root.join("objects"))?;
        ensure_directory(&root.join("tmp"))?;
        Ok(Self {
            root,
            max_object_bytes,
            put_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn ephemeral() -> Result<Self, CasError> {
        let root = std::env::temp_dir().join(format!("forgecad-cas-{}", Uuid::new_v4()));
        Self::new(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put(
        &self,
        bytes: &[u8],
        expected_sha256: Option<&str>,
        mime: &str,
        kind: &str,
        created_at: &str,
    ) -> Result<CasObject, CasError> {
        // Runtime is the product's sole writer. This additional in-process
        // guard also makes concurrent calls through cloned Store handles agree
        // on which put actually installed a hash, so rollback can never delete
        // a peer call's pre-existing object.
        let _put_guard = self
            .put_lock
            .lock()
            .map_err(|_| CasError::PutLockPoisoned)?;
        let size = u64::try_from(bytes.len()).map_err(|_| CasError::CapacityExceeded)?;
        if self.max_object_bytes.is_some_and(|maximum| size > maximum) {
            return Err(CasError::CapacityExceeded);
        }

        let actual = sha256_hex(bytes);
        if let Some(expected) = expected_sha256 {
            validate_hash(expected)?;
            if expected != actual {
                return Err(CasError::HashMismatch {
                    expected: expected.to_owned(),
                    actual,
                });
            }
        }

        let object_path = self.object_path(&actual)?;
        if object_path.exists() {
            self.verify_existing(&object_path, &actual, size)?;
            return Ok(self.record(actual, size, mime, kind, created_at, object_path, false));
        }

        let temporary_path = self
            .root
            .join("tmp")
            .join(format!("{}.{}", actual, Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);

        if let Some(parent) = object_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let created_new = match fs::rename(&temporary_path, &object_path) {
            Ok(()) => true,
            Err(_error) if object_path.exists() => {
                let _ = fs::remove_file(&temporary_path);
                self.verify_existing(&object_path, &actual, size)?;
                false
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                return Err(error.into());
            }
        };

        self.verify_existing(&object_path, &actual, size)?;
        Ok(self.record(
            actual,
            size,
            mime,
            kind,
            created_at,
            object_path,
            created_new,
        ))
    }

    pub fn read_verified(&self, sha256: &str) -> Result<Vec<u8>, CasError> {
        validate_hash(sha256)?;
        let path = self.object_path(sha256)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            return Err(CasError::Corrupt);
        }
        let mut bytes = Vec::new();
        File::open(&path)?.read_to_end(&mut bytes)?;
        let actual = sha256_hex(&bytes);
        if actual != sha256 {
            return Err(CasError::HashMismatch {
                expected: sha256.to_owned(),
                actual,
            });
        }
        Ok(bytes)
    }

    /// Read and verify a CAS object without allowing its on-disk size to
    /// trigger an unbounded allocation. The metadata check happens before the
    /// file is opened, while the `take(max + 1)` guard also fails closed if the
    /// file grows between the metadata lookup and the read.
    pub fn read_verified_bounded(&self, sha256: &str, max_bytes: u64) -> Result<Vec<u8>, CasError> {
        validate_hash(sha256)?;
        let path = self.object_path(sha256)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_file() {
            return Err(CasError::Corrupt);
        }
        if metadata.len() > max_bytes {
            return Err(CasError::CapacityExceeded);
        }
        Self::read_verified_bounded_after_metadata(&path, sha256, metadata.len(), max_bytes)
    }

    fn read_verified_bounded_after_metadata(
        path: &Path,
        sha256: &str,
        observed_len: u64,
        max_bytes: u64,
    ) -> Result<Vec<u8>, CasError> {
        let capacity = usize::try_from(observed_len).map_err(|_| CasError::CapacityExceeded)?;
        let mut bytes = Vec::with_capacity(capacity);
        File::open(&path)?
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            return Err(CasError::CapacityExceeded);
        }
        if bytes.len() as u64 != observed_len {
            return Err(CasError::Corrupt);
        }
        let actual = sha256_hex(&bytes);
        if actual != sha256 {
            return Err(CasError::HashMismatch {
                expected: sha256.to_owned(),
                actual,
            });
        }
        Ok(bytes)
    }

    pub fn verify(&self, sha256: &str, expected_size: u64) -> Result<(), CasError> {
        validate_hash(sha256)?;
        let path = self.object_path(sha256)?;
        self.verify_existing(&path, sha256, expected_size)
    }

    pub fn list_objects(&self) -> Result<Vec<PathBuf>, CasError> {
        let objects = self.root.join("objects");
        let mut paths = Vec::new();
        for prefix in fs::read_dir(objects)? {
            let prefix = prefix?;
            if !prefix.file_type()?.is_dir() {
                continue;
            }
            for entry in fs::read_dir(prefix.path())? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    paths.push(entry.path());
                }
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub fn copy_objects_to(&self, destination: impl AsRef<Path>) -> Result<(), CasError> {
        let destination = destination.as_ref();
        ensure_directory(destination)?;
        ensure_directory(&destination.join("objects"))?;
        ensure_directory(&destination.join("tmp"))?;
        for source in self.list_objects()? {
            let relative = source
                .strip_prefix(self.root.join("objects"))
                .map_err(|_| CasError::Corrupt)?;
            let target = destination.join("objects").join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, target)?;
        }
        Ok(())
    }

    fn record(
        &self,
        sha256: String,
        size: u64,
        mime: &str,
        kind: &str,
        created_at: &str,
        path: PathBuf,
        created_new: bool,
    ) -> CasObject {
        CasObject {
            record: CasObjectRecord {
                schema_version: "CasObject@1".to_owned(),
                sha256,
                size_bytes: size,
                mime: mime.to_owned(),
                kind: kind.to_owned(),
                reachability: "temporary".to_owned(),
                created_at: created_at.to_owned(),
            },
            path,
            created_new,
        }
    }

    fn object_path(&self, sha256: &str) -> Result<PathBuf, CasError> {
        validate_hash(sha256)?;
        Ok(self.root.join("objects").join(&sha256[..2]).join(sha256))
    }

    fn verify_existing(
        &self,
        path: &Path,
        expected: &str,
        expected_size: u64,
    ) -> Result<(), CasError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.len() != expected_size {
            return Err(CasError::Corrupt);
        }
        let mut file = File::open(path)?;
        let mut bytes = Vec::with_capacity(expected_size as usize);
        file.read_to_end(&mut bytes)?;
        if sha256_hex(&bytes) != expected {
            return Err(CasError::Corrupt);
        }
        Ok(())
    }
}

fn validate_hash(value: &str) -> Result<(), CasError> {
    if value.len() != HASH_LENGTH
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(CasError::InvalidHash);
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), CasError> {
    if path.exists() {
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(CasError::UnsafeRoot);
        }
        if !path.is_dir() {
            return Err(CasError::Corrupt);
        }
        return Ok(());
    }
    fs::create_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("forgecad-cas-test-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test root");
        root
    }

    #[test]
    fn put_and_read_are_content_addressed() {
        let root = test_root("roundtrip");
        let cas = CasStore::new(&root).expect("cas");
        let object = cas
            .put(b"hello", None, "text/plain", "fixture", "1")
            .expect("put");
        assert_eq!(object.record.size_bytes, 5);
        assert_eq!(
            cas.read_verified(&object.record.sha256).expect("read"),
            b"hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_read_rejects_oversized_object_before_loading_it() {
        let root = test_root("bounded-read");
        let cas = CasStore::new(&root).expect("cas");
        let object = cas
            .put(b"hello", None, "text/plain", "fixture", "1")
            .expect("put");
        assert!(matches!(
            cas.read_verified_bounded(&object.record.sha256, 4),
            Err(CasError::CapacityExceeded)
        ));
        assert_eq!(
            cas.read_verified_bounded(&object.record.sha256, 5)
                .expect("bounded read"),
            b"hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_read_rejects_growth_after_metadata_observation() {
        let root = test_root("bounded-read-growth");
        let cas = CasStore::new(&root).expect("cas");
        let object = cas
            .put(b"hello", None, "text/plain", "fixture", "1")
            .expect("put");
        let observed_len = fs::symlink_metadata(&object.path).expect("metadata").len();
        OpenOptions::new()
            .append(true)
            .open(&object.path)
            .expect("open for simulated concurrent growth")
            .write_all(b"!")
            .expect("grow object after metadata observation");
        assert!(matches!(
            CasStore::read_verified_bounded_after_metadata(
                &object.path,
                &object.record.sha256,
                observed_len,
                observed_len,
            ),
            Err(CasError::CapacityExceeded)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_same_hash_put_has_exactly_one_creator() {
        let root = test_root("concurrent-same-hash");
        let cas = CasStore::new(&root).expect("cas");
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let cas = cas.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                cas.put(
                    b"same-content",
                    None,
                    "application/octet-stream",
                    "fixture",
                    "1",
                )
                .expect("concurrent put")
                .created_new
            }));
        }
        let created_count = handles
            .into_iter()
            .map(|handle| handle.join().expect("put thread"))
            .filter(|created_new| *created_new)
            .count();
        assert_eq!(created_count, 1);
        assert_eq!(cas.list_objects().expect("objects").len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expected_hash_and_capacity_fail_closed() {
        let root = test_root("negative");
        let cas = CasStore::with_max_object_bytes(&root, Some(4)).expect("cas");
        assert!(matches!(
            cas.put(b"hello", None, "text/plain", "fixture", "1"),
            Err(CasError::CapacityExceeded)
        ));
        let unrestricted = CasStore::new(&root).expect("unrestricted cas");
        assert!(matches!(
            unrestricted.put(
                b"hello",
                Some(&"0".repeat(64)),
                "text/plain",
                "fixture",
                "1"
            ),
            Err(CasError::HashMismatch { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_or_missing_objects_never_read_as_valid() {
        let root = test_root("corrupt");
        let cas = CasStore::new(&root).expect("cas");
        let object = cas
            .put(
                b"original",
                None,
                "application/octet-stream",
                "fixture",
                "1",
            )
            .expect("put");
        fs::write(&object.path, b"tampered").expect("tamper");
        assert!(matches!(
            cas.read_verified(&object.record.sha256),
            Err(CasError::HashMismatch { .. })
        ));
        fs::remove_file(&object.path).expect("remove");
        assert!(matches!(
            cas.verify(&object.record.sha256, object.record.size_bytes),
            Err(CasError::Io(_))
        ));
        let _ = fs::remove_dir_all(root);
    }
}
