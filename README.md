# Trail Config

A Rust library for reading config files with path-based access, typed deserialization, environment overlays, deep merging, env variable interpolation, and hot reload support.

## Features

- 📖 Simple path-based config value access
- 🔧 Customizable path separators (`/`, `::`, etc.)
- 🌍 Environment-specific config files
- 🌐 Environment variable interpolation with defaults (`${VAR}`, `${VAR:-default}`)
- 📝 String formatting and interpolation
- ✅ Comprehensive error handling with custom `ConfigError` type
- 📋 Type conversion for strings, numbers, booleans, and sequences
- 🏗️ Struct deserialization — map the entire config or any subtree directly into a typed Rust struct
- 🔐 Escape sequence support for keys containing separators
- 🔄 Hot reload support for detecting configuration changes at runtime
- 🔀 Deep merge support for layering environment-specific config overlays
- 🆕 Auto-create config files from in-code defaults on first run
- 🧵 Thread-safe `ConfigHandle` for sharing config across threads
- ⚡ `config!` macro for concise loading and merging
- 📂 JSON and TOML support via optional feature flags

## Quick Start

```rust
use trail_config::Config;

// Load config.yaml file
let config = Config::default();

// Get values with lenient API (returns empty/None on missing)
let port = config.str("app/port");           // -> "8080"
let timeout = config.get_int("app/timeout"); // -> Some(30)

// Or use strict API for explicit error handling
match config.str_strict("database/host") {
    Ok(host) => println!("Connecting to {}", host),
    Err(e) => eprintln!("Config error: {}", e),
}
```

## Loading Configuration

Trail Config exposes four constructors with a clear, symmetric design:

| Constructor | File required? | Use case |
| ----------- | -------------- | -------- |
| `Config::load_required(filename, sep, env)` | Yes — errors if missing | Production: config must exist |
| `Config::load_optional(filename, sep, env)` | No — returns empty config if missing | Optional or environment-specific files |
| `Config::load_or_create(filename, sep, env, defaults)` | No — creates from defaults if missing | First-run config generation |
| `Config::default()` | No | Shorthand for `load_optional("config.yaml", "/", None)`. Empty if missing, **panics if broken** |

### Required config (production)

Use `Config::load_required()` when the configuration file **must** exist:

```rust
use trail_config::Config;

let config = Config::load_required("config.yaml", "/", None)?;
// Errors if file is missing, invalid YAML/JSON/TOML, or permission denied
```

### Optional config

Use `Config::load_optional()` for custom filenames or separators when the file may not exist:

```rust
use trail_config::Config;

// With custom separator
let config = Config::load_optional("config.yaml", "::", None)?;

// With environment substitution
let config = Config::load_optional("config.{env}.yaml", "/", Some("dev"))?;
```

A missing file yields an empty config that still **remembers the filename it looked for**, 
so a file created later is picked up by `reload()`:

```rust
let mut config = Config::load_optional("config.local.yaml", "/", None)?;
assert_eq!(config.get_filename(), "config.local.yaml"); // recorded even though absent

// Errors with IoError (NotFound) while the file is still missing, leaving the
// config unchanged; succeeds as soon as it appears.
config.reload()?;
```

### The `{env}` placeholder

Every method that takes a filename also takes an optional environment name. The rule is:
**the environment supplies a value for `{env}` if the filename uses it.**

| Filename | `env` | Result |
| -------- | ----- | ------ |
| `config.{env}.yaml` | `Some("prod")` | Loads `config.prod.yaml` |
| `config.yaml` | `Some("prod")` | Loads `config.yaml`; the environment is still recorded |
| `config.{env}.yaml` | `None` | `FormatError` — nothing to substitute |
| `config.yaml` | `None` | Loads `config.yaml` |

A filename without the placeholder is deliberately **not** an error. In a layered setup only 
some files are environment-specific, and the same `env` is applied to all of them:

```rust
let config = Config::load_required("config.yaml", "/", Some("prod"))?  // no placeholder
    .merge_required("config.{env}.yaml", Some("prod"))?                // placeholder
    .merge_optional("config.local.yaml", Some("prod"))?;               // no placeholder

assert_eq!(config.environment(), Some("prod"));
```

The reverse is an error, because a `{env}` with nothing to fill it would otherwise be passed to 
the OS verbatim and fail as a missing file called `config.{env}.yaml` — an error that points at 
the wrong problem.

