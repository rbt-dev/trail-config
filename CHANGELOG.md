# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - Unreleased

<!-- Cargo.toml is bumped to 0.5.0 ahead of the release; set the date here when publishing. -->

### Added

- `Format` and `_as` constructor twins — `load_required_as`, `load_optional_as`, `load_or_create_as` — pinning the parser for a file whose extension does not name its format. The choice is kept across `reload` and `reload_from`. Replaces `load_json_file` / `load_toml_file`.
- `merge_required_in_place` / `merge_optional_in_place` — `&mut self` merges that leave the receiver untouched on failure.
- `Config::separator()`, completing the trio with `filename()` and `environment()`.
- `Config::outline()` — every path in the document with values replaced by their types, so it is safe to log. Keys that no path can reach are listed and marked `# not addressable`.
- `ConfigHandle::reload_from`, plus the rest of the lenient accessor surface: `get`, `list`, `get_as`, `deserialize` and `fmt`.
- Re-exports so callers can name what the API hands back: `Value`, `Mapping`, `Sequence`, `Number`, and `JsonError` / `TomlError` under their features.
- `ValueError`, and `ConfigError::DeserializeError { file, path, source }` for failures that are not parse errors.
- `config!`'s positional form takes any combination of `sep`, `env`, `merge` and `merge_optional` (in that order), not exactly one.
- `rust-version = "1.85"`, verified against that toolchain.
- Crate-level documentation, `#![warn(missing_docs)]`, and docs.rs feature labels.
- Repository tooling: `criterion` benchmarks, a `tests/` suite exercising the crate as a consumer, `check.ps1` / `check.sh`, and `exclude` to keep internal notes out of the published crate.

### Changed

- **Breaking:** `ConfigError` is a `thiserror` enum, and `IoError` / `YamlError` / `JsonError` / `TomlError` are struct variants carrying `{ file: Option<String>, source }` instead of stringified messages — so `source()` chains and `Display` names the file. `JsonError` and `TomlError` exist only with their features.
- **Breaking:** `ConfigError` and its struct variants are `#[non_exhaustive]`: a match needs a `_` arm and a trailing `..`. `PathNotFound` and `FormatError` are exempt.
- **Breaking:** `YamlError` and `DeserializeError` carry `ValueError` rather than `yaml_serde::Error`, keeping a `0.x` type out of the public API. Same `Display`, plus `location()`; the upstream variants can no longer be matched.
- **Breaking:** `Config::get_filename()` is renamed to `Config::filename()`.
- **Breaking:** `Config::default()` panics on a present-but-broken `config.yaml` instead of silently returning an empty config. A missing file is still empty.
- **Breaking:** empty path segments are rejected, so `a//b`, `/a/b`, `a/b/` and a bare separator no longer resolve. Escapes (`db/host\/port`) are unaffected, and an empty `fmt` base still means top level.
- **Breaking:** a separator containing a backslash is a `FormatError`; it collided with the escape character, silently making every lookup return nothing.
- **Breaking:** `{env}` is one-directional — an environment with no placeholder is fine, a placeholder with no environment is a `FormatError`.
- **Breaking:** interpolation gained `$${` for a literal `${`, and brace-depth matching so defaults nest (`${A:-${B:-c}}`, capped at 32). Nesting inside a variable *name* is rejected; lone or doubled dollars elsewhere are untouched.
- **Breaking:** `fmt` / `fmt_strict` parse the format string — `{{` / `}}` for literal braces, indexed `{N}` placeholders, and a placeholder/key count mismatch is an error rather than silently dropped.
- **Breaking:** `fmt` keys are paths relative to `base` with the usual escaping, so `fmt("{}", "db", &["redis/port"])` works and a key containing the separator needs `r"a\/b"`.
- **Breaking:** `ConfigHandle::read` returns `Arc<Config>` rather than `RwLockReadGuard`. It derefs, so only code naming the guard type breaks; reads no longer block reloads, and a snapshot is stable across one.
- **Behaviour:** an overlay can clear a value with null, which used to be discarded. Empty and comment-only overlays remain a no-op.
- **Behaviour:** `list_strict` rejects non-scalar *elements*, naming them `path[index]`, instead of rendering them as `""`. `list` is unchanged.
- **Behaviour:** a `fmt` key resolving to a mapping or sequence is a `FormatError` from `fmt_strict`.
- **Behaviour:** `Debug` elides the document, which by then holds interpolated secrets, printing filename, separator, environment, format and overlays with `content: <N keys>`.
- `{env}` in an overlay or reload target resolves against the environment the config already carries, so a layered stack names it once; `reload_from` accepts a placeholder at all now.
- `ConfigHandle::reload` builds the new config with no lock held and takes the write lock only to swap it in; a mutex serializes concurrent reloads.
- Performance: accessors borrow instead of cloning (`deserialize` ~3× faster, `get_as` ~2.5×) and path splitting allocates nothing while scanning (~2× on three segments, ~7× on a hundred). Only `get` and `get_as`, which return owned values, still clone.
- Documentation: `get_as` / `deserialize` are the recommended default and `get` the escape hatch. Newly stated, behaviour unchanged — paths navigate mappings only and match keys as strings; interpolation never touches keys; `load_or_create` does not create parent directories; `env` reaches the filesystem unvalidated; and `Config` is `Send + Sync`, which the docs had denied.
- Internal: `config/mod.rs` split into `loader`, `accessor`, `merge`, `reload`, `path`, `env`, `fmt`, `outline` and `parser`; tests use temp directories and serialize environment mutation.

