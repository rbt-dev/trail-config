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
///
/// The extension is compared **case-insensitively**, because Windows and macOS are
/// case-insensitive by default: there, `config.TOML` names the same file on disk as
/// `config.toml`, and a byte-exact test routed one of them to the YAML parser. The two
/// halves failed differently and the JSON one was the worse: `.TOML` produced a parse
/// error that pointed at the wrong problem entirely (`[table]` headers read as a second
/// YAML document), while `.JSON` appeared to *work*, because YAML is a superset of JSON —
/// silently applying YAML's duplicate-key and number rules instead of `serde_json`'s,
/// until a config exercised one of the differences.
///
/// Matching is on the extension rather than on the end of the string, which also settles
/// the one case where they differ: a file named literally `.json` is a dotfile with no
/// extension, like `.gitignore`, so it parses as YAML rather than as JSON.
// With neither format feature enabled there is nothing to dispatch on: both branches
// below are compiled out and the extension is never inspected.
#[cfg_attr(not(any(feature = "json", feature = "toml")), allow(unused_variables))]
fn format_of(filename: &str) -> Format {
    #[cfg(any(feature = "json", feature = "toml"))]
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    #[cfg(feature = "json")]
    if ext.eq_ignore_ascii_case("json") {
        return Format::Json;
    }

    #[cfg(feature = "toml")]
    if ext.eq_ignore_ascii_case("toml") {
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
