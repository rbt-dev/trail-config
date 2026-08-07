//! Reading a server's settings out of a single file.
//!
//! Shows where each accessor style earns its place: lenient where a sensible fallback
//! exists, strict where a missing value should stop startup rather than be guessed at.
//!
//! ```text
//! cargo run --example web_server
//! ```

use std::error::Error;

use trail_config::Config;

const SERVER_YAML: &str = "
server:
  host: 0.0.0.0
  port: 8080
  ssl: true
  workers: 8
";

fn main() -> Result<(), Box<dyn Error>> {
    // Examples have to bring their own config file; a real program would already have one
    // sitting next to it. `set_current_dir` is what lets the loading code below read
    // exactly as it would in your own `main`.
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("server.yaml"), SERVER_YAML)?;
    std::env::set_current_dir(&dir)?;

    let config = Config::load_required("server.yaml", "/", None)?;

    // Lenient: a missing `ssl` or `workers` has an obvious default.
    let host = config.str("server/host");
    let ssl = config.get_bool("server/ssl").unwrap_or(false);
    let workers = config.get_int("server/workers").unwrap_or(4);

    // Strict: there is no sane default port, so a missing one is an error worth returning.
    let port = config.get_int_strict("server/port")?;

    println!("Starting server on {host}:{port} (ssl: {ssl}, workers: {workers})");

    Ok(())
}
