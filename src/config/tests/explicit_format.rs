//! The `_as` constructors: the format they are given has to stick.
//!
//! `load_required` already routes a `.json` file to the JSON parser, so the only reason to
//! name a format is a file whose extension does not. That was exactly the case the crate got
//! wrong: the file was parsed once with the chosen parser, and every later read — `reload`,
//! `reload_from` — re-derived the format from the extension and used a different one.
//! Silently, since YAML is a superset of JSON.
//!
//! These tests were written against `load_json_file` / `load_toml_file`, which
//! `load_required_as` replaced when the format became a parameter rather than an axis of
//! constructors; `load_optional_as` and `load_or_create_as` are covered at the end.

#![cfg(any(feature = "json", feature = "toml"))]

use super::{Config, ConfigError};
use crate::config::Format;
use crate::test_util::{temp_dir, write_file};
use std::fs;

#[test]
#[cfg(feature = "json")]
fn a_json_file_under_a_foreign_extension_reloads_as_json() {
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", r#"{"app": {"port": 8080}}"#);

    let mut config = Config::load_required_as(&path, "/", None, Format::Json).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    fs::write(&path, r#"{"app": {"port": 9090}}"#).unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));

    // The proof that it is the JSON parser and not YAML-as-a-superset: `{app: 1}` is a
    // valid YAML flow mapping and invalid JSON, so the error variant names the parser.
    fs::write(&path, "{app: 1}").unwrap();
    match config.reload() {
        Err(ConfigError::JsonError { .. }) => (),
        other => panic!("reload should have used the JSON parser, got: {:?}", other.err()),
    }
}

#[test]
#[cfg(feature = "toml")]
fn a_toml_file_under_a_foreign_extension_reloads_as_toml() {
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", "[app]\nport = 8080\n");

    let mut config = Config::load_required_as(&path, "/", None, Format::Toml).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    fs::write(&path, "[app]\nport = 9090\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));

    // Under the YAML parser a `[table]` header reads as something else entirely, so this
    // would have failed as a YamlError before the format was recorded
    fs::write(&path, "[app]\nport = 1\ninvalid = [unclosed\n").unwrap();
    match config.reload() {
        Err(ConfigError::TomlError { .. }) => (),
        other => panic!("reload should have used the TOML parser, got: {:?}", other.err()),
    }
}

#[test]
#[cfg(feature = "json")]
fn the_original_evidence_case_no_longer_diverges() {
    // A document that loads under one parser and fails under the other. Before the format
    // was recorded this loaded fine and then failed on reload with a *YAML* parse error,
    // naming a format the caller never asked for.
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", r#"{"a": "x", "b": "1"}"#);

    let mut config = Config::load_required_as(&path, "/", None, Format::Json).unwrap();
    assert_eq!(config.str("b"), "1");

    config.reload().unwrap();
    assert_eq!(config.str("b"), "1", "the reload must read the same document");
}

#[test]
#[cfg(feature = "json")]
fn an_explicit_format_survives_reload_from() {
    let dir = temp_dir();
    let first = write_file(&dir, "first.conf", r#"{"app": {"port": 1111}}"#);
    let second = write_file(&dir, "second.conf", r#"{"app": {"port": 2222}}"#);

    let mut config = Config::load_required_as(&first, "/", None, Format::Json).unwrap();
    config.reload_from(&second).unwrap();
    assert_eq!(config.get_int("app/port"), Some(2222));

    // And still on the file it switched to
    fs::write(&second, r#"{"app": {"port": 3333}}"#).unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(3333));
}

#[test]
#[cfg(feature = "json")]
fn switching_to_another_format_with_reload_from_fails_loudly() {
    // The deliberate consequence of preserving the pin. Dropping it instead would send a
    // JSON-pinned config reading a YAML file *successfully* — YAML is a superset, so the
    // wrong rules would apply in silence. Failing here is the point.
    let dir = temp_dir();
    let json = write_file(&dir, "a.conf", r#"{"app": {"port": 1111}}"#);
    let yaml = write_file(&dir, "b.yaml", "app:\n  port: 2222\n");

    let mut config = Config::load_required_as(&json, "/", None, Format::Json).unwrap();
    assert!(matches!(config.reload_from(&yaml), Err(ConfigError::JsonError { .. })));

    // And the failed switch left everything intact, as `reload_from` promises
    assert_eq!(config.get_int("app/port"), Some(1111));
    assert_eq!(config.filename(), json);
}

#[test]
#[cfg(feature = "json")]
fn overlays_still_choose_their_own_parser() {
    // The pin is on the base file only. A JSON base taking a YAML overlay is the mixed
    // -format layering the crate advertises, and it has to keep working — including
    // across a reload, where both files are read again.
    let dir = temp_dir();
    let base = write_file(&dir, "base.conf", r#"{"app": {"port": 8080, "name": "base"}}"#);
    let overlay = write_file(&dir, "over.yaml", "app:\n  name: overlaid\n");

    let mut config = Config::load_required_as(&base, "/", None, Format::Json).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.str("app/name"), "overlaid");

    fs::write(&overlay, "app:\n  name: reloaded\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.str("app/name"), "reloaded");
}

#[test]
#[cfg(feature = "json")]
fn an_env_placeholder_resolves_in_the_filename() {
    // These two took no `env` argument at all, so they alone could not load a
    // `config.{env}.json` — the braces went to the OS verbatim.
    let dir = temp_dir();
    write_file(&dir, "cfg.prod.conf", r#"{"app": {"port": 8080}}"#);
    let template = dir.path().join("cfg.{env}.conf").to_string_lossy().into_owned();

    let config = Config::load_required_as(&template, "/", Some("prod"), Format::Json).unwrap();

    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.environment(), Some("prod"), "the environment must be recorded");
    // The *resolved* name is stored, so a later reload reads the same file
    assert!(config.filename().ends_with("cfg.prod.conf"));
}

#[test]
#[cfg(feature = "json")]
fn a_placeholder_without_an_environment_is_an_error() {
    // Matching every other file constructor: `{env}` with nothing to substitute is
    // reported rather than handed to the OS as a literal
    let dir = temp_dir();
    let template = dir.path().join("cfg.{env}.conf").to_string_lossy().into_owned();

    assert!(matches!(
        Config::load_required_as(&template, "/", None, Format::Json),
        Err(ConfigError::FormatError(_))
    ));
}

#[test]
#[cfg(feature = "json")]
fn the_shared_argument_checks_apply() {
    // Both used to bypass the empty-filename and separator checks that every other
    // constructor runs, because they did not go through `load_internal`.
    assert!(matches!(
        Config::load_required_as("", "/", None, Format::Json),
        Err(ConfigError::IoError { .. })
    ));
    assert!(matches!(
        Config::load_required_as("x.conf", "", None, Format::Json),
        Err(ConfigError::FormatError(_))
    ));
    assert!(matches!(
        Config::load_required_as("x.conf", "a\\b", None, Format::Json),
        Err(ConfigError::FormatError(_))
    ));
}

#[test]
#[cfg(feature = "json")]
fn a_config_without_a_pin_still_follows_the_extension() {
    // The default path is untouched: no pin means "decide from the extension, every
    // time", including after a `reload_from` onto a different format.
    let dir = temp_dir();
    let yaml = write_file(&dir, "a.yaml", "app:\n  port: 1111\n");
    let json = write_file(&dir, "b.json", r#"{"app": {"port": 2222}}"#);

    let mut config = Config::load_required(&yaml, "/", None).unwrap();
    config.reload_from(&json).unwrap();
    assert_eq!(config.get_int("app/port"), Some(2222));
}

#[test]
#[cfg(feature = "json")]
fn load_optional_as_keeps_the_pin_across_an_absent_file() {
    // The gap `load_json_file` left: an optional config whose extension does not name its
    // format had no constructor at all. The pin has to survive the *absent* case too, or
    // the reload that finally finds the file reads it by extension — as YAML, which
    // usually succeeds because YAML is a superset of JSON.
    let dir = temp_dir();
    let path = dir.path().join("overrides.cfg").to_string_lossy().into_owned();

    let mut config = Config::load_optional_as(&path, "/", None, Format::Json).unwrap();
    assert_eq!(config.get_int("app/port"), None, "absent, so empty");
    assert_eq!(config.filename(), path, "but the filename is recorded");

    // `{app: 1}` is a valid YAML flow mapping and invalid JSON, so the error variant
    // proves which parser the reload used
    fs::write(&path, "{app: 1}").unwrap();
    let err = config.reload().unwrap_err();
    assert!(matches!(err, ConfigError::JsonError { .. }), "got {err:?}");

    fs::write(&path, r#"{"app": {"port": 8080}}"#).unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
#[cfg(feature = "json")]
fn load_or_create_as_validates_the_defaults_in_the_pinned_format() {
    // The one place deriving the format from the extension would be outright wrong rather
    // than merely re-derived: YAML defaults under a `.conf` name pinned to JSON. Validated
    // by extension they would pass as YAML and be written to a file the very next read
    // parses as JSON — a file broken on creation, by the step that exists to prevent that.
    let dir = temp_dir();
    let path = dir.path().join("settings.conf").to_string_lossy().into_owned();

    let err = Config::load_or_create_as(&path, "/", None, Format::Json, "app:\n  port: 8080\n")
        .unwrap_err();

    assert!(matches!(err, ConfigError::JsonError { .. }), "got {err:?}");
    assert!(fs::metadata(&path).is_err(), "nothing should have been written");
}

#[test]
#[cfg(feature = "json")]
fn load_or_create_as_creates_and_then_reloads_in_the_pinned_format() {
    let dir = temp_dir();
    let path = dir.path().join("settings.conf").to_string_lossy().into_owned();

    let mut config =
        Config::load_or_create_as(&path, "/", None, Format::Json, r#"{"app": {"port": 8080}}"#)
            .unwrap();

    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"app": {"port": 8080}}"#);

    // The pin is recorded by the create path too, not just by a plain load
    fs::write(&path, "{app: 1}").unwrap();
    assert!(matches!(config.reload().unwrap_err(), ConfigError::JsonError { .. }));
}

#[test]
#[cfg(feature = "json")]
fn debug_shows_a_pinned_format_and_omits_it_otherwise() {
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", r#"{"app": {"port": 8080}}"#);

    let pinned = Config::load_required_as(&path, "/", None, Format::Json).unwrap();
    assert!(format!("{:?}", pinned).contains("format: Json"), "got: {:?}", pinned);

    // Nothing unusual to report for the common case, so nothing is printed
    let plain = Config::load_yaml("app:\n  port: 1\n", "/").unwrap();
    assert!(!format!("{:?}", plain).contains("format"), "got: {:?}", plain);
}
