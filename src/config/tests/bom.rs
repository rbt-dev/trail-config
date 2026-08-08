//! A leading UTF-8 BOM is stripped, identically for every format.
//!
//! One test per format on purpose. `serde_json` was the only parser of the three that
//! rejected a BOM — `yaml_serde` and `toml` skip it — so a JSON-only test would pass
//! over a fix placed anywhere, including one that leaves the rule stated per-format and
//! drifting. These assert the three agree about the same bytes.

use crate::config::Config;
use crate::test_util::{temp_dir, write_file};

/// U+FEFF, encoded `EF BB BF` in the UTF-8 that lands on disk. This is what
/// PowerShell's `>`, `>>` and `Out-File` prepend by default.
const BOM: &str = "\u{feff}";

#[test]
fn a_yaml_file_with_a_bom_loads() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.yaml", &format!("{BOM}app:\n  port: 8080\n"));

    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn a_yaml_string_with_a_bom_loads() {
    let config = Config::load_yaml(&format!("{BOM}app:\n  port: 8080\n"), "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[cfg(feature = "json")]
#[test]
fn a_json_file_with_a_bom_loads() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.json", &format!("{BOM}{{\"app\": {{\"port\": 8080}}}}"));

    // The reported failure: "JSON parse error in ...: expected value at line 1
    // column 1", naming a file that looks correct in every editor.
    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[cfg(feature = "json")]
#[test]
fn a_json_string_with_a_bom_loads() {
    let config = Config::load_json(&format!("{BOM}{{\"app\": {{\"port\": 8080}}}}"), "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[cfg(feature = "toml")]
#[test]
fn a_toml_file_with_a_bom_loads() {
    let dir = temp_dir();
    let path = write_file(&dir, "config.toml", &format!("{BOM}[app]\nport = 8080\n"));

    let config = Config::load_required(&path, "/", None).unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[cfg(feature = "toml")]
#[test]
fn a_toml_string_with_a_bom_loads() {
    let config = Config::load_toml(&format!("{BOM}[app]\nport = 8080\n"), "/").unwrap();
    assert_eq!(config.get_int("app/port"), Some(8080));
}

#[test]
fn the_three_formats_agree_about_the_same_bom_prefixed_document() {
    // The point of the fix stated directly: one document, three spellings, three
    // extensions — all of them load, where previously only `.json` failed.
    let dir = temp_dir();

    let yaml = write_file(&dir, "a.yaml", &format!("{BOM}app:\n  port: 8080\n"));
    assert_eq!(
        Config::load_required(&yaml, "/", None).unwrap().get_int("app/port"),
        Some(8080)
    );

    #[cfg(feature = "json")]
    {
        let json = write_file(&dir, "a.json", &format!("{BOM}{{\"app\": {{\"port\": 8080}}}}"));
        assert_eq!(
            Config::load_required(&json, "/", None).unwrap().get_int("app/port"),
            Some(8080)
        );
    }

    #[cfg(feature = "toml")]
    {
        let toml = write_file(&dir, "a.toml", &format!("{BOM}[app]\nport = 8080\n"));
        assert_eq!(
            Config::load_required(&toml, "/", None).unwrap().get_int("app/port"),
            Some(8080)
        );
    }
}

#[test]
fn only_a_leading_bom_is_stripped() {
    // U+FEFF elsewhere is a legitimate (if deprecated) zero-width no-break space and
    // belongs to the document. Stripping it wherever it appeared would silently edit
    // a config value.
    let config = Config::load_yaml(&format!("{BOM}app:\n  name: a{BOM}b\n"), "/").unwrap();
    assert_eq!(config.str("app/name"), format!("a{BOM}b"));
}

#[test]
fn a_bom_on_an_overlay_is_stripped_too() {
    // The overlay path parses through the same choke point, but it is a separate call
    // site — and a config written by a setup script is exactly the kind of file that
    // ends up as an overlay.
    let dir = temp_dir();
    let base = write_file(&dir, "config.yaml", "app:\n  port: 8080\n  name: base\n");
    let over = write_file(&dir, "over.yaml", &format!("{BOM}app:\n  name: overlaid\n"));

    let config = Config::load_required(&base, "/", None)
        .unwrap()
        .merge_required(&over, None)
        .unwrap();

    assert_eq!(config.str("app/name"), "overlaid");
    assert_eq!(config.get_int("app/port"), Some(8080));
}
