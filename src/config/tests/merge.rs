use super::{Config, ConfigError};
use crate::test_util::{temp_dir, write_file};

#[test]
fn merge_required_overlay_overrides_base() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_required_deep_preserves_siblings() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "db:\n  host: prodserver\n");

    let base = Config::load_yaml("db:\n  host: localhost\n  port: 5432\n  name: mydb", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("db/host"), "prodserver");
    assert_eq!(config.str("db/port"), "5432");
    assert_eq!(config.str("db/name"), "mydb");
}

#[test]
fn merge_required_adds_new_keys() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  debug: true\n");

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn merge_required_replaces_sequences_wholesale() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "features:\n  - x\n  - y\n");

    let base = Config::load_yaml("features:\n  - a\n  - b\n  - c", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    let list = config.list("features");
    assert_eq!(list, vec!["x", "y"]);
}

#[test]
fn merge_required_missing_file_returns_error() {
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_required("nonexistent_overlay_xyz.yaml", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing required overlay"),
    }
}

#[test]
fn merge_optional_missing_file_is_identity() {
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge_optional("nonexistent_overlay_xyz.yaml", None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_optional_present_file_overrides() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_optional_invalid_yaml_returns_error() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "invalid: [unclosed\n");

    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_optional(&overlay_file, None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for invalid optional overlay"),
    }
}

#[test]
fn merge_chaining() {
    let dir = temp_dir();
    let file1 = write_file(&dir, "chain1.yaml", "app:\n  port: 9090\n");
    let file2 = write_file(&dir, "chain2.yaml", "app:\n  debug: true\n");

    let config = Config::load_yaml("app:\n  port: 8080\n  debug: false\n  name: base", "/").unwrap()
        .merge_required(&file1, None).unwrap()
        .merge_required(&file2, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "base");
}

#[test]
fn merge_preserves_base_separator() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let base = Config::load_yaml("app:\n  port: 8080", "::").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app::port"), "9090");
}

#[test]
fn merge_required_with_env_substitution() {
    let dir = temp_dir();
    write_file(&dir, "overlay_prod.yaml", "app:\n  port: 9090\n");
    let template = dir.path().join("overlay_{env}.yaml").to_string_lossy().into_owned();

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_required(&template, Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn merge_optional_with_env_substitution() {
    let dir = temp_dir();
    write_file(&dir, "overlay_prod.yaml", "app:\n  debug: true\n");
    let template = dir.path().join("overlay_{env}.yaml").to_string_lossy().into_owned();

    let base = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let config = base.merge_optional(&template, Some("prod")).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn merge_required_rejects_empty_filename() {
    // Previously reached the OS and failed with `IO error in : ...` — the misleading
    // message the InvalidInput guard exists to prevent
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_required("", None);

    match result {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename, got {:?}", result),
    }
}

#[test]
fn merge_optional_rejects_empty_filename() {
    // Previously this silently no-opped: reading an empty path yields NotFound,
    // which is exactly the case merge_optional is designed to ignore
    let base = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let result = base.merge_optional("", None);

    match result {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename, got {:?}", result),
    }
}

#[test]
fn merge_null_overlay_value_clears_base_value() {
    let dir = temp_dir();
    // YAML's bare `key:` form — the way a null is written by accident as often as
    // on purpose. Previously discarded, leaving the base credential in place.
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  token:\n");

    let base = Config::load_yaml("app:\n  name: real\n  token: secret\n  retries: 3", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/token"), "");
    assert_eq!(config.get("app/token"), Some(yaml_serde::Value::Null));
    // The key is cleared, not removed — it is present holding a null
    assert!(config.contains("app/token"));
    // Siblings untouched
    assert_eq!(config.str("app/name"), "real");
    assert_eq!(config.get_int("app/retries"), Some(3));
}

#[test]
fn merge_explicit_null_overlay_value_clears_base_value() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  token: null\n");

    let base = Config::load_yaml("app:\n  token: secret", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/token"), "");
}

#[test]
fn merge_null_overlay_clears_whole_subtree() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\nkeep: 1\n");

    let base = Config::load_yaml("app:\n  name: real\n  token: secret\nkeep: 0", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/name"), "");
    assert_eq!(config.str("app/token"), "");
    assert!(!config.contains("app/name"));
    assert_eq!(config.get_int("keep"), Some(1));
}

#[test]
fn merge_empty_overlay_document_is_a_no_op() {
    let dir = temp_dir();
    // An empty document parses to the same `Value::Null` as a cleared key, but at the
    // document level it means "nothing to overlay" — the distinction `merge_documents`
    // exists to draw. A comment-only file is the same case.
    let empty = write_file(&dir, "empty.yaml", "");
    let comments = write_file(&dir, "comments.yaml", "# nothing here\n");

    let config = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap()
        .merge_required(&empty, None).unwrap()
        .merge_optional(&comments, None).unwrap();

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(false));
}

#[test]
fn reload_reapplies_null_clearing_overlay() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  token: secret\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  token:\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();
    assert_eq!(config.str("app/token"), "");

    // The clear must survive a reload, which rebuilds the merge from scratch
    config.reload().unwrap();
    assert_eq!(config.str("app/token"), "");
}

/// Top-level key order of a config, as seen by a caller deserializing the whole
/// document into an order-preserving type.
fn key_order(config: &Config) -> Vec<String> {
    let map: yaml_serde::Mapping = config.deserialize_strict().unwrap();
    map.keys().map(|k| k.as_str().unwrap().to_string()).collect()
}

#[test]
fn merge_preserves_base_key_order() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "a: 9\n");

    // `Mapping::remove` is `swap_remove`: overriding `a` used to move `c` into its
    // slot and append `a` at the end, giving c, b, a
    let base = Config::load_yaml("a: 1\nb: 2\nc: 3", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    assert_eq!(key_order(&config), ["a", "b", "c"]);
    assert_eq!(config.get_int("a"), Some(9));
    assert_eq!(config.get_int("c"), Some(3));
}

#[test]
fn merge_appends_genuinely_new_keys() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "d: 4\nb: 9\n");

    let base = Config::load_yaml("a: 1\nb: 2\nc: 3", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    // Overridden keys hold their place; only `d` is new, so it goes last
    assert_eq!(key_order(&config), ["a", "b", "c", "d"]);
    assert_eq!(config.get_int("b"), Some(9));
}

