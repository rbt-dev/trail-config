//! YAML merge keys — `<<: *anchor` and `<<: [*a, *b]`.
//!
//! `yaml_serde` parses `<<` as an ordinary key, and the crate used to pass that through:
//! the anchored mapping sat under a literal `<<` key, merged-in paths read as missing,
//! strict deserializing failed with "missing field", and a struct with serde defaults
//! deserialized without error but held the defaults instead of the anchored values.

use super::{Config, ConfigError};
use crate::test_util::{env_lock, temp_dir, write_file};
use serde::Deserialize;
use std::fs;

const DATABASE_YML: &str = "\
defaults: &defaults
  host: db.internal
  port: 5432
development:
  <<: *defaults
  database: dev
";

#[test]
fn merged_keys_are_readable() {
    let config = Config::load_yaml(DATABASE_YML, "/").unwrap();

    assert_eq!(config.str("development/host"), "db.internal");
    assert_eq!(config.get_int("development/port"), Some(5432));
    assert_eq!(config.str("development/database"), "dev");
    assert!(!config.contains("development/<<"));
    assert_eq!(
        config.outline(),
        "defaults/host: <string>\ndefaults/port: <number>\n\
         development/host: <string>\ndevelopment/port: <number>\ndevelopment/database: <string>\n",
    );
}

#[test]
fn merged_keys_deserialize_strictly() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Db { host: String, port: u16, database: String }

    let config = Config::load_yaml(DATABASE_YML, "/").unwrap();
    let db: Db = config.get_as_strict("development").unwrap();
    assert_eq!(db, Db { host: "db.internal".into(), port: 5432, database: "dev".into() });
}

/// The quiet failure: serde defaults filled the gap, so nothing errored.
#[test]
fn merged_keys_win_over_serde_defaults() {
    fn local() -> String { "localhost".into() }
    fn other_port() -> u16 { 1 }

    #[derive(Debug, Deserialize)]
    struct Db {
        #[serde(default = "local")] host: String,
        #[serde(default = "other_port")] port: u16,
    }

    let config = Config::load_yaml(DATABASE_YML, "/").unwrap();
    let db: Db = config.get_as("development").unwrap();
    assert_eq!(db.host, "db.internal");
    assert_eq!(db.port, 5432);
}

#[test]
fn explicit_keys_win_wherever_they_appear() {
    let yaml = "\
base: &base
  a: from-base
  b: from-base
  c: from-base
before:
  a: explicit
  <<: *base
after:
  <<: *base
  c: explicit
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.str("before/a"), "explicit");
    assert_eq!(config.str("before/b"), "from-base");
    assert_eq!(config.str("after/c"), "explicit");
    assert_eq!(config.str("after/b"), "from-base");
}

/// Merged keys take the position `<<` held, in their source's order; explicit keys keep
/// theirs. Upstream `apply_merge` turned `<<, a, c, y` into `y, a, c, x`.
#[test]
fn merged_keys_take_the_position_of_the_merge_key() {
    let yaml = "\
src: &src
  x: 1
  y: 2
first:
  <<: *src
  a: 1
  c: 1
  y: 9
middle:
  a: 1
  <<: *src
  c: 1
last:
  a: 1
  c: 1
  <<: *src
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    let keys = |path: &str| -> Vec<String> {
        config.get(path).unwrap().as_mapping().unwrap()
            .keys().map(|k| k.as_str().unwrap().to_string()).collect()
    };

    assert_eq!(keys("first"), ["x", "a", "c", "y"]);
    assert_eq!(config.get_int("first/y"), Some(9));
    assert_eq!(keys("middle"), ["a", "x", "y", "c"]);
    assert_eq!(keys("last"), ["a", "c", "x", "y"]);
}

#[test]
fn earlier_sources_in_a_merge_list_win() {
    let yaml = "\
a: &a
  shared: from-a
  only_a: 1
b: &b
  shared: from-b
  only_b: 2
merged:
  <<: [*a, *b]
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.str("merged/shared"), "from-a");
    assert_eq!(config.get_int("merged/only_a"), Some(1));
    assert_eq!(config.get_int("merged/only_b"), Some(2));
    assert_eq!(
        config.outline(),
        "a/shared: <string>\na/only_a: <number>\nb/shared: <string>\nb/only_b: <number>\n\
         merged/shared: <string>\nmerged/only_a: <number>\nmerged/only_b: <number>\n",
    );
}

/// An anchored mapping that itself merges another. Resolving top-down — as upstream
/// `apply_merge` does — copied the inner `<<` in unresolved.
#[test]
fn chained_merges_resolve_fully() {
    let yaml = "\
base: &base
  timeout: 30
service: &service
  <<: *base
  retries: 3
api:
  <<: *service
  name: api
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.get_int("api/timeout"), Some(30));
    assert_eq!(config.get_int("api/retries"), Some(3));
    assert!(!config.contains("api/<<"));
    assert!(!config.contains("service/<<"));
}

