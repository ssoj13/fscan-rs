//! Filesystem traversal with streaming output and bounded in-process buffering.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use walkdir::WalkDir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: OsString,
    pub kind: EntryKind,
    pub len: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    pub entries: u64,
    pub files: u64,
    pub directories: u64,
    pub errors: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visit {
    Continue,
    Stop,
}

#[derive(Debug)]
pub enum ScanError<E> {
    Cancelled,
    Root(walkdir::Error),
    Sink(E),
}

impl<E: std::fmt::Display> std::fmt::Display for ScanError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("scan cancelled"),
            Self::Root(error) => write!(f, "cannot scan root: {error}"),
            Self::Sink(error) => write!(f, "scan sink failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ScanError<E> {}

/// Walk a directory without retaining entries. Directory errors count toward
/// `Progress::errors`; callers should treat a nonzero count as a partial scan.
/// `include` can reject a subtree before it is traversed.
pub fn scan_standard<E>(
    root: &Path,
    cancel: &AtomicBool,
    include: impl Fn(&Path) -> bool,
    mut on_entry: impl FnMut(&Entry, Progress) -> Result<Visit, E>,
) -> Result<Progress, ScanError<E>> {
    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_open(16)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || include(entry.path()));
    let mut progress = Progress::default();
    for item in walker {
        if cancel.load(Ordering::Acquire) {
            return Err(ScanError::Cancelled);
        }
        let item = match item {
            Ok(item) => item,
            Err(error) if error.depth() == 0 => return Err(ScanError::Root(error)),
            Err(_) => {
                progress.errors = progress.errors.saturating_add(1);
                continue;
            }
        };
        if item.depth() == 0 {
            continue;
        }
        let kind = if item.file_type().is_symlink() {
            EntryKind::Symlink
        } else if item.file_type().is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        };
        let metadata = if kind == EntryKind::Symlink {
            std::fs::symlink_metadata(item.path()).ok()
        } else {
            item.metadata().ok()
        };
        let metadata = match metadata {
            Some(metadata) => metadata,
            None => {
                progress.errors = progress.errors.saturating_add(1);
                continue;
            }
        };
        progress.entries = progress.entries.saturating_add(1);
        match kind {
            EntryKind::Directory => progress.directories = progress.directories.saturating_add(1),
            EntryKind::File | EntryKind::Symlink => {
                progress.files = progress.files.saturating_add(1)
            }
        }
        let entry = Entry {
            path: item.path().to_path_buf(),
            name: item.file_name().to_owned(),
            kind,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        };
        if on_entry(&entry, progress).map_err(ScanError::Sink)? == Visit::Stop {
            break;
        }
    }
    Ok(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_entries_and_skips_subtrees() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("keep")).unwrap();
        std::fs::create_dir(tmp.path().join("skip")).unwrap();
        std::fs::write(tmp.path().join("keep").join("one"), b"1").unwrap();
        std::fs::write(tmp.path().join("skip").join("two"), b"2").unwrap();
        let mut paths = Vec::new();
        let progress = scan_standard(
            tmp.path(),
            &AtomicBool::new(false),
            |path| !path.ends_with("skip"),
            |entry, _| {
                paths.push(entry.path.clone());
                Ok::<_, std::convert::Infallible>(Visit::Continue)
            },
        )
        .unwrap();
        assert_eq!(progress.entries, 2);
        assert!(paths.iter().any(|path| path.ends_with("one")));
        assert!(!paths.iter().any(|path| path.ends_with("two")));
    }

    #[test]
    fn cancellation_is_observed() {
        let tmp = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(true);
        let result = scan_standard(
            tmp.path(),
            &cancel,
            |_| true,
            |_, _| Ok::<_, std::convert::Infallible>(Visit::Continue),
        );
        assert!(matches!(result, Err(ScanError::Cancelled)));
    }
}