#[test]
fn merge_preserves_key_order_in_nested_mappings() {
    let dir = temp_dir();
    let overlay_file = write_file(&dir, "overlay.yaml", "db:\n  host: prodserver\n");

    let base = Config::load_yaml("db:\n  host: localhost\n  port: 5432\n  name: myapp", "/").unwrap();
    let config = base.merge_required(&overlay_file, None).unwrap();

    let db: yaml_serde::Mapping = config.get_as_strict("db").unwrap();
    let keys: Vec<&str> = db.keys().map(|k| k.as_str().unwrap()).collect();
    assert_eq!(keys, ["host", "port", "name"]);
    assert_eq!(config.str("db/host"), "prodserver");
}

#[test]
fn merge_key_order_does_not_depend_on_overlay_order() {
    let dir = temp_dir();
    let forward = write_file(&dir, "forward.yaml", "a: 9\nc: 7\n");
    let reverse = write_file(&dir, "reverse.yaml", "c: 7\na: 9\n");

    let one = Config::load_yaml("a: 1\nb: 2\nc: 3", "/").unwrap()
        .merge_required(&forward, None).unwrap();
    let two = Config::load_yaml("a: 1\nb: 2\nc: 3", "/").unwrap()
        .merge_required(&reverse, None).unwrap();

    assert_eq!(key_order(&one), ["a", "b", "c"]);
    assert_eq!(key_order(&one), key_order(&two));
}

#[test]
fn merge_records_an_environment_the_base_did_not_carry() {
    // The natural layered shape: the base file is not environment-specific, the
    // overlay is. The merge resolved `{env}` and then dropped the environment, so
    // `environment()` under-reported and `reload_from` — which takes no `env`
    // argument because it reads the one on the config — could not resolve a template
    // the merge had resolved a moment earlier.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    write_file(&dir, "over.prod.yaml", "app:\n  name: prod\n");
    let template = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    let mut config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&template, Some("prod"))
        .unwrap();

    assert_eq!(config.str("app/name"), "prod");
    assert_eq!(config.environment(), Some("prod"), "the merge's environment is recorded");

    // ...and is usable, which is the point of recording it
    let switch_to = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();
    config.reload_from(&switch_to).unwrap();
    assert_eq!(config.str("app/name"), "prod");
}

