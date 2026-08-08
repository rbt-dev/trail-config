//! Environment variable interpolation (`${VAR}`, `${VAR:-default}`).

use std::env;
use yaml_serde::{Value, value::TaggedValue};
use crate::error::ConfigError;

/// How deeply `${VAR:-default}` fallbacks may nest.
///
/// Each level recurses, so an unbounded chain — `${A:-${A:-${A:-…}}}` — would
/// overflow the stack and abort the process rather than surface an error. Two or
/// three fallback levels is the deepest sensible config, so this leaves ample room.
const MAX_DEFAULT_DEPTH: usize = 32;

/// Recursively walks the Value tree and resolves `${VAR}` and `${VAR:-default}`
/// placeholders in all string values using environment variables.
///
/// **Values only — never keys.** Each key is reinserted unchanged and only its value is
/// recursed into, so a `${VAR}` written as a key stays the literal text `${VAR}` and is
/// addressed by that text. This is deliberate: interpolating keys would make the set of
/// valid config *paths* depend on the environment, so a path that resolves on one machine
/// would silently miss on another, and an unset variable would turn into a missing key
/// rather than an error. It is stated here because this function's name implies otherwise.
///
/// The match is exhaustive rather than ending in a catch-all. `yaml_serde::Value` is not
/// `#[non_exhaustive]`, so this way a variant added upstream is a compile error here —
/// which is what a catch-all cost: `Value::Tagged` landed in it silently, and every
/// `${VAR}` under a `!Tag` went uninterpolated for as long as that arm existed.
pub(super) fn resolve_env_vars(value: Value) -> Result<Value, ConfigError> {
    match value {
        Value::String(s) => {
            let resolved = resolve_env_string(&s, 0)?;
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
        // A tag is how serde spells an enum variant in YAML (`db: !Postgres`), so a
        // tagged node is an ordinary subtree wearing a label — and every string under it
        // needs interpolating like any other. Skipping them did not merely leave a
        // `${VAR}` unsubstituted: it also disabled the one guarantee this function makes,
        // that a required variable which is not set stops the load. Under a tag, an unset
        // `${DB_PASSWORD}` silently became the literal text.
        //
        // The tag itself is deliberately left alone, by the same reasoning as keys above:
        // it selects a variant, so interpolating it would make the document's *shape*
        // depend on the environment.
        Value::Tagged(tagged) => {
            let TaggedValue { tag, value } = *tagged;
            Ok(Value::Tagged(Box::new(TaggedValue { tag, value: resolve_env_vars(value)? })))
        },
        // Nothing to interpolate in a scalar that is not a string.
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value),
    }
}

/// Resolves all `${VAR}` and `${VAR:-default}` placeholders in a single string.
///
/// `$${` escapes to a literal `${`. A `$` that does not begin `${` is literal, so
/// values like `$100` and `Pa$$w0rd!` pass through untouched.
///
/// `depth` is how many nested defaults deep this call is; the top-level call passes 0.
fn resolve_env_string(input: &str, depth: usize) -> Result<String, ConfigError> {
    let mut result = String::with_capacity(input.len());
    let mut rest = input;

    while !rest.is_empty() {
        // Everything up to the next '$' is literal
        match rest.find('$') {
            Some(0) => {},
            Some(at) => {
                let (literal, tail) = rest.split_at(at);
                result.push_str(literal);
                rest = tail;
            },
            None => {
                result.push_str(rest);
                break;
            },
        }

        if let Some(tail) = rest.strip_prefix("$${") {
            // Escaped — emit the placeholder syntax instead of expanding it.
            // Deliberately narrow: only `$${` is an escape, so a `$$` anywhere
            // else (a password, a shell snippet) is left exactly as written.
            result.push_str("${");
            rest = tail;
        } else if let Some(after) = rest.strip_prefix("${") {
            let (spec, tail) = split_placeholder(after, input)?;
            result.push_str(&resolve_placeholder(spec, input, depth)?);
            rest = tail;
        } else {
            // A '$' that does not begin a placeholder
            result.push('$');
            rest = &rest[1..];
        }
    }

    Ok(result)
}

/// Splits `after` — the text following an opening `${` — into the placeholder body
/// and the remainder after its matching `}`.
///
/// Counts nesting depth rather than stopping at the first `}`, so a placeholder may
/// contain a complete `${...}` of its own. Scanning by byte is safe because `$`, `{`
/// and `}` are ASCII, which never appear inside a multi-byte UTF-8 sequence.
fn split_placeholder<'a>(after: &'a str, input: &str) -> Result<(&'a str, &'a str), ConfigError> {
    let bytes = after.as_bytes();
    let mut depth = 1usize;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Ok((&after[..i], &after[i + 1..]));
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    Err(ConfigError::FormatError(
        format!("Unclosed env var placeholder in: {}", input)
    ))
}

/// Resolves one placeholder body — `VAR` or `VAR:-default`.
///
/// The default is itself run through [`resolve_env_string`], so defaults may nest:
/// `${A:-${B:-c}}` falls back through both levels. That recursion is capped at
/// [`MAX_DEFAULT_DEPTH`] levels so a pathological chain errors instead of overflowing
/// the stack.
///
/// A default applies only when the variable is **absent**. Both ways of being *set*
/// without yielding a usable string — empty, and not valid Unicode — are values rather
/// than absences, and neither falls back.
fn resolve_placeholder(spec: &str, input: &str, depth: usize) -> Result<String, ConfigError> {
    let (var_name, default) = match spec.find(":-") {
        Some(pos) => (&spec[..pos], Some(&spec[pos + 2..])),
        None => (spec, None),
    };

    if var_name.is_empty() {
        return Err(ConfigError::FormatError(
            format!("Empty env var name in: {}", input)
        ));
    }

    // Indirect names (`${${PREFIX}_HOST}`) would otherwise reach `env::var` as a
    // literal and fail with a confusing "not set" error
    if var_name.contains("${") {
        return Err(ConfigError::FormatError(format!(
            "Nested placeholder in the variable name of '${{{}}}' in: {} \
             — nesting is supported in defaults only",
            spec, input
        )));
    }

    match env::var(var_name) {
        // A variable that is set but empty is a value, not an absence: the default
        // applies only when the variable is missing. This differs from shell `:-`.
        Ok(value) => Ok(value),

        // Set, but holding bytes that are not valid Unicode. By the same reasoning as
        // set-but-empty, this is not an absence — so the default is deliberately *not*
        // consulted. Folding it in with `NotPresent` produced one of two wrong answers:
        // an error claiming the variable "is not set" when it demonstrably is, leaving
        // the operator nowhere to go; or, with a default, silently running the
        // deployment on the fallback while they believed their setting had taken.
        Err(env::VarError::NotUnicode(raw)) => Err(ConfigError::FormatError(format!(
            "Environment variable '{}' is set but is not valid Unicode ({:?}) \
             — the default, if any, is not applied because the variable is set",
            var_name, raw
        ))),

        Err(env::VarError::NotPresent) => match default {
            Some(_) if depth >= MAX_DEFAULT_DEPTH => Err(ConfigError::FormatError(format!(
                "Env var default nesting exceeds the maximum depth of {} at '{}' \
                 — check for a runaway ${{VAR:-...}} chain",
                MAX_DEFAULT_DEPTH, var_name
            ))),
            Some(default) => resolve_env_string(default, depth + 1),
            None => Err(ConfigError::FormatError(format!(
                "Environment variable '{}' is not set and no default provided",
                var_name
            ))),
        },
    }
}
