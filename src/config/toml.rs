use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)?;
    parse(&content)
}

pub(crate) fn parse(toml_str: &str) -> Result<Value, ConfigError> {
    let toml_value: toml::Value = toml::from_str(toml_str)
        .map_err(|e| ConfigError::TomlError(e.to_string()))?;
    toml_to_yaml(toml_value)
}

fn toml_to_yaml(toml_val: toml::Value) -> Result<Value, ConfigError> {
    yaml_serde::to_value(toml_val)
        .map_err(|e| ConfigError::TomlError(format!("TOML to YAML conversion error: {}", e)))
}