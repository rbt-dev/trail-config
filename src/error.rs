use std::io;
use thiserror::Error;

/// The detail behind [`ConfigError::YamlError`] and [`ConfigError::DeserializeError`].
///
/// Both come from the value model this crate reads through: the first from parsing YAML
/// text into it, the second from deserializing a document out of it into your type. The
/// second happens for JSON and TOML configs too, which is why this is named for the value
/// model rather than for YAML.
///
/// # Why it is opaque
///
/// The value model is [`yaml_serde`](https://docs.rs/yaml_serde)'s, and that crate is
/// `0.x`, where Cargo treats **every minor release as semver-incompatible**. Exposing its
/// error type directly — which this crate did, re-exported as `YamlError` — meant a
/// `0.10 → 0.11` bump there changed the identity of a type in this crate's public API, so
/// a routine dependency update became a breaking release here. Wrapping it moves that
/// boundary inside: the inner type can change without any signature here changing.
///
/// The same argument does *not* apply to [`ConfigError::JsonError`] and
/// [`ConfigError::TomlError`], which still carry `serde_json::Error` and `toml::de::Error`
/// concretely. Both of those crates are `1.x`, so naming their types costs nothing and
/// gives callers everything those errors offer. The asymmetry is the point rather than an
/// oversight — this wraps the one dependency whose version is a liability.
///
/// What you can still do with it: print it (the `Display` text is the underlying error's,
/// unchanged), and ask a parse error where it happened with [`location`](Self::location).
/// What you can no longer do is match on the upstream error's own variants, which is the
/// trade.
#[derive(Debug)]
pub struct ValueError(yaml_serde::Error);

impl ValueError {
    /// The 1-based line and column the error points at, when it has one.
    ///
    /// A parse error from reading YAML text usually does; an error from deserializing an
    /// already-parsed document into a type usually does not, because by then there is no
    /// text to point into — and for a config that came from JSON or TOML there never was
    /// any YAML text to begin with.
    ///
    /// # Example
    /// ```
    /// # use trail_config::{Config, ConfigError};
    /// let err = Config::load_yaml("app:\n  - [unclosed\n", "/").unwrap_err();
    ///
    /// if let ConfigError::YamlError { source, .. } = &err {
    ///     if let Some((line, column)) = source.location() {
    ///         eprintln!("bad YAML at {line}:{column}");
    ///     }
    /// }
    /// ```
    pub fn location(&self) -> Option<(usize, usize)> {
        self.0.location().map(|at| (at.line(), at.column()))
    }
}

impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ValueError {
    /// Deliberately `None`: this *is* the underlying error, repackaged, not a wrapper
    /// around a further one. Returning the inner `yaml_serde::Error` would put it back in
    /// reach of `downcast_ref`, which is the coupling this type exists to remove.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<yaml_serde::Error> for ValueError {
    fn from(source: yaml_serde::Error) -> Self {
        ValueError(source)
    }
}

/// Error type for all fallible Trail Config operations.
///
/// Load and parse errors record the file they occurred in (`file` is `None`
/// when the config was parsed from a string) and preserve the underlying
/// error, available via [`std::error::Error::source`].
///
/// # Matching on it
///
/// The enum and its struct variants are `#[non_exhaustive]`, so a `match` from
/// another crate needs a `_ => …` arm, and a struct variant's fields are bound with
/// a trailing `..`:
///
/// ```
/// # use trail_config::{Config, ConfigError};
/// match Config::load_required("config.yaml", "/", None) {
///     Ok(config) => { /* ... */ },
///     Err(ConfigError::IoError { file, .. }) => {
///         eprintln!("could not read {}", file.as_deref().unwrap_or("the config"));
///     },
///     Err(ConfigError::PathNotFound(path)) => eprintln!("missing: {path}"),
///     Err(e) => eprintln!("{e}"),
/// }
/// ```
///
/// This is deliberate: the variant list has grown twice already (`JsonError` /
/// `TomlError` with their features, then `DeserializeError`), and the `json` and
/// `toml` features vary which variants exist at all. Without `#[non_exhaustive]`
/// every such addition silently breaks any consumer that matched exhaustively, which
/// would force a major version for what is otherwise a purely additive change.
///
/// `PathNotFound` and `FormatError` are exempt. They carry one `String` and nothing
/// could be added to them without replacing them outright, so they stay directly
/// matchable — `ConfigError::PathNotFound(path)` binds the path as it always has.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// File I/O failure (missing file, permission denied, ...).
    #[error("IO error{}: {source}", fmt_file(.file))]
    #[non_exhaustive]
    IoError {
        /// The file being read or written, if the error relates to one.
        file: Option<String>,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// YAML parsing or deserialization failure.
    #[error("YAML parse error{}: {source}", fmt_file(.file))]
    #[non_exhaustive]
    YamlError {
        /// The file being parsed, or `None` for string input or `from_value` failures.
        file: Option<String>,
        /// The underlying YAML error, from parsing or from deserializing into a type.
        source: ValueError,
    },

    /// JSON parsing failure (requires the `json` feature).
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    #[error("JSON parse error{}: {source}", fmt_file(.file))]
    #[non_exhaustive]
    JsonError {
        /// The file being parsed, or `None` for string input.
        file: Option<String>,
        /// The underlying JSON parse error.
        source: serde_json::Error,
    },

    /// TOML parsing failure (requires the `toml` feature).
    #[cfg(feature = "toml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
    #[error("TOML parse error{}: {source}", fmt_file(.file))]
    #[non_exhaustive]
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
    /// variant names no format. Deserialization runs through the same value model for JSON
    /// and TOML configs too, which is why the underlying error is a [`ValueError`]
    /// regardless of where the document came from — and why that type is named for the
    /// value model rather than for YAML.
    #[error("Cannot deserialize {}: {source}", fmt_target(.path, .file))]
    #[non_exhaustive]
    DeserializeError {
        /// The file the document came from, or `None` for a config parsed from a string.
        file: Option<String>,
        /// The path of the subtree being deserialized, or `None` for the whole document.
        path: Option<String>,
        /// The underlying error from the value model.
        source: ValueError,
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

    /// Takes `Option<&str>` like [`json_in`](Self::json_in) and [`toml_in`](Self::toml_in),
    /// so a string parse (`None`) and a file parse go through one constructor. The `None`
    /// case used to be a public `From<yaml_serde::Error>` impl, which named the wrapped
    /// crate in this crate's API for no benefit a caller could use.
    pub(crate) fn yaml_in(file: Option<&str>, source: yaml_serde::Error) -> Self {
        ConfigError::YamlError { file: file.map(str::to_string), source: source.into() }
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

