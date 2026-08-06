#![cfg(feature = "json")]

use super::{Config, ConfigError};
use crate::test_util::{env_lock, temp_dir, write_file};
use std::fs;

#[test]
fn load_json_string() {
    let config = Config::load_json(r#"{"app": {"port": 8080, "debug": true}}"#, "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn load_json_nested() {
    let json = r#"{
        "db": {
            "redis": {
                "host": "127.0.0.1",
                "port": 6379
            }
        }
    }"#;
    let config = Config::load_json(json, "/").unwrap();
    assert_eq!(config.str("db/redis/host"), "127.0.0.1");
    assert_eq!(config.get_int("db/redis/port"), Some(6379));
}

#[test]
fn load_json_with_custom_separator() {
    let config = Config::load_json(r#"{"app": {"port": 8080}}"#, "::").unwrap();
    assert_eq!(config.get_int("app::port"), Some(8080));
}

#[test]
fn load_json_file_auto_detect() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.json", r#"{"app": {"port": 3000}}"#);

    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(3000));
}

#[test]
fn load_json_file_explicit() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.json", r#"{"app": {"name": "myapp"}}"#);

    let config = Config::load_json_file(&path, "/").unwrap();
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn load_json_list() {
    let config = Config::load_json(r#"{"items": ["one", "two", "three"]}"#, "/").unwrap();
    assert_eq!(config.list("items"), vec!["one", "two", "three"]);
}

#[test]
fn load_json_env_var_interpolation() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_JSON_HOST", "json-server");
    let config = Config::load_json(r#"{"db": {"host": "${TRAIL_TEST_JSON_HOST}"}}"#, "/").unwrap();
    assert_eq!(config.str("db/host"), "json-server");
    std::env::remove_var("TRAIL_TEST_JSON_HOST");
}

#[test]
fn duplicate_json_keys_are_rejected() {
    // All three formats now agree: YAML and TOML always rejected a duplicated key, and
    // JSON used to take the last one silently, because it parsed through
    // `serde_json::Value` whose map simply overwrites. Deserializing straight into this
    // crate's value model — done for key order — brought JSON in line, since that
    // model's visitor refuses a duplicate entry.
    let result = Config::load_json(r#"{"a": 1, "a": 2}"#, "/");

    match result {
        Err(ConfigError::JsonError { ref source, .. }) => {
            assert!(source.to_string().contains("duplicate"), "got: {source}");
        },
        other => panic!("Expected JsonError for a duplicate key, got: {:?}", other.map(|_| ())),
    }

    // Nested, and not confused by the same key name at a different level
    assert!(Config::load_json(r#"{"o": {"a": 1, "a": 2}}"#, "/").is_err());
    assert!(Config::load_json(r#"{"o": {"a": 1}, "p": {"a": 2}}"#, "/").is_ok());
}

#[test]
fn json_documents_reject_trailing_content() {
    // Parsing now drives `serde_json` into this crate's value model rather than into
    // `serde_json::Value`. `from_str` still checks that the document is the whole input,
    // which a hand-driven `Deserializer` would not have.
    assert!(Config::load_json(r#"{"a": 1} trailing"#, "/").is_err());
    assert!(Config::load_json(r#"{"a": 1}{"b": 2}"#, "/").is_err());
}

#[test]
fn load_json_invalid_errors() {
    let result = Config::load_json("{invalid json}", "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::JsonError { .. }) => (),
        other => panic!("Expected JsonError, got: {:?}", other),
    }
}

#[test]
fn load_json_empty_separator_errors() {
    let result = Config::load_json(r#"{"a": 1}"#, "");
    assert!(result.is_err());
}

#[test]
fn uppercase_json_extension_reaches_the_json_parser() {
    let dir = temp_dir();

    // `{a: 1}` is valid YAML (a flow mapping with an unquoted key) and invalid JSON, so
    // the error variant is what proves which parser ran. Under the old byte-exact
    // `ends_with(".json")` this loaded *successfully* as YAML — the worse half of the
    // bug, since JSON is a subset of YAML and the divergence only shows up later, in
    // duplicate-key and number handling.
    for name in ["c.JSON", "c.Json"] {
        let path = write_file(&dir, name, "{a: 1}");
        match Config::load_required(&path, "/", None) {
            Err(ConfigError::JsonError { .. }) => (),
            other => panic!("{name} should reach the JSON parser, got: {:?}", other.err()),
        }
    }

    // Valid JSON under an uppercase extension loads, as it always did
    let path = write_file(&dir, "ok.JSON", r#"{"app": {"port": 8080}}"#);
    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn a_file_named_dot_json_is_a_dotfile_not_an_extension() {
    // `.json` is all name and no extension, like `.gitignore`, so it parses as YAML.
    // `ends_with(".json")` could not tell the two apart.
    let dir = temp_dir();
    let path = write_file(&dir, ".json", "app:\n  port: 8080\n");

    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn load_or_create_json_defaults() {
    let dir = temp_dir();
    let path = dir.path().join("new.json").to_string_lossy().into_owned();

    // Defaults for a .json file are parsed as JSON
    let config = Config::load_or_create(&path, "/", None, r#"{"app": {"port": 8080}}"#).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"app": {"port": 8080}}"#);

    // Invalid JSON defaults surface as JsonError
    let path2 = dir.path().join("bad.json").to_string_lossy().into_owned();
    let result = Config::load_or_create(&path2, "/", None, "{invalid json}");
    match result {
        Err(ConfigError::JsonError { .. }) => (),
        other => panic!("Expected JsonError, got: {:?}", other),
    }
    assert!(!fs::exists(&path2).unwrap(), "defaults that fail to parse must not be written");
}

#[test]
fn merge_json_overlay() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let overlay = write_file(&dir, "overlay.json", r#"{"app": {"port": 9090}}"#);

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn reload_json_file() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.json", r#"{"app": {"port": 8080}}"#);

    let mut config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    fs::write(&path, r#"{"app": {"port": 9090}}"#).unwrap();

    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
}
