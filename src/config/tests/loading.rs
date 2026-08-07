use super::{Config, ConfigError, YAML};
use crate::test_util::{temp_dir, write_file, CwdGuard};

#[test]
fn yaml_parse_error() {
    let invalid_yaml = "invalid: [unclosed";
    let result = Config::load_yaml(invalid_yaml, "/");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError"),
    }
}

#[test]
fn invalid_yaml_formats() {
    let test_cases = vec![
        "invalid: {unclosed",
        "- item1\n - item2\n- item3\n : invalid",
        ": invalid_key",
    ];

    for invalid_yaml in test_cases {
        let result = Config::load_yaml(invalid_yaml, "/");
        assert!(result.is_err(), "Expected error for: {}", invalid_yaml);

        match result {
            Err(ConfigError::YamlError { .. }) => (),
            _ => panic!("Expected YamlError for: {}", invalid_yaml),
        }
    }
}

#[test]
fn empty_yaml() {
    let result = Config::load_yaml("", "/");

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
}

#[test]
fn empty_separator_in_load_optional() {
    let result = Config::load_optional("config.yaml", "", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
}

#[test]
fn empty_separator_in_load_yaml() {
    let result = Config::load_yaml(YAML, "");

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("empty")),
        _ => panic!("Expected FormatError for empty separator"),
    }
}

#[test]
fn backslash_separator_is_rejected() {
    // `\` is the escape character in path syntax, so the splitter consumes it before
    // it can match as a separator: every path collapsed to one segment and every
    // lookup silently returned None, with no error raised anywhere
    let result = Config::load_yaml("a:\n  b: 42", "\\");

    assert!(result.is_err());
    match result {
        Err(ConfigError::FormatError(msg)) => assert!(msg.contains("backslash"), "got: {}", msg),
        _ => panic!("Expected FormatError for backslash separator"),
    }
}

#[test]
fn separator_containing_a_backslash_is_rejected() {
    // Rejected wherever the backslash sits, not just in leading position
    for sep in ["\\", "\\::", "a\\b", "->\\"] {
        let result = Config::load_yaml("a:\n  b: 42", sep);
        match result {
            Err(ConfigError::FormatError(msg)) => assert!(msg.contains("backslash"), "got: {}", msg),
            _ => panic!("Expected FormatError for separator {:?}", sep),
        }
    }
}

#[test]
fn backslash_separator_is_rejected_by_every_constructor() {
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "a:\n  b: 42\n");

    assert!(Config::load_required(&file, "\\", None).is_err());
    assert!(Config::load_optional(&file, "\\", None).is_err());
    assert!(Config::load_or_create(&file, "\\", None, "a:\n  b: 1\n").is_err());
}

#[test]
fn unusual_separators_still_work() {
    // Only the escape character collides — multi-character and brace-bearing
    // separators are unaffected
    for (sep, path) in [("::", "a::b"), ("->", "a->b"), ("{env}", "a{env}b")] {
        let config = Config::load_yaml("a:\n  b: 42", sep).unwrap();
        assert_eq!(config.get_int(path), Some(42), "separator {:?}", sep);
    }
}

#[test]
fn load_optional_missing_file_returns_empty_config() {
    let result = Config::load_optional("nonexistent_file_12345.yaml", "/", None);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(!config.contains("any/path"));
    assert_eq!(config.str("any/path"), "");
}

#[test]
fn load_optional_invalid_yaml_returns_error() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "invalid.yaml", "invalid: [unclosed\n");

    let result = Config::load_optional(&test_file, "/", None);
    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for malformed file"),
    }
}

#[test]
fn load_or_create_creates_file_when_missing() {
    use std::fs;

    const DEFAULTS: &str = "app:\n  port: 8080\n  debug: false\n";
    let dir = temp_dir();
    let test_file = dir.path().join("new.yaml").to_string_lossy().into_owned();

    let result = Config::load_or_create(&test_file, "/", None, DEFAULTS);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.str("app/port"), "8080");
    assert_eq!(config.get_bool("app/debug"), Some(false));

    assert!(fs::metadata(&test_file).is_ok());
    let written = fs::read_to_string(&test_file).unwrap();
    assert_eq!(written, DEFAULTS);
}

#[test]
fn load_or_create_loads_existing_file() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "existing.yaml", "app:\n  port: 9090\n");

    let config = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n").unwrap();
    assert_eq!(config.str("app/port"), "9090");
}

#[test]
fn load_or_create_waits_out_a_file_that_is_still_being_written() {
    use std::{fs, thread, time::Duration};

    // The first-run race, staged deterministically. A zero-length file is exactly what
    // the winner of `create_new` leaves behind between creating it and filling it, and a
    // loser arriving in that gap used to read nothing and return an empty config —
    // silently, with the defaults discarded because the file "existed".
    let dir = temp_dir();
    let path = dir.path().join("config.yaml");
    fs::write(&path, "").unwrap();
    let file = path.to_string_lossy().into_owned();

    // Stands in for the winner completing its write, inside the settle window.
    let writer = {
        let path = path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            fs::write(&path, "app:\n  port: 9090\n").unwrap();
        })
    };

    let config = Config::load_or_create(&file, "/", None, "app:\n  port: 8080\n").unwrap();
    writer.join().unwrap();

    // The winner's document, not the defaults and not an empty config
    assert_eq!(config.get_int("app/port"), Some(9090));
}

