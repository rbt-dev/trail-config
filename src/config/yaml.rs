use std::fs;
use yaml_serde::{Value, from_str};
use crate::error::ConfigError;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let yaml = fs::read_to_string(filename)?;
    parse(&yaml)
}

pub(crate) fn parse(yaml: &str) -> Result<Value, ConfigError> {
    from_str(yaml).map_err(|e| ConfigError::YamlError(e.to_string()))
}