### Removed

- **Breaking:** `load_json_file` / `load_toml_file` — use `load_required_as(f, sep, env, Format::Json)`.
- **Breaking:** the `YamlError` re-export of `yaml_serde::Error` and `impl From<yaml_serde::Error> for ConfigError` — use `ValueError`.

### Fixed

- **`json`, breaking:** a duplicated JSON key is an error, matching YAML and TOML.
- **`toml`, breaking:** datetimes read as the text in the file instead of `toml_datetime`'s private marker mapping, which broke `str`, `get_as` and `outline`. They no longer deserialize into `toml::value::Datetime`.
- Extension dispatch is case-insensitive and matches the extension rather than the path suffix, so `config.TOML` no longer routes to the YAML parser and a file named `.json` is a dotfile that parses as YAML.
- A leading UTF-8 BOM is stripped for every format and for strings. Previously only `.json` rejected one — and a BOM is what PowerShell's `>` and `Out-File` write.
- JSON and TOML keep document key order instead of arriving alphabetized, making `outline`'s and the merge's order guarantees true for all three formats. No new dependency, MSRV unmoved.
- JSON and TOML parse straight into the value model, so neither can fail with a `YamlError` any more.
- Deep merge keeps mapping key order; `Mapping::remove` is `swap_remove`, so a single override used to displace two keys.
- A YAML `!Tag` is no longer a hole in the value walkers: tagged subtrees are interpolated (an unset `${VAR}` under a tag used to survive silently), deep-merged under the same tag, and transparent to `str`, `get_int`, `list`, `outline` and `Debug`, while `get` / `get_as` still see the tag.
- A variable that is set but not valid Unicode is a `FormatError` naming the cause, instead of being reported as unset or silently taking the default.
- Variables are resolved once per file, at load. The merges re-scanned the merged tree and `reload()` resolved over it once, so a value containing `${` broke or double-expanded.
- The merges record the environment they resolve, unless the config already carries one.
- `load_or_create` is safe on first run: defaults are parsed before anything is written, the file is created with `create_new`, the loser of that race waits out the winner's zero-length window instead of discarding the defaults, defaults parse in the file's own format, and the filename is recorded so `reload()` works.
- `load_optional` records the resolved filename when the file is missing, so a later `reload()` can pick it up. `filename()` is now `""` only for a config parsed from a string.
- `reload_from` commits filename, content and overlays together, so a failed switch no longer leaves the new name with the old content and a stale overlay chain.
- Empty filenames are `IoError(InvalidInput)` everywhere — every loader, `reload_from` and both merges. `merge_optional("")` used to succeed and record a dead overlay.
- `config!` works by full path (`trail_config::config!`), not only when imported by name.
- `fmt` no longer rescans substituted values, so a value containing `{}` is not read as the next placeholder. Missing keys are named with the config's separator instead of a hardcoded `/`.

## [0.4.0] - 2026-04-01

### Changed

