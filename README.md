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
- `PathNotFound(String)` - Path not found in configuration (for future use)
- `FormatError(String)` - String formatting errors (for future use)

## API Reference

### Main Methods (Lenient - return empty/None on missing values)

- `Config::new(filename, separator, env)` - Create config from file with optional environment substitution
- `Config::default()` - Load `config.yaml` with `/` separator
- `Config::load_yaml(yaml_str, separator)` - Parse YAML string directly
- `get(path)` - Get value as `serde_yaml::Value`, returns `None` if not found
- `str(path)` - Get string representation of value, returns empty string if not found
- `list(path)` - Get sequence as `Vec<String>`, returns empty vec if not found
- `fmt(format, path)` - Format multiple values, returns empty string on any error
- `contains(path)` - Check if path exists in config
- `get_filename()` - Get the loaded config filename

### Strict Methods (Return errors for missing values)

For applications requiring explicit error handling:

- `get_strict(path)` - Returns `Result<Value, ConfigError>` - fails with `PathNotFound` if not found
- `str_strict(path)` - Returns `Result<String, ConfigError>` - fails with `PathNotFound` if not found
- `list_strict(path)` - Returns `Result<Vec<String>, ConfigError>` - fails with `PathNotFound` if not found
- `fmt_strict(format, path)` - Returns `Result<String, ConfigError>` - fails with `PathNotFound` or `FormatError`

### Example: Using Strict Methods

```rust
use trail_config::{Config, ConfigError};

let config = Config::default()?;

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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details