# Escape Sequences

[← Documentation index](README.md)

Keys containing the path separator can be accessed using escape sequences.

- `\<sep>` — include a literal separator in the key (e.g. `\/` for `/`, `\::` for `::`)
- `\\` — include a literal backslash in the key
- Works with any separator: `/`, `::`, `->`, etc.

Because `\` is the escape character, a separator may not contain one — the splitter would 
read it as the start of an escape sequence and never match it as a separator. Constructing 
a config with such a separator is a `FormatError`.

```yaml
database:
  "host/port": localhost:5432      # Key contains /
  "user\name": admin\user          # Key contains \
```

```rust
let config = Config::load_yaml(yaml, "/").unwrap();

// Access key containing separator (/)
let value = config.str("database/host\\/port"); // -> "localhost:5432"

// Access key containing backslash (\)
let value = config.str("database/user\\\\name"); // -> "admin\user"
```

With a custom separator:

```rust
let config = Config::load_yaml(yaml, "::").unwrap();

// Path: a::b\::c::d navigates to keys ["a", "b::c", "d"]
let value = config.str("a::b\\::c::d");
```

## Overlapping multi-character separators

Escapes cover a **complete** separator or a backslash. They do not escape individual
characters of a multi-character separator. Some key boundaries therefore cannot be
represented unambiguously. For example, with separator `::`:

```yaml
'a:':
  b: 1
a:
  ':b': 2
```

The spelling `a:::b` resolves to keys `a` and `:b`, returning `2`. It cannot select
keys `a:` and `b`. `outline()` marks the first entry so it cannot be mistaken for a
usable path to that value:

```text
a:::b: <number>  # not addressable
a:::b: <number>
```

A marked spelling may resolve to a **different** value. Only unmarked entries are
paths to their corresponding leaves. Existing lookup and escape rules are unchanged;
in particular, `\:` does not escape a single colon when the separator is `::`.

Read an addressable parent mapping to access such keys, or choose a separator that
does not overlap their boundaries:

```rust
let parent = config.get_as::<trail_config::Value>("a:").unwrap();
assert_eq!(parent["b"].as_i64(), Some(1));
```

## Keys that have no path

Path segments are matched as **strings**, so a key that is not a string cannot be reached
by any accessor. YAML permits several — `1:`, `true:`, `~:`, even a sequence as a key:

```yaml
retries:
  1: fast        # an integer key
  2: slow
```

`config.str("retries/1")` looks up the *string* key `"1"`, which is a different key from
the integer `1`, so it returns `""`. There is no escape that changes this, and deliberately
so: one document can hold both `1:` and `"1":`, and any rule that made `retries/1` reach
the integer would make the string permanently unreachable instead. `outline()` lists such
keys and marks them `# not addressable`, so they are visible rather than silently missing.

The subtree containing them is still fully readable — deserialize it and the keys come back
typed:

```rust
use std::collections::BTreeMap;

let retries: BTreeMap<i64, String> = config.get_as("retries").unwrap();
assert_eq!(retries[&1], "fast");
```

The same applies to `true:` / `false:` (`BTreeMap<bool, _>`) and to the raw document via
`get("retries")`.

