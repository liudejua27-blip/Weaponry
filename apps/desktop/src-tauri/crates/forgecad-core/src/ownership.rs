use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{migration::open_connection, CoreError, CoreResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateOwner {
    PythonCompatibilityAdapter,
    RustAppServer,
}

impl StateOwner {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PythonCompatibilityAdapter => "python_compatibility_adapter",
            Self::RustAppServer => "rust_app_server",
        }
    }
}

impl FromStr for StateOwner {
    type Err = CoreError;

    fn from_str(value: &str) -> CoreResult<Self> {
        match value {
            "python_compatibility_adapter" => Ok(Self::PythonCompatibilityAdapter),
            "rust_app_server" => Ok(Self::RustAppServer),
            _ => Err(CoreError::invalid_data(
                "STATE_OWNER_INVALID",
                "ForgeCAD core ownership marker is invalid.",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OwnershipMarker {
    pub schema_version: String,
    pub state_owner: StateOwner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_writer_instance_id: Option<String>,
    pub writer_epoch: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquired_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<String>,
    pub updated_at: String,
}

/// Process-local evidence that a newly acquired Rust epoch replaced a stale
/// active-writer marker left by an abruptly terminated Rust process.
///
/// This deliberately does not add another durable ownership phase. Holding
/// the OS lock proves the prior process is no longer active; advancing the
/// durable epoch is the fence that makes every handle from that process stale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WriterLeaseRecovery {
    pub previous_instance_id: String,
    pub previous_epoch: u64,
}

/// Exclusive bootstrap lock held before schema or data migrations begin.
///
/// The existing database ownership row does not exist on a fresh library, so
/// migration cannot use the SQLite writer epoch as its first fence. This lock
/// uses the same file as [`WriterLease`] and transfers its open file handle
/// into the writer without an unlock gap.
#[derive(Debug)]
pub struct BootstrapLease {
    lock_path: PathBuf,
    lock_file: Option<File>,
}

impl BootstrapLease {
    pub fn acquire(library_root: impl AsRef<Path>) -> CoreResult<Self> {
        let library_root = library_root.as_ref();
        std::fs::create_dir_all(library_root)?;
        let library_root = library_root.canonicalize()?;
        let lock_path = library_root.join(".forgecad-core.writer.lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(CoreError::conflict(
                    "RUST_CORE_WRITER_ALREADY_ACTIVE",
                    "Another ForgeCAD product-state writer already holds the library lease.",
                ));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(CoreError::Io(error)),
        }
        Ok(Self {
            lock_path,
            lock_file: Some(lock_file),
        })
    }

    /// Converts the bootstrap fence into the durable Rust writer epoch while
    /// keeping the same OS lock held throughout the transition.
    pub fn cutover(
        mut self,
        db_path: impl AsRef<Path>,
        instance_id: impl Into<String>,
        expected_owner: StateOwner,
    ) -> CoreResult<Arc<WriterLease>> {
        let instance_id = instance_id.into();
        validate_instance_id(&instance_id)?;
        let db_path = db_path.as_ref().canonicalize()?;
        let lock_file = self.lock_file.take().ok_or_else(|| {
            CoreError::conflict(
                "RUST_CORE_BOOTSTRAP_LEASE_CONSUMED",
                "The ForgeCAD bootstrap writer lease was already consumed.",
            )
        })?;
        WriterLease::acquire_with_lock_file(
            db_path,
            self.lock_path.clone(),
            lock_file,
            instance_id,
            expected_owner,
        )
    }
}

impl Drop for BootstrapLease {
    fn drop(&mut self) {
        if let Some(file) = self.lock_file.take() {
            let _ = file.unlock();
        }
    }
}

/// Process-scoped exclusive writer lease.
///
/// The OS lock closes the crash window that a SQLite marker alone cannot
/// cover. The durable marker records the ownership cutover and epoch; a stale
/// process can never write because every repository transaction rechecks both
/// the writer ID and epoch while this file lock remains held.
#[derive(Debug)]
pub struct WriterLease {
    db_path: PathBuf,
    _lock_path: PathBuf,
    lock_file: File,
    instance_id: String,
    epoch: u64,
    previous_owner: StateOwner,
    recovered_writer: Option<WriterLeaseRecovery>,
    published: AtomicBool,
    rolled_back: AtomicBool,
}

impl WriterLease {
    pub fn acquire(
        db_path: impl AsRef<Path>,
        library_root: impl AsRef<Path>,
        instance_id: impl Into<String>,
        expected_owner: StateOwner,
    ) -> CoreResult<Arc<Self>> {
        BootstrapLease::acquire(library_root)?.cutover(db_path, instance_id, expected_owner)
    }

    fn acquire_with_lock_file(
        db_path: PathBuf,
        lock_path: PathBuf,
        lock_file: File,
        instance_id: String,
        expected_owner: StateOwner,
    ) -> CoreResult<Arc<Self>> {
        let mut connection = open_connection(&db_path)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_marker_from(&transaction)?;
        if current.state_owner != expected_owner {
            return Err(CoreError::conflict(
                "RUST_CORE_OWNERSHIP_STALE",
                "Ownership changed before the Rust writer lease was acquired.",
            ));
        }
        // `active_writer_instance_id` may remain populated when a process is
        // killed between durable acquire and in-memory publish. Because this
        // code already holds the exclusive OS lock, such an ID is stale. Keep
        // explicit process-local recovery evidence and advance the epoch in
        // the same CAS below; never hand ownership back to Python.
        let recovered_writer = if current.state_owner == StateOwner::RustAppServer {
            current
                .active_writer_instance_id
                .as_ref()
                .map(|previous_instance_id| WriterLeaseRecovery {
                    previous_instance_id: previous_instance_id.clone(),
                    previous_epoch: current.writer_epoch,
                })
        } else {
            None
        };
        let epoch = current.writer_epoch.checked_add(1).ok_or_else(|| {
            CoreError::conflict(
                "RUST_CORE_WRITER_EPOCH_EXHAUSTED",
                "ForgeCAD writer epoch cannot advance.",
            )
        })?;
        let acquired_at = system_timestamp();
        let changed = transaction.execute(
            "UPDATE forgecad_core_ownership SET state_owner='rust_app_server', active_writer_instance_id=?, writer_epoch=?, acquired_at=?, released_at=NULL, updated_at=? WHERE singleton=1 AND state_owner=? AND writer_epoch=?",
            params![
                instance_id,
                epoch,
                acquired_at,
                acquired_at,
                expected_owner.as_str(),
                current.writer_epoch,
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::conflict(
                "RUST_CORE_OWNERSHIP_STALE",
                "Ownership changed during the Rust writer cutover.",
            ));
        }
        transaction.commit()?;

        Ok(Arc::new(Self {
            db_path,
            _lock_path: lock_path,
            lock_file,
            instance_id,
            epoch,
            previous_owner: current.state_owner,
            recovered_writer,
            published: AtomicBool::new(false),
            rolled_back: AtomicBool::new(false),
        }))
    }

    pub fn marker(&self) -> CoreResult<OwnershipMarker> {
        let connection = open_connection(&self.db_path)?;
        read_marker_from(&connection)
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns crash-recovery evidence for this acquisition. Clean restarts
    /// have no active writer marker and therefore return `None`.
    pub fn recovered_writer(&self) -> Option<&WriterLeaseRecovery> {
        self.recovered_writer.as_ref()
    }

    /// Marks the Rust handlers as externally visible. After this point the
    /// ownership phase is permanent; shutdown clears only the active writer.
    pub fn publish(&self) -> CoreResult<()> {
        let connection = open_connection(&self.db_path)?;
        self.assert_current(&connection)?;
        self.published.store(true, Ordering::Release);
        Ok(())
    }

    pub fn is_published(&self) -> bool {
        self.published.load(Ordering::Acquire)
    }

    /// Reverts only an unpublished first Python→Rust initialization. A Rust
    /// restart or any published handler can never hand ownership back.
    pub fn rollback_before_publish(&self) -> CoreResult<bool> {
        if self.previous_owner != StateOwner::PythonCompatibilityAdapter {
            return Ok(false);
        }
        if self.is_published() {
            return Err(CoreError::conflict(
                "RUST_CORE_CUTOVER_ALREADY_PUBLISHED",
                "Published Rust ownership cannot roll back to Python.",
            ));
        }
        let mut connection = open_connection(&self.db_path)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.assert_current(&transaction)?;
        let released_at = system_timestamp();
        let changed = transaction.execute(
            "UPDATE forgecad_core_ownership SET state_owner='python_compatibility_adapter', active_writer_instance_id=NULL, released_at=?, updated_at=? WHERE singleton=1 AND state_owner='rust_app_server' AND active_writer_instance_id=? AND writer_epoch=?",
            params![released_at, released_at, self.instance_id, self.epoch],
        )?;
        if changed != 1 {
            return Err(CoreError::conflict(
                "RUST_CORE_WRITER_LEASE_STALE",
                "The unpublished Rust cutover is no longer authoritative.",
            ));
        }
        transaction.commit()?;
        self.rolled_back.store(true, Ordering::Release);
        Ok(true)
    }

    pub(crate) fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub(crate) fn assert_current(&self, connection: &rusqlite::Connection) -> CoreResult<()> {
        let marker = read_marker_from(connection)?;
        if marker.state_owner != StateOwner::RustAppServer
            || marker.active_writer_instance_id.as_deref() != Some(self.instance_id.as_str())
            || marker.writer_epoch != self.epoch
        {
            return Err(CoreError::conflict(
                "RUST_CORE_WRITER_LEASE_STALE",
                "The Rust product-state writer lease is no longer authoritative.",
            ));
        }
        Ok(())
    }
}

impl Drop for WriterLease {
    fn drop(&mut self) {
        if self.rolled_back.load(Ordering::Acquire) {
            let _ = self.lock_file.unlock();
            return;
        }
        if let Ok(mut connection) = open_connection(&self.db_path) {
            if let Ok(transaction) =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)
            {
                let released_at = system_timestamp();
                if self.previous_owner == StateOwner::PythonCompatibilityAdapter
                    && !self.published.load(Ordering::Acquire)
                {
                    let _ = transaction.execute(
                        "UPDATE forgecad_core_ownership SET state_owner='python_compatibility_adapter', active_writer_instance_id=NULL, released_at=?, updated_at=? WHERE singleton=1 AND state_owner='rust_app_server' AND active_writer_instance_id=? AND writer_epoch=?",
                        params![released_at, released_at, self.instance_id, self.epoch],
                    );
                } else {
                    let _ = transaction.execute(
                        "UPDATE forgecad_core_ownership SET active_writer_instance_id=NULL, released_at=?, updated_at=? WHERE singleton=1 AND state_owner='rust_app_server' AND active_writer_instance_id=? AND writer_epoch=?",
                        params![released_at, released_at, self.instance_id, self.epoch],
                    );
                }
                let _ = transaction.commit();
            }
        }
        let _ = self.lock_file.unlock();
    }
}

fn read_marker_from(connection: &rusqlite::Connection) -> CoreResult<OwnershipMarker> {
    connection
        .query_row(
            "SELECT schema_version, state_owner, active_writer_instance_id, writer_epoch, acquired_at, released_at, updated_at FROM forgecad_core_ownership WHERE singleton=1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| CoreError::not_found("ForgeCAD core ownership marker"))
        .and_then(|row| {
            Ok(OwnershipMarker {
                schema_version: row.0,
                state_owner: row.1.parse()?,
                active_writer_instance_id: row.2,
                writer_epoch: row.3,
                acquired_at: row.4,
                released_at: row.5,
                updated_at: row.6,
            })
        })
}

pub fn read_ownership_marker(db_path: impl AsRef<Path>) -> CoreResult<OwnershipMarker> {
    let connection = open_connection(db_path.as_ref())?;
    read_marker_from(&connection)
}

fn validate_instance_id(value: &str) -> CoreResult<()> {
    if !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err(CoreError::invalid_data(
            "RUST_CORE_WRITER_ID_INVALID",
            "Writer instance ID must be bounded ASCII.",
        ))
    }
}

fn system_timestamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("unix_ms_{millis}")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::MigrationRunner;

    use super::*;

    #[test]
    fn bootstrap_lock_fences_schema_work_and_transfers_without_unlock_gap() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let bootstrap = BootstrapLease::acquire(root.path()).unwrap();
        let blocked = BootstrapLease::acquire(root.path()).unwrap_err();
        assert_eq!(blocked.code(), "RUST_CORE_WRITER_ALREADY_ACTIVE");

        MigrationRunner::new(&db).run().unwrap();
        let writer = bootstrap
            .cutover(
                &db,
                "writer_bootstrap_transfer",
                StateOwner::PythonCompatibilityAdapter,
            )
            .unwrap();
        assert_eq!(writer.epoch(), 1);
        let still_blocked = BootstrapLease::acquire(root.path()).unwrap_err();
        assert_eq!(still_blocked.code(), "RUST_CORE_WRITER_ALREADY_ACTIVE");
    }