`reload_from()` also accepts `{env}`, resolved against the environment the config already 
carries; it has no `env` argument because it preserves the existing one.

**`env` is substituted into a filesystem path verbatim**, with no validation — an `env` of 
`../../secrets` builds exactly that path. This is fine for the normal case of a literal or a 
trusted `APP_ENV`, but do not pass a value that came from an untrusted source such as a request 
parameter or an uploaded file.

### Default (shorthand)

Use `Config::default()` when `config.yaml` with `/` separator is acceptable and the file is optional. A missing file yields an empty config; a present-but-**broken** file panics (use `load_optional` to get the error instead):

```rust
let config = Config::default(); // Empty if missing; panics if config.yaml is broken
```

### From a YAML string

Use `Config::load_yaml()` to load configuration directly from a string rather than a file.
This is useful for tests, embedded defaults, or configs received over the network:

```rust
let config = Config::load_yaml("app:\n  port: 8080", "/")?;
```

### From a JSON file or string (requires `json` feature)

Enable the `json` feature in your `Cargo.toml`:

```toml
[dependencies]
trail-config = { version = "0.5", features = ["json"] }
```

JSON files are auto-detected by extension (case-insensitively — `.json`, `.JSON` and
`.Json` all reach the JSON parser):

```rust
use trail_config::Config;

// Auto-detected by .json extension
let config = Config::load_required("config.json", "/", None)?;

// Or load explicitly from a string
let config = Config::load_json(r#"{"app": {"port": 8080}}"#, "/")?;

// Mix YAML base with JSON overlay
let config = Config::load_required("config.yaml", "/", None)?
    .merge_required("overrides.json", None)?;
```

### From a TOML file or string (requires `toml` feature)

Enable the `toml` feature in your `Cargo.toml`:

```toml
[dependencies]
trail-config = { version = "0.5", features = ["toml"] }
```

TOML files are auto-detected by extension (case-insensitively — `.toml`, `.TOML` and
`.Toml` all reach the TOML parser):

```rust
use trail_config::Config;

// Auto-detected by .toml extension
let config = Config::load_required("config.toml", "/", None)?;

// Or load explicitly from a string
let config = Config::load_toml("[app]\nport = 8080", "/")?;

// Mix formats freely
let config = Config::load_required("config.yaml", "/", None)?
    .merge_required("overrides.toml", None)?;
```

### Using the `config!` macro

The `config!` macro provides a concise syntax for loading and merging configs:

```rust
use trail_config::config;

// Minimal
let config = config!("config.yaml")?;

// With custom separator
let config = config!("config.yaml", sep: "::")?;

// With environment
let config = config!("config.{env}.yaml", env: "prod")?;

// With merges
let config = config!("config.yaml", merge: ["config.prod.yaml"])?;

// Full syntax
let config = config! {
    file: "config.yaml",
    sep: "/",
    env: "prod",
    merge: ["config.{env}.yaml"],
    merge_optional: ["config.local.yaml"],
}?;
```

## API Overview

Trail Config organizes methods into two styles. Every method has both a lenient and a strict variant:

| Style | Returns | Behaviour on missing path |
| ----- | ------- | ------------------------- |
| Lenient — `get()`, `str()`, `get_int()`, etc. | `Option<T>` or empty default | Returns `None` or `""` / `[]` |
| Strict — `get_strict()`, `str_strict()`, `get_int_strict()`, etc. | `Result<T, ConfigError>` | Returns `Err(PathNotFound)` |

Both styles share the same path syntax and navigate nested config values using separators (default: `/`).

### Path syntax

A path is a list of keys joined by the separator. **Every segment must be non-empty**, so a 
leading, trailing or doubled separator makes the lookup fail rather than being quietly ignored:

| Path | Result |
| ---- | ------ |
| `db/redis/port` | Resolves |
| `/db/redis/port` | Fails — empty leading segment |
| `db/redis/port/` | Fails — empty trailing segment |
| `db//redis/port` | Fails — empty middle segment |
| `/` | Fails — no segments at all |
| `""` | Fails — empty path |

