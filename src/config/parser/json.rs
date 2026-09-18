use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::value::RawValue;
use yaml_serde::{Mapping, Value};
use crate::error::ConfigError;
use std::{fmt, fs};

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

/// Raw tokens keep numbers independent of serde_json's unified dependency features.
/// In particular, `arbitrary_precision` exposes numbers to generic value visitors as
/// private marker maps. Interpreting those maps would also misinterpret real user keys.
fn parse_internal(json: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // `serde_json` is the one parser of the three that rejects a BOM rather than
    // skipping it, so without this the same bytes load as YAML and TOML and fail here.
    // See `super::strip_bom`.
    let json = super::strip_bom(json);
    let result = (|| {
        // Validate the complete document before converting its borrowed tokens.
        let raw: &RawValue = serde_json::from_str(json)?;
        parse_raw(raw, 128).map_err(|error| rebase_error(error, json, raw))
    })();
    result.map_err(|e| ConfigError::json_in(file, e))
}

fn parse_raw(raw: &RawValue, remaining: usize) -> Result<Value, serde_json::Error> {
    let text = raw.get();
    match text.as_bytes()[0] {
        b'{' | b'[' => {
            // RawValue skips containers without enforcing the deserializer's usual
            // recursion limit. Keep an explicit bound across the borrowed subparsers.
            if remaining == 0 {
                return Err(de::Error::custom("recursion limit exceeded"));
            }
            let mut deserializer = serde_json::Deserializer::from_str(text);
            let visitor = Container { text, remaining: remaining - 1 };
            if text.starts_with('{') {
                serde::Deserializer::deserialize_map(&mut deserializer, visitor)
            } else {
                serde::Deserializer::deserialize_seq(&mut deserializer, visitor)
            }
        },
        b'"' => serde_json::from_str(text).map(Value::String),
        b't' | b'f' => serde_json::from_str(text).map(Value::Bool),
        b'n' => Ok(Value::Null),
        _ => {
            // Keep integers exact where the value model supports them. Decimal,
            // exponent and larger integer tokens use finite f64, as in the default
            // serde_json parser. Preserve negative zero rather than converting to 0.
            if text != "-0" {
                if let Ok(number) = text.parse::<u64>() {
                    return Ok(Value::Number(number.into()));
                }
                if let Ok(number) = text.parse::<i64>() {
                    return Ok(Value::Number(number.into()));
                }
            }
            // Typed f64 deserialization bypasses the arbitrary-precision marker and
            // rejects overflow instead of admitting infinity or a marker mapping.
            serde_json::from_str::<f64>(text).map(|number| Value::Number(number.into()))
        },
    }
}

struct Container<'a> {
    text: &'a str,
    remaining: usize,
}

impl<'de> DeserializeSeed<'de> for &Container<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?;
        parse_raw(raw, self.remaining)
            .map_err(|error| de::Error::custom(rebase_error(error, self.text, raw)))
    }
}

fn rebase_error(error: serde_json::Error, parent: &str, raw: &RawValue) -> serde_json::Error {
    // RawValue borrows a slice of parent. Serde JSON's custom error conversion
    // preserves its standard "at line ... column ..." location suffix.
    let offset = raw.get().as_ptr() as usize - parent.as_ptr() as usize;
    if offset == 0 {
        return error;
    }
    let prefix = &parent[..offset];
    let lines = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line = lines + error.line().max(1);
    let column = if error.line() <= 1 {
        prefix.rsplit('\n').next().unwrap_or_default().len() + error.column().max(1)
    } else {
        error.column()
    };
    let message = error.to_string();
    let suffix = format!(" at line {} column {}", error.line(), error.column());
    let message = message.strip_suffix(&suffix).unwrap_or(&message);
    de::Error::custom(format!("{message} at line {line} column {column}"))
}

impl<'de> Visitor<'de> for Container<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object or array")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        let mut values = Mapping::new();
        while let Some(key) = map.next_key::<String>()? {
            let key = Value::String(key);
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let value = map.next_value_seed(&self)?;
            values.insert(key, value);
        }
        Ok(Value::Mapping(values))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Value, S::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(&self)? {
            values.push(value);
        }
        Ok(Value::Sequence(values))
    }
}
