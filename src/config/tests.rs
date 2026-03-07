use serde_yaml_bw::{Value, from_str, Number};
use super::Config;
use crate::ConfigError;

const YAML: &str = "
db:
    redis:
        server: 127.0.0.1
        port: 6379
        key_expiry: 3600
    sql:
        driver: SQL Server
        server: 127.0.0.1
        database: my_db
        username: user
        password: Pa$$w0rd!
sources:
    - one
    - two
    - three
app:
    debug: true
    max_retries: 5
    timeout: 2.5
";

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
fn fmt_test()  {
    let parsed: Config = Config::load_yaml(YAML, "/").unwrap();
    let formatted = parsed.fmt("{}:{}", "db/sql", &["database", "username"]);

    assert_eq!(formatted, String::from("my_db:user"));
}

#[test]
fn get_leaf_test()  {
    let parsed: Value = from_str(YAML).unwrap();
    let value1 = Config::get_leaf(&parsed, "db/redis/port", "/");
    let value2 = Config::get_leaf(&parsed, "db/redis/username", "/");
    
    assert_eq!(value1, Some(Value::Number(Number::from(6379), None)));
    assert_eq!(value2, None);
}

#[test]
fn get_file_test() {
    let result = Config::get_file("config_{env}.yaml", Some("dev"));

    assert!(result.is_ok());
    let (file, env) = result.unwrap();
    assert_eq!(env, Some(String::from("dev")));
    assert_eq!(file, "config_dev.yaml");
}

#[test]
fn get_file_invalid_template() {
    // env is provided but filename has no {env} placeholder
    let result = Config::get_file("config.yaml", Some("dev"));

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(_)) => (),
        _ => panic!("Expected FormatError for missing {{env}} placeholder"),
    }
}

#[test]
fn to_string_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value = Config::get_leaf(&parsed, "db/redis/port", "/").unwrap();
    let str_value = Config::to_string(&value);

    assert_eq!(str_value, "6379");
}

#[test]
fn to_list_test()  {
    let parsed: Value = from_str(YAML).unwrap();
    let value = Config::get_leaf(&parsed, "sources", "/").unwrap();
    let list = Config::to_list(&value);

    let mut vec = Vec::new();        
    vec.push(String::from("one"));
    vec.push(String::from("two"));
    vec.push(String::from("three"));

    assert_eq!(list, vec);
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
fn yaml_parse_error() {
    let invalid_yaml = "invalid: [unclosed";
    let result = Config::load_yaml(invalid_yaml, "/");
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError"),
    }
}

#[test]
fn load_optional_missing_file_returns_empty_config() {
    // Missing file is not an error for load_optional
    let result = Config::load_optional("nonexistent_file_12345.yaml", "/", None);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
    assert_eq!(config.str("any/path"), "");
}

#[test]
fn load_optional_invalid_yaml_returns_error() {
    use std::fs::{self, File};
    use std::io::Write;

    // A present-but-broken file should still surface an error
    let test_file = "test_optional_invalid.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "invalid: [unclosed").unwrap();
    drop(file);

    let result = Config::load_optional(test_file, "/", None);
    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError for malformed file"),
    }

    fs::remove_file(test_file).ok();
}

#[test]
fn load_or_create_creates_file_when_missing() {
    use std::fs;

    const DEFAULTS: &str = "app:
  port: 8080
  debug: false
";
    let test_file = "test_load_or_create_new.yaml";
    fs::remove_file(test_file).ok(); // ensure it doesn't exist

    let result = Config::load_or_create(test_file, "/", None, DEFAULTS);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(false));

    // File should have been written to disk
    assert!(fs::metadata(test_file).is_ok());
    let written = fs::read_to_string(test_file).unwrap();
    assert_eq!(written, DEFAULTS);

    fs::remove_file(test_file).ok();
}

#[test]
fn load_or_create_loads_existing_file() {
    use std::fs::{self, File};
    use std::io::Write;

    const DEFAULTS: &str = "app:
  port: 8080
";
    let test_file = "test_load_or_create_existing.yaml";

    // Write a different config to disk
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "app:
  port: 9090").unwrap();
    drop(file);

    let config = Config::load_or_create(test_file, "/", None, DEFAULTS).unwrap();

    // Should use file content, not defaults
    assert_eq!(config.str("app/port"), "9090");

    fs::remove_file(test_file).ok();
}

