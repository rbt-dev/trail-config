# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

### Changed
- **Breaking:** `Config::new` is now private. Replace usages with `Config::load_optional` (same signature, same behaviour) or `Config::load_required` if the file must exist.
- **Breaking:** `fmt(format, path)` and `fmt_strict(format, path)` now take an explicit `base: &str` and `keys: &[&str]` instead of a single path with `+`-joined leaf keys. The base path and the leaf keys are now separate arguments, removing the need for a special `+` delimiter and making the API consistent with the rest of the library. Error messages for missing keys now report the full path (e.g. `db/redis/nonexistent`) rather than the raw input string.
- `strfmt` dependency removed — string formatting and filename template substitution are now handled with standard Rust `str::replace` / `str::replacen`. No API changes.

### Fixed
- `Config::default()` is now documented as a shorthand for `Config::load_optional("config.yaml", "/", None)`.
- `parse_path` escape detection now correctly requires the *full* separator to follow a backslash before treating it as an escaped separator. Previously, with a multi-character separator like `::`, a lone `\:` would incorrectly match. The escape syntax `\<sep>` (e.g. `\::` for `::`) now works correctly for all separator lengths.
- `fmt_strict` (and `fmt`) now use `parse_path` for path traversal instead of a raw `split`, so escaped separators in path segments work correctly.

### Added
- `load_optional(filename, sep, env)` — a new public constructor for loading optional config files. Returns `Ok` with an empty config if the file is not found, but still returns `Err` for other failures (invalid YAML, permission denied, bad separator) so that a present-but-broken config file is not silently ignored. Replaces the former `Config::new`.
- `load_or_create(filename, sep, env, defaults)` — loads a config file if it exists, or writes the provided default YAML string to disk and returns it as the active config if it doesn't. The defaults string is written as-is, preserving formatting and comments. If the file exists its content is used and the defaults are discarded entirely. Errors on invalid YAML in either the file or the defaults string, or on write failure.
- `merge(overlay: Config) -> Config` — deep-merges another config on top of `self`, returning a new `Config`. Overlay values take precedence over base values. Mappings are merged recursively so sibling keys are preserved; sequences are replaced wholesale. The returned config inherits the separator and filename of the base. Calls can be chained: `base.merge(env).merge(local)`.
- `get_as<T>(path)` — deserializes a config subtree at the given path into any `T: DeserializeOwned`, returning `None` on missing path or deserialization failure
- `get_as_strict<T>(path)` — same as `get_as` but returns `Result<T, ConfigError>`, with `PathNotFound` if the path is missing or `YamlError` if deserialization fails
- `serde` added as an explicit dependency (with `derive` feature) so users can use `#[derive(Deserialize)]` without adding `serde` themselves
- Regression test `parse_path_escape_requires_full_separator` covering the multi-character separator escape bug
- Test `fmt_strict_with_escaped_separator_in_path` covering escaped separators in `fmt` paths

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

## [0.3.1] - 2026-04-03

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

## [0.1.0] - Unreleased

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
