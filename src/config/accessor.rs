//! Reading values out of a config: raw, string, typed, and deserialized.

use yaml_serde::Value;
use crate::error::ConfigError;
use super::Config;
use super::path::get_leaf;

impl Config {
    /// Gets a value at the specified path
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
    pub fn list(&self, path: &str) -> Vec<String> {
        match get_leaf(&self.content, path, &self.separator) {
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
        match value {
            Value::Sequence(v) => Ok(v.iter().map(to_string).collect()),
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

        match value {
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

        match value {
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

        match value {
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
    /// Returns `ConfigError::YamlError` if the value cannot be deserialized into `T`
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
        T::deserialize(value).map_err(ConfigError::from)
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
    /// Returns `ConfigError::YamlError` if the config cannot be deserialized into `T`
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
        T::deserialize(&self.content).map_err(ConfigError::from)
    }
}

pub(super) fn to_string(value: &Value) -> String {
    match value {
        Value::String(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        _ => String::new()
    }
}

pub(super) fn to_string_strict(value: &Value, path: &str) -> Result<String, ConfigError> {
    match value {
        Value::String(v) => Ok(v.to_string()),
        Value::Number(v) => Ok(v.to_string()),
        Value::Bool(v) => Ok(v.to_string()),
        _ => Err(ConfigError::FormatError(format!("Value at {} is not a scalar", path)))
    }
}
