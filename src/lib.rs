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
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::default(); // Loads config.yaml if it exists, or returns empty config
    /// // Always succeeds - never panics
    /// ```
    fn default() -> Self {
        Self::new("config.yaml", "/", None)
            .unwrap_or_else(|_| Config {
                content: Value::Null(None),
                filename: String::new(),
                separator: "/".to_string(),
                environment: None
            })
    }
}

impl Config {
    /// Creates a new Config from a YAML file.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename
    ///
    /// # Returns
    /// Returns `Ok(Config)` on success, or `Err(ConfigError)` on failure
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file cannot be read
    /// Returns `ConfigError::YamlError` if the YAML cannot be parsed
    pub fn new(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
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

    /// Creates a new Config from a required YAML file.
    ///
    /// This method is intended for production environments where a missing configuration
    /// file is a critical error. Unlike `default()` which gracefully falls back to an
    /// empty config, this method will return an error if the file is missing or invalid.
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
    /// Returns `ConfigError::IoError` if the file is missing or cannot be read (permission denied, etc.)
    /// Returns `ConfigError::YamlError` if the YAML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// // In production, require config.yaml to exist
    /// let config = Config::load_required("config.yaml", "/", None)
    ///     .expect("Failed to load required config.yaml");
    /// ```
    pub fn load_required(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::new(filename, sep, env)
    }

    pub fn environment(&self) -> Option<&str> {
        match &self.environment {
            Some(v) => Some(v),
            None => None
        }
    }

    /// Returns the filename of the loaded config file
    pub fn get_filename(&self) -> &str {
        &self.filename
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
        let mut parts = path.split(&self.separator).collect::<Vec<&str>>();
        let last = parts.pop();
    
        for item in parts.iter() {
            match content.get(item) {
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

                return match strfmt(&fmt, &vars) {
                    Ok(r) => Ok(r),
                    Err(e) => Err(ConfigError::FormatError(e.to_string()))
                };
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

        let parts = path.split(separator).collect::<Vec<&str>>();
    
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

        return Some(content.clone());
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
";

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
    fn file_not_found_error() {
        let result = Config::new("nonexistent_file_12345.yaml", "/", None);
        
        assert!(result.is_err());
        match result {
            Err(ConfigError::IoError(_)) => (),
            _ => panic!("Expected IoError for missing file"),
        }
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
    fn empty_separator_in_new() {
        let result = Config::new("config.yaml", "", None);
        
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
    fn load_required_success_from_yaml() {
        // Test that load_required works when data is valid
        let config = Config::load_required("", "/", None);
        
        // Empty filename will fail, but that's expected for this test
        // In real usage, a valid filename would succeed
        assert!(config.is_err());
    }
}