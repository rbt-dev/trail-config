use super::{Config, ConfigError};
use crate::test_util::{temp_dir, write_file};

#[test]
fn merge_required_overlay_overrides_base() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_required_deep_preserves_siblings() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "db:\n  host: prodserver\n");

    let base = Config::load_yaml("db:\n  host: localhost\n  port: 5432\n  name: mydb", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("db/host"), "prodserver");
    assert_eq!(config.str("db/port"), "5432");
    assert_eq!(config.str("db/name"), "mydb");
}

#[test]
fn merge_required_adds_new_keys() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  debug: true\n");

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn merge_required_replaces_sequences_wholesale() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "features:\n  - x\n  - y\n");

    let base = Config::load_yaml("features:\n  - a\n  - b\n  - c", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    let list = config.list("features");
    assert_eq!(list, vec!["x", "y"]);
}

#[test]
fn merge_required_missing_file_returns_error() {
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_required("nonexistent_overlay_xyz.yaml", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing required overlay"),
    }
}

#[test]
fn merge_optional_missing_file_is_identity() {
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge_optional("nonexistent_overlay_xyz.yaml", None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_optional_present_file_overrides() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_optional_invalid_yaml_returns_error() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "invalid: [unclosed\n");

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_optional(&overlay_file, None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for invalid optional overlay"),
    }
}

#[test]
fn merge_chaining() {
    let dir = temp_dir();
    let file1 = write_file(&dir, "chain1.yaml", "app:\n  port: 9090\n");
    let file2 = write_file(&dir, "chain2.yaml", "app:\n  debug: true\n");

    let config = Config::load_yaml("app:\n  port: 8080\n  debug: false\n  name: base", "/").unwrap()
        .merge_required(&file1, None).unwrap()
        .merge_required(&file2, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "base");
}

#[test]
fn merge_preserves_base_separator() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080", "::").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app::port"), "9090");
}

#[test]
fn merge_required_with_env_substitution() {
    let dir = temp_dir();
    write_file(&dir, "overlay_prod.yaml", "app:\n  port: 9090\n");
    let template = dir.path().join("overlay_{env}.yaml").to_string_lossy().into_owned();

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required(&template, Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_optional_with_env_substitution() {
    let dir = temp_dir();
    write_file(&dir, "overlay_prod.yaml", "app:\n  debug: true\n");
    let template = dir.path().join("overlay_{env}.yaml").to_string_lossy().into_owned();

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional(&template, Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}
