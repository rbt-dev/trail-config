use super::{Config, ConfigError};
use crate::test_util::{env_lock, temp_dir, write_file};
use std::env;

#[test]
fn resolves_env_var() {
    let _env = env_lock();
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
    let _env = env_lock();
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
    let _env = env_lock();
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
    let _env = env_lock();
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
    let _env = env_lock();
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
    let _env = env_lock();
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
fn resolved_value_with_placeholder_syntax_survives_merge() {
    let _env = env_lock();

    // The env var's *value* contains placeholder syntax — after resolution it
    // must be preserved verbatim, not re-resolved when an overlay is merged.
    env::set_var("TRAIL_TEST_LITERAL", "pa${ss}word");

    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  debug: true\n");

    let config = Config::load_yaml("db:\n  password: ${TRAIL_TEST_LITERAL}", "/").unwrap()
        .merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("db/password"), "pa${ss}word");
    assert_eq!(config.get_bool("app/debug"), Some(true));

    env::remove_var("TRAIL_TEST_LITERAL");
}

#[test]
fn reload_resolves_each_file_once() {
    let _env = env_lock();
    env::set_var("TRAIL_TEST_RELOAD_LITERAL", "se${cr}et");

    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "db:\n  password: ${TRAIL_TEST_RELOAD_LITERAL}\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  debug: true\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();
    assert_eq!(config.str("db/password"), "se${cr}et");

    // Reload must produce the same result as the original load-then-merge
    config.reload().unwrap();
    assert_eq!(config.str("db/password"), "se${cr}et");
    assert_eq!(config.get_bool("app/debug"), Some(true));

    env::remove_var("TRAIL_TEST_RELOAD_LITERAL");
}

#[test]
fn empty_default_is_valid() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_EMPTY_DEFAULT");
    let yaml = "
app:
  optional: ${TRAIL_TEST_EMPTY_DEFAULT:-}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/optional"), "");
}
