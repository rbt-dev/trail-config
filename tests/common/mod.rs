//! Shared helpers for the consumer-vantage tests.
//!
//! Not a test target of its own — Cargo treats `tests/common/mod.rs` as a module that
//! the sibling test binaries include.

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// A temp directory that is removed when it drops, even if a test panics.
pub fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

/// Writes `contents` to `name` inside `dir`, returning the full path as a `String` —
/// which is what every `Config` constructor takes.
pub fn write_file(dir: &TempDir, name: &str, contents: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, contents).expect("failed to write test file");
    path_string(&path)
}

/// The full path of `name` inside `dir`, without creating it.
pub fn path_in(dir: &TempDir, name: &str) -> String {
    path_string(&dir.path().join(name))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
