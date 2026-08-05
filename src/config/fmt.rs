//! Formatting sibling config values into a string template.

use crate::error::ConfigError;
use super::Config;
use super::accessor::to_string;
use super::path::parse_path;

/// One resolved piece of a format string.
///
/// `Literal` borrows from the format string; `{{` and `}}` yield the static
/// `"{"` / `"}"`, so unescaping costs no allocation.
enum Piece<'a> {
    Literal(&'a str),
    /// Index into the caller's `keys` slice.
    Value(usize),
}

impl Config {
    /// Formats a string template with values from the config
    ///
    /// See [`fmt_strict`](Config::fmt_strict) for the placeholder syntax. Returns an
    /// empty string if the template is invalid or any value is missing.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt("{}:{}", "db/redis", &["server", "port"]);
    /// assert_eq!(result, "127.0.0.1:6379");
    /// ```
    pub fn fmt(&self, format: &str, base: &str, keys: &[&str]) -> String {
        self.fmt_strict(format, base, keys).unwrap_or_else(|_| String::new())
    }

    /// Formats a string template with values from the config, returning an error if the
    /// template is invalid or any value is missing.
    ///
    /// # Placeholders
    ///
    /// | Syntax | Meaning |
    /// | ------ | ------- |
    /// | `{}` | The next unused key, left to right |
    /// | `{N}` | `keys[N]` — may reorder and repeat keys |
    /// | `{{` / `}}` | A literal `{` / `}` |
    ///
    /// Auto-numbered and indexed placeholders can be mixed; `{}` counts only its own
    /// occurrences, exactly as [`std::format!`] does.
    ///
    /// Every placeholder must have a corresponding key and every key must be used at
    /// least once — a mismatch in either direction is an error rather than a silently
    /// half-formatted string.
    ///
    /// Substituted values are never rescanned, so a config value containing `{}` is
    /// emitted verbatim instead of consuming the next key.
    ///
    /// # Errors
    /// Returns `ConfigError::FormatError` if the template has an unclosed `{`, an
    /// unmatched `}`, a named placeholder, an index with no matching key, or if the
    /// placeholder and key counts disagree
    /// Returns `ConfigError::PathNotFound` if `base` or any key does not exist
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// # let yaml = "db:\n  redis:\n    server: 127.0.0.1\n    port: 6379";
    /// # let config = Config::load_yaml(yaml, "/").unwrap();
    /// let result = config.fmt_strict("{}:{}", "db/redis", &["server", "port"]).unwrap();
    /// assert_eq!(result, "127.0.0.1:6379");
    ///
    /// // Indices allow reuse, and `{{`/`}}` emit literal braces
    /// let result = config.fmt_strict("{{{0}:{1}}} via {0}", "db/redis", &["server", "port"]).unwrap();
    /// assert_eq!(result, "{127.0.0.1:6379} via 127.0.0.1");
    /// ```
    pub fn fmt_strict(&self, format: &str, base: &str, keys: &[&str]) -> Result<String, ConfigError> {
        let pieces = parse_format(format)?;
        check_keys_match(&pieces, keys)?;

        let mut content = &self.content;
        let parts = parse_path(base, &self.separator);

        for item in parts.iter() {
            if item.is_empty() { continue; }
            match content.get(item.as_str()) {
                Some(v) => { content = v; },
                None => return Err(ConfigError::PathNotFound(base.to_string()))
            }
        }

        // Resolve every key up front so a value containing `{}` can never be
        // rescanned as a placeholder for a later key.
        let values = keys.iter()
            .map(|key| {
                content.get(*key)
                    .map(to_string)
                    .ok_or_else(|| ConfigError::PathNotFound(format!("{}/{}", base, key)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = String::with_capacity(format.len());
        for piece in pieces {
            match piece {
                Piece::Literal(text) => result.push_str(text),
                Piece::Value(index) => result.push_str(&values[index]),
            }
        }

        Ok(result)
    }
}

/// Splits a format string into literals and placeholder indices.
///
/// Auto-numbered `{}` placeholders are assigned indices in order of appearance,
/// independently of any explicit `{N}`, matching `std::format!`.
fn parse_format(format: &str) -> Result<Vec<Piece<'_>>, ConfigError> {
    let mut pieces = Vec::new();
    let mut auto_index = 0usize;
    let mut rest = format;

    while !rest.is_empty() {
        // Emit everything up to the next brace as one borrowed literal
        match rest.find(['{', '}']) {
            Some(0) => {},
            Some(at) => {
                let (literal, tail) = rest.split_at(at);
                pieces.push(Piece::Literal(literal));
                rest = tail;
            },
            None => {
                pieces.push(Piece::Literal(rest));
                break;
            },
        }

        if let Some(tail) = rest.strip_prefix("{{") {
            pieces.push(Piece::Literal("{"));
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("}}") {
            pieces.push(Piece::Literal("}"));
            rest = tail;
        } else if let Some(after_brace) = rest.strip_prefix('{') {
            let end = after_brace.find('}').ok_or_else(|| {
                ConfigError::FormatError(format!(
                    "Unclosed '{{' in format string \"{}\" (use '{{{{' for a literal brace)",
                    format
                ))
            })?;

            let spec = &after_brace[..end];
            let index = if spec.is_empty() {
                auto_index += 1;
                auto_index - 1
            } else {
                spec.parse::<usize>().map_err(|_| {
                    ConfigError::FormatError(format!(
                        "Unsupported placeholder '{{{}}}' in format string \"{}\": \
                         only '{{}}' and '{{N}}' are supported",
                        spec, format
                    ))
                })?
            };

            pieces.push(Piece::Value(index));
            rest = &after_brace[end + 1..];
        } else {
            return Err(ConfigError::FormatError(format!(
                "Unmatched '}}' in format string \"{}\" (use '}}}}' for a literal brace)",
                format
            )));
        }
    }

    Ok(pieces)
}

/// Rejects a template whose placeholders and keys do not correspond one to one.
///
/// Both directions used to fail silently: a surplus placeholder was left in the output
/// verbatim, and a surplus key was dropped without a trace.
fn check_keys_match(pieces: &[Piece<'_>], keys: &[&str]) -> Result<(), ConfigError> {
    let mut used = vec![false; keys.len()];

    for piece in pieces {
        if let Piece::Value(index) = piece {
            match used.get_mut(*index) {
                Some(slot) => *slot = true,
                None => return Err(ConfigError::FormatError(format!(
                    "Format string references key {} but only {} key(s) were provided",
                    index, keys.len()
                ))),
            }
        }
    }

    match used.iter().position(|used| !used) {
        Some(unused) => Err(ConfigError::FormatError(format!(
            "Key '{}' is never used by the format string", keys[unused]
        ))),
        None => Ok(()),
    }
}
