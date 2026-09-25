//! Cross-process transactions and atomic, private file replacement.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Serialize, de::DeserializeOwned};

pub fn lock(path: &Path) -> Result<File> {
    let file = lock_file(path)?;
    FileExt::lock_exclusive(&file).context("locking saved state")?;
    Ok(file)
}

pub fn try_lock(path: &Path) -> Result<Option<File>> {
    let file = lock_file(path)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(file)),
        // Windows reports contention as ERROR_LOCK_VIOLATION, not WouldBlock.
        Err(e) if e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn lock_file(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path.with_extension("lock"))?)
}

/// The temporary file is unique and private from creation, including on first sign-in.
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("missing state directory")?;
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error).with_context(|| format!("saving {}", path.display()))?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("reading {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn update_json<T: DeserializeOwned + Serialize + Default, R>(path: &Path, update: impl FnOnce(&mut T) -> R) -> Result<R> {
    let _lock = lock(path)?;
    let mut value = read_json(path)?;
    let before = serde_json::to_vec_pretty(&value)?;
    let result = update(&mut value);
    let after = serde_json::to_vec_pretty(&value)?;
    // Polling workers often have nothing to claim or recover. Keep those transactions read-only.
    if before != after || !path.exists() {
        write(path, &after)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_transactions_do_not_replace_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"[1,2]").unwrap();
        update_json::<Vec<u64>, _>(&path, |_| ()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"[1,2]");
    }

    #[test]
    fn concurrent_writers_preserve_every_update() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("counter.json");
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let path = &path;
                scope.spawn(move || {
                    for _ in 0..20 {
                        update_json::<u64, _>(path, |n| *n += 1).unwrap();
                    }
                });
            }
        });
        assert_eq!(read_json::<u64>(&path).unwrap(), 160);
    }

    #[test]
    fn corrupt_state_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, b"broken").unwrap();
        assert!(update_json::<Vec<String>, _>(&path, |v| v.clear()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    }

    #[test]
    fn transfer_lease_excludes_another_writer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("worker");
        let first = try_lock(&path).unwrap().unwrap();
        assert!(try_lock(&path).unwrap().is_none());
        drop(first);
        assert!(try_lock(&path).unwrap().is_some());
    }
}
