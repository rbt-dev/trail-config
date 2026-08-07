//! Constructors: loading a `Config` from files or strings.

use std::{fs, io, io::Write, thread, time::Duration};
use yaml_serde::Value;
use crate::error::ConfigError;
use super::Config;
use super::env::resolve_env_vars;
use super::parser;

/// How many times [`Config::load_or_create`] re-reads a zero-length config file before
/// accepting it as genuinely empty, and how long it waits between attempts.
///
/// Together these bound the wait at 200 ms. The window exists because `create_new` makes
/// *creating* the file atomic but not *filling* it — see [`create_new_file`] — so the
/// loser of a first-run race can read the winner's file between the two syscalls and get
/// nothing. Waiting turns that from a silently empty config into a brief pause.
///
/// Only a **zero-length** file waits, and only when `defaults` is not itself empty, so
/// nothing on the ordinary paths pays for this: a file with content settles on the first
/// read, and a file holding only comments is not zero-length and is accepted immediately.
/// The one case that pays the full 200 ms is a deliberately empty file loaded with
/// non-empty defaults, which is a contradiction in the call itself.
const EMPTY_FILE_RETRIES: usize = 10;
const EMPTY_FILE_BACKOFF: Duration = Duration::from_millis(20);

impl Config {
    /// Loads a Config from a file, returning an error if the file is missing or invalid.
    ///
    /// Use this in production code where a missing config file is a critical error.
    /// For optional config files, use [`load_optional`](Config::load_optional) or [`default`](Config::default).
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Returns
    /// Returns `Ok(Config)` if the file is found and valid, or `Err(ConfigError)` otherwise
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the file is missing, empty filename, or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::Config;
    /// let config = Config::load_required("config.yaml", "/", None)
    ///     .expect("Failed to load required config.yaml");
    /// ```
    pub fn load_required(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_internal_as(filename, sep, env, None)
    }

