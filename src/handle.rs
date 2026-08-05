use std::mem;
use std::sync::{Arc, RwLock};
use crate::{Config, ConfigError};

/// A thread-safe, cloneable handle to a [`Config`].
///
/// `ConfigHandle` wraps a `Config` in an `Arc<RwLock<Arc<Config>>>` so it can be
/// shared across threads and reloaded at runtime without restarting.
/// Cloning a `ConfigHandle` is cheap — all clones refer to the same
/// underlying config.
///
/// The config is stored behind an inner `Arc` so that neither side holds a lock for
/// long: [`read`](ConfigHandle::read) locks only long enough to clone that `Arc` and
/// hands back an immutable snapshot, and [`reload`](ConfigHandle::reload) does its
/// file read and parse with no lock held, taking the write lock only for a pointer
/// swap. Readers are never blocked on disk I/O, and holding a snapshot never blocks
/// a reload.
///
/// # Example
/// ```no_run
/// # use trail_config::{Config, ConfigHandle, ConfigError};
/// # fn main() -> Result<(), ConfigError> {
/// let handle = ConfigHandle::new(
///     Config::load_required("config.yaml", "/", None)?
/// );
///
/// // Cheap to clone and send to other threads
/// let handle2 = handle.clone();
///
/// // Read values
/// let port = handle.str("app/port");
///
/// // Reload from disk (re-applies all overlays)
/// handle.reload()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct ConfigHandle {
    inner: Arc<RwLock<Arc<Config>>>,
}

impl ConfigHandle {
    /// Creates a new `ConfigHandle` wrapping the given [`Config`].
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(config))),
        }
    }

    /// Returns an immutable snapshot of the current [`Config`].
    ///
    /// `Arc<Config>` derefs to `Config`, so every [`Config`] method is available
    /// directly on the returned value.
    ///
    /// The read lock is held only long enough to clone an `Arc`, so a snapshot can be
    /// kept for as long as you like without blocking [`reload`](ConfigHandle::reload).
    /// The snapshot is also stable: a concurrent reload swaps a *new* config into the
    /// handle and leaves this one intact, so a series of reads from one snapshot can
    /// never straddle a reload.
    ///
    /// # Example
    /// ```
    /// # use trail_config::{Config, ConfigHandle};
    /// # let config = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    /// # let handle = ConfigHandle::new(config);
    /// let port = handle.read().get_int("app/port");
    ///
    /// // Or keep a snapshot for a consistent multi-value read
    /// let snapshot = handle.read();
    /// let host = snapshot.str("app/host");
    /// let port = snapshot.get_int("app/port");
    /// ```
    pub fn read(&self) -> Arc<Config> {
        Arc::clone(&self.inner.read().unwrap_or_else(|e| e.into_inner()))
    }

    /// Reloads the config from disk, re-applying all overlays in order.
    ///
    /// The file reads and parsing happen with **no lock held**. The source list
    /// (base filename plus the overlay chain) is copied under a read lock, the new
    /// config is built off to the side, and the write lock is taken only to swap the
    /// finished config in. Readers are therefore never blocked on disk I/O — only for
    /// the swap itself.
    ///
    /// If the reload fails, no swap occurs and the existing configuration is preserved
    /// unchanged.
    ///
    /// # Errors
    /// Returns the same errors as [`Config::reload`].
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigHandle, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// # let handle = ConfigHandle::new(Config::load_required("config.yaml", "/", None)?);
    /// handle.reload()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reload(&self) -> Result<(), ConfigError> {
        // Snapshot the sources: filenames and the overlay chain, not the document.
        let mut next = self.read().sources();

        // No lock held — disk I/O and parsing happen here. On failure we return
        // early and the live config is left untouched.
        next.reload()?;
        let next = Arc::new(next);

        // Write lock held for a pointer swap and nothing else.
        let previous = {
            let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
            mem::replace(&mut *guard, next)
        };

        // Released outside the lock: if no snapshot outlives this handle's reference,
        // dropping the old config walks and frees the whole document tree.
        drop(previous);
        Ok(())
    }

    /// Convenience method — gets a value as a string at the specified path.
    ///
    /// Equivalent to `handle.read().str(path)`.
    pub fn str(&self, path: &str) -> String {
        self.read().str(path)
    }

    /// Convenience method — gets a value as an integer at the specified path.
    ///
    /// Equivalent to `handle.read().get_int(path)`.
    pub fn get_int(&self, path: &str) -> Option<i64> {
        self.read().get_int(path)
    }

    /// Convenience method — gets a value as a float at the specified path.
    ///
    /// Equivalent to `handle.read().get_float(path)`.
    pub fn get_float(&self, path: &str) -> Option<f64> {
        self.read().get_float(path)
    }

    /// Convenience method — gets a value as a boolean at the specified path.
    ///
    /// Equivalent to `handle.read().get_bool(path)`.
    pub fn get_bool(&self, path: &str) -> Option<bool> {
        self.read().get_bool(path)
    }

    /// Convenience method — checks if a path exists in the configuration.
    ///
    /// Equivalent to `handle.read().contains(path)`.
    pub fn contains(&self, path: &str) -> bool {
        self.read().contains(path)
    }
}

