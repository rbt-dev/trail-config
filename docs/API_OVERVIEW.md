# API Overview

[← Documentation index](README.md)

Trail Config organizes methods into two styles. Every method has both a lenient and a strict variant:

| Style | Returns | Behaviour on missing path |
| ----- | ------- | ------------------------- |
| Lenient — `get()`, `str()`, `get_int()`, etc. | `Option<T>` or empty default | Returns `None` or `""` / `[]` |
| Strict — `get_strict()`, `str_strict()`, `get_int_strict()`, etc. | `Result<T, ConfigError>` | Returns `Err(PathNotFound)` |

Both styles share the same path syntax and navigate nested config values using separators (default: `/`).

## Path syntax

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
escape sequence — see [Escape Sequences](ESCAPING.md) — not by doubling it.

**Paths navigate mappings only.** A sequence has no addressable elements: `items/0` is not a 
path into the first element, just a lookup for a key named `0`, and it fails like any other 
missing key. Read the whole sequence with `list(path)`, or deserialize it into a typed field 
with `get_as` / `deserialize`.

## Reading values

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

**Prefer typed access where you can.** `get_as` and `deserialize` are generic over your own
types, so the value model never appears in your code and the shape your program expects is
stated once instead of at every call site — see [Typed Access](TYPED_ACCESS.md) and
[Struct Deserialization](TYPED_ACCESS.md#struct-deserialization). Reach for `get` when you genuinely want
the raw document: inspecting a shape you do not know ahead of time, or walking keys that
are not fixed.

For that case the raw value type is re-exported, so nothing the API hands back needs a
second dependency to name:

```rust
use trail_config::{Config, Value};

let config = Config::load_required("config.yaml", "/", None)?;

match config.get("app/port") {
    Some(Value::Number(n)) => println!("port {}", n),
    Some(other) => eprintln!("app/port is not a number: {:?}", other),
    None => eprintln!("app/port is not set"),
}
```

`Value`, `Mapping`, `Sequence` and `Number` come from
[`yaml_serde`](https://docs.rs/yaml_serde). Use these names rather than depending on that
crate directly — a version that differs from the one this crate resolved produces two
incompatible `Value` types.

The error types work the other way round. `JsonError` and `TomlError` are re-exported
concretely with their features, because `serde_json` and `toml` are both `1.x` and naming
them costs nothing. `ConfigError::YamlError` and `ConfigError::DeserializeError` instead
carry `ValueError`, a type of this crate's own, because `yaml_serde` is `0.x` — where Cargo
treats every minor release as semver-incompatible, so exposing its error type made routine
dependency updates breaking changes here. `ValueError` prints exactly what the underlying
error printed and exposes `location()` for a parse error's line and column.

## Typed access

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

## Formatting

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `fmt(format, base, keys)` | `String` | Format sibling values into a string, empty on error |
| `fmt_strict(format, base, keys)` | `Result<String, ConfigError>` | Format, errors if the template is invalid or any value is missing |

## Metadata and hot reload

| Method | Returns | Description |
| ------ | ------- | ----------- |
| `filename()` | `&str` | Resolved filename of the config; `""` only for configs parsed from a string |
| `environment()` | `Option<&str>` | Environment name used when loading |
| `separator()` | `&str` | Separator this config's paths are written with |
| `reload()` | `Result<(), ConfigError>` | Reload from current file |
| `reload_from(filename)` | `Result<(), ConfigError>` | Load from a different file, discarding the overlay chain |
| `outline()` | `String` | Every path in the document, with values replaced by their types |

The three settings a config carries — `filename()`, `environment()` and `separator()` — are
readable for the same reason. Code handed a `Config` it did not construct cannot otherwise
spell a path for it, since the separator is chosen at construction and every accessor is
defined in terms of it:

```rust
use trail_config::Config;

// A helper that works whatever separator its caller chose
fn db_host(config: &Config) -> String {
    let sep = config.separator();
    config.str(&format!("database{sep}host"))
}

let config = Config::load_required("config.yaml", "::", None)?;
assert_eq!(db_host(&config), config.str("database::host"));
```

`outline()`'s escaping is defined in terms of the same separator, so this is also what lets
its output be interpreted by code that did not pick it.