    /// Loads a Config from a file read as `format`, whatever its extension.
    ///
    /// [`load_required`](Config::load_required) already routes a `.json` or `.toml` file to
    /// the matching parser, so the reason to reach for this is a file whose *extension does
    /// not name its format* — `settings.conf`, `app.cfg`, a file with no extension at all.
    ///
    /// The choice is recorded on the config, so [`reload`](Config::reload) and
    /// [`reload_from`](Config::reload_from) read it the same way. Without that the file was
    /// parsed one way once and another way ever after, which failed silently rather than
    /// loudly: YAML is a superset of JSON, so a reload usually *worked*, applying YAML's
    /// rules to a document that had been read under `serde_json`'s.
    ///
    /// Overlays are unaffected and still choose their own parser by their own extension,
    /// which is what lets a JSON base take a YAML overlay.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    /// * `format` - The parser to read the file with, now and on every reload
    ///
    /// # Errors
    /// The same as [`load_required`](Config::load_required), except that the parse error is
    /// whichever `format` names rather than whichever the extension does
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, Format};
    /// // A YAML document in a file named as though it were something else. Reading a
    /// // JSON or TOML one is the same call with `Format::Json` / `Format::Toml`, which
    /// // exist once their features are enabled.
    /// let config = Config::load_required_as("settings.json", "/", None, Format::Yaml)
    ///     .expect("Failed to load settings.json");
    /// ```
    pub fn load_required_as(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: parser::Format,
    ) -> Result<Config, ConfigError> {
        Self::load_internal_as(filename, sep, env, Some(format))
    }

    /// Loads a Config from a file, treating a missing file as an empty config.
    ///
    /// Use this when the config file is optional. If the file doesn't exist, returns
    /// `Ok` with an empty config. If the file *does* exist but is invalid (bad YAML/JSON/TOML,
    /// permission denied), returns `Err` — a present-but-broken config file is likely
    /// a mistake worth surfacing.
    ///
    /// For a file that must exist, use [`load_required`](Config::load_required).
    ///
    /// The resolved filename is recorded even when the file is missing, so a config
    /// loaded before its file exists can pick it up on a later [`reload`](Config::reload).
    /// Until the file appears, `reload` returns `IoError` (`NotFound`) and leaves the
    /// config unchanged.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Returns
    /// Returns `Ok(Config)` on success or if the file is not found
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file exists but cannot be read (e.g. permission denied)
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// // Load an environment-specific override file -- fine if it doesn't exist
    /// let config = Config::load_optional("config.dev.yaml", "/", None)?;
    ///
    /// // With custom separator
    /// let config = Config::load_optional("config.yaml", "::", None)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_optional(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        Self::load_optional_internal(filename, sep, env, None)
    }

    /// Loads a Config from a file read as `format`, treating a missing file as an empty
    /// config.
    ///
    /// [`load_optional`](Config::load_optional) with the format pinned, for the same reason
    /// [`load_required_as`](Config::load_required_as) exists: an extension that does not
    /// name the format. An optional `app.cfg` holding JSON is exactly as likely as a
    /// required one, and without this the choice was rename the file, hand-roll the
    /// exists-check (losing the recorded filename a later [`reload`](Config::reload) needs),
    /// or let YAML's rules be applied to a JSON document — which usually succeeds, quietly.
    ///
    /// The format is recorded even when the file is absent, alongside the filename, so it
    /// still governs the reload that picks the file up once it appears.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    /// * `format` - The parser to read the file with, now and on every reload
    ///
    /// # Errors
    /// The same as [`load_optional`](Config::load_optional), except that the parse error is
    /// whichever `format` names rather than whichever the extension does
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError, Format};
    /// # fn main() -> Result<(), ConfigError> {
    /// // Optional, and YAML despite the extension. `Format::Json` reads the same file as
    /// // JSON instead, once the `json` feature is enabled.
    /// let config = Config::load_optional_as("overrides.json", "/", None, Format::Yaml)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_optional_as(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: parser::Format,
    ) -> Result<Config, ConfigError> {
        Self::load_optional_internal(filename, sep, env, Some(format))
    }

    fn load_optional_internal(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: Option<parser::Format>,
    ) -> Result<Config, ConfigError> {
        match Self::load_internal_as(filename, sep, env, format) {
            Ok(config) => Ok(config),
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                // Record the file this config *would* have come from. Discarding it
                // would leave the config with no source at all, and `reload()` refuses
                // to run without one — so a file that appears later could never be
                // picked up. `load_internal_as` resolved both `sep` and the filename
                // template before failing, so neither can fail here.
                //
                // The format is recorded too, for the same reason: it is the other half of
                // "how this config is re-read", and dropping it would send the reload that
                // finally finds the file to the parser its extension names.
                let (file, env) = get_file(filename, env)?;
                Self::from_parsed(Value::Null, &file, sep, env, format)
            },
            Err(e) => Err(e),
        }
    }

    /// Loads a Config from a file, creating it from a default string if it doesn't exist.
    ///
    /// If the file exists, its content is loaded and returned — the `defaults` string is
    /// discarded. If the file does not exist, `defaults` is written to disk and returned as
    /// the active config, so the app behaves identically whether or not the file was present.
    ///
    /// The `defaults` string is written as-is, preserving formatting and comments. It must
    /// be in the same format as the file: YAML by default, or JSON/TOML when the filename
    /// has a matching extension and the corresponding feature is enabled. The created config
    /// records the filename, so [`reload`](Config::reload) works after a first run.
    ///
    /// Defaults that do not parse are rejected **before** anything is written, so a failure
    /// here leaves no file behind and the next run retries the creation from scratch. The
    /// file is created exclusively: if a second process wins the race to create it, this
    /// call loads that file rather than overwriting it.
    ///
    /// # First-run races
    ///
    /// Creating the file and filling it are two syscalls, so the winner of that race leaves
    /// a zero-length file visible for a moment. A loser arriving in that gap would read
    /// nothing and return an **empty** config — no error, defaults discarded, every accessor
    /// answering `""` / `None` / `[]`. To close it, a config that reads as empty *from a
    /// zero-length file* is re-read for up to 200 ms before being accepted, which is far
    /// longer than the gap and is only ever waited out when the file really is empty.
    ///
    /// What remains: a file still zero-length after that wait is returned as an empty
    /// config, and one still unparseable is returned as the parse error. Both are the right
    /// answer by then — 200 ms is not a partial write. A deliberately empty file is
    /// therefore honoured, at the cost of that wait; pass empty `defaults` and it is
    /// returned immediately instead.
    ///
    /// Only the file is created — **parent directories are not**. A missing parent returns
    /// `IoError` rather than being created, so a mistyped path cannot leave a junk directory
    /// tree behind; call [`std::fs::create_dir_all`] first if the directory may not exist.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    /// * `defaults` - Config string to write and use if the file does not exist, in the file's format
    ///
    /// # Returns
    /// Returns `Ok(Config)` with the file content, or the defaults if the file was created
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, the file exists but cannot be read, or if creating or writing the file fails
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file
    ///     or the defaults string cannot be parsed in the format matching the file extension.
    ///     A defaults string that fails to parse is reported without the file having been created
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash, or the filename template is invalid
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// const DEFAULTS: &str = r#"
    /// app:
    ///   port: 8080
    ///   debug: false
    /// database:
    ///   host: localhost
    ///   port: 5432
    /// "#;
    ///
    /// let config = Config::load_or_create("config.yaml", "/", None, DEFAULTS)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_or_create(filename: &str, sep: &str, env: Option<&str>, defaults: &str) -> Result<Config, ConfigError> {
        Self::load_or_create_internal(filename, sep, env, None, defaults)
    }

    /// Loads a Config from a file read as `format`, creating it from `defaults` if it does
    /// not exist.
    ///
    /// [`load_or_create`](Config::load_or_create) with the format pinned, completing the
    /// set alongside [`load_required_as`](Config::load_required_as) and
    /// [`load_optional_as`](Config::load_optional_as). A first-run file whose extension does
    /// not name its format is as ordinary as any other; without this there was no way to
    /// create one.
    ///
    /// `defaults` must be in `format` — and, unlike everything else here, that is not merely
    /// a matter of re-deriving the same answer twice. The defaults are validated before
    /// being written, and validating them against the *extension* while the reader parses
    /// the *format* is the one combination that gives an outright wrong answer: JSON
    /// defaults under a `.conf` name would be checked as YAML, pass because YAML is a
    /// superset of JSON, and be written to a file this constructor then reads as JSON.
    /// `format` governs both halves.
    ///
    /// # Arguments
    /// * `filename` - Path to the config file (can contain `{env}` placeholder)
    /// * `sep` - Path separator for accessing nested values
    /// * `env` - Optional environment name to substitute in filename. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    /// * `format` - The parser to read the file and validate `defaults` with, now and on
    ///   every reload
    /// * `defaults` - Config string to write and use if the file does not exist, in `format`
    ///
    /// # Errors
    /// The same as [`load_or_create`](Config::load_or_create), with `format` deciding which
    /// parse error the file and the defaults can produce
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError, Format};
    /// # fn main() -> Result<(), ConfigError> {
    /// // `defaults` is in `format`, not in whatever the extension suggests
    /// const DEFAULTS: &str = r#"
    /// app:
    ///   port: 8080
    /// "#;
    ///
    /// let config = Config::load_or_create_as("settings.json", "/", None, Format::Yaml, DEFAULTS)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_or_create_as(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: parser::Format,
        defaults: &str,
    ) -> Result<Config, ConfigError> {
        Self::load_or_create_internal(filename, sep, env, Some(format), defaults)
    }

    fn load_or_create_internal(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: Option<parser::Format>,
        defaults: &str,
    ) -> Result<Config, ConfigError> {
        match Self::load_internal_as(filename, sep, env, format) {
            // The file was already there. It may still be the *winner's* file caught
            // between its creation and its contents, which reads as an empty config —
            // this is the likelier half of the race, since a process arriving a moment
            // late never reaches the create path below at all.
            Ok(config) => Self::settle_empty(config, filename, sep, env, format, defaults),
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                let (file, _) = get_file(filename, env)?;

                // Validate the defaults *before* writing them. Writing first and parsing
                // second left a broken file on disk when the defaults did not parse in the
                // file's format — and because the file then existed, this branch never ran
                // again: every subsequent run read the same broken file and failed
                // identically, turning a first-run error into a permanent one.
                //
                // In `format` when one was pinned, so this checks the defaults against the
                // parser that will actually read them back rather than against the one the
                // extension names.
                parser::parse_in(format, defaults, &file)?;

                // The parsed value is deliberately discarded and the file re-read below,
                // so the created config is built by exactly the same path as an existing
                // one — filename recorded, `reload()` working, no second code path to
                // keep in step.
                match create_new_file(&file, defaults) {
                    // We created *and* filled it, so there is nothing to wait for.
                    Ok(()) => return Self::load_internal_as(filename, sep, env, format),
                    // Another process created the file between the not-found check and
                    // here — the first-run race this method exists for. `create_new`
                    // means we did not clobber it; fall through and load what they wrote,
                    // once they have written it.
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
                    Err(e) => return Err(ConfigError::io_in(&file, e)),
                }

                let config = Self::load_internal_as(filename, sep, env, format)?;
                Self::settle_empty(config, filename, sep, env, format, defaults)
            },
            Err(e) => Err(e),
        }
    }

    /// Waits out a config that read as empty from a zero-length file, in case the file is
    /// mid-creation by another process.
    ///
    /// `create_new` makes creating the file atomic but not filling it, so between those two
    /// syscalls the file exists and is empty. Every format parses nothing to `Value::Null`,
    /// so a reader in that gap gets a `Config` that is indistinguishable from a legitimately
    /// empty one and reports no error at all — the silent-wrong-answer shape this crate
    /// rejects elsewhere (an empty path segment resolving, a `${VAR}` served as literal
    /// text). Re-reading for a bounded spell tells the two apart, because only one of them
    /// changes.
    ///
    /// Three guards keep this off every ordinary path, in increasing order of cost:
    /// a document that is not null has nothing to settle; empty `defaults` mean an empty
    /// file is the correct answer and there is nothing better to wait for; and a file that
    /// is not zero-length is empty for its own reasons — a comment-only document — rather
    /// than because a write is in flight.
    ///
    /// A parse failure during the wait is treated as "not settled yet" for the same reason:
    /// a partly-written document is not valid in any of the three formats. If it is still
    /// failing when the attempts run out, that error is returned — by then the file is
    /// broken rather than incomplete, which is what the caller needs to hear.
    fn settle_empty(
        config: Config,
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: Option<parser::Format>,
        defaults: &str,
    ) -> Result<Config, ConfigError> {
        if !matches!(config.content, Value::Null)
            || defaults.is_empty()
            || !is_zero_length(&config.filename)
        {
            return Ok(config);
        }

        // Holds whatever the most recent attempt saw, so the wait ending in a parse error
        // reports that rather than the empty config it started from.
        let mut latest = Ok(config);

        for _ in 0..EMPTY_FILE_RETRIES {
            thread::sleep(EMPTY_FILE_BACKOFF);
            match Self::load_internal_as(filename, sep, env, format) {
                Ok(config) if !matches!(config.content, Value::Null) => return Ok(config),
                outcome => latest = outcome,
            }
        }

        latest
    }

    /// Loads a file with an explicitly chosen parser, or by extension when `format` is
    /// `None`.
    ///
    /// The one path every file constructor takes, so the filename check, the separator
    /// check and `{env}` resolution cannot differ between them — which is what makes the
    /// format a *parameter* of the three constructors rather than an axis of new ones.
    /// `load_json_file` and `load_toml_file`, which this replaces, each bypassed all three
    /// checks, and skipping the last meant they alone could not take a `config.{env}.json`
    /// template.
    fn load_internal_as(
        filename: &str,
        sep: &str,
        env: Option<&str>,
        format: Option<parser::Format>,
    ) -> Result<Config, ConfigError> {
        if filename.is_empty() {
            return Err(empty_filename_error());
        }
        check_separator(sep)?;

        let (file, env) = get_file(filename, env)?;
        let parsed = parser::load_in(format, &file)?;

        Self::from_parsed(parsed, &file, sep, env, format)
    }

    /// Builds a `Config` from an already-parsed document, resolving `${VAR}`
    /// placeholders in its string values.
    ///
    /// Every constructor funnels through here, so the shape of a freshly-loaded
    /// `Config` — no overlays, env vars resolved exactly once — is defined in one place.
    ///
    /// Callers validate `sep` with `check_separator` *before* parsing, so that an empty
    /// separator is reported ahead of any parse error.
    fn from_parsed(
        content: Value,
        filename: &str,
        sep: &str,
        env: Option<String>,
        format: Option<parser::Format>,
    ) -> Result<Config, ConfigError> {
        Ok(Config {
            content: resolve_env_vars(content)?,
            filename: filename.to_string(),
            separator: sep.to_string(),
            environment: env,
            overlays: Vec::new(),
            format,
        })
    }

    /// Parses a YAML string into a Config object
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    /// Returns `ConfigError::YamlError` if YAML parsing fails
    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::yaml::parse(yaml)?, "", sep, None, None)
    }

    /// Parses a JSON string into a Config object.
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    /// Returns `ConfigError::JsonError` if JSON parsing fails
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_json(r#"{"app": {"port": 8080}}"#, "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    /// ```
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn load_json(json_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::json::parse(json_str)?, "", sep, None, None)
    }

    /// Parses a TOML string into a Config object.
    ///
    /// # Datetimes
    ///
    /// TOML has a date-time type and the value model this crate reads through does not,
    /// so a datetime is surfaced as the text the file contained: RFC 3339 for an offset
    /// date-time, and TOML's own forms for the local date, time and date-time variants.
    /// It behaves as a scalar like any other — readable with [`str`](Config::str), listed
    /// by [`outline`](Config::outline), and deserializable into a `String` or into
    /// `chrono`/`time`/`jiff`'s date types, all of which parse RFC 3339.
    ///
    /// It will *not* deserialize into `toml::value::Datetime`, which recognises only the
    /// private marker its own crate serializes; naming that type means taking a direct
    /// dependency on `toml`, which the [crate docs](crate#value-model) advise against.
    ///
    /// # Errors
    /// Returns `ConfigError::TomlError` if the TOML cannot be parsed
    /// Returns `ConfigError::FormatError` if the separator is empty or contains a backslash
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_toml("[app]\nport = 8080", "/").unwrap();
    /// assert_eq!(config.get_int("app/port"), Some(8080));
    ///
    /// // A datetime reads back as the text the file held
    /// let config = Config::load_toml("[window]\nstarts = 2024-01-01T00:00:00Z", "/").unwrap();
    /// assert_eq!(config.str("window/starts"), "2024-01-01T00:00:00Z");
    /// ```
    #[cfg(feature = "toml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
    pub fn load_toml(toml_str: &str, sep: &str) -> Result<Config, ConfigError> {
        check_separator(sep)?;
        Self::from_parsed(parser::toml::parse(toml_str)?, "", sep, None, None)
    }
}

