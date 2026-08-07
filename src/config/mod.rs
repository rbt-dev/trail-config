use yaml_serde::Value;

mod accessor;
mod env;
mod fmt;
mod loader;
mod merge;
mod outline;
mod parser;
mod path;
mod reload;

// `Format` is part of the public API — the `_as` constructors take one — even though the
// module holding it is private. Re-exported here and again from the crate root, so the
// parser modules stay an implementation detail while the type callers must name does not.
pub use parser::Format;

#[derive(Debug, Clone)]
enum OverlaySource {
    Required(String),
    Optional(String),
}

/// A loaded configuration document, with the settings used to read it.
///
/// A `Config` owns the parsed document, the separator its paths are written with, the
/// environment it was loaded for, and the chain of overlays merged onto it — everything
/// [`reload`](Config::reload) needs to rebuild itself from disk.
///
/// Construct one with [`load_required`](Config::load_required),
/// [`load_optional`](Config::load_optional), [`load_or_create`](Config::load_or_create)
/// or [`Config::default`], read values with the lenient
/// (`get`, `str`, `list`, `get_int`, ...) or strict (`*_strict`) accessors, and layer
/// files with [`merge_required`](Config::merge_required) /
/// [`merge_optional`](Config::merge_optional).
///
/// `Config` is `Send + Sync`, so sharing one across threads needs nothing more than an
/// `Arc<Config>` — no lock and no second indirection. What that cannot do is *replace*
/// the document behind those shared references, since `reload` takes `&mut self`. That
/// is what [`ConfigHandle`](crate::ConfigHandle) adds: interior mutability, so every
/// holder sees a new document after a reload. Reach for it when the config changes at
/// runtime, and for an `Arc<Config>` when it does not.
#[derive(Clone)]
pub struct Config {
    content: Value,
    filename: String,
    separator: String,
    environment: Option<String>,
    overlays: Vec<OverlaySource>,
    /// The parser to re-read [`filename`](Self::filename) with, when it was chosen
    /// explicitly rather than derived from the extension.
    ///
    /// `None` — the usual case — means "decide from the extension, every time". `Some` is
    /// set only by the `_as` constructors — [`load_required_as`](Config::load_required_as),
    /// [`load_optional_as`](Config::load_optional_as) and
    /// [`load_or_create_as`](Config::load_or_create_as) — whose whole reason to exist is a
    /// file whose extension does not name its format. Without recording it, those parsed the
    /// file one way and every later `reload` parsed it another.
    ///
    /// Applies to the base file only. Each overlay decides for itself by its own
    /// extension, which is what lets a JSON base take a YAML overlay.
    format: Option<parser::Format>,
}

// Note: `std::fmt` is spelled out rather than imported, because this module already
// has a `fmt` submodule (`Config::fmt`, the string formatter).
impl std::fmt::Debug for Config {
    /// Prints the config's *shape*, never its values.
    ///
    /// A config crate is precisely where `${DB_PASSWORD}` and `${API_TOKEN}` get
    /// interpolated, and by the time `Debug` runs the real values have already been
    /// substituted in. A derived impl put them in cleartext into every `{:?}` — a panic
    /// message, a `tracing` span, an `anyhow` context chain, or a `#[derive(Debug)]` on
    /// any struct that happens to hold a `Config`. Nobody debugs by reading a whole
    /// config tree out of a log line, so nothing of value is lost by eliding it.
    ///
    /// Filenames are printed: they are not secrets, and the overlay chain is exactly
    /// what you want when a [`reload`](Config::reload) does not do what you expected.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shape = describe(&self.content);
        let mut out = f.debug_struct("Config");
        out.field("filename", &self.filename)
            .field("separator", &self.separator)
            .field("environment", &self.environment)
            .field("overlays", &self.overlays);
        // Only when the format was pinned, which is rare. It belongs here for the same
        // reason the overlay chain does — it is one of the things that decides what a
        // `reload` reads — but printing `format: None` on every other config would be
        // noise standing for "nothing unusual".
        if let Some(format) = self.format {
            out.field("format", &format);
        }
        out.field("content", &std::format_args!("{shape}")).finish()
    }
}

