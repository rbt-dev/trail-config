# Error Handling

[← Documentation index](README.md)

Trail Config uses a custom `ConfigError` enum:

```rust
use trail_config::ConfigError;

// - IoError { file, source }    - File I/O errors (missing file, permission denied, etc.)
// - YamlError { file, source }  - YAML parsing or deserialization errors
// - JsonError { file, source }  - JSON parse errors (requires `json` feature)
// - TomlError { file, source }  - TOML parse errors (requires `toml` feature)
// - DeserializeError { file, path, source }
//                               - A document or subtree did not match the requested Rust type
// - PathNotFound(String)        - Configuration path not found in document
// - FormatError(String)         - String formatting or configuration errors
```

`DeserializeError` is deliberately separate from the parse errors: the file was read and
parsed successfully, whatever its format, and the mismatch is between the resulting
document and the type you asked for. It names no format — a `.toml` config that fails to
deserialize used to report a "YAML parse error", which pointed at both the wrong format
and a phase that had already succeeded.

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
        Some((line, column)) => eprintln!("bad YAML at {line}:{column}: {source}"),
        None => eprintln!("bad YAML: {source}"),
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
    Err(e) => eprintln!("{}", e),
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
    Err(ConfigError::IoError { file, source, .. }) => {
        eprintln!("Config file error in {}: {}", file.as_deref().unwrap_or("?"), source);
    },
    Err(ConfigError::YamlError { source, .. }) => {
        eprintln!("Invalid YAML: {}", source);
    },
    Err(e) => eprintln!("Config error: {}", e),
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
    Err(e) => eprintln!("Config error: {}", e),
}

match config.str_strict("database") {
    Ok(value) => println!("Database: {}", value),
    Err(ConfigError::FormatError(msg)) => {
        eprintln!("Not a scalar: {}", msg);
    },
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Not found: {}", path);
    },
    Err(e) => eprintln!("Unexpected error: {}", e),
}

match config.get_int_strict("app/port") {
    Ok(port) => println!("Port: {}", port),
    Err(ConfigError::FormatError(msg)) => {
        eprintln!("Port value has wrong type: {}", msg);
    },
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Port config not found: {}", path);
    },
    Err(e) => eprintln!("Unexpected error: {}", e),
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

