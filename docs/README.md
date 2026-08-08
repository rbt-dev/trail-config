# Trail Config documentation

The full guide, one file per topic. Everything here is also on
[docs.rs](https://docs.rs/trail-config), rendered against the API and with the doctests
run against it — these files are for reading the manual straight through, or on GitHub,
without leaving the repository.

Start with the [README](../README.md) for installation and a first config.

## Reading configuration

| Guide | Covers |
| ----- | ------ |
| [Loading](LOADING.md) | The constructors, the `{env}` placeholder, YAML/JSON/TOML, formats that the extension does not name, the `config!` macro |
| [API overview](API_OVERVIEW.md) | Lenient vs strict accessors, path syntax, reading values, metadata |
| [Typed access](TYPED_ACCESS.md) | Scalar conversions, and deserializing a subtree or a whole document into your own struct |
| [Formatting](FORMATTING.md) | `fmt` — building a string from several config values at once |
| [Escaping](ESCAPING.md) | Keys that contain the separator, and keys that have no path at all |

## Layering and change

| Guide | Covers |
| ----- | ------ |
| [Merging](MERGING.md) | Deep merge, overlay chains, tagged values, clearing a value with `null` |
| [Environment variables](ENV_INTERPOLATION.md) | `${VAR}` and `${VAR:-default}`, escaping, resolution timing, scope |
| [Hot reload](HOT_RELOAD.md) | `reload`, `reload_from`, change detection |
| [Shared config](SHARED_CONFIG.md) | `ConfigHandle` — swapping the document behind shared references |
| [Auto-creating files](AUTO_CREATE.md) | `load_or_create`, writing defaults on first run |

## Operating

| Guide | Covers |
| ----- | ------ |
| [Error handling](ERROR_HANDLING.md) | The `ConfigError` variants, matching on them, what each one means |
| [Debugging](DEBUGGING.md) | `outline` and `Debug`, and why both elide values |
| [Examples](EXAMPLES.md) | Four runnable programs in [`examples/`](../examples) |

Working on the crate itself is covered by [CONTRIBUTING.md](../CONTRIBUTING.md).