/// Describes a document's shape for [`Debug`] — its size, never its contents.
///
/// A `!Tag` is looked through, as everywhere else that reads rather than deserializes:
/// a tagged mapping is described by its key count, not as a scalar.
fn describe(content: &Value) -> String {
    match accessor::untagged(content) {
        Value::Null => "<empty>".to_string(),
        Value::Mapping(map) if map.len() == 1 => "<1 key>".to_string(),
        Value::Mapping(map) => format!("<{} keys>", map.len()),
        Value::Sequence(seq) if seq.len() == 1 => "<1 item>".to_string(),
        Value::Sequence(seq) => format!("<{} items>", seq.len()),
        _ => "<scalar>".to_string(),
    }
}

impl Default for Config {
    /// Creates a `Config`, loading from `config.yaml` in the current directory.
    ///
    /// Shorthand for `Config::load_optional("config.yaml", "/", None)`:
    ///
    /// - If `config.yaml` does **not exist**, returns an empty config.
    /// - If `config.yaml` exists and is **valid**, loads it.
    /// - If `config.yaml` exists but is **broken** (invalid YAML/JSON/TOML,
    ///   permission denied, ...), this **panics** rather than silently returning
    ///   an empty config — a present-but-broken config file is almost always a
    ///   deployment mistake worth surfacing immediately.
    ///
    /// `load_optional` already treats "file not found" as an empty config, so the
    /// only errors that reach this method are genuine failures (parse errors,
    /// permission denied, ...), and those are surfaced as a panic.
    ///
    /// # Panics
    /// Panics if `config.yaml` exists but cannot be read or parsed. For
    /// non-panicking behaviour use [`Config::load_optional`] (returns the error
    /// as a `Result`) or [`Config::load_required`].
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::default(); // Loads config.yaml if present and valid
    /// ```
    fn default() -> Self {
        Self::load_optional("config.yaml", "/", None).unwrap_or_else(|e| {
            panic!(
                "Config::default() failed to load config.yaml: {e}.\n\
                 A present-but-broken config file is likely a mistake; refusing to silently \
                 fall back to an empty config. Use Config::load_optional(\"config.yaml\", \
                 \"/\", None) to obtain the error as a Result, or Config::load_required."
            )
        })
    }
}

impl Config {
    /// Returns the environment name used when loading the config file
    pub fn environment(&self) -> Option<&str> {
        self.environment.as_deref()
    }

    /// Returns the separator this config's paths are written with.
    ///
    /// The third of the settings a config carries, alongside
    /// [`filename`](Config::filename) and [`environment`](Config::environment) —
    /// and the only one *required* to address anything. Code handed a `Config` it did not
    /// construct could not build a path for it, or read an [`outline`](Config::outline),
    /// whose escaping is written in terms of this separator.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_yaml("db:\n  host: localhost\n", "::").unwrap();
    ///
    /// // Build a path without having to know how the config was constructed
    /// let path = ["db", "host"].join(config.separator());
    /// assert_eq!(config.str(&path), "localhost");
    /// ```
    pub fn separator(&self) -> &str {
        &self.separator
    }

    /// Returns the filename of the loaded config file.
    ///
    /// This is the *resolved* name, with any `{env}` placeholder substituted. A config
    /// loaded optionally from a file that does not exist still reports that filename —
    /// it is the file a later [`reload`](Config::reload) will read. Only a config parsed
    /// from a string ([`load_yaml`](Config::load_yaml), [`load_json`](Config::load_json),
    /// [`load_toml`](Config::load_toml)) has no filename, and returns `""`.
    pub fn filename(&self) -> &str {
        &self.filename
    }
}

#[cfg(test)]
mod tests;
