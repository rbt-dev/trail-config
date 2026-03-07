# Trail Config

Simple [Rust](https://www.rust-lang.org/) library to help with reading (and formatting) values from config files.
Supports YAML format (uses [serde_yaml_bw](https://github.com/bourumir-wyngs/serde-yaml-bw) library).

## Features

- 📖 Simple path-based config value access
- 🔧 Customizable path separators (`/`, `::`, etc.)
- 🌍 Environment-specific config files
- 📝 String formatting and interpolation
- ✅ Comprehensive error handling with custom `ConfigError` type
- 📋 Type conversion for strings, numbers, booleans, and sequences
- 🔐 Escape sequence support for keys containing separators
- 🔄 Hot reload support for detecting configuration changes at runtime

## Quick Start

```rust
use trail_config::Config;

// Load config.yaml file
let config = Config::default();

// Get values with lenient API (returns empty/None on missing)
let port = config.str("app/port");          // -> "8080"
let timeout = config.get_int("app/timeout"); // -> Some(30)

// Or use strict API for explicit error handling
match config.str_strict("database/host") {
    Ok(host) => println!("Connecting to {}", host),
    Err(e) => eprintln!("Config error: {}", e),
}
```

## Loading Configuration

Trail Config exposes three constructors with a clear, symmetric design:

| Constructor | File required? | Use case |
|---|---|---|
| `Config::load_required(filename, sep, env)` | Yes — errors if missing | Production: config must exist |
| `Config::load_optional(filename, sep, env)` | No — returns empty config if missing | Optional or environment-specific files |
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

```rust
let config = Config::load_yaml("app:\n  port: 8080", "/")?;
```

## API Overview

Trail Config organizes methods into two API styles:

| Goal | Method Style | Returns |
|------|--------------|---------|
| Lenient access (handles missing gracefully) | `get()`, `str()`, `list()`, etc. | `Option<T>` or empty defaults |
| Strict access (explicit error handling) | `get_strict()`, `str_strict()`, etc. | `Result<T, ConfigError>` |

Both styles share the same path syntax and navigate nested YAML using separators (default: `/`).

## Main Methods (Lenient API)

Lenient methods return `None` or empty values for missing paths or type mismatches.

### Reading Values

- `get(path)` → `Option<Value>` - Get raw `serde_yaml::Value`
- `str(path)` → `String` - Get string representation (empty if missing)
- `list(path)` → `Vec<String>` - Get sequence as vector (empty if missing)
- `contains(path)` → `bool` - Check if path exists

### Type Conversion

- `get_int(path)` → `Option<i64>` - Get integer value
- `get_float(path)` → `Option<f64>` - Get floating-point value
- `get_bool(path)` → `Option<bool>` - Get boolean value

### Formatting

- `fmt(format, path)` → `String` - Format multiple values (empty on any error)

### Configuration Metadata

- `get_filename()` → `&str` - Get loaded config filename
- `environment()` → `Option<&str>` - Get environment name (if used)

### Hot Reload

- `reload()` → `Result<(), ConfigError>` - Reload from current file
- `reload_from(filename)` → `Result<(), ConfigError>` - Load from different file

## Strict Methods (Error Handling API)

Strict methods return `Result<T, ConfigError>` for explicit error handling.

- `get_strict(path)` - Get value, fails with `PathNotFound` if missing
- `str_strict(path)` - Get string, fails with `PathNotFound` if missing
- `list_strict(path)` - Get sequence, fails with `PathNotFound` if missing
- `fmt_strict(format, path)` - Format values, fails with `PathNotFound` or `FormatError`
- `get_int_strict(path)` - Get integer, fails with `PathNotFound` or `FormatError` on type mismatch
- `get_float_strict(path)` - Get float, fails with `PathNotFound` or `FormatError` on type mismatch
- `get_bool_strict(path)` - Get boolean, fails with `PathNotFound` or `FormatError` on type mismatch

## Type Conversion

Convert config values to typed Rust values safely:

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

## String Formatting

Combine multiple config values into a formatted string without separate calls using the `fmt()` method:

### Basic Formatting Example

Instead of loading each value separately:

```rust
let host = config.str("database/host");
let port = config.str("database/port");
let connection = format!("{}:{}", host, port);  // Separate calls
```

Use `fmt()` to combine them in one call:

```rust
let connection = config.fmt("{}:{}", "database/host+port");
```

The `+` in the path tells `fmt()` to combine multiple attributes at the same level.

### How It Works

The `fmt()` method takes:
1. **format** - A format string with `{}` placeholders (one per value)
2. **path** - A path ending with attributes joined by `+` (e.g., `db/redis/server+port`)

It navigates to the parent (`database`), then extracts and formats the specified attributes (`host` and `port`).

### Lenient vs Strict Formatting

Both APIs are available:

```rust
let config = Config::default();

// Lenient - returns empty string if any value is missing
let connection = config.fmt("{}:{}", "database/host+port");

// Strict - returns error if any value is missing
match config.fmt_strict("{}:{}", "database/host+port") {
    Ok(conn) => println!("Connecting to {}", conn),
    Err(e) => eprintln!("Config error: {}", e),
}
```

### Multi-Value Formatting

Format more than two values with additional `+` separators:

```rust
// YAML structure
// databasse:
//   host: localhost
//   port: 5432
//   name: myapp_db
//   username: admin

// Format all four values
let db_url = config.fmt(
    "postgresql://{}@{}:{}/{}",
    "database/username+host+port+name"
);
// Result: "postgresql://admin@localhost:5432/myapp_db"
```

### Escape Sequences in fmt Paths

Escape sequences work in `fmt` paths the same way they do in regular paths. If a key contains the separator, escape it with `\`:

```rust
// YAML structure
// sections:
//   "db/redis":        <- key contains a literal slash
//     server: 127.0.0.1
//     port: 6379

let connection = config.fmt("{}:{}", "sections/db\/redis/server+port");
// Result: "127.0.0.1:6379"
```

## Error Handling

Trail Config uses a custom `ConfigError` enum for precise error handling:

### Error Types

```rust
use trail_config::ConfigError;

// Four error variants:
// - IoError(io::Error)       - File I/O errors (missing file, permission denied, etc.)
// - YamlError(String)        - YAML parsing errors
// - PathNotFound(String)     - Configuration path not found in document
// - FormatError(String)      - String formatting or configuration errors
```

### Basic Error Handling

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

### Strict Method Error Handling

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

// Type conversion with error details
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

## Hot Reload

Detect and apply configuration changes at runtime without restarting:

```rust
let mut config = Config::load_required("config.yaml", "/", None)?;

// Reload from the same file
config.reload()?; // Updates content from disk

// Or switch to a different config file
config.reload_from("other_config.yaml")?;
```

> **Note:** If a reload fails (e.g. the file is temporarily invalid or missing), the existing
> configuration is preserved unchanged. The error is returned, but the config remains valid and usable.

### Server Loop Example

```rust
use trail_config::Config;
use std::thread;
use std::time::Duration;

fn main() {
    let mut config = Config::load_required("config.yaml", "/", None)
        .expect("Failed to load config");
    
    loop {
        // Check for config updates every 5 seconds
        if let Ok(_) = config.reload() {
            println!("✓ Configuration reloaded");
            
            // Apply updated settings
            let timeout = config.get_int("app/timeout").unwrap_or(30);
            let debug = config.get_bool("app/debug").unwrap_or(false);
            
            println!("Timeout: {} seconds, Debug: {}", timeout, debug);
        }
        
        // Main application logic here
        thread::sleep(Duration::from_secs(5));
    }
}
```

## Escape Sequences

Keys containing the path separator can be accessed using escape sequences.

### Syntax

- `\<sep>` - Include a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
- `\\` - Include literal backslash in the key
- Works with any separator: `/`, `::`, `->`, etc.

### Example

Given this YAML with special characters in keys:

```yaml
database:
  "host/port": localhost:5432      # Key contains /
  "user\name": admin\user          # Key contains \
```

Access using escape sequences:

```rust
let config = Config::load_yaml(yaml, "/").unwrap();

// Access key containing separator (/)
let value = config.str("database/host\\/port"); // -> "localhost:5432"

// Access key containing backslash (\)
let value = config.str("database/user\\\\name"); // -> "admin\user"
```

With custom separator:

```rust
let config = Config::load_yaml(yaml, "::").unwrap();

// Path: a::b\::c::d navigates to keys ["a", "b::c", "d"]
let value = config.str("a::b\\::c::d");
```

## Input Validation

Trail Config validates inputs automatically and returns `FormatError` for invalid configurations:

| Input | Constraint | Error |
|-------|-----------|-------|
| Path Separator | Cannot be empty | Returns `FormatError` |
| File Paths (`load_required`) | Empty filename explicitly rejected | Returns `IoError` |
| File Paths (`load_optional`) | Empty filename passed to OS | Returns `IoError` |
| Paths | Empty paths safely handled | Returns `None` or empty |
| Separators (leading/trailing) | Handled gracefully | No error |
| Filename Templates | Must be valid format strings | Returns `FormatError` |

Examples:

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

## Real-World Examples

### Web Server Configuration

```rust
use trail_config::Config;

let config = Config::load_required("server.yaml", "/", None)?;

let host = config.str("server/host");
let port = config.get_int_strict("server/port")?;
let ssl = config.get_bool("server/ssl").unwrap_or(false);
let workers = config.get_int("server/workers").unwrap_or(4);

println!("Starting server on {}:{} (workers: {})", host, port, workers);
```

### Environment-Specific Configuration

```rust
use trail_config::Config;
use std::env;

let env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
let config = Config::load_required(
    "config.{env}.yaml",
    "/",
    Some(&env)
)?;

let db_url = config.str_strict("database/url")?;
let log_level = config.str("logging/level");

println!("Using {} environment", env);
```

### Database Connection Pooling

```rust
use trail_config::Config;

let config = Config::default();

let db_config = DatabaseConfig {
    host: config.str("db/host"),
    port: config.get_int("db/port").unwrap_or(5432) as u16,
    username: config.str("db/username"),
    password: config.str("db/password"),
    pool_size: config.get_int("db/pool_size").unwrap_or(10) as usize,
    timeout: config.get_float("db/timeout").unwrap_or(30.0),
};

let pool = create_pool(db_config)?;
```

Sample YAML:
```yaml
db:
  host: localhost
  port: 5432
  username: admin
  password: secret
  pool_size: 20
  timeout: 60.0
```

### Feature Flags and Feature Detection

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
