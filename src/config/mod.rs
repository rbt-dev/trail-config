use std::{env, fs, io};
use yaml_serde::{Value, from_str};
use crate::error::ConfigError;

#[derive(Debug, Clone)]
enum OverlaySource {
    Required(String),
    Optional(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    content: Value,
    filename: String,
    separator: String,
    environment: Option<String>,
    overlays: Vec<OverlaySource>,
}

impl Default for Config {
    /// Creates a Config, attempting to load from `config.yaml` if it exists.
    ///
    /// If `config.yaml` is found and valid, it will be loaded. If the file doesn't exist
    /// or fails to parse, this returns an empty config without panicking.
    ///
    /// Shorthand for `Config::load_optional("config.yaml", "/", None)`.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::default(); // Loads config.yaml if it exists, or returns empty config
    /// // Always succeeds - never panics
    /// ```
    fn default() -> Self {
        Self::load_optional("config.yaml", "/", None)
            .unwrap_or_else(|_| Config {
                content: Value::Null,
                filename: String::new(),
                separator: "/".to_string(),
                environment: None,
                overlays: Vec::new(),
            })
    }
}

impl Config {
    /// Loads a Config from a YAML file, returning an error if the file is missing or invalid.
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
    /// Returns `Ok(Config)` if the file is found and valid YAML, or `Err(ConfigError)` otherwise
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, empty filename, or cannot be read
    /// Returns `ConfigError::YamlError` if the YAML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_required("config.yaml", "/", None)
    ///     .expect("Failed to load required config.yaml");
    /// ```
    pub fn load_required(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        if filename.is_empty() {
            return Err(ConfigError::IoError(io::Error::new(
                io::ErrorKind::InvalidInput,
                "load_required: filename cannot be empty",
            )));
        }
        Self::load_internal(filename, sep, env)
    }

