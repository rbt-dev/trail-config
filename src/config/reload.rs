//! Re-reading the config (and its overlays) from disk at runtime.

use std::{fs, io, thread};
use yaml_serde::Value;
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::{empty_filename_error, get_file, EMPTY_FILE_BACKOFF, EMPTY_FILE_RETRIES};
use super::merge::merge_documents;
use super::parser::{parse_in, Format};

impl Config {
    /// Reloads the configuration from disk, re-applying all overlays in order.
    ///
    /// Re-reads the base file and each overlay file that was added via
    /// [`merge_required`](Config::merge_required) or [`merge_optional`](Config::merge_optional),
    /// then re-merges them in the original order. Required overlays that are missing will
    /// return an error; optional overlays that are missing are silently skipped.
    ///
    /// A file that is zero bytes long — typically one an editor has truncated but not yet
    /// rewritten — is re-read for up to about 200 ms in case the write completes. A base
    /// file that stays empty is an error; an overlay that stays empty is a no-op, as
    /// always. A half-written file that still parses cannot be detected: write watched
    /// files atomically (to a temporary file, then rename), and see
    /// [`ConfigHandle::with_validator`](crate::ConfigHandle::with_validator) for a guard.
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
    ///     filename, so the reload succeeds once the file appears.
    ///     It also includes a base file that is still zero bytes long after the wait
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
        let mut content = resolve_env_vars(read_base(self.format, &self.filename)?)?;

        for overlay in &self.overlays {
            match overlay {
                OverlaySource::Required(filename) => {
                    let yaml = resolve_env_vars(read_overlay(filename)?)?;
                    content = merge_documents(content, yaml);
                },
                OverlaySource::Optional(filename) => {
                    match read_overlay(filename) {
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
    /// Returns `ConfigError::IoError` if the file is missing, the filename is empty, the file cannot be read,
    ///     or it is still zero bytes long after the same wait as [`reload`](Config::reload)
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

        // Finish all fallible work before committing content, filename, and overlays
        // together. Preserve the pinned parser across filename changes; callers must
        // construct a new Config to choose a different format.
        let content = resolve_env_vars(read_base(self.format, &file)?)?;

        self.content = content;
        self.filename = file;
        self.overlays.clear();
        Ok(())
    }
}

/// Reads and parses the base file for [`reload`](Config::reload) or
/// [`reload_from`](Config::reload_from), refusing a file that stays zero-length.
///
/// An empty base parses as an empty document, so a reload that caught an editor between
/// truncating the file and writing it used to succeed and publish a config holding none
/// of the base's values. That is never a meaningful replacement for a loaded config, any
/// more than a missing base is, so it is an error like that one and the current config
/// is kept. A file that merely *looks* empty — a comment, a lone `{}` — is not
/// zero-length and loads as before.
fn read_base(format: Option<Format>, filename: &str) -> Result<Value, ConfigError> {
    let text = read_settled(filename)?;
    if text.is_empty() {
        return Err(ConfigError::io_in(filename, io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "file is empty, probably caught mid-save; the current config was kept",
        )));
    }
    parse_in(format, &text, filename)
}

/// Reads and parses an overlay for [`reload`](Config::reload), by its own extension.
///
/// An overlay that stays zero-length keeps its documented meaning — an empty overlay is a
/// no-op — so only the wait in [`read_settled`] applies to it.
fn read_overlay(filename: &str) -> Result<Value, ConfigError> {
    parse_in(None, &read_settled(filename)?, filename)
}

/// Reads `filename`, re-reading for up to the `load_or_create` retry window while it is
/// zero-length, in case a writer is between truncating and filling it.
///
/// Decided on the text that was read rather than a separate metadata check, so the
/// content parsed is the content that was judged non-empty. Narrows the window rather
/// than closing it: a half-written file that happens to be valid is undetectable here,
/// which is what atomic writes or a validator are for (see HOT_RELOAD.md). A legitimately
/// empty file costs its reload the full window.
fn read_settled(filename: &str) -> Result<String, ConfigError> {
    let read = || fs::read_to_string(filename).map_err(|e| ConfigError::io_in(filename, e));
    let mut text = read()?;
    for _ in 0..EMPTY_FILE_RETRIES {
        if !text.is_empty() {
            break;
        }
        thread::sleep(EMPTY_FILE_BACKOFF);
        text = read()?;
    }
    Ok(text)
}
