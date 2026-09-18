mod common;

use std::{fs, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use trail_config::{Config, ConfigError, ConfigHandle};
use common::{temp_dir, write_file, path_in};

fn valid_port(config: &Config) -> Result<(), ConfigError> {
    let port = config.get_as_strict::<u16>("port")?;
    if port == 0 {
        return Err(ConfigError::FormatError("port must be nonzero".into()));
    }
    Ok(())
}

#[test]
fn initial_validation_rejects_types_and_domain_errors() {
    for yaml in ["port: broken", "port: 0"] {
        assert!(ConfigHandle::with_validator(Config::load_yaml(yaml, "/").unwrap(), valid_port).is_err());
    }
}

#[test]
fn validator_panic_preserves_snapshot_and_later_reload_recovers() {
    let dir = temp_dir();
    let file = write_file(&dir, "base.yaml", "port: 8080");
    let handle = ConfigHandle::with_validator(Config::load_required(&file, "/", None).unwrap(), |candidate| {
        assert_ne!(candidate.get_int("port"), Some(0), "validation panic");
        valid_port(candidate)
    }).unwrap();
    let before = handle.read();
    fs::write(&file, "port: 0").unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.reload())).is_err());
    assert!(Arc::ptr_eq(&before, &handle.read()));
    fs::write(&file, "port: 9090").unwrap();
    handle.reload().unwrap();
    assert_eq!(handle.get_int("port"), Some(9090));
}

#[test]
fn validator_errors_are_returned_unchanged_and_plain_handles_remain_unvalidated() {
    let dir = temp_dir();
    let file = write_file(&dir, "base.yaml", "port: 8080");
    let handle = ConfigHandle::with_validator(Config::load_required(&file, "/", None).unwrap(), valid_port).unwrap();
    let plain = ConfigHandle::new(Config::load_required(&file, "/", None).unwrap());
    fs::write(&file, "port: 0").unwrap();
    match handle.reload().unwrap_err() {
        ConfigError::FormatError(message) => assert_eq!(message, "port must be nonzero"),
        other => panic!("unexpected error: {other:?}"),
    }
    plain.reload().unwrap();
    assert_eq!(plain.get_int("port"), Some(0));
}

#[test]
fn cloned_handle_rejects_invalid_reload_and_preserves_snapshots() {
    let dir = temp_dir();
    let file = write_file(&dir, "base.yaml", "port: 8080");
    let handle = ConfigHandle::with_validator(Config::load_required(&file, "/", None).unwrap(), valid_port).unwrap();
    let clone = handle.clone();
    let before = handle.read();
    for yaml in ["port: broken", "port: 0"] {
        fs::write(&file, yaml).unwrap();
        assert!(clone.reload().is_err());
        assert!(Arc::ptr_eq(&before, &handle.read()));
    }
    fs::write(&file, "port: 9090").unwrap();
    clone.reload().unwrap();
    assert_eq!(handle.get_int("port"), Some(9090));
    assert_eq!(before.get_int("port"), Some(8080));
}

#[test]
fn failed_switch_preserves_sources_and_successful_switch_clears_overlays() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "port: 0");
    let overlay = write_file(&dir, "overlay.yaml", "port: 8080");
    let other = write_file(&dir, "other.yaml", "port: 0");
    let config = Config::load_required(&base, "/", None).unwrap().merge_required(&overlay, None).unwrap();
    let handle = ConfigHandle::with_validator(config, valid_port).unwrap();
    let before = handle.read();
    assert!(handle.reload_from(&other).is_err());
    assert!(Arc::ptr_eq(&before, &handle.read()));
    fs::write(&overlay, "port: 9090").unwrap();
    handle.reload().unwrap();
    assert_eq!(handle.get_int("port"), Some(9090));
    assert_eq!(handle.read().filename(), base);
    fs::write(&other, "port: 1234").unwrap();
    handle.reload_from(&other).unwrap();
    handle.reload().unwrap();
    assert_eq!(handle.get_int("port"), Some(1234));
    assert_eq!(handle.read().filename(), other);
}

#[test]
fn validator_sees_merged_and_interpolated_candidate_and_skips_load_errors() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "port: bad");
    let overlay = write_file(&dir, "overlay.yaml", "port: '${TRAIL_VALIDATION_TEST_UNSET:-8080}'");
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let config = Config::load_required(&base, "/", None).unwrap().merge_required(&overlay, None).unwrap();
    let handle = ConfigHandle::with_validator(config, move |candidate| {
        calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(candidate.str_strict("port")?, "8080");
        Ok(())
    }).unwrap();
    handle.reload().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    fs::write(&overlay, "bad: [").unwrap();
    assert!(handle.reload().is_err());
    assert!(handle.reload_from(&path_in(&dir, "absent.yaml")).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn readers_can_read_while_validation_waits() {
    use std::{sync::{mpsc, Mutex}, thread, time::Duration};
    let dir = temp_dir();
    let file = write_file(&dir, "base.yaml", "port: 8080");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let handle = ConfigHandle::with_validator(Config::load_required(&file, "/", None).unwrap(), move |candidate| {
        if candidate.get_int("port") == Some(9090) {
            entered_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
        }
        Ok(())
    }).unwrap();
    fs::write(&file, "port: 9090").unwrap();
    let worker_handle = handle.clone();
    let worker = thread::spawn(move || worker_handle.reload());
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let reader = handle.clone();
    let (read_tx, read_rx) = mpsc::channel();
    let reading = thread::spawn(move || read_tx.send(reader.get_int("port")).unwrap());
    let observed = read_rx.recv_timeout(Duration::from_secs(5));
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    reading.join().unwrap();
    assert_eq!(observed.unwrap(), Some(8080));
    assert_eq!(handle.get_int("port"), Some(9090));
}
