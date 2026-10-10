use super::{Config, ConfigError};
use crate::test_util::{temp_dir, write_file};
use std::fs;

#[test]
fn reload_from_same_file() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  debug: false\n");

    let mut config = Config::load_optional(&test_file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/debug"), "false");

    fs::write(&test_file, "app:\n  port: 9090\n  debug: true\n").unwrap();

    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "true");
}

#[test]
fn reload_from_different_file() {
    let dir = temp_dir();
    let file1 = write_file(&dir, "first.yaml", "config:\n  name: first\n  value: 100\n");
    let file2 = write_file(&dir, "second.yaml", "config:\n  name: second\n  value: 200\n");

    let mut config = Config::load_optional(&file1, "/", None).unwrap();
    assert_eq!(config.str("config/name"), "first");
    assert_eq!(config.str("config/value"), "100");
    assert_eq!(config.filename(), file1);

    config.reload_from(&file2).unwrap();
    assert_eq!(config.str("config/name"), "second");
    assert_eq!(config.str("config/value"), "200");
    assert_eq!(config.filename(), file2);
}

#[test]
fn reload_preserves_separator() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "db:\n  host: localhost\n  port: 5432\n");

    let mut config = Config::load_optional(&test_file, "::", None).unwrap();
    assert_eq!(config.str("db::host"), "localhost");

    fs::write(&test_file, "db:\n  host: remote\n  port: 3306\n").unwrap();

    config.reload().unwrap();
    assert_eq!(config.str("db::host"), "remote");
}

#[test]
fn missing_optional_file_records_its_filename() {
    let dir = temp_dir();
    let missing = dir.path().join("absent.yaml").to_string_lossy().into_owned();

    let config = Config::load_optional(&missing, "/", None).unwrap();
    assert_eq!(config.filename(), missing);
    assert_eq!(config.str("app/port"), "");
}

#[test]
fn missing_optional_file_is_picked_up_once_it_appears() {
    let dir = temp_dir();
    let file = dir.path().join("late.yaml").to_string_lossy().into_owned();

    // Loaded before the file exists — empty, but not sourceless
    let mut config = Config::load_optional(&file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "");

    // Still absent: reload names the missing file rather than refusing outright
    match config.reload() {
        Err(ConfigError::IoError { .. }) => {},
        other => panic!("Expected IoError, got: {:?}", other),
    }

    fs::write(&file, "app:\n  port: 8080\n").unwrap();

    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn missing_optional_file_records_the_resolved_env_filename() {
    let dir = temp_dir();
    let template = dir.path().join("config.{env}.yaml").to_string_lossy().into_owned();
    let resolved = dir.path().join("config.prod.yaml").to_string_lossy().into_owned();

    let mut config = Config::load_optional(&template, "/", Some("prod")).unwrap();
    // The placeholder is substituted before the name is recorded
    assert_eq!(config.filename(), resolved);
    assert_eq!(config.environment(), Some("prod"));

    fs::write(&resolved, "app:\n  port: 8080\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "8080");
}

#[test]
fn reload_from_string_config_fails() {
    let yaml = "test: value";
    let mut config = Config::load_yaml(yaml, "/").unwrap();

    let result = config.reload();
    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("no file path"));
        },
        _ => panic!("Expected FormatError"),
    }
}

#[test]
fn reload_from_invalid_yaml_fails() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "valid:\n  yaml: content\n");

    let mut config = Config::load_optional(&test_file, "/", None).unwrap();

    fs::write(&test_file, "invalid: [unclosed\n").unwrap();

    let result = config.reload();
    assert!(result.is_err());

    // Original config still intact
    assert_eq!(config.str("valid/yaml"), "content");
}

#[test]
fn reload_re_applies_required_overlay() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  debug: false\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.str("app/debug"), "false");

    // Update both files
    fs::write(&base_file, "app:\n  port: 1111\n  debug: true\n").unwrap();
    fs::write(&overlay_file, "app:\n  port: 2222\n").unwrap();

    config.reload().unwrap();

    assert_eq!(config.str("app/port"), "2222");  // overlay wins
    assert_eq!(config.str("app/debug"), "true"); // from updated base
}

#[test]
fn reload_skips_missing_optional_overlay() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_optional(&overlay_file, None).unwrap();

    assert_eq!(config.str("app/port"), "9090");

    // Remove the optional overlay before reload
    fs::remove_file(&overlay_file).unwrap();

    // Update base file
    fs::write(&base_file, "app:\n  port: 1111\n").unwrap();

    // Reload should succeed, falling back to base value
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "1111");
}

#[test]
fn reload_fails_if_required_overlay_deleted() {
    let dir = temp_dir();
    let base_file = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay_file = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");

    let mut config = Config::load_required(&base_file, "/", None).unwrap()
        .merge_required(&overlay_file, None).unwrap();

    fs::remove_file(&overlay_file).unwrap();

    let result = config.reload();
    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError when required overlay is deleted"),
    }

    // Original config preserved
    assert_eq!(config.str("app/port"), "9090");
}

