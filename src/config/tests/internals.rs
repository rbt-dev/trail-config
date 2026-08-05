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
fn get_file_invalid_template() {
    let result = get_file("config.yaml", Some("dev"));

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(_)) => (),
        _ => panic!("Expected FormatError for missing {{env}} placeholder"),
    }
}

#[test]
fn to_string_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value = get_leaf(&parsed, "db/redis/port", "/").unwrap();
    let str_value = to_string(value);

    assert_eq!(str_value, "6379");
}
