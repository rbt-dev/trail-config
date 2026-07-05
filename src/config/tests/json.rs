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
