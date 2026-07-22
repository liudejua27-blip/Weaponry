//! Filesystem boundaries for the local ForgeCAD Library.
//!
//! Unix mode bits are an explicit final guarantee.  This module deliberately
//! never changes an existing owner permission to a broader one: opening an
//! old library removes group/other access while preserving a restrictive
//! owner mask.  Windows ACL enforcement is a follow-up; the non-Unix branch
//! keeps the crate buildable and still rejects symlinks and wrong file types.

use std::{
    fs::{self, File, Metadata, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
};

use crate::{CoreError, CoreResult};

const UNTRUSTED_TYPE: &str = "LIBRARY_PATH_UNTRUSTED_TYPE";
const PERMISSION_REPAIR_FAILED: &str = "FILESYSTEM_PERMISSION_REPAIR_FAILED";
const PATH_INVALID: &str = "LIBRARY_PATH_INVALID";

pub(crate) fn ensure_private_directory_tree(path: &Path) -> CoreResult<()> {
    let path = absolute_without_parent_components(path)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure_directory_metadata(&path, &metadata)?;
            return tighten_existing_directory(&path, &metadata);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(CoreError::Io(error)),
    }

    // Resolve only the already-existing parent prefix.  macOS commonly has
    // system symlinks such as /var -> /private/var; rejecting those ancestors
    // would make an otherwise safe temporary/user Library unusable.  The
    // Library root and every directory created below are still checked with
    // symlink_metadata and never accepted as symlinks.
    let mut missing = Vec::new();
    let mut existing = path.clone();
    while matches!(fs::symlink_metadata(&existing), Err(error) if error.kind() == io::ErrorKind::NotFound)
    {
        let name = existing.file_name().ok_or_else(|| {
            CoreError::invalid_data(
                PATH_INVALID,
                "Library path has no creatable directory name.",
            )
        })?;
        missing.push(name.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| {
                CoreError::invalid_data(PATH_INVALID, "Library path has no existing parent.")
            })?
            .to_path_buf();
    }
    let current_existing = existing.canonicalize()?;
    let existing_metadata = fs::symlink_metadata(&existing).map_err(CoreError::Io)?;
    ensure_directory_metadata(&existing, &existing_metadata)?;
    let current_metadata = fs::symlink_metadata(&current_existing).map_err(CoreError::Io)?;
    ensure_directory_metadata(&current_existing, &current_metadata)?;
    let mut current = current_existing;
    for name in missing.into_iter().rev() {
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => ensure_directory_metadata(&current, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(CoreError::Io)?;
                if let Err(error) = set_new_directory_mode(&current) {
                    let _ = fs::remove_dir(&current);
                    return Err(error);
                }
            }
            Err(error) => return Err(CoreError::Io(error)),
        }
    }
    Ok(())
}

pub(crate) fn ensure_private_file(path: &Path) -> CoreResult<File> {
    let path = absolute_without_parent_components(path)?;
    let parent = path.parent().ok_or_else(|| {
        CoreError::invalid_data(PATH_INVALID, "A Library file must have a parent directory.")
    })?;
    ensure_existing_directory(parent)?;

    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure_regular_metadata(&path, &metadata)?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|error| permission_or_io(error, "could not open a Library file"))?;
            tighten_existing_file(&file, &path, &metadata)?;
            Ok(file)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            set_new_file_mode(&mut options);
            options.open(&path).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    CoreError::invalid_data(
                        UNTRUSTED_TYPE,
                        "Library file changed while it was being initialized.",
                    )
                } else {
                    permission_or_io(error, "could not create a Library file")
                }
            })
        }
        Err(error) => Err(CoreError::Io(error)),
    }
}

/// Creates a new private file.  The caller owns the returned handle and must
/// still sync it at its own durability boundary.
pub(crate) fn create_private_file(path: &Path) -> CoreResult<File> {
    let path = absolute_without_parent_components(path)?;
    let parent = path.parent().ok_or_else(|| {
        CoreError::invalid_data(PATH_INVALID, "A Library file must have a parent directory.")
    })?;
    ensure_existing_directory(parent)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_new_file_mode(&mut options);
    options.open(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            CoreError::invalid_data(
                UNTRUSTED_TYPE,
                "A new Library marker or object already exists at the target path.",
            )
        } else {
            permission_or_io(error, "could not create a private Library file")
        }
    })
}

pub(crate) fn ensure_existing_regular_file(path: &Path) -> CoreResult<Metadata> {
    let path = absolute_without_parent_components(path)?;
    if let Some(parent) = path.parent() {
        ensure_existing_directory(parent)?;
    }
    let metadata = fs::symlink_metadata(&path).map_err(CoreError::Io)?;
    ensure_regular_metadata(&path, &metadata)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| permission_or_io(error, "could not open a Library object"))?;
    tighten_existing_file(&file, &path, &metadata)?;
    Ok(metadata)
}

