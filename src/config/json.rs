use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)?;
    parse(&content)
}

pub(crate) fn parse(json: &str) -> Result<Value, ConfigError> {
    let json_value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ConfigError::JsonError(e.to_string()))?;
    json_to_yaml(json_value)
}

fn json_to_yaml(json: serde_json::Value) -> Result<Value, ConfigError> {
    yaml_serde::to_value(json)
        .map_err(|e| ConfigError::JsonError(format!("JSON to YAML conversion error: {}", e)))
}