#[test]
fn load_or_create_invalid_defaults_returns_error() {
    use std::fs;

    let test_file = "test_load_or_create_invalid.yaml";
    fs::remove_file(test_file).ok();

    let result = Config::load_or_create(test_file, "/", None, "invalid: [unclosed");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError for invalid defaults"),
    }

    // File was written before parse attempt — clean up
    fs::remove_file(test_file).ok();
}

#[test]
fn load_or_create_invalid_existing_file_returns_error() {
    use std::fs::{self, File};
    use std::io::Write;

    let test_file = "test_load_or_create_broken.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "invalid: [unclosed").unwrap();
    drop(file);

    let result = Config::load_or_create(test_file, "/", None, "app:
  port: 8080
");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError(_)) => (),
        _ => panic!("Expected YamlError for broken existing file"),
    }

    fs::remove_file(test_file).ok();
}

#[test]
fn invalid_yaml_formats() {
    let test_cases = vec![
        "invalid: {unclosed",      // unclosed mapping
        "- item1\n - item2\n- item3\n : invalid",  // invalid key colon
        ": invalid_key",           // invalid key starting with colon
    ];

    for invalid_yaml in test_cases {
        let result = Config::load_yaml(invalid_yaml, "/");
        assert!(result.is_err(), "Expected error for: {}", invalid_yaml);
        
        match result {
            Err(ConfigError::YamlError(_)) => (),
            _ => panic!("Expected YamlError for: {}", invalid_yaml),
        }
    }
}

#[test]
fn error_display_messages() {
    // Test IoError display
    let io_err = ConfigError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "test file not found",
    ));
    assert!(io_err.to_string().contains("IO error"));

    // Test YamlError display
    let yaml_err = ConfigError::YamlError("invalid syntax".to_string());
    assert!(yaml_err.to_string().contains("YAML parse error"));

    // Test PathNotFound display
    let path_err = ConfigError::PathNotFound("db/missing/key".to_string());
    assert!(path_err.to_string().contains("Path not found"));

    // Test FormatError display
    let fmt_err = ConfigError::FormatError("invalid format".to_string());
    assert!(fmt_err.to_string().contains("Format error"));
}

#[test]
fn empty_yaml() {
    let empty_yaml = "";
    let result = Config::load_yaml(empty_yaml, "/");
    
    // Empty YAML should parse but result in empty config
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
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
fn fmt_strict_success() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/redis", &["server", "port"]);
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "127.0.0.1:6379");
}

#[test]
fn fmt_strict_with_escaped_separator_in_path() {
    let yaml = r#"
sections:
  "db/redis":
server: 127.0.0.1
port: 6379
"#;
    let config = Config::load_yaml(yaml, "/").unwrap();
    // "db/redis" is a key containing a literal slash — escape it in the base path
    let result = config.fmt_strict("{}:{}", r"sections/db\/redis", &["server", "port"]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "127.0.0.1:6379");
}
#[test]
fn fmt_strict_missing_path() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/nonexistent", &["server", "port"]);
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn fmt_strict_missing_attribute() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    let result = config.fmt_strict("{}:{}", "db/redis", &["server", "nonexistent"]);
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::PathNotFound(_)) => (),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[test]
fn empty_separator_in_load_optional() {
    let result = Config::load_optional("config.yaml", "", None);
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
}

#[test]
fn empty_separator_in_load_yaml() {
    let result = Config::load_yaml(YAML, "");
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
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
    
    // "/" is the root path - should return the root content
    let result = config.get("/");
    assert!(result.is_some());

    // "//" results in empty strings which are skipped, also returns root
    let result = config.get("//");
    assert!(result.is_some());
}

#[test]
fn path_with_leading_trailing_separator() {
    let config = Config::load_yaml(YAML, "/").unwrap();
    
    // "/db/redis/port/" should skip empty parts and find "port"
    let result = config.get("/db/redis/port/");
    assert!(result.is_some());
}

#[test]
fn load_required_file_not_found() {
    let result = Config::load_required("nonexistent_file_xyz.yaml", "/", None);
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError(_)) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_with_env() {
    // This will fail because config_dev.yaml doesn't exist, 
    // but it tests that the method attempts to load with env substitution
    let result = Config::load_required("config_{env}.yaml", "/", Some("dev"));
    
    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError(_)) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_rejects_empty_filename() {
    // load_required enforces a non-empty filename, unlike Config::load_optional
    let config = Config::load_required("", "/", None);
    
    // Empty filename is rejected with IoError
    assert!(config.is_err());
    match config {
        Err(ConfigError::IoError(_)) => (),
        _ => panic!("Expected IoError for empty filename"),
    }
}

