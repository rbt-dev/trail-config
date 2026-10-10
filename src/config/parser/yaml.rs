use std::fs;
use serde::de::Error as _;
use yaml_serde::{Mapping, Value, from_str};
use crate::error::ConfigError;

pub(crate) fn load_file(filename: &str) -> Result<Value, ConfigError> {
    let yaml = fs::read_to_string(filename)
        .map_err(|e| ConfigError::io_in(filename, e))?;
    parse_in(&yaml, filename)
}

pub(crate) fn parse(yaml: &str) -> Result<Value, ConfigError> {
    parse_internal(yaml, None)
}

/// Parses YAML that belongs to `filename`, naming it in any error.
pub(crate) fn parse_in(yaml: &str, filename: &str) -> Result<Value, ConfigError> {
    parse_internal(yaml, Some(filename))
}

fn parse_internal(yaml: &str, file: Option<&str>) -> Result<Value, ConfigError> {
    // Every YAML path — file or string — passes through here, so the BOM rule is
    // applied exactly once per format. See `super::strip_bom`.
    let yaml = super::strip_bom(yaml);
    let mut value: Value = from_str(yaml).map_err(|e| ConfigError::yaml_in(file, e))?;
    // Here rather than in the loader for the same reason: base files, overlays, strings,
    // reloads and `load_or_create`'s defaults all parse through this one function, so
    // merge keys are resolved before interpolation and before any overlay merging.
    resolve_merge_keys(&mut value)
        .map_err(|e| ConfigError::yaml_in(file, yaml_serde::Error::custom(e.message())))?;
    Ok(value)
}

/// The YAML merge key, as in `<<: *defaults`.
const MERGE_KEY: &str = "<<";

/// Resolves YAML merge keys (`<<: *anchor` and `<<: [*a, *b]`) throughout `value`.
///
/// `yaml_serde` parses `<<` as an ordinary key and resolves it only when
/// `Value::apply_merge` is called. Left unresolved, the anchored mapping sat under a
/// literal `<<` key: every merged-in path read as missing, strict deserializing failed
/// with "missing field", and a struct with serde defaults quietly took the defaults.
///
/// Written here rather than calling `apply_merge`, which differs in two ways that matter:
///
/// - **Key order.** It `swap_remove`s the `<<` entry — moving the last key into its slot —
///   and appends the merged keys, so `<<, a, c, y` came out as `y, a, c, x`. Here the
///   merged keys take the position `<<` held, in the order their source mappings list
///   them, and every explicit key keeps its own position. Document order is a promise
///   `outline` and the overlay merge both make.
/// - **Chained merges.** It works top-down, so an anchored mapping that itself contains
///   `<<` was copied in with that `<<` still unresolved, which put a literal `<<` key back
///   into the target. Resolving children first means every merge source is already
///   resolved by the time it is merged.
///
/// The semantics are the merge-key spec's, and `apply_merge`'s:
///
/// - Explicit keys win over merged ones, wherever they appear relative to `<<`.
/// - In `<<: [*a, *b]`, an earlier mapping wins over a later one.
/// - Anything but a mapping or a sequence of mappings is an error, with `apply_merge`'s
///   wording plus the path to the offending mapping. Neither names a value.
///
/// Recursion depth is bounded by the parser's own 128-level nesting limit, which applies
/// to the expanded document, and resolving a merge never makes the document deeper.
fn resolve_merge_keys(value: &mut Value) -> Result<(), MergeError> {
    match value {
        Value::Mapping(mapping) => {
            // Resolves the `<<` value along with everything else, which is what makes
            // chained merges work: each source mapping is resolved before it is merged.
            for (key, child) in mapping.iter_mut() {
                resolve_merge_keys(child).map_err(|e| e.within(key_label(key)))?;
            }
            merge_into(mapping)
        }
        Value::Sequence(sequence) => {
            for (index, item) in sequence.iter_mut().enumerate() {
                resolve_merge_keys(item).map_err(|e| e.within(format!("[{index}]")))?;
            }
            Ok(())
        }
        // A tag is transparent to the walkers in this crate, so a tagged mapping merges
        // like any other.
        Value::Tagged(tagged) => resolve_merge_keys(&mut tagged.value),
        _ => Ok(()),
    }
}

