//! Constructors: loading a `Config` from files or strings.

use std::{fs, io, io::Write};
use yaml_serde::Value;
use crate::error::ConfigError;
use super::Config;
use super::env::resolve_env_vars;
use super::parser;

impl Config {
    /// Loads a Config from a file, returning an error if the file is missing or invalid.
    ///
    /// Use this in production code where a missing config file is a critical error.
    /// For optional config files, use [`load_optional`](Config::load_optional) or [`default`](Config::default).
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Returns
    /// Returns `Ok(Config)` if the file is found and valid, or `Err(ConfigError)` otherwise
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, empty filename, or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_required("config.yaml", "/", None)
    ///     .expect("Failed to load required config.yaml");
    /// ```
    pub fn load_required(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_internal(filename, sep, env)
    }

    /// Loads a Config from a file, treating a missing file as an empty config.
    ///
    /// Use this when the config file is optional. If the file doesn't exist, returns
    /// `Ok` with an empty config. If the file *does* exist but is invalid (bad YAML/JSON/TOML,
    /// permission denied), returns `Err` — a present-but-broken config file is likely
    /// a mistake worth surfacing.
    ///
    /// For a file that must exist, use [`load_required`](Config::load_required).
    ///
    /// The resolved filename is recorded even when the file is missing, so a config
    /// loaded before its file exists can pick it up on a later [`reload`](Config::reload).
    /// Until the file appears, `reload` returns `IoError` (`NotFound`) and leaves the
    /// config unchanged.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Returns
    /// Returns `Ok(Config)` on success or if the file is not found
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file exists but cannot be read (e.g. permission denied)
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// // Load an environment-specific override file -- fine if it doesn't exist
    /// let config = Config::load_optional("config.dev.yaml", "/", None)?;
    ///
    /// // With custom separator
    /// let config = Config::load_optional("config.yaml", "::", None)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_optional(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        match Self::load_internal(filename, sep, env) {
            Ok(config) => Ok(config),
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                // Record the file this config *would* have come from. Discarding it
                // would leave the config with no source at all, and `reload()` refuses
                // to run without one — so a file that appears later could never be
                // picked up. `load_internal` resolved both `sep` and the filename
                // template before failing, so neither can fail here.
                let (file, env) = get_file(filename, env)?;
                Self::from_parsed(Value::Null, &file, sep, env, None)
            },
            Err(e) => Err(e),
        }
    }

    /// Loads a Config from a file, creating it from a default string if it doesn't exist.
    ///
    /// If the file exists, its content is loaded and returned — the `defaults` string is
    /// discarded. If the file does not exist, `defaults` is written to disk and returned as
    /// the active config, so the app behaves identically whether or not the file was present.
    ///
    /// The `defaults` string is written as-is, preserving formatting and comments. It must
    /// be in the same format as the file: YAML by default, or JSON/TOML when the filename
    /// has a matching extension and the corresponding feature is enabled. The created config
    /// records the filename, so [`reload`](Config::reload) works after a first run.
    ///
    /// Defaults that do not parse are rejected **before** anything is written, so a failure
    /// here leaves no file behind and the next run retries the creation from scratch. The
    /// file is created exclusively: if a second process wins the race to create it, this
    /// call loads that file rather than overwriting it.
    ///
    /// Only the file is created — **parent directories are not**. A missing parent returns
    /// `IoError` rather than being created, so a mistyped path cannot leave a junk directory
    /// tree behind; call [`std::fs::create_dir_all`] first if the directory may not exist.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    /// * `defaults` - Config string to write and use if the file does not exist, in the file's format
    ///
    /// # Returns
    /// Returns `Ok(Config)` with the file content, or the defaults if the file was created
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, the file exists but cannot be read, or if creating or writing the file fails
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file
    ///     or the defaults string cannot be parsed in the format matching the file extension.
    ///     A defaults string that fails to parse is reported without the file having been created
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// const DEFAULTS: &str = r#"
    /// app:
    ///   port: 8080
    ///   debug: false
    /// database:
    ///   host: localhost
    ///   port: 5432
    /// "#;
    ///
    /// let config = Config::load_or_create("config.yaml", "/", None, DEFAULTS)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_or_create(filename: &str, sep: &str, env: Option<&str>, defaults: &str) -> Result<Config, ConfigError> {
        match Self::load_internal(filename, sep, env) {
            Ok(config) => Ok(config),
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                let (file, _) = get_file(filename, env)?;

                // Validate the defaults *before* writing them. Writing first and parsing
                // second left a broken file on disk when the defaults did not parse in the
                // file's format — and because the file then existed, this branch never ran
                // again: every subsequent run read the same broken file and failed
                // identically, turning a first-run error into a permanent one.
                parser::parse_auto(defaults, &file)?;

                // The parsed value is deliberately discarded and the file re-read below,
                // so the created config is built by exactly the same path as an existing
                // one — filename recorded, `reload()` working, no second code path to
                // keep in step.
                match create_new_file(&file, defaults) {
                    Ok(()) => {},
                    // Another process created the file between the not-found check and
                    // here — the first-run race this method exists for. `create_new`
                    // means we did not clobber it; fall through and load what they wrote.
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
                    Err(e) => return Err(ConfigError::io_in(&file, e)),
                }

                Self::load_internal(filename, sep, env)
            },
            Err(e) => Err(e),
        }
    }

    fn load_internal(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_internal_as(filename, sep, env, None)
    }

    /// Loads a file with an explicitly chosen parser, or by extension when `format` is
    /// `None`.
    ///
    /// The one path every file constructor takes, so the filename check, the separator
    /// check and `{env}` resolution cannot differ between them. `load_json_file` and
    /// `load_toml_file` used to bypass all three, and skipping the last meant they alone
    /// could not take a `config.{env}.json` template.
    fn load_internal_as(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: Option<parser::Format>,
    ) -> Result<Config, ConfigError> {
        if filename.is_empty() {
            return Err(empty_filename_error());
        }
        check_separator(sep)?;

        let (file, env) = get_file(filename, env)?;
        let parsed = parser::load_in(format, &file)?;

        Self::from_parsed(parsed, &file, sep, env, format)
    }

    /// Builds a `Config` from an already-parsed document, resolving `${VAR}`
    /// placeholders in its string values.
    ///
    /// Every constructor funnels through here, so the shape of a freshly-loaded
    /// `Config` — no overlays, env vars resolved exactly once — is defined in one place.
    ///
    /// Callers validate `sep` with `check_separator` *before* parsing, so that an empty
    /// separator is reported ahead of any parse error.
    fn from_parsed(
        content: Value,
        filename: &str,
        sep: &str,
        env: Option<String>,
        format: Option<parser::Format>,
    ) -> Result<Config, ConfigError> {
        Ok(Config {
            content: resolve_env_vars(content)?,
            filename: filename.to_string(),
            separator: sep.to_string(),
            environment: env,
            overlays: Vec::new(),
            format,
        })
    }

    /// Parses a YAML string into a Config object
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    /// Returns `ConfigError::YamlError` if YAML parsing fails
    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::yaml::parse(yaml)?, "", sep, None, None)
    }

    /// Loads a Config from a JSON file, whatever its extension.
    ///
    /// [`load_required`](Config::load_required) already routes a `.json` file to the JSON
    /// parser, so the reason to reach for this is a file whose *extension does not name
    /// its format* — `settings.conf`, `app.cfg`, a file with no extension at all.
    ///
    /// The choice is recorded on the config, so [`reload`](Config::reload) and
    /// [`reload_from`](Config::reload_from) read it as JSON too. Without that the file was
    /// parsed as JSON once and as YAML ever after, which failed silently rather than
    /// loudly: YAML is a superset of JSON, so the reload usually *worked*, applying YAML's
    /// rules to a document that had been read under `serde_json`'s.
    ///
    /// Overlays are unaffected and still choose their own parser by their own extension,
    /// which is what lets a JSON base take a YAML overlay.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file is missing or cannot be read
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    /// Returns `ConfigError::JsonError` if JSON cannot be parsed
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// // A JSON document under an extension that does not say so
    /// let config = Config::load_json_file("settings.conf", "/", None)
    ///     .expect("Failed to load settings.conf");
    /// ```
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn load_json_file(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_internal_as(filename, sep, env, Some(parser::Format::Json))
    }

    /// Parses a JSON string into a Config object.
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    /// Returns `ConfigError::JsonError` if JSON parsing fails
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_json(r#"{"app": {"port": 8080}}"#, "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    /// ```
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn load_json(json_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::json::parse(json_str)?, "", sep, None, None)
    }

    /// Loads a Config from a TOML file, whatever its extension.
    ///
    /// The TOML counterpart to [`load_json_file`](Config::load_json_file), with the same
    /// reason to exist and the same recorded-format behaviour: reach for it when the
    /// extension does not name the format, and [`reload`](Config::reload) will read the
    /// file as TOML rather than falling back to YAML.
    ///
    /// TOML datetimes are read as strings — see [`load_toml`](Config::load_toml).
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file is missing or cannot be read
    /// Returns `ConfigError::TomlError` if the TOML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_toml_file("settings.conf", "/", None)
    ///     .expect("Failed to load settings.conf");
    /// ```
    #[cfg(feature = "toml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
    pub fn load_toml_file(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_internal_as(filename, sep, env, Some(parser::Format::Toml))
    }

    /// Parses a TOML string into a Config object.
    ///
    /// # Datetimes
    ///
    /// TOML has a date-time type and the value model this crate reads through does not,
    /// so a datetime is surfaced as the text the file contained: RFC 3339 for an offset
    /// date-time, and TOML's own forms for the local date, time and date-time variants.
    /// It behaves as a scalar like any other — readable with [`str`](Config::str), listed
    /// by [`outline`](Config::outline), and deserializable into a `String` or into
    /// `chrono`/`time`/`jiff`'s date types, all of which parse RFC 3339.
    ///
    /// It will *not* deserialize into `toml::value::Datetime`, which recognises only the
    /// private marker its own crate serializes; naming that type means taking a direct
    /// dependency on `toml`, which the [crate docs](crate#value-model) advise against.
    ///
    /// # Errors
    /// Returns `ConfigError::TomlError` if the TOML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_toml("[app]\nport = 8080", "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    ///
    /// // A datetime reads back as the text the file held
    /// let config = Config::load_toml("[window]\nstarts = 2024-01-01T00:00:00Z", "/").unwrap();
    /// assert_eq!(config.str("window/starts"), "2024-01-01T00:00:00Z");
    /// ```
    #[cfg(feature = "toml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
    pub fn load_toml(toml_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::toml::parse(toml_str)?, "", sep, None, None)
    }
}