#[test]
fn escaped_separator_in_key() {
    let yaml = "
database:
  host/port: localhost:5432
  server: db.example.com
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    
    // Access key with escaped separator
    let value = config.get("database/host\\/port");
    assert!(value.is_some());
    assert_eq!(config.str("database/host\\/port"), "localhost:5432");
}

#[test]
fn escaped_backslash_in_key() {
    let yaml = "
paths:
  'file\\path': C:\\Users\\data
  'normal': value
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    
    // Access key with escaped backslash (literal backslash in key)
    let value = config.get("paths/file\\\\path");
    assert!(value.is_some());
}

#[test]
fn mixed_escaped_and_normal_separators() {
    let yaml = "
config:
  app/version: 1.0
  'db/host:port': localhost:5432
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    
    // Key with slash requires escaping
    let value1 = config.get("config/app\\/version");
    assert!(value1.is_some());
    assert_eq!(config.str("config/app\\/version"), "1.0");
    
    // Escaped separator in second key
    let value2 = config.get("config/db\\/host:port");
    assert!(value2.is_some());
}

#[test]
fn escape_sequences_in_strict_methods() {
    let yaml = "
database:
  'user/pass': myuser/mypass
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    
    // Test that escape sequences work with strict methods too
    let result = config.get_strict("database/user\\/pass");
    assert!(result.is_ok());
    
    let result = config.str_strict("database/user\\/pass");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "myuser/mypass");
}

#[test]
fn parse_path_basic() {
    let parts = Config::parse_path("a/b/c", "/");
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn parse_path_with_escaped_separator() {
    let parts = Config::parse_path("a/b\\/c/d", "/");
    assert_eq!(parts, vec!["a", "b/c", "d"]);
}

#[test]
fn parse_path_with_escaped_backslash() {
    let parts = Config::parse_path("a/b\\\\c/d", "/");
    assert_eq!(parts, vec!["a", "b\\c", "d"]);
}

#[test]
fn parse_path_multiple_escapes() {
    let parts = Config::parse_path("a\\/b\\/c/d", "/");
    assert_eq!(parts, vec!["a/b/c", "d"]);
}

#[test]
fn parse_path_with_custom_separator() {
    let parts = Config::parse_path("a::b\\::c::d", "::");
    assert_eq!(parts, vec!["a", "b::c", "d"]);
}

#[test]
fn parse_path_escape_requires_full_separator() {
    // With separator "::", a lone "\:" should NOT be treated as an escaped separator —
    // only the full "\::" should be. Previously this was a known bug.
    let parts = Config::parse_path(r"a::b\:c::d", "::");
    // "\:" is not a valid escape, backslash kept as-is, ":" is just a literal char
    assert_eq!(parts, vec!["a", r"b\:c", "d"]);
}

#[test]
fn reload_from_same_file() {
    use std::fs::{self, File};
    use std::io::Write;
    
    // Create a temporary test file
    let test_file = "test_reload_config.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "app:\n  port: 8080\n  debug: false").unwrap();
    drop(file);
    
    // Load initial config
    let mut config = Config::load_optional(test_file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/debug"), "false");
    
    // Modify the file
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "app:\n  port: 9090\n  debug: true").unwrap();
    drop(file);
    
    // Reload the config
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "true");
    
    // Cleanup
    fs::remove_file(test_file).ok();
}

#[test]
fn reload_from_different_file() {
    use std::fs::{self, File};
    use std::io::Write;
    
    let file1 = "test_reload_file1.yaml";
    let file2 = "test_reload_file2.yaml";
    
    // Create first file
    let mut file = File::create(file1).unwrap();
    writeln!(file, "config:\n  name: first\n  value: 100").unwrap();
    drop(file);
    
    // Create second file
    let mut file = File::create(file2).unwrap();
    writeln!(file, "config:\n  name: second\n  value: 200").unwrap();
    drop(file);
    
    // Load from first file
    let mut config = Config::load_optional(file1, "/", None).unwrap();
    assert_eq!(config.str("config/name"), "first");
    assert_eq!(config.str("config/value"), "100");
    assert_eq!(config.get_filename(), file1);
    
    // Reload from second file
    config.reload_from(file2).unwrap();
    assert_eq!(config.str("config/name"), "second");
    assert_eq!(config.str("config/value"), "200");
    assert_eq!(config.get_filename(), file2);
    
    // Cleanup
    fs::remove_file(file1).ok();
    fs::remove_file(file2).ok();
}

