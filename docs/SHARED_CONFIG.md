# Thread-Safe Shared Config

[← Documentation index](README.md)

`Config` is `Send + Sync`, so sharing one across threads needs nothing from this crate:

```rust
use trail_config::Config;
use std::{sync::Arc, thread};

let config = Arc::new(Config::load_required("config.yaml", "/", None)?);

for _ in 0..8 {
    let config = Arc::clone(&config);
    thread::spawn(move || println!("{}", config.str("app/name")));
}
```

What an `Arc<Config>` cannot do is **replace** the document behind those shared references — 
`reload()` takes `&mut self`, and an `Arc` hands out `&`. That is what `ConfigHandle` adds: 
interior mutability, so every holder sees the new document after a reload. It wraps `Config` in 
an `Arc<RwLock<Arc<Config>>>` — cloning the handle is cheap, and all clones refer to the same 
underlying config.

So the choice is about *reloading*, not about thread-safety: use an `Arc<Config>` when the 
config is read-only for the life of the process, and a `ConfigHandle` when it changes at 
runtime.

```rust
use trail_config::{Config, ConfigHandle};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)?
);

// Cheap to clone — share across threads
let handle2 = handle.clone();

// Convenience methods for common accessors
let port = handle.get_int("app/port");
let debug = handle.get_bool("app/debug");
let features = handle.list("features/beta");

// Full Config access via a snapshot (Arc<Config> derefs to Config)
let db: DatabaseConfig = handle.read().get_as_strict("database")?;

// Reload from disk — re-applies all overlays
handle.reload()?;
// All clones immediately see the updated values

// Or switch to a different file, clearing the overlay chain
handle.reload_from("other_config.yaml")?;
```

Neither reads nor reloads hold a lock for long. `read()` locks only long enough to clone an
`Arc` and returns an immutable **snapshot**; `reload()` copies the source list (base filename
plus the overlay chain), does all file reads and parsing with **no lock held**, and takes the
write lock only for a pointer swap. So readers are never blocked on disk I/O, and holding a
snapshot never blocks a reload. If the reload fails, no swap happens and the existing config
is left unchanged.

Reloads are serialized against each other by a separate lock that readers never touch, so a
second concurrent reload waits for the first rather than reading the files alongside it.
Without that, two overlapping reloads would each build a config off to the side and the
*slower* one would swap last — leaving the handle serving a superseded document indefinitely
even though both calls returned `Ok`. Two call sites that can reload independently is all it
takes: a debounced file watcher that fires twice on one write, or a watcher plus a SIGHUP
handler. The wait is paid only by reloads.

`ConfigHandle` mirrors the complete **lenient** accessor surface of `Config` — `get`, `str`,
`list`, `contains`, `get_int`, `get_float`, `get_bool`, `get_as`, `deserialize` and `fmt` — each
a shorthand for the same call on a snapshot. The `*_strict` variants and the metadata accessors
(`filename`, `environment`) are reached through `read()`, which gets you every `Config`
method taking `&self`.

The methods that don't take `&self` can't be reached that way, since a snapshot only ever
derefs to `&Config`:

| `Config` method | Reaching it from a handle |
| --------------- | ------------------------- |
| `reload` (`&mut self`) | Mirrored as `handle.reload()` |
| `reload_from` (`&mut self`) | Mirrored as `handle.reload_from(file)` |
| `merge_required` / `merge_optional` (consume `self`) | Not offered — layer the files, *then* wrap the result in a handle |
| `merge_required_in_place` / `merge_optional_in_place` (`&mut self`) | Not offered either — see below |

So a handle is not bound to its file for life, but its overlay chain is fixed at
construction: `reload` re-applies it, `reload_from` clears it.

The [in-place merges](MERGING.md#chaining-or-in-place) have a signature a handle *could* mirror, since
`&mut self` is no obstacle to interior mutability — they are left off for a reason about
layering rather than about signatures. A handle re-reads the sources its config was built
from; it does not acquire new ones behind existing snapshots. Layer the files first, then
wrap the result.

Because a snapshot is immutable, a concurrent reload can never change it underneath you — take
one snapshot when you need several values to agree (each convenience call takes its own):

```rust
let snapshot = handle.read();
let host = snapshot.str("database/host");
let port = snapshot.get_int("database/port"); // guaranteed to match `host`
```

## Background reload example

```rust
use trail_config::{Config, ConfigHandle};
use std::{thread, time::Duration};

let handle = ConfigHandle::new(
    Config::load_required("config.yaml", "/", None)?
        .merge_optional("config.local.yaml", None)?
);

// Spawn a background thread to reload every 30 seconds
let reload_handle = handle.clone();
thread::spawn(move || {
    loop {
        thread::sleep(Duration::from_secs(30));
        if let Err(e) = reload_handle.reload() {
            eprintln!("Config reload failed: {}", e);
        }
    }
});

// Main thread reads are never blocked except during the brief reload swap
loop {
    let timeout = handle.get_int("app/timeout").unwrap_or(30);
    // ...
}
```