#[test]
fn merges_resolve_inside_sequences_and_tagged_values() {
    let yaml = "\
defaults: &defaults
  port: 80
servers:
  - <<: *defaults
    name: one
tagged: !Server
  <<: *defaults
  name: two
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    #[derive(Debug, Deserialize, PartialEq)]
    struct Server { name: String, port: u16 }
    let servers: Vec<Server> = config.get_as_strict("servers").unwrap();
    assert_eq!(servers, [Server { name: "one".into(), port: 80 }]);
    assert_eq!(config.get_int("tagged/port"), Some(80));
}

#[test]
fn merged_values_are_interpolated() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_MERGE_HOST", "from-env");

    let yaml = "\
defaults: &defaults
  host: ${TRAIL_TEST_MERGE_HOST}
production:
  <<: *defaults
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    assert_eq!(config.str("production/host"), "from-env");
}

/// An overlay's `<<` is resolved within the overlay before the deep merge, rather than
/// being deep-merged as an ordinary key.
#[test]
fn merge_keys_resolve_in_overlays() {
    let dir = temp_dir();
    let overlay = write_file(&dir, "local.yaml", "\
shared: &shared
  host: overlay-host
db:
  <<: *shared
");
    let base = Config::load_yaml("db:\n  host: base-host\n  port: 5432\n", "/").unwrap();
    let config = base.merge_required(&overlay, None).unwrap();

    assert_eq!(config.str("db/host"), "overlay-host");
    assert_eq!(config.get_int("db/port"), Some(5432));
    assert!(!config.contains("db/<<"));
}

#[test]
fn reload_resolves_merge_keys() {
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 1\n");
    let mut config = Config::load_required(&file, "/", None).unwrap();

    fs::write(&file, DATABASE_YML).unwrap();
    config.reload().unwrap();

    assert_eq!(config.str("development/host"), "db.internal");
    assert!(!config.contains("development/<<"));
}

#[test]
fn documents_without_merge_keys_are_unchanged() {
    let config = Config::load_yaml("a: 1\nb:\n  c: 2\n", "/").unwrap();
    assert_eq!(config.outline(), "a: <number>\nb/c: <number>\n");
}

fn merge_error(yaml: &str) -> String {
    match Config::load_yaml(yaml, "/") {
        Err(ConfigError::YamlError { file: None, source, .. }) => source.to_string(),
        other => panic!("expected a YamlError, got {other:?}"),
    }
}

#[test]
fn a_scalar_merge_value_is_an_error() {
    assert_eq!(
        merge_error("outer:\n  inner:\n    <<: not-a-mapping\n"),
        "merge key `<<` under `outer` > `inner`: \
         expected a mapping or list of mappings for merging, but found scalar",
    );
    assert_eq!(
        merge_error("<<:\n"),
        "merge key `<<` at the document root: \
         expected a mapping or list of mappings for merging, but found scalar",
    );
}

#[test]
fn a_bad_merge_list_element_is_an_error() {
    assert_eq!(
        merge_error("list:\n  - x:\n      <<: [1]\n"),
        "merge key `<<` under `list` > `[0]` > `x`: \
         expected a mapping for merging, but found scalar",
    );
    assert_eq!(
        merge_error("a:\n  <<: [[]]\n"),
        "merge key `<<` under `a`: expected a mapping for merging, but found sequence",
    );
}

#[test]
fn a_tagged_merge_value_is_an_error() {
    assert_eq!(
        merge_error("a:\n  <<: !Tag {x: 1}\n"),
        "merge key `<<` under `a`: unexpected tagged value in merge",
    );
    assert_eq!(
        merge_error("a:\n  <<: [!Tag {x: 1}]\n"),
        "merge key `<<` under `a`: unexpected tagged value in merge",
    );
}

/// Errors name the key path, never the value that was found.
#[test]
fn merge_errors_do_not_echo_values() {
    let message = merge_error("a:\n  <<: s3cr3t-value\n");
    assert!(!message.contains("s3cr3t"), "{message}");
}

#[test]
fn merge_errors_name_the_file() {
    let dir = temp_dir();
    let file = write_file(&dir, "bad.yaml", "a:\n  <<: 1\n");
    match Config::load_required(&file, "/", None) {
        Err(ConfigError::YamlError { file: Some(named), .. }) => assert_eq!(named, file),
        other => panic!("expected a YamlError naming the file, got {other:?}"),
    }
}

/// The parsed document no longer records whether `<<` was quoted, so a quoted key merges
/// too — as documented in MERGING.md, and as upstream `apply_merge` does.
#[test]
fn a_quoted_merge_key_also_merges() {
    let config = Config::load_yaml("a: &a\n  x: 1\nb:\n  \"<<\": *a\n", "/").unwrap();
    assert_eq!(config.get_int("b/x"), Some(1));
    assert!(!config.contains("b/<<"));
}

/// Merge keys are YAML's: in JSON, `<<` is an ordinary key like any other.
#[test]
#[cfg(feature = "json")]
fn json_treats_the_merge_key_literally() {
    let config = Config::load_json(r#"{"a": {"<<": {"x": 1}}}"#, "/").unwrap();
    assert!(config.contains("a/<<"));
    assert!(!config.contains("a/x"));
}
