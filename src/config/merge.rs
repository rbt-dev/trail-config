//! Layering overlay files on top of a base config via deep merge.

use std::{io, mem};
use yaml_serde::{Value, value::TaggedValue};
use crate::error::ConfigError;
use super::{Config, OverlaySource};
use super::env::resolve_env_vars;
use super::loader::{empty_filename_error, get_file};
use super::parser::load_auto;

impl Config {
    /// Merges a required overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// A `!Tag` is part of a value's shape, not a value inside it: two nodes under the
    /// *same* tag merge like the mappings they usually are, while a differing tag — or an
    /// untagged overlay onto a tagged base — replaces. The tag names a serde enum variant,
    /// so changing it changes which variant the document describes, and merging the fields
    /// of two variants would produce one belonging to neither.
    ///
    /// A null in the overlay is a value like any other and takes precedence, so a key set
    /// to null — including through YAML's bare `key:` form — clears the base value, and a
    /// subtree set to null clears the entire subtree. The key itself remains present
    /// holding a null: [`contains`](Config::contains) still reports `true`, while
    /// [`str`](Config::str) returns `""` and the typed accessors return `None`. An overlay
    /// file that is *entirely* empty is the one exception — it is a no-op, not a
    /// document-wide clear.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, an error is returned.
    ///
    /// Environment variable placeholders (`${VAR}`, `${VAR:-default}`) are resolved in the
    /// overlay before merging. Values already present in the base config are not re-resolved,
    /// so resolved values containing `${` are preserved verbatim.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename. `None` falls back to
    ///   the environment this config already carries, so a base loaded for `prod` takes a
    ///   `config.{env}.yaml` overlay without repeating it; pass `Some` only to override
    ///   that. Also recorded on the config if it does not already carry one, so
    ///   [`environment`](Config::environment) and [`reload_from`](Config::reload_from) see
    ///   it; an environment set by the constructor is never replaced. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty, or the file is missing or cannot be read
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the filename contains `{env}` and neither `env`
    ///     nor this config supplies one
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "merge returns a new Config; the original is consumed"]
    pub fn merge_required(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        self.merge_required_in_place(filename, env)?;
        Ok(self)
    }

    /// Merges a required overlay file into this config, in place.
    ///
    /// Identical to [`merge_required`](Config::merge_required) in every respect but the
    /// signature — same overlay rules, same recording of the filename and the environment,
    /// same errors. What it adds is a config that is still there afterwards if the merge
    /// fails.
    ///
    /// The chaining form consumes `self` and returns it inside the `Ok`, which is right for
    /// a builder and wrong for a caller who wants to carry on. `config.merge_required(f)?`
    /// moves the config into the call, so an error path has nothing left to fall back to.
    /// This form leaves the receiver untouched on failure — filename, document and overlay
    /// chain all as they were — which is the same guarantee [`reload`](Config::reload) and
    /// [`reload_from`](Config::reload_from) already make, and which the merges could not
    /// make while they consumed what they were preserving.
    ///
    /// That guarantee is not defensive bookkeeping: the file is read, parsed and
    /// interpolated into a local before anything on `self` is touched, so every failure
    /// returns before the first mutation.
    ///
    /// Prefer the chaining form when building a config from a known set of files, and this
    /// one when a merge might fail and the base is worth keeping — which is most of the
    /// time for [`merge_optional_in_place`](Config::merge_optional_in_place).
    ///
    /// # Errors
    /// The same as [`merge_required`](Config::merge_required).
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let mut config = Config::load_required("config.yaml", "/", None)?;
    ///
    /// // The config survives a failed merge, so this can be reported and shrugged off
    /// if let Err(e) = config.merge_required_in_place("config.prod.yaml", None) {
    ///     eprintln!("ignoring unusable overlay: {e}");
    /// }
    ///
    /// let port = config.get_int("app/port"); // still the base's value
    /// # Ok(())
    /// # }
    /// ```
    pub fn merge_required_in_place(&mut self, filename: &str, env: Option<&str>) -> Result<(), ConfigError> {
        if filename.is_empty() {
            return Err(empty_filename_error());
        }

        let (file, resolved_env) = get_file(filename, self.environment_for(env))?;
        let overlay = resolve_env_vars(load_auto(&file)?)?;

        // Past the last fallible step, so from here nothing can leave `self` half-merged.
        let base = mem::replace(&mut self.content, Value::Null);
        self.content = merge_documents(base, overlay);
        self.overlays.push(OverlaySource::Required(file));
        self.adopt_environment(resolved_env);
        Ok(())
    }

    /// Merges an optional overlay file into this config, returning a new `Config`.
    ///
    /// Values in the overlay take precedence over values in `self`. The merge is deep —
    /// nested mappings are merged recursively so individual leaf values can be overridden
    /// without clobbering sibling keys. Sequences are replaced wholesale rather than
    /// merged element-by-element.
    ///
    /// A `!Tag` is part of a value's shape, not a value inside it: two nodes under the
    /// *same* tag merge like the mappings they usually are, while a differing tag — or an
    /// untagged overlay onto a tagged base — replaces. The tag names a serde enum variant,
    /// so changing it changes which variant the document describes, and merging the fields
    /// of two variants would produce one belonging to neither.
    ///
    /// A null in the overlay is a value like any other and takes precedence, so a key set
    /// to null — including through YAML's bare `key:` form — clears the base value, and a
    /// subtree set to null clears the entire subtree. The key itself remains present
    /// holding a null: [`contains`](Config::contains) still reports `true`, while
    /// [`str`](Config::str) returns `""` and the typed accessors return `None`. An overlay
    /// file that is *entirely* empty is the one exception — it is a no-op, not a
    /// document-wide clear.
    ///
    /// The overlay filename is recorded so that [`reload`](Config::reload) can re-read and
    /// re-apply it. If the overlay file is missing during a reload, it is silently skipped.
    /// If the file exists but cannot be parsed, an error is returned.
    ///
    /// Environment variable placeholders (`${VAR}`, `${VAR:-default}`) are resolved in the
    /// overlay before merging. Values already present in the base config are not re-resolved,
    /// so resolved values containing `${` are preserved verbatim.
    ///
    /// # Arguments
    /// * `filename` - Path to the overlay file (can contain `{env}` placeholder)
    /// * `env` - Optional environment name to substitute in filename. `None` falls back to
    ///   the environment this config already carries, so a base loaded for `prod` takes a
    ///   `config.{env}.yaml` overlay without repeating it; pass `Some` only to override
    ///   that. Also recorded on the config if it does not already carry one, so
    ///   [`environment`](Config::environment) and [`reload_from`](Config::reload_from) see
    ///   it; an environment set by the constructor is never replaced. Interpolated into a
    ///   filesystem path with no validation — do not pass untrusted input
    ///
    /// # Errors
    /// Returns `ConfigError::IoError` if the filename is empty — a caller bug, unlike an
    ///     absent file, which is the case this method exists to tolerate
    /// Returns `ConfigError::YamlError`, `ConfigError::JsonError` or `ConfigError::TomlError` if the file cannot be parsed
    /// Returns `ConfigError::FormatError` if the filename contains `{env}` and neither `env`
    ///     nor this config supplies one
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let config = Config::load_required("config.yaml", "/", None)?
    ///     .merge_required("config.{env}.yaml", Some("prod"))?
    ///     .merge_optional("config.local.yaml", None)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use = "merge returns a new Config; the original is consumed"]
    pub fn merge_optional(mut self, filename: &str, env: Option<&str>) -> Result<Config, ConfigError> {
        self.merge_optional_in_place(filename, env)?;
        Ok(self)
    }

    /// Merges an optional overlay file into this config, in place.
    ///
    /// The `&mut self` counterpart to [`merge_optional`](Config::merge_optional), and the
    /// one of the pair that most wants it. That method exists to make an overlay's
    /// *absence* survivable — a missing file is skipped and the config carries on — but the
    /// other way an optional overlay can be unusable, a parse error, was not merely
    /// propagated: it was propagated after the base config had been moved into the call and
    /// could no longer be recovered. "Use `config.local.yaml` if it is present *and
    /// readable*, otherwise carry on" could not be written, because writing the fallback
    /// needed a config the signature had already taken away.
    ///
    /// The receiver is untouched on failure — see
    /// [`merge_required_in_place`](Config::merge_required_in_place), which explains why
    /// that holds mechanically rather than by care.
    ///
    /// # Errors
    /// The same as [`merge_optional`](Config::merge_optional). Note that an absent file is
    /// not one of them: it is skipped, and the overlay is still recorded so a later
    /// [`reload`](Config::reload) picks the file up once it appears.
    ///
    /// # Example
    /// ```no_run
    /// # use trail_config::{Config, ConfigError};
    /// # fn main() -> Result<(), ConfigError> {
    /// let mut config = Config::load_required("config.yaml", "/", None)?;
    ///
    /// // Absent is fine and silent; unreadable is reported and the base survives it
    /// if let Err(e) = config.merge_optional_in_place("config.local.yaml", None) {
    ///     eprintln!("config.local.yaml is unusable, continuing without it: {e}");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn merge_optional_in_place(&mut self, filename: &str, env: Option<&str>) -> Result<(), ConfigError> {
        // Checked even though this method tolerates a missing file: reading an empty
        // path yields `NotFound`, which is exactly the case it is designed to ignore,
        // so without this an empty filename would silently no-op and then push a dead
        // overlay that every later `reload()` re-walks.
        if filename.is_empty() {
            return Err(empty_filename_error());
        }

        let (file, resolved_env) = get_file(filename, self.environment_for(env))?;
        match load_auto(&file) {
            Ok(yaml) => {
                // Resolved before the base is touched, so an unset `${VAR}` in the overlay
                // returns with `self` still holding the document it had.
                let overlay = resolve_env_vars(yaml)?;
                let base = mem::replace(&mut self.content, Value::Null);
                self.content = merge_documents(base, overlay);
            },
            Err(ConfigError::IoError { ref source, .. }) if source.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
        self.overlays.push(OverlaySource::Optional(file));
        self.adopt_environment(resolved_env);
        Ok(())
    }

    /// Chooses the environment a merge resolves `{env}` with: the caller's, or failing
    /// that the one this config already carries.
    ///
    /// The counterpart to [`adopt_environment`](Config::adopt_environment) below, and
    /// added because only that direction was wired. A merge would resolve a template
    /// against an environment the caller repeated and record it on a config that had
    /// none, but a config that *already knew* its environment could not use it — so
    ///
    /// ```text
    /// Config::load_required("config.yaml", "/", Some("prod"))?
    ///     .merge_required("config.{env}.yaml", None)?
    /// ```
    ///
    /// failed with "contains '{env}' but no environment was supplied", naming a value the
    /// config was holding at the time. That is the exact inverse of the rule
    /// [`reload_from`](Config::reload_from) settles for the same placeholder, whose
    /// rustdoc explains it takes no `env` argument precisely *because* it reads the one on
    /// the config. Two methods resolving one placeholder against one field, and only one
    /// of them looked.
    ///
    /// The caller's argument still wins when given. An overlay for a different environment
    /// than the base is a legitimate thing to want, and passing it explicitly is how you
    /// say so; `None` now means "the one this config already has" rather than "none at
    /// all". Purely additive — every call that passes `Some(..)` behaves as it did, and a
    /// filename without a placeholder never consulted this either way.
    fn environment_for<'a>(&'a self, env: Option<&'a str>) -> Option<&'a str> {
        env.or(self.environment.as_deref())
    }

    /// Records an environment supplied at a merge, if this config has none yet.
    ///
    /// The natural shape when the base file is not environment-specific but an overlay
    /// is: `load_required("config.yaml", "/", None).merge_required("config.{env}.yaml",
    /// Some("prod"))`. The merge resolved `{env}` correctly and then dropped the
    /// environment on the floor, so [`environment`](Config::environment) under-reported
    /// it and [`reload_from`](Config::reload_from) — which takes no `env` argument
    /// precisely because it reads the one on the config — could not resolve a template
    /// the merge had resolved a moment earlier.
    ///
    /// **Only when absent.** An environment already on the config was chosen by the
    /// constructor, which is the config's own identity; letting a later overlay
    /// overwrite it would silently change what a subsequent `reload_from` resolves. So
    /// this fills a gap and never reassigns.
    ///
    /// Symmetric with [`environment_for`](Config::environment_for) above, which is the
    /// same rule pointing the other way: the config's environment is what a merge resolves
    /// `{env}` with unless the caller names one, and a merge's environment is what the
    /// config records unless it already has one. Whichever end supplies it, there is
    /// exactly one environment afterwards and it is the more specific of the two.
    fn adopt_environment(&mut self, resolved: Option<String>) {
        if self.environment.is_none() {
            self.environment = resolved;
        }
    }
}