#[test]
fn reload_from_does_not_reapply_stale_overlays() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9999\n");
    let new_file = write_file(&dir, "new.yaml", "app:\n  port: 3000\n");

    // Load base, merge overlay that overrides port to 9999
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.str("app/port"), "9999");

    // Switch to a completely different file
    config.reload_from(&new_file).unwrap();
    assert_eq!(config.str("app/port"), "3000");

    // Now reload() should NOT re-apply the old overlay on top of new_file
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "3000");
}

#[test]
fn reload_from_substitutes_env_placeholder() {
    let dir = temp_dir();
    let first = write_file(&dir, "config_dev.yaml", "config:\n  name: dev\n");
    write_file(&dir, "other_dev.yaml", "config:\n  name: other-dev\n");

    let first_template = dir.path().join("config_{env}.yaml").to_string_lossy().into_owned();
    let other_template = dir.path().join("other_{env}.yaml").to_string_lossy().into_owned();
    let _ = &first;

    let mut config = Config::load_required(&first_template, "/", Some("dev")).unwrap();
    assert_eq!(config.str("config/name"), "dev");
    assert_eq!(config.environment(), Some("dev"));

    // `{env}` resolves against the environment the config already carries
    config.reload_from(&other_template).unwrap();
    assert_eq!(config.str("config/name"), "other-dev");
    assert_eq!(config.environment(), Some("dev"));

    // The *resolved* name is recorded, so reload() still works afterwards
    assert!(config.filename().ends_with("other_dev.yaml"));
    config.reload().unwrap();
    assert_eq!(config.str("config/name"), "other-dev");
}

#[test]
fn reload_from_placeholder_without_environment_errors() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");

    // Loaded without an environment, so there is nothing to substitute
    let mut config = Config::load_required(&base, "/", None).unwrap();

    let template = dir.path().join("other_{env}.yaml").to_string_lossy().into_owned();
    match config.reload_from(&template) {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("{env}"), "got: {}", msg);
        },
        other => panic!("Expected FormatError, got {:?}", other),
    }

    // And the config is left untouched
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.filename(), base);
}

#[test]
fn reload_from_preserves_state_when_env_resolution_fails() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  tag: overlay\n");
    // Parses fine, then fails during env resolution — the one path that used to
    // commit the new filename before it could fail
    let broken = write_file(
        &dir,
        "broken.yaml",
        "app:\n  port: ${TRAIL_CONFIG_TEST_UNSET_RELOAD_FROM}\n",
    );

    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/tag"), "overlay");

    let result = config.reload_from(&broken);
    match result {
        Err(ConfigError::FormatError(_)) => (),
        other => panic!("Expected FormatError for unresolvable env var, got {:?}", other),
    }

    // Content and filename untouched
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.str("app/tag"), "overlay");
    assert_eq!(config.filename(), base);

    // The clinching check: a later reload() must still read the *base* file and
    // re-apply the overlay. If reload_from had committed the filename it would read
    // broken.yaml; if it had cleared the overlays, `tag` would be gone.
    fs::write(&base, "app:\n  port: 1111\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "1111");   // base was re-read
    assert_eq!(config.str("app/tag"), "overlay"); // overlay chain survived
}

#[test]
fn reload_from_preserves_state_when_new_file_is_invalid() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let broken = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    let mut config = Config::load_required(&base, "/", None).unwrap();

    assert!(config.reload_from(&broken).is_err());
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.filename(), base);
}

#[test]
fn a_failed_reload_from_keeps_the_overlay_chain() {
    // `reload_from` clears the overlays on success, so the failure path has to be checked
    // separately: clearing them on the way out would leave a config that had lost its
    // layering without having gained the file it was switching to.
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n  name: base\n");
    let overlay = write_file(&dir, "over.yaml", "app:\n  name: over\n");
    let broken = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    let mut config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&overlay, None)
        .unwrap();
    assert_eq!(config.str("app/name"), "over");

    assert!(config.reload_from(&broken).is_err());

    // Not merely the current document: the chain itself must still be registered, which a
    // reload is what actually proves — it re-reads the base and re-applies the overlay.
    config.reload().unwrap();
    assert_eq!(config.str("app/name"), "over", "the overlay is still in the chain");
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.filename(), base);
}

#[test]
fn reload_from_preserves_state_when_new_file_is_missing() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");

    let mut config = Config::load_required(&base, "/", None).unwrap();

    let missing = format!("{}.nope", base);
    assert!(config.reload_from(&missing).is_err());
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.filename(), base);
}

#[test]
fn reload_from_rejects_empty_filename() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let mut config = Config::load_optional(&test_file, "/", None).unwrap();
    assert_eq!(config.str("app/port"), "8080");

    let result = config.reload_from("");
    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename"),
    }

    // Config is left unchanged — the check fires before any state mutation
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.filename(), test_file);
}

// Files caught mid-save. An editor that truncates a file and then writes it leaves a
// zero-length file in between, and an empty document is a valid one: an empty base used
// to reload as an empty config, and an empty overlay as a no-op, both reporting `Ok`.

