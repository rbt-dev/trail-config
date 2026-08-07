//! Listing a document's addressable paths, with the values elided.

use yaml_serde::Value;
use super::Config;
use super::accessor::untagged;

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
    /// A `!Tag` is looked through, exactly as the accessors look through it, so a tagged
    /// mapping's keys are listed rather than the tag being printed as a leaf. The tag is
    /// not shown: it is not part of any path, and every line here is a path.
    ///
    /// # Keys that are not addressable
    ///
    /// Two kinds of key cannot be written as a path at all: an **empty** key, because
    /// every path segment must be non-empty, and a **non-string** key (`1:`, `true:`),
    /// because a path segment is matched as a string. YAML allows both. They are still
    /// listed — a key you cannot reach is exactly what you want to see when a lookup is
    /// failing — but marked, so the output never claims a path that does not resolve:
    ///
    /// ```text
    /// app/port: <number>
    /// "": <string>                   # not addressable
    /// retries/1: <string>            # not addressable
    /// ```
    ///
    /// The marker covers the whole line, so everything nested under such a key carries it
    /// too. Every line without one resolves as written; that is the property worth
    /// relying on, and it is what the marker exists to keep true.
    ///
    /// Sequences are leaves: their elements have no addressable path (`items/0` is a
    /// lookup for a key named `0`), so a sequence prints as its length. An empty mapping
    /// likewise prints as itself, since it contains no path to list. A document that is
    /// not a mapping at all — an empty file, or one holding a bare sequence — has no
    /// paths, so its type is printed alone.
    ///
    /// Keys appear in document order, the same order the merge preserves — so the outline
    /// of a layered config also shows where each key ended up. That holds for all three
    /// formats: JSON and TOML would otherwise arrive alphabetically sorted, since both
    /// crates' own value types are `BTreeMap`-backed by default.
    ///
    /// # Example
    /// ```
    /// # use trail_config::Config;
    /// let config = Config::load_yaml("app:\n  port: 8080\n  debug: true\n", "/").unwrap();
    /// assert_eq!(config.outline(), "app/port: <number>\napp/debug: <bool>\n");
    /// ```
    pub fn outline(&self) -> String {
        let mut out = String::new();
        write_outline(&self.content, &mut String::new(), &self.separator, true, &mut out);
        out
    }
}

/// Appended to any line whose path the accessors cannot resolve.
///
/// Marking beats omitting: a key you cannot reach is precisely what you want to see when a
/// lookup is failing, and dropping it silently leaves you comparing the outline against the
/// file wondering which of the two is lying.
const NOT_ADDRESSABLE: &str = "  # not addressable";

/// Walks the document depth-first, appending one line per leaf.
///
/// `prefix` is the path built so far; it is extended and truncated in place rather than
/// re-joined at every level. `addressable` tracks whether every key on the way here could
/// be written as a path segment — once one cannot, nothing below it can either, so it only
/// ever goes from true to false.
fn write_outline(
    value: &Value,
    prefix: &mut String,
    separator: &str,
    addressable: bool,
    out: &mut String,
) {
    // A `!Tag` is transparent to addressing — `Value::get` untags before looking a key
    // up, so `db/host` resolves whether or not `db` is tagged. Listing had to agree: a
    // tagged mapping was treated as a leaf, so its keys were addressable but never
    // printed, and the outline's whole job is to show which paths exist.
    match untagged(value) {
        Value::Mapping(map) if !map.is_empty() => {
            let base = prefix.len();
            for (key, child) in map {
                if base > 0 {
                    prefix.push_str(separator);
                }
                let key_addressable = push_key(prefix, key, separator);
                write_outline(child, prefix, separator, addressable && key_addressable, out);
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
            if !addressable {
                out.push_str(NOT_ADDRESSABLE);
            }
            out.push('\n');
        },
    }
}

/// Renders one mapping key into `prefix`, reporting whether a path containing it resolves.
///
/// YAML permits keys this crate's path syntax cannot express, and printing them as though
/// it could was the bug. An empty key rendered as nothing at all, so `{a: {"": 1}}` printed
/// `a/` — which `get_leaf` rejects, correctly, for having an empty segment — and at the top
/// level it printed a bare `<number>`, byte-identical to what a document holding a single
/// scalar prints. A non-string key printed the literal text `<non-string key>`, so `1:` and
/// `true:` collapsed onto one line and neither resolved.
///
/// Each is now rendered as itself and its line marked, which claims nothing and hides
/// nothing. An empty key shows as `""` — so does a key genuinely made of two quote
/// characters, but that one is addressable and carries no marker, which tells them apart.
fn push_key(out: &mut String, key: &Value, separator: &str) -> bool {
    match key {
        Value::String(key) if !key.is_empty() => {
            push_escaped(out, key, separator);
            true
        },
        Value::String(_) => {
            out.push_str("\"\"");
            false
        },
        Value::Null => {
            out.push_str("null");
            false
        },
        Value::Bool(value) => {
            out.push_str(&value.to_string());
            false
        },
        Value::Number(value) => {
            out.push_str(&value.to_string());
            false
        },
        // A sequence, a mapping or a tagged value used as a *key*. YAML allows it, there
        // is no sensible one-line rendering, and nothing could address it either way.
        Value::Sequence(_) | Value::Mapping(_) | Value::Tagged(_) => {
            out.push_str("<complex key>");
            false
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
        // The `!separator.is_empty()` guard is the same backstop
        // [`segments`](super::path::segments) states: an empty separator matches at
        // every position without advancing, so this arm would loop forever — and
        // unlike the splitter, grow `out` until it exhausted memory. Unreachable
        // today, since `check_separator` runs in every constructor and `sources()`
        // only clones, but two functions with the identical hazard should not
        // disagree about whether the guard is worth restating locally.
        } else if !separator.is_empty() && rest.starts_with(separator) {
            out.push('\\');
            out.push_str(separator);
            rest = &rest[separator.len()..];
        } else {
            let next = rest.chars().next().expect("rest is non-empty");
            out.push(next);
            rest = &rest[next.len_utf8()..];
        }
    }
}

/// Collects [`push_escaped`] into an owned string.
///
/// Only used by tests: `push_escaped` is private to this module and appends into a
/// caller's buffer, neither of which suits an assertion. Mirrors
/// [`parse_path`](super::path::parse_path), which exists for the same reason.
#[cfg(test)]
pub(super) fn escape_key(key: &str, separator: &str) -> String {
    let mut out = String::new();
    push_escaped(&mut out, key, separator);
    out
}

/// Names a leaf's type, never its content.
///
/// Exhaustive on purpose — `yaml_serde::Value` is not `#[non_exhaustive]`, and the
/// catch-all this replaces is where `Value::Tagged` used to land, printing `<value>` for
/// a subtree whose keys were perfectly addressable.
fn describe_leaf(value: &Value) -> String {
    match untagged(value) {
        Value::Null => "<null>".to_string(),
        Value::Bool(_) => "<bool>".to_string(),
        Value::Number(_) => "<number>".to_string(),
        Value::String(_) => "<string>".to_string(),
        Value::Sequence(seq) if seq.len() == 1 => "<1 item>".to_string(),
        Value::Sequence(seq) => format!("<{} items>", seq.len()),
        // Only reachable for an empty mapping; a populated one recurses instead
        Value::Mapping(_) => "<empty mapping>".to_string(),
        // `untagged` loops until the value is not tagged, so this cannot be reached
        Value::Tagged(_) => unreachable!("untagged() never returns a tagged value"),
    }
}
