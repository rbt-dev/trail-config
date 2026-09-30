use yaml_serde::{Value, from_str, Number};
use super::{ConfigError, YAML};
use crate::config::accessor::to_string;
use crate::config::loader::get_file;
use crate::config::path::get_leaf;

#[test]
fn get_leaf_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value1 = get_leaf(&parsed, "db/redis/port", "/");
    let value2 = get_leaf(&parsed, "db/redis/username", "/");

    assert_eq!(value1, Some(&Value::Number(Number::from(6379))));
    assert_eq!(value2, None);
}

#[test]
fn get_file_test() {
    let result = get_file("config_{env}.yaml", Some("dev"));

    assert!(result.is_ok());
    let (file, env) = result.unwrap();
    assert_eq!(env, Some(String::from("dev")));
    assert_eq!(file, "config_dev.yaml");
}

#[test]
fn get_file_without_placeholder_keeps_the_environment() {
    // Not an error: in a layered setup only some files are environment-specific,
    // but the environment is still worth recording on the Config.
    let result = get_file("config.yaml", Some("dev"));

    assert!(result.is_ok(), "got {:?}", result);
    let (file, env) = result.unwrap();
    assert_eq!(file, "config.yaml");
    assert_eq!(env, Some(String::from("dev")));
}

#[test]
fn get_file_placeholder_without_environment_errors() {
    // The reverse *is* an error — a literal "config.{env}.yaml" handed to the OS
    // would come back as a missing file, pointing at the wrong problem.
    let result = get_file("config_{env}.yaml", None);

    match result {
        Err(ConfigError::FormatError(msg)) => {
            assert!(msg.contains("{env}"), "message should name the placeholder: {}", msg);
        },
        other => panic!("Expected FormatError for unsubstituted {{env}}, got {:?}", other),
    }
}

#[test]
fn get_file_without_placeholder_or_environment_is_unchanged() {
    let result = get_file("config.yaml", None);

    assert!(result.is_ok());
    let (file, env) = result.unwrap();
    assert_eq!(file, "config.yaml");
    assert_eq!(env, None);
}

#[test]
fn create_new_file_refuses_to_overwrite() {
    use std::fs;
    use crate::config::loader::create_new_file;
    use crate::test_util::temp_dir;

    let dir = temp_dir();
    let file = dir.path().join("once.yaml").to_string_lossy().into_owned();

    create_new_file(&file, "app:\n  port: 9090\n").unwrap();

    // The second caller is the process that lost the creation race. `fs::write`
    // truncates and would have replaced the winner's config with its own defaults;
    // `create_new` reports AlreadyExists and leaves the file untouched, which is what
    // lets `load_or_create` fall through to loading it.
    let err = create_new_file(&file, "app:\n  port: 8080\n").unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&file).unwrap(), "app:\n  port: 9090\n");
}

/// The names in `dir`, sorted — for asserting that no temporary file was left behind.
fn dir_entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn create_new_file_leaves_no_temporary_file_behind() {
    use crate::config::loader::create_new_file;
    use crate::test_util::temp_dir;

    let dir = temp_dir();
    let file = dir.path().join("once.yaml").to_string_lossy().into_owned();

    create_new_file(&file, "app:\n  port: 9090\n").unwrap();
    assert_eq!(dir_entries(dir.path()), ["once.yaml"]);

    // Losing the race removes the temporary file too, not only winning it
    create_new_file(&file, "app:\n  port: 8080\n").unwrap_err();
    assert_eq!(dir_entries(dir.path()), ["once.yaml"]);
}

#[test]
fn create_new_file_publishes_only_complete_contents() {
    use std::{cell::Cell, fs};
    use crate::config::loader::create_new_file_with;
    use crate::test_util::temp_dir;

    // Observed at the last moment before publication rather than by racing a reader
    // against the write, which finishes too fast to be caught reliably. Everything
    // before this point happens under another name, so a process that dies there, or a
    // write that fails there, leaves no config behind to be loaded instead of the defaults.
    const CONTENTS: &str = "app:\n  port: 9090\n";
    let dir = temp_dir();
    let file = dir.path().join("once.yaml").to_string_lossy().into_owned();
    let published = Cell::new(false);

    create_new_file_with(&file, CONTENTS, |temp, target| {
        assert!(!target.exists(), "the config appeared before it was published");
        assert_eq!(fs::read_to_string(temp).unwrap(), CONTENTS, "published before the write was complete");
        published.set(true);
        fs::hard_link(temp, target)
    })
    .unwrap();

    assert!(published.get(), "the file was not published through a link");
    assert_eq!(fs::read_to_string(&file).unwrap(), CONTENTS);
    assert_eq!(dir_entries(dir.path()), ["once.yaml"]);
}

#[test]
fn create_new_file_writes_in_place_when_links_are_unsupported() {
    use std::{fs, io};
    use crate::config::loader::create_new_file_with;
    use crate::test_util::temp_dir;

    let unsupported = |_: &std::path::Path, _: &std::path::Path| Err(io::Error::from(io::ErrorKind::Unsupported));
    let dir = temp_dir();
    let file = dir.path().join("fat.yaml").to_string_lossy().into_owned();

    create_new_file_with(&file, "app:\n  port: 9090\n", unsupported).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "app:\n  port: 9090\n");
    assert_eq!(dir_entries(dir.path()), ["fat.yaml"]);

    // The fallback is still exclusive: it reports the existing file rather than replacing it
    let err = create_new_file_with(&file, "app:\n  port: 8080\n", unsupported).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&file).unwrap(), "app:\n  port: 9090\n");
    assert_eq!(dir_entries(dir.path()), ["fat.yaml"]);
}

#[test]
fn to_string_test() {
    let parsed: Value = from_str(YAML).unwrap();
    let value = get_leaf(&parsed, "db/redis/port", "/").unwrap();
    let str_value = to_string(value);

    assert_eq!(str_value, "6379");
}
