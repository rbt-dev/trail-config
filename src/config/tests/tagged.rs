//! YAML `!Tag` values — how they interpolate, merge, read and list.
//!
//! A tag is how serde spells an enum variant in YAML, so a tagged node is an ordinary
//! subtree wearing a label. The rule these tests pin is that a tag is **transparent to
//! reading and addressing, and preserved for deserializing**: every accessor looks
//! through it, and only `get` / `get_as` still see it, because deserializing an enum is
//! what the tag is for.
//!
//! `Value::Tagged` used to land in a catch-all arm in three separate walkers, so all of
//! this went uninterpolated, unmerged and unlisted.

use super::Config;
use crate::test_util::{env_lock, temp_dir, write_file};

#[test]
fn env_vars_resolve_inside_a_tagged_value() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_TAGGED_HOST", "tagged-server");

    let yaml = "\
db: !Postgres
  host: ${TRAIL_TEST_TAGGED_HOST}
  nested:
    deeper: ${TRAIL_TEST_TAGGED_HOST}
  items:
    - ${TRAIL_TEST_TAGGED_HOST}
scalar: !Custom ${TRAIL_TEST_TAGGED_HOST}
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    assert_eq!(config.str("db/host"), "tagged-server");
    assert_eq!(config.str("db/nested/deeper"), "tagged-server");
    assert_eq!(config.list("db/items"), vec!["tagged-server"]);
    assert_eq!(config.str("scalar"), "tagged-server");

    std::env::remove_var("TRAIL_TEST_TAGGED_HOST");
}

#[test]
fn an_unset_required_var_under_a_tag_still_errors() {
    let _env = env_lock();
    std::env::remove_var("TRAIL_TEST_TAGGED_MISSING");

    // The worse half of the bug: `${VAR}` with no default is the crate's one fail-fast
    // guarantee, and a tag anywhere above it used to disable that for the whole subtree —
    // the load succeeded and served the literal placeholder text.
    for yaml in [
        "db: !Pg\n  password: ${TRAIL_TEST_TAGGED_MISSING}\n",
        "db: !Pg ${TRAIL_TEST_TAGGED_MISSING}\n",
        "db: !Pg\n  - ${TRAIL_TEST_TAGGED_MISSING}\n",
    ] {
        let result = Config::load_yaml(yaml, "/");
        assert!(result.is_err(), "an unset variable under a tag must error: {yaml}");
        assert!(
            result.unwrap_err().to_string().contains("TRAIL_TEST_TAGGED_MISSING"),
            "the error should name the variable",
        );
    }
}

#[test]
fn the_tag_survives_interpolation_of_its_contents() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_TAGGED_KEPT", "resolved");

    // Interpolation rebuilds the tagged node, so the tag has to be carried across intact —
    // it is what `get_as` selects the enum variant by. A `${VAR}` cannot be written *as* a
    // tag in the first place: YAML's tag syntax rejects `$` and `{`, so "tags are never
    // interpolated" is enforced by the format before it reaches this crate.
    let config = Config::load_yaml("db: !Postgres\n  host: ${TRAIL_TEST_TAGGED_KEPT}\n", "/").unwrap();

    assert_eq!(config.str("db/host"), "resolved");
    assert!(
        format!("{:?}", config.get("db").unwrap()).contains("Postgres"),
        "the tag must survive interpolation",
    );

    std::env::remove_var("TRAIL_TEST_TAGGED_KEPT");
}

#[test]
fn a_same_tag_overlay_deep_merges_instead_of_replacing() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "db: !Postgres\n  host: keep-me\n  port: 5432\n");
    let overlay = write_file(&dir, "over.yaml", "db: !Postgres\n  port: 6543\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    // The sibling the overlay did not restate used to vanish
    assert_eq!(config.str("db/host"), "keep-me");
    assert_eq!(config.get_int("db/port"), Some(6543));
}

#[test]
fn a_secondary_handle_names_a_different_tag() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "db: !Postgres\n  host: keep-me\n  port: 5432\n");
    // `!!Postgres` is YAML's *secondary* tag handle and expands to something other than
    // `!Postgres`, so this is the replace case rather than the merge case — the tags are
    // genuinely different, not two spellings of one.
    let overlay = write_file(&dir, "over.yaml", "db: !!Postgres\n  port: 6543\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    assert_eq!(config.get_int("db/port"), Some(6543));
    assert!(!config.contains("db/host"), "a secondary handle is not the same tag");
}

#[test]
fn a_different_tag_replaces_the_subtree() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "db: !Postgres\n  host: pg-host\n  port: 5432\n");
    let overlay = write_file(&dir, "over.yaml", "db: !Sqlite\n  path: /tmp/db\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    // Deliberate: the tag names the variant, so this is a change of shape rather than a
    // patch to the existing one. Merging the fields would produce a document belonging to
    // neither variant.
    assert_eq!(config.str("db/path"), "/tmp/db");
    assert!(!config.contains("db/host"), "a differing tag must replace, not merge");
    assert!(!config.contains("db/port"));
}

