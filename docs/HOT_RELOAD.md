# Hot Reload

[← Documentation index](README.md)

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

## Server loop example

```rust
use trail_config::Config;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    let mut config = match Config::load_required("config.yaml", "/", None)
        .and_then(|config| config.merge_optional("config.local.yaml", None))
    {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Config startup failed: {}", error.safe_diagnostic());
            return ExitCode::FAILURE;
        }
    };

    loop {
        // Check for config updates every 5 seconds
        match config.reload() {
            Ok(()) => {
                println!("✓ Configuration reloaded");

                let timeout = config.get_int("app/timeout").unwrap_or(30);
                let debug = config.get_bool("app/debug").unwrap_or(false);

                println!("Timeout: {} seconds, Debug: {}", timeout, debug);
            }
            Err(error) => eprintln!("Config reload failed: {}", error.safe_diagnostic()),
        }

        // Main application logic here
        thread::sleep(Duration::from_secs(5));
    }
}
```

## Files caught mid-save

A reload triggered by a file watcher can run while an editor or a deployment tool is still
writing. Many save in place: truncate the file, then write the new content. A reload that
lands in between sees an empty file, and an empty document is a valid one.

- **A zero-length file is waited on.** The reload re-reads it for up to about 200 ms in
  case the write completes, then carries on with whatever it finds.
- **A base file that stays empty fails the reload** with an `IoError`, and the current
  config is kept, the same as when the base is missing. A file holding only a comment is
  not zero-length and loads as before.
- **An overlay that stays empty is a no-op**, as it always is, so its overrides revert
  to the base values until the next reload. Because of the wait, an overlay kept empty
  on purpose adds about 200 ms to every reload; delete it instead, or give it a comment.
- **A missing optional overlay is skipped**, which is what an editor that deletes the
  file before writing it again looks like.

None of that can detect a **half-written** file that still parses: a valid prefix of
the real file reloads successfully with the later keys missing. Two things close that
gap:

- **Write watched files atomically.** Write to a temporary file in the same directory,
  then rename it over the original. A rename replaces the file in one step, so a reader
  sees either the old content or the new.

  ```rust
  std::fs::write("config.yaml.tmp", new_content)?;
  std::fs::rename("config.yaml.tmp", "config.yaml")?;
  ```

- **Validate before publishing.** A [`ConfigHandle` validator](SHARED_CONFIG.md#validating-replacements)
  that requires the keys your application cannot run without rejects a truncated
  replacement and keeps serving the old one:

  ```rust
  let handle = ConfigHandle::with_validator(config, |candidate| {
      candidate.get_as_strict::<String>("database/url")?;
      candidate.get_as_strict::<u16>("server/port")?;
      Ok(())
  })?;
  ```

## Application validation

Parsing and interpolation do not validate application types or domain rules. Use
[`ConfigHandle::with_validator`](SHARED_CONFIG.md#validating-replacements) to check
the complete replacement before publication and retain the old snapshot on rejection.
