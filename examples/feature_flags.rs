//! Booleans and lists read leniently, where an absent key means "off" rather than
//! "misconfigured" — the one case where swallowing a missing path is the correct
//! behaviour and not a bug waiting to happen.
//!
//! ```text
//! cargo run --example feature_flags
//! ```

use std::error::Error;

use trail_config::Config;

const CONFIG_YAML: &str = "
features:
  analytics: true
  profiling: false
  beta:
    - new_ui
    - advanced_search
";

fn main() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::write(dir.path().join("config.yaml"), CONFIG_YAML)?;
    std::env::set_current_dir(&dir)?;

    let config = Config::load_required("config.yaml", "/", None)?;

    // `unwrap_or(false)` gives a flag that was never written the same meaning as one
    // written `false`, so a new flag can ship before the config files mention it.
    if config.get_bool("features/analytics").unwrap_or(false) {
        println!("analytics: on");
    }
    if config.get_bool("features/profiling").unwrap_or(false) {
        println!("profiling: on");
    }
    // Absent, and never written into the file above — the lenient read makes it `false`.
    if config.get_bool("features/tracing").unwrap_or(false) {
        println!("tracing: on");
    }

    // `list` is lenient in the same way: an absent or non-sequence path is an empty Vec.
    for feature in config.list("features/beta") {
        println!("beta feature: {feature}");
    }

    Ok(())
}
