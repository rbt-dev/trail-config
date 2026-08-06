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

#[test]
fn escaped_placeholder_is_literal() {
    let _env = env_lock();
    env::set_var("TRAIL_TEST_ESCAPED", "should-not-appear");
    let yaml = "
app:
  template: $${TRAIL_TEST_ESCAPED}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("app/template"), "${TRAIL_TEST_ESCAPED}");
    env::remove_var("TRAIL_TEST_ESCAPED");
}

#[test]
fn escaped_placeholder_does_not_require_the_var_to_exist() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_NEVER_SET");
    // Escaped, so the missing variable must not be an error
    let config = Config::load_yaml("app:\n  t: $${TRAIL_TEST_NEVER_SET}", "/").unwrap();
    assert_eq!(config.str("app/t"), "${TRAIL_TEST_NEVER_SET}");
}

#[test]
fn dollar_dollar_not_before_brace_is_unchanged() {
    // Only `$${` is an escape — a password like Pa$$w0rd! must survive intact
    let config = Config::load_yaml("db:\n  password: Pa$$w0rd!", "/").unwrap();
    assert_eq!(config.str("db/password"), "Pa$$w0rd!");
}

#[test]
fn nested_default_falls_back_through_both_levels() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_OUTER");
    env::remove_var("TRAIL_TEST_INNER");
    let yaml = "
db:
  host: ${TRAIL_TEST_OUTER:-${TRAIL_TEST_INNER:-fallback}}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("db/host"), "fallback");
}

#[test]
fn nested_default_resolves_inner_variable() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_OUTER2");
    env::set_var("TRAIL_TEST_INNER2", "inner-value");
    let yaml = "
db:
  host: ${TRAIL_TEST_OUTER2:-${TRAIL_TEST_INNER2}}
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("db/host"), "inner-value");
    env::remove_var("TRAIL_TEST_INNER2");
}

/// Builds `${VAR:-${VAR:-…x…}}` nested `levels` deep.
fn nested_defaults(var: &str, levels: usize) -> String {
    let mut s = String::new();
    for _ in 0..levels {
        s.push_str(&format!("${{{}:-", var));
    }
    s.push('x');
    for _ in 0..levels {
        s.push('}');
    }
    s
}

#[test]
fn nesting_at_the_depth_limit_still_resolves() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_DEPTH_OK");
    // 32 is MAX_DEFAULT_DEPTH — far beyond any legitimate config, but allowed
    let yaml = format!("app:\n  v: {}", nested_defaults("TRAIL_TEST_DEPTH_OK", 32));
    let config = Config::load_yaml(&yaml, "/").unwrap();
    assert_eq!(config.str("app/v"), "x");
}

#[test]
fn nesting_past_the_depth_limit_errors() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_DEPTH_OVER");
    let yaml = format!("app:\n  v: {}", nested_defaults("TRAIL_TEST_DEPTH_OVER", 33));
    match Config::load_yaml(&yaml, "/") {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("depth"), "got: {}", msg);
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
}

#[test]
fn pathological_nesting_errors_instead_of_overflowing_the_stack() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_DEPTH_BOMB");
    // Without the depth cap this recurses 10_000 frames deep and aborts the process
    // with a stack overflow — not a catchable panic
    let yaml = format!("app:\n  v: {}", nested_defaults("TRAIL_TEST_DEPTH_BOMB", 10_000));
    assert!(Config::load_yaml(&yaml, "/").is_err());
}

#[test]
fn nested_placeholder_in_variable_name_errors() {
    let _env = env_lock();
    env::set_var("TRAIL_TEST_PREFIX", "APP");
    let result = Config::load_yaml("db:\n  host: ${${TRAIL_TEST_PREFIX}_HOST}", "/");
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(
                msg.contains("Nested") && msg.contains("name"),
                "message should say nesting is not allowed in the variable name: {}",
                msg
            );
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
    env::remove_var("TRAIL_TEST_PREFIX");
}

#[test]
fn unclosed_nested_placeholder_errors() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_UNCLOSED_OUTER");
    // The inner placeholder closes, the outer one never does
    let result = Config::load_yaml("db:\n  host: ${TRAIL_TEST_UNCLOSED_OUTER:-${X}", "/");
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("Unclosed"), "got: {}", msg);
        },
        other => panic!("Expected FormatError, got: {:?}", other),
    }
}

#[test]
fn unbalanced_closing_brace_in_default_ends_the_placeholder() {
    let _env = env_lock();
    env::remove_var("TRAIL_TEST_UNBALANCED");
    // Documented limitation: a bare '}' cannot appear in a default. The placeholder
    // ends at the first unmatched '}', so the rest is literal.
    let config = Config::load_yaml("app:\n  v: ${TRAIL_TEST_UNBALANCED:-a}b}", "/").unwrap();
    assert_eq!(config.str("app/v"), "ab}");
}

#[test]
fn set_but_empty_variable_does_not_fall_back_to_default() {
    let _env = env_lock();
    // Unlike shell `${VAR:-default}`, an empty *set* value is a value:
    // the default applies only when the variable is absent
    env::set_var("TRAIL_TEST_SET_EMPTY", "");
    let config = Config::load_yaml("app:\n  v: ${TRAIL_TEST_SET_EMPTY:-fallback}", "/").unwrap();
    assert_eq!(config.str("app/v"), "");
    env::remove_var("TRAIL_TEST_SET_EMPTY");
}