#[test]
fn load_or_create_accepts_a_file_that_stays_empty() {
    use std::{fs, time::Instant};

    // The other side of the wait: nothing arrives, so the empty file is honoured rather
    // than overwritten with the defaults. `load_or_create` loads a file that exists.
    let dir = temp_dir();
    let path = dir.path().join("config.yaml");
    fs::write(&path, "").unwrap();
    let file = path.to_string_lossy().into_owned();

    let started = Instant::now();
    let config = Config::load_or_create(&file, "/", None, "app:\n  port: 8080\n").unwrap();

    assert_eq!(config.get_int("app/port"), None, "the empty file wins, not the defaults");
    assert_eq!(fs::read_to_string(&path).unwrap(), "", "and is left as it was");
    assert!(started.elapsed().as_millis() >= 100, "it was waited on, not accepted at once");
}

#[test]
fn load_or_create_does_not_wait_on_a_comment_only_file() {
    use std::time::Instant;

    // A comment-only document also parses to nothing, but it is not zero-length, so it is
    // not what a half-written file looks like and must not pay the wait.
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "# nothing here yet\n");

    let started = Instant::now();
    let config = Config::load_or_create(&file, "/", None, "app:\n  port: 8080\n").unwrap();

    assert_eq!(config.get_int("app/port"), None);
    assert!(started.elapsed().as_millis() < 100, "took {:?}", started.elapsed());
}

#[test]
fn load_or_create_does_not_wait_when_the_defaults_are_empty() {
    use std::{fs, time::Instant};

    // With nothing better to substitute, an empty file is already the right answer.
    let dir = temp_dir();
    let path = dir.path().join("config.yaml");
    fs::write(&path, "").unwrap();
    let file = path.to_string_lossy().into_owned();

    let started = Instant::now();
    let config = Config::load_or_create(&file, "/", None, "").unwrap();

    assert_eq!(config.get_int("app/port"), None);
    assert!(started.elapsed().as_millis() < 100, "took {:?}", started.elapsed());
}

#[test]
fn load_or_create_does_not_wait_on_a_file_with_content() {
    use std::time::Instant;

    // The ordinary path, pinned so the settle can never creep onto it.
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 9090\n");

    let started = Instant::now();
    let config = Config::load_or_create(&file, "/", None, "app:\n  port: 8080\n").unwrap();

    assert_eq!(config.get_int("app/port"), Some(9090));
    assert!(started.elapsed().as_millis() < 100, "took {:?}", started.elapsed());
}

#[test]
fn load_or_create_supports_reload() {
    use std::fs;

    let dir = temp_dir();
    let test_file = dir.path().join("new.yaml").to_string_lossy().into_owned();

    let mut config = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n").unwrap();
    assert_eq!(config.filename(), test_file);
    assert_eq!(config.str("app/port"), "8080");

    // The created config records its filename, so reload() picks up edits
    fs::write(&test_file, "app:\n  port: 9090\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.str("app/port"), "9090");
}

#[test]
fn load_or_create_invalid_defaults_returns_error() {
    let dir = temp_dir();
    let test_file = dir.path().join("invalid_defaults.yaml").to_string_lossy().into_owned();

    let result = Config::load_or_create(&test_file, "/", None, "invalid: [unclosed");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for invalid defaults"),
    }
}

#[test]
fn load_or_create_invalid_defaults_leaves_no_file_behind() {
    use std::fs;

    // The failure that made this permanent: the broken defaults were written first and
    // parsed second, so the file existed on every later run and the create branch was
    // never taken again — the same error forever, indistinguishable from a hand-edited
    // broken config.
    let dir = temp_dir();
    let test_file = dir.path().join("wedged.yaml").to_string_lossy().into_owned();

    assert!(Config::load_or_create(&test_file, "/", None, "invalid: [unclosed").is_err());
    assert!(!fs::exists(&test_file).unwrap(), "no file should have been written");

    // So a corrected second call still creates the file, rather than re-reading a broken one
    let config = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn load_or_create_invalid_existing_file_returns_error() {
    let dir = temp_dir();
    let test_file = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");

    let result = Config::load_or_create(&test_file, "/", None, "app:\n  port: 8080\n");

    assert!(result.is_err());
    match result {
        Err(ConfigError::YamlError { .. }) => (),
        _ => panic!("Expected YamlError for broken existing file"),
    }
}

#[test]
fn load_required_file_not_found() {
    let result = Config::load_required("nonexistent_file_xyz.yaml", "/", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_with_env() {
    let result = Config::load_required("config_{env}.yaml", "/", Some("dev"));

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { .. }) => (),
        _ => panic!("Expected IoError for missing file"),
    }
}

#[test]
fn load_required_rejects_empty_filename() {
    let config = Config::load_required("", "/", None);

    assert!(config.is_err());
    match config {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename"),
    }
}

#[test]
fn load_optional_rejects_empty_filename() {
    // Previously this silently returned an empty config: reading an empty path
    // yields NotFound, which load_optional treated as a missing optional file.
    // An empty filename is a caller bug and must be rejected upfront.
    let result = Config::load_optional("", "/", None);

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename, got {:?}", result),
    }
}

