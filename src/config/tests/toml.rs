#![cfg(feature = "toml")]

use super::{Config, ConfigError};
use crate::test_util::{env_lock, temp_dir, write_file};
use std::fs;

#[test]
fn load_toml_string() {
    let config = Config::load_toml("[app]\nport = 8080\ndebug = true", "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn load_toml_nested() {
    let toml_str = r#"
[db.redis]
host = "127.0.0.1"
port = 6379
"#;
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.str("db/redis/host"), "127.0.0.1");
    assert_eq!(config.get_int("db/redis/port"), Some(6379));
}

#[test]
fn load_toml_with_custom_separator() {
    let config = Config::load_toml("[app]\nport = 8080", "::").unwrap();
    assert_eq!(config.get_int("app::port"), Some(8080));
}

#[test]
fn load_toml_file_auto_detect() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.toml", "[app]\nport = 3000");

    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(3000));
}

#[test]
fn load_toml_file_explicit() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.toml", "[app]\nname = \"myapp\"");

    let config = Config::load_toml_file(&path, "/").unwrap();
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn load_toml_list() {
    let toml_str = "items = [\"one\", \"two\", \"three\"]";
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.list("items"), vec!["one", "two", "three"]);
}

#[test]
fn load_toml_env_var_interpolation() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_TOML_HOST", "toml-server");
    let toml_str = "[db]\nhost = \"${TRAIL_TEST_TOML_HOST}\"";
    let config = Config::load_toml(toml_str, "/").unwrap();
    assert_eq!(config.str("db/host"), "toml-server");
    std::env::remove_var("TRAIL_TEST_TOML_HOST");
}

#[test]
fn load_toml_invalid_errors() {
    let result = Config::load_toml("invalid = [unclosed", "/");
    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlError { .. }) => (),
        other => panic!("Expected TomlError, got: {:?}", other),
    }
}

#[test]
fn load_toml_empty_separator_errors() {
    let result = Config::load_toml("[a]\nb = 1", "");
    assert!(result.is_err());
}

#[test]
fn uppercase_toml_extension_reaches_the_toml_parser() {
    let dir = temp_dir();

    // Under the old byte-exact `ends_with(".toml")` this went to the YAML parser and
    // failed with "deserializing from YAML containing more than one document is not
    // supported" — `[table]` headers read as document separators, an error pointing at
    // the wrong problem entirely. On Windows and macOS this file *is* `c.toml`.
    for name in ["c.TOML", "c.ToMl"] {
        let path = write_file(&dir, name, "[app]\nport = 8080\n");
        let config = Config::load_required(&path, "/", None).unwrap();
        assert_eq!(config.get_int("app/port"), Some(8080), "{name}");
    }

    // And a genuine TOML error still surfaces as TomlError, not YamlError
    let path = write_file(&dir, "bad.TOML", "invalid = [unclosed");
    match Config::load_required(&path, "/", None) {
        Err(ConfigError::TomlError { .. }) => (),
        other => panic!("Expected TomlError, got: {:?}", other.err()),
    }
}

#[test]
fn uppercase_extension_selects_the_format_for_created_defaults() {
    // `load_or_create` validates its defaults through the same dispatch, so the
    // extension rule cannot drift between reading a file and parsing a string for it.
    let dir = temp_dir();
    let path = dir.path().join("new.TOML").to_string_lossy().into_owned();

    let config = Config::load_or_create(&path, "/", None, "[app]\nport = 8080\n").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    // YAML-shaped defaults are rejected under an uppercase .TOML too, and write nothing
    let path2 = dir.path().join("bad.TOML").to_string_lossy().into_owned();
    assert!(matches!(
        Config::load_or_create(&path2, "/", None, "app:\n  port: 8080\n"),
        Err(ConfigError::TomlError { .. })
    ));
    assert!(!fs::exists(&path2).unwrap());
}

#[test]
fn load_or_create_toml_defaults() {
    let dir = temp_dir();
    let path = dir.path().join("new.toml").to_string_lossy().into_owned();

    // Defaults for a .toml file are parsed as TOML
    let config = Config::load_or_create(&path, "/", None, "[app]\nport = 8080\n").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(fs::read_to_string(&path).unwrap(), "[app]\nport = 8080\n");

    // Invalid TOML defaults surface as TomlError
    let path2 = dir.path().join("bad.toml").to_string_lossy().into_owned();
    let result = Config::load_or_create(&path2, "/", None, "invalid = [unclosed");
    match result {
        Err(ConfigError::TomlError { .. }) => (),
        other => panic!("Expected TomlError, got: {:?}", other),
    }
    assert!(!fs::exists(&path2).unwrap(), "defaults that fail to parse must not be written");
}

