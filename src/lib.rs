//! Read config files with path-based access, typed deserialization, environment
//! overlays, deep merging, environment variable interpolation and hot reload.
//!
//! ```
//! use trail_config::Config;
//!
//! let config = Config::load_yaml("app:\n  port: 8080\n  debug: true", "/")?;
//!
//! assert_eq!(config.str("app/port"), "8080");
//! assert_eq!(config.get_bool("app/debug"), Some(true));
//! # Ok::<(), trail_config::ConfigError>(())
//! ```
//!
//! # Loading
//!
//! Four constructors, differing only in what they do about a missing file:
//!
//! | Constructor | Missing file |
//! | ----------- | ------------ |
//! | [`Config::load_required`] | Error |
//! | [`Config::load_optional`] | Empty config, filename still recorded for [`reload`](Config::reload) |
//! | [`Config::load_or_create`] | Written from the supplied defaults |
//! | [`Config::default`] | Empty config — shorthand for `load_optional("config.yaml", "/", None)`, panics on a broken file |
//!
//! Each takes an optional environment name, which supplies a value for an `{env}`
//! placeholder in the filename. [`Config::load_yaml`] (and [`load_json`](Config::load_json) /
//! [`load_toml`](Config::load_toml) behind their feature flags) parse from a string instead.
//!
//! # Reading
//!
//! Every accessor comes in two styles: lenient methods return `Option<T>` or an empty
//! default, and `*_strict` methods return [`Result<T, ConfigError>`](ConfigError).
//!
//! ```
//! # use trail_config::{Config, ConfigError};
//! let config = Config::load_yaml("db:\n  host: localhost\n  port: 5432", "/")?;
//!
//! // Lenient — "" / None on a missing path
//! assert_eq!(config.str("db/host"), "localhost");
//! assert_eq!(config.get_int("db/missing"), None);
//!
//! // Strict — a descriptive error instead
//! assert!(matches!(
//!     config.str_strict("db/missing"),
//!     Err(ConfigError::PathNotFound(_))
//! ));
//! # Ok::<(), ConfigError>(())
//! ```
//!
//! Paths join keys with the config's separator (`/` above, but any string works) and
//! navigate mappings only. Every segment must be non-empty, so `/db/host`, `db//host`
//! and `db/host/` all fail rather than being quietly accepted. A key that genuinely
//! contains the separator is escaped with a backslash: `db/host\/port`.
//!
//! Whole subtrees deserialize into your own types with [`get_as`](Config::get_as) /
//! [`deserialize`](Config::deserialize), and sibling values format into a string with
//! [`fmt`](Config::fmt).
//!
//! # Layering and reloading
//!
//! [`merge_required`](Config::merge_required) and [`merge_optional`](Config::merge_optional)
//! deep-merge overlay files onto a base, preserving sibling keys and the base's key order.
//! An overlay value takes precedence whatever it is, so a key set to null clears the base
//! value rather than being ignored. The overlay chain is recorded, so
//! [`reload`](Config::reload) re-reads and re-applies everything in order:
//!
//! ```no_run
//! # use trail_config::{Config, ConfigError};
//! let mut config = Config::load_required("config.yaml", "/", Some("prod"))?
//!     .merge_required("config.{env}.yaml", Some("prod"))?
//!     .merge_optional("config.local.yaml", Some("prod"))?;
//!
//! config.reload()?; // re-reads the base and every overlay
//! # Ok::<(), ConfigError>(())
//! ```
//!
//! [`ConfigHandle`] wraps a `Config` for sharing across threads, handing out immutable
//! snapshots and reloading without blocking readers on disk I/O.
//!
//! # Value model
//!
//! [`Value`] — what [`Config::get`] returns — and the error types behind [`ConfigError`]'s
//! parse variants are re-exported here, along with [`Mapping`], [`Sequence`] and
//! [`Number`]. Name them through this crate rather than adding the underlying crates as
//! dependencies, so the types always match the versions this crate resolved.
//!
//! # Environment variables
//!
//! String values are interpolated at load time, and again on every reload:
//! `${VAR}` requires the variable to be set, `${VAR:-default}` falls back if it is
//! absent, and `$${` is a literal `${`. Values only — a `${VAR}` written as a *key* stays
//! literal, so the set of valid config paths never depends on the environment.
//!
//! # Debugging
//!
//! [`Config::outline`] lists every path in the document with the values replaced by their
//! types, spelled exactly as the accessors take them. `Debug` and `outline` both elide
//! values on purpose: interpolation has already happened by the time a `Config` exists, so
//! anything that printed the document would print the secrets in it.
//!
//! # Feature flags
//!
//! `json` and `toml` add the corresponding parsers. Format is chosen by file extension,
//! case-insensitively, so a YAML base can take a JSON or TOML overlay once the feature
//! is enabled.
//!
//! See the [README](https://github.com/rbt-dev/trail-config) for the full guide.