#[test]
fn merge_optional_also_records_the_environment() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    write_file(&dir, "local.dev.yaml", "app:\n  port: 9090\n");
    let template = dir.path().join("local.{env}.yaml").to_string_lossy().into_owned();

    let config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_optional(&template, Some("dev"))
        .unwrap();

    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.environment(), Some("dev"));
}

#[test]
fn merge_does_not_overwrite_an_environment_the_config_already_has() {
    // The config's environment is chosen by its constructor and is part of its
    // identity. Letting a later overlay reassign it would silently change what a
    // subsequent `reload_from` resolves, so the merge fills a gap and never replaces.
    let dir = temp_dir();
    let base = write_file(&dir, "config.prod.yaml", "app:\n  name: prod\n");
    let overlay = write_file(&dir, "over.yaml", "app:\n  extra: 1\n");

    let config = Config::load_required(&base, "/", Some("prod"))
        .unwrap()
        .merge_required(&overlay, Some("staging"))
        .unwrap();

    assert_eq!(config.environment(), Some("prod"), "the constructor's environment wins");
}

#[test]
fn a_merge_resolves_env_against_the_environment_the_config_carries() {
    // The commoner half of the layered shape, and the one that used to fail: the base
    // file is environment-specific, so the config knows its environment, and the overlay
    // template can simply use it. Passing `None` was a hard `FormatError` naming a value
    // the config was holding at the time.
    let dir = temp_dir();
    let base = write_file(&dir, "config.prod.yaml", "app:\n  port: 8080\n  name: base\n");
    write_file(&dir, "over.prod.yaml", "app:\n  name: prod\n");
    let template = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    let config = Config::load_required(&base, "/", Some("prod"))
        .unwrap()
        .merge_required(&template, None)
        .unwrap();

    assert_eq!(config.str("app/name"), "prod");
    assert_eq!(config.get_int("app/port"), Some(8080), "sibling from the base survives");
    assert_eq!(config.environment(), Some("prod"));
}

#[test]
fn merge_optional_also_resolves_env_against_the_config() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.dev.yaml", "app:\n  port: 8080\n");
    write_file(&dir, "local.dev.yaml", "app:\n  port: 9090\n");
    let template = dir.path().join("local.{env}.yaml").to_string_lossy().into_owned();

    let config = Config::load_required(&base, "/", Some("dev"))
        .unwrap()
        .merge_optional(&template, None)
        .unwrap();

    assert_eq!(config.get_int("app/port"), Some(9090));
}

#[test]
fn an_explicit_environment_at_a_merge_overrides_the_config() {
    // `None` means "the one this config already has", not "ignore mine" — so an overlay
    // for a *different* environment than the base is still expressible, and saying so
    // explicitly is how you say it. The config's own environment is unchanged, since
    // `adopt_environment` fills a gap and never reassigns.
    let dir = temp_dir();
    let base = write_file(&dir, "config.prod.yaml", "app:\n  name: base\n");
    write_file(&dir, "over.staging.yaml", "app:\n  name: staging\n");
    let template = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    let config = Config::load_required(&base, "/", Some("prod"))
        .unwrap()
        .merge_required(&template, Some("staging"))
        .unwrap();

    assert_eq!(config.str("app/name"), "staging", "the argument wins over the carried env");
    assert_eq!(config.environment(), Some("prod"), "which the config does not adopt");
}

#[test]
fn a_merge_template_with_no_environment_anywhere_still_errors() {
    // The fallback fills a gap; it does not invent one. With neither the argument nor the
    // config supplying an environment, `{env}` still has nothing to substitute, and
    // reporting that beats handing the literal braces to the OS as a missing file.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let template = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    let err = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&template, None)
        .unwrap_err();

    assert!(matches!(err, ConfigError::FormatError(_)), "got {err:?}");
    assert!(err.to_string().contains("{env}"), "got {err}");
}

