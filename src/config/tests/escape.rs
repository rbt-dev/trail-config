use super::Config;
use crate::config::path::parse_path;

#[test]
fn escaped_separator_in_key() {
    let yaml = "
database:
  host/port: localhost:5432
  server: db.example.com
";
    let config = Config::load_yaml(yaml, "/").unwrap();

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

    let value1 = config.get("config/app\\/version");
    assert!(value1.is_some());
    assert_eq!(config.str("config/app\\/version"), "1.0");

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

    let result = config.get_strict("database/user\\/pass");
    assert!(result.is_ok());

    let result = config.str_strict("database/user\\/pass");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "myuser/mypass");
}

#[test]
fn multibyte_separator_does_not_panic() {
    // '→' is a 3-byte UTF-8 character (E2 86 92).
    // parse_path uses &separator[1..] which is a byte slice —
    // slicing at byte 1 lands inside the multi-byte character and panics.
    let yaml = "
database:
  port: 1234
";
    let config = Config::load_yaml(yaml, "→").unwrap();
    assert_eq!(config.str("database→port"), "1234");
}

#[test]
fn parse_path_basic() {
    let parts = parse_path("a/b/c", "/");
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn parse_path_with_escaped_separator() {
    let parts = parse_path("a/b\\/c/d", "/");
    assert_eq!(parts, vec!["a", "b/c", "d"]);
}

#[test]
fn parse_path_with_escaped_backslash() {
    let parts = parse_path("a/b\\\\c/d", "/");
    assert_eq!(parts, vec!["a", "b\\c", "d"]);
}

#[test]
fn parse_path_multiple_escapes() {
    let parts = parse_path("a\\/b\\/c/d", "/");
    assert_eq!(parts, vec!["a/b/c", "d"]);
}

#[test]
fn parse_path_with_custom_separator() {
    let parts = parse_path("a::b\\::c::d", "::");
    assert_eq!(parts, vec!["a", "b::c", "d"]);
}

#[test]
fn parse_path_escape_requires_full_separator() {
    let parts = parse_path(r"a::b\:c::d", "::");
    assert_eq!(parts, vec!["a", r"b\:c", "d"]);
}

#[test]
fn parse_path_trailing_backslash_is_literal() {
    // A backslash with nothing left to escape stays as itself
    let parts = parse_path(r"a/b\", "/");
    assert_eq!(parts, vec!["a", r"b\"]);
}

#[test]
fn parse_path_backslash_before_ordinary_char_is_literal() {
    let parts = parse_path(r"a/b\c", "/");
    assert_eq!(parts, vec!["a", r"b\c"]);
}

#[test]
fn parse_path_empty_segments_are_preserved() {
    // Leading, doubled and trailing separators all yield empty segments here;
    // `get_leaf` is what skips them when navigating.
    let parts = parse_path("/a//b/", "/");
    assert_eq!(parts, vec!["", "a", "", "b", ""]);
}

#[test]
fn parse_path_multibyte_separator() {
    let parts = parse_path("a→b\\→c→d", "→");
    assert_eq!(parts, vec!["a", "b→c", "d"]);
}

#[test]
fn parse_path_empty_separator_terminates() {
    // Unreachable through the public API — `check_separator` rejects an empty
    // separator at construction — but must not loop forever if it is reached.
    let parts = parse_path("a/b", "");
    assert_eq!(parts, vec!["a/b"]);
}
