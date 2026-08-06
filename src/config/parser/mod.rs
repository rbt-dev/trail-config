//! Format adapters: reading and parsing config files into `yaml_serde::Value`.

use yaml_serde::Value;
use crate::error::ConfigError;

pub(super) mod yaml;

#[cfg(feature = "json")]
pub(super) mod json;

#[cfg(feature = "toml")]
pub(super) mod toml;

/// Removes a leading UTF-8 byte-order mark, if there is one.
///
/// Written once here for the same reason as [`format_of`]: the three formats must not
/// disagree about a file that every editor displays identically. `yaml_serde` and `toml`
/// skip a BOM of their own accord, `serde_json` rejects it — so without this, the same
/// bytes loaded as `.yaml` and `.toml` and failed as `.json` with "expected value at
/// line 1 column 1", pointing at a file that looks perfectly correct.
///
/// That is a Windows-shaped trap in particular: PowerShell's `>`, `>>` and `Out-File`
/// write UTF-8 **with** BOM by default, as do older versions of Notepad, so a config
/// produced by a setup script could be unreadable by the crate the script installed.
///
/// Applied to strings as well as to file content. A BOM in a string literal is the
/// caller's doing rather than an editor's, but having `load_json(s)` and
/// `load_json_file(f)` disagree about the same bytes would be its own surprise, and
/// consistency costs nothing.
///
/// Only a *leading* BOM, and only one: U+FEFF anywhere else is a legitimate (if
/// deprecated) zero-width no-break space and belongs to the document.
fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

/// Which parser reads a file.
///
/// Normally derived from the filename by [`format_of`], but [`Config`](crate::Config) can
/// carry one chosen explicitly — see `Config::format` — so that a file loaded by
/// [`load_json_file`](crate::Config::load_json_file) or
/// [`load_toml_file`](crate::Config::load_toml_file) is re-read by the same parser on a
/// [`reload`](crate::Config::reload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::config) enum Format {
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
/// silently applying YAML's number and quoting rules instead of `serde_json`'s, until a
/// config exercised one of the differences. (Duplicate keys were on that list too, until
/// the JSON path started deserializing into this crate's value model and inherited its
/// rejection of them; the remaining differences are enough on their own.)
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
    load_in(None, filename)
}

/// Loads a config file with `format`, falling back to the extension when it is `None`.
///
/// The single entry point for reading a file, so that a config which pinned its format at
/// construction is re-read the same way. Without it, `load_json_file("settings.conf")`
/// parsed as JSON and the `reload()` that followed parsed the same bytes as YAML —
/// silently, since YAML is a superset of JSON, until a document exercised a difference.
pub(super) fn load_in(format: Option<Format>, filename: &str) -> Result<Value, ConfigError> {
    match format.unwrap_or_else(|| format_of(filename)) {
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
