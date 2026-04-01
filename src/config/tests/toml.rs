#![cfg(feature = "toml")]

use super::{Config, ConfigError};
use std::fs::{self, File};
use std::io::Write;

#[test]
fn load_toml_string() {
    let config = Config::load_toml("[app]\nport = 8080\ndebug = true", "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn load_toml_nested() {
    let toml_str = r#"
[db.redis]
host = "127.0.0.1"
port = 6379
"#;
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.str("db/redis/host"), "127.0.0.1");
    assert_eq!(config.get_int("db/redis/port"), Some(6379));
}

#[test]
fn load_toml_with_custom_separator() {
    let config = Config::load_toml("[app]\nport = 8080", "::").unwrap();
    assert_eq!(config.get_int("app::port"), Some(8080));
}

#[test]
fn load_toml_file_auto_detect() {
    let path = "test_auto_detect.toml";
    let mut f = File::create(path).unwrap();
    write!(f, "[app]\nport = 3000").unwrap();
    drop(f);

    let config = Config::load_required(path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(3000));

    fs::remove_file(path).ok();
}

#[test]
fn load_toml_file_explicit() {
    let path = "test_explicit_toml.toml";
    let mut f = File::create(path).unwrap();
    write!(f, "[app]\nname = \"myapp\"").unwrap();
    drop(f);

    let config = Config::load_toml_file(path, "/").unwrap();
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(path).ok();
}

#[test]
fn load_toml_list() {
    let toml_str = "items = [\"one\", \"two\", \"three\"]";
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.list("items"), vec!["one", "two", "three"]);
}

#[test]
fn load_toml_env_var_interpolation() {
    std::env::set_var("TRAIL_TEST_TOML_HOST", "toml-server");
    let toml_str = "[db]\nhost = \"${TRAIL_TEST_TOML_HOST}\"";
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.str("db/host"), "toml-server");
    std::env::remove_var("TRAIL_TEST_TOML_HOST");
}

#[test]
fn load_toml_invalid_errors() {
    let result = Config::load_toml("invalid = [unclosed", "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlError(_)) => (),
        other => panic!("Expected TomlError, got: {:?}", other),
    }
}

#[test]
fn load_toml_empty_separator_errors() {
    let result = Config::load_toml("[a]\nb = 1", "");
    assert!(result.is_err());
}

#[test]
fn merge_toml_overlay() {
    let base = "test_merge_toml_base.yaml";
    let overlay = "test_merge_toml_overlay.toml";

    let mut f = File::create(base).unwrap();
    writeln!(f, "app:\n  port: 8080\n  name: myapp").unwrap();
    drop(f);

    let mut f = File::create(overlay).unwrap();
    write!(f, "[app]\nport = 9090").unwrap();
    drop(f);

    let config = Config::load_required(base, "/", None).unwrap()
        .merge_required(overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(base).ok();
    fs::remove_file(overlay).ok();
}

#[test]
fn reload_toml_file() {
    let path = "test_reload_toml.toml";
    let mut f = File::create(path).unwrap();
    write!(f, "[app]\nport = 8080").unwrap();
    drop(f);

    let mut config = Config::load_required(path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    let mut f = File::create(path).unwrap();
    write!(f, "[app]\nport = 9090").unwrap();
    drop(f);

    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));

    fs::remove_file(path).ok();
}
