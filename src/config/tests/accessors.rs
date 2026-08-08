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
    assert!(result.unwrap());
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
    let config = Config::load_yaml(YAML, "/").unwrap();

    // Scalar should work fine
    assert!(config.str_strict("app/debug").is_ok());

    // A mapping should return an error, not Ok("")
    let result = config.str_strict("db/redis");
    assert!(result.is_err(), "Expected error for mapping, got: {:?}", result);

    // A sequence should return an error, not Ok("")
    let result = config.str_strict("app");
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
fn list_strict_errors_on_non_sequence() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    // Actual sequence should work
    assert!(config.list_strict("sources").is_ok());

    // A mapping should return an error, not Ok(vec![])
    let result = config.list_strict("app");
    assert!(result.is_err(), "Expected error for mapping, got: {:?}", result);
}

#[test]
fn list_strict_errors_on_a_non_scalar_element() {
    // Previously only the container was type-checked: a nested sequence, a nested
    // mapping and a null all came back as "", indistinguishable from an element that
    // genuinely is the empty string.
    let config = Config::load_yaml(
        "nested_seq:\n  - [1, 2]\n  - x\nnested_map:\n  - k: v\n  - x\nnulls:\n  - x\n  -\n",
        "/",
    ).unwrap();

    for (path, index) in [("nested_seq", 0), ("nested_map", 0), ("nulls", 1)] {
        match config.list_strict(path) {
            Err(ConfigError::FormatError(msg)) => {
                assert!(
                    msg.contains(&format!("{}[{}]", path, index)),
                    "message should name the offending element: {}",
                    msg
                );
            },
            other => panic!("Expected FormatError for {}, got: {:?}", path, other),
        }
    }
}

#[test]
fn list_strict_accepts_every_scalar_type_and_the_empty_string() {
    let config = Config::load_yaml("mixed:\n  - text\n  - 42\n  - 3.5\n  - true\n  - \"\"\n", "/").unwrap();

    // An element that genuinely is "" is a scalar and stays one — the value the old
    // behaviour was indistinguishable from
    assert_eq!(config.list_strict("mixed").unwrap(), ["text", "42", "3.5", "true", ""]);
}

#[test]
fn list_stays_lenient_about_elements() {
    // The lenient half is unchanged: it flattens rather than reports, like every other
    // lenient accessor
    let config = Config::load_yaml("a:\n  - [1, 2]\n  - x\n  -\n", "/").unwrap();

    assert_eq!(config.list("a"), ["", "x", ""]);
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
fn path_of_only_separators_does_not_return_the_whole_tree() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    // Every segment is empty, so there is nothing to navigate to. Skipping them
    // used to hand back the entire document.
    assert!(config.get("/").is_none());
    assert!(config.get("//").is_none());
    assert!(!config.contains("/"));
}

#[test]
fn leading_or_trailing_separator_is_rejected() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    // The same path without the stray separators resolves
    assert!(config.get("db/redis/port").is_some());

    assert!(config.get("/db/redis/port").is_none());
    assert!(config.get("db/redis/port/").is_none());
    assert!(config.get("/db/redis/port/").is_none());
}

#[test]
fn doubled_separator_is_rejected() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    assert!(config.get("db//redis/port").is_none());
    assert_eq!(config.str("db//redis/port"), "");
    assert!(!config.contains("db//redis/port"));
}

#[test]
fn empty_segment_reports_the_path_in_strict_methods() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    match config.str_strict("db//redis/port") {
        Err(ConfigError::PathNotFound(path)) => assert_eq!(path, "db//redis/port"),
        other => panic!("Expected PathNotFound, got {:?}", other),
    }
}


#[test]
fn sequence_elements_are_not_addressable_by_index() {
    // Documented design line: paths navigate mappings only. `list` is the way into
    // a sequence — a numeric segment is just a key that does not exist.
    let config = Config::load_yaml("sources:\n  - one\n  - two\n", "/").unwrap();

    assert_eq!(config.get("sources/0"), None);
    assert!(!config.contains("sources/0"));
    assert_eq!(config.str("sources/0"), "");
    assert!(matches!(
        config.get_strict("sources/0"),
        Err(ConfigError::PathNotFound(_))
    ));

    assert_eq!(config.list("sources"), vec!["one", "two"]);
}

#[test]
fn non_string_keys_have_no_path_but_still_deserialize() {
    // The other documented design line, and the one that was stated only in a code
    // comment: segments are matched as strings, so `retries/1` looks up the *string* "1"
    // and never the integer 1. Pinned here because the docs now promise the way around it.
    use std::collections::BTreeMap;

    let config = Config::load_yaml(
        "retries:\n  1: fast\n  2: slow\nflags:\n  true: on\n  false: off\n",
        "/",
    ).unwrap();

    // The parent resolves; the non-string keys under it do not
    assert!(config.contains("retries"));
    assert!(!config.contains("retries/1"));
    assert_eq!(config.str("retries/1"), "");
    assert!(!config.contains("flags/true"));

    // ...and the outline says so rather than leaving them looking absent
    assert!(config.outline().contains("retries/1: <string>  # not addressable"));

    // The documented workaround: deserialize the subtree and the keys come back typed
    let retries: BTreeMap<i64, String> = config.get_as_strict("retries").unwrap();
    assert_eq!(retries[&1], "fast");
    assert_eq!(retries[&2], "slow");

    let flags: BTreeMap<bool, String> = config.get_as_strict("flags").unwrap();
    assert_eq!(flags[&true], "on");
}

#[test]
fn a_string_key_and_its_scalar_twin_are_different_keys() {
    // Why there is no escape for reaching a non-string key, and why the accessors do not
    // fall back to one: a document can hold both, so any rule that made `mixed/1` reach
    // the integer would make the string permanently unreachable instead. The string is
    // the one that resolves, because a path segment *is* a string.
    let config = Config::load_yaml("mixed:\n  1: int-key\n  \"1\": string-key\n", "/").unwrap();

    assert_eq!(config.str("mixed/1"), "string-key");

    // Both are listed, and the marker is what tells the two lines apart
    let outline = config.outline();
    assert!(outline.contains("mixed/1: <string>  # not addressable"));
    assert!(outline.contains("mixed/1: <string>\n"));
}