pub(crate) fn ensure_private_file_if_present(path: &Path) -> CoreResult<bool> {
    match fs::symlink_metadata(path) {
        // SQLite is allowed to remove its transient WAL/SHM file after the
        // metadata read but before the hardening open. Treat only that exact
        // NotFound race as an absent optional file; every other type,
        // permission or I/O failure remains fail-closed.
        Ok(_) => optional_file_hardening_result(ensure_existing_regular_file(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CoreError::Io(error)),
    }
}

fn optional_file_hardening_result(result: CoreResult<Metadata>) -> CoreResult<bool> {
    match result {
        Ok(_) => Ok(true),
        Err(CoreError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn secure_sqlite_files(db_path: &Path) -> CoreResult<()> {
    // The primary database was created and opened immediately before this
    // call. Unlike WAL/SHM it is not optional and must remain a trusted file.
    ensure_existing_regular_file(db_path)?;
    let wal = PathBuf::from(format!("{}-wal", db_path.display()));
    let shm = PathBuf::from(format!("{}-shm", db_path.display()));
    ensure_private_file_if_present(&wal)?;
    ensure_private_file_if_present(&shm)?;

    // BootstrapLease owns this marker and creates it before migration.  Do
    // not create it here, but harden it whenever it is already present.
    if let Some(parent) = db_path.parent() {
        let lock = parent.join(".forgecad-core.writer.lock");
        ensure_private_file_if_present(&lock)?;
    }
    Ok(())
}

fn ensure_existing_directory(path: &Path) -> CoreResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(CoreError::Io)?;
    ensure_directory_metadata(path, &metadata)?;
    tighten_existing_directory(path, &metadata)
}

fn ensure_directory_metadata(path: &Path, metadata: &Metadata) -> CoreResult<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CoreError::invalid_data(
            UNTRUSTED_TYPE,
            format!(
                "Library directory is not a trusted directory: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn ensure_regular_metadata(path: &Path, metadata: &Metadata) -> CoreResult<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CoreError::invalid_data(
            UNTRUSTED_TYPE,
            format!(
                "Library file is not a trusted regular file: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn absolute_without_parent_components(path: &Path) -> CoreResult<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CoreError::invalid_data(
            PATH_INVALID,
            "Library paths may not contain parent-directory components.",
        ));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn permission_or_io(error: io::Error, context: &str) -> CoreError {
    if matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
    ) {
        CoreError::conflict(PERMISSION_REPAIR_FAILED, context)
    } else {
        CoreError::Io(error)
    }
}

#[cfg(unix)]
fn set_new_directory_mode(path: &Path) -> CoreResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| permission_or_io(error, "could not set a new Library directory to 0700"))
}

#[cfg(not(unix))]
fn set_new_directory_mode(_path: &Path) -> CoreResult<()> {
    // Windows ACL enforcement is intentionally a later task.  Symlink/type
    // checks remain active on this platform so this is not a silent fallback.
    Ok(())
}

#[cfg(unix)]
fn tighten_existing_directory(path: &Path, metadata: &Metadata) -> CoreResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o700;
    if metadata.permissions().mode() & 0o7777 == mode {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
        permission_or_io(error, "could not tighten a Library directory permission")
    })
}

#[cfg(not(unix))]
fn tighten_existing_directory(_path: &Path, _metadata: &Metadata) -> CoreResult<()> {
    Ok(())
}

#[cfg(unix)]
fn set_new_file_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_new_file_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn tighten_existing_file(file: &File, _path: &Path, metadata: &Metadata) -> CoreResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o600;
    if metadata.permissions().mode() & 0o7777 == mode {
        return Ok(());
    }
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| permission_or_io(error, "could not tighten a Library file permission"))
}

#[cfg(not(unix))]
fn tighten_existing_file(_file: &File, _path: &Path, _metadata: &Metadata) -> CoreResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[cfg(unix)]
    #[test]
    fn new_and_existing_paths_are_private_and_idempotent() {
        let root = tempdir().unwrap();
        let library = root.path().join("Library");
        ensure_private_directory_tree(&library).unwrap();
        assert_eq!(
            fs::metadata(&library).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::set_permissions(&library, fs::Permissions::from_mode(0o755)).unwrap();
        ensure_private_directory_tree(&library).unwrap();
        assert_eq!(
            fs::metadata(&library).unwrap().permissions().mode() & 0o777,
            0o700
        );

        let file = library.join("marker");
        let handle = ensure_private_file(&file).unwrap();
        handle.sync_all().unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        ensure_private_file(&file).unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        ensure_private_file(&file).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_wrong_type_are_rejected() {
        let root = tempdir().unwrap();
        let library = root.path().join("Library");
        ensure_private_directory_tree(&library).unwrap();
        let target = library.join("target");
        ensure_private_file(&target).unwrap();
        let link = library.join("link");
        symlink(&target, &link).unwrap();
        assert_eq!(
            ensure_existing_regular_file(&link).unwrap_err().code(),
            UNTRUSTED_TYPE
        );
        let dir_as_file = library.join("directory");
        ensure_private_directory_tree(&dir_as_file).unwrap();
        assert_eq!(
            ensure_private_file(&dir_as_file).unwrap_err().code(),
            UNTRUSTED_TYPE
        );
    }

    #[test]
    fn optional_sqlite_auxiliary_disappearance_is_not_a_filesystem_failure() {
        let disappeared = CoreError::Io(io::Error::from(io::ErrorKind::NotFound));
        assert!(!optional_file_hardening_result(Err(disappeared)).unwrap());

        let denied = CoreError::Io(io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(
            optional_file_hardening_result(Err(denied))
                .unwrap_err()
                .code(),
            "FILESYSTEM_OPERATION_FAILED"
        );
    }
}