- **Breaking:** `Config::new` is now private. Replace usages with `Config::load_optional` (same signature, same behaviour) or `Config::load_required` if the file must exist.
- **Breaking:** `fmt(format, path)` and `fmt_strict(format, path)` now take an explicit `base: &str` and `keys: &[&str]` instead of a single path with `+`-joined leaf keys. The base path and the leaf keys are now separate arguments, removing the need for a special `+` delimiter and making the API consistent with the rest of the library. Error messages for missing keys now report the full path (e.g. `db/redis/nonexistent`) rather than the raw input string.
- `strfmt` dependency removed — string formatting and filename template substitution are now handled with standard Rust `str::replace` / `str::replacen`. No API changes.
- `reload()` now re-reads the base file and re-applies all overlays registered via `merge_required` and `merge_optional`, in the original order. Required overlays that are missing return an error; optional overlays that are missing are silently skipped. If reloading fails the existing configuration is preserved unchanged.
- Replaced `serde_yaml_bw` with `yaml_serde`

### Fixed

- `Config::default()` is now documented as a shorthand for `Config::load_optional("config.yaml", "/", None)`.
- `parse_path` escape detection now correctly requires the *full* separator to follow a backslash before treating it as an escaped separator. Previously, with a multi-character separator like `::`, a lone `\:` would incorrectly match. The escape syntax `\<sep>` (e.g. `\::` for `::`) now works correctly for all separator lengths.
- `fmt_strict` (and `fmt`) now use `parse_path` for path traversal instead of a raw `split`, so escaped separators in path segments work correctly.
- `parse_path` now uses `separator.chars().count()` instead of `separator.len()` when advancing the iterator past a matched separator, fixing incorrect path splitting for multi-byte Unicode separators.
- `parse_path` byte-slicing with multi-byte UTF-8 separators (e.g. `→`) no longer panics. Uses `len_utf8()` instead of a hardcoded byte offset.
- `reload_from` now clears the overlay chain, matching the documented behaviour. Previously, stale overlays from the original file would be re-applied on subsequent `reload()` calls.
- `str_strict` now returns `Err(FormatError)` for non-scalar values (mappings, sequences) instead of `Ok("")`.
- `list_strict` now returns `Err(FormatError)` for non-sequence values instead of `Ok(vec![])`.

### Added

- `load_optional(filename, sep, env)` — a new public constructor for loading optional config files. Returns `Ok` with an empty config if the file is not found, but still returns `Err` for other failures (invalid YAML/JSON/TOML, permission denied, bad separator) so that a present-but-broken config file is not silently ignored. Replaces the former `Config::new`.
- `load_or_create(filename, sep, env, defaults)` — loads a config file if it exists, or writes the provided default YAML string to disk and returns it as the active config if it doesn't. The defaults string is written as-is, preserving formatting and comments. If the file exists its content is used and the defaults are discarded entirely. Errors on invalid YAML in either the file or the defaults string, or on write failure.
- `merge_required(filename: &str, env: Option<&str>) -> Result<Config, ConfigError>` — deep-merges an overlay file on top of `self`. Accepts an optional `{env}` placeholder in the filename, consistent with the load methods. The resolved filename is recorded; if the file is missing during a `reload()` an error is returned.
- `merge_optional(filename: &str, env: Option<&str>) -> Result<Config, ConfigError>` — same as `merge_required` but silently skips the overlay if the file is missing, both at merge time and during `reload()`. Returns `Err` if the file exists but cannot be parsed.
- `deserialize<T>() -> Option<T>` — deserializes the entire config into a typed struct, returning `None` on failure.
- `deserialize_strict<T>() -> Result<T, ConfigError>` — same as `deserialize` but returns `YamlError` on failure.
- `get_as<T>(path)` — deserializes a config subtree at the given path into any `T: DeserializeOwned`, returning `None` on missing path or deserialization failure.
- `get_as_strict<T>(path)` — same as `get_as` but returns `Result<T, ConfigError>`, with `PathNotFound` if the path is missing or `YamlError` if deserialization fails.
- `serde` added as an explicit dependency (with `derive` feature) so users can use `#[derive(Deserialize)]` without adding `serde` themselves.
- `ConfigHandle` — a thread-safe, cloneable handle to a `Config`, wrapping it in `Arc<RwLock<...>>` for safe sharing across threads.
  - `ConfigHandle::new(config)` — wraps an existing `Config`
  - `From<Config>` — allows `config.into()` as a shorthand
  - `clone()` — cheap clone; all clones share the same underlying config
  - `read()` — acquires a shared read lock and returns a guard giving full access to the inner `Config`
  - `reload()` — acquires a write lock and reloads from disk, re-applying all overlays; returns the same errors as `Config::reload()`
  - Convenience pass-through methods for the most common accessors: `str`, `get_int`, `get_float`, `get_bool`, `contains`
  - A poisoned lock is recovered transparently via `into_inner()` on both reads and writes — the config data remains valid since `Config::reload()` only commits changes at the very end
