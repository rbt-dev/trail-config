//! Environment variable interpolation (`${VAR}`, `${VAR:-default}`).

use std::env;
use yaml_serde::Value;
use crate::error::ConfigError;

/// Recursively walks the Value tree and resolves `${VAR}` and `${VAR:-default}`
/// placeholders in all string values using environment variables.
pub(super) fn resolve_env_vars(value: Value) -> Result<Value, ConfigError> {
    match value {
        Value::String(s) => {
            let resolved = resolve_env_string(&s)?;
            Ok(Value::String(resolved))
        },
        Value::Mapping(map) => {
            let mut resolved_map = yaml_serde::Mapping::new();
            for (k, v) in map {
                resolved_map.insert(k, resolve_env_vars(v)?);
            }
            Ok(Value::Mapping(resolved_map))
        },
        Value::Sequence(seq) => {
            let resolved_seq: Result<Vec<Value>, ConfigError> =
                seq.into_iter().map(resolve_env_vars).collect();
            Ok(Value::Sequence(resolved_seq?))
        },
        other => Ok(other),
    }
}

/// Resolves all `${VAR}` and `${VAR:-default}` placeholders in a single string.
fn resolve_env_string(input: &str) -> Result<String, ConfigError> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut placeholder = String::new();
            let mut found_close = false;

            for c in chars.by_ref() {
                if c == '}' {
                    found_close = true;
                    break;
                }
                placeholder.push(c);
            }

            if !found_close {
                return Err(ConfigError::FormatError(
                    format!("Unclosed env var placeholder in: {}", input)
                ));
            }

            let (var_name, default) = match placeholder.find(":-") {
                Some(pos) => (&placeholder[..pos], Some(&placeholder[pos + 2..])),
                None => (placeholder.as_str(), None),
            };

            if var_name.is_empty() {
                return Err(ConfigError::FormatError(
                    format!("Empty env var name in: {}", input)
                ));
            }

            match env::var(var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => match default {
                    Some(d) => result.push_str(d),
                    None => return Err(ConfigError::FormatError(
                        format!("Environment variable '{}' is not set and no default provided", var_name)
                    )),
                }
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}
