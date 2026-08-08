//! Reading values out of a config: raw, string, typed, and deserialized.
//!
//! Paths navigate **mappings only**. A sequence has no addressable elements: `items/0`
//! is not a path into the first element, just a lookup for a key named `0`, and it fails
//! like any other missing key. Read a sequence whole with [`Config::list`], or into a
//! typed field with [`Config::get_as`] / [`Config::deserialize`].
//!
//! Segments are matched as **strings**, so a key that is not one has no path. YAML permits
//! `1:`, `true:`, `~:`, even a sequence as a key; `retries/1` looks up the *string* `"1"`,
//! which is a different key from the integer `1`, and one document can hold both. Such keys
//! are not hidden — [`Config::outline`] lists them, marked as not addressable — and the
//! subtree containing them is still readable by deserializing it:
//!
//! ```
//! # use std::collections::BTreeMap;
//! # use trail_config::Config;
//! let config = Config::load_yaml("retries:\n  1: fast\n  2: slow\n", "/").unwrap();
//!
//! assert!(!config.contains("retries/1"));            // no path reaches an integer key
//! let retries: BTreeMap<i64, String> = config.get_as("retries").unwrap();
//! assert_eq!(retries[&1], "fast");                   // deserializing does
//! ```
//!
//! A YAML `!Tag` is **transparent to reading and addressing, and preserved for
//! deserializing**. `db/host` resolves whether or not `db` is tagged, and `str` on a
//! tagged scalar returns the scalar — the tag names a serde enum variant, which says
//! nothing about how the value is read. [`Config::get`] and [`Config::get_as`] are the
//! exception and still see the tag, because selecting that variant is what it is for.

use yaml_serde::Value;
use crate::error::ConfigError;
use super::Config;
use super::path::get_leaf;

impl Config {
    /// Gets a raw value at the specified path.
    ///
    /// Returns the crate's [`Value`](crate::Value), so this is the one accessor whose
    /// return type is the value model itself. Prefer [`get_as`](Config::get_as) — or
    /// [`str`](Config::str) / [`get_int`](Config::get_int) / [`get_bool`](Config::get_bool)
    /// for a single scalar — which are generic over your own types and keep the model out
    /// of your code. Reach for this when you genuinely want the raw document: inspecting a
    /// shape you do not know ahead of time, or walking keys that are not fixed.
    ///
    /// # Arguments
    /// * `path` - Path to the value (e.g., "db/redis/port")
    ///
    /// # Returns
    /// Returns `Some(Value)` if found, `None` otherwise
    pub fn get(&self, path: &str) -> Option<Value> {
        self.get_strict(path).ok()
    }

    /// Gets a value as a string at the specified path
    ///
    /// # Arguments
    /// * `path` - Path to the value
    ///
    /// # Returns
    /// Returns the string representation of the value, or empty string if not found or not convertible
    pub fn str(&self, path: &str) -> String {
        self.str_strict(path).unwrap_or_else(|_| String::new())
    }

    /// Gets a value as a list of strings at the specified path
    ///
    /// # Arguments
    /// * `path` - Path to the sequence value
    ///
    /// # Returns
    /// Returns a `Vec<String>` with the sequence elements, or empty vec if not found or not a sequence
    ///
    /// Elements that are not scalars — a nested mapping or sequence, or a null — render as
    /// empty strings, in keeping with the rest of the lenient API. Use
    /// [`list_strict`](Config::list_strict) to have them reported instead.
    pub fn list(&self, path: &str) -> Vec<String> {
        match get_leaf(&self.content, path, &self.separator).map(untagged) {
            Some(Value::Sequence(v)) => v.iter().map(to_string).collect(),
            _ => vec![]
        }
    }

    /// Checks if a path exists in the configuration
    ///
    /// # Arguments
    /// * `path` - Path to check
    ///
    /// # Returns
    /// Returns `true` if the path exists, `false` otherwise
    pub fn contains(&self, path: &str) -> bool {
        get_leaf(&self.content, path, &self.separator).is_some()
    }