- Regression test `parse_path_escape_requires_full_separator` covering the multi-character separator escape bug.
- Test `fmt_strict_with_escaped_separator_in_path` covering escaped separators in `fmt` paths.
- Environment variable interpolation in string values. `${VAR}` is replaced with the variable's value; `${VAR:-default}` falls back to the default if unset. Variables are resolved at load time and re-resolved on `reload()`. Missing variables without a default return `ConfigError::FormatError`.
- `config!` macro for concise config loading and merging.
- `From<yaml_serde::Error>` impl for `ConfigError`.
- Compile-time `Send + Sync` assertion for `ConfigHandle`.
- `#[must_use]` on `merge_required` and `merge_optional` to prevent silently discarding the merged result.
- JSON config file support behind the `json` feature flag. Auto-detected by `.json` extension in `load_required`/`load_optional`/`load_or_create`, or loaded explicitly with `load_json(str, sep)` and `load_json_file(filename, sep)`. JSON overlays work with `merge_required`/`merge_optional` and are handled correctly on `reload()`.
- `ConfigError::JsonError` variant for JSON-specific parse errors.
- TOML config file support behind the `toml` feature flag. Auto-detected by `.toml` extension in `load_required`/`load_optional`/`load_or_create`, or loaded explicitly with `load_toml(str, sep)` and `load_toml_file(filename, sep)`. TOML overlays work with `merge_required`/`merge_optional` and are handled correctly on `reload()`.
- `ConfigError::TomlError` variant for TOML-specific parse errors.

### Removed

- `to_list` internal helper (inlined into `list` and `list_strict`).

### Migration guide

```rust
// Before (0.3.x)
let config = Config::new("config.yaml", "/", None)?;
let config = Config::new("config.yaml", "::", None)?;
let config = Config::new("config.{env}.yaml", "/", Some("dev"))?;

// After (0.4.0) — no error if file is missing
let config = Config::load_optional("config.yaml", "/", None)?;
let config = Config::load_optional("config.yaml", "::", None)?;
let config = Config::load_optional("config.{env}.yaml", "/", Some("dev"))?;

// Or, if the file must exist
let config = Config::load_required("config.yaml", "/", None)?;
```

```rust
// Before (0.3.x)
config.fmt("{}:{}", "db/redis/server+port");
config.fmt("postgresql://{}@{}:{}/{}", "database/username+host+port+name");

// After (0.4.0)
config.fmt("{}:{}", "db/redis", &["server", "port"]);
config.fmt("postgresql://{}@{}:{}/{}", "database", &["username", "host", "port", "name"]);
```

## [0.3.1] - 2026-03-04

### Added

- `load_required` now explicitly rejects empty filenames with `IoError(InvalidInput)` before attempting file I/O
- Documentation note on `reload` clarifying that a failed reload preserves the existing configuration unchanged
- Documentation note on `parse_path` describing the known limitation of escape detection for multi-character separators (based on first character only)

### Fixed

- Improved test coverage for `load_required` with empty filename — test now asserts the correct `IoError` variant instead of just checking `is_err()`

## [0.3.0] - 2026-03-01

### Added

- `ConfigError` — a custom error enum replacing `Box<dyn Error>` across the entire public API, with four variants:
  - `IoError(io::Error)` — file I/O failures
  - `YamlError(String)` — YAML parse failures
  - `PathNotFound(String)` — missing config path
  - `FormatError(String)` — formatting and configuration errors
- `load_required(filename, sep, env)` — a strict constructor that returns an error if the config file is missing or invalid, intended for production use
- `reload()` — reloads configuration content from the same file at runtime (hot reload)
- `reload_from(filename)` — reloads configuration from a different file, updating the stored filename
- `contains(path)` — checks whether a path exists in the configuration
- Strict API methods that return `Result<T, ConfigError>` instead of `Option<T>` or empty defaults:
  - `get_strict(path)`
  - `str_strict(path)`
  - `list_strict(path)`
  - `fmt_strict(format, base, keys)`
  - `get_int_strict(path)`
  - `get_float_strict(path)`
  - `get_bool_strict(path)`
- Typed accessors for numeric and boolean values:
  - `get_int(path)` → `Option<i64>`
  - `get_float(path)` → `Option<f64>`
  - `get_bool(path)` → `Option<bool>`
