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


## Application validation

Parsing and interpolation do not validate application types or domain rules. Use
[`ConfigHandle::with_validator`](SHARED_CONFIG.md#validating-replacements) to check
the complete replacement before publication and retain the old snapshot on rejection.