Failing means `None` / `""` / `[]` from the lenient methods and `PathNotFound` from the strict 
ones, naming the path as written. A key that genuinely contains the separator is reached with an 
escape sequence — see [Escape Sequences](#escape-sequences) — not by doubling it.

**Paths navigate mappings only.** A sequence has no addressable elements: `items/0` is not a 
path into the first element, just a lookup for a key named `0`, and it fails like any other 
missing key. Read the whole sequence with `list(path)`, or deserialize it into a typed field 
with `get_as` / `deserialize`.

### Reading values

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `get(path)` | `Option<Value>` | Raw value — `trail_config::Value` |
| `get_strict(path)` | `Result<Value, ConfigError>` | Raw value, errors if missing |
| `str(path)` | `String` | String representation, empty if missing |
| `str_strict(path)` | `Result<String, ConfigError>` | String, errors if missing |
| `list(path)` | `Vec<String>` | Sequence as string vector, empty if missing; non-scalar elements become `""` |
| `list_strict(path)` | `Result<Vec<String>, ConfigError>` | Sequence, errors if missing or if any element is not a scalar |
| `contains(path)` | `bool` | Returns `true` if path exists |

`list_strict` checks the elements as well as the container. A nested mapping, a nested
sequence or a null among them is a `FormatError` naming the element as `path[index]` —
brackets rather than a path, because a sequence element is not addressable as one:

```yaml
sources:
  - one
  - [nested, sequence]   # list_strict: FormatError, "Value at sources[1] is not a scalar"
                         # list:        flattens it to "", like an element that is ""
```

The raw value type and the errors behind `ConfigError` are re-exported, so nothing the API
hands back needs a second dependency to name:

```rust
use trail_config::{Config, ConfigError, Value, YamlError};

let config = Config::load_required("config.yaml", "/", None)?;

match config.get("app/port") {
    Some(Value::Number(n)) => println!("port {}", n),
    Some(other) => eprintln!("app/port is not a number: {:?}", other),
    None => eprintln!("app/port is not set"),
}
```

`Value`, `Mapping`, `Sequence`, `Number` and `YamlError` come from
[`yaml_serde`](https://docs.rs/yaml_serde); `JsonError` and `TomlError` are re-exported
with the corresponding feature. Use these names rather than depending on the underlying
crates directly — a version that differs from the one this crate resolved produces two
incompatible `Value` types.

### Typed access

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `get_int(path)` | `Option<i64>` | Integer value |
| `get_int_strict(path)` | `Result<i64, ConfigError>` | Integer, errors if missing or wrong type |
| `get_float(path)` | `Option<f64>` | Floating-point value |
| `get_float_strict(path)` | `Result<f64, ConfigError>` | Float, errors if missing or wrong type |
| `get_bool(path)` | `Option<bool>` | Boolean value |
| `get_bool_strict(path)` | `Result<bool, ConfigError>` | Boolean, errors if missing or wrong type |
| `get_as<T>(path)` | `Option<T>` | Deserialize subtree into typed struct |
| `get_as_strict<T>(path)` | `Result<T, ConfigError>` | Deserialize subtree, errors if missing or type mismatch |
| `deserialize<T>()` | `Option<T>` | Deserialize entire config into typed struct |
| `deserialize_strict<T>()` | `Result<T, ConfigError>` | Deserialize entire config, errors on type mismatch |

### Formatting

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `fmt(format, base, keys)` | `String` | Format sibling values into a string, empty on error |
| `fmt_strict(format, base, keys)` | `Result<String, ConfigError>` | Format, errors if the template is invalid or any value is missing |

### Metadata and hot reload

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `get_filename()` | `&str` | Resolved filename of the config; `""` only for configs parsed from a string |
| `environment()` | `Option<&str>` | Environment name used when loading |
| `reload()` | `Result<(), ConfigError>` | Reload from current file |
| `reload_from(filename)` | `Result<(), ConfigError>` | Load from a different file |

## Debug Output

`{:?}` on a `Config` or `ConfigHandle` prints the config's **shape**, never its values:

```text
Config { filename: "config.yaml", separator: "/", environment: Some("prod"),
         overlays: [Required("config.prod.yaml"), Optional("config.local.yaml")],
         content: <4 keys> }
```

Environment variables are already interpolated by the time `Debug` runs, so printing the 
document would put `${DB_PASSWORD}` and `${API_TOKEN}` in cleartext into any panic message, 
log line or `anyhow` context chain. Filenames and the overlay chain are printed — they are 
not secrets, and they are what you need when a `reload()` does not do what you expected.

To inspect actual values, read them explicitly with the accessors.

## Error Handling

Trail Config uses a custom `ConfigError` enum:

```rust
use trail_config::ConfigError;

// - IoError { file, source }    - File I/O errors (missing file, permission denied, etc.)
// - YamlError { file, source }  - YAML parsing or deserialization errors
// - JsonError { file, source }  - JSON parse errors (requires `json` feature)
// - TomlError { file, source }  - TOML parse errors (requires `toml` feature)
// - PathNotFound(String)        - Configuration path not found in document
// - FormatError(String)         - String formatting or configuration errors
```

Load and parse errors record the offending file (`file` is `None` when parsing from a
string) and preserve the original underlying error in `source`, which is also returned
by `std::error::Error::source()` for error-chain reporting. Display messages include
the filename when known, e.g. `YAML parse error in config.prod.yaml: ...`.

### Handling load errors

```rust
use trail_config::{Config, ConfigError};

match Config::load_required("config.yaml", "/", None) {
    Ok(config) => {
        let host = config.str("database/host");
        println!("Connecting to {}", host);
    },
    Err(ConfigError::IoError { file, source }) => {
        eprintln!("Config file error in {}: {}", file.as_deref().unwrap_or("?"), source);
    },
    Err(ConfigError::YamlError { source, .. }) => {
        eprintln!("Invalid YAML: {}", source);
    },
    Err(e) => eprintln!("Config error: {}", e),
}
```

### Handling strict method errors

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

### Input validation

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

## Typed Access

Convert config values to Rust primitives safely:

```rust
let config = Config::default();

// Lenient - returns None on missing or type mismatch
let port = config.get_int("app/port");
let timeout = config.get_float("app/timeout");
let debug = config.get_bool("app/debug");

if let Some(port) = port {
    println!("Listening on port {}", port);
}

// Strict - returns error details
match config.get_int_strict("app/port") {
    Ok(port) => println!("Port: {}", port),
    Err(e) => eprintln!("Failed to read port: {}", e),
}
```

Example config (YAML):

```yaml
app:
  port: 8080
  timeout: 30.5
  debug: true
```

## Struct Deserialization

Use `deserialize` / `deserialize_strict` to map the **entire config** into a typed Rust struct, 
or `get_as` / `get_as_strict` to deserialize a subtree at a specific path. Both approaches are 
more concise than reading fields one by one, and let the compiler verify you haven't missed any 
required fields.

Any struct that derives `serde::Deserialize` can be used:

```rust
use serde::Deserialize;
use trail_config::Config;

#[derive(Deserialize)]
struct FullConfig {
    app: AppConfig,
    database: DatabaseConfig,
}

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
    debug: bool,
    timeout: f64,
}

#[derive(Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
}

let config = Config::load_required("config.yaml", "/", None)?;

// Deserialize the entire config at once
let full: FullConfig = config.deserialize_strict()?;

// Or deserialize just a subtree
let db: DatabaseConfig = config.get_as_strict("database")?; // Strict — returns a descriptive error on failure
let db: Option<DatabaseConfig> = config.get_as("database"); // Lenient — returns None if path is missing or struct doesn't match
```

`deserialize_strict` returns `YamlError` if the config can't be deserialized into `T`.
`get_as_strict` additionally returns `PathNotFound` if the path doesn't exist.

Sample YAML:

```yaml
app:
  port: 8080
  debug: false
  timeout: 30.0

database:
  host: localhost
  port: 5432
  username: admin
  password: secret
```

## String Formatting

Use `fmt()` to combine multiple sibling config values into a formatted string in a single call:

```rust
// Instead of:
let host = config.str("database/host");
let port = config.str("database/port");
let connection = format!("{}:{}", host, port);

// You can write:
let connection = config.fmt("{}:{}", "database", &["host", "port"]);
```

The `fmt()` method takes a format string, a base path to the parent node, and a slice of key 
names. It navigates to the base path, then substitutes the specified keys into the template.

`base` and each key use the same path syntax as every other accessor — the separator splits 
them, `\` escapes a literal separator, and empty segments are rejected. Keys are paths 
*relative to* `base`, so they are usually a single sibling key but may reach deeper:

```rust
// These are equivalent
config.fmt("{}:{}", "db/redis", &["server", "port"]);
config.fmt("{}:{}", "db",       &["redis/server", "redis/port"]);
```

Each key must resolve to a scalar; one naming a mapping or a sequence is an error from 
`fmt_strict` and an empty string from `fmt`.

### Placeholder syntax

| Syntax | Meaning |
| ------ | ------- |
| `{}` | The next unused key, left to right |
| `{N}` | `keys[N]` — may reorder and repeat keys |
| `{{` / `}}` | A literal `{` / `}` |

Auto-numbered and indexed placeholders can be mixed, and `{}` counts only its own occurrences — 
the same rules as `std::format!`:

```rust
// db:
//   redis:
//     server: 127.0.0.1
//     port: 6379

let s = config.fmt("{{{0}:{1}}} via {0}", "db/redis", &["server", "port"]);
// Result: "{127.0.0.1:6379} via 127.0.0.1"
```

Every placeholder must have a corresponding key and every key must be used at least once. A 
mismatch in either direction is an error (empty string from `fmt`, `FormatError` from 
`fmt_strict`) rather than a silently half-formatted result. Substituted values are never 
rescanned, so a config value that itself contains `{}` is emitted verbatim.

### Multi-value formatting

```rust
// database:
//   host: localhost
//   port: 5432
//   name: myapp_db
//   username: admin

let db_url = config.fmt(
    "postgresql://{}@{}:{}/{}",
    "database",
    &["username", "host", "port", "name"]
);
// Result: "postgresql://admin@localhost:5432/myapp_db"
```

### Lenient vs strict

```rust
// Lenient - returns empty string if any value is missing
let connection = config.fmt("{}:{}", "database", &["host", "port"]);

// Strict - returns error if any value is missing
let connection = config.fmt_strict("{}:{}", "database", &["host", "port"])?;
```

### Escape sequences in fmt

If a key contains the separator, escape it with `\` — in the base path and in the keys 
alike, since both are ordinary paths:

```rust
// sections:
//   "db/redis":        <- key contains a literal slash
//     server: 127.0.0.1
//     port: 6379

let connection = config.fmt("{}:{}", r"sections/db\/redis", &["server", "port"]);
// Result: "127.0.0.1:6379"

// db:
//   "a/b": 1

let value = config.fmt("{}", "db", &[r"a\/b"]);
// Result: "1"
```

## Escape Sequences

Keys containing the path separator can be accessed using escape sequences.

- `\<sep>` — include a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
- `\\` — include a literal backslash in the key
- Works with any separator: `/`, `::`, `->`, etc.

Because `\` is the escape character, a separator may not contain one — the splitter would 
read it as the start of an escape sequence and never match it as a separator. Constructing 
a config with such a separator is a `FormatError`.

```yaml
database:
  "host/port": localhost:5432      # Key contains /
  "user\name": admin\user          # Key contains \
```

```rust
let config = Config::load_yaml(yaml, "/").unwrap();

// Access key containing separator (/)
let value = config.str("database/host\\/port"); // -> "localhost:5432"

// Access key containing backslash (\)
let value = config.str("database/user\\\\name"); // -> "admin\user"
```

With a custom separator:

```rust
let config = Config::load_yaml(yaml, "::").unwrap();

// Path: a::b\::c::d navigates to keys ["a", "b::c", "d"]
let value = config.str("a::b\\::c::d");
```

## Thread-Safe Shared Config

`Config` is not `Send + Sync` on its own. Use `ConfigHandle` to share a `Config` across threads 
and reload it at runtime without restarting. It wraps `Config` in an `Arc<RwLock<Arc<Config>>>` — cloning 
the handle is cheap, and all clones refer to the same underlying config.

```rust
use trail_config::{Config, ConfigHandle};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)?
);

