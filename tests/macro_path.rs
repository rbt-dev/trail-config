//! `config!` invoked by its full path, the way the rustdoc and README show it.
//!
//! **This file must not import `config!`.** `macro_rules!` expansion is textual and
//! resolved at the call site, so a `use trail_config::config;` anywhere in the file
//! would satisfy the macro's internal recursions and the test would pass over a macro
//! a consumer still cannot call. That is exactly why the defect survived a suite of
//! 245 unit and 48 integration tests: `src/config/tests/macros.rs` and
//! `tests/lifecycle.rs` both import the name, and every arm resolves for them.
//!
//! Every reference below is therefore spelled `trail_config::…` in full.

mod common;

use common::{path_in, temp_dir, write_file};

/// The block arm — the form the README leads with, and the only one that recurses
/// into `config!` for its `@sep` / `@env` helpers.
#[test]
fn the_block_form_expands_without_the_macro_in_scope() {
    let dir = temp_dir();
    write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    write_file(&dir, "config.prod.yaml", "app:\n  name: prod\n");

    let base = path_in(&dir, "config.yaml");
    let overlay = path_in(&dir, "config.{env}.yaml");
    let local = path_in(&dir, "config.local.yaml");

    let config = trail_config::config! {
        file: &base,
        sep: "::",
        env: "prod",
        merge: [&overlay],
        merge_optional: [&local],
    }
    .unwrap();

    // Each option reached the expansion intact: the separator from `sep:`, the
    // overlay resolved through `env:`, the base value the overlay left alone, and
    // the absent optional overlay skipped rather than erroring.
    assert_eq!(config.str("app::name"), "prod");
    assert_eq!(config.get_int("app::port"), Some(8080));
    assert_eq!(config.environment(), Some("prod"));
}

/// The block arm's defaults come from the same recursion, so an invocation that
/// omits `sep` and `env` exercises the `@sep` / `@env` arms that take no argument.
#[test]
fn the_block_form_defaults_expand_without_the_macro_in_scope() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let config = trail_config::config! {
        file: &base,
    }
    .unwrap();

    assert_eq!(config.get_int("app/port"), Some(8080)); // the default `/` separator
    assert_eq!(config.environment(), None);
}

/// The positional arms expand to `$crate::Config::…` directly and were never
/// affected. Pinned anyway so the whole documented surface is covered from a file
/// that cannot accidentally satisfy an unqualified recursion.
#[test]
fn the_positional_forms_expand_without_the_macro_in_scope() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    let prod = write_file(&dir, "config.prod.yaml", "app:\n  name: prod\n");
    let templated = path_in(&dir, "config.{env}.yaml");

    let minimal = trail_config::config!(&base).unwrap();
    assert_eq!(minimal.get_int("app/port"), Some(8080));

    let with_sep = trail_config::config!(&base, sep: "::").unwrap();
    assert_eq!(with_sep.get_int("app::port"), Some(8080));

    let with_env = trail_config::config!(&templated, env: "prod").unwrap();
    assert_eq!(with_env.str("app/name"), "prod");

    let with_merge = trail_config::config!(&base, merge: [&prod]).unwrap();
    assert_eq!(with_merge.str("app/name"), "prod");
    assert_eq!(with_merge.get_int("app/port"), Some(8080));
}

/// A failed load still comes back as the crate's own error type, nameable in full.
#[test]
fn a_failing_invocation_returns_a_nameable_error() {
    let dir = temp_dir();
    let missing = path_in(&dir, "no_such_file.yaml");

    let err = trail_config::config! {
        file: &missing,
        sep: "/",
    }
    .unwrap_err();

    assert!(matches!(err, trail_config::ConfigError::IoError { .. }), "got {:?}", err);
}
