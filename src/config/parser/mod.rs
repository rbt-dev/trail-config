//! Format adapters: reading and parsing config files into `yaml_serde::Value`.

use yaml_serde::Value;
use crate::error::ConfigError;

pub(super) mod yaml;

#[cfg(feature = "json")]
pub(super) mod json;

#[cfg(feature = "toml")]
pub(super) mod toml;

/// Loads a config file, choosing the parser by file extension.
///
/// `.json` and `.toml` are dispatched to their respective parsers when the
/// corresponding feature is enabled; everything else is parsed as YAML.
pub(super) fn load_auto(filename: &str) -> Result<Value, ConfigError> {
    #[cfg(feature = "json")]
    if filename.ends_with(".json") {
        return json::load_file(filename);
    }

    #[cfg(feature = "toml")]
    if filename.ends_with(".toml") {
        return toml::load_file(filename);
    }

    yaml::load_file(filename)
}
