use yaml_serde::{Value, from_str, Number};
use super::{ConfigError, YAML};
use crate::config::accessor::to_string;
use crate::config::loader::get_file;
use crate::config::path::get_leaf;

#[test]
fn get_leaf_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value1 = get_leaf(&parsed, "db/redis/port", "/");
    let value2 = get_leaf(&parsed, "db/redis/username", "/");

    assert_eq!(value1, Some(&Value::Number(Number::from(6379))));
    assert_eq!(value2, None);
}

#[test]
fn get_file_test() {
    let result = get_file("config_{env}.yaml", Some("dev"));

    assert!(result.is_ok());
    let (file, env) = result.unwrap();
    assert_eq!(env, Some(String::from("dev")));
    assert_eq!(file, "config_dev.yaml");
}

#[test]
fn get_file_without_placeholder_keeps_the_environment() {
    // Not an error: in a layered setup only some files are environment-specific,
    // but the environment is still worth recording on the Config.
    let result = get_file("config.yaml", Some("dev"));

    assert!(result.is_ok(), "got {:?}", result);
    let (file, env) = result.unwrap();
    assert_eq!(file, "config.yaml");
    assert_eq!(env, Some(String::from("dev")));
}

#[test]
fn get_file_placeholder_without_environment_errors() {
    // The reverse *is* an error — a literal "config.{env}.yaml" handed to the OS
    // would come back as a missing file, pointing at the wrong problem.
    let result = get_file("config_{env}.yaml", None);

    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("{env}"), "message should name the placeholder: {}", msg);
        },
        other => panic!("Expected FormatError for unsubstituted {{env}}, got {:?}", other),
    }
}

#[test]
fn get_file_without_placeholder_or_environment_is_unchanged() {
    let result = get_file("config.yaml", None);

    assert!(result.is_ok());
    let (file, env) = result.unwrap();
    assert_eq!(file, "config.yaml");
    assert_eq!(env, None);
}

#[test]
fn to_string_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value = get_leaf(&parsed, "db/redis/port", "/").unwrap();
    let str_value = to_string(value);

    assert_eq!(str_value, "6379");
}
