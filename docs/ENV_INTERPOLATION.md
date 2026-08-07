# Environment Variable Interpolation

[← Documentation index](README.md)

Trail Config resolves `${VAR}` placeholders in string values at load time using environment 
variables. Placeholders can include a default value with `${VAR:-default}`.

```yaml
# config.yaml
database:
  host: ${DB_HOST:-localhost}
  port: 5432
  password: ${DB_PASSWORD}
app:
  url: ${APP_PROTO:-https}://${APP_DOMAIN}/api
```

```rust
use trail_config::Config;

// With DB_HOST=prodserver, DB_PASSWORD=secret and APP_DOMAIN=example.com set.
// All three are needed: `DB_PASSWORD` and `APP_DOMAIN` have no `:-default`, so an
// absent one fails the whole load with a FormatError rather than yielding "".
let config = Config::load_required("config.yaml", "/", None)?;
assert_eq!(config.str("database/host"), "prodserver");
assert_eq!(config.str("database/password"), "secret");
assert_eq!(config.str("app/url"), "https://example.com/api");
```

## Syntax

| Pattern | Behaviour |
| ------- | --------- |
| `${VAR}` | Replaced with the value of `VAR`. Error if not set. |
| `${VAR:-default}` | Replaced with the value of `VAR`, or `default` if `VAR` is not set. |
| `${VAR:-}` | Replaced with the value of `VAR`, or an empty string if not set. |
| `${VAR:-${OTHER:-x}}` | Defaults may nest — resolution falls through each level in turn. |
| `$${VAR}` | Escaped — produces the literal text `${VAR}`, with no lookup. |
| `$VAR` | Not a placeholder — left as-is. |
| `$` anywhere else | Left as-is (`$100` and `Pa$$w0rd!` pass through unchanged). |

## Set is not absent

A default applies only when the variable is **absent**. There are two ways a variable can 
be set and still not give you a usable string, and neither falls back.

**Set but empty.** The empty string is used:

```rust
// VAR="" (set, empty)
config.str("app/value") // -> ""  — not "fallback"
```

This differs from shell `${VAR:-default}`, which falls back when the variable is unset *or* 
empty. Use `${VAR:-}` when you want "empty if missing" explicitly.

**Set but not valid Unicode.** A `FormatError` naming that as the cause — the default is 
not applied:

```text
Environment variable 'DB_HOST' is set but is not valid Unicode ("a\u{d800}")
  — the default, if any, is not applied because the variable is set
```

Applying the default here would be the worse outcome: the deployment would run on a 
fallback value while you believed your setting had taken effect. Reporting it as "not set" 
would be almost as bad, sending you to verify an export that is already correct.

## Escaping

Only `$${` is an escape sequence, producing a literal `${`. Every other `$` is passed through 
untouched, so passwords and shell snippets survive without modification:

```yaml
db:
  password: Pa$$w0rd!               # unchanged
  template: $${HOME}/data           # -> "${HOME}/data", no lookup
  price: $100                       # unchanged
```

## Resolution timing

Environment variables are resolved at load time and re-resolved on every `reload()` call. 
This means changes to environment variables are picked up when the config is reloaded.

A variable's *value* is never rescanned, so a secret that happens to contain `${...}` is 
used verbatim rather than being expanded again.

## Scope: values, never keys

Interpolation applies to string **values** only. A `${VAR}` written as a mapping key stays 
the literal text `${VAR}` and is addressed by that text:

```yaml
${DB_HOST}: value      # a key literally named "${DB_HOST}" — no lookup, no error
host: ${DB_HOST}       # interpolated
```

This is deliberate. Interpolating keys would make the set of valid config *paths* depend 
on the environment, so a path that resolves on one machine would silently miss on another, 
and an unset variable would become a missing key rather than an error.

A YAML tag is exempt for the same reason — it selects a variant, so interpolating it would 
make the document's shape depend on the environment. Values *underneath* a tag are 
interpolated normally, including the unset-variable error:

```yaml
db: !Postgres
  password: ${DB_PASSWORD}   # interpolated; still an error if DB_PASSWORD is unset
```

## Error handling

If a placeholder references an unset variable and no default is provided, loading returns 
a `ConfigError::FormatError`. These also return errors:

| Input | Reason |
| ----- | ------ |
| `${VAR` | Unclosed placeholder |
| `${VAR:-${X}` | Unclosed — the inner placeholder closes, the outer one does not |
| `${:-default}` | Empty variable name |
| `${${PREFIX}_HOST}` | Nesting is supported in defaults, not in the variable name |
| `${A:-${A:-${A:-…}}}` | Defaults nested more than 32 levels deep |
| `${VAR}` / `${VAR:-x}` where `VAR` is set to non-Unicode bytes | Set is not absent — the default does not apply |

Defaults may nest, but only to a fixed depth of 32 — far beyond the two or three
fallback levels any real config uses. A deeper chain is a `FormatError` rather than an
unbounded recursion that would abort the process.

The one shape that cannot be expressed is an unbalanced `}` inside a default — `${VAR:-a}b}` 
ends the placeholder at the first `}`, giving the default `a` followed by the literal `b}`.