- Escape sequence support in path strings — keys containing the separator can be accessed by escaping with `\` (e.g. `database/host\/port`)
- Empty separator validation in `new` and `load_yaml` — returns `FormatError` instead of silently misbehaving
- Comprehensive test suite (58 tests) covering happy paths, missing paths, type mismatches, invalid YAML, edge cases, escape sequences, and hot reload behavior
- Full doc comments on all public methods with `# Arguments`, `# Returns`, `# Errors`, and `# Example` sections
- `serde_yaml_bw` updated from `2.2.0` to `2.5.2`

### Changed

- **Breaking:** All public methods that previously returned `Result<_, Box<dyn Error>>` now return `Result<_, ConfigError>`
- Empty separator now returns a `FormatError` rather than panicking or producing incorrect results

## [0.2.0] - 2025-09-15

### Added

- `get_filename()` — returns the filename of the loaded config file
- `load_yaml(yaml, sep)` — constructs a `Config` directly from a YAML string, without reading from disk
- Initial unit test module with 5 tests: `fmt_test`, `get_leaf_test`, `get_file_test`, `to_string_test`, `to_list_test`

### Changed

- Switched YAML dependency from `serde_yaml` to `serde_yaml_bw 2.2.0`
- Updated `to_string` to handle the new `serde_yaml_bw` value variants that carry an additional tag field (`String(v, _)`, `Number(v, _)`, `Bool(v, _)`)

## [0.1.5] - 2024-06-26

### Changed

- Updated `serde_yaml` from `0.8.21` to `0.9.33`
- Updated `strfmt` from `0.1.6` to `0.2.4`

## [0.1.4] - 2021-12-13

### Added

- `list(path)` — retrieves a YAML sequence as `Vec<String>`, returning an empty vector if the path is missing or not a sequence
- `get_leaf` private helper — centralises nested path traversal, replacing duplicated traversal logic in `get` and `str`
- `to_list` private helper — converts a `serde_yaml::Value::Sequence` to `Vec<String>`

### Changed

- `get` and `str` now delegate to `get_leaf` instead of duplicating path traversal logic

## [0.1.3] - 2021-09-15

### Changed

- **Breaking:** `environment()` now takes `&self` instead of consuming `self`, and returns `Option<&str>` instead of `Option<String>`

## [0.1.2] - 2021-09-15

### Changed

- **Breaking:** `new(filename, sep, env)` now accepts `env: Option<&str>` instead of `Option<String>`, reducing unnecessary allocations at the call site
- `get_file` now returns a tuple `(String, Option<String>)` so the resolved environment name is preserved on the `Config` struct

## [0.1.1] - 2021-09-15

### Changed

- Added package `description` field in `Cargo.toml`
- Removed `yml` from crate keywords (kept `yaml`)

## [0.1.0] - 2021-09-15

### Added

- Initial release
- `Config` struct with path-based access to YAML config files
- `Config::new(filename, sep, env)` — loads a YAML file with optional environment substitution in the filename (e.g. `config.{env}.yaml`)
- `Config::default()` — loads `config.yaml` with `/` separator, falling back to an empty config if the file is missing
- `get(path)` — retrieves a raw `serde_yaml::Value` by path
- `str(path)` — retrieves a value as a `String`, returning an empty string if missing
- `fmt(format, path)` — formats multiple sibling config values into a single string using `+`-joined attribute names (e.g. `"db/host+port"`)
- `environment()` — returns the environment name used when loading the file
- Customisable path separator (e.g. `/`, `::`)
- Environment-specific config file loading via `{env}` placeholder
- Dependencies: `serde_yaml 0.8.21`, `strfmt 0.1.6`

<!-- Every version heading above is a link into the repository: each one compares against its
     predecessor, so a heading answers "what actually changed" as well as naming the release.
     0.5.0 points at HEAD while it is unreleased; on publishing, retag and change it to
     `v0.4.0...v0.5.0`. -->

[0.5.0]: https://github.com/rbt-dev/trail-config/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/rbt-dev/trail-config/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/rbt-dev/trail-config/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/rbt-dev/trail-config/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rbt-dev/trail-config/compare/v0.1.5...v0.2.0
[0.1.5]: https://github.com/rbt-dev/trail-config/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/rbt-dev/trail-config/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/rbt-dev/trail-config/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/rbt-dev/trail-config/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rbt-dev/trail-config/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rbt-dev/trail-config/releases/tag/v0.1.0