    /// Gets a value at the specified path, returning an error if not found
    ///
    /// Returns an owned `Value`, so the subtree at `path` is cloned. When the target is
    /// a mapping or sequence and you want it as a struct, prefer
    /// [`get_as_strict`](Config::get_as_strict), which deserializes from a borrow and
    /// does not clone.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let value = config.get_strict("db/redis/port").unwrap();
    /// ```
    pub fn get_strict(&self, path: &str) -> Result<Value, ConfigError> {
        get_leaf(&self.content, path, &self.separator)
            .cloned()
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))
    }

    /// Gets a value as a string at the specified path, returning an error if not found
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "app:\n  port: 8080";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let port = config.str_strict("app/port").unwrap();
    /// assert_eq!(port, "8080");
    /// ```
    pub fn str_strict(&self, path: &str) -> Result<String, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        to_string_strict(value, path)
    }

    /// Gets a value as a list of strings at the specified path, returning an error if not found
    ///
    /// Every element must be a scalar. A mapping, a sequence or a null among them is a
    /// `FormatError` naming the offending element as `path[index]` — unlike
    /// [`list`](Config::list), which renders them as empty strings, indistinguishable from
    /// an element that genuinely is `""`. The index is written in brackets because a
    /// sequence element is not addressable as a path: `items/0` is a lookup for a key
    /// named `0`, so rendering one would name something the accessors cannot resolve.
    ///
    /// # Errors
    /// Returns `ConfigError::PathNotFound` if the path does not exist
    /// Returns `ConfigError::FormatError` if the value is not a sequence, or if any
    ///     element is not a scalar
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "items:\n  - first\n  - second";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let list = config.list_strict("items").unwrap();
    /// assert_eq!(list.len(), 2);
    /// ```
    pub fn list_strict(&self, path: &str) -> Result<Vec<String>, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        match untagged(value) {
            // The whole point of the strict half is to report rather than paper over, and
            // checking only the container left every element unchecked. `collect` into a
            // `Result` stops at the first bad element, so the message names one place to look.
            Value::Sequence(seq) => seq
                .iter()
                .enumerate()
                .map(|(index, element)| {
                    scalar_to_string(element)
                        .ok_or_else(|| not_a_scalar(&format!("{}[{}]", path, index)))
                })
                .collect(),
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a sequence", path)))
        }
    }

    /// Gets a value as an integer at the specified path
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "app:\n  port: 8080";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let port = config.get_int("app/port");
    /// assert_eq!(port, Some(8080));
    /// ```
    pub fn get_int(&self, path: &str) -> Option<i64> {
        self.get_int_strict(path).ok()
    }

    /// Gets a value as an integer at the specified path, returning an error if not found or not a number
    pub fn get_int_strict(&self, path: &str) -> Result<i64, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match untagged(value) {
            Value::Number(num) => {
                num.as_i64()
                    .ok_or_else(|| ConfigError::FormatError(format!("Cannot convert {} to i64", num)))
            },
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a number", path)))
        }
    }

    /// Gets a value as a floating-point number at the specified path
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "app:\n  timeout: 3.14";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let timeout = config.get_float("app/timeout");
    /// assert!(timeout.is_some());
    /// ```
    pub fn get_float(&self, path: &str) -> Option<f64> {
        self.get_float_strict(path).ok()
    }

    /// Gets a value as a floating-point number at the specified path, returning an error if not found or not a number
    pub fn get_float_strict(&self, path: &str) -> Result<f64, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match untagged(value) {
            Value::Number(num) => {
                num.as_f64()
                    .ok_or_else(|| ConfigError::FormatError(format!("Cannot convert {} to f64", num)))
            },
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a number", path)))
        }
    }

    /// Gets a value as a boolean at the specified path
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "app:\n  debug: true";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let debug = config.get_bool("app/debug");
    /// assert_eq!(debug, Some(true));
    /// ```
    pub fn get_bool(&self, path: &str) -> Option<bool> {
        self.get_bool_strict(path).ok()
    }

    /// Gets a value as a boolean at the specified path, returning an error if not found or not a boolean
    pub fn get_bool_strict(&self, path: &str) -> Result<bool, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match untagged(value) {
            Value::Bool(b) => Ok(*b),
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a boolean", path)))
        }
    }

    /// Deserializes a config subtree at the specified path into a typed struct
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # use serde::Deserialize;
    /// # let yaml = "database:\n  host: localhost\n  port: 5432";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// #[derive(Deserialize)]
    /// struct DatabaseConfig {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let db: Option<DatabaseConfig> = config.get_as("database");
    /// ```
    pub fn get_as<T: serde::de::DeserializeOwned>(&self, path: &str) -> Option<T> {
        self.get_as_strict(path).ok()
    }

    /// Deserializes a config subtree at the specified path into a typed struct, returning an error if not found or deserialization fails
    ///
    /// # Errors
    /// Returns `ConfigError::PathNotFound` if the path does not exist
    /// Returns `ConfigError::DeserializeError`, naming the path and the file, if the
    ///     value cannot be deserialized into `T`
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # use serde::Deserialize;
    /// # let yaml = "database:\n  host: localhost\n  port: 5432";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// #[derive(Deserialize)]
    /// struct DatabaseConfig {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let db: DatabaseConfig = config.get_as_strict("database").unwrap();
    /// assert_eq!(db.host, "localhost");
    /// assert_eq!(db.port, 5432);
    /// ```
    pub fn get_as_strict<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ConfigError> {
        let value = get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        // Deserialize straight from the borrowed subtree. `yaml_serde::from_value`
        // takes `Value` by value and would force a deep clone of the subtree first.
        T::deserialize(value).map_err(|e| self.deserialize_error(Some(path), e))
    }

    /// Deserializes the entire config into a typed struct
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # use serde::Deserialize;
    /// # let yaml = "app:\n  port: 8080\ndatabase:\n  host: localhost\n  port: 5432";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// #[derive(Deserialize)]
    /// struct AppConfig {
    ///     app: AppSettings,
    ///     database: DatabaseSettings,
    /// }
    /// #[derive(Deserialize)]
    /// struct AppSettings { port: u16 }
    /// #[derive(Deserialize)]
    /// struct DatabaseSettings { host: String, port: u16 }
    ///
    /// let cfg: Option<AppConfig> = config.deserialize();
    /// ```
    pub fn deserialize<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        self.deserialize_strict().ok()
    }

    /// Deserializes the entire config into a typed struct, returning an error if deserialization fails
    ///
    /// # Errors
    /// Returns `ConfigError::DeserializeError`, naming the file, if the config cannot be
    ///     deserialized into `T`
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # use serde::Deserialize;
    /// # let yaml = "app:\n  port: 8080\ndatabase:\n  host: localhost\n  port: 5432";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// #[derive(Deserialize)]
    /// struct AppConfig {
    ///     app: AppSettings,
    ///     database: DatabaseSettings,
    /// }
    /// #[derive(Deserialize)]
    /// struct AppSettings { port: u16 }
    /// #[derive(Deserialize)]
    /// struct DatabaseSettings { host: String, port: u16 }
    ///
    /// let cfg: AppConfig = config.deserialize_strict().unwrap();
    /// assert_eq!(cfg.app.port, 8080);
    /// assert_eq!(cfg.database.host, "localhost");
    /// ```
    pub fn deserialize_strict<T: serde::de::DeserializeOwned>(&self) -> Result<T, ConfigError> {
        // Borrowed, not cloned — see `get_as_strict`. This matters most here, where
        // the alternative is deep-cloning the entire document on every call.
        T::deserialize(&self.content).map_err(|e| self.deserialize_error(None, e))
    }

    /// Builds the error for a failed deserialization, attributing it to this config's
    /// file and — for [`get_as_strict`](Config::get_as_strict) — the subtree path.
    ///
    /// These used to go through `From<yaml_serde::Error>` and surface as `YamlError`,
    /// rendering as "YAML parse error: …" even for a config loaded from `.toml`.
    /// Mechanically true — deserialization runs through the `yaml_serde` value model
    /// whatever the source format — but a caller has to know the crate's internals for
    /// that to make sense, and nothing was parsed at this point in any case.
    fn deserialize_error(&self, path: Option<&str>, source: yaml_serde::Error) -> ConfigError {
        ConfigError::DeserializeError {
            file: (!self.filename.is_empty()).then(|| self.filename.clone()),
            path: path.map(str::to_string),
            source: source.into(),
        }
    }
}

