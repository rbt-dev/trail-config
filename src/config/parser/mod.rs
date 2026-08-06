//! Format adapters: reading and parsing config files into `yaml_serde::Value`.

use yaml_serde::Value;
use crate::error::ConfigError;

pub(super) mod yaml;

#[cfg(feature = "json")]
pub(super) mod json;

#[cfg(feature = "toml")]
pub(super) mod toml;

/// The parser a filename selects.
enum Format {
    Yaml,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "toml")]
    Toml,
}

/// Chooses the format for a filename by its extension.
///
/// `.json` and `.toml` select their respective parsers when the corresponding feature is
/// enabled; everything else is YAML. This is the only place the extension rule is written,
/// so reading a file and parsing a string for that same file can never disagree about the
/// format — which is what `load_or_create` relies on to validate its defaults.
// With neither format feature enabled there is nothing to dispatch on: both branches
// below are compiled out and the extension is never inspected.
#[cfg_attr(not(any(feature = "json", feature = "toml")), allow(unused_variables))]
fn format_of(filename: &str) -> Format {
    #[cfg(feature = "json")]
    if filename.ends_with(".json") {
        return Format::Json;
    }

    #[cfg(feature = "toml")]
    if filename.ends_with(".toml") {
        return Format::Toml;
    }

    Format::Yaml
}

/// Loads a config file, choosing the parser by file extension.
pub(super) fn load_auto(filename: &str) -> Result<Value, ConfigError> {
    match format_of(filename) {
        Format::Yaml => yaml::load_file(filename),
        #[cfg(feature = "json")]
        Format::Json => json::load_file(filename),
        #[cfg(feature = "toml")]
        Format::Toml => toml::load_file(filename),
    }
}

/// Parses a config string as if it had been read from `filename`, choosing the parser by
/// that filename's extension and attributing any parse error to it.
///
/// The counterpart to [`load_auto`] for content that is not on disk — or not on disk
/// *yet*, which is the case that motivates it: `load_or_create` validates its defaults
/// through this before writing them, so a defaults string that does not parse in the
/// file's format never reaches the filesystem.
pub(super) fn parse_auto(content: &str, filename: &str) -> Result<Value, ConfigError> {
    match format_of(filename) {
        Format::Yaml => yaml::parse_in(content, filename),
        #[cfg(feature = "json")]
        Format::Json => json::parse_in(content, filename),
        #[cfg(feature = "toml")]
        Format::Toml => toml::parse_in(content, filename),
    }
}