#[test]
fn an_env_adopted_from_one_merge_resolves_the_next() {
    // The two halves composing: the first merge supplies an environment the base did not
    // have, `adopt_environment` records it, and `environment_for` hands it to the second
    // merge. Neither rule is much use without the other.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  name: base\n  extra: 0\n");
    write_file(&dir, "one.prod.yaml", "app:\n  name: one\n");
    write_file(&dir, "two.prod.yaml", "app:\n  extra: 2\n");
    let first = dir.path().join("one.{env}.yaml").to_string_lossy().into_owned();
    let second = dir.path().join("two.{env}.yaml").to_string_lossy().into_owned();

    let config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&first, Some("prod"))
        .unwrap()
        .merge_required(&second, None)
        .unwrap();

    assert_eq!(config.str("app/name"), "one");
    assert_eq!(config.get_int("app/extra"), Some(2));
    assert_eq!(config.environment(), Some("prod"));
}

#[test]
fn merge_optional_in_place_keeps_the_config_when_the_overlay_is_broken() {
    // The case `merge_optional` exists for and could not express. It tolerates an *absent*
    // overlay, but a broken one used to take the base config down with it — `self` was
    // moved into the call, so the error path had nothing to fall back to.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    let broken = write_file(&dir, "broken.yaml", "a: [unclosed\n");

    let mut config = Config::load_required(&base, "/", None).unwrap();
    let err = config.merge_optional_in_place(&broken, None).unwrap_err();

    assert!(matches!(err, ConfigError::YamlError { .. }), "got {err:?}");
    assert_eq!(config.get_int("app/port"), Some(8080), "the base is still readable");
    assert_eq!(config.str("app/name"), "base");
    assert_eq!(config.filename(), base);
}

#[test]
fn a_failed_in_place_merge_does_not_record_the_overlay() {
    // Not just the document: a config left holding a dead overlay would re-walk it on
    // every later reload, and the failure would come back each time.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let broken = write_file(&dir, "broken.yaml", "a: [unclosed\n");

    let mut config = Config::load_required(&base, "/", None).unwrap();
    assert!(config.merge_required_in_place(&broken, None).is_err());

    // A reload re-reads the base and nothing else, so it succeeds
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert!(!format!("{config:?}").contains("broken.yaml"), "got {config:?}");
}

#[test]
fn in_place_merges_agree_with_the_chaining_ones() {
    // The chaining forms are defined in terms of these, so the two must not be able to
    // drift — same document, same overlay chain, same environment.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    let required = write_file(&dir, "over.yaml", "app:\n  name: over\n");
    let optional = write_file(&dir, "local.yaml", "app:\n  port: 9090\n");

    let chained = Config::load_required(&base, "/", Some("prod"))
        .unwrap()
        .merge_required(&required, None)
        .unwrap()
        .merge_optional(&optional, None)
        .unwrap();

    let mut in_place = Config::load_required(&base, "/", Some("prod")).unwrap();
    in_place.merge_required_in_place(&required, None).unwrap();
    in_place.merge_optional_in_place(&optional, None).unwrap();

    assert_eq!(in_place.str("app/name"), chained.str("app/name"));
    assert_eq!(in_place.get_int("app/port"), chained.get_int("app/port"));
    assert_eq!(in_place.environment(), chained.environment());
    // `Debug` prints the overlay chain, which is the part with no accessor of its own
    assert_eq!(format!("{in_place:?}"), format!("{chained:?}"));
}

#[test]
fn merge_optional_in_place_still_tolerates_an_absent_file() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let missing = dir.path().join("nope.yaml").to_string_lossy().into_owned();

    let mut config = Config::load_required(&base, "/", None).unwrap();
    config.merge_optional_in_place(&missing, None).unwrap();

    assert_eq!(config.get_int("app/port"), Some(8080));
    // Recorded even though absent, so a later reload picks it up once it appears
    assert!(format!("{config:?}").contains("nope.yaml"), "got {config:?}");
}

#[test]
fn a_merge_without_an_environment_leaves_the_config_without_one() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "over.yaml", "app:\n  port: 9090\n");

    let config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&overlay, None)
        .unwrap();

    assert_eq!(config.environment(), None);
}
