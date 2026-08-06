use yaml_serde::Value;
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&content, filename)
}

pub(crate) fn parse(json: &str) -> Result<Value, ConfigError> {
    parse_internal(json, None)
}

/// Parses JSON that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(json: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(json, Some(filename))
}

/// Parses JSON straight into the crate's value model.
///
/// Deliberately *not* via `serde_json::Value`. That type's object map is a `BTreeMap`
/// unless `serde_json`'s `preserve_order` feature is on, so routing through it sorted
/// every JSON config's keys alphabetically before this crate ever saw the document —
/// making `outline`'s "keys appear in document order" and the merge's key-order
/// preservation untrue for `.json` files. Deserializing directly lands the keys in
/// `yaml_serde::Mapping`, which is `IndexMap`-backed, in the order the file lists them.
///
/// Enabling `preserve_order` would have worked too, and is the usual answer, but cargo
/// features are additive across the whole dependency graph: switching it on here would
/// silently change `serde_json::Value`'s iteration order for every other crate in a
/// consumer's build. Not this crate's call to make.
///
/// Going direct also drops the intermediate document and a fallible step with it —
/// `to_value` returned a `Result`, so a `.json` file could fail with a `YamlError`.
fn parse_internal(json: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // `serde_json` is the one parser of the three that rejects a BOM rather than
    // skipping it, so without this the same bytes load as YAML and TOML and fail here.
    // See `super::strip_bom`.
    let json = super::strip_bom(json);
    // `from_str`, not a hand-driven `Deserializer`: it checks that nothing follows the
    // document, so trailing junk is still rejected.
    serde_json::from_str(json).map_err(|e| ConfigError::json_in(file, e))
}
