//! The layered setup this crate is built around: a committed base, an environment
//! overlay chosen at run time, and an optional personal file that is never committed.
//!
//! ```text
//! cargo run --example environments
//! APP_ENV=production cargo run --example environments
//! ```

use std::error::Error;

use trail_config::Config;

const BASE: &str = "
database:
  url: postgres://localhost/myapp_dev
  pool_size: 5
logging:
  level: debug
";

const PRODUCTION: &str = "
database:
  url: postgres://db.internal/myapp
  pool_size: 40
logging:
  level: warn
";

const LOCAL: &str = "
logging:
  level: trace
";

fn main() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("config.yaml"), BASE)?;
    std::fs::write(dir.path().join("config.production.yaml"), PRODUCTION)?;
    std::fs::write(dir.path().join("config.local.yaml"), LOCAL)?;
    std::env::set_current_dir(&dir)?;

    let environment = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

    // `config.{env}.yaml` resolves against the environment named here. `merge_optional`
    // for the local file, because it is the one file that is allowed not to exist.
    // `None` on the last two merges reuses the environment the config already carries.
    let config = Config::load_required("config.yaml", "/", Some(&environment))?
        .merge_optional("config.{env}.yaml", None)?
        .merge_optional("config.local.yaml", None)?;

    println!("environment: {environment}");
    println!("database/url:       {}", config.str_strict("database/url")?);
    println!("database/pool_size: {}", config.get_int_strict("database/pool_size")?);

    // The overlays layer: `production` raises the level to `warn`, and the local file
    // then wins with `trace`. Under any other environment there is no overlay to apply
    // and the base `debug` survives — which is why this merge is optional too.
    println!("logging/level:      {}", config.str("logging/level"));

    Ok(())
}
