use super::{Config, ConfigError, YAML};

#[test]
fn fmt_test() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt("{}:{}", "db/sql", &["database", "username"]);
    assert_eq!(result, "my_db:user");
}

#[test]
fn fmt_strict_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/sql", &["database", "username"]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my_db:user");
}

#[test]
fn fmt_strict_with_escaped_separator_in_path() {
    let yaml = r#"
sections:
  "db/redis":
    server: 127.0.0.1
    port: 6379
"#;
    let config = Config::load_yaml(yaml, "/").unwrap();
    let result = config.fmt_strict("{}:{}", r"sections/db\/redis", &["server", "port"]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "127.0.0.1:6379");
}

#[test]
fn fmt_strict_missing_path() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/nonexistent", &["server", "port"]);

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn fmt_strict_missing_attribute() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/redis", &["server", "nonexistent"]);

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}
