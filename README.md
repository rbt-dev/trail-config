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
- 🧵 `ConfigHandle` for swapping the config at runtime behind shared references (a plain
  `Config` is already `Send + Sync`)
- ⚡ `config!` macro for concise loading and merging
- 📂 JSON and TOML support via optional feature flags

## Installation

```toml
[dependencies]
trail-config = "0.5"
```

That is the whole crate for YAML, which needs no feature flags. JSON and TOML are additive
gates — see [From a JSON file or string](#from-a-json-file-or-string-requires-json-feature)
and [From a TOML file or string](#from-a-toml-file-or-string-requires-toml-feature) — and
either or both can be enabled:

```toml
[dependencies]
trail-config = { version = "0.5", features = ["json", "toml"] }
```

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

Trail Config's constructors fall into three groups. The four below load a **file**, and are
the ones to reach for first:

| Constructor | File required? | Use case |
| ----------- | -------------- | -------- |
| `Config::load_required(filename, sep, env)` | Yes — errors if missing | Production: config must exist |
| `Config::load_optional(filename, sep, env)` | No — returns empty config if missing | Optional or environment-specific files |
| `Config::load_or_create(filename, sep, env, defaults)` | No — creates from defaults if missing | First-run config generation |
| `Config::default()` | No | Shorthand for `load_optional("config.yaml", "/", None)`. Empty if missing, **panics if broken** |

The other two groups are variations on those. Each of the three named file constructors has
an `_as` twin taking an explicit `Format`, for
[files whose extension does not name their format](#files-whose-extension-does-not-name-their-format);
and `load_yaml`, `load_json` and `load_toml` build a config from a **string** already in
memory rather than from disk.

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
assert_eq!(config.filename(), "config.local.yaml"); // recorded even though absent

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
some files are environment-specific, and the same `env` applies to all of them:

```rust
let config = Config::load_required("config.yaml", "/", Some("prod"))?  // no placeholder
    .merge_required("config.{env}.yaml", Some("prod"))?                // placeholder
    .merge_optional("config.local.yaml", Some("prod"))?;               // no placeholder

assert_eq!(config.environment(), Some("prod"));
```

The reverse is an error, because a `{env}` with nothing to fill it would otherwise be passed to 
the OS verbatim and fail as a missing file called `config.{env}.yaml` — an error that points at 
the wrong problem.

**A merge with `env: None` reuses the environment the config already carries**, so it only 
has to be named once. The table above describes the *constructors*, which have no config to 
fall back to; for `merge_required` and `merge_optional`, `None` means "the one this config 
already has" rather than "none at all":

```rust
let config = Config::load_required("config.{env}.yaml", "/", Some("prod"))?
    .merge_required("overrides.{env}.yaml", None)?   // resolves to overrides.prod.yaml
    .merge_optional("config.local.yaml", None)?;
```

Pass `Some` to a merge only to give one overlay a *different* environment than the base, or 
when the base carries none of its own. An explicit argument always wins, and the config's own 
environment is not changed by it. Where neither the argument nor the config supplies one, a 
`{env}` template is still a `FormatError` — the fallback fills a gap, it does not invent one.

`reload_from()` also accepts `{env}`, resolved against the environment the config already 
carries; it has no `env` argument because it preserves the existing one.

**A merge records the environment it is given, if the config has none yet.** So the common 
shape — an environment-agnostic base with an environment-specific overlay — leaves the 
config carrying it, and `reload_from()` can resolve a `{env}` template afterwards:

```rust
let mut config = Config::load_required("config.yaml", "/", None)?   // no env
    .merge_required("config.{env}.yaml", Some("prod"))?;            // env supplied here

assert_eq!(config.environment(), Some("prod"));
config.reload_from("other.{env}.yaml")?;  // resolves against "prod"
```

An environment already on the config is never replaced by a later merge: it was chosen by 
the constructor, and letting an overlay reassign it would silently change what a subsequent 
`reload_from()` resolves. A merge fills the gap, it does not overwrite.

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

### Files whose extension does not name their format

`load_required` picks the parser from the extension, which covers `.json` and `.toml`
already. For the other case — a JSON document in `settings.conf`, or a file with no
extension at all — the format is a parameter rather than a different constructor. Each of
the three file constructors has an `_as` twin that takes a `Format`:

| Constructor | `_as` twin |
| ----------- | ---------- |
| `load_required(file, sep, env)` | `load_required_as(file, sep, env, format)` |
| `load_optional(file, sep, env)` | `load_optional_as(file, sep, env, format)` |
| `load_or_create(file, sep, env, defaults)` | `load_or_create_as(file, sep, env, format, defaults)` |

```rust
use trail_config::{Config, Format};

// Read as JSON regardless of what the file is called
let mut config = Config::load_required_as("settings.conf", "/", None, Format::Json)?;

// ...and the choice sticks: this re-reads it as JSON, not YAML
config.reload()?;
```

Each twin keeps its base method's behaviour exactly, adding only the pinned format.
`load_optional_as` still treats an absent file as an empty config — and still records the
filename, so the format is already pinned when a later `reload()` finds the file:

```rust
use trail_config::{Config, Format};

// Absent is fine; the config comes back empty, still knowing what to read and how
let mut config = Config::load_optional_as("settings.conf", "/", None, Format::Toml)?;
assert_eq!(config.filename(), "settings.conf");

// IoError (NotFound) while the file is still missing; once it appears it is
// parsed as TOML, not as YAML
config.reload()?;
```

`Format` is `#[non_exhaustive]`, so matching on one needs a `_ => ...` arm; `Format::Json`
and `Format::Toml` exist only with their features enabled.

`load_or_create_as` uses the format for both halves of its job — the defaults are validated
against the same parser that will read them back, so they must be written in it:

```rust
use trail_config::{Config, Format};

const DEFAULTS: &str = r#"{"app": {"port": 8080}}"#;

// Creates settings.conf holding DEFAULTS on first run, reads it as JSON on every run
let config = Config::load_or_create_as("settings.conf", "/", None, Format::Json, DEFAULTS)?;
assert_eq!(config.get_int("app/port"), Some(8080));
```

That is the one place deriving the format from the extension would be outright wrong rather
than merely redundant: YAML-shaped defaults under a `.conf` name pinned to JSON would pass a
YAML check (YAML is a superset of JSON) and then be written to a file the very next read
parses as JSON.

The format is recorded on the config, so `reload` and `reload_from` use the same parser.
Overlays are unaffected and still pick their own parser by their own extension, so a JSON
base can still take a YAML overlay.

Because the choice is preserved, `reload_from` onto a file of a *different* format fails
with a parse error rather than switching — construct a new `Config` to change format. That
is deliberate: YAML is a superset of JSON, so a pin that silently lapsed would usually
*succeed* while applying the wrong rules.

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

**Datetimes are read as strings.** TOML has a date-time type and the value model this
crate reads through does not, so a datetime is surfaced as the text the file contained —
RFC 3339 for an offset date-time, and TOML's own forms for the local date, time and
date-time variants. It is a scalar like any other: readable with `str`, listed by
`outline`, and deserializable into a `String` or into `chrono`/`time`/`jiff`'s date types,
which all parse RFC 3339.

```rust
let config = Config::load_toml("[window]\nstarts = 2024-01-01T00:00:00Z", "/")?;
assert_eq!(config.str("window/starts"), "2024-01-01T00:00:00Z");
```

The one type it will *not* deserialize into is `toml::value::Datetime`, which recognises
only the private marker its own crate serializes. Naming that type means adding `toml` as
a direct dependency, which "Reading values" below advises against for its own reasons.

### Byte-order marks

A leading UTF-8 BOM is stripped before parsing, for every format and for strings as well 
as files. This matters most on Windows: PowerShell's `>`, `>>` and `Out-File` write UTF-8 
**with** BOM by default, so a config generated by a setup script carries one. Without the 
strip, `serde_json` rejected those bytes while `yaml_serde` and `toml` accepted them — the 
same file loading or failing depending only on its extension, with an error naming line 1 
column 1 of a file that looks correct in every editor.

Only a leading BOM is removed. U+FEFF elsewhere is a legitimate zero-width no-break space 
and is left in the document.

### Using the `config!` macro

The `config!` macro provides a concise syntax for loading and merging configs. There are two 
spellings — positional, and a block that labels the filename — and both take the same four 
optional settings:

| Option | Meaning | Default |
| ------ | ------- | ------- |
| `sep:` | Path separator | `"/"` |
| `env:` | Environment name, for `{env}` in any filename | `None` |
| `merge:` | Required overlays, applied in order | none |
| `merge_optional:` | Optional overlays, applied after the required ones | none |

```rust
use trail_config::config;

// Minimal
let config = config!("config.yaml")?;

// Any subset of the options
let config = config!("config.yaml", sep: "::")?;
let config = config!("config.{env}.yaml", env: "prod")?;
let config = config!("config.yaml", merge: ["config.prod.yaml"])?;

// Combined
let config = config!("config.{env}.yaml", sep: "::", env: "prod", merge: ["over.{env}.yaml"])?;

// The same options under the block spelling
let config = config! {
    file: "config.yaml",
    sep: "/",
    env: "prod",
    merge: ["config.{env}.yaml"],
    merge_optional: ["config.local.yaml"],
}?;
```

**Options must appear in the order given in the table.** `config!("f.yaml", env: "prod", 
sep: "::")` does not compile — it is a "no rules expected this token" error pointing at 
`sep`. Everything else composes: any subset, in that order, in either spelling, with or 
without a trailing comma.

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

**Prefer typed access where you can.** `get_as` and `deserialize` are generic over your own
types, so the value model never appears in your code and the shape your program expects is
stated once instead of at every call site — see [Typed Access](#typed-access) and
[Struct Deserialization](#struct-deserialization). Reach for `get` when you genuinely want
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

### Listing the paths a config contains

`outline()` answers "why is this path not resolving" by printing the paths that **do** 
resolve, one per line, with values replaced by their types:

```rust
let config = Config::load_required("config.yaml", "/", None)?;
println!("{}", config.outline());
```

```text
app/name: <string>
app/port: <number>
db/redis/server: <string>
features: <2 items>
```

Each line is spelled exactly as an accessor takes it — the config's own separator, with 
keys containing a separator or a backslash escaped — so a line can be pasted straight into 
`str()` or `get()`. Keys appear in document order.

Two kinds of key cannot be written as a path at all: an **empty** key, since every path 
segment must be non-empty, and a **non-string** key (`1:`, `true:`), since a segment is 
matched as a string. YAML allows both. They are still listed — a key you cannot reach is 
exactly what you want to see when a lookup is failing — but marked, so no line ever claims 
a path that does not resolve:

```text
app/port: <number>
"": <string>                   # not addressable
retries/1: <string>            # not addressable
```

The marker covers the whole line, so anything nested under such a key carries it too. 
Every line *without* one resolves as written.

Values are never printed, only their types, which is what makes the output safe to log or 
paste into an issue: `${DB_PASSWORD}` is already interpolated by the time a `Config` 
exists. If you do want the whole document, deserialize it into a `Value` or `Mapping` and 
serialize that yourself — an explicit act at the call site.

## Error Handling

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

### Matching on `ConfigError`

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

### Handling load errors

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

### Keys that have no path

Path segments are matched as **strings**, so a key that is not a string cannot be reached
by any accessor. YAML permits several — `1:`, `true:`, `~:`, even a sequence as a key:

```yaml
retries:
  1: fast        # an integer key
  2: slow
```

`config.str("retries/1")` looks up the *string* key `"1"`, which is a different key from
the integer `1`, so it returns `""`. There is no escape that changes this, and deliberately
so: one document can hold both `1:` and `"1":`, and any rule that made `retries/1` reach
the integer would make the string permanently unreachable instead. `outline()` lists such
keys and marks them `# not addressable`, so they are visible rather than silently missing.

The subtree containing them is still fully readable — deserialize it and the keys come back
typed:

```rust
use std::collections::BTreeMap;

let retries: BTreeMap<i64, String> = config.get_as("retries").unwrap();
assert_eq!(retries[&1], "fast");
```

The same applies to `true:` / `false:` (`BTreeMap<bool, _>`) and to the raw document via
`get("retries")`.

## Thread-Safe Shared Config

`Config` is `Send + Sync`, so sharing one across threads needs nothing from this crate:

```rust
use trail_config::Config;
use std::{sync::Arc, thread};

let config = Arc::new(Config::load_required("config.yaml", "/", None)?);

for _ in 0..8 {
    let config = Arc::clone(&config);
    thread::spawn(move || println!("{}", config.str("app/name")));
}
```

What an `Arc<Config>` cannot do is **replace** the document behind those shared references — 
`reload()` takes `&mut self`, and an `Arc` hands out `&`. That is what `ConfigHandle` adds: 
interior mutability, so every holder sees the new document after a reload. It wraps `Config` in 
an `Arc<RwLock<Arc<Config>>>` — cloning the handle is cheap, and all clones refer to the same 
underlying config.

So the choice is about *reloading*, not about thread-safety: use an `Arc<Config>` when the 
config is read-only for the life of the process, and a `ConfigHandle` when it changes at 
runtime.

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

// Or switch to a different file, clearing the overlay chain
handle.reload_from("other_config.yaml")?;
```

Neither reads nor reloads hold a lock for long. `read()` locks only long enough to clone an
`Arc` and returns an immutable **snapshot**; `reload()` copies the source list (base filename
plus the overlay chain), does all file reads and parsing with **no lock held**, and takes the
write lock only for a pointer swap. So readers are never blocked on disk I/O, and holding a
snapshot never blocks a reload. If the reload fails, no swap happens and the existing config
is left unchanged.

Reloads are serialized against each other by a separate lock that readers never touch, so a
second concurrent reload waits for the first rather than reading the files alongside it.
Without that, two overlapping reloads would each build a config off to the side and the
*slower* one would swap last — leaving the handle serving a superseded document indefinitely
even though both calls returned `Ok`. Two call sites that can reload independently is all it
takes: a debounced file watcher that fires twice on one write, or a watcher plus a SIGHUP
handler. The wait is paid only by reloads.

`ConfigHandle` mirrors the complete **lenient** accessor surface of `Config` — `get`, `str`,
`list`, `contains`, `get_int`, `get_float`, `get_bool`, `get_as`, `deserialize` and `fmt` — each
a shorthand for the same call on a snapshot. The `*_strict` variants and the metadata accessors
(`filename`, `environment`) are reached through `read()`, which gets you every `Config`
method taking `&self`.

The methods that don't take `&self` can't be reached that way, since a snapshot only ever
derefs to `&Config`:

| `Config` method | Reaching it from a handle |
| --------------- | ------------------------- |
| `reload` (`&mut self`) | Mirrored as `handle.reload()` |
| `reload_from` (`&mut self`) | Mirrored as `handle.reload_from(file)` |
| `merge_required` / `merge_optional` (consume `self`) | Not offered — layer the files, *then* wrap the result in a handle |
| `merge_required_in_place` / `merge_optional_in_place` (`&mut self`) | Not offered either — see below |

So a handle is not bound to its file for life, but its overlay chain is fixed at
construction: `reload` re-applies it, `reload_from` clears it.

The [in-place merges](#chaining-or-in-place) have a signature a handle *could* mirror, since
`&mut self` is no obstacle to interior mutability — they are left off for a reason about
layering rather than about signatures. A handle re-reads the sources its config was built
from; it does not acquire new ones behind existing snapshots. Layer the files first, then
wrap the result.

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

Document order holds for all three formats, including a chain that mixes them. Both 
`serde_json` and `toml` store objects in a `BTreeMap` by default, which would sort every 
`.json` and `.toml` config's keys alphabetically before this crate ever saw the file; 
neither does here.

The overlay filenames are recorded so that `reload()` can re-read and re-apply them in 
order — required overlays that are missing on reload return an error, optional overlays that 
are missing are silently skipped.

### Chaining or in place

Each merge comes in two forms, differing only in signature:

| Form | Signature | On failure |
| ---- | --------- | ---------- |
| `merge_required(file, env)` / `merge_optional(file, env)` | consume `self`, return `Result<Config, _>` | the base config is gone — it was moved into the call |
| `merge_required_in_place(file, env)` / `merge_optional_in_place(file, env)` | `&mut self`, return `Result<(), _>` | the receiver is untouched — filename, document and overlay chain all as they were |

Overlay rules, recorded filenames, recorded environments and errors are identical; the
chaining forms are defined in terms of the in-place ones. Use the chaining form to build a
config from a known set of files, and the in-place form when a merge might fail and the base
is worth keeping:

```rust
use trail_config::Config;

let mut config = Config::load_required("config.yaml", "/", None)?;

// Absent is fine and silent; unreadable is reported and the base survives it
if let Err(e) = config.merge_optional_in_place("config.local.yaml", None) {
    eprintln!("config.local.yaml is unusable, continuing without it: {e}");
}

let port = config.get_int("app/port"); // the base's value, whichever way that went
```

That matters most for `merge_optional`, whose whole purpose is making an overlay
survivable. A *missing* optional file is skipped, but a present-but-unparseable one is an
error — and with the chaining form that error arrived after the base had already been moved
into the call, so "use `config.local.yaml` if it is present **and readable**, otherwise carry
on" could not be written at all.

The guarantee is mechanical rather than careful bookkeeping: the overlay is resolved, read,
parsed and interpolated into a local before anything on `self` is touched, so every failure
returns before the first mutation. It is the same guarantee `reload` and `reload_from`
already make.

### Tagged values

A YAML `!Tag` — how serde spells an enum variant — is part of a value's *shape*, not a
value inside it. Two nodes under the same tag merge like the mappings they usually are:

```yaml
# base.yaml            # overlay.yaml         # result
db: !Postgres          db: !Postgres          db: !Postgres
  host: keep-me          port: 6543             host: keep-me
  port: 5432                                    port: 6543
```

A **differing** tag replaces instead, as does an untagged overlay onto a tagged base.
Changing or dropping the tag changes which variant the document describes, so merging the
two sets of fields would produce a document belonging to neither — and an overlay that
quietly dropped the tag would leave a document that no longer deserializes into the enum
the base named.

Everywhere else a tag is transparent: `db/host` resolves whether or not `db` is tagged,
`str` on a tagged scalar returns the scalar, and `outline` lists a tagged mapping's keys.
Only `get` and `get_as` still see the tag, because selecting the variant is what it is for.

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

// Named once at the base; the overlay's {env} resolves against it
let mut config = Config::load_required("config.yaml", "/", Some(&env))?
    .merge_required("config.{env}.yaml", None)?
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

### Set is not absent

A default applies only when the variable is **absent**. There are two ways a variable can 
be set and still not give you a usable string, and neither falls back.

**Set but empty.** The empty string is used:

```rust
// VAR="" (set, empty)
config.str("app/value") // -> ""  — not "fallback"
```

This differs from shell `${VAR:-default}`, which falls back when the variable is unset *or* 
empty. Use `${VAR:-}` when you want "empty if missing" explicitly.

**Set but not valid Unicode.** A `FormatError` naming that as the cause — the default is 
not applied:

```text
Environment variable 'DB_HOST' is set but is not valid Unicode ("a\u{d800}")
  — the default, if any, is not applied because the variable is set
```

Applying the default here would be the worse outcome: the deployment would run on a 
fallback value while you believed your setting had taken effect. Reporting it as "not set" 
would be almost as bad, sending you to verify an export that is already correct.

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

### Scope: values, never keys

Interpolation applies to string **values** only. A `${VAR}` written as a mapping key stays 
the literal text `${VAR}` and is addressed by that text:

```yaml
${DB_HOST}: value      # a key literally named "${DB_HOST}" — no lookup, no error
host: ${DB_HOST}       # interpolated
```

This is deliberate. Interpolating keys would make the set of valid config *paths* depend 
on the environment, so a path that resolves on one machine would silently miss on another, 
and an unset variable would become a missing key rather than an error.

A YAML tag is exempt for the same reason — it selects a variant, so interpolating it would 
make the document's shape depend on the environment. Values *underneath* a tag are 
interpolated normally, including the unset-variable error:

```yaml
db: !Postgres
  password: ${DB_PASSWORD}   # interpolated; still an error if DB_PASSWORD is unset
```

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
| `${VAR}` / `${VAR:-x}` where `VAR` is set to non-Unicode bytes | Set is not absent — the default does not apply |

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

Creating the file and filling it are two separate syscalls, so the winner leaves a
zero-length file visible for a moment, and a loser arriving in that gap would read nothing
and return an **empty** config — no error, defaults discarded, every accessor answering
`""` / `None` / `[]`. To close that, a config that reads as empty *from a zero-length file*
is re-read for up to 200 ms before being accepted:

| File on disk | `load_or_create` |
| ------------ | ---------------- |
| Has content | Loaded at once — the wait never applies |
| Zero-length, filled within 200 ms | Loads what the winner wrote |
| Zero-length for the whole 200 ms | Returned as an empty config; the file is not overwritten |
| Only comments (not zero-length) | Loaded at once as an empty document |
| `defaults` is `""` | Loaded at once — there is nothing better to wait for |

A deliberately empty config file is therefore still honoured, at the cost of that one wait
at startup. A file still unparseable after 200 ms is returned as a parse error: by then it
is broken rather than half-written.

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

This section describes working on the crate itself, from a checkout of the
[repository](https://github.com/rbt-dev/trail-config). The scripts it names are developer
tooling and are deliberately excluded from the published package, so they are not in the
crate you get from crates.io — only in the repository.

One command runs the full pre-release check — clippy and tests across every feature
combination, the doctests, the doc build and the package contents. There are two copies of
it, one per platform, doing the same steps in the same order:

```powershell
.\check.ps1           # clippy + tests × 5 feature combinations, doctests, docs, package
.\check.ps1 -Msrv     # also `cargo +1.85 check` (needs `rustup toolchain install 1.85`)
.\check.ps1 -Bench    # also the criterion benchmarks
.\check.ps1 -Docsrs   # also builds docs as docs.rs does (needs `rustup toolchain install nightly`)
```

```bash
./check.sh            # the same, under Linux — run it in WSL
./check.sh --msrv
./check.sh --bench
./check.sh --docsrs
```

**Run both.** This project has no CI, so a check script is the gate — but a script only
ever runs on the machine you are sitting at, and one machine is one platform. This crate
reads files by path, derives format from extensions and creates files exclusively, so the
platform axis is where the untested surface is. Between the two: Windows supplies a
case-insensitive filesystem, Linux supplies the Unix error kinds, a case-sensitive
filesystem and Unix path handling. macOS is essentially Linux plus a case-insensitive
filesystem, so it adds nothing on top of those two for this crate.

WSL is enough for the Linux half even with the checkout on a Windows drive: every
filesystem test goes through `tempfile::tempdir()`, which resolves to `/tmp` — inside WSL2
that is the ext4 VHD, not the mounted drive. `check.sh` builds into `target-linux/` rather
than `target/`, because cargo namespaces the build directory by profile and not by host
triple, so sharing it would make each toolchain invalidate the other's fingerprints and
rebuild everything on every switch.

`-Docsrs` / `--docsrs` builds with nightly and `--cfg docsrs`, which is the only
configuration where the `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]` labels on the
feature-gated items compile. Without it a mistake in one of those is invisible until the
crate is published and the rendered page is wrong.

The feature combinations matter: `json` and `toml` are additive gates, so code that
compiles with both enabled can still fail to compile with neither.

Tests live in two places. `src/**/tests/` holds unit tests with access to internals;
`tests/` exercises the crate as a downstream consumer sees it, which is the only vantage
point that can catch a type missing from the public exports or a `$crate` path breaking
in the `config!` macro.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
