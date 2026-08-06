//! The reading surface, exercised as a consumer uses it.
//!
//! Every accessor pair (lenient / strict) and the path syntax they share. These are
//! covered by unit tests too; the point of repeating them here is the vantage point —
//! this file compiles against nothing but the crate's public exports, so it also pins
//! that each of these methods *is* public and keeps its signature.

use serde::Deserialize;
use trail_config::{Config, ConfigError, Value};

const YAML: &str = r#"
app:
  name: myapp
  port: 8080
  timeout: 30.5
  debug: true
db:
  redis:
    server: 127.0.0.1
    port: 6379
sources:
  - one
  - two
empty_list: []
"#;

fn config() -> Config {
    Config::load_yaml(YAML, "/").unwrap()
}

#[test]
fn lenient_accessors_return_defaults_for_a_missing_path() {
    let config = config();

    assert_eq!(config.get("nope/nope"), None);
    assert_eq!(config.str("nope/nope"), "");
    assert_eq!(config.list("nope/nope"), Vec::<String>::new());
    assert_eq!(config.get_int("nope/nope"), None);
    assert_eq!(config.get_float("nope/nope"), None);
    assert_eq!(config.get_bool("nope/nope"), None);
    assert!(!config.contains("nope/nope"));
}

#[test]
fn strict_accessors_report_a_missing_path() {
    let config = config();

    for result in [
        config.get_strict("nope").err(),
        config.str_strict("nope").err(),
        config.list_strict("nope").err(),
        config.get_int_strict("nope").err(),
        config.get_float_strict("nope").err(),
        config.get_bool_strict("nope").err(),
    ] {
        match result {
            Some(ConfigError::PathNotFound(path)) => assert_eq!(path, "nope"),
            other => panic!("expected PathNotFound naming the path, got {:?}", other),
        }
    }
}

#[test]
fn typed_accessors_read_their_own_type_and_reject_others() {
    let config = config();

    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.get_float("app/timeout"), Some(30.5));
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "myapp");

    // A wrong type is a FormatError, not a PathNotFound — the distinction a caller
    // matches on to tell "missing" from "misconfigured"
    match config.get_int_strict("app/name") {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("expected FormatError for a string read as int, got {:?}", other),
    }
    assert_eq!(config.get_int("app/name"), None);
}

#[test]
fn sequences_are_read_whole_not_by_index() {
    let config = config();

    assert_eq!(config.list("sources"), ["one", "two"]);
    assert_eq!(config.list_strict("sources").unwrap(), ["one", "two"]);
    assert_eq!(config.list("empty_list"), Vec::<String>::new());

    // Documented: paths navigate mappings only, so `sources/0` is a lookup for a key
    // named `0` and fails like any other missing key
    assert!(!config.contains("sources/0"));
    assert_eq!(config.str("sources/0"), "");
}

#[test]
fn every_path_segment_must_be_non_empty() {
    let config = config();

    for path in ["/db/redis/port", "db/redis/port/", "db//redis/port", "/", ""] {
        assert!(!config.contains(path), "{path:?} should not resolve");
        assert_eq!(config.str(path), "", "{path:?} should not resolve");
        match config.str_strict(path) {
            Err(ConfigError::PathNotFound(named)) => assert_eq!(named, path),
            other => panic!("expected PathNotFound for {path:?}, got {:?}", other),
        }
    }

    assert_eq!(config.str("db/redis/port"), "6379");
}

#[test]
fn keys_containing_the_separator_are_reached_with_an_escape() {
    let yaml = "database:\n  \"host/port\": localhost:5432\n  \"user\\\\name\": admin\n";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.str(r"database/host\/port"), "localhost:5432");
    assert_eq!(config.str(r"database/user\\name"), "admin");
    // Doubling the separator is not the way to do it
    assert_eq!(config.str("database/host//port"), "");
}

#[test]
fn a_custom_separator_applies_to_every_path() {
    let config = Config::load_yaml(YAML, "::").unwrap();

    assert_eq!(config.get_int("db::redis::port"), Some(6379));
    assert_eq!(config.fmt("{}:{}", "db::redis", &["server", "port"]), "127.0.0.1:6379");
    // The `/` spelling is just a key name now
    assert!(!config.contains("db/redis/port"));
}

