//! Constructors: loading a `Config` from files or strings.

use std::{fs, io};
use yaml_serde::Value;
use crate::error::ConfigError;
use super::Config;
use super::env::resolve_env_vars;
use super::parser::{self, load_auto};

impl Config {
    /// Loads a Config from a file, returning an error if the file is missing or invalid.
    ///
    /// Use this in production code where a missing config file is a critical error.
    /// For optional config files, use [`load_optional`](Config::load_optional) or [`default`](Config::default).
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Returns
    /// Returns `Ok(Config)` if the file is found and valid, or `Err(ConfigError)` otherwise
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, empty filename, or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or filename template is invalid
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
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Returns
    /// Returns `Ok(Config)` on success or if the file is not found
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file exists but cannot be read (e.g. permission denied)
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or filename template is invalid
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
                // `load_internal` validated `sep` before failing, so this cannot fail on it
                Self::from_parsed(Value::Null, "", sep, env.map(|s| s.to_string()))
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
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename
    /// * `defaults` - Config string to write and use if the file does not exist, in the file's format
    ///
    /// # Returns
    /// Returns `Ok(Config)` with the file content, or the defaults if the file was created
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, the file exists but cannot be read, or if writing fails
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file
    ///     or the defaults string cannot be parsed in the format matching the file extension
    /// Returns `ConfigError::FormatError` if the separator is empty or filename template is invalid
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
                fs::write(&file, defaults).map_err(|e| ConfigError::io_in(&file, e))?;
                // Load the file we just wrote, so the defaults are parsed
                // according to the file's format (YAML/JSON/TOML by extension)
                // and the filename is recorded for reload()
                Self::load_internal(filename, sep, env)
            },
            Err(e) => Err(e),
        }
    }

    fn load_internal(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        if filename.is_empty() {
            return Err(empty_filename_error());
        }
        check_separator(sep)?;

        let (file, env) = get_file(filename, env)?;
        let parsed = load_auto(&file)?;

        Self::from_parsed(parsed, &file, sep, env)
    }

    /// Builds a `Config` from an already-parsed document, resolving `${VAR}`
    /// placeholders in its string values.
    ///
    /// Every constructor funnels through here, so the shape of a freshly-loaded
    /// `Config` — no overlays, env vars resolved exactly once — is defined in one place.
    ///
    /// Callers validate `sep` with `check_separator` *before* parsing, so that an empty
    /// separator is reported ahead of any parse error.
    fn from_parsed(content: Value, filename: &str, sep: &str, env: Option<String>) -> Result<Config, ConfigError> {
        Ok(Config {
            content: resolve_env_vars(content)?,
            filename: filename.to_string(),
            separator: sep.to_string(),
            environment: env,
            overlays: Vec::new(),
        })
    }

    /// Parses a YAML string into a Config object
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if separator is empty
    /// Returns `ConfigError::YamlError` if YAML parsing fails
    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::yaml::parse(yaml)?, "", sep, None)
    }

    /// Loads a Config from a JSON file, returning an error if the file is missing or invalid.
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::FormatError` if the separator is empty
    /// Returns `ConfigError::JsonError` if JSON cannot be parsed
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_json_file("config.json", "/")
    ///     .expect("Failed to load config.json");
    /// ```
    #[cfg(feature = "json")]
    pub fn load_json_file(filename: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::json::load_file(filename)?, filename, sep, None)
    }

    /// Parses a JSON string into a Config object.
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if separator is empty
    /// Returns `ConfigError::JsonError` if JSON parsing fails
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_json(r#"{"app": {"port": 8080}}"#, "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    /// ```
    #[cfg(feature = "json")]
    pub fn load_json(json_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::json::parse(json_str)?, "", sep, None)
    }

    /// Loads a Config from a TOML file, returning an error if the file is missing or invalid.
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::TomlError` if the TOML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_toml_file("config.toml", "/")
    ///     .expect("Failed to load config.toml");
    /// ```
    #[cfg(feature = "toml")]
    pub fn load_toml_file(filename: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::toml::load_file(filename)?, filename, sep, None)
    }

    /// Parses a TOML string into a Config object.
    ///
    /// # Errors
    /// Returns `ConfigError::TomlError` if the TOML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_toml("[app]\nport = 8080", "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    /// ```
    #[cfg(feature = "toml")]
    pub fn load_toml(toml_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::toml::parse(toml_str)?, "", sep, None)
    }
}

/// Rejects an empty path separator, which would make every config path unparseable.
///
/// Called at the top of each constructor, before parsing, so that an empty separator
/// is reported ahead of any parse error in the document.
fn check_separator(sep: &str) -> Result<(), ConfigError> {
    if sep.is_empty() {
        return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
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
