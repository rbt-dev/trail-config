use super::Config;
use crate::config::outline::escape_key;

#[test]
fn outline_lists_leaf_paths_with_their_types() {
    let yaml = "\
app:
  name: myapp
  port: 8080
  debug: true
  ratio: 0.5
db:
  redis:
    server: 127.0.0.1
features:
  - a
  - b
nothing:
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.outline(), "\
app/name: <string>
app/port: <number>
app/debug: <bool>
app/ratio: <number>
db/redis/server: <string>
features: <2 items>
nothing: <null>
");
}

#[test]
fn outline_never_prints_a_value() {
    // The reason it prints types rather than the document: env vars are already
    // interpolated by the time a Config exists, so a dump is a dump of the secrets
    let config = Config::load_yaml("db:\n  password: hunter2\n  port: 5432\n", "/").unwrap();
    let outline = config.outline();

    assert!(!outline.contains("hunter2"), "outline leaked a value: {}", outline);
    assert!(!outline.contains("5432"), "outline leaked a value: {}", outline);
    assert_eq!(outline, "db/password: <string>\ndb/port: <number>\n");
}

#[test]
fn outline_paths_are_resolvable_by_the_accessors() {
    // Every line must be usable as written — that is the entire point of printing paths
    // rather than a tree
    let yaml = "\
sections:
  \"db/redis\":
    port: 6379
  \"back\\\\slash\": 1
plain:
  key: value
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    let outline = config.outline();

    assert_eq!(outline, "\
sections/db\\/redis/port: <number>
sections/back\\\\slash: <number>
plain/key: <string>
");

    for line in outline.lines() {
        let path = line.rsplit_once(": ").expect("every line has a type").0;
        assert!(config.contains(path), "outline printed an unresolvable path: {}", path);
    }
}

#[test]
fn every_unmarked_outline_line_resolves() {
    // The invariant the test above asserts, over a document that actually contains the
    // keys the path syntax cannot express. It passed only because no key it used was of
    // either kind: an empty key printed `a/`, which `get_leaf` rejects, and a non-string
    // key printed the literal text `<non-string key>`.
    let yaml = "\
plain: 1
\"\": 2
nested:
  \"\": 3
  ok: 4
1: 5
true: 6
2.5: 7
~: 8
under:
  9:
    deeper: 10
? [a, b]
: 11
";
    let config = Config::load_yaml(yaml, "/").unwrap();
    let outline = config.outline();

    for line in outline.lines() {
        let path = line.rsplit_once(": ").expect("every line has a type").0;
        if line.ends_with("# not addressable") {
            assert!(!config.contains(path), "a marked line resolved after all: {}", line);
        } else {
            assert!(config.contains(path), "outline printed an unresolvable path: {}", line);
        }
    }

    // Both kinds of unusable key are still *listed* — dropping them would leave the
    // reader comparing the outline against the file
    assert!(outline.contains("\"\": <number>  # not addressable"));
    assert!(outline.contains("nested/\"\": <number>  # not addressable"));
    assert!(outline.contains("1: <number>  # not addressable"));
    assert!(outline.contains("true: <number>  # not addressable"));
    assert!(outline.contains("2.5: <number>  # not addressable"));
    assert!(outline.contains("null: <number>  # not addressable"));
    assert!(outline.contains("<complex key>: <number>  # not addressable"));

    // A distinct non-string key is no longer collapsed onto the same text as every other
    assert_eq!(outline.matches("# not addressable").count(), 8);

    // The marker covers the whole line, so a subtree under an unusable key carries it too
    assert!(outline.contains("under/9/deeper: <number>  # not addressable"));

    // ...and the addressable siblings are untouched
    assert!(outline.contains("plain: <number>\n"));
    assert!(outline.contains("nested/ok: <number>\n"));
}

#[test]
fn a_top_level_empty_key_is_not_confused_with_a_scalar_document() {
    // The empty key at the root printed a bare `<number>` — byte-identical to what
    // `outline_of_a_pathless_document_prints_its_type_alone` asserts for a document that
    // holds a single scalar. Two different documents, one output.
    let empty_key = Config::load_yaml("\"\": 1\n", "/").unwrap();
    let scalar_doc = Config::load_yaml("1\n", "/").unwrap();

    assert_eq!(empty_key.outline(), "\"\": <number>  # not addressable\n");
    assert_eq!(scalar_doc.outline(), "<number>\n");
    assert_ne!(empty_key.outline(), scalar_doc.outline());
}

#[test]
fn a_quoted_empty_key_is_told_apart_from_a_two_quote_key() {
    // Both render as `""`. The marker is what distinguishes them: a key made of two quote
    // characters is an ordinary string key and resolves.
    let config = Config::load_yaml("'\"\"': 1\n", "/").unwrap();

    assert_eq!(config.outline(), "\"\": <number>\n");
    assert!(config.contains("\"\""), "a two-quote key is an ordinary addressable key");
}

#[test]
fn outline_uses_the_configs_own_separator() {
    let config = Config::load_yaml("db:\n  redis:\n    port: 6379\n", "::").unwrap();

    assert_eq!(config.outline(), "db::redis::port: <number>\n");
    assert!(config.contains("db::redis::port"));
}

#[test]
fn outline_escapes_a_multi_character_separator_in_a_key() {
    let config = Config::load_yaml("a:\n  \"b::c\": 1\n", "::").unwrap();

    assert_eq!(config.outline(), "a::b\\::c: <number>\n");
    assert!(config.contains(r"a::b\::c"));
}

#[test]
fn outline_of_a_pathless_document_prints_its_type_alone() {
    // `load_optional` on a missing file yields an empty document. Printing ": <null>"
    // would put a line in the output whose path the accessors reject — every other
    // line is resolvable as written
    let empty = Config::load_yaml("", "/").unwrap();
    assert_eq!(empty.outline(), "<null>\n");

    // Same for a document that is a bare sequence: no key is addressable
    let sequence = Config::load_yaml("- a\n- b\n- c\n", "/").unwrap();
    assert_eq!(sequence.outline(), "<3 items>\n");
}

#[test]
fn outline_marks_an_empty_mapping_rather_than_skipping_it() {
    let config = Config::load_yaml("a: {}\nb: 1\n", "/").unwrap();

    assert_eq!(config.outline(), "a: <empty mapping>\nb: <number>\n");
}

#[test]
fn escape_key_empty_separator_terminates() {
    // The counterpart of `parse_path_empty_separator_terminates` in escape.rs.
    // Unreachable through the public API — `check_separator` rejects an empty
    // separator at construction — but the loop must not spin forever, growing the
    // output until it exhausts memory, if it is ever reached.
    assert_eq!(escape_key("a/b", ""), "a/b");

    // A backslash is still escaped: only separator matching is skipped, since with
    // no separator there is nothing for a path to be split on.
    assert_eq!(escape_key(r"a\b", ""), r"a\\b");
}

#[test]
fn escape_key_escapes_separators_and_backslashes() {
    // The behaviour the guard above must leave untouched
    assert_eq!(escape_key("a/b", "/"), r"a\/b");
    assert_eq!(escape_key(r"a\b", "/"), r"a\\b");
    assert_eq!(escape_key("a::b", "::"), r"a\::b");
    assert_eq!(escape_key("plain", "/"), "plain");
}
