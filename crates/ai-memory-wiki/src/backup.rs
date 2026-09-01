//! Pre-migration backup of the entire data directory (docs/okf.md).
//!
//! The OKF migration's first step — and its gate: no verified archive,
//! no migration. The archive lands OUTSIDE the data dir, in the user's
//! home (`AI_MEMORY_BACKUP_DIR` overrides the destination for machines
//! where home is small), and a receipt is recorded at
//! `<data_dir>/pre-migration-backup.json` so the wiki homepage can show
//! where it is until the user deletes it.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{WikiError, WikiResult};

/// File name of the receipt inside the data dir.
pub const BACKUP_RECEIPT_FILE: &str = "pre-migration-backup.json";

/// What the backup step produced; persisted as the receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReceipt {
    /// Absolute path of the archive.
    pub archive_path: PathBuf,
    /// Archive size in bytes.
    pub size_bytes: u64,
    /// Entries in the archive (files archived).
    pub entries: usize,
    /// ISO-8601 creation instant.
    pub created_at: String,
    /// What the backup was taken for (e.g. "okf-v0.2-migration").
    pub label: String,
}

impl BackupReceipt {
    /// Read the receipt from `data_dir`, if one exists and parses.
    #[must_use]
    pub fn load(data_dir: &Path) -> Option<Self> {
        let raw = std::fs::read(data_dir.join(BACKUP_RECEIPT_FILE)).ok()?;
        serde_json::from_slice(&raw).ok()
    }

    /// Whether the archive the receipt points at still exists on disk.
    #[must_use]
    pub fn archive_present(&self) -> bool {
        self.archive_path.exists()
    }
}

/// Destination directory for archives: the explicit override when given
/// (callers read `AI_MEMORY_BACKUP_DIR`), else the user's home. Erroring
/// (rather than falling back into the data dir) is deliberate: a backup
/// inside the tree it protects is not a backup.
fn destination_dir(dest_override: Option<&Path>) -> WikiResult<PathBuf> {
    if let Some(dir) = dest_override {
        return Ok(dir.to_path_buf());
    }
    dirs::home_dir().ok_or_else(|| {
        WikiError::Io(std::io::Error::other(
            "no home directory found for the pre-migration backup; \
             set AI_MEMORY_BACKUP_DIR to a directory outside the data dir",
        ))
    })
}

/// Compress `data_dir` into a timestamped tar.gz in the destination dir,
/// verify the archive is readable and complete, write the receipt, and
/// return it. Any failure aborts (the caller must not migrate).
pub fn create_pre_migration_backup(
    data_dir: &Path,
    label: &str,
    dest_override: Option<&Path>,
) -> WikiResult<BackupReceipt> {
    let dest_dir = destination_dir(dest_override)?;
    std::fs::create_dir_all(&dest_dir)?;
    let stamp = jiff::Timestamp::now().strftime("%Y%m%d-%H%M%S").to_string();
    let archive_path = dest_dir.join(format!("ai-memory-backup-{label}-{stamp}.tar.gz"));

    // The migration runs at server startup before any traffic, so the
    // SQLite files (db + WAL/SHM) are quiescent and archived as a set.
    let mut expected = 0usize;
    {
        let file = BufWriter::new(File::create(&archive_path)?);
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.follow_symlinks(false);
        let mut stack = vec![data_dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                let rel = path
                    .strip_prefix(data_dir)
                    .map_err(|e| WikiError::Io(std::io::Error::other(e)))?;
                let ft = entry.file_type()?;
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() {
                    tar.append_path_with_name(&path, rel)?;
                    expected += 1;
                }
                // Symlinks are skipped: nothing in a data dir should be
                // one, and following one out of the tree must not happen.
            }
        }
        tar.into_inner()?.finish()?;
    }

    // Verify: re-open and count entries; a torn or unreadable archive
    // fails here and the caller aborts the migration.
    let mut counted = 0usize;
    {
        let file = BufReader::new(File::open(&archive_path)?);
        let dec = flate2::read::GzDecoder::new(file);
        let mut ar = tar::Archive::new(dec);
        for entry in ar.entries()? {
            entry?;
            counted += 1;
        }
    }
    let size_bytes = std::fs::metadata(&archive_path)?.len();
    if counted != expected || size_bytes == 0 {
        let _ = std::fs::remove_file(&archive_path);
        return Err(WikiError::Io(std::io::Error::other(format!(
            "backup verification failed: archived {expected} files but the \
             archive lists {counted} (size {size_bytes}); refusing to migrate"
        ))));
    }

    let receipt = BackupReceipt {
        archive_path,
        size_bytes,
        entries: counted,
        created_at: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        label: label.to_string(),
    };
    let tmp = data_dir.join(format!("{BACKUP_RECEIPT_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&receipt)?)?;
    std::fs::rename(&tmp, data_dir.join(BACKUP_RECEIPT_FILE))?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn data_dir_with_content() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("wiki/ws/proj/notes")).unwrap();
        std::fs::write(tmp.path().join("wiki/ws/proj/notes/a.md"), "---\n---\nbody").unwrap();
        std::fs::write(tmp.path().join("db.sqlite"), b"not really a db").unwrap();
        tmp
    }

    #[test]
    fn a_backup_archives_everything_and_writes_a_receipt() {
        let data = data_dir_with_content();
        let dest = TempDir::new().unwrap();
        let receipt = create_pre_migration_backup(data.path(), "test", Some(dest.path())).unwrap();
        assert!(receipt.archive_path.starts_with(dest.path()));
        assert_eq!(receipt.entries, 2);
        assert!(receipt.size_bytes > 0);
        assert!(receipt.archive_present());
        let loaded = BackupReceipt::load(data.path()).unwrap();
        assert_eq!(loaded.archive_path, receipt.archive_path);
    }

    #[test]
    fn an_unwritable_destination_aborts_instead_of_falling_back() {
        let data = data_dir_with_content();
        let dest = TempDir::new().unwrap();
        let blocked = dest.path().join("blocked-file");
        std::fs::write(&blocked, b"a file, not a dir").unwrap();
        let err = create_pre_migration_backup(data.path(), "test", Some(&blocked));
        assert!(err.is_err(), "backup must fail when it cannot write");
        assert!(
            BackupReceipt::load(data.path()).is_none(),
            "no receipt without an archive"
        );
    }

    #[test]
    fn a_deleted_archive_is_reported_absent() {
        let data = data_dir_with_content();
        let dest = TempDir::new().unwrap();
        let receipt = create_pre_migration_backup(data.path(), "test", Some(dest.path())).unwrap();
        std::fs::remove_file(&receipt.archive_path).unwrap();
        assert!(!receipt.archive_present());
    }
}