/// Creates `file` and writes `contents`, failing with `AlreadyExists` if it is already there.
///
/// `fs::write` truncates, so it would silently clobber a config written by a second process
/// starting at the same moment — precisely the first-run scenario `load_or_create` exists
/// for. `create_new` makes the existence check and the creation a single atomic operation,
/// and turns the loser of the race into an `AlreadyExists` the caller can handle by loading
/// the winner's file.
///
/// The file is created empty and filled a moment later, so a racing reader can still
/// observe it part-written. Closing that here would need a write-to-temp-and-rename dance,
/// and rename replaces the destination — reintroducing the clobbering this call prevents.
/// It is closed on the *reading* side instead, by `Config::settle_empty`, which waits a
/// zero-length file out rather than accepting it as an empty config.
pub(super) fn create_new_file(file: &str, contents: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)?
        .write_all(contents.as_bytes())
}

/// Reports whether `file` exists and holds no bytes at all.
///
/// The precise shape a lost `create_new` race leaves behind, and narrower than "parses to
/// nothing": a document of only comments also parses to `Value::Null` but is not
/// zero-length, so it is accepted at once rather than waited on. An unreadable file
/// answers `false` — whatever is wrong with it, a wait will not fix it, and the caller
/// already has the parse or I/O error that says so.
fn is_zero_length(file: &str) -> bool {
    fs::metadata(file).map(|meta| meta.len() == 0).unwrap_or(false)
}

