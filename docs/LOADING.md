# Loading Configuration

[← Documentation index](README.md)

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

## Required config (production)

Use `Config::load_required()` when the configuration file **must** exist:

```rust
use trail_config::Config;

let config = Config::load_required("config.yaml", "/", None)?;
// Errors if file is missing, invalid YAML/JSON/TOML, or permission denied
```

## Optional config

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

## The `{env}` placeholder

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

## Default (shorthand)

Use `Config::default()` when `config.yaml` with `/` separator is acceptable and the file is optional. A missing file yields an empty config; a present-but-**broken** file panics (use `load_optional` to get the error instead):

```rust
let config = Config::default(); // Empty if missing; panics if config.yaml is broken
```

## From a YAML string

Use `Config::load_yaml()` to load configuration directly from a string rather than a file.
This is useful for tests, embedded defaults, or configs received over the network:

```rust
let config = Config::load_yaml("app:\n  port: 8080", "/")?;
```

## From a JSON file or string (requires `json` feature)

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

## Files whose extension does not name their format

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

## From a TOML file or string (requires `toml` feature)

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
a direct dependency, which [Reading values](API_OVERVIEW.md#reading-values) advises against
for its own reasons.

## Byte-order marks

A leading UTF-8 BOM is stripped before parsing, for every format and for strings as well 
as files. This matters most on Windows: PowerShell's `>`, `>>` and `Out-File` write UTF-8 
**with** BOM by default, so a config generated by a setup script carries one. Without the 
strip, `serde_json` rejected those bytes while `yaml_serde` and `toml` accepted them — the 
same file loading or failing depending only on its extension, with an error naming line 1 
column 1 of a file that looks correct in every editor.

Only a leading BOM is removed. U+FEFF elsewhere is a legitimate zero-width no-break space 
and is left in the document.

## Using the `config!` macro

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

