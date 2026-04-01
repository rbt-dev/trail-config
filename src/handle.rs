use std::sync::{Arc, RwLock, RwLockReadGuard};
use crate::{Config, ConfigError};

/// A thread-safe, cloneable handle to a [`Config`].
///
/// `ConfigHandle` wraps a `Config` in an `Arc<RwLock<...>>` so it can be
/// shared across threads and reloaded at runtime without restarting.
/// Cloning a `ConfigHandle` is cheap — all clones refer to the same
/// underlying config.
///
/// Reads acquire a shared read lock; [`reload`](ConfigHandle::reload) acquires
/// an exclusive write lock for the duration of the file read and parse.
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
    inner: Arc<RwLock<Config>>,
}

impl ConfigHandle {
    /// Creates a new `ConfigHandle` wrapping the given [`Config`].
    pub fn new(config: Config) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config)),
        }
    }

    /// Acquires a read lock and returns a guard giving access to the inner [`Config`].
    ///
    /// Use this to call any [`Config`] method directly. The lock is released
    /// when the guard is dropped.
    ///
    /// # Example
    /// ```
    /// # use trail_config::{Config, ConfigHandle};
    /// # let config = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    /// # let handle = ConfigHandle::new(config);
    /// let port = handle.read().get_int("app/port");
    /// ```
    pub fn read(&self) -> RwLockReadGuard<'_, Config> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Reloads the config from disk, re-applying all overlays in order.
    ///
    /// Acquires a write lock for the duration of the reload. All reads will
    /// block until the reload completes. If the reload fails, the existing
    /// configuration is preserved unchanged.
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
        self.inner.write()
            .unwrap_or_else(|e| e.into_inner())
            .reload()
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
mod tests {
    use super::*;

    const YAML: &str = "
app:
  port: 8080
  debug: true
  timeout: 3.14
";

    #[test]
    fn new_and_read() {
        let handle = ConfigHandle::new(Config::load_yaml(YAML, "/").unwrap());
        assert_eq!(handle.str("app/port"), "8080");
        assert_eq!(handle.get_int("app/port"), Some(8080));
        assert_eq!(handle.get_bool("app/debug"), Some(true));
        assert_eq!(handle.get_float("app/timeout"), Some(3.14));
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
        use std::fs::{self, File};
        use std::io::Write;

        let path = "test_handle_reload.yaml";
        let mut f = File::create(path).unwrap();
        writeln!(f, "app:\n  port: 8080").unwrap();
        drop(f);

        let handle = ConfigHandle::new(
            Config::load_required(path, "/", None).unwrap()
        );
        assert_eq!(handle.str("app/port"), "8080");

        let mut f = File::create(path).unwrap();
        writeln!(f, "app:\n  port: 9090").unwrap();
        drop(f);

        handle.reload().unwrap();
        assert_eq!(handle.str("app/port"), "9090");

        fs::remove_file(path).ok();
    }

    #[test]
    fn reload_visible_to_all_clones() {
        use std::fs::{self, File};
        use std::io::Write;

        let path = "test_handle_reload_clones.yaml";
        let mut f = File::create(path).unwrap();
        writeln!(f, "app:\n  port: 1111").unwrap();
        drop(f);

        let handle1 = ConfigHandle::new(
            Config::load_required(path, "/", None).unwrap()
        );
        let handle2 = handle1.clone();

        let mut f = File::create(path).unwrap();
        writeln!(f, "app:\n  port: 2222").unwrap();
        drop(f);

        handle1.reload().unwrap();
        // handle2 sees the change because they share the same Arc
        assert_eq!(handle2.str("app/port"), "2222");

        fs::remove_file(path).ok();
    }

    #[test]
    fn reload_preserves_config_on_failure() {
        use std::fs::{self, File};
        use std::io::Write;

        let path = "test_handle_reload_fail.yaml";
        let mut f = File::create(path).unwrap();
        writeln!(f, "app:\n  port: 8080").unwrap();
        drop(f);

        let handle = ConfigHandle::new(
            Config::load_required(path, "/", None).unwrap()
        );

        let mut f = File::create(path).unwrap();
        writeln!(f, "invalid: [unclosed").unwrap();
        drop(f);

        assert!(handle.reload().is_err());
        assert_eq!(handle.str("app/port"), "8080"); // still intact

        fs::remove_file(path).ok();
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

#[cfg(test)]
const _: () = {
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<ConfigHandle>();
    }
};