# fscan-rs

`fscan-rs` streams filesystem entries to a caller-owned sink. It provides the traversal layer shared by Squarebob and filesystem Locate. The sink can build a tree, write an index, or process records immediately. The scanner itself does not retain the file list.

## Scan a directory

```rust
use fscan_rs::{Visit, scan_standard};
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cancel = AtomicBool::new(false);
    let progress = scan_standard(
        Path::new("."),
        &cancel,
        |_| true,
        |entry, _| {
            println!("{}", entry.path.display());
            Ok::<Visit, std::io::Error>(Visit::Continue)
        },
    )?;
    eprintln!("{} entries, {} errors", progress.entries, progress.errors);
    Ok(())
}
```

The `include` callback rejects a path and, for directories, its entire subtree. Set the `AtomicBool` to cancel a running scan. The sink can return `Visit::Stop` to finish early. A sink error stops the scan and is returned as `ScanError::Sink`.

## Memory and scan results

The standard backend uses a depth-first iterator with at most 16 open directory handles. It does not collect paths into a queue or a tree. A consumer's own storage is separate: Squarebob currently builds a complete tree for its UI, while Locate writes batches to SQLite. This crate does not make Squarebob's tree memory bounded.

An unreadable child path increments `Progress::errors`; callers should treat a nonzero count as a partial scan. Failure to open the root is an error. The scanner does not follow symlinks. File names remain `OsString`, so callers can decide how to handle paths that are not UTF-8.

## Backend status

The standard backend runs on Windows, Linux, and macOS. Squarebob's NTFS MFT backend is still in Squarebob. It builds a tree and can retain many MFT records in memory, so it has not yet been moved into this streaming crate. NTFS extraction needs a bounded record pipeline and live tests on a large volume before Locate can use it safely. This version does not implement incremental filesystem watching; a new scan traverses the tree again.

## Development

```text
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

This crate began as an extraction of Squarebob's standard scan boundary. It is intentionally independent of Squarebob's UI, cache format, and Locate's SQLite schema.
