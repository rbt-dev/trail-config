# Merging Configs

[← Documentation index](README.md)

Use `merge_required` / `merge_optional` to layer configs on top of each other. Values in 
the overlay take precedence over the base; nested mappings are merged recursively so sibling 
keys are preserved. Sequences are replaced wholesale. The base config's separator is preserved.

Key order follows the base document: an overridden key keeps its position and genuinely-new 
overlay keys are appended, at every level of nesting. The merged order therefore does not 
depend on the order of the overlays. This is invisible through the scalar accessors but 
visible to anything order-preserving — deserializing into a `Mapping`, an `IndexMap` or a 
`Vec<(K, V)>`, and any re-serialization you do downstream.

Document order holds for all three formats, including a chain that mixes them. Both 
`serde_json` and `toml` store objects in a `BTreeMap` by default, which would sort every 
`.json` and `.toml` config's keys alphabetically before this crate ever saw the file; 
neither does here.

The overlay filenames are recorded so that `reload()` can re-read and re-apply them in 
order — required overlays that are missing on reload return an error, optional overlays that 
are missing are silently skipped.

## Chaining or in place

Each merge comes in two forms, differing only in signature:

| Form | Signature | On failure |
| ---- | --------- | ---------- |
| `merge_required(file, env)` / `merge_optional(file, env)` | consume `self`, return `Result<Config, _>` | the base config is gone — it was moved into the call |
| `merge_required_in_place(file, env)` / `merge_optional_in_place(file, env)` | `&mut self`, return `Result<(), _>` | the receiver is untouched — filename, document and overlay chain all as they were |

Overlay rules, recorded filenames, recorded environments and errors are identical; the
chaining forms are defined in terms of the in-place ones. Use the chaining form to build a
config from a known set of files, and the in-place form when a merge might fail and the base
is worth keeping:

```rust
use trail_config::Config;

let mut config = Config::load_required("config.yaml", "/", None)?;

// Absent is fine and silent; unreadable is reported and the base survives it
if let Err(e) = config.merge_optional_in_place("config.local.yaml", None) {
    eprintln!("config.local.yaml is unusable, continuing without it: {e}");
}

let port = config.get_int("app/port"); // the base's value, whichever way that went
```

That matters most for `merge_optional`, whose whole purpose is making an overlay
survivable. A *missing* optional file is skipped, but a present-but-unparseable one is an
error — and with the chaining form that error arrived after the base had already been moved
into the call, so "use `config.local.yaml` if it is present **and readable**, otherwise carry
on" could not be written at all.

The guarantee is mechanical rather than careful bookkeeping: the overlay is resolved, read,
parsed and interpolated into a local before anything on `self` is touched, so every failure
returns before the first mutation. It is the same guarantee `reload` and `reload_from`
already make.

## Tagged values

A YAML `!Tag` — how serde spells an enum variant — is part of a value's *shape*, not a
value inside it. Two nodes under the same tag merge like the mappings they usually are:

```yaml
# base.yaml            # overlay.yaml         # result
db: !Postgres          db: !Postgres          db: !Postgres
  host: keep-me          port: 6543             host: keep-me
  port: 5432                                    port: 6543
```

A **differing** tag replaces instead, as does an untagged overlay onto a tagged base.
Changing or dropping the tag changes which variant the document describes, so merging the
two sets of fields would produce a document belonging to neither — and an overlay that
quietly dropped the tag would leave a document that no longer deserializes into the enum
the base named.

Everywhere else a tag is transparent: `db/host` resolves whether or not `db` is tagged,
`str` on a tagged scalar returns the scalar, and `outline` lists a tagged mapping's keys.
Only `get` and `get_as` still see the tag, because selecting the variant is what it is for.

## Clearing a value with null

An overlay value takes precedence whatever it is, including null — so setting a key to null 
is how an overlay clears an inherited value:

```yaml
# config.local.yaml
database:
  password:        # bare `key:` is YAML for null — clears the base password
telemetry: null    # explicit form, identical in effect
cache:             # a subtree set to null clears the whole subtree
```

The key is *cleared*, not removed: it remains present holding a null, so `contains` still 
returns `true` while `str` returns `""` and the typed accessors (`get_int`, `get_bool`, …) 
return `None`. `get(path)` tells the two apart: a cleared key yields `Some` holding a null 
value, a key that was never set yields `None`.

An overlay file that is **entirely** empty — zero bytes, or nothing but comments — is the one 
exception. It is a no-op rather than a document-wide clear, which is what makes an absent 
`merge_optional` file and an empty one behave the same way.

```rust
use trail_config::Config;

let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

// Named once at the base; the overlay's {env} resolves against it
let mut config = Config::load_required("config.yaml", "/", Some(&env))?
    .merge_required("config.{env}.yaml", None)?
    .merge_optional("config.local.yaml", None)?;
```

Given these files:

```yaml
# config.yaml (base)
app:
  port: 8080
  debug: false
  name: myapp
database:
  host: localhost
  port: 5432
```

```yaml
# config.prod.yaml (overlay)
app:
  debug: false
database:
  host: prodserver
```

```yaml
# config.local.yaml (optional personal overrides)
app:
  debug: true
```

The merged result will be:

```yaml
app:
  port: 8080        # from base
  debug: true       # from config.local.yaml (last overlay wins)
  name: myapp       # from base
database:
  host: prodserver  # from config.prod.yaml
  port: 5432        # from base — sibling preserved
```

