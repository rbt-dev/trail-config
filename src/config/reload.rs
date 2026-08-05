//! Re-reading the config (and its overlays) from disk at runtime.

use std::io;
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
        let yaml = load_auto(filename)?;
        self.filename = filename.to_string();
        self.content = resolve_env_vars(yaml)?;
        self.overlays.clear();
        Ok(())
    }
}
