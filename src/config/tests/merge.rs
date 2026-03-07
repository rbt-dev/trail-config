use super::Config;

#[test]
fn merge_overlay_overrides_base() {
    let base    = Config::load_yaml("app:\n  port: 8080\n  debug: false", "/").unwrap();
    let overlay = Config::load_yaml("app:\n  port: 9090", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "9090");    // overridden
    assert_eq!(config.str("app/debug"), "false");  // preserved
}

#[test]
fn merge_deep_preserves_siblings() {
    let base    = Config::load_yaml("db:\n  host: localhost\n  port: 5432\n  name: mydb", "/").unwrap();
    let overlay = Config::load_yaml("db:\n  host: prodserver", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("db/host"), "prodserver"); // overridden
    assert_eq!(config.str("db/port"), "5432");       // preserved
    assert_eq!(config.str("db/name"), "mydb");       // preserved
}

#[test]
fn merge_adds_new_keys_from_overlay() {
    let base    = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let overlay = Config::load_yaml("app:\n  debug: true", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn merge_replaces_sequences_wholesale() {
    let base    = Config::load_yaml("features:\n  - a\n  - b\n  - c", "/").unwrap();
    let overlay = Config::load_yaml("features:\n  - x\n  - y", "/").unwrap();
    let config = base.merge(overlay);

    let list = config.list("features");
    assert_eq!(list, vec!["x", "y"]); // replaced, not appended
}

#[test]
fn merge_with_empty_overlay_is_identity() {
    let base  = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let empty = Config::default();
    let config = base.merge(empty);

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_empty_base_with_overlay() {
    let base    = Config::default();
    let overlay = Config::load_yaml("app:\n  port: 8080", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn merge_chaining() {
    let base   = Config::load_yaml("app:\n  port: 8080\n  debug: false\n  name: base", "/").unwrap();
    let first  = Config::load_yaml("app:\n  port: 9090", "/").unwrap();
    let second = Config::load_yaml("app:\n  debug: true", "/").unwrap();
    let config = base.merge(first).merge(second);

    assert_eq!(config.str("app/port"), "9090");           // from first overlay
    assert_eq!(config.get_bool("app/debug"), Some(true)); // from second overlay
    assert_eq!(config.str("app/name"), "base");           // preserved from base
}

#[test]
fn merge_preserves_base_separator() {
    let base    = Config::load_yaml("app:\n  port: 8080", "::").unwrap();
    let overlay = Config::load_yaml("app:\n  port: 9090", "/").unwrap();
    let config = base.merge(overlay);

    assert_eq!(config.str("app::port"), "9090");
}