    #[test]
    fn cutover_is_explicit_and_second_writer_is_rejected() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let first = WriterLease::acquire(
            &db,
            root.path(),
            "writer_first",
            StateOwner::PythonCompatibilityAdapter,
        )
        .unwrap();
        let marker = first.marker().unwrap();
        assert_eq!(marker.state_owner, StateOwner::RustAppServer);
        assert_eq!(
            marker.active_writer_instance_id.as_deref(),
            Some("writer_first")
        );
        assert_eq!(marker.writer_epoch, 1);
        first.publish().unwrap();

        let error =
            WriterLease::acquire(&db, root.path(), "writer_second", StateOwner::RustAppServer)
                .unwrap_err();
        assert_eq!(error.code(), "RUST_CORE_WRITER_ALREADY_ACTIVE");
        drop(first);

        let restarted = WriterLease::acquire(
            &db,
            root.path(),
            "writer_restarted",
            StateOwner::RustAppServer,
        )
        .unwrap();
        assert_eq!(restarted.epoch(), 2);
    }

    #[test]
    fn wrong_expected_owner_cannot_claim_cutover() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let error = WriterLease::acquire(
            &db,
            root.path(),
            "writer_wrong_phase",
            StateOwner::RustAppServer,
        )
        .unwrap_err();
        assert_eq!(error.code(), "RUST_CORE_OWNERSHIP_STALE");
    }

    #[test]
    fn unpublished_first_cutover_rolls_back_but_published_cutover_cannot() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let first = WriterLease::acquire(
            &db,
            root.path(),
            "writer_initializing",
            StateOwner::PythonCompatibilityAdapter,
        )
        .unwrap();
        assert!(first.rollback_before_publish().unwrap());
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::PythonCompatibilityAdapter
        );
        drop(first);

        let published = WriterLease::acquire(
            &db,
            root.path(),
            "writer_published",
            StateOwner::PythonCompatibilityAdapter,
        )
        .unwrap();
        published.publish().unwrap();
        assert_eq!(
            published.rollback_before_publish().unwrap_err().code(),
            "RUST_CORE_CUTOVER_ALREADY_PUBLISHED"
        );
        drop(published);
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::RustAppServer
        );
    }
}
