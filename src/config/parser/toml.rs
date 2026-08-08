use yaml_serde::{Mapping, Value};
use crate::error::ConfigError;
use std::fs;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let content = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&content, filename)
}

pub(crate) fn parse(toml_str: &str) -> Result<Value, ConfigError> {
    parse_internal(toml_str, None)
}

/// Parses TOML that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(toml_str: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(toml_str, Some(filename))
}

fn parse_internal(toml_str: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // `toml` skips a BOM already; stripping here anyway keeps the rule uniform across
    // the three formats rather than dependent on each parser's leniency. See
    // `super::strip_bom`.
    let toml_str = super::strip_bom(toml_str);
    let toml_value: toml::Value = toml::from_str(toml_str)
        .map_err(|e| ConfigError::toml_in(file, e))?;
    Ok(convert(toml_value))
}

/// Converts a parsed TOML document into the crate's value model.
///
/// Written out by hand rather than routed through `yaml_serde::to_value`, because that
/// path goes through serde — and TOML's datetime has no place in the serde data model.
/// `toml_datetime` works around that by serializing a datetime as a one-field struct
/// with a private marker name, expecting its own deserializer to recognise it on the way
/// back. `yaml_serde` has never heard of the marker, so it faithfully materialized the
/// workaround: `started = 2024-01-01T00:00:00Z` became the mapping
/// `{"$__toml_private_datetime": "2024-01-01T00:00:00Z"}`, and a dependency's internal
/// protocol became an addressable config path. `str` returned `""` for a value plainly
/// present in the file, `str_strict` called it "not a scalar", deserializing into a
/// struct failed with "invalid type: map, expected a string", and `outline` printed the
/// marker as though the user had written it.
///
/// Converting explicitly also drops a fallible step: `to_value` returned a `Result`, so a
/// TOML file could fail with a `YamlError`. Nothing here can fail, and TOML parsing now
/// reports `TomlError` or nothing at all.
///
/// The match is deliberately exhaustive. `toml::Value` is not `#[non_exhaustive]`, so a
/// variant added upstream breaks this build rather than silently taking a catch-all arm —
/// which is precisely how the datetime case went unnoticed.
fn convert(value: toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => Value::Number(f.into()),
        toml::Value::Boolean(b) => Value::Bool(b),
        // TOML's fourth scalar type, and the one the YAML value model has no counterpart
        // for. Surfaced as the text the file contained — RFC 3339 for an offset
        // date-time, and the same date/time/date-time forms TOML itself writes for the
        // local variants. That makes it a scalar like any other: readable with `str`,
        // listed by `outline`, and deserializable into the types callers actually reach
        // for, `String` and `chrono`/`time`/`jiff`'s date types, all of which parse
        // RFC 3339. The one thing it is no longer deserializable into is
        // `toml::value::Datetime`, which needs the marker mapping its own `Deserialize`
        // impl looks for — a trade for a type that can only be named by taking a direct
        // dependency on `toml`, which this crate's docs already advise against.
        toml::Value::Datetime(datetime) => Value::String(datetime.to_string()),
        toml::Value::Array(items) => Value::Sequence(items.into_iter().map(convert).collect()),
        // Document order, not alphabetical: `Cargo.toml` enables the `toml` crate's
        // `preserve_order`, which backs `toml::map::Map` with an `IndexMap`. Without that
        // flag every `.toml` config's keys arrive sorted, and `outline`'s "keys appear in
        // document order" and the merge's key-order preservation are untrue for this
        // format alone — see the note beside the dependency in `Cargo.toml`. Iterating
        // `toml::Value` therefore preserves what the file wrote, and collecting into
        // `Mapping` (also `IndexMap`-backed) keeps it.
        //
        // The JSON side of the same problem is answered differently — `super::json` parses
        // straight into this value model rather than enabling `serde_json`'s
        // `preserve_order`, because that feature would unify across the whole dependency
        // graph. TOML's parse has to go through `toml::Value` for the datetime handling
        // above, so the flag is the only route here.
        toml::Value::Table(table) => Value::Mapping(
            table.into_iter()
                .map(|(key, value)| (Value::String(key), convert(value)))
                .collect::<Mapping>(),
        ),
    }
}