#[test]
fn load_or_create_rejects_empty_filename() {
    let result = Config::load_or_create("", "/", None, "app:\n  port: 8080\n");

    assert!(result.is_err());
    match result {
        Err(ConfigError::IoError { source, .. }) => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
        },
        _ => panic!("Expected IoError(InvalidInput) for empty filename, got {:?}", result),
    }
}

#[test]
fn all_loaders_reject_empty_filename_uniformly() {
    for result in [
        Config::load_required("", "/", None),
        Config::load_optional("", "/", None),
        Config::load_or_create("", "/", None, "app:\n  port: 1\n"),
    ] {
        match result {
            Err(ConfigError::IoError { source, .. }) => {
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
            },
            other => panic!("Expected IoError(InvalidInput), got {:?}", other),
        }
    }
}

#[test]
fn error_display_messages() {
    let io_err = ConfigError::IoError {
        file: Some("config.yaml".to_string()),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "test file not found"),
    };
    assert!(io_err.to_string().contains("IO error"));
    assert!(io_err.to_string().contains("config.yaml"));

    let yaml_err = Config::load_yaml("invalid: [unclosed", "/").unwrap_err();
    assert!(yaml_err.to_string().contains("YAML parse error"));

    let path_err = ConfigError::PathNotFound("db/missing/key".to_string());
    assert!(path_err.to_string().contains("Path not found"));

    let fmt_err = ConfigError::FormatError("invalid format".to_string());
    assert!(fmt_err.to_string().contains("Format error"));
}

#[test]
fn load_errors_carry_filename_and_source() {
    use std::error::Error as _;

    // Parse error from a file: Display names the file, source() is the parser error
    let dir = temp_dir();
    let path = write_file(&dir, "broken.yaml", "invalid: [unclosed\n");
    let err = Config::load_required(&path, "/", None).unwrap_err();
    assert!(err.to_string().contains("broken.yaml"));
    assert!(err.source().is_some());
    match &err {
        ConfigError::YamlError { file: Some(f), .. } => assert!(f.contains("broken.yaml")),
        other => panic!("Expected YamlError with file, got: {:?}", other),
    }

    // IO error: same
    let err = Config::load_required("no_such_file_xyz.yaml", "/", None).unwrap_err();
    assert!(err.to_string().contains("no_such_file_xyz.yaml"));
    assert!(err.source().is_some());

    // Parse error from a string: no file recorded
    let err = Config::load_yaml("invalid: [unclosed", "/").unwrap_err();
    match &err {
        ConfigError::YamlError { file: None, .. } => (),
        other => panic!("Expected YamlError without file, got: {:?}", other),
    }
    assert!(err.source().is_some());
}

#[test]
fn default_returns_empty_config_when_file_missing() {
    // config.yaml is hardcoded as CWD-relative; isolate via a temp CWD.
    let dir = temp_dir();
    let _cwd = CwdGuard::new(dir.path());

    let config = Config::default();
    assert!(!config.contains("any/path"));
    assert_eq!(config.str("any/path"), "");
}

#[test]
fn default_loads_valid_config_yaml_from_cwd() {
    let dir = temp_dir();
    write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let _cwd = CwdGuard::new(dir.path());

    let config = Config::default();
    assert_eq!(config.str("app/port"), "8080");
}

#[test]
#[should_panic(expected = "Config::default() failed to load config.yaml")]
fn default_panics_on_broken_config_yaml_in_cwd() {
    // Previously a broken config.yaml was silently swallowed into an empty
    // config; it must now panic to surface the mistake. CwdGuard restores the
    // working directory even though default() panics (drop runs on unwind).
    let dir = temp_dir();
    write_file(&dir, "config.yaml", "invalid: [unclosed\n");
    let _cwd = CwdGuard::new(dir.path());

    let _ = Config::default();
}


#[test]
fn load_or_create_does_not_create_parent_directories() {
    // Documented: only the file itself is created. A missing parent is reported
    // rather than conjured, so a typo'd path cannot leave junk directories behind.
    let dir = temp_dir();
    let nested = dir.path().join("sub").join("config.yaml").to_string_lossy().into_owned();

    let result = Config::load_or_create(&nested, "/", None, "app:\n  port: 1\n");

    match result {
        Err(ConfigError::IoError { .. }) => {},
        other => panic!("Expected IoError for a missing parent directory, got {:?}", other.err()),
    }
    assert!(!dir.path().join("sub").exists(), "no directory should have been created");
}
