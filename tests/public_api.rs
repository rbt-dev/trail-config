//! The re-export surface, exercised from outside the crate.
//!
//! These have to live in `tests/` rather than in a `#[cfg(test)]` module: a unit test
//! inside `src/` links against the crate's own dependency graph, so `yaml_serde::Value`
//! is nameable there whether or not it is re-exported, and the gap this file pins is
//! invisible. A consumer sees only what `lib.rs` makes public.
//!
//! Scope is deliberately narrow — the types a caller must be able to *name* in order to
//! use the values and errors the public API hands back. A broader consumer-vantage suite
//! is still worth having.

use trail_config::{Config, ConfigError, ConfigHandle, Mapping, Number, Sequence, Value, YamlError};

const YAML: &str = "app:\n  port: 8080\n  name: myapp\nfeatures:\n  - a\n  - b\n";

#[test]
fn get_returns_a_value_the_caller_can_name_and_destructure() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    // The binding is the point: without the re-export this needed a direct `yaml_serde`
    // dependency pinned to whatever version trail-config resolved, and `Debug` was the
    // only operation available on the result.
    let value: Option<Value> = config.get("app/port");

    let Some(Value::Number(number)) = value else {
        panic!("expected a number at app/port");
    };
    let number: Number = number;
    assert_eq!(number.as_i64(), Some(8080));

    assert_eq!(config.get("app/missing"), None);
}

#[test]
fn value_payload_types_are_nameable() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let Some(Value::Sequence(items)) = config.get("features") else {
        panic!("expected a sequence at features");
    };
    let items: Sequence = items;
    assert_eq!(items.len(), 2);

    // `Mapping` is how a caller observes key order, which the merge documents as
    // meaningful — so it has to be nameable for that claim to be checkable.
    let map: Mapping = config.deserialize_strict().unwrap();
    let keys: Vec<&str> = map.keys().map(|k| k.as_str().unwrap()).collect();
    assert_eq!(keys, ["app", "features"]);
}

#[test]
fn get_strict_returns_a_nameable_value() {
    let config = Config::load_yaml(YAML, "/").unwrap();

    let value: Value = config.get_strict("app/name").unwrap();
    assert_eq!(value, Value::String("myapp".to_string()));
}

#[test]
fn config_handle_hands_back_the_same_value_type() {
    let handle = ConfigHandle::new(Config::load_yaml(YAML, "/").unwrap());

    let value: Option<Value> = handle.get("app/name");
    assert_eq!(value, Some(Value::String("myapp".to_string())));
}

#[test]
fn the_yaml_error_behind_a_config_error_is_nameable() {
    match Config::load_yaml("app: [unclosed", "/") {
        Err(ConfigError::YamlError { file, source, .. }) => {
            // Binding `source` by type is what a caller needs to pass it on, wrap it or
            // inspect it — the public field was previously of an unnameable type.
            let source: YamlError = source;
            assert!(!source.to_string().is_empty());
            assert_eq!(file, None, "a string config has no file to name");
        },
        other => panic!("expected YamlError, got {:?}", other.err()),
    }
}

#[test]
fn the_error_source_chain_is_reachable() {
    use std::error::Error;

    let err = Config::load_required("no_such_file_xyz.yaml", "/", None).unwrap_err();
    assert!(err.source().is_some(), "the underlying io::Error should be reachable");
}

#[cfg(feature = "json")]
#[test]
fn the_json_error_behind_a_config_error_is_nameable() {
    use trail_config::JsonError;

    match Config::load_json("{invalid json}", "/") {
        Err(ConfigError::JsonError { source, .. }) => {
            let source: JsonError = source;
            assert!(!source.to_string().is_empty());
        },
        other => panic!("expected JsonError, got {:?}", other.err()),
    }
}

#[cfg(feature = "toml")]
#[test]
fn the_toml_error_behind_a_config_error_is_nameable() {
    use trail_config::TomlError;

    match Config::load_toml("invalid = [unclosed", "/") {
        Err(ConfigError::TomlError { source, .. }) => {
            let source: TomlError = source;
            assert!(!source.to_string().is_empty());
        },
        other => panic!("expected TomlError, got {:?}", other.err()),
    }
}

#[test]
#[cfg(feature = "json")]
fn format_is_nameable_and_reaches_every_as_constructor() {
    use std::fs;
    use trail_config::Format;

    // `Format` lives in a private module, so this pins that the re-export at the crate
    // root is what makes the `_as` constructors callable at all — a consumer cannot spell
    // their fourth argument otherwise.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.conf");
    let file = path.to_string_lossy().into_owned();

    // load_or_create_as writes the defaults it validated...
    let config =
        Config::load_or_create_as(&file, "/", None, Format::Json, r#"{"app": {"port": 8080}}"#)
            .unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    // ...load_required_as reads the same file back under the same parser...
    let config = Config::load_required_as(&file, "/", None, Format::Json).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    // ...and load_optional_as tolerates one that is not there.
    let absent = dir.path().join("nope.cfg").to_string_lossy().into_owned();
    let config = Config::load_optional_as(&absent, "/", None, Format::Json).unwrap();
    assert_eq!(config.filename(), absent);

    // `#[non_exhaustive]`, so a consumer's match needs a wildcard arm
    let described = match Format::Json {
        Format::Yaml => "yaml",
        Format::Json => "json",
        _ => "other",
    };
    assert_eq!(described, "json");

    fs::remove_file(&path).ok();
}
