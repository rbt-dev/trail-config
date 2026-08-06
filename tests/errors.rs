//! The error surface a caller is expected to match on.
//!
//! `ConfigError`'s variants, their fields, their `Display` text and their `source()`
//! chain — all of it public API that a consumer writes `match` arms against.

mod common;

use std::error::Error;
use std::io::ErrorKind;
use trail_config::{Config, ConfigError};

use common::{path_in, temp_dir, write_file};

#[test]
fn a_missing_required_file_is_an_io_error_naming_the_file() {
    let dir = temp_dir();
    let missing = path_in(&dir, "absent.yaml");

    match Config::load_required(&missing, "/", None) {
        Err(ConfigError::IoError { file, source }) => {
            assert_eq!(file.as_deref(), Some(missing.as_str()));
            assert_eq!(source.kind(), ErrorKind::NotFound);
        },
        other => panic!("expected IoError, got {:?}", other.err()),
    }
}

#[test]
fn an_empty_filename_is_rejected_as_invalid_input_not_as_missing() {
    // The distinction matters to a caller: NotFound means "the file isn't there",
    // InvalidInput means "you passed nothing"
    let cases = [
        Config::load_required("", "/", None),
        Config::load_optional("", "/", None),
        Config::load_or_create("", "/", None, "a: 1\n"),
    ];

    for result in cases {
        match result {
            Err(ConfigError::IoError { file, source }) => {
                assert_eq!(source.kind(), ErrorKind::InvalidInput);
                assert_eq!(file, None, "there is no file to name");
            },
            other => panic!("expected IoError(InvalidInput), got {:?}", other.err()),
        }
    }
}

#[test]
fn a_broken_file_is_a_parse_error_naming_the_file() {
    let dir = temp_dir();
    let file = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    match Config::load_required(&file, "/", None) {
        Err(ConfigError::YamlError { file: named, source }) => {
            assert_eq!(named.as_deref(), Some(file.as_str()));
            assert!(!source.to_string().is_empty());
        },
        other => panic!("expected YamlError, got {:?}", other.err()),
    }
}

#[test]
fn parsing_from_a_string_leaves_the_file_unnamed() {
    match Config::load_yaml("invalid: [unclosed", "/") {
        Err(ConfigError::YamlError { file, .. }) => assert_eq!(file, None),
        other => panic!("expected YamlError, got {:?}", other.err()),
    }
}

#[test]
fn display_includes_the_filename_when_there_is_one() {
    let dir = temp_dir();
    let file = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    let with_file = Config::load_required(&file, "/", None).unwrap_err().to_string();
    assert!(with_file.starts_with("YAML parse error in "), "got {with_file}");
    assert!(with_file.contains("broken.yaml"), "got {with_file}");

    let without_file = Config::load_yaml("invalid: [unclosed", "/").unwrap_err().to_string();
    assert!(without_file.starts_with("YAML parse error:"), "got {without_file}");
}

#[test]
fn the_underlying_error_is_reachable_through_source() {
    let dir = temp_dir();

    // Every load and parse error preserves its cause for error-chain reporting
    let io = Config::load_required(&path_in(&dir, "absent.yaml"), "/", None).unwrap_err();
    assert!(io.source().is_some(), "IoError should expose its io::Error");

    let parse = Config::load_yaml("invalid: [unclosed", "/").unwrap_err();
    assert!(parse.source().is_some(), "YamlError should expose its parse error");

    // The variants that carry only a message have no deeper cause
    let not_found = Config::load_yaml("a: 1", "/").unwrap().str_strict("nope").unwrap_err();
    assert!(matches!(not_found, ConfigError::PathNotFound(_)));
    assert!(not_found.source().is_none());
}

#[test]
fn path_not_found_carries_the_path_as_written() {
    let config = Config::load_yaml("a:\n  b: 1\n", "/").unwrap();

    match config.get_strict("a/b/c") {
        Err(ConfigError::PathNotFound(path)) => assert_eq!(path, "a/b/c"),
        other => panic!("expected PathNotFound, got {:?}", other.err()),
    }
}

#[test]
fn format_error_covers_wrong_types_and_bad_templates() {
    let config = Config::load_yaml("a:\n  b: 1\n", "/").unwrap();

    // Wrong type
    assert!(matches!(config.str_strict("a"), Err(ConfigError::FormatError(_))));
    assert!(matches!(config.get_bool_strict("a/b"), Err(ConfigError::FormatError(_))));
    // Bad template
    assert!(matches!(config.fmt_strict("{", "a", &["b"]), Err(ConfigError::FormatError(_))));
    // Placeholder and key counts disagreeing
    assert!(matches!(config.fmt_strict("{}", "a", &["b", "b"]), Err(ConfigError::FormatError(_))));
}

#[test]
fn an_unresolvable_env_placeholder_is_a_format_error() {
    // No environment variable is set here — `${VAR}` with no default cannot resolve,
    // and the failure surfaces at load time rather than at read time
    let result = Config::load_yaml("db:\n  password: ${TRAIL_CONFIG_SURELY_UNSET_VAR}\n", "/");

    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("TRAIL_CONFIG_SURELY_UNSET_VAR"), "message should name the variable: {msg}");
        },
        other => panic!("expected FormatError, got {:?}", other.err()),
    }

    // ...and a default makes it resolvable without touching the environment
    let config = Config::load_yaml("db:\n  host: ${TRAIL_CONFIG_SURELY_UNSET_VAR:-localhost}\n", "/").unwrap();
    assert_eq!(config.str("db/host"), "localhost");
}

#[test]
fn reload_without_a_source_file_is_a_format_error() {
    let mut config = Config::load_yaml("a: 1", "/").unwrap();

    match config.reload() {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("no file path")),
        other => panic!("expected FormatError, got {:?}", other),
    }
}

#[cfg(feature = "json")]
#[test]
fn json_parse_failures_use_the_json_variant() {
    let dir = temp_dir();
    let file = write_file(&dir, "broken.json", "{invalid json}");

    match Config::load_required(&file, "/", None) {
        Err(ConfigError::JsonError { file: named, .. }) => {
            assert_eq!(named.as_deref(), Some(file.as_str()));
        },
        other => panic!("expected JsonError, got {:?}", other.err()),
    }
}

#[cfg(feature = "toml")]
#[test]
fn toml_parse_failures_use_the_toml_variant() {
    let dir = temp_dir();
    let file = write_file(&dir, "broken.toml", "invalid = [unclosed");

    match Config::load_required(&file, "/", None) {
        Err(ConfigError::TomlError { file: named, .. }) => {
            assert_eq!(named.as_deref(), Some(file.as_str()));
        },
        other => panic!("expected TomlError, got {:?}", other.err()),
    }
}
