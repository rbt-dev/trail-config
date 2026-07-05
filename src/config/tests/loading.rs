use super::{Config, ConfigError, YAML};
use crate::test_util::{temp_dir, write_file};

#[test]
fn yaml_parse_error() {
    let invalid_yaml = "invalid: [unclosed";
    let result = Config::load_yaml(invalid_yaml, "/");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError"),
    }
}

#[test]
fn invalid_yaml_formats() {
    let test_cases = vec![
        "invalid: {unclosed",
        "- item1\n - item2\n- item3\n : invalid",
        ": invalid_key",
    ];

    for invalid_yaml in test_cases {
        let result = Config::load_yaml(invalid_yaml, "/");
        assert!(result.is_err(), "Expected error for: {}", invalid_yaml);

        match result {
            Err(ConfigError::YamlError { .. }) => (),
            _ => panic!("Expected YamlError for: {}", invalid_yaml),
        }
    }
}

#[test]
fn empty_yaml() {
    let result = Config::load_yaml("", "/");

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
}

#[test]
fn empty_separator_in_load_optional() {
    let result = Config::load_optional("config.yaml", "", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
}

#[test]
fn empty_separator_in_load_yaml() {
    let result = Config::load_yaml(YAML, "");

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
}

#[test]
fn load_optional_missing_file_returns_empty_config() {
    let result = Config::load_optional("nonexistent_file_12345.yaml", "/", None);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
    assert_eq!(config.str("any/path"), "");
}

#[test]
fn load_optional_invalid_yaml_returns_error() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "invalid.yaml", "invalid: [unclosed\n");

    let result = Config::load_optional(&test_file, "/", None);
    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for malformed file"),
    }
}

#[test]
fn load_or_create_creates_file_when_missing() {
    use std::fs;

    const DEFAULTS: &str = "app:\n  port: 8080\n  debug: false\n";
    let dir = temp_dir();
    let test_file = dir.path().join("new.yaml").to_string_lossy().into_owned();

    let result = Config::load_or_create(&test_file, "/", None, DEFAULTS);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(false));

    assert!(fs::metadata(&test_file).is_ok());
    let written = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written, DEFAULTS);
}

#[test]
fn load_or_create_loads_existing_file() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "existing.yaml", "app:\n  port: 9090\n");

    let config = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n").unwrap();
    assert_eq!(config.str("app/port"), "9090");
}

#[test]
fn load_or_create_invalid_defaults_returns_error() {
    let dir = temp_dir();
    let test_file = dir.path().join("invalid_defaults.yaml").to_string_lossy().into_owned();

    let result = Config::load_or_create(&test_file, "/", None, "invalid: [unclosed");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for invalid defaults"),
    }
}

#[test]
fn load_or_create_invalid_existing_file_returns_error() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    let result = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for broken existing file"),
    }
}

#[test]
fn load_required_file_not_found() {
    let result = Config::load_required("nonexistent_file_xyz.yaml", "/", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_with_env() {
    let result = Config::load_required("config_{env}.yaml", "/", Some("dev"));

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_rejects_empty_filename() {
    let config = Config::load_required("", "/", None);

    assert!(config.is_err());
    match config {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for empty filename"),
    }
}

#[test]
fn error_display_messages() {
    let io_err = ConfigError::IoError {
        file: Some("config.yaml".to_string()),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "test file not found"),
    };
    assert!(io_err.to_string().contains("IO error"));
    assert!(io_err.to_string().contains("config.yaml"));

    let yaml_err = Config::load_yaml("invalid: [unclosed", "/").unwrap_err();
    assert!(yaml_err.to_string().contains("YAML parse error"));

    let path_err = ConfigError::PathNotFound("db/missing/key".to_string());
    assert!(path_err.to_string().contains("Path not found"));

    let fmt_err = ConfigError::FormatError("invalid format".to_string());
    assert!(fmt_err.to_string().contains("Format error"));
}

#[test]
fn load_errors_carry_filename_and_source() {
    use std::error::Error as _;

    // Parse error from a file: Display names the file, source() is the parser error
    let dir = temp_dir();
    let path = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");
    let err = Config::load_required(&path, "/", None).unwrap_err();
    assert!(err.to_string().contains("broken.yaml"));
    assert!(err.source().is_some());
    match &err {
        ConfigError::YamlError { file: Some(f), .. } => assert!(f.contains("broken.yaml")),
        other => panic!("Expected YamlError with file, got: {:?}", other),
    }

    // IO error: same
    let err = Config::load_required("no_such_file_xyz.yaml", "/", None).unwrap_err();
    assert!(err.to_string().contains("no_such_file_xyz.yaml"));
    assert!(err.source().is_some());

    // Parse error from a string: no file recorded
    let err = Config::load_yaml("invalid: [unclosed", "/").unwrap_err();
    match &err {
        ConfigError::YamlError { file: None, .. } => (),
        other => panic!("Expected YamlError without file, got: {:?}", other),
    }
    assert!(err.source().is_some());
}
