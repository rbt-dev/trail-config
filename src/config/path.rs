//! Path parsing and navigation into the config value tree.

use std::mem;
use yaml_serde::Value;

pub(super) fn get_leaf<'a>(mut content: &'a Value, path: &str, separator: &str) -> Option<&'a Value> {
    if path.is_empty() || separator.is_empty() {
        return None;
    }

    let parts = parse_path(path, separator);

    for item in parts.iter() {
        if item.is_empty() {
            continue;
        }
        match content.get(item) {
            Some(v) => { content = v; },
            None => return None
        }
    }

    Some(content)
}

/// Parses a path with escape sequence support.
///
/// - `\<sep>` becomes a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
/// - `\\` becomes a literal backslash in the key
///
/// Walks the path as a shrinking `&str` suffix and tests it with `strip_prefix`, so
/// nothing is allocated while scanning — only the returned segments. Matching on the
/// escape sequence before the separator preserves the precedence a separator that
/// itself starts with `\` would otherwise disturb.
pub(super) fn parse_path(path: &str, separator: &str) -> Vec<String> {
    // An empty separator would make `strip_prefix` match at every position and never
    // advance. Callers are guarded (`check_separator` on construction, `get_leaf`
    // above), so this is only a backstop against an infinite loop.
    if separator.is_empty() {
        return vec![path.to_string()];
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut rest = path;

    while !rest.is_empty() {
        if let Some(after_escape) = rest.strip_prefix('\\') {
            if let Some(tail) = after_escape.strip_prefix(separator) {
                // `\<sep>` — a literal separator inside the key
                current.push_str(separator);
                rest = tail;
            } else if let Some(tail) = after_escape.strip_prefix('\\') {
                // `\\` — a literal backslash
                current.push('\\');
                rest = tail;
            } else {
                // A backslash escaping nothing in particular stays as itself
                current.push('\\');
                rest = after_escape;
            }
        } else if let Some(tail) = rest.strip_prefix(separator) {
            parts.push(mem::take(&mut current));
            rest = tail;
        } else {
            let mut chars = rest.chars();
            let ch = chars.next().expect("rest is non-empty");
            current.push(ch);
            rest = chars.as_str();
        }
    }

    parts.push(current);
    parts
}