/// Looks through any `!Tag` wrapping a value.
///
/// A tag names a serde enum variant; it says nothing about how the value is *read*. The
/// value model already takes this view when indexing — `Value::get("key")` untags before
/// looking the key up — so `db/host` resolves whether or not `db` is tagged. The readers
/// below have to agree, or a tagged scalar would resolve as a path and then read back as
/// `""` from `str` and `None` from every typed accessor.
///
/// Looping rather than unwrapping once mirrors the value model, which allows a tag to
/// wrap a tag.
///
/// The tag is only skipped for *reading*. [`Config::get`] and
/// [`get_as`](Config::get_as) still see the tagged value, because deserializing an enum
/// is exactly what the tag is for.
pub(super) fn untagged(mut value: &Value) -> &Value {
    while let Value::Tagged(tagged) = value {
        value = &tagged.value;
    }
    value
}

/// Renders a scalar as a string, or `None` for a mapping, a sequence or a null.
///
/// The single definition of what "converts to a string" means, so the lenient and strict
/// accessors can never disagree about which values do.
pub(super) fn scalar_to_string(value: &Value) -> Option<String> {
    match untagged(value) {
        Value::String(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        Value::Bool(v) => Some(v.to_string()),
        _ => None
    }
}

pub(super) fn not_a_scalar(path: &str) -> ConfigError {
    ConfigError::FormatError(format!("Value at {} is not a scalar", path))
}

pub(super) fn to_string(value: &Value) -> String {
    scalar_to_string(value).unwrap_or_default()
}

pub(super) fn to_string_strict(value: &Value, path: &str) -> Result<String, ConfigError> {
    scalar_to_string(value).ok_or_else(|| not_a_scalar(path))
}
