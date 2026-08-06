use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&content, filename)
}

pub(crate) fn parse(toml_str: &str) -> Result<Value, ConfigError> {
    parse_internal(toml_str, None)
}

/// Parses TOML that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(toml_str: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(toml_str, Some(filename))
}

fn parse_internal(toml_str: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // `toml` skips a BOM already; stripping here anyway keeps the rule uniform across
    // the three formats rather than dependent on each parser's leniency. See
    // `super::strip_bom`.
    let toml_str = super::strip_bom(toml_str);
    let toml_value: toml::Value = toml::from_str(toml_str)
        .map_err(|e| ConfigError::toml_in(file, e))?;
    // Conversion into the YAML value model happens in yaml_serde, so a
    // failure here surfaces as a YamlError
    yaml_serde::to_value(toml_value).map_err(ConfigError::from)
}