// Cheap to clone — share across threads
let handle2 = handle.clone();

// Convenience methods for common accessors
let port = handle.get_int("app/port");
let debug = handle.get_bool("app/debug");
let features = handle.list("features/beta");

// Full Config access via a snapshot (Arc<Config> derefs to Config)
let db: DatabaseConfig = handle.read().get_as_strict("database")?;

// Reload from disk — re-applies all overlays
handle.reload()?;
// All clones immediately see the updated values
```

Neither reads nor reloads hold a lock for long. `read()` locks only long enough to clone an
`Arc` and returns an immutable **snapshot**; `reload()` copies the source list (base filename
plus the overlay chain), does all file reads and parsing with **no lock held**, and takes the
write lock only for a pointer swap. So readers are never blocked on disk I/O, and holding a
snapshot never blocks a reload. If the reload fails, no swap happens and the existing config
is left unchanged.

`ConfigHandle` mirrors the complete **lenient** accessor surface of `Config` — `get`, `str`,
`list`, `contains`, `get_int`, `get_float`, `get_bool`, `get_as`, `deserialize` and `fmt` — each
a shorthand for the same call on a snapshot. The `*_strict` variants and the metadata accessors
(`get_filename`, `environment`) are reached through `read()`, which gives access to every
`Config` method.

Because a snapshot is immutable, a concurrent reload can never change it underneath you — take
one snapshot when you need several values to agree (each convenience call takes its own):

```rust
let snapshot = handle.read();
let host = snapshot.str("database/host");
let port = snapshot.get_int("database/port"); // guaranteed to match `host`
```

### Background reload example

```rust
use trail_config::{Config, ConfigHandle};
use std::{thread, time::Duration};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)?
        .merge_optional("config.local.yaml", None)?
);