impl From<Config> for ConfigHandle {
    fn from(config: Config) -> Self {
        Self::new(config)
    }
}

#[cfg(test)]
const _: () = {
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<ConfigHandle>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "
app:
  port: 8080
  debug: true
  timeout: 4.55
";

    #[test]
    fn new_and_read() {
        let handle = ConfigHandle::new(Config::load_yaml(YAML, "/").unwrap());
        assert_eq!(handle.str("app/port"), "8080");
        assert_eq!(handle.get_int("app/port"), Some(8080));
        assert_eq!(handle.get_bool("app/debug"), Some(true));
        assert_eq!(handle.get_float("app/timeout"), Some(4.55));
        assert!(handle.contains("app/port"));
        assert!(!handle.contains("app/missing"));
    }

    #[test]
    fn clone_shares_state() {
        let handle1 = ConfigHandle::new(Config::load_yaml(YAML, "/").unwrap());
        let handle2 = handle1.clone();
        assert_eq!(handle1.str("app/port"), handle2.str("app/port"));
        // Both refer to the same Arc — pointer equality
        assert!(Arc::ptr_eq(&handle1.inner, &handle2.inner));
    }

    #[test]
    fn from_config() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let handle: ConfigHandle = config.into();
        assert_eq!(handle.str("app/port"), "8080");
    }

    #[test]
    fn reload_picks_up_changes() {
        use crate::test_util::{temp_dir, write_file};
        use std::fs;

        let dir = temp_dir();
        let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

        let handle = ConfigHandle::new(
            Config::load_required(&path, "/", None).unwrap()
        );
        assert_eq!(handle.str("app/port"), "8080");

        fs::write(&path, "app:\n  port: 9090\n").unwrap();

        handle.reload().unwrap();
        assert_eq!(handle.str("app/port"), "9090");
    }

    #[test]
    fn reload_visible_to_all_clones() {
        use crate::test_util::{temp_dir, write_file};
        use std::fs;

        let dir = temp_dir();
        let path = write_file(&dir, "config.yaml", "app:\n  port: 1111\n");

        let handle1 = ConfigHandle::new(
            Config::load_required(&path, "/", None).unwrap()
        );
        let handle2 = handle1.clone();

        fs::write(&path, "app:\n  port: 2222\n").unwrap();

        handle1.reload().unwrap();
        // handle2 sees the change because they share the same Arc
        assert_eq!(handle2.str("app/port"), "2222");
    }

    #[test]
    fn reload_preserves_config_on_failure() {
        use crate::test_util::{temp_dir, write_file};
        use std::fs;

        let dir = temp_dir();
        let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

        let handle = ConfigHandle::new(
            Config::load_required(&path, "/", None).unwrap()
        );

        fs::write(&path, "invalid: [unclosed\n").unwrap();

        assert!(handle.reload().is_err());
        assert_eq!(handle.str("app/port"), "8080"); // still intact
    }

    #[test]
    fn read_snapshot_is_stable_across_reload() {
        use crate::test_util::{temp_dir, write_file};
        use std::fs;

        let dir = temp_dir();
        let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

        let handle = ConfigHandle::new(
            Config::load_required(&path, "/", None).unwrap()
        );

        let snapshot = handle.read();
        assert_eq!(snapshot.get_int("app/port"), Some(8080));

        fs::write(&path, "app:\n  port: 9090\n").unwrap();

        // Holding a snapshot must not block the reload. Under the old
        // guard-returning `read()` this call would have deadlocked.
        handle.reload().unwrap();

        // The snapshot still sees the config it was taken from...
        assert_eq!(snapshot.get_int("app/port"), Some(8080));
        // ...while the handle sees the new one.
        assert_eq!(handle.get_int("app/port"), Some(9090));
    }

    #[test]
    fn reload_while_readers_are_active() {
        use crate::test_util::{temp_dir, write_file};
        use std::{fs, thread};

        let dir = temp_dir();
        let path = write_file(&dir, "config.yaml", "app:\n  port: 1000\n");

        let handle = ConfigHandle::new(
            Config::load_required(&path, "/", None).unwrap()
        );

        // Readers hammer the handle while the reload runs. This would hang if
        // reload took a read lock and then a write lock without releasing.
        let readers: Vec<_> = (0..4).map(|_| {
            let h = handle.clone();
            thread::spawn(move || {
                for _ in 0..500 {
                    // Always a committed value — never a half-swapped state
                    let port = h.get_int("app/port").unwrap();
                    assert!(port == 1000 || port == 2000, "unexpected port {}", port);
                }
            })
        }).collect();

        fs::write(&path, "app:\n  port: 2000\n").unwrap();
        handle.reload().unwrap();

        for r in readers { r.join().unwrap(); }
        assert_eq!(handle.get_int("app/port"), Some(2000));
    }

    #[test]
    fn multithreaded_reads() {
        use std::thread;

        let handle = ConfigHandle::new(Config::load_yaml(YAML, "/").unwrap());
        let threads: Vec<_> = (0..8).map(|_| {
            let h = handle.clone();
            thread::spawn(move || {
                assert_eq!(h.str("app/port"), "8080");
                assert_eq!(h.get_int("app/port"), Some(8080));
            })
        }).collect();
        for t in threads { t.join().unwrap(); }
    }
}
