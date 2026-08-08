# String Formatting

[← Documentation index](README.md)

Use `fmt()` to combine multiple sibling config values into a formatted string in a single call:

```rust
// Instead of:
let host = config.str("database/host");
let port = config.str("database/port");
let connection = format!("{}:{}", host, port);

// You can write:
let connection = config.fmt("{}:{}", "database", &["host", "port"]);
```

The `fmt()` method takes a format string, a base path to the parent node, and a slice of key 
names. It navigates to the base path, then substitutes the specified keys into the template.

`base` and each key use the same path syntax as every other accessor — the separator splits 
them, `\` escapes a literal separator, and empty segments are rejected. Keys are paths 
*relative to* `base`, so they are usually a single sibling key but may reach deeper:

```rust
// These are equivalent
config.fmt("{}:{}", "db/redis", &["server", "port"]);
config.fmt("{}:{}", "db",       &["redis/server", "redis/port"]);
```

Each key must resolve to a scalar; one naming a mapping or a sequence is an error from 
`fmt_strict` and an empty string from `fmt`.

## Placeholder syntax

| Syntax | Meaning |
| ------ | ------- |
| `{}` | The next unused key, left to right |
| `{N}` | `keys[N]` — may reorder and repeat keys |
| `{{` / `}}` | A literal `{` / `}` |

Auto-numbered and indexed placeholders can be mixed, and `{}` counts only its own occurrences — 
the same rules as `std::format!`:

```rust
// db:
//   redis:
//     server: 127.0.0.1
//     port: 6379

let s = config.fmt("{{{0}:{1}}} via {0}", "db/redis", &["server", "port"]);
// Result: "{127.0.0.1:6379} via 127.0.0.1"
```

Every placeholder must have a corresponding key and every key must be used at least once. A 
mismatch in either direction is an error (empty string from `fmt`, `FormatError` from 
`fmt_strict`) rather than a silently half-formatted result. Substituted values are never 
rescanned, so a config value that itself contains `{}` is emitted verbatim.

## Multi-value formatting

```rust
// database:
//   host: localhost
//   port: 5432
//   name: myapp_db
//   username: admin

let db_url = config.fmt(
    "postgresql://{}@{}:{}/{}",
    "database",
    &["username", "host", "port", "name"]
);
// Result: "postgresql://admin@localhost:5432/myapp_db"
```

## Lenient vs strict

```rust
// Lenient - returns empty string if any value is missing
let connection = config.fmt("{}:{}", "database", &["host", "port"]);

// Strict - returns error if any value is missing
let connection = config.fmt_strict("{}:{}", "database", &["host", "port"])?;
```

## Escape sequences in fmt

If a key contains the separator, escape it with `\` — in the base path and in the keys 
alike, since both are ordinary paths:

```rust
// sections:
//   "db/redis":        <- key contains a literal slash
//     server: 127.0.0.1
//     port: 6379

let connection = config.fmt("{}:{}", r"sections/db\/redis", &["server", "port"]);
// Result: "127.0.0.1:6379"

// db:
//   "a/b": 1

let value = config.fmt("{}", "db", &[r"a\/b"]);
// Result: "1"
```