/// Creates `file` and writes `contents`, failing with `AlreadyExists` if it is already there.
///
/// `fs::write` truncates, so it would silently clobber a config written by a second process
/// starting at the same moment — precisely the first-run scenario `load_or_create` exists
/// for. `create_new` makes the existence check and the creation a single atomic operation,
/// and turns the loser of the race into an `AlreadyExists` the caller can handle by loading
/// the winner's file.
///
/// The file is created empty and filled a moment later, so a racing reader can still
/// observe it part-written. Closing that would need a write-to-temp-and-rename dance, and
/// rename replaces the destination — reintroducing the clobbering this call prevents.
pub(super) fn create_new_file(file: &str, contents: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)?
        .write_all(contents.as_bytes())
}

/// Rejects a path separator that cannot work: empty, or containing a backslash.
///
/// An empty separator would make every config path unparseable. A backslash collides
/// with its own role as the escape character in path syntax (`src/config/path.rs`) —
/// the splitter tests for an escape sequence before it tests for the separator, so a
/// separator starting with `\` is consumed as an escape and never matches. The path
/// then collapses to a single segment and *every* lookup returns `None` / `""` / `[]`
/// with no error raised anywhere, which is the worst failure shape a config library
/// has. Any backslash is rejected, not just a leading one: a separator with one buried
/// in the middle happens to split correctly today, but the rule "the separator may not
/// contain the escape character" is the one worth being able to state.
///
/// Called at the top of each constructor, before parsing, so that a bad separator is
/// reported ahead of any parse error in the document.
fn check_separator(sep: &str) -> Result<(), ConfigError> {
    if sep.is_empty() {
        return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
    }
    if sep.contains('\\') {
        return Err(ConfigError::FormatError(format!(
            "Separator {:?} cannot contain a backslash: '\\' is the escape character \
             used to put a literal separator in a key",
            sep
        )));
    }
    Ok(())
}

