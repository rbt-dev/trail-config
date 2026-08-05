//! Path parsing and navigation into the config value tree.

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
pub(super) fn parse_path(path: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();
    let sep_first_char = separator.chars().next().unwrap_or('/');

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let remaining: String = chars.clone().collect();
            if remaining.starts_with(separator) {
                current.push_str(separator);
                for _ in 0..separator.chars().count() {
                    chars.next();
                }
            } else if let Some(&next) = chars.peek() {
                if next == '\\' {
                    current.push('\\');
                    chars.next();
                } else {
                    current.push(ch);
                }
            } else {
                current.push(ch);
            }
        } else if ch == sep_first_char {
            let remaining: String = chars.clone().collect();
            let expected_rest = &separator[sep_first_char.len_utf8()..];
            if remaining.starts_with(expected_rest) {
                parts.push(current.clone());
                current.clear();
                for _ in 1..separator.chars().count() {
                    chars.next();
                }
            } else {
                current.push(ch);
            }
        } else {
            current.push(ch);
        }
    }

    parts.push(current);
    parts
}
