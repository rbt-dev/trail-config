use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    YamlError(String),
    JsonError(String),
    PathNotFound(String),
    FormatError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::YamlError(msg) => write!(f, "YAML parse error: {}", msg),
            ConfigError::JsonError(msg) => write!(f, "JSON parse error: {}", msg),
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

impl From<yaml_serde::Error> for ConfigError {
    fn from(err: yaml_serde::Error) -> Self {
        ConfigError::YamlError(err.to_string())
    }
}
