use super::{Config, ConfigError, YAML};

#[test]
fn get_as_strict_success() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct RedisConfig {
        server: String,
        port: u16,
        key_expiry: u32,
    }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let redis: RedisConfig = config.get_as_strict("db/redis").unwrap();

    assert_eq!(redis.server, "127.0.0.1");
    assert_eq!(redis.port, 6379);
    assert_eq!(redis.key_expiry, 3600);
}

#[test]
fn get_as_strict_path_not_found() {
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Dummy { value: String }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.get_as_strict::<Dummy>("db/nonexistent");

    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound"),
    }
}

#[test]
fn get_as_strict_type_mismatch() {
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Wrong { totally_made_up_field: String }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.get_as_strict::<Wrong>("db/redis/port");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError"),
    }
}

#[test]
fn get_as_lenient_returns_none_on_missing() {
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Dummy { value: String }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.get_as::<Dummy>("nonexistent/path");

    assert!(result.is_none());
}

#[test]
fn get_as_nested_struct() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct AppConfig {
        debug: bool,
        max_retries: i32,
        timeout: f64,
    }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let app: AppConfig = config.get_as_strict("app").unwrap();

    assert_eq!(app.debug, true);
    assert_eq!(app.max_retries, 5);
    assert!((app.timeout - 2.5).abs() < 0.001);
}