// Spawn a background thread to reload every 30 seconds
let reload_handle = handle.clone();
thread::spawn(move || {
    loop {
        thread::sleep(Duration::from_secs(30));
        if let Err(e) = reload_handle.reload() {
            eprintln!("Config reload failed: {}", e);
        }
    }
});

// Main thread reads are never blocked except during the brief reload swap
loop {
    let timeout = handle.get_int("app/timeout").unwrap_or(30);
    // ...
}
```

## Hot Reload

Detect and apply configuration changes at runtime without restarting:

```rust
let mut config = Config::load_required("config.yaml", "/", None)?
    .merge_required("config.prod.yaml", None)?
    .merge_optional("config.local.yaml", None)?;

// Reloads base file and re-applies all overlays in order.
// Required overlays that are missing return an error;
// optional overlays that are missing are silently skipped.
// If reload fails, the existing configuration is preserved unchanged.
config.reload()?;

// Or switch to a different config file (clears overlay chain)
config.reload_from("other_config.yaml")?;
```

### Server loop example

```rust
use trail_config::Config;
use std::thread;
use std::time::Duration;

fn main() {
    let mut config = Config::load_required("config.yaml", "/", None)
        .expect("Failed to load config")
        .merge_optional("config.local.yaml", None)
        .expect("Failed to merge local config");

    loop {
        // Check for config updates every 5 seconds
        if let Ok(_) = config.reload() {
            println!("✓ Configuration reloaded");

            let timeout = config.get_int("app/timeout").unwrap_or(30);
            let debug = config.get_bool("app/debug").unwrap_or(false);

            println!("Timeout: {} seconds, Debug: {}", timeout, debug);
        }

        // Main application logic here
        thread::sleep(Duration::from_secs(5));
    }
}
```

## Merging Configs

Use `merge_required` / `merge_optional` to layer configs on top of each other. Values in 
the overlay take precedence over the base; nested mappings are merged recursively so sibling 
keys are preserved. Sequences are replaced wholesale. The base config's separator is preserved.

Key order follows the base document: an overridden key keeps its position and genuinely-new 
overlay keys are appended, at every level of nesting. The merged order therefore does not 
depend on the order of the overlays. This is invisible through the scalar accessors but 
visible to anything order-preserving — deserializing into a `Mapping`, an `IndexMap` or a 
`Vec<(K, V)>`, and any re-serialization you do downstream.

The overlay filenames are recorded so that `reload()` can re-read and re-apply them in 
order — required overlays that are missing on reload return an error, optional overlays that 
are missing are silently skipped.

### Clearing a value with null

An overlay value takes precedence whatever it is, including null — so setting a key to null 
is how an overlay clears an inherited value:

```yaml
# config.local.yaml
database:
  password:        # bare `key:` is YAML for null — clears the base password