#[test]
fn load_or_create_yaml_defaults_for_a_toml_file_writes_nothing() {
    // The mistake the format-aware defaults parsing exists to catch: YAML-shaped
    // defaults under a .toml filename. Writing before parsing left this file on disk,
    // so the create branch never ran again and every later run failed identically.
    let dir = temp_dir();
    let path = dir.path().join("cfg.toml").to_string_lossy().into_owned();

    assert!(matches!(
        Config::load_or_create(&path, "/", None, "app:\n  port: 8080\n"),
        Err(ConfigError::TomlError { .. })
    ));
    assert!(!fs::exists(&path).unwrap(), "no file should have been created");

    // The app can therefore still recover once the defaults are corrected
    let config = Config::load_or_create(&path, "/", None, "[app]\nport = 8080\n").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn toml_datetimes_are_scalar_strings() {
    // All four of TOML's date-time forms. Each used to arrive as the mapping
    // `{"$__toml_private_datetime": "..."}` — the `toml_datetime` crate's private serde
    // workaround, materialized because `yaml_serde` does not know the protocol.
    let toml_str = "\
[s]
offset = 2024-01-01T00:00:00Z
local_dt = 1979-05-27T07:32:00
local_date = 1979-05-27
local_time = 07:32:00
";
    let config = Config::load_toml(toml_str, "/").unwrap();

    assert_eq!(config.str("s/offset"), "2024-01-01T00:00:00Z");
    assert_eq!(config.str("s/local_dt"), "1979-05-27T07:32:00");
    assert_eq!(config.str("s/local_date"), "1979-05-27");
    assert_eq!(config.str("s/local_time"), "07:32:00");

    // The strict half agrees — it used to report "not a scalar"
    assert_eq!(config.str_strict("s/offset").unwrap(), "2024-01-01T00:00:00Z");

    // And the marker is gone entirely, not merely bypassed
    assert!(!config.contains("s/offset/$__toml_private_datetime"));
    assert!(!config.outline().contains("toml_private"));
}

#[test]
fn toml_datetime_deserializes_into_a_string_field() {
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    struct Window {
        starts: String,
        label: String,
    }

    // Previously "invalid type: map, expected a string" — the failure that made the
    // `toml` feature unusable for any config carrying a timestamp.
    let config = Config::load_toml("[window]\nstarts = 2024-01-01T00:00:00Z\nlabel = \"nightly\"", "/").unwrap();
    let window: Window = config.get_as_strict("window").unwrap();

    assert_eq!(window, Window {
        starts: "2024-01-01T00:00:00Z".to_string(),
        label: "nightly".to_string(),
    });
}

#[test]
fn toml_datetime_outlines_as_a_resolvable_path() {
    // `outline` promised every line could be pasted straight into an accessor; the
    // datetime line named a path built from a dependency's internal field name.
    let config = Config::load_toml("[s]\nstarted = 2024-01-01T00:00:00Z\n", "/").unwrap();

    assert_eq!(config.outline(), "s/started: <string>\n");
    assert!(config.contains("s/started"));
}

#[test]
fn toml_datetimes_survive_a_list_a_merge_and_a_reload() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.toml", "[s]\nstarted = 2024-01-01T00:00:00Z\nkept = 1\n");
    let overlay = write_file(&dir, "over.toml", "[s]\nstarted = 2025-06-15T12:00:00Z\n");

    // A sequence of them reads like any other list of scalars
    let dates = Config::load_toml("d = [1979-05-27, 1980-01-01]", "/").unwrap();
    assert_eq!(dates.list("d"), vec!["1979-05-27", "1980-01-01"]);
    assert_eq!(dates.list_strict("d").unwrap(), vec!["1979-05-27", "1980-01-01"]);

    // An overlay overrides one leaf without disturbing its sibling — a datetime is now
    // an ordinary scalar to the merge, where it used to be a mapping merged key by key
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.str("s/started"), "2025-06-15T12:00:00Z");
    assert_eq!(config.get_int("s/kept"), Some(1));

    fs::write(&base, "[s]\nstarted = 2030-12-25T00:00:00Z\nkept = 2\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("s/started"), "2025-06-15T12:00:00Z", "the overlay still wins");
    assert_eq!(config.get_int("s/kept"), Some(2));
}

#[test]
fn toml_scalars_convert_faithfully() {
    // The hand-written conversion replaced a serde round-trip, so every variant of
    // `toml::Value` is worth an assertion rather than the datetime alone.
    let toml_str = "\
text = \"hello\"
int = 42
neg = -9223372036854775808
float = 3.5
neg_float = -0.5
yes = true
no = false
list = [1, 2, 3]
mixed = [\"a\", 1, true]

[table]
nested = \"deep\"

[[items]]
name = \"first\"

[[items]]
name = \"second\"
";
    let config = Config::load_toml(toml_str, "/").unwrap();

    assert_eq!(config.str("text"), "hello");
    assert_eq!(config.get_int("int"), Some(42));
    assert_eq!(config.get_int("neg"), Some(i64::MIN));
    assert_eq!(config.get_float("float"), Some(3.5));
    assert_eq!(config.get_float("neg_float"), Some(-0.5));
    assert_eq!(config.get_bool("yes"), Some(true));
    assert_eq!(config.get_bool("no"), Some(false));
    assert_eq!(config.list("list"), vec!["1", "2", "3"]);
    assert_eq!(config.list("mixed"), vec!["a", "1", "true"]);
    assert_eq!(config.str("table/nested"), "deep");
    // An array of tables stays a sequence of mappings, addressable by deserializing
    assert_eq!(config.get_as::<Vec<std::collections::BTreeMap<String, String>>>("items").unwrap().len(), 2);

    // TOML's special floats round-trip through the number model too
    let floats = Config::load_toml("nan = nan\npos = inf\nneg = -inf\n", "/").unwrap();
    assert!(floats.get_float("nan").unwrap().is_nan());
    assert_eq!(floats.get_float("pos"), Some(f64::INFINITY));
    assert_eq!(floats.get_float("neg"), Some(f64::NEG_INFINITY));
}

#[test]
fn merge_toml_overlay() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let overlay = write_file(&dir, "overlay.toml", "[app]\nport = 9090");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn reload_toml_file() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.toml", "[app]\nport = 8080");

    let mut config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    fs::write(&path, "[app]\nport = 9090").unwrap();

    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));
}
