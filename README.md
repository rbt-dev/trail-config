# Trail Config

A Rust library for reading YAML config files with path-based access, typed and struct deserialization, environment-specific overlays, deep merging, and hot reload support. Uses [yaml_serde](https://github.com/yaml/yaml-serde) for YAML parsing.

## Features

- 📖 Simple path-based config value access
- 🔧 Customizable path separators (`/`, `::`, etc.)
- 🌍 Environment-specific config files
- 📝 String formatting and interpolation
- ✅ Comprehensive error handling with custom `ConfigError` type
- 📋 Type conversion for strings, numbers, booleans, and sequences
- 🏗️ Struct deserialization — map the entire config or any subtree directly into a typed Rust struct
- 🔐 Escape sequence support for keys containing separators
- 🔄 Hot reload support for detecting configuration changes at runtime
- 🔀 Deep merge support for layering environment-specific config overlays
- 🆕 Auto-create config files from in-code defaults on first run
- 🧵 Thread-safe `ConfigHandle` for sharing config across threads

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
|---|---|---|
| `Config::load_required(filename, sep, env)` | Yes — errors if missing | Production: config must exist |
| `Config::load_optional(filename, sep, env)` | No — returns empty config if missing | Optional or environment-specific files |
| `Config::load_or_create(filename, sep, env, defaults)` | No — creates from defaults if missing | First-run config generation |
| `Config::default()` | No | Shorthand for `load_optional("config.yaml", "/", None)` |

### Required config (production)

Use `Config::load_required()` when the configuration file **must** exist:

```rust
use trail_config::Config;

let config = Config::load_required("config.yaml", "/", None)?;
// Errors if file is missing, invalid YAML, or permission denied
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

### Default (shorthand)

Use `Config::default()` when `config.yaml` with `/` separator is acceptable and the file is optional:

```rust
let config = Config::default(); // Never panics, gracefully handles missing config.yaml
```

### From a YAML string

Use `Config::load_yaml()` to load configuration directly from a string rather than a file. This is useful for tests, embedded defaults, or configs received over the network:

```rust
let config = Config::load_yaml("app:\n  port: 8080", "/")?;
```

## API Overview

Trail Config organizes methods into two styles. Every method has both a lenient and a strict variant:

| Style | Returns | Behaviour on missing path |
|-------|---------|--------------------------|
| Lenient — `get()`, `str()`, `get_int()`, etc. | `Option<T>` or empty default | Returns `None` or `""` / `[]` |
| Strict — `get_strict()`, `str_strict()`, `get_int_strict()`, etc. | `Result<T, ConfigError>` | Returns `Err(PathNotFound)` |

Both styles share the same path syntax and navigate nested YAML using separators (default: `/`).

### Reading values

| Method | Returns | Description |
|--------|---------|-------------|
| `get(path)` | `Option<Value>` | Raw `yaml_serde::Value` |
| `get_strict(path)` | `Result<Value, ConfigError>` | Raw value, errors if missing |
| `str(path)` | `String` | String representation, empty if missing |
| `str_strict(path)` | `Result<String, ConfigError>` | String, errors if missing |
| `list(path)` | `Vec<String>` | Sequence as string vector, empty if missing |
| `list_strict(path)` | `Result<Vec<String>, ConfigError>` | Sequence, errors if missing |
| `contains(path)` | `bool` | Returns `true` if path exists |

### Typed access

| Method | Returns | Description |
|--------|---------|-------------|
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
|--------|---------|-------------|
| `fmt(format, base, keys)` | `String` | Format sibling values into a string, empty on error |
| `fmt_strict(format, base, keys)` | `Result<String, ConfigError>` | Format, errors if any value is missing |

### Metadata and hot reload

| Method | Returns | Description |
|--------|---------|-------------|
| `get_filename()` | `&str` | Filename of the loaded config |
| `environment()` | `Option<&str>` | Environment name used when loading |
| `reload()` | `Result<(), ConfigError>` | Reload from current file |
| `reload_from(filename)` | `Result<(), ConfigError>` | Load from a different file |

## Error Handling

Trail Config uses a custom `ConfigError` enum with four variants:

```rust
use trail_config::ConfigError;

// - IoError(io::Error)       - File I/O errors (missing file, permission denied, etc.)
// - YamlError(String)        - YAML parsing or deserialization errors
// - PathNotFound(String)     - Configuration path not found in document
// - FormatError(String)      - String formatting or configuration errors
```

### Handling load errors

```rust
use trail_config::{Config, ConfigError};

match Config::load_required("config.yaml", "/", None) {
    Ok(config) => {
        let host = config.str("database/host");
        println!("Connecting to {}", host);
    }
    Err(ConfigError::IoError(e)) => {
        eprintln!("Config file error: {}", e);
    }
    Err(ConfigError::YamlError(msg)) => {
        eprintln!("Invalid YAML: {}", msg);
    }
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
    }
    Err(e) => eprintln!("Config error: {}", e),
}

