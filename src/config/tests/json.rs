#![cfg(feature = "json")]

use super::{Config, ConfigError};
use std::fs::{self, File};
use std::io::Write;

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
    let path = "test_auto_detect.json";
    let mut f = File::create(path).unwrap();
    write!(f, r#"{{"app": {{"port": 3000}}}}"#).unwrap();
    drop(f);

    let config = Config::load_required(path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(3000));

    fs::remove_file(path).ok();
}

#[test]
fn load_json_file_explicit() {
    let path = "test_explicit_json.json";
    let mut f = File::create(path).unwrap();
    write!(f, r#"{{"app": {{"name": "myapp"}}}}"#).unwrap();
    drop(f);

    let config = Config::load_json_file(path, "/").unwrap();
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(path).ok();
}

#[test]
fn load_json_list() {
    let config = Config::load_json(r#"{"items": ["one", "two", "three"]}"#, "/").unwrap();
    assert_eq!(config.list("items"), vec!["one", "two", "three"]);
}

#[test]
fn load_json_env_var_interpolation() {
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
        Err(ConfigError::JsonError(_)) => (),
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
    let base = "test_merge_json_base.yaml";
    let overlay = "test_merge_json_overlay.json";

    let mut f = File::create(base).unwrap();
    writeln!(f, "app:\n  port: 8080\n  name: myapp").unwrap();
    drop(f);

    let mut f = File::create(overlay).unwrap();
    write!(f, r#"{{"app": {{"port": 9090}}}}"#).unwrap();
    drop(f);

    let config = Config::load_required(base, "/", None).unwrap()
        .merge_required(overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(base).ok();
    fs::remove_file(overlay).ok();
}

#[test]
fn reload_json_file() {
    let path = "test_reload_json.json";
    let mut f = File::create(path).unwrap();
    write!(f, r#"{{"app": {{"port": 8080}}}}"#).unwrap();
    drop(f);

    let mut config = Config::load_required(path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    let mut f = File::create(path).unwrap();
    write!(f, r#"{{"app": {{"port": 9090}}}}"#).unwrap();
    drop(f);

    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));

    fs::remove_file(path).ok();
}