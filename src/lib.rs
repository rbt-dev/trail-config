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
//! Segments are matched as strings, so a non-string key — YAML permits `1:` and `true:` —
//! has no path: `retries/1` looks up the string `"1"`, a different key from the integer
//! `1`. [`outline`](Config::outline) lists such keys and marks them, and the subtree
//! containing them still deserializes (`get_as::<BTreeMap<i64, String>>`).
//!
//! A YAML `!Tag` — how serde spells an enum variant — is transparent to reading and
//! addressing, and preserved for deserializing: `db/host` resolves whether or not `db`
//! is tagged, while [`get`](Config::get) and [`get_as`](Config::get_as) still see the
//! tag so the variant can be selected.
//!
//! Whole subtrees deserialize into your own types with [`get_as`](Config::get_as) /
//! [`deserialize`](Config::deserialize), and sibling values format into a string with
//! [`fmt`](Config::fmt).
//!
//! ```
//! # use serde::Deserialize;
//! # use trail_config::{Config, ConfigError};
//! #[derive(Deserialize)]
//! struct Db { host: String, port: u16 }
//!
//! # let config = Config::load_yaml("db:\n  host: localhost\n  port: 5432", "/")?;
//! let db: Db = config.get_as_strict("db")?;
//! assert_eq!(db.port, 5432);
//! # Ok::<(), ConfigError>(())
//! ```
//!
//! **Prefer this over [`get`](Config::get) where you can.** The typed accessors are generic
//! over your own types, so they never mention this crate's value model — which means your
//! code does not depend on what that model happens to be, and a struct states the shape
//! your program actually expects in one place instead of at every call site. Reach for
//! `get` when you genuinely want the raw document: inspecting an unknown shape, or walking
//! something whose keys are not known ahead of time.
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
//!     .merge_required("config.{env}.yaml", None)?  // `None` reuses the config's "prod"
//!     .merge_optional("config.local.yaml", None)?;
//!
//! config.reload()?; // re-reads the base and every overlay
//! # Ok::<(), ConfigError>(())
//! ```
//!
//! A merge resolves `{env}` against the environment the config already carries, so the
//! environment is named once. Pass `Some` to a merge only to give one overlay a
//! *different* environment than the base, or when the base has none of its own.
//!
//! `Config` is `Send + Sync`, so a config that never changes is shared across threads as
//! an `Arc<Config>`. [`ConfigHandle`] is for the case that does change: it swaps the
//! document behind shared references, handing out immutable snapshots and reloading
//! without blocking readers on disk I/O.
//!
//! # Value model
//!
//! [`Value`] — what [`Config::get`] returns — is re-exported here along with [`Mapping`],
//! [`Sequence`] and [`Number`], as are the concrete error types behind
//! [`ConfigError::JsonError`] and [`ConfigError::TomlError`]. Name them through this crate
//! rather than adding the underlying crates as dependencies, so the types always match the
//! versions this crate resolved.
//!
//! Most programs never need any of them. [`get_as`](Config::get_as) and
//! [`deserialize`](Config::deserialize) are generic over your own types, so the value model
//! stays an implementation detail unless you ask for it — which is the recommended way
//! round, and the reason [`ConfigError::YamlError`] and [`ConfigError::DeserializeError`]
//! carry a crate-local [`ValueError`] rather than the underlying crate's error type.
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
//! is enabled. When the extension does not name the format — `settings.conf`, a file with
//! no extension — name it with [`Format`] instead: each file constructor has an `_as` twin
//! ([`load_required_as`](Config::load_required_as),
//! [`load_optional_as`](Config::load_optional_as),
//! [`load_or_create_as`](Config::load_or_create_as)) that takes one and records it, so
//! every later [`reload`](Config::reload) reads the file the same way. TOML's date-time
//! type has no counterpart in the value model above, so a datetime is read as the text the
//! file contained — see [`load_toml`](Config::load_toml).
//!
//! # Further reading
//!
//! The full guide lives in the repository, one file per topic — see the
//! [documentation index](https://github.com/rbt-dev/trail-config/blob/main/docs/README.md).
//! It goes further than this page on [loading](https://github.com/rbt-dev/trail-config/blob/main/docs/LOADING.md),
//! [merging](https://github.com/rbt-dev/trail-config/blob/main/docs/MERGING.md),
//! [environment variables](https://github.com/rbt-dev/trail-config/blob/main/docs/ENV_INTERPOLATION.md)
//! and [error handling](https://github.com/rbt-dev/trail-config/blob/main/docs/ERROR_HANDLING.md),
//! and [`examples/`](https://github.com/rbt-dev/trail-config/tree/main/examples) holds four
//! runnable programs.

#![warn(missing_docs)]
// Lets rustdoc render a "Available on crate feature `json`" banner beside each
// feature-gated item. `doc_cfg` is nightly-only, so it is requested behind `docsrs` —
// a cfg set exclusively by the docs.rs build (see `Cargo.toml`), never by an ordinary
// `cargo build` or `cargo doc`. The MSRV is therefore untouched.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;
mod config;
mod handle;

#[cfg(test)]
mod test_util;

pub use error::{ConfigError, ValueError};
pub use config::{Config, Format};
pub use handle::ConfigHandle;

