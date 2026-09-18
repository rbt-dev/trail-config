# Error Handling

[← Documentation index](README.md)

## Logging without configuration values

Use `error.safe_diagnostic()` for application logs. Both its `Display` and `Debug`
omit configuration values and underlying error messages:

```rust
use trail_config::Config;

let config = Config::load_yaml("port: secret", "/").unwrap();
if let Err(error) = config.get_as_strict::<u16>("port") {
    eprintln!("{}", error.safe_diagnostic());
    // Cannot deserialize port: expected u16
}
```

The view retains file names, requested configuration paths, the requested Rust type,
I/O error kinds, and available parser locations. The requested type is the whole
target type, not necessarily the nested field that failed. File names and keys are
treated as public metadata; do not embed secrets in them. Free-form `FormatError`
messages are omitted because callers can construct them with arbitrary text.

Ordinary `ConfigError` and `ValueError` `Display`/`Debug`, public `source` fields,
and `std::error::Error::source()` preserve detailed errors and **may expose secrets**.
Parser errors can include input excerpts; deserializers can echo rejected values or
produce arbitrary custom messages. Use these details only in a controlled diagnostic
context. The safe view does not implement `Error` or expose a source chain; log the
view itself, rather than attaching the original error to an error-reporting system.

Crate-generated interpolation, numeric-conversion, and format-template errors do not
echo raw values or template literals. Ordinary messages still include metadata such as
environment-variable names, paths, and separators. `Config`'s redacted `Debug` does
not make errors, raw values, or accessor results safe to log.

## Error variants

Trail Config uses a custom `ConfigError` enum:

```rust
use trail_config::ConfigError;

// - IoError { file, source }    - File I/O errors (missing file, permission denied, etc.)
// - YamlError { file, source }  - YAML parsing or deserialization errors
// - JsonError { file, source }  - JSON parse errors (requires `json` feature)
// - TomlError { file, source }  - TOML parse errors (requires `toml` feature)
// - DeserializeError { file, merged, path, expected_type, source }
//                               - A document or subtree did not match the requested Rust type
// - PathNotFound(String)        - Configuration path not found in document
// - FormatError(String)         - String formatting or configuration errors
```

`DeserializeError` is deliberately separate from the parse errors: the file was read and
parsed successfully, whatever its format, and the mismatch is between the resulting
document and the type you asked for. It names no format — a `.toml` config that fails to
deserialize used to report a "YAML parse error", which pointed at both the wrong format
and a phase that had already succeeded.

For a single-file configuration, `DeserializeError.file` names that file. Once an
overlay chain is registered, `file` is `None` and `merged` is `true`: the error refers
to the merged configuration, because the library does not track which file supplied
each value. For example, if an overlay replaces a numeric `port` with a string,
`safe_diagnostic()` reports `Cannot deserialize port in the merged configuration:
expected u16`, rather than blaming the base file. Whole-document errors use the same
attribution, and the rule also applies to errors returned by reload validators.

This is conservative: even an absent optional overlay, or a value untouched by an
overlay, uses merged attribution. A successful `reload_from` clears the overlay chain
and restores single-file attribution. A string-loaded config without overlays has
`file: None, merged: false`. `Config::filename()` still identifies the base file for
reloading; it is not evidence of an individual value's origin.

Parse and I/O errors still identify the actual file being read. Full per-value
provenance would need to follow replacements, nested merges, tags, and reloads; this
diagnostic correction avoids that added state while making the current limits explicit.

Load and parse errors record the offending file (`file` is `None` when parsing from a
string) and preserve the original underlying error in `source`, which is also returned
by `std::error::Error::source()` for error-chain reporting. Display messages include
the filename when known, e.g. `YAML parse error in config.prod.yaml: ...`.

The `source` types differ by variant, on purpose. `IoError`, `JsonError` and `TomlError`
carry `std::io::Error`, `serde_json::Error` and `toml::de::Error` concretely — all stable
`1.x` types, so naming them costs nothing. `YamlError` and `DeserializeError` carry
`ValueError`, a type of this crate's own, because the value model underneath is a `0.x`
dependency and Cargo treats every `0.x` minor release as semver-incompatible: exposing its
error type directly would make a routine dependency update a breaking change here.
`ValueError` prints exactly what the underlying error printed, and adds `location()`:

```rust
if let Err(ConfigError::YamlError { source, .. }) = Config::load_required("config.yaml", "/", None) {
    match source.location() {
        Some((line, column)) => eprintln!("bad YAML at {line}:{column}"),
        None => eprintln!("bad YAML"),
    }
}
```

