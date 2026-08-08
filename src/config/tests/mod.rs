mod accessors;
mod bom;
mod debug;
mod env_vars;
mod escape;
mod explicit_format;
mod formatting;
mod internals;
#[cfg(feature = "json")]
mod json;
mod key_order;
mod loading;
mod macros;
mod merge;
mod outline;
mod reload;
mod structs;
mod tagged;
#[cfg(feature = "toml")]
mod toml;

use super::Config;
use crate::ConfigError;

pub(super) const YAML: &str = "
db:
    redis:
        server: 127.0.0.1
        port: 6379
        key_expiry: 3600
    sql:
        driver: SQL Server
        server: 127.0.0.1
        database: my_db
        username: user
        password: Pa$$w0rd!
sources:
    - one
    - two
    - three
app:
    debug: true
    max_retries: 5
    timeout: 2.5
";
