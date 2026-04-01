use super::{Config, ConfigError};
use std::env;

#[test]
fn resolves_env_var() {
    env::set_var("TRAIL_TEST_HOST", "prod-server");
    let yaml = "
db:
  host: ${TRAIL_TEST_HOST}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("db/host"), "prod-server");
    env::remove_var("TRAIL_TEST_HOST");
}

#[test]
fn resolves_env_var_with_default() {
    env::remove_var("TRAIL_TEST_MISSING");
    let yaml = "
db:
  host: ${TRAIL_TEST_MISSING:-localhost}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("db/host"), "localhost");
}

#[test]
fn env_var_set_overrides_default() {
    env::set_var("TRAIL_TEST_PORT", "9090");
    let yaml = "
db:
  port: ${TRAIL_TEST_PORT:-5432}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("db/port"), "9090");
    env::remove_var("TRAIL_TEST_PORT");
}

#[test]
fn missing_env_var_no_default_errors() {
    env::remove_var("TRAIL_TEST_UNDEFINED");
    let yaml = "
db:
  host: ${TRAIL_TEST_UNDEFINED}
";
    let result = Config::load_yaml(yaml, "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("TRAIL_TEST_UNDEFINED"));
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
}

#[test]
fn unclosed_placeholder_errors() {
    let yaml = "
db:
  host: ${TRAIL_TEST_UNCLOSED
";
    let result = Config::load_yaml(yaml, "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("Unclosed"));
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
}

#[test]
fn empty_var_name_errors() {
    let yaml = "
db:
  host: ${:-default}
";
    let result = Config::load_yaml(yaml, "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("Empty"));
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
}

#[test]
fn mixed_text_and_env_vars() {
    env::set_var("TRAIL_TEST_PROTO", "https");
    env::set_var("TRAIL_TEST_DOMAIN", "example.com");
    let yaml = "
app:
  url: ${TRAIL_TEST_PROTO}://${TRAIL_TEST_DOMAIN}/api
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/url"), "https://example.com/api");
    env::remove_var("TRAIL_TEST_PROTO");
    env::remove_var("TRAIL_TEST_DOMAIN");
}

#[test]
fn env_var_in_sequence() {
    env::set_var("TRAIL_TEST_ITEM", "resolved");
    let yaml = "
items:
  - ${TRAIL_TEST_ITEM}
  - static
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.list("items"), vec!["resolved", "static"]);
    env::remove_var("TRAIL_TEST_ITEM");
}

#[test]
fn no_placeholders_unchanged() {
    let yaml = "
app:
  port: 8080
  name: myapp
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/name"), "myapp");
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn dollar_without_brace_unchanged() {
    let yaml = "
app:
  price: $100
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/price"), "$100");
}

#[test]
fn empty_default_is_valid() {
    env::remove_var("TRAIL_TEST_EMPTY_DEFAULT");
    let yaml = "
app:
  optional: ${TRAIL_TEST_EMPTY_DEFAULT:-}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/optional"), "");
}