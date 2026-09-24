# Changelog

## Unreleased

- Keep NTFS cancellation, backend unavailability, and other scan failures distinct through `ScanFailure`, so callers can choose their fallback behavior.

## 0.1.0 — 2026-09-23

- Extract Squarebob's Windows NTFS directory and MFT scanners into a reusable crate with progress reporting and a 250,000-node safety limit.
- Add a portable streaming directory walker with cancellation, subtree filtering, and partial-scan diagnostics.
- Let consumers preserve every hard-link name for indexing or deduplicate file IDs for disk-usage trees.
