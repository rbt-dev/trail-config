use super::{Config, ConfigError};

#[test]
fn reload_from_same_file() {
    use std::fs::{self, File};
    use std::io::Write;

    let test_file = "test_reload_config.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "app:\n  port: 8080\n  debug: false").unwrap();
    drop(file);

    let mut config = Config::load_optional(test_file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/debug"), "false");

    let mut file = File::create(test_file).unwrap();
    writeln!(file, "app:\n  port: 9090\n  debug: true").unwrap();
    drop(file);

    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "true");

    fs::remove_file(test_file).ok();
}

#[test]
fn reload_from_different_file() {
    use std::fs::{self, File};
    use std::io::Write;

    let file1 = "test_reload_file1.yaml";
    let file2 = "test_reload_file2.yaml";

    let mut file = File::create(file1).unwrap();
    writeln!(file, "config:\n  name: first\n  value: 100").unwrap();
    drop(file);

    let mut file = File::create(file2).unwrap();
    writeln!(file, "config:\n  name: second\n  value: 200").unwrap();
    drop(file);

    let mut config = Config::load_optional(file1, "/", None).unwrap();
    assert_eq!(config.str("config/name"), "first");
    assert_eq!(config.str("config/value"), "100");
    assert_eq!(config.get_filename(), file1);

    config.reload_from(file2).unwrap();
    assert_eq!(config.str("config/name"), "second");
    assert_eq!(config.str("config/value"), "200");
    assert_eq!(config.get_filename(), file2);

    fs::remove_file(file1).ok();
    fs::remove_file(file2).ok();
}

#[test]
fn reload_preserves_separator() {
    use std::fs::{self, File};
    use std::io::Write;

    let test_file = "test_reload_sep.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "db:\n  host: localhost\n  port: 5432").unwrap();
    drop(file);

    let mut config = Config::load_optional(test_file, "::", None).unwrap();
    assert_eq!(config.str("db::host"), "localhost");

    let mut file = File::create(test_file).unwrap();
    writeln!(file, "db:\n  host: remote\n  port: 3306").unwrap();
    drop(file);

    config.reload().unwrap();
    assert_eq!(config.str("db::host"), "remote");

    fs::remove_file(test_file).ok();
}

#[test]
fn reload_from_string_config_fails() {
    let yaml = "test: value";
    let mut config = Config::load_yaml(yaml, "/").unwrap();

    let result = config.reload();
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("no file path"));
        },
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn reload_from_invalid_yaml_fails() {
    use std::fs::{self, File};
    use std::io::Write;

    let test_file = "test_reload_invalid.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "valid:\n  yaml: content").unwrap();
    drop(file);

    let mut config = Config::load_optional(test_file, "/", None).unwrap();

    let mut file = File::create(test_file).unwrap();
    writeln!(file, "invalid: [unclosed").unwrap();
    drop(file);

    let result = config.reload();
    assert!(result.is_err());

    // Original config still intact
    assert_eq!(config.str("valid/yaml"), "content");

    fs::remove_file(test_file).ok();
}