#[test]
fn an_unusable_separator_is_rejected_at_construction() {
    for sep in ["", "\\", "a\\b"] {
        match Config::load_yaml("a: 1", sep) {
            Err(ConfigError::FormatError(_)) => (),
            other => panic!("expected FormatError for separator {sep:?}, got {:?}", other.err()),
        }
    }
}

#[test]
fn raw_values_can_be_matched_on() {
    let config = config();

    assert!(matches!(config.get("app/port"), Some(Value::Number(_))));
    assert!(matches!(config.get("app/name"), Some(Value::String(_))));
    assert!(matches!(config.get("app/debug"), Some(Value::Bool(_))));
    assert!(matches!(config.get("db/redis"), Some(Value::Mapping(_))));
    assert!(matches!(config.get("sources"), Some(Value::Sequence(_))));
}

#[derive(Deserialize, Debug, PartialEq)]
struct Redis {
    server: String,
    port: u16,
}

#[test]
fn subtrees_and_whole_documents_deserialize_into_structs() {
    let config = config();

    let redis: Redis = config.get_as_strict("db/redis").unwrap();
    assert_eq!(redis, Redis { server: "127.0.0.1".to_string(), port: 6379 });
    assert_eq!(config.get_as::<Redis>("db/redis"), Some(redis));

    // A struct that does not match the document is None / an error, not a panic
    assert_eq!(config.get_as::<Redis>("app"), None);
    match config.get_as::<Redis>("nope") {
        None => (),
        other => panic!("expected None for a missing path, got {:?}", other),
    }

    #[derive(Deserialize)]
    struct Whole {
        app: App,
    }
    #[derive(Deserialize)]
    struct App {
        port: u16,
    }

    let whole: Whole = config.deserialize_strict().unwrap();
    assert_eq!(whole.app.port, 8080);
}

#[test]
fn fmt_combines_sibling_values() {
    let config = config();

    assert_eq!(config.fmt("{}:{}", "db/redis", &["server", "port"]), "127.0.0.1:6379");
    assert_eq!(
        config.fmt_strict("{{{0}:{1}}} via {0}", "db/redis", &["server", "port"]).unwrap(),
        "{127.0.0.1:6379} via 127.0.0.1"
    );

    // Lenient returns "" where strict reports
    assert_eq!(config.fmt("{}", "db/redis", &["nope"]), "");
    assert!(config.fmt_strict("{}", "db/redis", &["nope"]).is_err());
}

#[test]
fn metadata_is_readable_and_a_string_config_has_no_filename() {
    let config = config();

    assert_eq!(config.get_filename(), "");
    assert_eq!(config.environment(), None);
}

#[test]
fn outline_lists_resolvable_paths_without_values() {
    let config = Config::load_yaml("db:\n  password: hunter2\n  port: 5432\n", "/").unwrap();
    let outline = config.outline();

    assert!(!outline.contains("hunter2"), "outline leaked a value: {outline}");

    // Every line is a path the accessors resolve, spelled as they take it
    for line in outline.lines() {
        let path = line.rsplit_once(": ").expect("every line names a type").0;
        assert!(config.contains(path), "outline printed an unresolvable path: {path}");
    }
}

#[test]
fn environment_placeholders_are_resolved_in_values_but_not_in_keys() {
    // Interpolating keys would make the set of valid *paths* depend on the environment.
    // The variable here is certainly unset, so if keys were interpolated this would be
    // a FormatError instead of a config with a literal key.
    let config = Config::load_yaml("${TRAIL_CONFIG_UNSET_KEY}: 1\n", "/").unwrap();

    assert_eq!(config.get_int("${TRAIL_CONFIG_UNSET_KEY}"), Some(1));
    assert_eq!(config.outline(), "${TRAIL_CONFIG_UNSET_KEY}: <number>\n");

    // ...while the same text in a *value* position does resolve
    let config = Config::load_yaml("key: ${TRAIL_CONFIG_UNSET_KEY:-resolved}\n", "/").unwrap();
    assert_eq!(config.str("key"), "resolved");
}

#[test]
fn debug_output_never_contains_a_value() {
    // The secrets guarantee, from the outside: `{:?}` on a Config prints its shape only
    let config = Config::load_yaml("db:\n  password: hunter2\n", "/").unwrap();
    let printed = format!("{:?}", config);

    assert!(!printed.contains("hunter2"), "Debug leaked a value: {printed}");
    assert!(printed.contains("content: <1 key>"), "unexpected Debug output: {printed}");
}
