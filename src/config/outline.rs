//! Listing a document's addressable paths, with the values elided.

use yaml_serde::Value;
use super::Config;

impl Config {
    /// Lists every path in the document, one per line, with values replaced by their type.
    ///
    /// The answer to "why is this path not resolving": it prints the paths that *do*
    /// resolve, spelled exactly as an accessor takes them — the config's own separator,
    /// with keys containing a separator or a backslash escaped — so a line can be pasted
    /// straight into [`str`](Config::str) or [`get`](Config::get).
    ///
    /// ```text
    /// app/name: <string>
    /// app/port: <number>
    /// db/redis/server: <string>
    /// features: <2 items>
    /// ```
    ///
    /// **Values are never printed**, only their types. That is what makes this safe to
    /// log or paste into an issue: by the time a `Config` exists, `${DB_PASSWORD}` has
    /// already been interpolated, so a full dump would put the real secret wherever the
    /// output goes — the same reasoning that elides the document from
    /// [`Debug`](std::fmt::Debug). The key shape is the part with debugging value; the
    /// values are the part you can read deliberately, one path at a time, once you know
    /// the path exists.
    ///
    /// A caller who genuinely wants the whole document can deserialize it into a
    /// [`Value`](crate::Value) or a [`Mapping`](crate::Mapping) and serialize that —
    /// an explicit act at the call site, rather than something this crate does on the
    /// way to a log line.
    ///
    /// Sequences are leaves: their elements have no addressable path (`items/0` is a
    /// lookup for a key named `0`), so a sequence prints as its length. An empty mapping
    /// likewise prints as itself, since it contains no path to list. A document that is
    /// not a mapping at all — an empty file, or one holding a bare sequence — has no
    /// paths, so its type is printed alone.
    ///
    /// Keys appear in document order, the same order the merge preserves — so the outline
    /// of a layered config also shows where each key ended up.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_yaml("app:\n  port: 8080\n  debug: true\n", "/").unwrap();
    /// assert_eq!(config.outline(), "app/port: <number>\napp/debug: <bool>\n");
    /// ```
    pub fn outline(&self) -> String {
        let mut out = String::new();
        write_outline(&self.content, &mut String::new(), &self.separator, &mut out);
        out
    }
}

/// Walks the document depth-first, appending one line per leaf.
///
/// `prefix` is the path built so far; it is extended and truncated in place rather than
/// re-joined at every level.
fn write_outline(value: &Value, prefix: &mut String, separator: &str, out: &mut String) {
    match value {
        Value::Mapping(map) if !map.is_empty() => {
            let base = prefix.len();
            for (key, child) in map {
                if base > 0 {
                    prefix.push_str(separator);
                }
                match key.as_str() {
                    Some(key) => push_escaped(prefix, key, separator),
                    // A non-string key cannot be written as a path at all — YAML allows
                    // them, this crate's accessors cannot address them. Say so rather
                    // than print something unusable.
                    None => prefix.push_str("<non-string key>"),
                }
                write_outline(child, prefix, separator, out);
                prefix.truncate(base);
            }
        },
        leaf => {
            // At the root there is no path to print, and printing an empty one would
            // put a line in the output that the accessors reject — every other line is
            // resolvable as written, and this one would not be
            if !prefix.is_empty() {
                out.push_str(prefix);
                out.push_str(": ");
            }
            out.push_str(&describe_leaf(leaf));
            out.push('\n');
        },
    }
}

/// Escapes a key so the rendered path resolves back to it.
///
/// Mirrors the escape sequences [`segments`](super::path::segments) resolves: `\` before
/// a literal separator, and `\\` for a literal backslash. Without this, a key containing
/// the separator would print as two path segments that navigate somewhere else — the
/// same class of untrue path that `fmt`'s error messages used to render.
fn push_escaped(out: &mut String, key: &str, separator: &str) {
    let mut rest = key;

    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix('\\') {
            out.push_str("\\\\");
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix(separator) {
            out.push('\\');
            out.push_str(separator);
            rest = tail;
        } else {
            let next = rest.chars().next().expect("rest is non-empty");
            out.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
}

/// Names a leaf's type, never its content.
fn describe_leaf(value: &Value) -> String {
    match value {
        Value::Null => "<null>".to_string(),
        Value::Bool(_) => "<bool>".to_string(),
        Value::Number(_) => "<number>".to_string(),
        Value::String(_) => "<string>".to_string(),
        Value::Sequence(seq) if seq.len() == 1 => "<1 item>".to_string(),
        Value::Sequence(seq) => format!("<{} items>", seq.len()),
        // Only reachable for an empty mapping; a populated one recurses instead
        Value::Mapping(_) => "<empty mapping>".to_string(),
        _ => "<value>".to_string(),
    }
}