/// Deep-merges a whole overlay *document* onto a base document.
///
/// An empty document is a no-op, leaving the base untouched — that is what an absent
/// file tolerated by [`Config::merge_optional`], an empty file and a comment-only file
/// all parse to. The check belongs here, at the document level, because it is a property
/// of the overlay *as a whole*: inside a document an explicit null is an ordinary value
/// that overrides the base, and `merge_values` cannot tell the two apart — both arrive
/// as `Value::Null`.
///
/// Every merge of a complete file goes through this function; `merge_values` recurses
/// only into keys, where the empty-document rule must not apply.
pub(super) fn merge_documents(base: Value, overlay: Value) -> Value {
    match overlay {
        Value::Null => base,
        overlay => merge_values(base, overlay),
    }
}

/// Deep-merges `overlay` onto `base`, preserving the base's key order.
///
/// Overridden keys keep their position in the base document and genuinely-new overlay
/// keys are appended. This matters because `yaml_serde::Mapping` wraps an `IndexMap` and
/// is insertion-ordered, so the merged order is visible to anything that preserves it —
/// `get_as` / `deserialize` into a `Value`, a `Mapping` or an `IndexMap`, and any
/// re-serialization the caller does downstream.
fn merge_values(base: Value, overlay: Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(mut base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                match base_map.get_mut(&key) {
                    // Merged in place. `Mapping::remove` is `swap_remove`, so
                    // remove-then-insert would move the map's *last* entry into the
                    // vacated slot and append the overridden key at the end — two keys
                    // displaced for a single-key overlay.
                    Some(slot) => {
                        let base_val = mem::replace(slot, Value::Null);
                        *slot = merge_values(base_val, overlay_val);
                    },
                    None => {
                        base_map.insert(key, overlay_val);
                    },
                }
            }
            Value::Mapping(base_map)
        },
        // Two values under the *same* tag are the same enum variant, so they merge like
        // the mappings they usually are. Without this arm a tagged subtree took the
        // catch-all below and the overlay replaced it whole, silently dropping every
        // sibling key the overlay did not restate — the one place the deep merge this
        // crate promises quietly stopped being deep.
        //
        // Differing tags are left to the catch-all deliberately, as is a tagged base under
        // an untagged overlay. The tag names the variant, so `!Postgres` overlaid by
        // `!Sqlite` is a change of shape rather than a patch to the existing one, and
        // merging their fields would produce a document belonging to neither. An overlay
        // that drops the tag while keeping the fields is the same case: the merged result
        // would no longer deserialize into the enum the base named, and replacing makes
        // that visible immediately instead of at the next `get_as`.
        (Value::Tagged(base), Value::Tagged(overlay)) if base.tag == overlay.tag => {
            Value::Tagged(Box::new(TaggedValue {
                tag: overlay.tag,
                value: merge_values(base.value, overlay.value),
            }))
        },
        // Sequences are replaced wholesale; all other types are overridden by the
        // overlay — including an explicit null, which is how a key is cleared. The
        // whole-document case is handled in `merge_documents`, above.
        (_, overlay) => overlay,
    }
}
