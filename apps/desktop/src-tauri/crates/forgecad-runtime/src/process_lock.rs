use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Process-lifetime writer lock for the single-user MVP.
///
/// The operating system releases the lock when the Runtime exits, including
/// an ungraceful crash. This intentionally replaces an expiring database
/// lease, so the MVP has no heartbeat or stale-lease takeover path.
pub(crate) struct ProcessLock {
    file: File,
}

impl ProcessLock {
    pub(crate) fn acquire_for_database(database: &Path) -> Result<Self, ProcessLockError> {
        let path = database.with_extension("writer.lock");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ProcessLockError::Io)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(ProcessLockError::Io)?;

        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if matches!(
                    error.raw_os_error(),
                    Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN
                ) {
                    return Err(ProcessLockError::Busy);
                }
                return Err(ProcessLockError::Io(error));
            }
        }

        #[cfg(not(unix))]
        {
            let _ = file;
            return Err(ProcessLockError::Unsupported);
        }

        Ok(Self { file })
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[derive(Debug)]
pub(crate) enum ProcessLockError {
    Busy,
    Io(io::Error),
    #[allow(dead_code)]
    Unsupported,
}

impl std::fmt::Display for ProcessLockError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("RUNTIME_BUSY: Runtime writer lock is already held"),
            Self::Io(error) => write!(formatter, "Runtime writer lock failed: {error}"),
            Self::Unsupported => {
                formatter.write_str("Runtime writer lock is unsupported on this platform")
            }
        }
    }
}

impl std::error::Error for ProcessLockError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn second_process_lock_is_busy_until_first_drops() {
        let root =
            std::env::temp_dir().join(format!("forgecad-process-lock-{}", std::process::id()));
        let database = root.join("runtime.sqlite");
        let first = ProcessLock::acquire_for_database(&database).expect("first lock");
        assert!(matches!(
            ProcessLock::acquire_for_database(&database),
            Err(ProcessLockError::Busy)
        ));
        drop(first);
        let second = ProcessLock::acquire_for_database(&database).expect("released lock");
        drop(second);
        let _ = fs::remove_dir_all(root);
    }
}
