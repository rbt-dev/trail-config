//! Formatting sibling config values into a string template.

use crate::error::ConfigError;
use super::Config;
use super::accessor::to_string;
use super::path::parse_path;

impl Config {
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
        let parts = parse_path(base, &self.separator);

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
                    result = result.replacen("{}", &to_string(v), 1);
                },
                None => return Err(ConfigError::PathNotFound(format!("{}/{}", base, key)))
            }
        }

        Ok(result)
    }
}
