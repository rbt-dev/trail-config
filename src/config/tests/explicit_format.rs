//! `load_json_file` / `load_toml_file`: the format they choose has to stick.
//!
//! `load_required` already routes a `.json` file to the JSON parser, so the only reason to
//! call these is a file whose extension does not name its format. That was exactly the case
//! they got wrong: the file was parsed once with the chosen parser, and every later read —
//! `reload`, `reload_from` — re-derived the format from the extension and used a different
//! one. Silently, since YAML is a superset of JSON.

#![cfg(any(feature = "json", feature = "toml"))]

use super::{Config, ConfigError};
use crate::test_util::{temp_dir, write_file};
use std::fs;

#[test]
#[cfg(feature = "json")]
fn a_json_file_under_a_foreign_extension_reloads_as_json() {
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", r#"{"app": {"port": 8080}}"#);

    let mut config = Config::load_json_file(&path, "/", None).unwrap();
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

    let mut config = Config::load_toml_file(&path, "/", None).unwrap();
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

    let mut config = Config::load_json_file(&path, "/", None).unwrap();
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

    let mut config = Config::load_json_file(&first, "/", None).unwrap();
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

    let mut config = Config::load_json_file(&json, "/", None).unwrap();
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

    let mut config = Config::load_json_file(&base, "/", None).unwrap()
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

    let config = Config::load_json_file(&template, "/", Some("prod")).unwrap();

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
        Config::load_json_file(&template, "/", None),
        Err(ConfigError::FormatError(_))
    ));
}

#[test]
#[cfg(feature = "json")]
fn the_shared_argument_checks_apply() {
    // Both used to bypass the empty-filename and separator checks that every other
    // constructor runs, because they did not go through `load_internal`.
    assert!(matches!(
        Config::load_json_file("", "/", None),
        Err(ConfigError::IoError { .. })
    ));
    assert!(matches!(
        Config::load_json_file("x.conf", "", None),
        Err(ConfigError::FormatError(_))
    ));
    assert!(matches!(
        Config::load_json_file("x.conf", "a\\b", None),
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
fn debug_shows_a_pinned_format_and_omits_it_otherwise() {
    let dir = temp_dir();
    let path = write_file(&dir, "settings.conf", r#"{"app": {"port": 8080}}"#);

    let pinned = Config::load_json_file(&path, "/", None).unwrap();
    assert!(format!("{:?}", pinned).contains("format: Json"), "got: {:?}", pinned);

    // Nothing unusual to report for the common case, so nothing is printed
    let plain = Config::load_yaml("app:\n  port: 1\n", "/").unwrap();
    assert!(!format!("{:?}", plain).contains("format"), "got: {:?}", plain);
}
