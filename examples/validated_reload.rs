//! Explicit scalar conversion and validation before publishing a replacement.
//! Run with `cargo run --example validated_reload`.

use std::{error::Error, fs};
use trail_config::{Config, ConfigError, ConfigHandle};

struct Settings {
    port: u16,
    debug: bool,
}

impl Settings {
    fn read(config: &Config) -> Result<Self, ConfigError> {
        // str_strict accepts scalar strings, numbers, and booleans. Parsing is an
        // application policy: no trimming, and booleans accept only true/false.
        let port = config.str_strict("port")?.parse::<u16>()
            .map_err(|_| ConfigError::FormatError("port must be a u16".into()))?;
        let debug = config.str_strict("debug")?.parse::<bool>()
            .map_err(|_| ConfigError::FormatError("debug must be true or false".into()))?;
        if port == 0 {
            return Err(ConfigError::FormatError("port must be nonzero".into()));
        }
        Ok(Self { port, debug })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("settings.yaml");
    fs::write(&file, "port: '${TRAIL_EXAMPLE_PORT:-8080}'\ndebug: '${TRAIL_EXAMPLE_DEBUG:-false}'\n")?;
    let config = Config::load_required(&file.to_string_lossy(), "/", None)?;
    let handle = ConfigHandle::with_validator(config, |candidate| Settings::read(candidate).map(|_| ()))?;
    let before = handle.read();
    let settings = Settings::read(&before)?;
    println!("Initial settings: port={}, debug={}", settings.port, settings.debug);

    // Syntactically valid YAML can still be invalid application configuration.
    for invalid in ["port: broken\ndebug: false", "port: 0\ndebug: false", "port: 8080\ndebug: yes"] {
        fs::write(&file, invalid)?;
        let error = handle.reload().unwrap_err();
        eprintln!("Reload rejected: {}", error.safe_diagnostic());
        assert!(std::sync::Arc::ptr_eq(&before, &handle.read()));
    }

    fs::write(&file, "port: 9090\ndebug: true")?;
    handle.reload()?;
    let current = Settings::read(&handle.read())?;
    assert_eq!(current.port, 9090);
    assert!(current.debug);
    println!("Accepted replacement: port={}, debug={}", current.port, current.debug);
    Ok(())
}
