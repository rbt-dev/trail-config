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
