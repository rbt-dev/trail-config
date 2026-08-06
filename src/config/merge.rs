//! Layering overlay files on top of a base config via deep merge.

use std::{io, mem};
use yaml_serde::Value;
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::{empty_filename_error, get_file};
use super::parser::load_auto;

impl Config {
    /// Merges a required overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// A null in the overlay is a value like any other and takes precedence, so a key set
    /// to null — including through YAML's bare `key:` form — clears the base value, and a
    /// subtree set to null clears the entire subtree. The key itself remains present
    /// holding a null: [`contains`](Config::contains) still reports `true`, while
    /// [`str`](Config::str) returns `""` and the typed accessors return `None`. An overlay
    /// file that is *entirely* empty is the one exception — it is a no-op, not a
    /// document-wide clear.
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
    /// * `env` - Optional environment name to substitute in filename. Also recorded on the
    ///   config if it does not already carry one, so [`environment`](Config::environment)
    ///   and [`reload_from`](Config::reload_from) see it; an environment set by the
    ///   constructor is never replaced. Interpolated into a filesystem path with no
    ///   validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file is missing or cannot be read
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
        if filename.is_empty() {
            return Err(empty_filename_error());
        }

        let (file, resolved_env) = get_file(filename, env)?;
        let overlay = resolve_env_vars(load_auto(&file)?)?;
        self.content = merge_documents(self.content, overlay);
        self.overlays.push(OverlaySource::Required(file));
        self.adopt_environment(resolved_env);
        Ok(self)
    }

    /// Merges an optional overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// A null in the overlay is a value like any other and takes precedence, so a key set
    /// to null — including through YAML's bare `key:` form — clears the base value, and a
    /// subtree set to null clears the entire subtree. The key itself remains present
    /// holding a null: [`contains`](Config::contains) still reports `true`, while
    /// [`str`](Config::str) returns `""` and the typed accessors return `None`. An overlay
    /// file that is *entirely* empty is the one exception — it is a no-op, not a
    /// document-wide clear.
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
    /// * `env` - Optional environment name to substitute in filename. Also recorded on the
    ///   config if it does not already carry one, so [`environment`](Config::environment)
    ///   and [`reload_from`](Config::reload_from) see it; an environment set by the
    ///   constructor is never replaced. Interpolated into a filesystem path with no
    ///   validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty — a caller bug, unlike an
    ///     absent file, which is the case this method exists to tolerate
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
        // Checked even though this method tolerates a missing file: reading an empty
        // path yields `NotFound`, which is exactly the case it is designed to ignore,
        // so without this an empty filename would silently no-op and then push a dead
        // overlay that every later `reload()` re-walks.
        if filename.is_empty() {
            return Err(empty_filename_error());
        }

        let (file, resolved_env) = get_file(filename, env)?;
        match load_auto(&file) {
            Ok(yaml) => {
                let overlay = resolve_env_vars(yaml)?;
                self.content = merge_documents(self.content, overlay);
            },
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
        self.overlays.push(OverlaySource::Optional(file));
        self.adopt_environment(resolved_env);
        Ok(self)
    }

    /// Records an environment supplied at a merge, if this config has none yet.
    ///
    /// The natural shape when the base file is not environment-specific but an overlay
    /// is: `load_required("config.yaml", "/", None).merge_required("config.{env}.yaml",
    /// Some("prod"))`. The merge resolved `{env}` correctly and then dropped the
    /// environment on the floor, so [`environment`](Config::environment) under-reported
    /// it and [`reload_from`](Config::reload_from) — which takes no `env` argument
    /// precisely because it reads the one on the config — could not resolve a template
    /// the merge had resolved a moment earlier.
    ///
    /// **Only when absent.** An environment already on the config was chosen by the
    /// constructor, which is the config's own identity; letting a later overlay
    /// overwrite it would silently change what a subsequent `reload_from` resolves. So
    /// this fills a gap and never reassigns.
    fn adopt_environment(&mut self, resolved: Option<String>) {
        if self.environment.is_none() {
            self.environment = resolved;
        }
    }
}

/// Deep-merges a whole overlay *document* onto a base document.
///
/// An empty document is a no-op, leaving the base untouched — that is what an absent
/// file tolerated by [`Config::merge_optional`], an empty file and a comment-only file
/// all parse to. The check belongs here, at the document level, because it is a property
/// of the overlay *as a whole*: inside a document an explicit null is an ordinary value
/// that overrides the base, and `merge_values` cannot tell the two apart — both arrive
/// as `Value::Null`.
///
/// Every merge of a complete file goes through this function; `merge_values` recurses
/// only into keys, where the empty-document rule must not apply.
pub(super) fn merge_documents(base: Value, overlay: Value) -> Value {
    match overlay {
        Value::Null => base,
        overlay => merge_values(base, overlay),
    }
}

/// Deep-merges `overlay` onto `base`, preserving the base's key order.
///
/// Overridden keys keep their position in the base document and genuinely-new overlay
/// keys are appended. This matters because `yaml_serde::Mapping` wraps an `IndexMap` and
/// is insertion-ordered, so the merged order is visible to anything that preserves it —
/// `get_as` / `deserialize` into a `Value`, a `Mapping` or an `IndexMap`, and any
/// re-serialization the caller does downstream.
fn merge_values(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                match base_map.get_mut(&key) {
                    // Merged in place. `Mapping::remove` is `swap_remove`, so
                    // remove-then-insert would move the map's *last* entry into the
                    // vacated slot and append the overridden key at the end — two keys
                    // displaced for a single-key overlay.
                    Some(slot) => {
                        let base_val = mem::replace(slot, Value::Null);
                        *slot = merge_values(base_val, overlay_val);
                    },
                    None => {
                        base_map.insert(key, overlay_val);
                    },
                }
            }
            Value::Mapping(base_map)
        },
        // Sequences are replaced wholesale; all other types are overridden by the
        // overlay — including an explicit null, which is how a key is cleared. The
        // whole-document case is handled in `merge_documents`, above.
        (_, overlay) => overlay,
    }
}
