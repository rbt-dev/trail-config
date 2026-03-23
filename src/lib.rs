mod error;
mod config;
mod handle;

pub use error::ConfigError;
pub use config::Config;
pub use handle::ConfigHandle;

/// Macro for building a [`Config`] with a concise syntax.
///
/// Loads a config file, optionally sets a separator and environment,
/// and applies required and optional overlays in order.
///
/// # Examples
///
/// ```no_run
/// # use trail_config::config;
/// // Minimal
/// let cfg = config!("config.yaml");
///
/// // With all options
/// let cfg = config! {
///     file: "config.yaml",
///     sep: "::",
///     env: "prod",
///     merge: ["config.{env}.yaml"],
///     merge_optional: ["config.local.yaml"]
/// };
/// ```
#[macro_export]
macro_rules! config {
    // Minimal: config!("file.yaml")
    ($file:expr) => {
        $crate::Config::load_required($file, "/", None)
    };

    // Positional with sep: config!("file.yaml", sep: "::")
    ($file:expr, sep: $sep:expr) => {
        $crate::Config::load_required($file, $sep, None)
    };

    // Positional with env: config!("file.yaml", env: "prod")
    ($file:expr, env: $env:expr) => {
        $crate::Config::load_required($file, "/", Some($env))
    };

    // Positional with merge: config!("file.yaml", merge: ["overlay.yaml"])
    ($file:expr, merge: [$($req:expr),* $(,)?]) => {{
        let _cfg = $crate::Config::load_required($file, "/", None);
        $(
            let _cfg = _cfg.and_then(|c| c.merge_required($req, None));
        )*
        _cfg
    }};

    // Block syntax: config! { file: "...", ... }
    ( file: $file:expr $(, sep: $sep:expr)? $(, env: $env:expr)? $(, merge: [$($req:expr),* $(,)?])? $(, merge_optional: [$($opt:expr),* $(,)?])? $(,)? ) => {{
        let _sep = config!(@sep $($sep)?);
        let _env: Option<&str> = config!(@env $($env)?);

        let _cfg = $crate::Config::load_required($file, _sep, _env);

        $($(
            let _cfg = _cfg.and_then(|c| c.merge_required($req, _env));
        )*)?

        $($(
            let _cfg = _cfg.and_then(|c| c.merge_optional($opt, _env));
        )*)?

        _cfg
    }};

    // Internal helpers
    (@sep) => { "/" };
    (@sep $sep:expr) => { $sep };
    (@env) => { None };
    (@env $env:expr) => { Some($env) };
}