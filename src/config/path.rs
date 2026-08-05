//! Path parsing and navigation into the config value tree.

use std::borrow::Cow;
use yaml_serde::Value;

/// Navigates to the value at `path`.
///
/// Every segment must be non-empty, so a leading, trailing or doubled separator makes
/// the lookup fail rather than being skipped. Skipping meant `a//b`, `/a/b` and `a/b/`
/// all resolved like `a/b`, and a path of just the separator returned the entire
/// document — turning typos into silently successful lookups.
pub(super) fn get_leaf<'a>(mut content: &'a Value, path: &str, separator: &str) -> Option<&'a Value> {
    if path.is_empty() || separator.is_empty() {
        return None;
    }

    for segment in segments(path, separator) {
        if segment.is_empty() {
            return None;
        }
        content = content.get(segment.as_ref())?;
    }

    Some(content)
}

/// Splits a path into its segments, resolving escape sequences.
///
/// - `\<sep>` becomes a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
/// - `\\` becomes a literal backslash in the key
///
/// Yields lazily and borrows wherever it can: a segment is only copied when it actually
/// contains an escape sequence, which real paths almost never do. Collecting into
/// `Vec<String>` instead cost roughly 36 ns per segment — about 78% of the total time
/// spent traversing a path.
pub(super) fn segments<'p, 's>(path: &'p str, separator: &'s str) -> Segments<'p, 's> {
    Segments { rest: path, separator, done: false }
}

pub(super) struct Segments<'p, 's> {
    rest: &'p str,
    separator: &'s str,
    done: bool,
}

impl<'p> Iterator for Segments<'p, '_> {
    type Item = Cow<'p, str>;

    fn next(&mut self) -> Option<Cow<'p, str>> {
        if self.done {
            return None;
        }

        // An empty separator would match at every position and never advance. Callers
        // are guarded (`check_separator` on construction, `get_leaf` above), so this is
        // only a backstop against an infinite loop.
        if self.separator.is_empty() {
            self.done = true;
            return Some(Cow::Borrowed(self.rest));
        }

        let input = self.rest;
        let sep = self.separator;

        // `i` scans; `flushed` marks how much of `input` has been copied into `owned`.
        // While `owned` is None the segment is still a contiguous slice of `input` and
        // can be handed back borrowed.
        let mut i = 0;
        let mut flushed = 0;
        let mut owned: Option<String> = None;

        while i < input.len() {
            let tail = &input[i..];

            if let Some(after_escape) = tail.strip_prefix('\\') {
                if after_escape.starts_with(sep) {
                    // `\<sep>` — a literal separator inside the key
                    let buf = owned.get_or_insert_with(String::new);
                    buf.push_str(&input[flushed..i]);
                    buf.push_str(sep);
                    i += 1 + sep.len();
                    flushed = i;
                    continue;
                } else if after_escape.starts_with('\\') {
                    // `\\` — a literal backslash
                    let buf = owned.get_or_insert_with(String::new);
                    buf.push_str(&input[flushed..i]);
                    buf.push('\\');
                    i += 2;
                    flushed = i;
                    continue;
                }
                // A backslash escaping nothing in particular stays as itself, and the
                // segment can still be borrowed
                i += 1;
                continue;
            }

            if tail.starts_with(sep) {
                let segment = finish(input, i, flushed, owned);
                self.rest = &input[i + sep.len()..];
                return Some(segment);
            }

            i += tail.chars().next().expect("tail is non-empty").len_utf8();
        }

        let segment = finish(input, input.len(), flushed, owned);
        self.done = true;
        self.rest = "";
        Some(segment)
    }
}

/// Closes off a segment ending at byte `end`, borrowing unless an escape forced a copy.
fn finish(input: &str, end: usize, flushed: usize, owned: Option<String>) -> Cow<'_, str> {
    match owned {
        None => Cow::Borrowed(&input[..end]),
        Some(mut buf) => {
            buf.push_str(&input[flushed..end]);
            Cow::Owned(buf)
        },
    }
}

/// Collects [`segments`] into owned strings.
///
/// Only used by tests, which predate the iterator and assert against `Vec<String>`.
#[cfg(test)]
pub(super) fn parse_path(path: &str, separator: &str) -> Vec<String> {
    segments(path, separator).map(Cow::into_owned).collect()
}
