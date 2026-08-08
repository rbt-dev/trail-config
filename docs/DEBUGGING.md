# Debug Output

[← Documentation index](README.md)

`{:?}` on a `Config` or `ConfigHandle` prints the config's **shape**, never its values:

```text
Config { filename: "config.yaml", separator: "/", environment: Some("prod"),
         overlays: [Required("config.prod.yaml"), Optional("config.local.yaml")],
         content: <4 keys> }
```

Environment variables are already interpolated by the time `Debug` runs, so printing the 
document would put `${DB_PASSWORD}` and `${API_TOKEN}` in cleartext into any panic message, 
log line or `anyhow` context chain. Filenames and the overlay chain are printed — they are 
not secrets, and they are what you need when a `reload()` does not do what you expected.

To inspect actual values, read them explicitly with the accessors.

## Listing the paths a config contains

`outline()` answers "why is this path not resolving" by printing the paths that **do** 
resolve, one per line, with values replaced by their types:

```rust
let config = Config::load_required("config.yaml", "/", None)?;
println!("{}", config.outline());
```

```text
app/name: <string>
app/port: <number>
db/redis/server: <string>
features: <2 items>
```

Each line is spelled exactly as an accessor takes it — the config's own separator, with 
keys containing a separator or a backslash escaped — so a line can be pasted straight into 
`str()` or `get()`. Keys appear in document order.

Two kinds of key cannot be written as a path at all: an **empty** key, since every path 
segment must be non-empty, and a **non-string** key (`1:`, `true:`), since a segment is 
matched as a string. YAML allows both. They are still listed — a key you cannot reach is 
exactly what you want to see when a lookup is failing — but marked, so no line ever claims 
a path that does not resolve:

```text
app/port: <number>
"": <string>                   # not addressable
retries/1: <string>            # not addressable
```

The marker covers the whole line, so anything nested under such a key carries it too. 
Every line *without* one resolves as written.

Values are never printed, only their types, which is what makes the output safe to log or 
paste into an issue: `${DB_PASSWORD}` is already interpolated by the time a `Config` 
exists. If you do want the whole document, deserialize it into a `Value` or `Mapping` and 
serialize that yourself — an explicit act at the call site.

