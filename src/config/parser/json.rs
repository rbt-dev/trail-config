use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_internal(&content, Some(filename))
}

pub(crate) fn parse(json: &str) -> Result<Value, ConfigError> {
    parse_internal(json, None)
}

fn parse_internal(json: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    let json_value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ConfigError::json_in(file, e))?;
    // Conversion into the YAML value model happens in yaml_serde, so a
    // failure here surfaces as a YamlError
    yaml_serde::to_value(json_value).map_err(ConfigError::from)
}