#![warn(missing_docs)]

mod error;
mod config;
mod handle;

#[cfg(test)]
mod test_util;

pub use error::ConfigError;
pub use config::Config;
pub use handle::ConfigHandle;

// The value model and the underlying error types appear in this crate's public API —
// `get` returns a `Value`, and every parse-error variant of `ConfigError` carries the
// originating error as a public `source` field. Without these re-exports a caller could
// not *name* what those APIs hand back: `get` was usable only through `Debug`, and
// matching on `ConfigError::YamlError { source }` gave a binding of an unnameable type.
// The alternative — adding `yaml_serde` as a direct dependency — meant guessing the
// version this crate resolved, and picking a different one produces two incompatible
// `Value` types whose error message names the same path twice.
//
// Re-exporting does not settle the larger question of whether the value model should be
// a crate-local type so `yaml_serde` (still 0.x, where every minor bump is breaking) can
// be upgraded without a breaking release here. It makes the current API honest, and
// forecloses nothing.

/// The parsed value model, re-exported from [`yaml_serde`](https://docs.rs/yaml_serde).
///
/// [`Config::get`] and [`Config::get_strict`] return a [`Value`], and its variants carry
/// [`Mapping`], [`Sequence`] and [`Number`]. Prefer [`Config::get_as`] for a typed struct;
/// reach for these when you want the raw document.
pub use yaml_serde::{Mapping, Number, Sequence, Value};

/// The error carried by [`ConfigError::YamlError`], re-exported from
/// [`yaml_serde`](https://docs.rs/yaml_serde).
///
/// Covers both YAML parse failures and failures to deserialize a document into a type.
pub use yaml_serde::Error as YamlError;

/// The error carried by [`ConfigError::JsonError`], re-exported from
/// [`serde_json`](https://docs.rs/serde_json).
#[cfg(feature = "json")]
pub use serde_json::Error as JsonError;

/// The error carried by [`ConfigError::TomlError`], re-exported from
/// [`toml`](https://docs.rs/toml).
#[cfg(feature = "toml")]
pub use toml::de::Error as TomlError;

/// Macro for building a [`Config`] with a concise syntax.
///
/// Loads a config file, optionally sets a separator and environment,
/// and applies required and optional overlays in order.
///
/// # Examples
///
/// ```no_run
/// # use trail_config::config;
/// // Minimal
/// let cfg = config!("config.yaml");
///
/// // With all options
/// let cfg = config! {
///     file: "config.yaml",
///     sep: "::",
///     env: "prod",
///     merge: ["config.{env}.yaml"],
///     merge_optional: ["config.local.yaml"]
/// };
/// ```
#[macro_export]
macro_rules! config {
    // Minimal: config!("file.yaml")
    ($file:expr) => {
        $crate::Config::load_required($file, "/", None)
    };

    // Positional with sep: config!("file.yaml", sep: "::")
    ($file:expr, sep: $sep:expr) => {
        $crate::Config::load_required($file, $sep, None)
    };

    // Positional with env: config!("file.yaml", env: "prod")
    ($file:expr, env: $env:expr) => {
        $crate::Config::load_required($file, "/", Some($env))
    };

    // Positional with merge: config!("file.yaml", merge: ["overlay.yaml"])
    ($file:expr, merge: [$($req:expr),* $(,)?]) => {{
        let _cfg = $crate::Config::load_required($file, "/", None);
        $(
            let _cfg = _cfg.and_then(|c| c.merge_required($req, None));
        )*
        _cfg
    }};

    // Block syntax: config! { file: "...", ... }
    ( file: $file:expr $(, sep: $sep:expr)? $(, env: $env:expr)? $(, merge: [$($req:expr),* $(,)?])? $(, merge_optional: [$($opt:expr),* $(,)?])? $(,)? ) => {{
        let _sep = config!(@sep $($sep)?);
        let _env: Option<&str> = config!(@env $($env)?);

        let _cfg = $crate::Config::load_required($file, _sep, _env);

        $($(
            let _cfg = _cfg.and_then(|c| c.merge_required($req, _env));
        )*)?

        $($(
            let _cfg = _cfg.and_then(|c| c.merge_optional($opt, _env));
        )*)?

        _cfg
    }};

    // Internal helpers
    (@sep) => { "/" };
    (@sep $sep:expr) => { $sep };
    (@env) => { None };
    (@env $env:expr) => { Some($env) };
}