//! Document order of mapping keys, across the three formats.
//!
//! `outline` promises "keys appear in document order, the same order the merge preserves",
//! and `merge_values` goes to real trouble to keep it — an overridden key holds its
//! position and genuinely-new keys are appended. Both were true only for YAML: the other
//! two formats parsed through a `BTreeMap` and arrived sorted alphabetically, so the order
//! was gone before this crate saw the document and the merge preserved an order the file
//! never had.

use super::Config;
// Only the merge tests read files, and every one of those is behind a format feature —
// so with neither enabled these would be unused imports, which `-D warnings` rejects.
#[cfg(any(feature = "json", feature = "toml"))]
use crate::test_util::{temp_dir, write_file};

/// Deliberately reverse-alphabetical, so sorted and document order cannot be confused.
const EXPECTED: &str = "zebra: <number>\nmango: <number>\napple: <number>\n";

#[test]
fn yaml_keeps_document_order() {
    let config = Config::load_yaml("zebra: 1\nmango: 2\napple: 3\n", "/").unwrap();
    assert_eq!(config.outline(), EXPECTED);
}

#[test]
#[cfg(feature = "json")]
fn json_keeps_document_order() {
    let config = Config::load_json(r#"{"zebra": 1, "mango": 2, "apple": 3}"#, "/").unwrap();
    assert_eq!(config.outline(), EXPECTED);
}

#[test]
#[cfg(feature = "toml")]
fn toml_keeps_document_order() {
    let config = Config::load_toml("zebra = 1\nmango = 2\napple = 3\n", "/").unwrap();
    assert_eq!(config.outline(), EXPECTED);
}

#[test]
#[cfg(feature = "json")]
fn json_keeps_nested_document_order() {
    let config = Config::load_json(
        r#"{"outer": {"zebra": 1, "mango": 2, "apple": 3}, "after": 4}"#,
        "/",
    ).unwrap();

    assert_eq!(
        config.outline(),
        "outer/zebra: <number>\nouter/mango: <number>\nouter/apple: <number>\nafter: <number>\n"
    );
}

#[test]
#[cfg(feature = "toml")]
fn toml_keeps_nested_document_order() {
    // A `[table]` header closes the keys above it, so `after` is written first
    let config = Config::load_toml(
        "after = 4\n\n[outer]\nzebra = 1\nmango = 2\napple = 3\n",
        "/",
    ).unwrap();

    assert_eq!(
        config.outline(),
        "after: <number>\nouter/zebra: <number>\nouter/mango: <number>\nouter/apple: <number>\n"
    );
}

#[test]
#[cfg(feature = "json")]
fn a_json_merge_preserves_the_base_order() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.json", r#"{"zebra": 1, "mango": 2, "apple": 3}"#);
    let overlay = write_file(&dir, "over.json", r#"{"mango": 20, "brand_new": 4}"#);

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    // The overridden key holds its position; the new key is appended
    assert_eq!(
        config.outline(),
        "zebra: <number>\nmango: <number>\napple: <number>\nbrand_new: <number>\n"
    );
    assert_eq!(config.get_int("mango"), Some(20));
}

#[test]
#[cfg(feature = "toml")]
fn a_toml_merge_preserves_the_base_order() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.toml", "zebra = 1\nmango = 2\napple = 3\n");
    let overlay = write_file(&dir, "over.toml", "mango = 20\nbrand_new = 4\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    assert_eq!(
        config.outline(),
        "zebra: <number>\nmango: <number>\napple: <number>\nbrand_new: <number>\n"
    );
    assert_eq!(config.get_int("mango"), Some(20));
}

#[test]
#[cfg(feature = "json")]
fn a_yaml_base_under_a_json_overlay_keeps_the_base_order() {
    // The mixed-format case the crate advertises. Both halves have to agree about order
    // or the merged document's order depends on which format each layer happened to use.
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "zebra: 1\nmango: 2\napple: 3\n");
    let overlay = write_file(&dir, "over.json", r#"{"apple": 30, "brand_new": 4}"#);

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    assert_eq!(
        config.outline(),
        "zebra: <number>\nmango: <number>\napple: <number>\nbrand_new: <number>\n"
    );
}

#[test]
#[cfg(feature = "json")]
fn deserializing_a_mapping_sees_document_order() {
    // The order is only *visible* downstream through something that preserves it —
    // `outline` above, and deserializing into the value model here. This is the shape
    // that reaches a caller re-serializing a merged config.
    let config = Config::load_json(r#"{"zebra": 1, "mango": 2, "apple": 3}"#, "/").unwrap();
    let map: yaml_serde::Mapping = config.deserialize_strict().unwrap();

    let keys: Vec<&str> = map.keys().filter_map(|k| k.as_str()).collect();
    assert_eq!(keys, vec!["zebra", "mango", "apple"]);
}
