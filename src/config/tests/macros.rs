use crate::config;
use crate::test_util::{temp_dir, write_file};

#[test]
fn minimal() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = config!(&path).unwrap();
    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn with_sep() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = config!(&path, sep: "::").unwrap();
    assert_eq!(config.str("app::port"), "8080");
}

#[test]
fn with_env() {
    let dir = temp_dir();
    write_file(&dir, "config_dev.yaml", "app:\n  port: 3000\n");
    let template = dir.path().join("config_{env}.yaml").to_string_lossy().into_owned();

    let config = config!(&template, env: "dev").unwrap();
    assert_eq!(config.str("app/port"), "3000");
}

#[test]
fn with_merge() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let config = config!(&base, merge: [&overlay]).unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn full_block_syntax() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  debug: false\n  name: myapp\n");
    let required = write_file(&dir, "prod.yaml", "app:\n  debug: false\n  port: 9090\n");
    let optional = write_file(&dir, "local.yaml", "app:\n  debug: true\n");

    let config = config! {
        file: &base,
        merge: [&required],
        merge_optional: [&optional],
    }.unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("app/name"), "myapp");
}

#[test]
fn env_with_merges_matches_the_documented_example() {
    // The flagship `config!` block from `docs/LOADING.md`: an env is supplied, but only
    // the required overlay carries an `{env}` placeholder. The base file and the
    // optional overlay are plain names.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let prod = write_file(&dir, "config.prod.yaml", "app:\n  port: 9090\n");
    let local = write_file(&dir, "config.local.yaml", "app:\n  debug: true\n");

    let template = dir.path().join("config.{env}.yaml").to_string_lossy().into_owned();
    let _ = &prod; // written above; reached through the template

    let config = config! {
        file: &base,
        sep: "/",
        env: "prod",
        merge: [&template],
        merge_optional: [&local],
    }.unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/name"), "myapp");
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[test]
fn positional_options_compose() {
    // None of these compiled while each option had an arm of its own. The block form
    // could express them, but only by writing `file:` — which the guide's list of
    // positional examples (`docs/LOADING.md`) gives no reason to expect.
    let dir = temp_dir();
    write_file(&dir, "config.prod.yaml", "app:\n  port: 8080\n  name: base\n");
    write_file(&dir, "over.prod.yaml", "app:\n  name: over\n");
    let base = dir.path().join("config.{env}.yaml").to_string_lossy().into_owned();
    let over = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    // sep + env
    let config = config!(&base, sep: "::", env: "prod").unwrap();
    assert_eq!(config.str("app::port"), "8080");
    assert_eq!(config.separator(), "::");
    assert_eq!(config.environment(), Some("prod"));

    // sep + env + merge
    let config = config!(&base, sep: "::", env: "prod", merge: [&over]).unwrap();
    assert_eq!(config.str("app::name"), "over");
    assert_eq!(config.str("app::port"), "8080", "sibling survives the deep merge");
}

#[test]
fn positional_env_with_merge_and_merge_optional() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    write_file(&dir, "over.dev.yaml", "app:\n  name: over\n");
    let over = dir.path().join("over.{env}.yaml").to_string_lossy().into_owned();

    // The positional form reaches `merge_optional`, which it previously had no arm for
    // at all — the block form was the only way to write one.
    let config = config!(
        &base,
        env: "dev",
        merge: [&over],
        merge_optional: ["nonexistent.yaml"],
    ).unwrap();

    assert_eq!(config.str("app/name"), "over");
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.environment(), Some("dev"));
}

#[test]
fn the_positional_and_block_forms_agree() {
    // The positional arm expands into the block one, so the two cannot drift. Asserted
    // rather than assumed, because that delegation is the whole reason there is now one
    // positional arm instead of four.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    let over = write_file(&dir, "over.yaml", "app:\n  name: over\n");

    let positional = config!(&base, sep: "/", env: "prod", merge: [&over]).unwrap();
    let block = config! {
        file: &base,
        sep: "/",
        env: "prod",
        merge: [&over],
    }.unwrap();

    // `Debug` covers filename, separator, environment and the overlay chain at once
    assert_eq!(format!("{positional:?}"), format!("{block:?}"));
    assert_eq!(positional.str("app/name"), block.str("app/name"));
}

#[test]
fn a_trailing_comma_is_accepted_positionally() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = config!(&base, sep: "/",).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn missing_file_errors() {
    let result = config!("nonexistent_macro_test.yaml");
    assert!(result.is_err());
}

#[test]
fn merge_optional_missing_is_ok() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");

    let config = config! {
        file: &base,
        merge_optional: ["nonexistent.yaml"],
    }.unwrap();

    assert_eq!(config.str("app/port"), "8080");
}
