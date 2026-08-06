//! Loading, layering, reloading and sharing — the crate used the way an application
//! uses it, with real files on disk.

mod common;

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use trail_config::{config, Config, ConfigHandle};

use common::{path_in, temp_dir, write_file};

#[test]
fn the_four_constructors_differ_only_in_how_they_treat_a_missing_file() {
    let dir = temp_dir();
    let present = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let absent = path_in(&dir, "absent.yaml");

    assert_eq!(Config::load_required(&present, "/", None).unwrap().get_int("app/port"), Some(8080));
    assert!(Config::load_required(&absent, "/", None).is_err());

    assert_eq!(Config::load_optional(&present, "/", None).unwrap().get_int("app/port"), Some(8080));
    let empty = Config::load_optional(&absent, "/", None).unwrap();
    assert_eq!(empty.get_int("app/port"), None);
    // ...but it remembers the file it looked for, so a later reload can pick it up
    assert_eq!(empty.get_filename(), absent);

    let created = path_in(&dir, "created.yaml");
    let config = Config::load_or_create(&created, "/", None, "app:\n  port: 1234\n").unwrap();
    assert_eq!(config.get_int("app/port"), Some(1234));
    assert_eq!(fs::read_to_string(&created).unwrap(), "app:\n  port: 1234\n");
}

#[test]
fn an_optional_config_picks_up_its_file_when_it_appears() {
    let dir = temp_dir();
    let file = path_in(&dir, "later.yaml");

    let mut config = Config::load_optional(&file, "/", None).unwrap();
    assert!(config.reload().is_err(), "still absent");
    assert_eq!(config.get_int("app/port"), None, "a failed reload leaves the config unchanged");

    fs::write(&file, "app:\n  port: 8080\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn the_env_placeholder_selects_a_file_and_is_recorded() {
    let dir = temp_dir();
    write_file(&dir, "config.prod.yaml", "app:\n  tier: production\n");
    let template = path_in(&dir, "config.{env}.yaml");

    let config = Config::load_required(&template, "/", Some("prod")).unwrap();

    assert_eq!(config.str("app/tier"), "production");
    assert_eq!(config.environment(), Some("prod"));
    // The resolved name is what is recorded, so reload() reads the same file
    assert!(config.get_filename().ends_with("config.prod.yaml"));

    // A placeholder with no environment to fill it is an error, not a file named "{env}"
    assert!(Config::load_required(&template, "/", None).is_err());
}

#[test]
fn overlays_merge_deeply_and_are_reapplied_on_reload() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  debug: false\n  name: myapp\ndb:\n  host: localhost\n  port: 5432\n");
    let prod = write_file(&dir, "config.prod.yaml", "db:\n  host: prodserver\n");
    let local = path_in(&dir, "config.local.yaml");

    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&prod, None).unwrap()
        .merge_optional(&local, None).unwrap();   // absent — silently skipped

    assert_eq!(config.str("db/host"), "prodserver");
    assert_eq!(config.get_int("db/port"), Some(5432), "sibling preserved by the deep merge");
    assert_eq!(config.get_bool("app/debug"), Some(false));

    // The optional overlay appearing later is picked up by reload, without re-declaring it
    fs::write(&local, "app:\n  debug: true\n").unwrap();
    config.reload().unwrap();
    assert_eq!(config.get_bool("app/debug"), Some(true));
    assert_eq!(config.str("db/host"), "prodserver", "required overlay still applied");
    assert_eq!(config.str("app/name"), "myapp", "base still applied");
}

#[test]
fn an_overlay_clears_a_value_with_null() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "db:\n  password: from-base\n  host: localhost\n");
    let overlay = write_file(&dir, "override.yaml", "db:\n  password:\n");

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_optional(&overlay, None).unwrap();

    assert_eq!(config.str("db/password"), "", "an overlay null clears the base value");
    assert_eq!(config.str("db/host"), "localhost");
}