    /// Loads a Config from a YAML file, treating a missing file as an empty config.
    ///
    /// Use this when the config file is optional. If the file doesn't exist, returns
    /// `Ok` with an empty config. If the file *does* exist but is invalid (bad YAML,
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
    /// Returns `ConfigError::IoError` if the file exists but cannot be read (e.g. permission denied)
    /// Returns `ConfigError::YamlError` if the file exists but contains invalid YAML
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
            Err(ConfigError::IoError(ref e)) if e.kind() == io::ErrorKind::NotFound => {
                Ok(Config {
                    content: Value::Null,
                    filename: String::new(),
                    separator: sep.to_string(),
                    environment: env.map(|s| s.to_string()),
                    overlays: Vec::new(),
                })
            },
            Err(e) => Err(e),
        }
    }

    /// Loads a Config from a YAML file, creating it from a default YAML string if it doesn't exist.
    ///
    /// If the file exists, its content is loaded and returned — the `defaults` string is
    /// discarded. If the file does not exist, `defaults` is written to disk and returned as
    /// the active config, so the app behaves identically whether or not the file was present.
    ///
    /// The `defaults` string is written as-is, preserving formatting and comments.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename
    /// * `defaults` - YAML string to write and use if the file does not exist
    ///
    /// # Returns
    /// Returns `Ok(Config)` with the file content, or the defaults if the file was created
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file exists but cannot be read, or if writing fails
    /// Returns `ConfigError::YamlError` if the file or defaults string contains invalid YAML
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
            Err(ConfigError::IoError(ref e)) if e.kind() == io::ErrorKind::NotFound => {
                let (file, _) = Self::get_file(filename, env)?;
                fs::write(&file, defaults)?;
                Self::load_yaml(defaults, sep)
            },
            Err(e) => Err(e),
        }
    }

    fn load_internal(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        if sep.is_empty() {
            return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
        }

        let (file, env) = Self::get_file(filename, env)?;

        match Self::load(&file) {
            Ok(yaml) => Ok(Config {
                content: Self::resolve_env_vars(yaml)?,
                filename: file,
                separator: sep.to_string(),
                environment: env,
                overlays: Vec::new(),
            }),
            Err(e) => Err(e)
        }
    }

    /// Returns the environment name used when loading the config file
    pub fn environment(&self) -> Option<&str> {
        self.environment.as_deref()
    }

    /// Returns the filename of the loaded config file
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// Merges a required overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, an error is returned.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::YamlError` if the file contains invalid YAML
    /// Returns `ConfigError::FormatError` if the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn merge_required(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        let (file, _) = Self::get_file(filename, env)?;
        let yaml = Self::load(&file)?;
        self.content = Self::resolve_env_vars(Self::merge_values(self.content, yaml))?;
        self.overlays.push(OverlaySource::Required(file));
        Ok(self)
    }

    /// Merges an optional overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, it is silently skipped.
    /// If the file exists but contains invalid YAML, an error is returned.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Errors
    /// Returns `ConfigError::YamlError` if the file exists but contains invalid YAML
    /// Returns `ConfigError::FormatError` if the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn merge_optional(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        let (file, _) = Self::get_file(filename, env)?;
        match Self::load(&file) {
            Ok(yaml) => {
                self.content = Self::resolve_env_vars(Self::merge_values(self.content, yaml))?;
            },
            Err(ConfigError::IoError(ref e)) if e.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
        self.overlays.push(OverlaySource::Optional(file));
        Ok(self)
    }

    fn merge_values(base: Value, overlay: Value) -> Value {
        match (base, overlay) {
            (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
                for (key, overlay_val) in overlay_map {
                    let merged = match base_map.remove(&key) {
                        Some(base_val) => Self::merge_values(base_val, overlay_val),
                        None => overlay_val,
                    };
                    base_map.insert(key, merged);
                }
                Value::Mapping(base_map)
            },
            // A null overlay (e.g. from an empty Config) is a no-op — preserve the base
            (base, Value::Null) => base,
            // Sequences are replaced wholesale; all other types are overridden by overlay
            (_, overlay) => overlay,
        }
    }

    /// Reloads the configuration from disk, re-applying all overlays in order.
    ///
    /// Re-reads the base file and each overlay file that was added via
    /// [`merge_required`](Config::merge_required) or [`merge_optional`](Config::merge_optional),
    /// then re-merges them in the original order. Required overlays that are missing will
    /// return an error; optional overlays that are missing are silently skipped.
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if any required file cannot be read or is invalid YAML
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if no file path is associated with this config
    /// Returns `ConfigError::IoError` if the base file or a required overlay is missing or cannot be read
    /// Returns `ConfigError::YamlError` if any file contains invalid YAML
    ///
    /// # Note
    /// If reloading fails, the existing configuration is preserved unchanged.
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let mut config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.prod.yaml", None)?
    ///     .merge_optional("config.local.yaml", None)?;
    /// // Later, reload all files from disk
    /// config.reload()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        if self.filename.is_empty() {
            return Err(ConfigError::FormatError("Cannot reload: no file path associated with this config".to_string()));
        }

        let mut content = Self::load(&self.filename)?;
        
        for overlay in &self.overlays {
            match overlay {
                OverlaySource::Required(filename) => {
                    let yaml = Self::load(filename)?;
                    content = Self::merge_values(content, yaml);
                },
                OverlaySource::Optional(filename) => {
                    match Self::load(filename) {
                        Ok(yaml) => {
                            content = Self::merge_values(content, yaml);
                        },
                        Err(ConfigError::IoError(ref e)) if e.kind() == io::ErrorKind::NotFound => {},
                        Err(e) => return Err(e),
                    }
                },
            }
        }

        self.content = Self::resolve_env_vars(content)?;
        Ok(())
    }

    /// Reloads the configuration from a different file
    ///
    /// Changes the config's filename and reloads from the new file.
    /// The separator and environment settings remain the same.
    ///
    /// # Arguments
    /// * `filename` - New config file to load
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if the file cannot be read or is invalid YAML
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::YamlError` if the YAML cannot be parsed
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let mut config = Config::default();
    /// // Switch to loading from a different config file
    /// config.reload_from("other_config.yaml").expect("Failed to load");
    /// ```
    pub fn reload_from(&mut self, filename: &str) -> Result<(), ConfigError> {
        let yaml = Self::load(filename)?;
        self.filename = filename.to_string();
        self.content = Self::resolve_env_vars(yaml)?;
        self.overlays.clear();
        Ok(())
    }

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
        match Self::get_leaf(&self.content, path, &self.separator) {
            Some(Value::Sequence(v)) => v.iter().map(Self::to_string).collect(),
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
        Self::get_leaf(&self.content, path, &self.separator).is_some()
    }

    /// Gets a value at the specified path, returning an error if not found
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let value = config.get_strict("db/redis/port").unwrap();
    /// ```
    pub fn get_strict(&self, path: &str) -> Result<Value, ConfigError> {
        Self::get_leaf(&self.content, path, &self.separator)
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        Self::to_string_strict(&value, path)
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        match &value {
            Value::Sequence(v) => Ok(v.iter().map(Self::to_string).collect()),
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match &value {
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match &value {
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;

        match &value {
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
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        yaml_serde::from_value(value)
            .map_err(|e| ConfigError::YamlError(e.to_string()))
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
        yaml_serde::from_value(self.content.clone())
            .map_err(|e| ConfigError::YamlError(e.to_string()))
    }

    /// Formats a string template with values from the config
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt("{}:{}", "db/redis", &["server", "port"]);
    /// assert_eq!(result, "127.0.0.1:6379");
    /// ```
    pub fn fmt(&self, format: &str, base: &str, keys: &[&str]) -> String {
        self.fmt_strict(format, base, keys).unwrap_or_else(|_| String::new())
    }

    /// Parses a YAML string into a Config object
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if separator is empty
    /// Returns `ConfigError::YamlError` if YAML parsing fails
    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, ConfigError> {
        if sep.is_empty() {
            return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
        }

        let parsed = from_str(yaml)?;

        Ok(Config {
            content: Self::resolve_env_vars(parsed)?,
            filename: String::new(),
            separator: sep.to_string(),
            environment: None,
            overlays: Vec::new(),
        })
    }

    /// Formats a string template with values from the config, returning an error if any value is missing
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt_strict("{}:{}", "db/redis", &["server", "port"]).unwrap();
    /// assert_eq!(result, "127.0.0.1:6379");
    /// ```
    pub fn fmt_strict(&self, format: &str, base: &str, keys: &[&str]) -> Result<String, ConfigError> {
        let mut content = &self.content;
        let parts = Self::parse_path(base, &self.separator);

        for item in parts.iter() {
            if item.is_empty() { continue; }
            match content.get(item.as_str()) {
                Some(v) => { content = v; },
                None => return Err(ConfigError::PathNotFound(base.to_string()))
            }
        }

        let mut result = format.to_string();

        for key in keys.iter() {
            match content.get(*key) {
                Some(v) => {
                    result = result.replacen("{}", &Self::to_string(v), 1);
                },
                None => return Err(ConfigError::PathNotFound(format!("{}/{}", base, key)))
            }
        }

        Ok(result)
    }

    fn get_leaf(mut content: &Value, path: &str, separator: &str) -> Option<Value> {
        if path.is_empty() || separator.is_empty() {
            return None;
        }

        let parts = Self::parse_path(path, separator);

        for item in parts.iter() {
            if item.is_empty() {
                continue;
            }
            match content.get(item) {
                Some(v) => { content = v; },
                None => return None
            }
        }

        Some(content.clone())
    }

    /// Parses a path with escape sequence support.
    ///
    /// - `\<sep>` becomes a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
    /// - `\\` becomes a literal backslash in the key
    fn parse_path(path: &str, separator: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = path.chars().peekable();
        let sep_first_char = separator.chars().next().unwrap_or('/');

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                let remaining: String = chars.clone().collect();
                if remaining.starts_with(separator) {
                    current.push_str(separator);
                    for _ in 0..separator.chars().count() {
                        chars.next();
                    }
                } else if let Some(&next) = chars.peek() {
                    if next == '\\' {
                        current.push('\\');
                        chars.next();
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            } else if ch == sep_first_char {
                let remaining: String = chars.clone().collect();
                let expected_rest = &separator[sep_first_char.len_utf8()..];
                if remaining.starts_with(expected_rest) {
                    parts.push(current.clone());
                    current.clear();
                    for _ in 1..separator.chars().count() {
                        chars.next();
                    }
                } else {
                    current.push(ch);
                }
            } else {
                current.push(ch);
            }
        }

        parts.push(current);
        parts
    }

    fn get_file(filename: &str, env: Option<&str>) -> Result<(String, Option<String>), ConfigError> {
        match env {
            Some(v) => {
                if filename.contains("{env}") {
                    Ok((filename.replace("{env}", v), Some(v.to_string())))
                } else {
                    Err(ConfigError::FormatError(format!("Invalid filename template: '{{env}}' placeholder not found in \"{}\"", filename)))
                }
            },
            None => Ok((String::from(filename), None))
        }
    }

    fn load(filename: &str) -> Result<Value, ConfigError> {
        let yaml = fs::read_to_string(filename)?;
        let parsed = from_str(&yaml)?;
        Ok(parsed)
    }

    fn to_string(value: &Value) -> String {
        match value {
            Value::String(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            Value::Bool(v) => v.to_string(),
            _ => String::new()
        }
    }

    fn to_string_strict(value: &Value, path: &str) -> Result<String, ConfigError> {
        match value {
            Value::String(v) => Ok(v.to_string()),
            Value::Number(v) => Ok(v.to_string()),
            Value::Bool(v) => Ok(v.to_string()),
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a scalar", path)))
        }
    }

    /// Recursively walks the Value tree and resolves `${VAR}` and `${VAR:-default}`
    /// placeholders in all string values using environment variables.
    fn resolve_env_vars(value: Value) -> Result<Value, ConfigError> {
        match value {
            Value::String(s) => {
                let resolved = Self::resolve_env_string(&s)?;
                Ok(Value::String(resolved))
            },
            Value::Mapping(map) => {
                let mut resolved_map = yaml_serde::Mapping::new();
                for (k, v) in map {
                    resolved_map.insert(k, Self::resolve_env_vars(v)?);
                }
                Ok(Value::Mapping(resolved_map))
            },
            Value::Sequence(seq) => {
                let resolved_seq: Result<Vec<Value>, ConfigError> =
                    seq.into_iter().map(Self::resolve_env_vars).collect();
                Ok(Value::Sequence(resolved_seq?))
            },
            other => Ok(other),
        }
    }

    /// Resolves all `${VAR}` and `${VAR:-default}` placeholders in a single string.
    fn resolve_env_string(input: &str) -> Result<String, ConfigError> {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut placeholder = String::new();
                let mut found_close = false;

                for c in chars.by_ref() {
                    if c == '}' {
                        found_close = true;
                        break;
                    }
                    placeholder.push(c);
                }

                if !found_close {
                    return Err(ConfigError::FormatError(
                        format!("Unclosed env var placeholder in: {}", input)
                    ));
                }

                let (var_name, default) = match placeholder.find(":-") {
                    Some(pos) => (&placeholder[..pos], Some(&placeholder[pos + 2..])),
                    None => (placeholder.as_str(), None),
                };

                if var_name.is_empty() {
                    return Err(ConfigError::FormatError(
                        format!("Empty env var name in: {}", input)
                    ));
                }

                match env::var(var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => match default {
                        Some(d) => result.push_str(d),
                        None => return Err(ConfigError::FormatError(
                            format!("Environment variable '{}' is not set and no default provided", var_name)
                        )),
                    }
                }
            } else {
                result.push(ch);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests;
