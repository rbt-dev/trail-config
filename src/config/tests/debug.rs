use super::Config;
use crate::test_util::{temp_dir, write_file};

const SECRETS: &str = "
database:
  username: admin
  password: hunter2
api:
  token: sk-live-0123456789
";

#[test]
fn debug_does_not_print_config_values() {
    let config = Config::load_yaml(SECRETS, "/").unwrap();
    let printed = format!("{:?}", config);

    assert!(!printed.contains("hunter2"), "Debug leaked a password: {}", printed);
    assert!(!printed.contains("sk-live-0123456789"), "Debug leaked a token: {}", printed);
    // Keys are as revealing as values for a `Debug` that is meant to elide the document
    assert!(!printed.contains("password"), "Debug leaked the document: {}", printed);
}

#[test]
fn debug_prints_the_config_shape() {
    let config = Config::load_yaml(SECRETS, "::").unwrap();
    let printed = format!("{:?}", config);

    assert!(printed.starts_with("Config {"), "got: {}", printed);
    assert!(printed.contains("separator: \"::\""), "got: {}", printed);
    // Two top-level keys: `database` and `api`
    assert!(printed.contains("content: <2 keys>"), "got: {}", printed);
}

#[test]
fn debug_prints_filenames_and_the_overlay_chain() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let config = Config::load_required(&base, "/", Some("prod"))
        .unwrap()
        .merge_required(&overlay, Some("prod"))
        .unwrap()
        .merge_optional("absent.yaml", Some("prod"))
        .unwrap();
    let printed = format!("{:?}", config);

    // Filenames are not secrets, and the chain is what you want when a reload
    // does not do what you expected
    assert!(printed.contains("config.yaml"), "got: {}", printed);
    assert!(printed.contains("Required("), "got: {}", printed);
    assert!(printed.contains("Optional(\"absent.yaml\")"), "got: {}", printed);
    assert!(printed.contains("environment: Some(\"prod\")"), "got: {}", printed);
    assert!(printed.contains("content: <1 key>"), "got: {}", printed);
}

#[test]
fn debug_describes_a_missing_document_as_empty() {
    let dir = temp_dir();
    let missing = dir.path().join("absent.yaml").to_string_lossy().into_owned();

    let config = Config::load_optional(&missing, "/", None).unwrap();
    assert!(format!("{:?}", config).contains("content: <empty>"));
}