#[test]
fn reload_preserves_separator() {
    use std::fs::{self, File};
    use std::io::Write;
    
    let test_file = "test_reload_sep.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "db:\n  host: localhost\n  port: 5432").unwrap();
    drop(file);
    
    let mut config = Config::load_optional(test_file, "::", None).unwrap();
    assert_eq!(config.str("db::host"), "localhost");
    
    // Modify file
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "db:\n  host: remote\n  port: 3306").unwrap();
    drop(file);
    
    config.reload().unwrap();
    
    // Separator should still be "::"
    assert_eq!(config.str("db::host"), "remote");
    
    // Cleanup
    fs::remove_file(test_file).ok();
}

#[test]
fn reload_from_string_config_fails() {
    let yaml = "test: value";
    let mut config = Config::load_yaml(yaml, "/").unwrap();
    
    let result = config.reload();
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("no file path"));
        },
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn reload_from_invalid_yaml_fails() {
    use std::fs::{self, File};
    use std::io::Write;
    
    let test_file = "test_reload_invalid.yaml";
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "valid:\n  yaml: content").unwrap();
    drop(file);
    
    let mut config = Config::load_optional(test_file, "/", None).unwrap();
    
    // Overwrite with invalid YAML
    let mut file = File::create(test_file).unwrap();
    writeln!(file, "invalid: [unclosed").unwrap();
    drop(file);
    
    let result = config.reload();
    assert!(result.is_err());
    
    // Original config still intact
    assert_eq!(config.str("valid/yaml"), "content");
    
    // Cleanup
    fs::remove_file(test_file).ok();
}

#[test]
fn merge_overlay_overrides_base() {
    let base = Config::load_yaml("app:
  port: 8080
  debug: false", "/").unwrap();
    let overlay = Config::load_yaml("app:
  port: 9090", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "9090");      // overridden
    assert_eq!(config.str("app/debug"), "false");    // preserved
}

#[test]
fn merge_deep_preserves_siblings() {
    let base = Config::load_yaml(
        "db:
  host: localhost
  port: 5432
  name: mydb", "/").unwrap();
    let overlay = Config::load_yaml(
        "db:
  host: prodserver", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("db/host"), "prodserver");  // overridden
    assert_eq!(config.str("db/port"), "5432");        // preserved
    assert_eq!(config.str("db/name"), "mydb");        // preserved
}

#[test]
fn merge_adds_new_keys_from_overlay() {
    let base = Config::load_yaml("app:
  port: 8080", "/").unwrap();
    let overlay = Config::load_yaml("app:
  debug: true", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn merge_replaces_sequences_wholesale() {
    let base = Config::load_yaml("features:
  - a
  - b
  - c", "/").unwrap();
    let overlay = Config::load_yaml("features:
  - x
  - y", "/").unwrap();
    let config = base.merge(overlay);

    let list = config.list("features");
    assert_eq!(list, vec!["x", "y"]);  // replaced, not appended
}

#[test]
fn merge_with_empty_overlay_is_identity() {
    let base = Config::load_yaml("app:
  port: 8080", "/").unwrap();
    let empty = Config::default();
    let config = base.merge(empty);

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_empty_base_with_overlay() {
    let base = Config::default();
    let overlay = Config::load_yaml("app:
  port: 8080", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_chaining() {
    let base    = Config::load_yaml("app:
  port: 8080
  debug: false
  name: base", "/").unwrap();
    let first   = Config::load_yaml("app:
  port: 9090", "/").unwrap();
    let second  = Config::load_yaml("app:
  debug: true", "/").unwrap();
    let config = base.merge(first).merge(second);

    assert_eq!(config.str("app/port"), "9090");          // from first overlay
    assert_eq!(config.get_bool("app/debug"), Some(true)); // from second overlay
    assert_eq!(config.str("app/name"), "base");           // preserved from base
}

#[test]
fn merge_preserves_base_separator() {
    let base    = Config::load_yaml("app:
  port: 8080", "::").unwrap();
    let overlay = Config::load_yaml("app:
  port: 9090", "/").unwrap();
    let config = base.merge(overlay);

    // Base separator "::" should be preserved
    assert_eq!(config.str("app::port"), "9090");
}

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
    // "db/redis/port" is a scalar, not a mapping — can't deserialize into a struct
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