/// Truncates `path`, then has a writer fill it a tenth of the way into the retry window,
/// leaving nine tenths of margin for a slow machine to schedule the thread late.
fn refill_during_window(path: &str, content: &'static str) -> std::thread::JoinHandle<()> {
    use crate::config::loader::EMPTY_FILE_WINDOW;

    fs::write(path, "").unwrap();
    let path = path.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(EMPTY_FILE_WINDOW / 10);
        fs::write(&path, content).unwrap();
    })
}

fn assert_empty_file_error(result: Result<(), ConfigError>, file: &str) {
    match result {
        Err(ConfigError::IoError { file: Some(named), source, .. }) => {
            assert_eq!(named, file);
            assert_eq!(source.kind(), std::io::ErrorKind::UnexpectedEof);
        },
        other => panic!("expected an IoError for the empty file, got {other:?}"),
    }
}

#[test]
fn reload_waits_out_a_base_caught_mid_save() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: old\n");
    let mut config = Config::load_required(&base, "/", None).unwrap();

    let writer = refill_during_window(&base, "db:\n  host: new\n  port: 5432\n");
    config.reload().unwrap();
    writer.join().unwrap();

    assert_eq!(config.str("db/host"), "new");
    assert_eq!(config.get_int("db/port"), Some(5432));
}

#[test]
fn reload_rejects_a_base_that_stays_empty() {
    use crate::config::loader::EMPTY_FILE_WINDOW;
    use std::time::Instant;

    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: prod-db\n  port: 5432\n");
    let local = write_file(&dir, "config.local.yaml", "db:\n  host: override-db\n");
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_optional(&local, None).unwrap();

    // The review's worst case: the whole base silently gone, only the overlay's keys left
    fs::write(&base, "").unwrap();
    let started = Instant::now();
    assert_empty_file_error(config.reload(), &base);

    assert!(started.elapsed() >= EMPTY_FILE_WINDOW / 2, "gave up without waiting");
    assert_eq!(config.str("db/host"), "override-db");
    assert_eq!(config.get_int("db/port"), Some(5432));
}

/// Only zero bytes counts as empty: a file with any content, even just a comment, is
/// read at once and reloads as before.
#[test]
fn reload_accepts_a_comment_only_base_without_waiting() {
    use crate::config::loader::EMPTY_FILE_WINDOW;
    use std::time::Instant;

    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: prod-db\n");
    let mut config = Config::load_required(&base, "/", None).unwrap();

    fs::write(&base, "# nothing configured\n").unwrap();
    let started = Instant::now();
    config.reload().unwrap();

    assert!(started.elapsed() < EMPTY_FILE_WINDOW / 2, "waited on a non-empty file");
    assert!(!config.contains("db/host"));
}

#[test]
fn reload_waits_out_an_overlay_caught_mid_save() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: prod-db\n");
    let local = write_file(&dir, "config.local.yaml", "db:\n  host: override-db\n");
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_optional(&local, None).unwrap();

    let writer = refill_during_window(&local, "db:\n  host: newer-override\n");
    config.reload().unwrap();
    writer.join().unwrap();

    assert_eq!(config.str("db/host"), "newer-override");
}

/// An overlay that stays empty keeps its documented meaning: a no-op. Unlike an empty
/// base it describes a valid configuration, so it is waited on but never rejected.
#[test]
fn reload_treats_an_overlay_that_stays_empty_as_a_no_op() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: prod-db\n");
    let local = write_file(&dir, "config.local.yaml", "db:\n  host: override-db\n");
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&local, None).unwrap();

    fs::write(&local, "").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("db/host"), "prod-db");
}

#[test]
fn reload_from_rejects_a_file_that_stays_empty() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let local = write_file(&dir, "config.local.yaml", "app:\n  debug: true\n");
    let other = write_file(&dir, "other.yaml", "");
    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&local, None).unwrap();

    assert_empty_file_error(config.reload_from(&other), &other);

    // Nothing was committed: same file, same content, overlays kept
    assert_eq!(config.filename(), base);
    assert_eq!(config.str("app/port"), "8080");
    fs::write(&local, "app:\n  debug: false\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("app/debug"), "false");
}

#[test]
fn reload_from_waits_out_a_file_caught_mid_save() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let other = write_file(&dir, "other.yaml", "app:\n  port: 1\n");
    let mut config = Config::load_required(&base, "/", None).unwrap();

    let writer = refill_during_window(&other, "app:\n  port: 9090\n");
    config.reload_from(&other).unwrap();
    writer.join().unwrap();

    assert_eq!(config.str("app/port"), "9090");
    assert_eq!(config.filename(), other);
}

#[test]
fn a_handle_keeps_serving_its_config_when_the_base_stays_empty() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  host: prod-db\n");
    let handle = crate::ConfigHandle::new(Config::load_required(&base, "/", None).unwrap());

    fs::write(&base, "").unwrap();
    assert_empty_file_error(handle.reload(), &base);
    assert_eq!(handle.str("db/host"), "prod-db");
}
