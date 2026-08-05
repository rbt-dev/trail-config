use yaml_serde::Value;

mod accessor;
mod env;
mod fmt;
mod loader;
mod merge;
mod parser;
mod path;
mod reload;

#[derive(Debug, Clone)]
enum OverlaySource {
    Required(String),
    Optional(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    content: Value,
    filename: String,
    separator: String,
    environment: Option<String>,
    overlays: Vec<OverlaySource>,
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

    /// Returns the filename of the loaded config file
    pub fn get_filename(&self) -> &str {
        &self.filename
    }
}

#[cfg(test)]
mod tests;
