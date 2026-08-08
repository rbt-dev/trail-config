# Trail Config

A Rust library for reading config files with path-based access, typed deserialization, environment overlays, deep merging, env variable interpolation, and hot reload support.

## Features

- 📖 Path-based access (`config.str("db/host")`) with a customizable separator
- 🏗️ Struct deserialization — map the whole config or any subtree into your own type
- 🔀 Deep merge, for layering environment-specific overlays onto a base file
- 🌍 `{env}` in filenames, so one environment name drives the whole chain
- 🌐 Environment variable interpolation with defaults (`${VAR}`, `${VAR:-default}`)
- 🔄 Hot reload, and a `ConfigHandle` for swapping the config behind shared references
- ✅ Every accessor in a lenient and a strict flavour, over one `ConfigError` type
- 📂 YAML out of the box; JSON and TOML behind optional feature flags

## Installation

```toml
[dependencies]
trail-config = "0.5"
```

That is the whole crate for YAML, which needs no feature flags. JSON and TOML are additive
gates, and either or both can be enabled:

```toml
[dependencies]
trail-config = { version = "0.5", features = ["json", "toml"] }
```

## Quick start

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

Whole subtrees deserialize into your own types, which is the accessor to prefer — it states
the shape your program expects in one place instead of at every call site:

```rust
#[derive(serde::Deserialize)]
struct Db { host: String, port: u16 }

let db: Db = config.get_as_strict("db")?;
```

## Layering

The setup this crate is built around: a committed base, an environment overlay chosen at
run time, and an optional personal file that is never committed.

```rust
let mut config = Config::load_required("config.yaml", "/", Some("prod"))?
    .merge_required("config.{env}.yaml", None)?   // -> config.prod.yaml
    .merge_optional("config.local.yaml", None)?;  // absent is fine

config.reload()?; // re-reads the base and every overlay
```

The environment is named once, on the constructor; `None` on each merge reuses it. Values
in an overlay take precedence, and nested mappings merge recursively, so an overlay states
only what it changes.

## Loading

Four constructors, differing only in what they do about a missing file:

| Constructor | Missing file |
| ----------- | ------------ |
| `Config::load_required(file, sep, env)` | Error |
| `Config::load_optional(file, sep, env)` | Empty config, filename still recorded for `reload` |
| `Config::load_or_create(file, sep, env, defaults)` | Written from the supplied defaults |
| `Config::default()` | Empty config — shorthand for `load_optional("config.yaml", "/", None)`, panics on a broken file |

Each has an `_as` twin taking an explicit `Format`, for files whose extension does not name
their format. `load_yaml`, `load_json` and `load_toml` parse from a string instead of a
file, and the `config!` macro condenses a whole load-and-merge chain into one expression.

See [Loading](docs/LOADING.md) for all of it.

## Documentation

The full guide is in [`docs/`](docs), one file per topic — start at the
[documentation index](docs/README.md). The same material is on
[docs.rs](https://docs.rs/trail-config), rendered against the API.

| | |
| --- | --- |
| [Loading](docs/LOADING.md) | Constructors, `{env}`, YAML/JSON/TOML, the `config!` macro |
| [API overview](docs/API_OVERVIEW.md) | Lenient vs strict, path syntax, metadata |
| [Typed access](docs/TYPED_ACCESS.md) | Scalar conversions and struct deserialization |
| [Merging](docs/MERGING.md) | Deep merge, overlay chains, clearing a value |
| [Environment variables](docs/ENV_INTERPOLATION.md) | `${VAR}`, defaults, escaping, timing |
| [Hot reload](docs/HOT_RELOAD.md) · [Shared config](docs/SHARED_CONFIG.md) | `reload`, and `ConfigHandle` for shared references |
| [Formatting](docs/FORMATTING.md) · [Escaping](docs/ESCAPING.md) | `fmt`, and keys containing the separator |
| [Auto-creating files](docs/AUTO_CREATE.md) | Writing defaults on first run |
| [Error handling](docs/ERROR_HANDLING.md) · [Debugging](docs/DEBUGGING.md) | `ConfigError` variants, and `outline` |

## Examples

Four runnable programs in [`examples/`](examples) — each writes its own config into a
temporary directory, so they run from a fresh checkout with nothing to set up:

```bash
cargo run --example web_server      # lenient and strict accessors side by side
cargo run --example environments    # base + {env} overlay + local overrides
cargo run --example db_pool         # deserializing a subtree into a struct
cargo run --example feature_flags   # booleans and lists, read leniently
```

## Contributing

Working on the crate itself — the check scripts, the feature matrix, why both platforms
have to be run — is covered in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