/// Rejects a path separator that cannot work: empty, or containing a backslash.
///
/// An empty separator would make every config path unparseable. A backslash collides
/// with its own role as the escape character in path syntax (`src/config/path.rs`) —
/// the splitter tests for an escape sequence before it tests for the separator, so a
/// separator starting with `\` is consumed as an escape and never matches. The path
/// then collapses to a single segment and *every* lookup returns `None` / `""` / `[]`
/// with no error raised anywhere, which is the worst failure shape a config library
/// has. Any backslash is rejected, not just a leading one: a separator with one buried
/// in the middle happens to split correctly today, but the rule "the separator may not
/// contain the escape character" is the one worth being able to state.
///
/// Called at the top of each constructor, before parsing, so that a bad separator is
/// reported ahead of any parse error in the document.
fn check_separator(sep: &str) -> Result<(), ConfigError> {
    if sep.is_empty() {
        return Err(ConfigError::FormatError("Separator cannot be empty".to_string()));
    }
    if sep.contains('\\') {
        return Err(ConfigError::FormatError(format!(
            "Separator {:?} cannot contain a backslash: '\\' is the escape character \
             used to put a literal separator in a key",
            sep
        )));
    }
    Ok(())
}

/// Resolves a filename template against an optional environment name, returning the
/// concrete filename and the environment to record on the `Config`.
///
/// An environment supplies a value for `{env}` *if the filename uses it*. A filename
/// without the placeholder is not an error: in a layered setup only some of the files
/// are environment-specific, and the environment is still worth recording.
///
/// The reverse is an error. A `{env}` with nothing to substitute would otherwise be
/// handed to the OS verbatim and come back as a missing `config.{env}.yaml`, which
/// points at the wrong problem.
///
/// `env` is substituted into a filesystem path with no validation, so a value of
/// `../../secrets` builds exactly that path. Callers pass a literal or a trusted
/// `APP_ENV`, which is what makes this acceptable; the constructors' rustdoc says
/// not to pass untrusted input.
pub(super) fn get_file(filename: &str, env: Option<&str>) -> Result<(String, Option<String>), ConfigError> {
    let has_placeholder = filename.contains("{env}");

    match (env, has_placeholder) {
        (Some(value), true) => Ok((filename.replace("{env}", value), Some(value.to_string()))),
        (Some(value), false) => Ok((filename.to_string(), Some(value.to_string()))),
        (None, true) => Err(ConfigError::FormatError(format!(
            "Filename template \"{}\" contains '{{env}}' but no environment was supplied",
            filename
        ))),
        (None, false) => Ok((filename.to_string(), None)),
    }
}

/// The error returned when a config filename is empty.
///
/// An empty filename is almost always a caller bug rather than a "missing file",
/// so it is rejected upfront with `InvalidInput` instead of being handed to the OS
/// (which would report `NotFound`, indistinguishable from a genuinely-absent named file).
pub(super) fn empty_filename_error() -> ConfigError {
    ConfigError::IoError {
        file: None,
        source: io::Error::new(io::ErrorKind::InvalidInput, "filename cannot be empty"),
    }
}
