//! Deserializing a whole subtree into your own type in one call.
//!
//! This is the accessor to prefer. A struct states the shape the program expects in one
//! place instead of at every call site, and nothing in the signature mentions this
//! crate's value model — so the code does not depend on what that model happens to be.
//!
//! ```text
//! cargo run --example db_pool
//! ```

use std::error::Error;

use serde::Deserialize;
use trail_config::Config;

const CONFIG_YAML: &str = "
db:
  host: localhost
  port: 5432
  username: admin
  password: secret
  pool_size: 20
  timeout: 60.0
";

#[derive(Debug, Deserialize)]
struct DbConfig {
    host: String,
    port: u16,
    username: String,
    #[allow(dead_code)] // Read by a real pool builder; only printed as `***` here.
    password: String,
    pool_size: usize,
    timeout: f64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("config.yaml"), CONFIG_YAML)?;
    std::env::set_current_dir(&dir)?;

    let config = Config::load_required("config.yaml", "/", None)?;

    // One call for the whole section. A wrong type or a missing field is one error here,
    // at startup, rather than six lookups scattered through the pool builder.
    let db: DbConfig = config.get_as_strict("db")?;

    println!(
        "connecting to {}@{}:{} (pool: {}, timeout: {}s, password: {})",
        db.username,
        db.host,
        db.port,
        db.pool_size,
        db.timeout,
        "*".repeat(8),
    );

    Ok(())
}
