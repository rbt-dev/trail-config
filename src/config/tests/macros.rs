use std::fs::{self, File};
use std::io::Write;
use crate::config;

#[test]
fn minimal() {
    let path = "test_macro_minimal.yaml";
    let mut f = File::create(path).unwrap();
    writeln!(f, "app:\n  port: 8080").unwrap();
    drop(f);

    let config = config!(path).unwrap();
    assert_eq!(config.str("app/port"), "8080");

    fs::remove_file(path).ok();
}

#[test]
fn with_sep() {
    let path = "test_macro_sep.yaml";
    let mut f = File::create(path).unwrap();
    writeln!(f, "app:\n  port: 8080").unwrap();
    drop(f);

    let config = config!(path, sep: "::").unwrap();
    assert_eq!(config.str("app::port"), "8080");

    fs::remove_file(path).ok();
}

#[test]
fn with_env() {
    let path = "test_macro_{env}.yaml";
    let actual = "test_macro_dev.yaml";
    let mut f = File::create(actual).unwrap();
    writeln!(f, "app:\n  port: 3000").unwrap();
    drop(f);

    let config = config!(path, env: "dev").unwrap();
    assert_eq!(config.str("app/port"), "3000");

    fs::remove_file(actual).ok();
}

#[test]
fn with_merge() {
    let base = "test_macro_merge_base.yaml";
    let overlay = "test_macro_merge_overlay.yaml";

    let mut f = File::create(base).unwrap();
    writeln!(f, "app:\n  port: 8080\n  name: myapp").unwrap();
    drop(f);

    let mut f = File::create(overlay).unwrap();
    writeln!(f, "app:\n  port: 9090").unwrap();
    drop(f);

    let config = config!(base, merge: [overlay]).unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(base).ok();
    fs::remove_file(overlay).ok();
}

#[test]
fn full_block_syntax() {
    let base = "test_macro_full_base.yaml";
    let required = "test_macro_full_prod.yaml";
    let optional = "test_macro_full_local.yaml";

    let mut f = File::create(base).unwrap();
    writeln!(f, "app:\n  port: 8080\n  debug: false\n  name: myapp").unwrap();
    drop(f);

    let mut f = File::create(required).unwrap();
    writeln!(f, "app:\n  debug: false\n  port: 9090").unwrap();
    drop(f);

    let mut f = File::create(optional).unwrap();
    writeln!(f, "app:\n  debug: true").unwrap();
    drop(f);

    let config = config! {
        file: base,
        merge: [required],
        merge_optional: [optional],
    }.unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "myapp");

    fs::remove_file(base).ok();
    fs::remove_file(required).ok();
    fs::remove_file(optional).ok();
}

#[test]
fn missing_file_errors() {
    let result = config!("nonexistent_macro_test.yaml");
    assert!(result.is_err());
}

#[test]
fn merge_optional_missing_is_ok() {
    let base = "test_macro_opt_missing_base.yaml";
    let mut f = File::create(base).unwrap();
    writeln!(f, "app:\n  port: 8080").unwrap();
    drop(f);

    let config = config! {
        file: base,
        merge_optional: ["nonexistent.yaml"],
    }.unwrap();

    assert_eq!(config.str("app/port"), "8080");

    fs::remove_file(base).ok();
}