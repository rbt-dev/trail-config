# Auto-Creating Config Files

[← Documentation index](README.md)

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

