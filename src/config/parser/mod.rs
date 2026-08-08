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
/// caller's doing rather than an editor's, but having `load_json(s)` and a file read as
/// [`Format::Json`] disagree about the same bytes would be its own surprise, and
/// consistency costs nothing.
///
/// Only a *leading* BOM, and only one: U+FEFF anywhere else is a legitimate (if
/// deprecated) zero-width no-break space and belongs to the document.
fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{feff}').unwrap_or(content)
}

/// Which parser reads a config file.
///
/// Normally there is no reason to name one: the format is derived from the file's
/// extension, case-insensitively, and `.json` / `.toml` reach their parsers on their own
/// once the matching feature is enabled. This exists for the file whose **extension does
/// not name its format** — `settings.conf`, `app.cfg`, a file with no extension at all —
/// where the `_as` constructors take it explicitly:
/// [`load_required_as`](crate::Config::load_required_as),
/// [`load_optional_as`](crate::Config::load_optional_as) and
/// [`load_or_create_as`](crate::Config::load_or_create_as).
///
/// The choice is recorded on the [`Config`](crate::Config), so
/// [`reload`](crate::Config::reload) and [`reload_from`](crate::Config::reload_from) read
/// the file the same way rather than falling back to the extension. Overlays are
/// unaffected and still pick their own parser by their own extension, which is what lets a
/// JSON base take a YAML overlay.
///
/// `#[non_exhaustive]`, so a format added later is not a breaking change: match with a
/// `_ => ...` arm. Constructing the variants is unaffected.
///
/// # Example
///
/// `Format::Yaml` is used here because it is the one variant present in every feature
/// combination, which a doctest has to be; `Format::Json` and `Format::Toml` are spelled
/// the same way once their features are on.
///
/// ```no_run
/// # use trail_config::{Config, Format};
/// // A YAML document in a file named as though it were something else
/// let config = Config::load_required_as("settings.json", "/", None, Format::Yaml)?;
/// # Ok::<(), trail_config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// YAML, and the parser everything unrecognised falls back to.
    Yaml,
    /// JSON.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    Json,
    /// TOML. Datetimes are read as the text the file contained — see
    /// [`load_toml`](crate::Config::load_toml).
    #[cfg(feature = "toml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
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
/// construction is re-read the same way. Without it, a `settings.conf` loaded as
/// [`Format::Json`] parsed as JSON and the `reload()` that followed parsed the same bytes as
/// YAML — silently, since YAML is a superset of JSON, until a document exercised a
/// difference.
pub(super) fn load_in(format: Option<Format>, filename: &str) -> Result<Value, ConfigError> {
    match format.unwrap_or_else(|| format_of(filename)) {
        Format::Yaml => yaml::load_file(filename),
        #[cfg(feature = "json")]
        Format::Json => json::load_file(filename),
        #[cfg(feature = "toml")]
        Format::Toml => toml::load_file(filename),
    }
}

/// Parses a config string as if it had been read from `filename`, with `format` or — when
/// it is `None` — the parser that filename's extension names, attributing any parse error
/// to it.
///
/// The counterpart to [`load_in`] for content that is not on disk — or not on disk *yet*,
/// which is the case that motivates it: `load_or_create` validates its defaults through
/// this before writing them, so a defaults string that does not parse in the file's format
/// never reaches the filesystem.
///
/// Taking the format rather than always deriving it is what keeps that validation honest
/// for `load_or_create_as`, whose whole premise is a file whose extension does not name its
/// format. Deriving here would have checked JSON defaults against the YAML parser and then
/// written them to a file the reader parses as JSON — the one place in this design where
/// the answer would be outright wrong rather than merely re-derived.
pub(super) fn parse_in(format: Option<Format>, content: &str, filename: &str) -> Result<Value, ConfigError> {
    match format.unwrap_or_else(|| format_of(filename)) {
        Format::Yaml => yaml::parse_in(content, filename),
        #[cfg(feature = "json")]
        Format::Json => json::parse_in(content, filename),
        #[cfg(feature = "toml")]
        Format::Toml => toml::parse_in(content, filename),
    }
}