match config.str_strict("database") {
    Ok(value) => println!("Database: {}", value),
    Err(ConfigError::FormatError(msg)) => {
        eprintln!("Not a scalar: {}", msg);
    }
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Not found: {}", path);
    }
    Err(e) => eprintln!("Unexpected error: {}", e),
}

match config.get_int_strict("app/port") {
    Ok(port) => println!("Port: {}", port),
    Err(ConfigError::FormatError(msg)) => {
        eprintln!("Port value has wrong type: {}", msg);
    }
    Err(ConfigError::PathNotFound(path)) => {
        eprintln!("Port config not found: {}", path);
    }
    Err(e) => eprintln!("Unexpected error: {}", e),
}
```

### Input validation

Trail Config validates inputs automatically and returns `FormatError` for invalid configurations:

| Input | Constraint | Error |
|-------|-----------|-------|
| Path separator | Cannot be empty | Returns `FormatError` |
| File paths (`load_required`) | Empty filename explicitly rejected | Returns `IoError` |
| File paths (`load_optional`) | Empty filename passed to OS | Returns `IoError` |
| Paths | Empty paths safely handled | Returns `None` or empty |
| Separators (leading/trailing) | Handled gracefully | No error |
| Filename templates | Must be valid format strings | Returns `FormatError` |

```rust
// Empty separator - error
let result = Config::load_optional("config.yaml", "", None);
assert!(result.is_err()); // FormatError

// load_required rejects empty filename upfront
let result = Config::load_required("", "/", None);
assert!(result.is_err()); // IoError (InvalidInput)

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

Use `deserialize` / `deserialize_strict` to map the **entire config** into a typed Rust struct, or `get_as` / `get_as_strict` to deserialize a subtree at a specific path. Both approaches are more concise than reading fields one by one, and let the compiler verify you haven't missed any required fields.

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

`deserialize_strict` returns `YamlError` if the config can't be deserialized into `T`. `get_as_strict` additionally returns `PathNotFound` if the path doesn't exist.

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

The `fmt()` method takes a format string with `{}` placeholders, a base path to the parent node, and a slice of key names — one per placeholder. It navigates to the base path, then extracts and formats the specified keys in order.

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

### Escape sequences in fmt base path

If a key in the base path contains the separator, escape it with `\`:

```rust
// sections:
//   "db/redis":        <- key contains a literal slash
//     server: 127.0.0.1
//     port: 6379

let connection = config.fmt("{}:{}", r"sections/db\/redis", &["server", "port"]);
// Result: "127.0.0.1:6379"
```

## Escape Sequences

Keys containing the path separator can be accessed using escape sequences.

- `\<sep>` — include a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
- `\\` — include a literal backslash in the key
- Works with any separator: `/`, `::`, `->`, etc.

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

Use `ConfigHandle` to share a `Config` across threads and reload it at runtime without restarting. It wraps `Config` in an `Arc<RwLock<...>>` — cloning the handle is cheap, and all clones refer to the same underlying config.

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

// Full Config access via read guard
let db: DatabaseConfig = handle.read().get_as_strict("database")?;

// Reload from disk — write-locks for the duration, re-applies all overlays
handle.reload()?;
// All clones immediately see the updated values
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

## Thread Safety

`Config` is not `Send + Sync` on its own. Use `ConfigHandle` to share a config across threads — it wraps `Config` in an `Arc<RwLock<...>>` so it can be cloned freely and reloaded at runtime.

```rust
use trail_config::{Config, ConfigHandle};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)?
);

// Cheap to clone — all clones share the same underlying config
let handle2 = handle.clone();

// Convenience methods for common accessors
let port = handle.str("app/port");
let debug = handle.get_bool("app/debug");

// Full access via read guard
let host = handle.read().str_strict("database/host")?;

// Reload from disk (re-applies all overlays), visible to all clones
handle.reload()?;
```

### Background reload loop

```rust
use trail_config::{Config, ConfigHandle};
use std::{thread, time::Duration};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)
        .expect("Failed to load config")
);

// Share with the main application
let app_handle = handle.clone();

// Reload in the background every 5 seconds
thread::spawn(move || {
    loop {
        thread::sleep(Duration::from_secs(5));
        if let Err(e) = handle.reload() {
            eprintln!("Config reload failed: {}", e);
        }
    }
});

// Main thread reads are never blocked except during the brief reload swap
let port = app_handle.get_int("app/port").unwrap_or(8080);
```

## Merging Configs

Use `merge_required` / `merge_optional` to layer configs on top of each other. Values in the overlay take precedence over the base; nested mappings are merged recursively so sibling keys are preserved. Sequences are replaced wholesale. The base config's separator is preserved.

The overlay filenames are recorded so that `reload()` can re-read and re-apply them in order — required overlays that are missing on reload return an error, optional overlays that are missing are silently skipped.

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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
