use forgecad_contracts::CasObjectRecord;
use forgecad_core::sha256_hex;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
}

#[derive(Debug, Clone)]
pub struct CasObject {
    pub record: CasObjectRecord,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CasStore {
    root: PathBuf,
    max_object_bytes: Option<u64>,
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
            return Ok(self.record(actual, size, mime, kind, created_at, object_path));
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
        match fs::rename(&temporary_path, &object_path) {
            Ok(()) => {}
            Err(_error) if object_path.exists() => {
                let _ = fs::remove_file(&temporary_path);
                self.verify_existing(&object_path, &actual, size)?;
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                return Err(error.into());
            }
        }

        self.verify_existing(&object_path, &actual, size)?;
        Ok(self.record(actual, size, mime, kind, created_at, object_path))
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
