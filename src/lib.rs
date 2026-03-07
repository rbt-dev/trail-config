use std::{collections::HashMap, error::Error, fmt, fs, io};
use serde_yaml_bw::{Value, from_str};
use strfmt::strfmt;

/// Custom error type for Trail Config operations
#[derive(Debug)]
pub enum ConfigError {
    /// File I/O error (file not found, permission denied, etc.)
    IoError(io::Error),
    /// YAML parsing error
    YamlError(String),
    /// Path not found in configuration
    PathNotFound(String),
    /// String formatting error
    FormatError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::YamlError(msg) => write!(f, "YAML parse error: {}", msg),
            ConfigError::PathNotFound(path) => write!(f, "Path not found in config: {}", path),
            ConfigError::FormatError(msg) => write!(f, "Format error: {}", msg),
        }
    }
}

impl Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::IoError(err)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    content: Value,
    filename: String,
    separator: String,
    environment: Option<String>
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
                content: Value::Null(None),
                filename: String::new(),
                separator: "/".to_string(),
                environment: None
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
                    content: Value::Null(None),
                    filename: String::new(),
                    separator: sep.to_string(),
                    environment: env.map(|s| s.to_string()),
                })
            },
            Err(e) => Err(e),
        }
    }

    fn load_internal(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        // Validate separator
        if sep.is_empty() {
            return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
        }

        let (file, env) = Self::get_file(filename, env)?;

        match Self::load(&file) {
            Ok(yaml) => Ok(Config {
                content: yaml,
                filename: file,
                separator: sep.to_string(),
                environment: env
            }),
            Err(e) => Err(e)
        }
    }

    pub fn environment(&self) -> Option<&str> {
        self.environment.as_deref()
    }

    /// Returns the filename of the loaded config file
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// Merges another `Config` into this one, returning a new `Config`.
    ///
    /// Values in `overlay` take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// The returned `Config` inherits the separator and filename of `self`. Calls can be
    /// chained to apply multiple overlays in order:
    ///
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge(Config::load_optional("config.prod.yaml", "/", None)?)
    ///     .merge(Config::load_optional("config.local.yaml", "/", None)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn merge(self, overlay: Config) -> Config {
        Config {
            content: Self::merge_values(self.content, overlay.content),
            filename: self.filename,
            separator: self.separator,
            environment: self.environment,
        }
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
            (base, Value::Null(_)) => base,
            // Sequences are replaced wholesale; all other types are overridden by overlay
            (_, overlay) => overlay,
        }
    }

    /// Reloads the configuration from disk
    ///
    /// This allows you to update the config without creating a new Config instance.
    /// Useful for detecting configuration changes at runtime (hot reload).
    ///
    /// # Returns
    /// Returns `Ok(())` on success, or `Err(ConfigError)` if the file cannot be read or is invalid YAML
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read
    /// Returns `ConfigError::YamlError` if the YAML cannot be parsed
    ///
    /// # Note
    /// If reloading fails (e.g. the file contains invalid YAML or has been deleted), the
    /// existing configuration is preserved unchanged. The error is returned but the config
    /// remains valid and usable.
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let mut config = Config::default();
    /// // ... use config ...
    /// // Later, reload updated config from disk
    /// config.reload().expect("Failed to reload config");
    /// ```
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        if self.filename.is_empty() {
            return Err(ConfigError::FormatError("Cannot reload: config was loaded from YAML string, not a file".to_string()));
        }
        
        let yaml = Self::load(&self.filename)?;
        self.content = yaml;
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
        self.content = yaml;
        Ok(())
    }

    /// Gets a value at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value (e.g., "db/redis/port")
    ///
    /// # Returns
    /// Returns `Some(Value)` if found, `None` otherwise
    pub fn get(&self, path: &str) -> Option<Value> {
        self.get_strict(path).ok()
    }

    /// Gets a value as a string at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns the string representation of the value, or empty string if not found or not convertible
    pub fn str(&self, path: &str) -> String {
        self.str_strict(path).unwrap_or_else(|_| String::new())
    }

    /// Gets a value as a list of strings at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the sequence value
    ///
    /// # Returns
    /// Returns a `Vec<String>` with the sequence elements, or empty vec if not found or not a sequence
    pub fn list(&self, path: &str) -> Vec<String> {
        self.list_strict(path).unwrap_or_else(|_| vec![])
    }

    /// Checks if a path exists in the configuration
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to check
    ///
    /// # Returns
    /// Returns `true` if the path exists, `false` otherwise
    pub fn contains(&self, path: &str) -> bool {
        Self::get_leaf(&self.content, path, &self.separator).is_some()
    }

    /// Gets a value at the specified path, returning an error if not found
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value (e.g., "db/redis/port")
    ///
    /// # Returns
    /// Returns `Ok(Value)` if found, or `Err(ConfigError::PathNotFound)` if not found
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
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Ok(String)` with the string representation, or `Err(ConfigError::PathNotFound)` if not found
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
        Ok(Self::to_string(&value))
    }

    /// Gets a value as a list of strings at the specified path, returning an error if not found
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the sequence value
    ///
    /// # Returns
    /// Returns `Ok(Vec<String>)` if found and is a sequence, or `Err(ConfigError::PathNotFound)` if not found
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
        Ok(Self::to_list(&value))
    }

    /// Gets a value as an integer at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Some(i64)` if the value is a number, `None` otherwise
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
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Ok(i64)` if found and is a number, or `Err(ConfigError)` otherwise
    pub fn get_int_strict(&self, path: &str) -> Result<i64, ConfigError> {
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        
        match &value {
            Value::Number(num, _) => {
                num.as_i64()
                    .ok_or_else(|| ConfigError::FormatError(format!("Cannot convert {} to i64", num)))
            },
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a number", path)))
        }
    }

    /// Gets a value as a floating-point number at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Some(f64)` if the value is a number, `None` otherwise
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
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Ok(f64)` if found and is a number, or `Err(ConfigError)` otherwise
    pub fn get_float_strict(&self, path: &str) -> Result<f64, ConfigError> {
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        
        match &value {
            Value::Number(num, _) => {
                num.as_f64()
                    .ok_or_else(|| ConfigError::FormatError(format!("Cannot convert {} to f64", num)))
            },
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a number", path)))
        }
    }

    /// Gets a value as a boolean at the specified path
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Some(bool)` if the value is a boolean, `None` otherwise
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
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the value
    ///
    /// # Returns
    /// Returns `Ok(bool)` if found and is a boolean, or `Err(ConfigError)` otherwise
    pub fn get_bool_strict(&self, path: &str) -> Result<bool, ConfigError> {
        let value = Self::get_leaf(&self.content, path, &self.separator)
            .ok_or_else(|| ConfigError::PathNotFound(path.to_string()))?;
        
        match &value {
            Value::Bool(b, _) => Ok(*b),
            _ => Err(ConfigError::FormatError(format!("Value at {} is not a boolean", path)))
        }
    }

    /// Formats a string template with values from the config
    ///
    /// # Arguments
    /// * `format` - Format string with `{}` placeholders
    /// * `path` - Dot-separated path with multiple attributes joined by `+` (e.g., "db/redis/server+port")
    ///
    /// # Returns
    /// Returns the formatted string, or empty string if any referenced value is not found
    /// 
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt("{}:{}", "db/redis/server+port");
    /// assert_eq!(result, "127.0.0.1:6379");
    /// ```
    pub fn fmt(&self, format: &str, path: &str) -> String {
        self.fmt_strict(format, path).unwrap_or_else(|_| String::new())
    }

    /// Parses a YAML string into a Config object
    ///
    /// # Arguments
    /// * `yaml` - YAML content as a string
    /// * `sep` - Path separator for accessing nested values (cannot be empty)
    ///
    /// # Returns
    /// Returns `Ok(Config)` on success, or `Err(ConfigError)` on failure
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if separator is empty
    /// Returns `ConfigError::YamlError` if YAML parsing fails
    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, ConfigError> {
        // Validate separator
        if sep.is_empty() {
            return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
        }

        let parsed = from_str(yaml)
            .map_err(|e| ConfigError::YamlError(e.to_string()))?;

        Ok(Config {
            content: parsed,
            filename: String::new(),
            separator: sep.to_string(),
            environment: None
        })
    }

    /// Formats a string template with values from the config, returning an error if any value is missing
    ///
    /// # Arguments
    /// * `format` - Format string with `{}` placeholders
    /// * `path` - Dot-separated path with multiple attributes joined by `+` (e.g., "db/redis/server+port")
    ///
    /// # Returns
    /// Returns `Ok(String)` with the formatted result, or `Err(ConfigError)` if any value is not found or formatting fails
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt_strict("{}:{}", "db/redis/server+port").unwrap();
    /// assert_eq!(result, "127.0.0.1:6379");
    /// ```
    pub fn fmt_strict(&self, format: &str, path: &str) -> Result<String, ConfigError> {
        let mut content = &self.content;
        let mut parts = Self::parse_path(path, &self.separator);
        let last = parts.pop();

        for item in parts.iter() {
            match content.get(item.as_str()) {
                Some(v) => { content = v; },
                None => return Err(ConfigError::PathNotFound(path.to_string()))
            }
        }

        match last {
            Some(v) => {
                let attributes = v.split('+').collect::<Vec<&str>>();
                let mut fmt = format.to_string();
                let mut vars = HashMap::new();

                for item in attributes.iter() {
                    match content.get(item) {
                        Some(v) => {
                            fmt = fmt.replacen("{}", &format!("{{{}}}", item), 1);
                            vars.insert(item.to_string(), Self::to_string(v));
                        },
                        None => return Err(ConfigError::PathNotFound(path.to_string()))
                    }
                }

                match strfmt(&fmt, &vars) {
                    Ok(r) => Ok(r),
                    Err(e) => Err(ConfigError::FormatError(e.to_string()))
                }
            },
            None => Err(ConfigError::PathNotFound(path.to_string()))
        }
    }

    fn get_leaf(mut content: &Value, path: &str, separator: &str) -> Option<Value> {
        // Validate inputs
        if path.is_empty() {
            return None;
        }
        if separator.is_empty() {
            return None;
        }

        let parts = Self::parse_path(path, separator);
    
        for item in parts.iter() {
            if item.is_empty() {
                // Skip empty parts (e.g., from leading/trailing separators)
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
    /// Allows keys containing the separator by escaping them:
    /// - `\<sep>` becomes a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
    /// - `\\` becomes a literal backslash in the key
    /// 
    /// # Example
    /// With separator `/`, path `database/host\\/port` navigates to:
    /// 1. Key "database"
    /// 2. Key "host/port" (the separator is escaped)
    ///
    /// With separator `::`, path `a::b\\::c::d` navigates to:
    /// 1. Key "a"
    /// 2. Key "b::c" (the full separator is escaped)
    /// 3. Key "d"
    fn parse_path(path: &str, separator: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = path.chars().peekable();
        let sep_first_char = separator.chars().next().unwrap_or('/');

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                // Check if the full separator follows (escaped separator)
                let remaining: String = chars.clone().collect();
                if remaining.starts_with(separator) {
                    // Escaped separator — consume it and push it literally
                    current.push_str(separator);
                    for _ in 0..separator.chars().count() {
                        chars.next();
                    }
                } else if let Some(&next) = chars.peek() {
                    if next == '\\' {
                        // Escaped backslash
                        current.push('\\');
                        chars.next();
                    } else {
                        // Lone backslash — keep as-is
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            } else if ch == sep_first_char {
                // Check if this is the actual separator (for multi-char separators)
                let remaining: String = chars.clone().collect();
                let expected_rest = &separator[1..];
                if remaining.starts_with(expected_rest) {
                    // This is the real separator
                    parts.push(current.clone());
                    current.clear();
                    // Consume the rest of the separator
                    for _ in 1..separator.len() {
                        chars.next();
                    }
                } else {
                    // Just a matching char, not the full separator
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
                let mut vars = HashMap::new();
                vars.insert(String::from("env"), v);
                let file = strfmt(filename, &vars)
                    .map_err(|e| ConfigError::FormatError(format!("Invalid filename template: {}", e)))?;
                Ok((file, Some(v.to_string())))
            },
            None => Ok((String::from(filename), None))
        }
    }

    fn load(filename: &str) -> Result<Value, ConfigError> {
        let yaml = fs::read_to_string(filename)?;
        let parsed = from_str(&yaml)
            .map_err(|e| ConfigError::YamlError(e.to_string()))?;
        
        Ok(parsed)
    }
    
    fn to_string(value: &Value) -> String {
        match value {
            Value::String(v, _) => v.to_string(),
            Value::Number(v, _) => v.to_string(),
            Value::Bool(v, _) => v.to_string(),
            _ => String::new()
        }
    }

    fn to_list(value: &Value) -> Vec<String> {
        match value {
            Value::Sequence(v) => v.iter().map(Self::to_string).collect::<Vec<String>>(),
            _ => vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{from_str, Config, Value, ConfigError};
    use serde_yaml_bw::Number;

    const YAML: &str = "
db:
    redis:
        server: 127.0.0.1
        port: 6379
        key_expiry: 3600
    sql:
        driver: SQL Server
        server: 127.0.0.1
        database: my_db
        username: user
        password: Pa$$w0rd!
sources:
    - one
    - two
    - three
app:
    debug: true
    max_retries: 5
    timeout: 2.5
";

    #[test]
    fn get_int_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let port = config.get_int("db/redis/port");
        assert_eq!(port, Some(6379));

        let max_retries = config.get_int("app/max_retries");
        assert_eq!(max_retries, Some(5));
    }

    #[test]
    fn get_int_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let value = config.get_int("db/nonexistent");
        assert_eq!(value, None);
    }

    #[test]
    fn get_int_strict_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_int_strict("db/redis/port");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 6379);
    }

    #[test]
    fn get_int_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_int_strict("db/nonexistent");
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound"),
        }
    }

    #[test]
    fn get_int_strict_wrong_type() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_int_strict("db/redis/server");
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(_)) => (),
            _ => panic!("Expected FormatError"),
        }
    }

    #[test]
    fn get_float_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let timeout = config.get_float("app/timeout");
        assert!(timeout.is_some());
        assert!((timeout.unwrap() - 2.5).abs() < 0.001);
    }

    #[test]
    fn get_float_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let value = config.get_float("app/missing_timeout");
        assert_eq!(value, None);
    }

    #[test]
    fn get_float_strict_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_float_strict("app/timeout");
        assert!(result.is_ok());
        assert!((result.unwrap() - 2.5).abs() < 0.001);
    }

    #[test]
    fn get_float_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_float_strict("app/missing");
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound"),
        }
    }

    #[test]
    fn get_float_strict_wrong_type() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_float_strict("app/debug");
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(_)) => (),
            _ => panic!("Expected FormatError"),
        }
    }

    #[test]
    fn get_bool_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let debug = config.get_bool("app/debug");
        assert_eq!(debug, Some(true));
    }

    #[test]
    fn get_bool_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let value = config.get_bool("app/missing_bool");
        assert_eq!(value, None);
    }

    #[test]
    fn get_bool_strict_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_bool_strict("app/debug");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn get_bool_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_bool_strict("app/missing");
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound"),
        }
    }

    #[test]
    fn get_bool_strict_wrong_type() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get_bool_strict("app/max_retries");
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(_)) => (),
            _ => panic!("Expected FormatError"),
        }
    }

    #[test]
    fn fmt_test()  {
        let parsed: Config = Config::load_yaml(YAML, "/").unwrap();
        let formatted = parsed.fmt("{}:{}", "db/sql/database+username");

        assert_eq!(formatted, String::from("my_db:user"));
    }

    #[test]
    fn get_leaf_test()  {
        let parsed: Value = from_str(YAML).unwrap();
        let value1 = Config::get_leaf(&parsed, "db/redis/port", "/");
        let value2 = Config::get_leaf(&parsed, "db/redis/username", "/");
        
        assert_eq!(value1, Some(Value::Number(Number::from(6379), None)));
        assert_eq!(value2, None);
    }

    #[test]
    fn get_file_test() {
        let result = Config::get_file("config_{env}.yaml", Some("dev"));

        assert!(result.is_ok());
        let (file, env) = result.unwrap();
        assert_eq!(env, Some(String::from("dev")));
        assert_eq!(file, "config_dev.yaml");
    }

    #[test]
    fn get_file_invalid_template() {
        let result = Config::get_file("config_{invalid.yaml", Some("dev"));

        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(_)) => (),
            _ => panic!("Expected FormatError for invalid template"),
        }
    }

    #[test]
    fn to_string_test() {
        let parsed: Value = from_str(YAML).unwrap();
        let value = Config::get_leaf(&parsed, "db/redis/port", "/").unwrap();
        let str_value = Config::to_string(&value);

        assert_eq!(str_value, "6379");
    }

    #[test]
    fn to_list_test()  {
        let parsed: Value = from_str(YAML).unwrap();
        let value = Config::get_leaf(&parsed, "sources", "/").unwrap();
        let list = Config::to_list(&value);

        let mut vec = Vec::new();        
        vec.push(String::from("one"));
        vec.push(String::from("two"));
        vec.push(String::from("three"));

        assert_eq!(list, vec);
    }

    #[test]
    fn contains_test() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        assert!(config.contains("db/redis/port"));
        assert!(config.contains("db/redis/server"));
        assert!(!config.contains("db/redis/nonexistent"));
        assert!(!config.contains("nonexistent/path"));
    }

    #[test]
    fn yaml_parse_error() {
        let invalid_yaml = "invalid: [unclosed";
        let result = Config::load_yaml(invalid_yaml, "/");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::YamlError(_)) => (),
            _ => panic!("Expected YamlError"),
        }
    }

    #[test]
    fn load_optional_missing_file_returns_empty_config() {
        // Missing file is not an error for load_optional
        let result = Config::load_optional("nonexistent_file_12345.yaml", "/", None);

        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(!config.contains("any/path"));
        assert_eq!(config.str("any/path"), "");
    }

    #[test]
    fn load_optional_invalid_yaml_returns_error() {
        use std::fs::{self, File};
        use std::io::Write;

        // A present-but-broken file should still surface an error
        let test_file = "test_optional_invalid.yaml";
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "invalid: [unclosed").unwrap();
        drop(file);

        let result = Config::load_optional(test_file, "/", None);
        assert!(result.is_err());
        match result {
            Err(ConfigError::YamlError(_)) => (),
            _ => panic!("Expected YamlError for malformed file"),
        }

        fs::remove_file(test_file).ok();
    }

    #[test]
    fn invalid_yaml_formats() {
        let test_cases = vec![
            "invalid: {unclosed",      // unclosed mapping
            "- item1\n - item2\n- item3\n : invalid",  // invalid key colon
            ": invalid_key",           // invalid key starting with colon
        ];

        for invalid_yaml in test_cases {
            let result = Config::load_yaml(invalid_yaml, "/");
            assert!(result.is_err(), "Expected error for: {}", invalid_yaml);
            
            match result {
                Err(ConfigError::YamlError(_)) => (),
                _ => panic!("Expected YamlError for: {}", invalid_yaml),
            }
        }
    }

    #[test]
    fn error_display_messages() {
        // Test IoError display
        let io_err = ConfigError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "test file not found",
        ));
        assert!(io_err.to_string().contains("IO error"));

        // Test YamlError display
        let yaml_err = ConfigError::YamlError("invalid syntax".to_string());
        assert!(yaml_err.to_string().contains("YAML parse error"));

        // Test PathNotFound display
        let path_err = ConfigError::PathNotFound("db/missing/key".to_string());
        assert!(path_err.to_string().contains("Path not found"));

        // Test FormatError display
        let fmt_err = ConfigError::FormatError("invalid format".to_string());
        assert!(fmt_err.to_string().contains("Format error"));
    }

    #[test]
    fn empty_yaml() {
        let empty_yaml = "";
        let result = Config::load_yaml(empty_yaml, "/");
        
        // Empty YAML should parse but result in empty config
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(!config.contains("any/path"));
    }

    #[test]
    fn get_strict_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.get_strict("db/redis/port");
        
        assert!(result.is_ok());
    }

    #[test]
    fn get_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.get_strict("db/redis/nonexistent");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(path)) => assert_eq!(path, "db/redis/nonexistent"),
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn str_strict_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.str_strict("db/redis/port");
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "6379");
    }

    #[test]
    fn str_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.str_strict("app/nonexistent");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn list_strict_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.list_strict("sources");
        
        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "one");
    }

    #[test]
    fn list_strict_not_found() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.list_strict("nonexistent/list");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn fmt_strict_success() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.fmt_strict("{}:{}", "db/redis/server+port");
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "127.0.0.1:6379");
    }

    #[test]
    fn fmt_strict_with_escaped_separator_in_path() {
        let yaml = r#"
sections:
  "db/redis":
    server: 127.0.0.1
    port: 6379
"#;
        let config = Config::load_yaml(yaml, "/").unwrap();
        // "db/redis" is a key containing a literal slash — escape it in the path
        let result = config.fmt_strict("{}:{}", r"sections/db\/redis/server+port");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "127.0.0.1:6379");
    }

    #[test]
    fn fmt_strict_missing_path() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.fmt_strict("{}:{}", "db/redis/nonexistent+port");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn fmt_strict_missing_attribute() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        let result = config.fmt_strict("{}:{}", "db/redis/server+nonexistent");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::PathNotFound(_)) => (),
            _ => panic!("Expected PathNotFound error"),
        }
    }

    #[test]
    fn empty_separator_in_load_optional() {
        let result = Config::load_optional("config.yaml", "", None);
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
            _ => panic!("Expected FormatError for empty separator"),
        }
    }

    #[test]
    fn empty_separator_in_load_yaml() {
        let result = Config::load_yaml(YAML, "");
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
            _ => panic!("Expected FormatError for empty separator"),
        }
    }

    #[test]
    fn empty_path() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        let result = config.get("");
        assert!(result.is_none());

        let result = config.str("");
        assert_eq!(result, "");

        let result = config.list("");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn path_with_only_separator() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        // "/" is the root path - should return the root content
        let result = config.get("/");
        assert!(result.is_some());

        // "//" results in empty strings which are skipped, also returns root
        let result = config.get("//");
        assert!(result.is_some());
    }

    #[test]
    fn path_with_leading_trailing_separator() {
        let config = Config::load_yaml(YAML, "/").unwrap();
        
        // "/db/redis/port/" should skip empty parts and find "port"
        let result = config.get("/db/redis/port/");
        assert!(result.is_some());
    }

    #[test]
    fn load_required_file_not_found() {
        let result = Config::load_required("nonexistent_file_xyz.yaml", "/", None);
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::IoError(_)) => (),
            _ => panic!("Expected IoError for missing file"),
        }
    }

    #[test]
    fn load_required_with_env() {
        // This will fail because config_dev.yaml doesn't exist, 
        // but it tests that the method attempts to load with env substitution
        let result = Config::load_required("config_{env}.yaml", "/", Some("dev"));
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::IoError(_)) => (),
            _ => panic!("Expected IoError for missing file"),
        }
    }

    #[test]
    fn load_required_rejects_empty_filename() {
        // load_required enforces a non-empty filename, unlike Config::load_optional
        let config = Config::load_required("", "/", None);
        
        // Empty filename is rejected with IoError
        assert!(config.is_err());
        match config {
            Err(ConfigError::IoError(_)) => (),
            _ => panic!("Expected IoError for empty filename"),
        }
    }

    #[test]
    fn escaped_separator_in_key() {
        let yaml = "
database:
  host/port: localhost:5432
  server: db.example.com
";
        let config = Config::load_yaml(yaml, "/").unwrap();
        
        // Access key with escaped separator
        let value = config.get("database/host\\/port");
        assert!(value.is_some());
        assert_eq!(config.str("database/host\\/port"), "localhost:5432");
    }

    #[test]
    fn escaped_backslash_in_key() {
        let yaml = "
paths:
  'file\\path': C:\\Users\\data
  'normal': value
";
        let config = Config::load_yaml(yaml, "/").unwrap();
        
        // Access key with escaped backslash (literal backslash in key)
        let value = config.get("paths/file\\\\path");
        assert!(value.is_some());
    }

    #[test]
    fn mixed_escaped_and_normal_separators() {
        let yaml = "
config:
  app/version: 1.0
  'db/host:port': localhost:5432
";
        let config = Config::load_yaml(yaml, "/").unwrap();
        
        // Key with slash requires escaping
        let value1 = config.get("config/app\\/version");
        assert!(value1.is_some());
        assert_eq!(config.str("config/app\\/version"), "1.0");
        
        // Escaped separator in second key
        let value2 = config.get("config/db\\/host:port");
        assert!(value2.is_some());
    }

    #[test]
    fn escape_sequences_in_strict_methods() {
        let yaml = "
database:
  'user/pass': myuser/mypass
";
        let config = Config::load_yaml(yaml, "/").unwrap();
        
        // Test that escape sequences work with strict methods too
        let result = config.get_strict("database/user\\/pass");
        assert!(result.is_ok());
        
        let result = config.str_strict("database/user\\/pass");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "myuser/mypass");
    }

    #[test]
    fn parse_path_basic() {
        let parts = Config::parse_path("a/b/c", "/");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_path_with_escaped_separator() {
        let parts = Config::parse_path("a/b\\/c/d", "/");
        assert_eq!(parts, vec!["a", "b/c", "d"]);
    }

    #[test]
    fn parse_path_with_escaped_backslash() {
        let parts = Config::parse_path("a/b\\\\c/d", "/");
        assert_eq!(parts, vec!["a", "b\\c", "d"]);
    }

    #[test]
    fn parse_path_multiple_escapes() {
        let parts = Config::parse_path("a\\/b\\/c/d", "/");
        assert_eq!(parts, vec!["a/b/c", "d"]);
    }

    #[test]
    fn parse_path_with_custom_separator() {
        let parts = Config::parse_path("a::b\\::c::d", "::");
        assert_eq!(parts, vec!["a", "b::c", "d"]);
    }

    #[test]
    fn parse_path_escape_requires_full_separator() {
        // With separator "::", a lone "\:" should NOT be treated as an escaped separator —
        // only the full "\::" should be. Previously this was a known bug.
        let parts = Config::parse_path(r"a::b\:c::d", "::");
        // "\:" is not a valid escape, backslash kept as-is, ":" is just a literal char
        assert_eq!(parts, vec!["a", r"b\:c", "d"]);
    }

    #[test]
    fn reload_from_same_file() {
        use std::fs::{self, File};
        use std::io::Write;
        
        // Create a temporary test file
        let test_file = "test_reload_config.yaml";
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "app:\n  port: 8080\n  debug: false").unwrap();
        drop(file);
        
        // Load initial config
        let mut config = Config::load_optional(test_file, "/", None).unwrap();
        assert_eq!(config.str("app/port"), "8080");
        assert_eq!(config.str("app/debug"), "false");
        
        // Modify the file
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "app:\n  port: 9090\n  debug: true").unwrap();
        drop(file);
        
        // Reload the config
        config.reload().unwrap();
        assert_eq!(config.str("app/port"), "9090");
        assert_eq!(config.str("app/debug"), "true");
        
        // Cleanup
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn reload_from_different_file() {
        use std::fs::{self, File};
        use std::io::Write;
        
        let file1 = "test_reload_file1.yaml";
        let file2 = "test_reload_file2.yaml";
        
        // Create first file
        let mut file = File::create(file1).unwrap();
        writeln!(file, "config:\n  name: first\n  value: 100").unwrap();
        drop(file);
        
        // Create second file
        let mut file = File::create(file2).unwrap();
        writeln!(file, "config:\n  name: second\n  value: 200").unwrap();
        drop(file);
        
        // Load from first file
        let mut config = Config::load_optional(file1, "/", None).unwrap();
        assert_eq!(config.str("config/name"), "first");
        assert_eq!(config.str("config/value"), "100");
        assert_eq!(config.get_filename(), file1);
        
        // Reload from second file
        config.reload_from(file2).unwrap();
        assert_eq!(config.str("config/name"), "second");
        assert_eq!(config.str("config/value"), "200");
        assert_eq!(config.get_filename(), file2);
        
        // Cleanup
        fs::remove_file(file1).ok();
        fs::remove_file(file2).ok();
    }

    #[test]
    fn reload_preserves_separator() {
        use std::fs::{self, File};
        use std::io::Write;
        
        let test_file = "test_reload_sep.yaml";
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "db:\n  host: localhost\n  port: 5432").unwrap();
        drop(file);
        
        let mut config = Config::load_optional(test_file, "::", None).unwrap();
        assert_eq!(config.str("db::host"), "localhost");
        
        // Modify file
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "db:\n  host: remote\n  port: 3306").unwrap();
        drop(file);
        
        config.reload().unwrap();
        
        // Separator should still be "::"
        assert_eq!(config.str("db::host"), "remote");
        
        // Cleanup
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn reload_from_string_config_fails() {
        let yaml = "test: value";
        let mut config = Config::load_yaml(yaml, "/").unwrap();
        
        let result = config.reload();
        assert!(result.is_err());
        match result {
            Err(ConfigError::FormatError(msg)) => {
                assert!(msg.contains("YAML string"));
            },
            _ => panic!("Expected FormatError"),
        }
    }

    #[test]
    fn reload_from_invalid_yaml_fails() {
        use std::fs::{self, File};
        use std::io::Write;
        
        let test_file = "test_reload_invalid.yaml";
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "valid:\n  yaml: content").unwrap();
        drop(file);
        
        let mut config = Config::load_optional(test_file, "/", None).unwrap();
        
        // Overwrite with invalid YAML
        let mut file = File::create(test_file).unwrap();
        writeln!(file, "invalid: [unclosed").unwrap();
        drop(file);
        
        let result = config.reload();
        assert!(result.is_err());
        
        // Original config still intact
        assert_eq!(config.str("valid/yaml"), "content");
        
        // Cleanup
        fs::remove_file(test_file).ok();
    }

    #[test]
    fn merge_overlay_overrides_base() {
        let base = Config::load_yaml("app:
  port: 8080
  debug: false", "/").unwrap();
        let overlay = Config::load_yaml("app:
  port: 9090", "/").unwrap();
        let config = base.merge(overlay);

        assert_eq!(config.str("app/port"), "9090");      // overridden
        assert_eq!(config.str("app/debug"), "false");    // preserved
    }

    #[test]
    fn merge_deep_preserves_siblings() {
        let base = Config::load_yaml(
            "db:
  host: localhost
  port: 5432
  name: mydb", "/").unwrap();
        let overlay = Config::load_yaml(
            "db:
  host: prodserver", "/").unwrap();
        let config = base.merge(overlay);

        assert_eq!(config.str("db/host"), "prodserver");  // overridden
        assert_eq!(config.str("db/port"), "5432");        // preserved
        assert_eq!(config.str("db/name"), "mydb");        // preserved
    }

    #[test]
    fn merge_adds_new_keys_from_overlay() {
        let base = Config::load_yaml("app:
  port: 8080", "/").unwrap();
        let overlay = Config::load_yaml("app:
  debug: true", "/").unwrap();
        let config = base.merge(overlay);

        assert_eq!(config.str("app/port"), "8080");
        assert_eq!(config.get_bool("app/debug"), Some(true));
    }

    #[test]
    fn merge_replaces_sequences_wholesale() {
        let base = Config::load_yaml("features:
  - a
  - b
  - c", "/").unwrap();
        let overlay = Config::load_yaml("features:
  - x
  - y", "/").unwrap();
        let config = base.merge(overlay);

        let list = config.list("features");
        assert_eq!(list, vec!["x", "y"]);  // replaced, not appended
    }

    #[test]
    fn merge_with_empty_overlay_is_identity() {
        let base = Config::load_yaml("app:
  port: 8080", "/").unwrap();
        let empty = Config::default();
        let config = base.merge(empty);

        assert_eq!(config.str("app/port"), "8080");
    }

    #[test]
    fn merge_empty_base_with_overlay() {
        let base = Config::default();
        let overlay = Config::load_yaml("app:
  port: 8080", "/").unwrap();
        let config = base.merge(overlay);

        assert_eq!(config.str("app/port"), "8080");
    }

    #[test]
    fn merge_chaining() {
        let base    = Config::load_yaml("app:
  port: 8080
  debug: false
  name: base", "/").unwrap();
        let first   = Config::load_yaml("app:
  port: 9090", "/").unwrap();
        let second  = Config::load_yaml("app:
  debug: true", "/").unwrap();
        let config = base.merge(first).merge(second);

        assert_eq!(config.str("app/port"), "9090");          // from first overlay
        assert_eq!(config.get_bool("app/debug"), Some(true)); // from second overlay
        assert_eq!(config.str("app/name"), "base");           // preserved from base
    }

    #[test]
    fn merge_preserves_base_separator() {
        let base    = Config::load_yaml("app:
  port: 8080", "::").unwrap();
        let overlay = Config::load_yaml("app:
  port: 9090", "/").unwrap();
        let config = base.merge(overlay);

        // Base separator "::" should be preserved
        assert_eq!(config.str("app::port"), "9090");
    }
}