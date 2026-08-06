use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&content, filename)
}

pub(crate) fn parse(json: &str) -> Result<Value, ConfigError> {
    parse_internal(json, None)
}

/// Parses JSON that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(json: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(json, Some(filename))
}

fn parse_internal(json: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // `serde_json` is the one parser of the three that rejects a BOM rather than
    // skipping it, so without this the same bytes load as YAML and TOML and fail here.
    // See `super::strip_bom`.
    let json = super::strip_bom(json);
    let json_value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ConfigError::json_in(file, e))?;
    // Conversion into the YAML value model happens in yaml_serde, so a
    // failure here surfaces as a YamlError
    yaml_serde::to_value(json_value).map_err(ConfigError::from)
}
