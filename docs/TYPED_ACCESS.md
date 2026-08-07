# Typed Access

[← Documentation index](README.md)

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

# Struct Deserialization

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
let full: FullConfig = config.deserialize_strict()?;                // Strict — errors on a mismatch
let full: Option<FullConfig> = config.deserialize();                // Lenient — None on a mismatch

// Or deserialize just a subtree
let db: DatabaseConfig = config.get_as_strict("database")?; // Strict — returns a descriptive error on failure
let db: Option<DatabaseConfig> = config.get_as("database"); // Lenient — returns None if path is missing or struct doesn't match
```

The lenient `deserialize` suits a config whose absence is a normal state — an optional
overlay-only file, or a subsystem that is simply off when unconfigured — since it collapses
"missing" and "malformed" into `None`:

```rust
// No telemetry section, or one that does not match: run without it
match config.deserialize::<TelemetryConfig>() {
    Some(telemetry) => start_telemetry(telemetry),
    None => println!("telemetry not configured, skipping"),
}
```

Prefer `deserialize_strict` where the config is required, since it says *which* field was
wrong rather than only that something was.

`deserialize_strict` returns `DeserializeError` if the config can't be deserialized into 
`T`, naming the file and — for `get_as_strict` — the subtree path. `get_as_strict` 
additionally returns `PathNotFound` if the path doesn't exist.

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