telemetry: null    # explicit form, identical in effect
cache:             # a subtree set to null clears the whole subtree
```

The key is *cleared*, not removed: it remains present holding a null, so `contains` still 
returns `true` while `str` returns `""` and the typed accessors (`get_int`, `get_bool`, …) 
return `None`. `get(path)` tells the two apart: a cleared key yields `Some` holding a null 
value, a key that was never set yields `None`.

An overlay file that is **entirely** empty — zero bytes, or nothing but comments — is the one 
exception. It is a no-op rather than a document-wide clear, which is what makes an absent 
`merge_optional` file and an empty one behave the same way.

```rust
use trail_config::Config;

let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

let mut config = Config::load_required("config.yaml", "/", None)?
    .merge_required("config.{env}.yaml", Some(&env))?
    .merge_optional("config.local.yaml", None)?;
```

Given these files:

```yaml
# config.yaml (base)
app:
  port: 8080
  debug: false
  name: myapp
database:
  host: localhost
  port: 5432
```

```yaml
# config.prod.yaml (overlay)
app:
  debug: false
database:
  host: prodserver
```

```yaml
# config.local.yaml (optional personal overrides)
app:
  debug: true
```

The merged result will be:

```yaml
app:
  port: 8080        # from base
  debug: true       # from config.local.yaml (last overlay wins)
  name: myapp       # from base
