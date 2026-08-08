//! Re-reading the config (and its overlays) from disk at runtime.

use std::io;
use yaml_serde::Value;
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::{empty_filename_error, get_file};
use super::merge::merge_documents;
use super::parser::{load_auto, load_in};

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
    ///     (only a config parsed from a string has none)
    /// Returns `ConfigError::IoError` if the base file or a required overlay is missing or cannot be read.
    ///     This includes a base file that was absent at load time via
    ///     [`load_optional`](Config::load_optional) and is still absent — the config keeps its
    ///     filename, so the reload succeeds once the file appears
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

        // The base file is read with this config's pinned format when it has one, so a
        // file loaded as `Format::Json` under a `.conf` extension is re-read as JSON
        // rather than silently falling back to YAML. Overlays below stay on `load_auto`:
        // each one names its own format by its own extension, which is what lets a JSON
        // base take a YAML overlay.
        let mut content = resolve_env_vars(load_in(self.format, &self.filename)?)?;

        for overlay in &self.overlays {
            match overlay {
                OverlaySource::Required(filename) => {
                    let yaml = resolve_env_vars(load_auto(filename)?)?;
                    content = merge_documents(content, yaml);
                },
                OverlaySource::Optional(filename) => {
                    match load_auto(filename) {
                        Ok(yaml) => {
                            content = merge_documents(content, resolve_env_vars(yaml)?);
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
            format: self.format,
        }
    }

    /// Reloads the configuration from a different file, discarding the overlay chain.
    ///
    /// Changes the config's filename and reloads from the new file.
    /// The separator and environment settings remain the same, and so does an explicitly
    /// chosen format: a config built by one of the `_as` constructors —
    /// [`load_required_as`](Config::load_required_as),
    /// [`load_optional_as`](Config::load_optional_as),
    /// [`load_or_create_as`](Config::load_or_create_as) — reads the new file with that same
    /// parser. Construct a new `Config` to change format. Configs loaded any other way
    /// have no pinned format and pick the parser from the new file's extension, as always.
    ///
    /// **Overlays are not carried over.** Every file added by
    /// [`merge_required`](Config::merge_required) or
    /// [`merge_optional`](Config::merge_optional) is dropped, so the result holds the new
    /// file and nothing else, and a later [`reload`](Config::reload) re-reads only that
    /// file. Keeping them would be the worse default: an overlay chain describes what was
    /// layered onto *this* base, and re-applying it to a different one silently merges
    /// files the caller never paired. Layer the new base yourself if you want overlays on
    /// it — the merges take `&mut self` as well, so
    /// [`merge_required_in_place`](Config::merge_required_in_place) picks up right here.
    ///
    /// The filename may contain an `{env}` placeholder, which is resolved against the
    /// environment this config already carries — there is no `env` argument because
    /// `reload_from` preserves it. The *resolved* name is recorded, so a later
    /// [`reload`](Config::reload) reads the same file.
    ///
    /// # Arguments
    /// * `filename` - New config file to load (can contain `{env}` placeholder)
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if the file cannot be read or parsed
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, the filename is empty, or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the filename contains `{env}` but this config has no
    ///     environment, or if an environment variable placeholder cannot be resolved
    ///
    /// # Note
    /// If the switch fails for any reason, the existing configuration is preserved
    /// unchanged — filename, content and the overlay chain are committed together
    /// only once the new file has been read, parsed and resolved. So a failed switch
    /// keeps the overlays too, rather than clearing them on the way out.
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

        // `{env}` is resolved against the environment this config already carries,
        // which `reload_from` preserves. Storing the *resolved* name keeps `reload()`
        // working afterwards, exactly as the initial load does.
        let (file, _) = get_file(filename, self.environment.as_deref())?;

        // Read, parse and resolve into a local before touching `self`. Every failure
        // path returns here, leaving the filename, content and overlay chain as they
        // were — a partial switch would point `reload()` at the new file while the
        // old overlays were still registered.
        //
        // A pinned format is preserved, like the separator and the environment: it was an
        // explicit choice by whoever built the config, and the alternative is worse in the
        // specific way this crate cares about. Dropping it would send a JSON-pinned config
        // reading a new extensionless file as YAML — which usually *succeeds*, since YAML
        // is a superset of JSON, and quietly applies the wrong rules. Keeping it means a
        // genuine format switch fails with a parse error naming the format, which is
        // visible. Build a new `Config` to change format.
        let content = resolve_env_vars(load_in(self.format, &file)?)?;

        self.content = content;
        self.filename = file;
        self.overlays.clear();
        Ok(())
    }
}