#[test]
fn an_untagged_overlay_replaces_a_tagged_base() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "db: !Postgres\n  host: pg-host\n  port: 5432\n");
    let overlay = write_file(&dir, "over.yaml", "db:\n  port: 6543\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    // Also deliberate, and the same rule: tagged and untagged are different shapes. An
    // overlay that dropped the tag while keeping the fields would silently produce a
    // document that no longer deserializes into the enum the base named.
    assert_eq!(config.get_int("db/port"), Some(6543));
    assert!(!config.contains("db/host"));
}

#[test]
fn accessors_read_through_a_tag() {
    let yaml = "\
text: !Custom hello
number: !Custom 42
decimal: !Custom 2.5
flag: !Custom true
items: !Custom
  - a
  - b
nested: !Outer
  inner: !Inner deep
";
    let config = Config::load_yaml(yaml, "/").unwrap();

    // Lenient
    assert_eq!(config.str("text"), "hello");
    assert_eq!(config.get_int("number"), Some(42));
    assert_eq!(config.get_float("decimal"), Some(2.5));
    assert_eq!(config.get_bool("flag"), Some(true));
    assert_eq!(config.list("items"), vec!["a", "b"]);
    assert_eq!(config.str("nested/inner"), "deep");

    // Strict agrees — these used to report "not a scalar" / "not a number" for a value
    // the path resolved to perfectly well
    assert_eq!(config.str_strict("text").unwrap(), "hello");
    assert_eq!(config.get_int_strict("number").unwrap(), 42);
    assert!(config.get_bool_strict("flag").unwrap());
    assert_eq!(config.list_strict("items").unwrap(), vec!["a", "b"]);

    // And `fmt`, which shares the same scalar conversion
    assert_eq!(config.fmt("{}:{}", "", &["text", "number"]), "hello:42");
}

#[test]
fn get_and_get_as_still_see_the_tag() {
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    enum Backend {
        Postgres { host: String },
        Sqlite { path: String },
    }

    let config = Config::load_yaml("db: !Postgres\n  host: pg-host\n", "/").unwrap();

    // The whole point of a tag: `get_as` must still be able to pick the variant, so the
    // reading accessors looking through a tag must not have removed it from the document
    let backend: Backend = config.get_as_strict("db").unwrap();
    assert_eq!(backend, Backend::Postgres { host: "pg-host".to_string() });

    // `get` hands back the tagged value rather than its contents
    assert!(format!("{:?}", config.get("db").unwrap()).contains("Postgres"));
}

#[test]
fn a_tagged_mapping_is_listed_by_the_outline() {
    let config = Config::load_yaml(
        "db: !Postgres\n  host: h\n  port: 5432\nplain: x\nscalar: !Custom hello\n",
        "/",
    ).unwrap();

    // The tagged subtree used to print as `db: <value>`, hiding two addressable paths
    assert_eq!(
        config.outline(),
        "db/host: <string>\ndb/port: <number>\nplain: <string>\nscalar: <string>\n"
    );

    // Every line still resolves, which is the outline's actual contract
    for line in config.outline().lines() {
        let (path, _) = line.rsplit_once(": ").unwrap();
        assert!(config.contains(path), "outline printed an unresolvable path: {path}");
    }
}

#[test]
fn debug_describes_a_tagged_document_by_its_shape() {
    let config = Config::load_yaml("a: !Tagged\n  x: 1\n  y: 2\n", "/").unwrap();
    let printed = format!("{:?}", config);

    // The whole document is a 1-key mapping; the tag is inside it
    assert!(printed.contains("<1 key>"), "got: {printed}");

    // A tagged document at the root is described by what it wraps, not as "<scalar>"
    let root = Config::load_yaml("!Tagged\nx: 1\ny: 2\n", "/").unwrap();
    assert!(format!("{:?}", root).contains("<2 keys>"), "got: {:?}", root);
}

#[test]
fn a_reload_reinterpolates_inside_a_tag() {
    let _env = env_lock();
    std::env::set_var("TRAIL_TEST_TAGGED_RELOAD", "first");

    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", "db: !Pg\n  host: ${TRAIL_TEST_TAGGED_RELOAD}\n");

    let mut config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.str("db/host"), "first");

    std::env::set_var("TRAIL_TEST_TAGGED_RELOAD", "second");
    config.reload().unwrap();
    assert_eq!(config.str("db/host"), "second");

    std::env::remove_var("TRAIL_TEST_TAGGED_RELOAD");
}