database:
  host: prodserver  # from config.prod.yaml
  port: 5432        # from base — sibling preserved
```

## Environment Variable Interpolation

Trail Config resolves `${VAR}` placeholders in string values at load time using environment 
variables. Placeholders can include a default value with `${VAR:-default}`.

```yaml
# config.yaml
database:
  host: ${DB_HOST:-localhost}
  port: 5432
  password: ${DB_PASSWORD}
app:
  url: ${APP_PROTO:-https}://${APP_DOMAIN}/api
```

```rust
use trail_config::Config;

// If DB_HOST=prodserver and DB_PASSWORD=secret are set:
let config = Config::load_required("config.yaml", "/", None)?;
assert_eq!(config.str("database/host"), "prodserver");
assert_eq!(config.str("database/password"), "secret");
assert_eq!(config.str("app/url"), "https://example.com/api");
```

### Syntax

| Pattern | Behaviour |
| ------- | --------- |
| `${VAR}` | Replaced with the value of `VAR`. Error if not set. |
| `${VAR:-default}` | Replaced with the value of `VAR`, or `default` if `VAR` is not set. |
| `${VAR:-}` | Replaced with the value of `VAR`, or an empty string if not set. |
| `${VAR:-${OTHER:-x}}` | Defaults may nest — resolution falls through each level in turn. |
| `$${VAR}` | Escaped — produces the literal text `${VAR}`, with no lookup. |
| `$VAR` | Not a placeholder — left as-is. |
| `$` anywhere else | Left as-is (`$100` and `Pa$$w0rd!` pass through unchanged). |

### Set-but-empty is a value, not an absence

A default applies only when the variable is **absent**. If `VAR` is set to an empty string, 
that empty string is used:

```rust
// VAR="" (set, empty)
config.str("app/value") // -> ""  — not "fallback"
```

This differs from shell `${VAR:-default}`, which falls back when the variable is unset *or* 
empty. Use `${VAR:-}` when you want "empty if missing" explicitly.

### Escaping

Only `$${` is an escape sequence, producing a literal `${`. Every other `$` is passed through 
untouched, so passwords and shell snippets survive without modification:

```yaml
db:
  password: Pa$$w0rd!               # unchanged
  template: $${HOME}/data           # -> "${HOME}/data", no lookup
  price: $100                       # unchanged
```

### Resolution timing

Environment variables are resolved at load time and re-resolved on every `reload()` call. 
This means changes to environment variables are picked up when the config is reloaded.

A variable's *value* is never rescanned, so a secret that happens to contain `${...}` is 
used verbatim rather than being expanded again.

### Error handling

If a placeholder references an unset variable and no default is provided, loading returns 
a `ConfigError::FormatError`. These also return errors:

| Input | Reason |
| ----- | ------ |
| `${VAR` | Unclosed placeholder |
| `${VAR:-${X}` | Unclosed — the inner placeholder closes, the outer one does not |
| `${:-default}` | Empty variable name |
| `${${PREFIX}_HOST}` | Nesting is supported in defaults, not in the variable name |
| `${A:-${A:-${A:-…}}}` | Defaults nested more than 32 levels deep |

Defaults may nest, but only to a fixed depth of 32 — far beyond the two or three
fallback levels any real config uses. A deeper chain is a `FormatError` rather than an
unbounded recursion that would abort the process.

The one shape that cannot be expressed is an unbalanced `}` inside a default — `${VAR:-a}b}` 
ends the placeholder at the first `}`, giving the default `a` followed by the literal `b}`.

## Auto-Creating Config Files

Use `load_or_create` to handle first-run scenarios where no config file exists yet.
If the file is present its content is used as-is; if not, the provided default YAML
string is written to disk and returned as the active config. Either way the app gets
a fully usable config.

```rust
use trail_config::Config;

const DEFAULTS: &str = r#"
app:
  port: 8080
  debug: false
database:
  host: localhost
  port: 5432
"#;

