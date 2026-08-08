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

    match result {
        // Not a parse error: the document parsed, and this is a mismatch between it and
        // the requested type. The path is carried so the message says which subtree.
        Err(ConfigError::DeserializeError { path, file, .. }) => {
            assert_eq!(path.as_deref(), Some("db/redis/port"));
            assert_eq!(file, None, "a string config has no file to name");
        },
        other => panic!("Expected DeserializeError, got {:?}", other),
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

    assert!(app.debug);
    assert_eq!(app.max_retries, 5);
    assert!((app.timeout - 2.5).abs() < 0.001);
}

#[test]
fn deserialize_strict_full_config() {
    #[derive(serde::Deserialize, Debug)]
    struct FullConfig {
        db: DbConfig,
        app: AppConfig,
    }
    #[derive(serde::Deserialize, Debug)]
    struct DbConfig {
        redis: RedisConfig,
    }
    #[derive(serde::Deserialize, Debug)]
    struct RedisConfig {
        server: String,
        port: u16,
        key_expiry: u32,
    }
    #[derive(serde::Deserialize, Debug)]
    struct AppConfig {
        debug: bool,
        max_retries: i32,
        timeout: f64,
    }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let full: FullConfig = config.deserialize_strict().unwrap();

    assert_eq!(full.db.redis.server, "127.0.0.1");
    assert_eq!(full.db.redis.port, 6379);
    assert_eq!(full.db.redis.key_expiry, 3600);
    assert!(full.app.debug);
    assert_eq!(full.app.max_retries, 5);
    assert!((full.app.timeout - 2.5).abs() < 0.001);
}

#[test]
fn deserialize_strict_type_mismatch() {
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Wrong { totally_made_up_field: String }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.deserialize_strict::<Wrong>();

    match result {
        Err(ConfigError::DeserializeError { path, .. }) => {
            assert_eq!(path, None, "the whole document has no subtree path");
        },
        other => panic!("Expected DeserializeError, got {:?}", other),
    }
}

#[test]
fn deserialize_lenient_returns_none_on_mismatch() {
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct Wrong { totally_made_up_field: String }

    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.deserialize::<Wrong>();

    assert!(result.is_none());
}

#[test]
fn deserialize_strict_empty_config() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct NonEmpty { required_field: String }

    let config = Config::load_yaml("", "/").unwrap();
    let result = config.deserialize_strict::<NonEmpty>();

    match result {
        Err(ConfigError::DeserializeError { .. }) => (),
        other => panic!("Expected DeserializeError for empty config, got {:?}", other),
    }
}

#[test]
fn deserialize_error_names_the_file_it_came_from() {
    use crate::test_util::{temp_dir, write_file};

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Wrong { totally_made_up_field: String }

    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let config = Config::load_required(&file, "/", None).unwrap();

    let message = config.deserialize_strict::<Wrong>().unwrap_err().to_string();

    // The point of the dedicated variant: this used to read "YAML parse error", which
    // named a format the caller may not be using and a phase that had already succeeded
    assert!(message.starts_with("Cannot deserialize "), "got {}", message);
    assert!(message.contains("config.yaml"), "got {}", message);
}