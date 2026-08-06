use std::fs;
use yaml_serde::{Value, from_str};
use crate::error::ConfigError;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let yaml = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&yaml, filename)
}

pub(crate) fn parse(yaml: &str) -> Result<Value, ConfigError> {
    parse_internal(yaml, None)
}

/// Parses YAML that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(yaml: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(yaml, Some(filename))
}

fn parse_internal(yaml: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // Every YAML path — file or string — passes through here, so the BOM rule is
    // applied exactly once per format. See `super::strip_bom`.
    let yaml = super::strip_bom(yaml);
    from_str(yaml).map_err(|e| match file {
        Some(filename) => ConfigError::yaml_in(filename, e),
        None => ConfigError::from(e),
    })
}
