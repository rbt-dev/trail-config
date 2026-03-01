# Trail Config

Simple [Rust](https://www.rust-lang.org/) library to help with reading (and formatting) values from config files.\
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

## Examples

### Sample *config.yaml* file
```yaml
app:
  port: 1000
db:
  redis:
    server: 127.0.0.1
    port: 6379
    key_expiry: 3600
  sql:
    driver: SQL Server
    server: 127.0.0.1
    database: my_db
    username: user
    password: Pa$$w0rd!
```

### Default configuration
```rust
let config = Config::default(); // loads config.yaml file

let port = config.get("app/port").unwrap(); // returns serde_yaml::value::Value

let port = config.str("app/port");
assert_eq!("1000", port);

let redis = config.get("db/redis"); // returns serde_yaml::value::Value (in this case Mapping)

let redis = config.str("db/redis");
assert_eq!("", redis);

let expiry = config.str("db/redis/key_expiry");
assert_eq!("3600", expiry);

let redis = config.fmt("{}:{}", "db/redis/server+port");
assert_eq!("127.0.0.1:6379", redis);

let conn = config.fmt("Driver={{{}}};Server={};Database={};Uid={};Pwd={};", "db/sql/driver+server+database+username+password");
assert_eq!("Driver={SQL Server};Server=127.0.0.1;Database=my_db;Uid=user;Pwd=Pa$$w0rd!;", conn);
```

### With custom separator
```rust
let config = Config::new("config.yaml", "::", None).unwrap(); 

let port = config.str("app::port");
assert_eq!("1000", port);
```

### With environment variable
```rust
let config = Config::new("config.{env}.yaml", "/", Some("dev")).unwrap(); // loads config.dev.yaml
assert_eq!("dev", config.environment().unwrap());
```

### Checking if a path exists
```rust
let config = Config::default();

if config.contains("db/redis/port") {
    let port = config.str("db/redis/port");
    println!("Port: {}", port);
}
```

### Type Conversion (Int, Float, Bool)
```rust
let config = Config::default();

// Lenient - returns None if not found or type mismatch
let port = config.get_int("app/port");
let timeout = config.get_float("app/timeout");
let debug = config.get_bool("app/debug");

// Strict - returns Result with error details
match config.get_int_strict("db/redis/port") {
    Ok(port) => println!("Connecting to port {}", port),
    Err(trail_config::ConfigError::PathNotFound(path)) => eprintln!("Missing: {}", path),
    Err(trail_config::ConfigError::FormatError(msg)) => eprintln!("Invalid type: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Hot Reload
```rust
let mut config = Config::default();

// Reload config from the same file
config.reload().expect("Failed to reload config");

// Or reload from a different file
config.reload_from("other_config.yaml").expect("Failed to load different config");
```

## Error Handling

Trail Config uses a custom `ConfigError` enum for precise error handling:

```rust
use trail_config::{Config, ConfigError};

match Config::new("config.yaml", "/", None) {
    Ok(config) => {
        let port = config.str("app/port");
        println!("Port: {}", port);
    },
    Err(ConfigError::IoError(e)) => {
        eprintln!("File not found or permission denied: {}", e);
    },
    Err(ConfigError::YamlError(msg)) => {
        eprintln!("Invalid YAML syntax: {}", msg);
    },
    Err(e) => {
        eprintln!("Config error: {}", e);
    }
}
```

### ConfigError Variants

- `IoError(io::Error)` - File I/O errors (file not found, permission denied, etc.)
- `YamlError(String)` - YAML parsing errors
- `PathNotFound(String)` - Configuration path not found in document
- `FormatError(String)` - String formatting or configuration errors (e.g., invalid filename template, empty separator)

## API Reference

### Main Methods (Lenient - return empty/None on missing values)

- `Config::new(filename, separator, env)` - Create config from file with optional environment substitution (returns error if file missing)
- `Config::load_required(filename, separator, env)` - **Production use**: Load config file, returns error if file is missing or invalid
- `Config::default()` - Try to load `config.yaml` with `/` separator, gracefully falls back to empty config if missing
- `Config::load_yaml(yaml_str, separator)` - Parse YAML string directly
- `get(path)` - Get value as `serde_yaml::Value`, returns `None` if not found
- `str(path)` - Get string representation of value, returns empty string if not found
- `list(path)` - Get sequence as `Vec<String>`, returns empty vec if not found
- `fmt(format, path)` - Format multiple values, returns empty string on any error
- `get_int(path)` - Get value as `i64`, returns `None` if not found or type mismatch
- `get_float(path)` - Get value as `f64`, returns `None` if not found or type mismatch
- `get_bool(path)` - Get value as `bool`, returns `None` if not found or type mismatch
- `contains(path)` - Check if path exists in config
- `get_filename()` - Get the loaded config filename
- `reload()` - Reload configuration from the currently loaded file (hot reload)
- `reload_from(filename)` - Reload configuration from a different file

### Strict Methods (Return errors for missing values)

For applications requiring explicit error handling:

- `get_strict(path)` - Returns `Result<Value, ConfigError>` - fails with `PathNotFound` if not found
- `str_strict(path)` - Returns `Result<String, ConfigError>` - fails with `PathNotFound` if not found
- `list_strict(path)` - Returns `Result<Vec<String>, ConfigError>` - fails with `PathNotFound` if not found
- `fmt_strict(format, path)` - Returns `Result<String, ConfigError>` - fails with `PathNotFound` or `FormatError`
- `get_int_strict(path)` - Returns `Result<i64, ConfigError>` - fails with `PathNotFound` or `FormatError` on type mismatch
- `get_float_strict(path)` - Returns `Result<f64, ConfigError>` - fails with `PathNotFound` or `FormatError` on type mismatch
- `get_bool_strict(path)` - Returns `Result<bool, ConfigError>` - fails with `PathNotFound` or `FormatError` on type mismatch

## Loading Configuration

Trail Config provides different methods for different use cases:

### For Production Code
Use `Config::load_required()` when the configuration file **must** exist:
```rust
use trail_config::{Config, ConfigError};

let config = Config::load_required("config.yaml", "/", None)?;
// Will error if file is missing, invalid YAML, or permission denied
```

### For Testing/Optional Configs
Use `Config::default()` or `Config::new()` when missing config is acceptable:
```rust
let config = Config::default(); // Never panics, gracefully handles missing file
```

To prevent unexpected behavior, Trail Config validates inputs:

- **Path Separator**: Cannot be empty. Using an empty separator will return `FormatError`.
- **Empty Paths**: Paths like `""` are safely handled (return `None` or empty values).
- **Leading/Trailing Separators**: Handled gracefully (e.g., `/db/redis/port/` works correctly).
- **Filename Templates**: Must have valid format strings. Invalid templates like `"config_{invalid"` return `FormatError`.

## Escape Sequences

Keys containing the separator character can be accessed using escape sequences:

- `\/` - Escaped separator (includes separator in the key name)
- `\\` - Escaped backslash (includes backslash in the key name)

### Example with Special Characters in Keys

```yaml
database:
  "host/port": localhost:5432
  "user\pass": myuser/mypass
```

```rust
let config = Config::load_yaml(yaml, "/").unwrap();

// Access key containing the separator (/)
let value = config.get("database/host\\/port");
assert_eq!(config.str("database/host\\/port"), "localhost:5432");

// Access key containing a backslash
let value = config.get("database/user\\\\pass");
assert_eq!(config.str("database/user\\\\pass"), "myuser/mypass");
```

### Escape Sequence Rules

- Use `\` before the separator character to include it literally in the key
- Use `\\` to include a literal backslash
- Works with any separator: `/`, `::`, etc.
- Example with `::` separator: `"a::b\\::c::d"` navigates to keys `["a", "b::c", "d"]`

### Example: Default Configuration Behavior

```rust
let config = Config::default();
// Tries to load config.yaml if it exists
// Falls back to empty config if file is missing or invalid YAML
// Never panics
```

### Example: Using Strict Methods

```rust
use trail_config::{Config, ConfigError};

let config = Config::default();

// Strict method - returns error if path doesn't exist
match config.str_strict("database/host") {
    Ok(host) => println!("Connecting to {}", host),
    Err(ConfigError::PathNotFound(path)) => eprintln!("Missing required config: {}", path),
    Err(e) => eprintln!("Config error: {}", e),
}

// Strict formatting with error handling
match config.fmt_strict("{}:{}", "server/host+port") {
    Ok(addr) => println!("Server: {}", addr),
    Err(ConfigError::PathNotFound(path)) => eprintln!("Missing config: {}", path),
    Err(ConfigError::FormatError(msg)) => eprintln!("Format error: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Example: Hot Reload in a Server Loop

```rust
use trail_config::{Config, ConfigError};
use std::thread;
use std::time::Duration;

fn main() {
    let mut config = Config::load_required("config.yaml", "/", None)
        .expect("Failed to load config");
    
    loop {
        // Check for config updates periodically
        if let Ok(_) = config.reload() {
            println!("Configuration reloaded successfully");
            // Re-apply config changes
            let timeout = config.get_int("app/timeout").unwrap_or(30);
            println!("New timeout: {} seconds", timeout);
        }
        
        // Main application logic here
        thread::sleep(Duration::from_secs(5));
    }
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details