let config = Config::load_or_create("config.yaml", "/", None, DEFAULTS)?;
```

On first run `config.yaml` is created with the contents of `DEFAULTS`. On subsequent
runs the file is loaded normally and `DEFAULTS` is ignored — so users can edit the
file freely without their changes being overwritten.

The defaults string must be in the same format as the file: YAML by default, or
JSON/TOML when the filename has a matching extension and the corresponding feature
is enabled. The created config records its filename, so `reload()` works after a
first run.

Defaults that do not parse in that format are rejected **before** anything is written,
so a failed first run leaves no file behind and the next run retries the creation:

```rust
// YAML-shaped defaults under a .toml filename
let result = Config::load_or_create("config.toml", "/", None, "app:\n  port: 8080\n");
assert!(result.is_err()); // TomlError — and config.toml was not created

// So correcting the defaults is enough; there is no broken file to clean up first
let config = Config::load_or_create("config.toml", "/", None, "[app]\nport = 8080\n")?;
```

The file is also created **exclusively**. If a second process wins the race to create it —
the first-run scenario this method exists for — `load_or_create` loads that file rather
than overwriting it with its own defaults.

Only the file itself is created — **parent directories are not**. Writing to
`config/app.yaml` when `config/` does not exist returns an `IoError` rather than
creating the directory, so a mistyped path cannot leave a junk directory tree behind.
Call `std::fs::create_dir_all` yourself first if the directory may be missing.

The defaults string is written as-is, preserving formatting and any comments you include:

```rust
const DEFAULTS: &str = r#"
# Application settings
app:
  port: 8080       # HTTP port
  debug: false     # Set to true for verbose logging

# Database connection
database:
  host: localhost
  port: 5432
"#;
```

## Real-World Examples

### Web server configuration

```rust
use trail_config::Config;

let config = Config::load_required("server.yaml", "/", None)?;

let host = config.str("server/host");
let port = config.get_int_strict("server/port")?;
let ssl = config.get_bool("server/ssl").unwrap_or(false);
let workers = config.get_int("server/workers").unwrap_or(4);

println!("Starting server on {}:{} (workers: {})", host, port, workers);
```

### Environment-specific configuration

```rust
use trail_config::Config;
use std::env;

let env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

let config = Config::load_required("config.yaml", "/", None)?
    .merge_required("config.{env}.yaml", Some(&env))?
    .merge_optional("config.local.yaml", None)?;

let db_url = config.str_strict("database/url")?;
let log_level = config.str("logging/level");

println!("Using {} environment", env);
```

### Database connection pooling

Using `get_as_strict` to deserialize the entire `db` section into a struct at once:

```rust
use serde::Deserialize;
use trail_config::Config;

#[derive(Deserialize)]
struct DbConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    pool_size: usize,
    timeout: f64,
}

let config = Config::default();
let db: DbConfig = config.get_as_strict("db")?;
let pool = create_pool(db)?;
```

```yaml
db:
  host: localhost
  port: 5432
  username: admin
  password: secret
  pool_size: 20
  timeout: 60.0
```

### Feature flags

```rust
use trail_config::Config;

let config = Config::default();

if config.get_bool("features/analytics").unwrap_or(false) {
    init_analytics();
}

if config.get_bool("features/profiling").unwrap_or(false) {
    enable_profiling();
}

let beta_features = config.list("features/beta");
for feature in beta_features {
    println!("Beta feature enabled: {}", feature);
}
```

## Sample Configuration File

```yaml
app:
  name: MyApp
  port: 8080
  timeout: 30.5
  debug: false

database:
  host: localhost
  port: 5432
  name: myapp_db
  username: admin
  password: secret
  pool_size: 10

server:
  bind: 127.0.0.1
  workers: 4
  log_level: info

features:
  analytics: true
  profiling: false
  beta:
    - new_ui
    - advanced_search
```

## Development

`check.ps1` runs the full pre-release check in one command — clippy and tests across every
feature combination, the doctests, and the doc build:

```powershell
.\check.ps1           # clippy + tests × 5 feature combinations, doctests, docs
.\check.ps1 -Msrv     # also `cargo +1.85 check` (needs `rustup toolchain install 1.85`)
.\check.ps1 -Bench    # also the criterion benchmarks
```

The feature combinations matter: `json` and `toml` are additive gates, so code that
compiles with both enabled can still fail to compile with neither.

Tests live in two places. `src/**/tests/` holds unit tests with access to internals;
`tests/` exercises the crate as a downstream consumer sees it, which is the only vantage
point that can catch a type missing from the public exports or a `$crate` path breaking
in the `config!` macro.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
