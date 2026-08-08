//! Shared helpers for tests: isolated temp directories for file-based tests
//! and a process-wide lock serializing environment variable mutation.

use std::path::Path;
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

/// RAII guard that changes the process working directory for the duration of a
/// test and restores the original on drop — even if the test panics.
///
/// `std::env::set_current_dir` is process-global (like env-var mutation), so
/// this acquires [`env_lock`] to serialize with any other test touching the
/// process environment or working directory. Used to exercise constructors that
/// hardcode a CWD-relative path (e.g. `Config::default()`).
pub(crate) struct CwdGuard {
    original: Option<std::path::PathBuf>,
    _env: MutexGuard<'static, ()>,
}

impl CwdGuard {
    /// Changes the working directory to `dir`, holding the env lock until drop.
    pub(crate) fn new(dir: &Path) -> Self {
        let env = env_lock();
        let original = std::env::current_dir().ok();
        std::env::set_current_dir(dir).expect("failed to change directory");
        Self { original, _env: env }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        if let Some(orig) = self.original.take() {
            let _ = std::env::set_current_dir(orig);
        }
    }
}
