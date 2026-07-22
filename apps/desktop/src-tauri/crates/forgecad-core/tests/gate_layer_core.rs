//! Layer-2-only Core evidence that is intentionally independent of Tauri,
//! Python, the app-server, and packaged launch behavior.

use std::{fs, path::Path};

use forgecad_core::{ContentAddressedObjectStore, MigrationRunner};
use rusqlite::Connection;
use tempfile::tempdir;

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

#[test]
fn sqlite_checkpoint_and_cas_restore_is_readable() {
    let source = tempdir().unwrap();
    let source_db = source.path().join("library.sqlite3");
    let source_library = source.path().join("library");

    let migration = MigrationRunner::new(&source_db).run().unwrap();
    assert_eq!(migration.journal_mode, "wal");

    let source_store = ContentAddressedObjectStore::new(&source_library).unwrap();
    let bytes = b"pure-rust-core-backup-evidence";
    let promoted = source_store.stage(bytes, "bin").unwrap().promote().unwrap();
    let stored = promoted.metadata().clone();
    assert_eq!(source_store.read(&stored).unwrap(), bytes);
    drop(promoted);

    // The checkpoint makes the SQLite file and its CAS directory a stable
    // offline backup pair without invoking the application or a live writer.
    let connection = Connection::open(&source_db).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(connection);

    let restored = tempdir().unwrap();
    let restored_db = restored.path().join("library.sqlite3");
    let restored_library = restored.path().join("library");
    fs::copy(&source_db, &restored_db).unwrap();
    copy_tree(&source_library, &restored_library);

    let restored_report = MigrationRunner::new(&restored_db).run().unwrap();
    assert_eq!(restored_report.journal_mode, "wal");
    let restored_store = ContentAddressedObjectStore::new(&restored_library).unwrap();
    assert_eq!(restored_store.read(&stored).unwrap(), bytes);

    let restored_connection = Connection::open(&restored_db).unwrap();
    let core_schema_count: i64 = restored_connection
        .query_row(
            "SELECT COUNT(*) FROM forgecad_core_schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(core_schema_count > 0);
}
