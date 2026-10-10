//! YAML merge keys: shared defaults defined once with an anchor (`&name`) and merged into
//! other mappings with `<<: *name`, the way Rails' `database.yml` or docker-compose files
//! are written.
//!
//! ```text
//! cargo run --example merge_keys
//! ```

use std::error::Error;

use serde::Deserialize;
use trail_config::Config;

const DATABASE: &str = "
# Defined once, under an anchor
defaults: &defaults
  adapter: postgres
  host: localhost
  port: 5432
  pool: 5

logging: &logging
  level: info
  format: json

development:
  <<: *defaults              # everything from `defaults`
  database: myapp_dev

production:
  <<: [*defaults, *logging]  # several sources: the earlier one wins on a clash
  host: db.internal          # explicit keys always win over merged ones
  pool: 40
  database: myapp
";

#[derive(Deserialize)]
struct Database {
    adapter: String,
    host: String,
    port: u16,
    pool: u32,
    database: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::load_yaml(DATABASE, "/")?;

    // Merged keys read like keys written in place.
    for env in ["development", "production"] {
        println!(
            "{env:<12} host={:<12} port={} pool={:<3} database={}",
            config.str(&format!("{env}/host")),
            config.get_int(&format!("{env}/port")).unwrap_or_default(),
            config.get_int(&format!("{env}/pool")).unwrap_or_default(),
            config.str(&format!("{env}/database")),
        );
    }
    println!("production log level: {}", config.str("production/level"));

    // Typed access sees the merged mapping too, so required fields are found.
    let db: Database = config.get_as_strict("production")?;
    println!(
        "\ntyped: {}://{}:{}/{} (pool {})",
        db.adapter, db.host, db.port, db.database, db.pool,
    );

    // No `<<` key is left behind. Merged keys sit where `<<` was, ahead of the
    // explicit keys that follow it.
    println!("\noutline:\n{}", config.outline());
    Ok(())
}