## Matching on `ConfigError`

The enum and its struct variants are `#[non_exhaustive]`, so a `match` needs a `_ => ...`
arm and a struct variant's fields are bound with a trailing `..`:

```rust
match result {
    Err(ConfigError::IoError { file, .. }) => { /* ... */ },
    Err(ConfigError::PathNotFound(path)) => { /* ... */ },
    Err(e) => eprintln!("{}", e.safe_diagnostic()),
    Ok(config) => { /* ... */ },
}
```

That keeps a new variant — or a new field on an existing one — from being a breaking
change. The variant list has already grown twice, and the `json` and `toml` features
change which variants exist at all, so this would otherwise force a major version for a
purely additive change.

`PathNotFound` and `FormatError` are exempt: they carry a single `String` that nothing
could be added to, so they stay directly matchable.

## Handling load errors

```rust
use trail_config::{Config, ConfigError};

match Config::load_required("config.yaml", "/", None) {
    Ok(config) => {
        let host = config.str("database/host");
        println!("Connecting to {}", host);
    },
    Err(e @ ConfigError::IoError { .. }) => {
        eprintln!("Config file error: {}", e.safe_diagnostic());
    },
    Err(e @ ConfigError::YamlError { .. }) => {
        eprintln!("Invalid YAML: {}", e.safe_diagnostic());
    },
    Err(e) => eprintln!("Config error: {}", e.safe_diagnostic()),
}
```

## Handling strict method errors

```rust
use trail_config::{Config, ConfigError};

let config = Config::default();

match config.str_strict("database/host") {
    Ok(host) => println!("Connecting to {}", host),
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Missing required config: {}", path);
    },
    Err(e) => eprintln!("Config error: {}", e.safe_diagnostic()),
}

match config.str_strict("database") {
    Ok(value) => println!("Database: {}", value),
    Err(e @ ConfigError::FormatError(_)) => {
        eprintln!("Not a scalar: {}", e.safe_diagnostic());
    },
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Not found: {}", path);
    },
    Err(e) => eprintln!("Unexpected error: {}", e.safe_diagnostic()),
}

match config.get_int_strict("app/port") {
    Ok(port) => println!("Port: {}", port),
    Err(e @ ConfigError::FormatError(_)) => {
        eprintln!("Port value has wrong type: {}", e.safe_diagnostic());
    },
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Port config not found: {}", path);
    },
    Err(e) => eprintln!("Unexpected error: {}", e.safe_diagnostic()),
}
```

## Input validation

Trail Config validates inputs automatically and returns `FormatError` for invalid configurations:

| Input | Constraint | Error |
| ----- | ---------- | ----- |
| Path separator | Cannot be empty, and cannot contain `\` (the escape character) | Returns `FormatError` |
| File paths | Empty filename rejected upfront by every loader (`load_required`, `load_optional`, `load_or_create`), both merges (`merge_required`, `merge_optional`) and `reload_from` | Returns `IoError` (`InvalidInput`) |
| Paths | Empty paths rejected | Returns `None` or empty / `PathNotFound` |
| Path segments | Must be non-empty — leading, trailing and doubled separators are rejected | Returns `None` or empty / `PathNotFound` |
| Filename templates | Must be valid format strings | Returns `FormatError` |

```rust
// Empty separator - error
let result = Config::load_optional("config.yaml", "", None);
assert!(result.is_err()); // FormatError

// Separator containing the escape character - error
let result = Config::load_optional("config.yaml", "\\", None);
assert!(result.is_err()); // FormatError

// Empty filename - rejected upfront by all loaders with IoError (InvalidInput)
let result = Config::load_required("", "/", None);
assert!(result.is_err()); // IoError (InvalidInput)

let result = Config::load_optional("", "/", None);
assert!(result.is_err()); // IoError (InvalidInput) — no longer silently returns an empty config

let result = Config::load_or_create("", "/", None, "app:\n  port: 1\n");
assert!(result.is_err()); // IoError (InvalidInput)

// Same for the merges — an empty overlay filename is a caller bug, not an absent file
let result = Config::load_required("config.yaml", "/", None)?.merge_optional("", None);
assert!(result.is_err()); // IoError (InvalidInput) — no longer a silent no-op

// Missing file with load_required - error
let result = Config::load_required("missing.yaml", "/", None);
assert!(result.is_err()); // IoError

// Missing file with load_optional - ok, returns empty config
let config = Config::load_optional("missing.yaml", "/", None)?;
assert!(config.str("any/path") == ""); // Graceful fallback
```

