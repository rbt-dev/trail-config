//! Layering overlay files on top of a base config via deep merge.

use std::io;
use yaml_serde::Value;
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::get_file;
use super::parser::load_auto;

impl Config {
    /// Merges a required overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, an error is returned.
    ///
    /// Environment variable placeholders (`${VAR}`, `${VAR:-default}`) are resolved in the
    /// overlay before merging. Values already present in the base config are not re-resolved,
    /// so resolved values containing `${` are preserved verbatim.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "merge returns a new Config; the original is consumed"]
    pub fn merge_required(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        let (file, _) = get_file(filename, env)?;
        let overlay = resolve_env_vars(load_auto(&file)?)?;
        self.content = merge_values(self.content, overlay);
        self.overlays.push(OverlaySource::Required(file));
        Ok(self)
    }

    /// Merges an optional overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, it is silently skipped.
    /// If the file exists but cannot be parsed, an error is returned.
    ///
    /// Environment variable placeholders (`${VAR}`, `${VAR:-default}`) are resolved in the
    /// overlay before merging. Values already present in the base config are not re-resolved,
    /// so resolved values containing `${` are preserved verbatim.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Errors
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "merge returns a new Config; the original is consumed"]
    pub fn merge_optional(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        let (file, _) = get_file(filename, env)?;
        match load_auto(&file) {
            Ok(yaml) => {
                let overlay = resolve_env_vars(yaml)?;
                self.content = merge_values(self.content, overlay);
            },
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
        self.overlays.push(OverlaySource::Optional(file));
        Ok(self)
    }
}

pub(super) fn merge_values(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let merged = match base_map.remove(&key) {
                    Some(base_val) => merge_values(base_val, overlay_val),
                    None => overlay_val,
                };
                base_map.insert(key, merged);
            }
            Value::Mapping(base_map)
        },
        // A null overlay (e.g. from an empty Config) is a no-op — preserve the base
        (base, Value::Null) => base,
        // Sequences are replaced wholesale; all other types are overridden by overlay
        (_, overlay) => overlay,
    }
}
