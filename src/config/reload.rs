//! Re-reading the config (and its overlays) from disk at runtime.

use std::io;
use yaml_serde::Value;
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::empty_filename_error;
use super::merge::merge_values;
use super::parser::load_auto;

impl Config {
    /// Reloads the configuration from disk, re-applying all overlays in order.
    ///
    /// Re-reads the base file and each overlay file that was added via
    /// [`merge_required`](Config::merge_required) or [`merge_optional`](Config::merge_optional),
    /// then re-merges them in the original order. Required overlays that are missing will
    /// return an error; optional overlays that are missing are silently skipped.
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if any required file cannot be read or parsed
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if no file path is associated with this config
    /// Returns `ConfigError::IoError` if the base file or a required overlay is missing or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if any file cannot be parsed
    ///
    /// # Note
    /// If reloading fails, the existing configuration is preserved unchanged.
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let mut config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.prod.yaml", None)?
    ///     .merge_optional("config.local.yaml", None)?;
    /// // Later, reload all files from disk
    /// config.reload()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        if self.filename.is_empty() {
            return Err(ConfigError::FormatError("Cannot reload: no file path associated with this config".to_string()));
        }

        let mut content = resolve_env_vars(load_auto(&self.filename)?)?;

        for overlay in &self.overlays {
            match overlay {
                OverlaySource::Required(filename) => {
                    let yaml = resolve_env_vars(load_auto(filename)?)?;
                    content = merge_values(content, yaml);
                },
                OverlaySource::Optional(filename) => {
                    match load_auto(filename) {
                        Ok(yaml) => {
                            content = merge_values(content, resolve_env_vars(yaml)?);
                        },
                        Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {},
                        Err(e) => return Err(e),
                    }
                },
            }
        }

        self.content = content;
        Ok(())
    }

    /// Returns a copy of this config's *sources* — base filename, separator, environment
    /// and the overlay chain — with an empty document.
    ///
    /// Calling [`reload`](Config::reload) on the result yields a `Config` equivalent to
    /// reloading `self`, but without cloning the current document and without needing
    /// `&mut self`. [`ConfigHandle::reload`](crate::ConfigHandle::reload) uses this to do
    /// its file I/O off to the side and swap the finished config in afterwards, so readers
    /// are never blocked on disk.
    ///
    /// The overlay chain is preserved, which is why this cannot go through the loader's
    /// `from_parsed` builder — that always produces an overlay-free config.
    pub(crate) fn sources(&self) -> Config {
        Config {
            content: Value::Null,
            filename: self.filename.clone(),
            separator: self.separator.clone(),
            environment: self.environment.clone(),
            overlays: self.overlays.clone(),
        }
    }

    /// Reloads the configuration from a different file
    ///
    /// Changes the config's filename and reloads from the new file.
    /// The separator and environment settings remain the same.
    ///
    /// # Arguments
    /// * `filename` - New config file to load
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if the file cannot be read or parsed
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, the filename is empty, or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if an environment variable placeholder cannot be resolved
    ///
    /// # Note
    /// If the switch fails for any reason, the existing configuration is preserved
    /// unchanged — filename, content and the overlay chain are committed together
    /// only once the new file has been read, parsed and resolved.
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let mut config = Config::default();
    /// // Switch to loading from a different config file
    /// config.reload_from("other_config.yaml").expect("Failed to load");
    /// ```
    pub fn reload_from(&mut self, filename: &str) -> Result<(), ConfigError> {
        if filename.is_empty() {
            return Err(empty_filename_error());
        }

        // Read, parse and resolve into a local before touching `self`. Every failure
        // path returns here, leaving the filename, content and overlay chain as they
        // were — a partial switch would point `reload()` at the new file while the
        // old overlays were still registered.
        let content = resolve_env_vars(load_auto(filename)?)?;

        self.content = content;
        self.filename = filename.to_string();
        self.overlays.clear();
        Ok(())
    }
}