/// Replaces the `<<` entry of `mapping`, if it has one, with the keys it merges in.
fn merge_into(mapping: &mut Mapping) -> Result<(), MergeError> {
    let Some(position) = mapping.keys().position(|key| key.as_str() == Some(MERGE_KEY)) else {
        return Ok(());
    };
    let Some(merge) = mapping.shift_remove(MERGE_KEY) else {
        return Ok(());
    };

    let sources = match merge {
        Value::Mapping(source) => vec![source],
        Value::Sequence(sequence) => sequence
            .into_iter()
            .map(|item| match item {
                Value::Mapping(source) => Ok(source),
                Value::Sequence(_) => Err(MergeError::new(MergeProblem::SequenceInElement)),
                Value::Tagged(_) => Err(MergeError::new(MergeProblem::Tagged)),
                _ => Err(MergeError::new(MergeProblem::ScalarInElement)),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Value::Tagged(_) => return Err(MergeError::new(MergeProblem::Tagged)),
        _ => return Err(MergeError::new(MergeProblem::Scalar)),
    };

    // With `<<` removed, `mapping` holds exactly the explicit keys, which win; among the
    // sources, the first to supply a key wins.
    let mut merged = Mapping::new();
    for source in sources {
        for (key, value) in source {
            if !mapping.contains_key(&key) && !merged.contains_key(&key) {
                merged.insert(key, value);
            }
        }
    }

    // `Mapping` has no positional insert, so rebuild it with the merged keys spliced in
    // where `<<` was.
    let explicit = std::mem::take(mapping);
    mapping.reserve(explicit.len() + merged.len());
    let mut merged = Some(merged);
    for (index, (key, value)) in explicit.into_iter().enumerate() {
        if index == position {
            mapping.extend(merged.take().unwrap_or_default());
        }
        mapping.insert(key, value);
    }
    // `<<` was the last key, so the loop never reached its position.
    mapping.extend(merged.take().unwrap_or_default());
    Ok(())
}

/// How a key path segment is shown in a merge error. Keys are part of the document's
/// shape rather than its values, so naming them follows the crate's other errors, which
/// name paths but never values.
fn key_label(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "~".to_string(),
        _ => "<complex key>".to_string(),
    }
}

enum MergeProblem {
    Scalar,
    Tagged,
    ScalarInElement,
    SequenceInElement,
}

struct MergeError {
    problem: MergeProblem,
    /// Key path to the mapping holding the bad `<<`, innermost segment first, because it
    /// is collected while the error propagates outwards.
    path: Vec<String>,
}

impl MergeError {
    fn new(problem: MergeProblem) -> Self {
        MergeError { problem, path: Vec::new() }
    }

    fn within(mut self, segment: String) -> Self {
        self.path.push(segment);
        self
    }

    fn message(&self) -> String {
        // The wording is `apply_merge`'s, so it reads the same as the rest of the
        // upstream errors a YAML document can produce.
        let problem = match self.problem {
            MergeProblem::Scalar => "expected a mapping or list of mappings for merging, but found scalar",
            MergeProblem::Tagged => "unexpected tagged value in merge",
            MergeProblem::ScalarInElement => "expected a mapping for merging, but found scalar",
            MergeProblem::SequenceInElement => "expected a mapping for merging, but found sequence",
        };
        if self.path.is_empty() {
            format!("merge key `{MERGE_KEY}` at the document root: {problem}")
        } else {
            let path: Vec<String> = self.path.iter().rev().map(|s| format!("`{s}`")).collect();
            format!("merge key `{MERGE_KEY}` under {}: {problem}", path.join(" > "))
        }
    }
}
