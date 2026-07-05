use crate::config;
use crate::test_util::{temp_dir, write_file};

#[test]
fn minimal() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = config!(&path).unwrap();
    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn with_sep() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = config!(&path, sep: "::").unwrap();
    assert_eq!(config.str("app::port"), "8080");
}

#[test]
fn with_env() {
    let dir = temp_dir();
    write_file(&dir, "config_dev.yaml", "app:\n  port: 3000\n");
    let template = dir.path().join("config_{env}.yaml").to_string_lossy().into_owned();

    let config = config!(&template, env: "dev").unwrap();
    assert_eq!(config.str("app/port"), "3000");
}

#[test]
fn with_merge() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let config = config!(&base, merge: [&overlay]).unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn full_block_syntax() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  debug: false\n  name: myapp\n");
    let required = write_file(&dir, "prod.yaml", "app:\n  debug: false\n  port: 9090\n");
    let optional = write_file(&dir, "local.yaml", "app:\n  debug: true\n");

    let config = config! {
        file: &base,
        merge: [&required],
        merge_optional: [&optional],
    }.unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn missing_file_errors() {
    let result = config!("nonexistent_macro_test.yaml");
    assert!(result.is_err());
}

#[test]
fn merge_optional_missing_is_ok() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");

    let config = config! {
        file: &base,
        merge_optional: ["nonexistent.yaml"],
    }.unwrap();

    assert_eq!(config.str("app/port"), "8080");
}