/// Resolves a filename template against an optional environment name, returning the
/// concrete filename and the environment to record on the `Config`.
///
/// An environment supplies a value for `{env}` *if the filename uses it*. A filename
/// without the placeholder is not an error: in a layered setup only some of the files
/// are environment-specific, and the environment is still worth recording.
///
/// The reverse is an error. A `{env}` with nothing to substitute would otherwise be
/// handed to the OS verbatim and come back as a missing `config.{env}.yaml`, which
/// points at the wrong problem.
///
/// `env` is substituted into a filesystem path with no validation, so a value of
/// `../../secrets` builds exactly that path. Callers pass a literal or a trusted
/// `APP_ENV`, which is what makes this acceptable; the constructors' rustdoc says
/// not to pass untrusted input.
pub(super) fn get_file(filename: &str, env: Option<&str>) -> Result<(String, Option<String>), ConfigError> {
    let has_placeholder = filename.contains("{env}");

    match (env, has_placeholder) {
        (Some(value), true) => Ok((filename.replace("{env}", value), Some(value.to_string()))),
        (Some(value), false) => Ok((filename.to_string(), Some(value.to_string()))),
        (None, true) => Err(ConfigError::FormatError(format!(
            "Filename template \"{}\" contains '{{env}}' but no environment was supplied",
            filename
        ))),
        (None, false) => Ok((filename.to_string(), None)),
    }
}

/// The error returned when a config filename is empty.
///
/// An empty filename is almost always a caller bug rather than a "missing file",
/// so it is rejected upfront with `InvalidInput` instead of being handed to the OS
/// (which would report `NotFound`, indistinguishable from a genuinely-absent named file).
pub(super) fn empty_filename_error() -> ConfigError {
    ConfigError::IoError {
        file: None,
        source: io::Error::new(io::ErrorKind::InvalidInput, "filename cannot be empty"),
    }
}
