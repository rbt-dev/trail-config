# Real-World Examples

[← Documentation index](README.md)

These four live in [`examples/`](../examples) as programs the compiler checks and
`cargo run` executes, rather than as snippets on a page that can drift out of date. Each
one writes its config into a temporary directory first, so it runs from a fresh checkout
with nothing to set up:

```bash
cargo run --example web_server
cargo run --example environments
cargo run --example db_pool
cargo run --example feature_flags
```

`cargo build --examples` compiles all four, and the check scripts do it as part of a
release run.

## Web server configuration — [`examples/web_server.rs`](../examples/web_server.rs)

Reading a server's settings out of a single file, and where each accessor style earns its
place: lenient for `ssl` and `workers`, which have obvious defaults, strict for `port`,
which has none and should stop startup rather than be guessed at.

```rust
let host = config.str("server/host");
let ssl = config.get_bool("server/ssl").unwrap_or(false);
let port = config.get_int_strict("server/port")?;
```

## Environment-specific configuration — [`examples/environments.rs`](../examples/environments.rs)

The layered setup this crate is built around — a committed base, an environment overlay
chosen at run time, and an optional personal file that is never committed. Run it twice to
watch the overlay change what the same paths resolve to:

```bash
cargo run --example environments
APP_ENV=production cargo run --example environments
```

```rust
let config = Config::load_required("config.yaml", "/", Some(&environment))?
    .merge_optional("config.{env}.yaml", None)?
    .merge_optional("config.local.yaml", None)?;
```

The environment is named once, on the constructor; `None` on each merge reuses it. See
[Merging Configs](MERGING.md) and [the `{env}` placeholder](LOADING.md#the-env-placeholder).

## Database connection pooling — [`examples/db_pool.rs`](../examples/db_pool.rs)

Deserializing a whole subtree into your own type in one call, which is the accessor to
prefer — a struct states the shape the program expects in one place instead of at every
call site.

```rust
#[derive(Deserialize)]
struct DbConfig { host: String, port: u16, pool_size: usize, timeout: f64 }

let db: DbConfig = config.get_as_strict("db")?;
```

See [Typed Access](TYPED_ACCESS.md) for the rest of that surface.

## Feature flags — [`examples/feature_flags.rs`](../examples/feature_flags.rs)

Booleans and lists read leniently, which is the one case where a swallowed missing path is
the correct behaviour: `unwrap_or(false)` gives a flag that was never written the same
meaning as one written `false`, so a new flag can ship before the config files mention it.

```rust
if config.get_bool("features/analytics").unwrap_or(false) { /* ... */ }

for feature in config.list("features/beta") { /* ... */ }
```

## Sample configuration file

A document exercising every scalar type the accessors cover, plus a sequence:

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
