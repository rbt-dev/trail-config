use std::io;
use thiserror::Error;

/// Error type for all fallible Trail Config operations.
///
/// Load and parse errors record the file they occurred in (`file` is `None`
/// when the config was parsed from a string) and preserve the underlying
/// error, available via [`std::error::Error::source`].
#[derive(Debug, Error)]
pub enum ConfigError {
    /// File I/O failure (missing file, permission denied, ...).
    #[error("IO error{}: {source}", fmt_file(.file))]
    IoError {
        /// The file being read or written, if the error relates to one.
        file: Option<String>,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// YAML parsing or deserialization failure.
    #[error("YAML parse error{}: {source}", fmt_file(.file))]
    YamlError {
        /// The file being parsed, or `None` for string input or `from_value` failures.
        file: Option<String>,
        /// The underlying YAML error, from parsing or from deserializing into a type.
        source: yaml_serde::Error,
    },

    /// JSON parsing failure (requires the `json` feature).
    #[cfg(feature = "json")]
    #[error("JSON parse error{}: {source}", fmt_file(.file))]
    JsonError {
        /// The file being parsed, or `None` for string input.
        file: Option<String>,
        /// The underlying JSON parse error.
        source: serde_json::Error,
    },

    /// TOML parsing failure (requires the `toml` feature).
    #[cfg(feature = "toml")]
    #[error("TOML parse error{}: {source}", fmt_file(.file))]
    TomlError {
        /// The file being parsed, or `None` for string input.
        file: Option<String>,
        /// The underlying TOML parse error.
        source: toml::de::Error,
    },

    /// A document (or a subtree of one) could not be deserialized into the requested type.
    ///
    /// Distinct from the parse errors above, which report a file that could not be *read*
    /// as its format. Here the file parsed fine, whatever that format was, and the
    /// mismatch is between the resulting document and the Rust type asked for — so this
    /// variant names no format. Deserialization runs through the `yaml_serde` value model
    /// for JSON and TOML configs too, which is why the underlying error is a
    /// [`yaml_serde::Error`] regardless of where the document came from.
    #[error("Cannot deserialize {}: {source}", fmt_target(.path, .file))]
    DeserializeError {
        /// The file the document came from, or `None` for a config parsed from a string.
        file: Option<String>,
        /// The path of the subtree being deserialized, or `None` for the whole document.
        path: Option<String>,
        /// The underlying error from the value model.
        source: yaml_serde::Error,
    },

    /// Configuration path not found in the document.
    #[error("Path not found in config: {0}")]
    PathNotFound(String),

    /// String formatting or configuration error.
    #[error("Format error: {0}")]
    FormatError(String),
}

fn fmt_file(file: &Option<String>) -> String {
    match file {
        Some(f) => format!(" in {}", f),
        None => String::new(),
    }
}

/// Names what a deserialization was attempted on: a subtree, a file, both, or neither.
fn fmt_target(path: &Option<String>, file: &Option<String>) -> String {
    match (path.as_deref(), file.as_deref()) {
        (Some(path), Some(file)) => format!("{} in {}", path, file),
        (Some(path), None) => path.to_string(),
        (None, Some(file)) => file.to_string(),
        (None, None) => "the config".to_string(),
    }
}

impl ConfigError {
    pub(crate) fn io_in(file: &str, source: io::Error) -> Self {
        ConfigError::IoError { file: Some(file.to_string()), source }
    }

    pub(crate) fn yaml_in(file: &str, source: yaml_serde::Error) -> Self {
        ConfigError::YamlError { file: Some(file.to_string()), source }
    }

    #[cfg(feature = "json")]
    pub(crate) fn json_in(file: Option<&str>, source: serde_json::Error) -> Self {
        ConfigError::JsonError { file: file.map(str::to_string), source }
    }

    #[cfg(feature = "toml")]
    pub(crate) fn toml_in(file: Option<&str>, source: toml::de::Error) -> Self {
        ConfigError::TomlError { file: file.map(str::to_string), source }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::IoError { file: None, source: err }
    }
}

impl From<yaml_serde::Error> for ConfigError {
    fn from(err: yaml_serde::Error) -> Self {
        ConfigError::YamlError { file: None, source: err }
    }
}
