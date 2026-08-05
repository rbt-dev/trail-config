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

const BRACES: &str = r#"
tpl:
  a: "x{}y"
  b: "B"
  c: "C"
"#;

#[test]
fn fmt_strict_does_not_substitute_into_substituted_values() {
    // A value that itself contains "{}" must not become a placeholder for the
    // next key. Repeated `replacen` over the accumulating result did exactly that.
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{}-{}", "tpl", &["a", "b"]).unwrap();
    assert_eq!(result, "x{}y-B");
}

#[test]
fn fmt_strict_errors_on_more_placeholders_than_keys() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{}-{}-{}", "tpl", &["a", "b"]);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_strict_errors_on_more_keys_than_placeholders() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{}", "tpl", &["a", "b", "c"]);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_strict_escapes_literal_braces() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{{{}}}", "tpl", &["b"]).unwrap();
    assert_eq!(result, "{B}");
}

#[test]
fn fmt_strict_literal_braces_without_placeholders() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{{}}", "tpl", &[]).unwrap();
    assert_eq!(result, "{}");
}

#[test]
fn fmt_strict_indexed_placeholders() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    // Indices allow reordering and reuse, which auto-numbering cannot express
    let result = config.fmt_strict("{1}/{0}/{1}", "tpl", &["b", "c"]).unwrap();
    assert_eq!(result, "C/B/C");
}

#[test]
fn fmt_strict_errors_on_index_out_of_range() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{5}", "tpl", &["b"]);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_strict_errors_on_unclosed_brace() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{", "tpl", &["b"]);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_strict_errors_on_unmatched_closing_brace() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("}", "tpl", &["b"]);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_strict_errors_on_named_placeholder() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    let result = config.fmt_strict("{name}", "tpl", &["b"]);
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("name"), "message should name the placeholder: {}", msg);
        },
        other => panic!("Expected FormatError, got {:?}", other),
    }
}

#[test]
fn fmt_lenient_returns_empty_on_mismatch() {
    let config = Config::load_yaml(BRACES, "/").unwrap();
    // The lenient variant swallows the error; the point is that it no longer
    // returns a half-formatted string
    assert_eq!(config.fmt("{}-{}-{}", "tpl", &["a", "b"]), "");
    assert_eq!(config.fmt("{}", "tpl", &["a", "b"]), "");
}
