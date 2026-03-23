use super::{Config, ConfigError, YAML};

#[test]
fn get_int_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let port = config.get_int("db/redis/port");
    assert_eq!(port, Some(6379));

    let max_retries = config.get_int("app/max_retries");
    assert_eq!(max_retries, Some(5));
}

#[test]
fn get_int_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let value = config.get_int("db/nonexistent");
    assert_eq!(value, None);
}

#[test]
fn get_int_strict_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_int_strict("db/redis/port");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 6379);
}

#[test]
fn get_int_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_int_strict("db/nonexistent");
    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound"),
    }
}

#[test]
fn get_int_strict_wrong_type() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_int_strict("db/redis/server");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(_)) => (),
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn get_float_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let timeout = config.get_float("app/timeout");
    assert!(timeout.is_some());
    assert!((timeout.unwrap() - 2.5).abs() < 0.001);
}

#[test]
fn get_float_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let value = config.get_float("app/missing_timeout");
    assert_eq!(value, None);
}

#[test]
fn get_float_strict_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_float_strict("app/timeout");
    assert!(result.is_ok());
    assert!((result.unwrap() - 2.5).abs() < 0.001);
}

#[test]
fn get_float_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_float_strict("app/missing");
    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound"),
    }
}

#[test]
fn get_float_strict_wrong_type() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_float_strict("app/debug");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(_)) => (),
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn get_bool_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let debug = config.get_bool("app/debug");
    assert_eq!(debug, Some(true));
}

#[test]
fn get_bool_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let value = config.get_bool("app/missing_bool");
    assert_eq!(value, None);
}

#[test]
fn get_bool_strict_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_bool_strict("app/debug");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[test]
fn get_bool_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_bool_strict("app/missing");
    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound"),
    }
}

#[test]
fn get_bool_strict_wrong_type() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get_bool_strict("app/max_retries");
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(_)) => (),
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn get_strict_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.get_strict("db/redis/port");

    assert!(result.is_ok());
}

#[test]
fn get_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.get_strict("db/redis/nonexistent");

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(path)) => assert_eq!(path, "db/redis/nonexistent"),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn str_strict_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.str_strict("db/redis/port");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "6379");
}

#[test]
fn str_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.str_strict("app/nonexistent");

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn str_strict_errors_on_non_scalar() {
    let yaml = "
app:
  port: 8080
nested:
  child:
    key: value
items:
  - one
  - two
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    // Scalar should work fine
    assert!(config.str_strict("app/port").is_ok());

    // A mapping should return an error, not Ok("")
    let result = config.str_strict("nested/child");
    assert!(result.is_err(), "Expected error for mapping, got: {:?}", result);

    // A sequence should return an error, not Ok("")
    let result = config.str_strict("items");
    assert!(result.is_err(), "Expected error for sequence, got: {:?}", result);
}

#[test]
fn list_strict_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.list_strict("sources");

    assert!(result.is_ok());
    let list = result.unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0], "one");
}

#[test]
fn list_strict_not_found() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.list_strict("nonexistent/list");

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn contains_test() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    assert!(config.contains("db/redis/port"));
    assert!(config.contains("db/redis/server"));
    assert!(!config.contains("db/redis/nonexistent"));
    assert!(!config.contains("nonexistent/path"));
}

#[test]
fn empty_path() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get("");
    assert!(result.is_none());

    let result = config.str("");
    assert_eq!(result, "");

    let result = config.list("");
    assert_eq!(result.len(), 0);
}

#[test]
fn path_with_only_separator() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get("/");
    assert!(result.is_some());

    let result = config.get("//");
    assert!(result.is_some());
}

#[test]
fn path_with_leading_trailing_separator() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let result = config.get("/db/redis/port/");
    assert!(result.is_some());
}
