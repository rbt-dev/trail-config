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
//! # Environment variables
//!
//! String values are interpolated at load time, and again on every reload:
//! `${VAR}` requires the variable to be set, `${VAR:-default}` falls back if it is
//! absent, and `$${` is a literal `${`.
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