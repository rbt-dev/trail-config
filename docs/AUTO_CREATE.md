# Auto-Creating Config Files

[← Documentation index](README.md)

Use `load_or_create` to handle first-run scenarios where no config file exists yet.
If the file is present its content is used as-is; if not, the provided default YAML
string is written to disk and loaded as the active config. Loading can fail if reading,
writing, parsing, or interpolation fails.

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
so invalid default syntax leaves no file behind and the next run retries the creation:

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

## Complete or absent

The defaults are not written into the file directly. They go to a temporary file in the
same directory, named `.<name>.<pid>-<n>.tmp`, which is synced to disk and then
hard-linked into place. The link fails rather than replacing a file that already exists,
so creation stays exclusive, and the file appears with its complete contents or not at all.

That also makes a failed first run recoverable. If the write fails or the process dies
partway through, there is no `config.yaml` — at most a stray temporary file — and the next
run creates it from the defaults again. The temporary file is removed as soon as the link
is made. One is left behind if the process dies during creation or the removal fails, and
it is safe to delete. A file watcher on the directory will see it come and go.

## Filesystems without hard links: best-effort retries

On filesystems that do not support hard links — FAT, some network shares — the file is
created and written in place instead. Creating and filling it are then separate
operations, so another reader can observe it empty or partially written. A write failure
can leave such a file behind; later calls load the existing contents instead of replacing
them with defaults. The same applies to a config file written by anything other than
this library.

If the initial read succeeds with a null document, the file is zero-length, and `defaults`
is nonempty, `load_or_create` retries at most ten times with a 20 ms sleep before each
re-read. The requested sleeps total 200 ms; I/O and scheduling can add time. This is a
best-effort heuristic, not a test for write completion. `load_or_create_as` behaves the same way.

| File on disk | `load_or_create` |
| ------------ | ---------------- |
| Initial read parses to a non-null document | Returned immediately, even if it is a valid partial document |
| Initial read fails to parse | Parse error returned immediately |
| Retry observes a non-null document | Returned immediately; the writer may still be writing |
| Remains zero-length through all retries | Returned as an empty config; the file is not overwritten |
| Only comments (not zero-length) | Loaded at once as an empty document |
| `defaults` is `""` | Loaded at once — there is nothing better to wait for |

If retries expire, the most recent config or error is returned. An empty result or a
parse error does not prove the file is finished or permanently broken.

For example, a writer can pause after writing `first: 1\n` and later append `second: 2\n`.
The prefix is already valid YAML, so `load_or_create` can return just `first` while the
writer is paused. That returned snapshot does not acquire `second` when writing finishes;
a subsequent load or reload is needed.

If complete-file reads are required on such a filesystem, or from writers other than
this library, coordinate all participating readers and writers externally. Readers
cannot infer completion from a successful parse or a longer timeout. Application
validation can reject missing required settings, but cannot detect every valid partial
document (for example, one missing only optional settings).

## Directories and defaults

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

