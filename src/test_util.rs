//! Shared helpers for tests: isolated temp directories for file-based tests
//! and a process-wide lock serializing environment variable mutation.

use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that mutate process environment variables.
///
/// `std::env::set_var` / `remove_var` are process-global; without this lock,
/// parallel tests can race on the environment. Hold the returned guard for
/// the whole test. A poisoned lock is recovered — the environment is left in
/// whatever state the panicking test produced, but tests use unique variable
/// names so this cannot corrupt another test's variables.
pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Creates a temp directory that is removed on drop, even if the test panics.
pub(crate) fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

/// Writes `content` to `name` inside `dir` and returns the full path as a String.
pub(crate) fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("failed to write test file");
    path.to_string_lossy().into_owned()
}
