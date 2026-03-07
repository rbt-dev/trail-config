use super::{Config, ConfigError};

#[test]
fn merge_required_overlay_overrides_base() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_req_override.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  port: 9090").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required(overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_required_deep_preserves_siblings() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_req_siblings.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "db:\n  host: prodserver").unwrap();
    drop(file);

    let base = Config::load_yaml("db:\n  host: localhost\n  port: 5432\n  name: mydb", "/").unwrap();
    let config = base.merge_required(overlay_file, None).unwrap();

    assert_eq!(config.str("db/host"), "prodserver");
    assert_eq!(config.str("db/port"), "5432");
    assert_eq!(config.str("db/name"), "mydb");

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_required_adds_new_keys() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_req_new_keys.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  debug: true").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge_required(overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_required_replaces_sequences_wholesale() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_req_seq.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "features:\n  - x\n  - y").unwrap();
    drop(file);

    let base = Config::load_yaml("features:\n  - a\n  - b\n  - c", "/").unwrap();
    let config = base.merge_required(overlay_file, None).unwrap();

    let list = config.list("features");
    assert_eq!(list, vec!["x", "y"]);

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_required_missing_file_returns_error() {
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_required("nonexistent_overlay_xyz.yaml", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError(_)) => (),
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
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_opt_override.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  port: 9090").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional(overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_optional_invalid_yaml_returns_error() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_opt_invalid.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "invalid: [unclosed").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_optional(overlay_file, None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError for invalid optional overlay"),
    }

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_chaining() {
    use std::fs::{self, File};
    use std::io::Write;

    let file1 = "test_merge_chain_1.yaml";
    let file2 = "test_merge_chain_2.yaml";

    let mut f = File::create(file1).unwrap();
    writeln!(f, "app:\n  port: 9090").unwrap();
    drop(f);

    let mut f = File::create(file2).unwrap();
    writeln!(f, "app:\n  debug: true").unwrap();
    drop(f);

    let config = Config::load_yaml("app:\n  port: 8080\n  debug: false\n  name: base", "/").unwrap()
        .merge_required(file1, None).unwrap()
        .merge_required(file2, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "base");

    fs::remove_file(file1).ok();
    fs::remove_file(file2).ok();
}

#[test]
fn merge_preserves_base_separator() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_sep.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  port: 9090").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080", "::").unwrap();
    let config = base.merge_required(overlay_file, None).unwrap();

    assert_eq!(config.str("app::port"), "9090");

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_required_with_env_substitution() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_env_prod.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  port: 9090").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required("test_merge_env_{env}.yaml", Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");

    fs::remove_file(overlay_file).ok();
}

#[test]
fn merge_optional_with_env_substitution() {
    use std::fs::{self, File};
    use std::io::Write;

    let overlay_file = "test_merge_opt_env_prod.yaml";
    let mut file = File::create(overlay_file).unwrap();
    writeln!(file, "app:\n  debug: true").unwrap();
    drop(file);

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional("test_merge_opt_env_{env}.yaml", Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));

    fs::remove_file(overlay_file).ok();
}
