use super::{Config, ConfigError};
use crate::test_util::{temp_dir, write_file};
use std::fs;

#[test]
fn reload_from_same_file() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  debug: false\n");

    let mut config = Config::load_optional(&test_file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/debug"), "false");

    fs::write(&test_file, "app:\n  port: 9090\n  debug: true\n").unwrap();

    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "true");
}

#[test]
fn reload_from_different_file() {
    let dir = temp_dir();
    let file1 = write_file(&dir, "first.yaml", "config:\n  name: first\n  value: 100\n");
    let file2 = write_file(&dir, "second.yaml", "config:\n  name: second\n  value: 200\n");

    let mut config = Config::load_optional(&file1, "/", None).unwrap();
    assert_eq!(config.str("config/name"), "first");
    assert_eq!(config.str("config/value"), "100");
    assert_eq!(config.get_filename(), file1);

    config.reload_from(&file2).unwrap();
    assert_eq!(config.str("config/name"), "second");
    assert_eq!(config.str("config/value"), "200");
    assert_eq!(config.get_filename(), file2);
}

#[test]
fn reload_preserves_separator() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "db:\n  host: localhost\n  port: 5432\n");

    let mut config = Config::load_optional(&test_file, "::", None).unwrap();
    assert_eq!(config.str("db::host"), "localhost");

    fs::write(&test_file, "db:\n  host: remote\n  port: 3306\n").unwrap();

    config.reload().unwrap();
    assert_eq!(config.str("db::host"), "remote");
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
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "valid:\n  yaml: content\n");

    let mut config = Config::load_optional(&test_file, "/", None).unwrap();

    fs::write(&test_file, "invalid: [unclosed\n").unwrap();

    let result = config.reload();
    assert!(result.is_err());

    // Original config still intact
    assert_eq!(config.str("valid/yaml"), "content");
}

#[test]
fn reload_re_applies_required_overlay() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  debug: false\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");

    // Update both files
    fs::write(&base_file, "app:\n  port: 1111\n  debug: true\n").unwrap();
    fs::write(&overlay_file, "app:\n  port: 2222\n").unwrap();

    config.reload().unwrap();

    assert_eq!(config.str("app/port"), "2222");  // overlay wins
    assert_eq!(config.str("app/debug"), "true"); // from updated base
}

#[test]
fn reload_skips_missing_optional_overlay() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_optional(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");

    // Remove the optional overlay before reload
    fs::remove_file(&overlay_file).unwrap();

    // Update base file
    fs::write(&base_file, "app:\n  port: 1111\n").unwrap();

    // Reload should succeed, falling back to base value
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "1111");
}

#[test]
fn reload_fails_if_required_overlay_deleted() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();

    fs::remove_file(&overlay_file).unwrap();

    let result = config.reload();
    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError when required overlay is deleted"),
    }

    // Original config preserved
    assert_eq!(config.str("app/port"), "9090");
}

#[test]
fn reload_from_does_not_reapply_stale_overlays() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9999\n");
    let new_file = write_file(&dir, "new.yaml", "app:\n  port: 3000\n");

    // Load base, merge overlay that overrides port to 9999
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.str("app/port"), "9999");

    // Switch to a completely different file
    config.reload_from(&new_file).unwrap();
    assert_eq!(config.str("app/port"), "3000");

    // Now reload() should NOT re-apply the old overlay on top of new_file
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "3000");
}