// The value model appears in this crate's public API — `get` returns a `Value` — so
// without this re-export a caller could not *name* what that hands back, and `get` was
// usable only through `Debug`. The alternative, adding `yaml_serde` as a direct
// dependency, meant guessing the version this crate resolved; picking a different one
// produces two incompatible `Value` types whose error message names the same path twice.
//
// The errors used to be re-exported the same way and no longer are: `ConfigError`'s YAML
// and deserialize variants carry a crate-local `ValueError` instead, so a `yaml_serde`
// minor bump — always semver-breaking, since it is 0.x — cannot change a type in this
// crate's API. See `ValueError`'s docs for why the JSON and TOML variants keep their
// concrete error types.
//
// `Value` itself is still `yaml_serde`'s, which leaves the larger question open: a
// crate-local value model would let that dependency be upgraded without a breaking
// release here, at the cost of implementing `Deserializer` for it by hand. Narrowing the
// surface to one type rather than five makes that decision cheaper either way, and
// forecloses nothing.

/// The parsed value model, re-exported from [`yaml_serde`](https://docs.rs/yaml_serde).
///
/// Reach for these when you want the raw document. For reading configuration — which is
/// almost always the job — prefer [`Config::get_as`] and [`Config::deserialize`], which are
/// generic over your own types and never mention this model at all; see
/// [Reading](crate#reading).
pub use yaml_serde::{Mapping, Number, Sequence, Value};

/// The error carried by [`ConfigError::JsonError`], re-exported from
/// [`serde_json`](https://docs.rs/serde_json).
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use serde_json::Error as JsonError;

/// The error carried by [`ConfigError::TomlError`], re-exported from
/// [`toml`](https://docs.rs/toml).
#[cfg(feature = "toml")]
#[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
pub use toml::de::Error as TomlError;

/// Macro for building a [`Config`] with a concise syntax.
///
/// Loads a config file, optionally sets a separator and environment,
/// and applies required and optional overlays in order.
///
/// # Syntax
///
/// Two spellings, differing only in whether the filename is labelled. Both take the same
/// options, and all of them are optional:
///
/// | Option | Meaning | Default |
/// | ------ | ------- | ------- |
/// | `sep:` | Path separator | `"/"` |
/// | `env:` | Environment name, for `{env}` in any filename | `None` |
/// | `merge:` | Required overlays, applied in order | none |
/// | `merge_optional:` | Optional overlays, applied after the required ones | none |
///
/// **Options must appear in that order.** Writing them in any other — `sep:` after `env:`,
/// `merge:` before either — is a "no rules expected this token" error pointing at the
/// second one. Lifting that would take an accumulator arm consuming the options in any
/// order, roughly tripling the macro for a fixed list of four; the constraint is cheaper to
/// remember than to remove. Everything else composes freely: any subset, in that order,
/// with or without a trailing comma.
///
/// # Examples
///
/// ```no_run
/// # use trail_config::config;
/// // Minimal
/// let cfg = config!("config.yaml");
///
/// // Any subset of the options, positionally
/// let cfg = config!("config.yaml", sep: "::");
/// let cfg = config!("config.{env}.yaml", sep: "::", env: "prod");
/// let cfg = config!("config.yaml", env: "prod", merge: ["config.{env}.yaml"]);
///
/// // The same options under the block spelling
/// let cfg = config! {
///     file: "config.yaml",
///     sep: "::",
///     env: "prod",
///     merge: ["config.{env}.yaml"],
///     merge_optional: ["config.local.yaml"]
/// };
/// ```
///
/// The macro can equally be called by its full path, with nothing imported:
///
/// ```no_run
/// let cfg = trail_config::config! {
///     file: "config.yaml",
///     sep: "::",
///     env: "prod",
/// };
/// ```
#[macro_export]
macro_rules! config {
    // Positional: the filename, then any of the options the block form takes, in the
    // same order. One arm rather than one per option — there used to be four, each
    // matching the file plus exactly one option, so `config!("f.yaml", sep: "::",
    // env: "prod")` was a "no rules expected this token" error pointing at `env`. The
    // guide presents the options as a menu, which is how option lists read, and three
    // of the combinations it implied did not exist.
    //
    // Delegates to the block arm below rather than repeating its body, so the two
    // spellings cannot drift.
    (
        $file:expr
        $(, sep: $sep:expr)?
        $(, env: $env:expr)?
        $(, merge: [$($req:expr),* $(,)?])?
        $(, merge_optional: [$($opt:expr),* $(,)?])?
        $(,)?
    ) => {
        $crate::config! {
            file: $file
            $(, sep: $sep)?
            $(, env: $env)?
            $(, merge: [$($req),*])?
            $(, merge_optional: [$($opt),*])?
        }
    };

    // Block syntax: config! { file: "...", ... }
    ( file: $file:expr $(, sep: $sep:expr)? $(, env: $env:expr)? $(, merge: [$($req:expr),* $(,)?])? $(, merge_optional: [$($opt:expr),* $(,)?])? $(,)? ) => {{
        // `$crate::`, not a bare `config!`: macro_rules expansion is textual and
        // resolved at the call site, so an unqualified recursion compiles only when
        // the caller happens to have `use trail_config::config;` in scope. The
        // documented `trail_config::config! { .. }` form has no such import.
        let _sep = $crate::config!(@sep $($sep)?);
        let _env: Option<&str> = $crate::config!(@env $($env)?);

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