#[test]
fn reload_from_switches_file_and_drops_the_overlay_chain() {
    let dir = temp_dir();
    let base = write_file(&dir, "base.yaml", "app:\n  port: 8080\n");
    let overlay = write_file(&dir, "overlay.yaml", "app:\n  port: 9090\n");
    let other = write_file(&dir, "other.yaml", "app:\n  port: 3000\n");

    let mut config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(9090));

    config.reload_from(&other).unwrap();
    assert_eq!(config.get_int("app/port"), Some(3000), "the overlay must not be re-applied");
    assert_eq!(config.get_filename(), other);
}

#[test]
fn a_failed_reload_leaves_the_configuration_unchanged() {
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");
    let mut config = Config::load_required(&file, "/", None).unwrap();

    fs::write(&file, "invalid: [unclosed\n").unwrap();
    assert!(config.reload().is_err());
    assert_eq!(config.get_int("app/port"), Some(8080), "the old document survives a failed reload");
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn the_handle_is_send_and_sync() {
    // The reason ConfigHandle exists — a property no unit test can state any better,
    // but one that a downstream `Arc<ConfigHandle>` in a web server depends on
    assert_send_sync::<ConfigHandle>();
}

#[test]
fn clones_of_a_handle_all_see_a_reload() {
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n");

    let handle = ConfigHandle::new(Config::load_required(&file, "/", None).unwrap());
    let clone = handle.clone();

    // A snapshot taken before the reload is stable across it
    let snapshot = handle.read();

    fs::write(&file, "app:\n  port: 9090\n").unwrap();
    handle.reload().unwrap();

    assert_eq!(clone.get_int("app/port"), Some(9090), "every clone sees the new config");
    assert_eq!(snapshot.get_int("app/port"), Some(8080), "the snapshot is immutable");
}

#[test]
fn a_handle_is_readable_from_many_threads_while_it_reloads() {
    let dir = temp_dir();
    let file = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let handle = ConfigHandle::new(Config::load_required(&file, "/", None).unwrap());

    fs::write(&file, "app:\n  port: 9090\n  name: myapp\n").unwrap();

    let barrier = Arc::new(Barrier::new(5));
    let mut threads = Vec::new();

    for i in 0..4 {
        let handle = handle.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..200 {
                // Whichever config a reader lands on, it is a complete one: the port is
                // one of the two written values and never a torn or missing read
                let port = handle.get_int("app/port").expect("config is never observed empty");
                assert!(port == 8080 || port == 9090, "thread {i} saw {port}");
                assert_eq!(handle.str("app/name"), "myapp");
            }
        }));
    }

    let reloader = {
        let handle = handle.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            for _ in 0..50 {
                handle.reload().expect("reload should succeed");
            }
        })
    };

    for thread in threads {
        thread.join().unwrap();
    }
    reloader.join().unwrap();

    assert_eq!(handle.get_int("app/port"), Some(9090));
}

#[test]
fn the_config_macro_works_from_another_crate() {
    // `$crate` paths inside an exported macro resolve differently outside the defining
    // crate, so this is the only place the macro's own expansion is really exercised
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  debug: false\n");
    let prod = write_file(&dir, "config.prod.yaml", "app:\n  debug: true\n");
    let absent = path_in(&dir, "config.local.yaml");

    let config = config!(&base).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));

    let config = config!(&base, sep: "::").unwrap();
    assert_eq!(config.get_int("app::port"), Some(8080));

    let config = config!(&base, merge: [&prod]).unwrap();
    assert_eq!(config.get_bool("app/debug"), Some(true));

    let config = config! {
        file: &base,
        sep: "/",
        merge: [&prod],
        merge_optional: [&absent],
    }.unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
    assert_eq!(config.get_bool("app/debug"), Some(true));
}

#[cfg(feature = "json")]
#[test]
fn formats_can_be_mixed_across_a_layered_load() {
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: myapp\n");
    let overlay = write_file(&dir, "override.json", r#"{"app": {"port": 9090}}"#);

    let config = Config::load_required(&base, "/", None).unwrap()
        .merge_required(&overlay, None).unwrap();

    assert_eq!(config.get_int("app/port"), Some(9090));
    assert_eq!(config.str("app/name"), "myapp");
}
