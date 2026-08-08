# Contributing

Working on the crate itself, from a checkout of the
[repository](https://github.com/rbt-dev/trail-config). The scripts named below are
developer tooling and are deliberately excluded from the published package, so they are
not in the crate you get from crates.io — only in the repository.

For using the crate rather than working on it, see the [README](README.md) and the guides
in [`docs/`](docs).

## Running the checks

One command runs the full pre-release check — clippy and tests across every feature
combination, the doctests, the doc build and the package contents. There are two copies of
it, one per platform, doing the same steps in the same order:

```powershell
.\check.ps1           # clippy + tests × 5 feature combinations, doctests, docs, package
.\check.ps1 -Msrv     # also `cargo +1.85 check` (needs `rustup toolchain install 1.85`)
.\check.ps1 -Bench    # also the criterion benchmarks
.\check.ps1 -Docsrs   # also builds docs as docs.rs does (needs `rustup toolchain install nightly`)
```

```bash
./check.sh            # the same, under Linux — run it in WSL
./check.sh --msrv
./check.sh --bench
./check.sh --docsrs
```

**Run both.** This project has no CI, so a check script is the gate — but a script only
ever runs on the machine you are sitting at, and one machine is one platform. This crate
reads files by path, derives format from extensions and creates files exclusively, so the
platform axis is where the untested surface is. Between the two: Windows supplies a
case-insensitive filesystem, Linux supplies the Unix error kinds, a case-sensitive
filesystem and Unix path handling. macOS is essentially Linux plus a case-insensitive
filesystem, so it adds nothing on top of those two for this crate.

WSL is enough for the Linux half even with the checkout on a Windows drive: every
filesystem test goes through `tempfile::tempdir()`, which resolves to `/tmp` — inside WSL2
that is the ext4 VHD, not the mounted drive. `check.sh` builds into `target-linux/` rather
than `target/`, because cargo namespaces the build directory by profile and not by host
triple, so sharing it would make each toolchain invalidate the other's fingerprints and
rebuild everything on every switch.

`-Docsrs` / `--docsrs` builds with nightly and `--cfg docsrs`, which is the only
configuration where the `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]` labels on the
feature-gated items compile. Without it a mistake in one of those is invisible until the
crate is published and the rendered page is wrong.

The feature combinations matter: `json` and `toml` are additive gates, so code that
compiles with both enabled can still fail to compile with neither.

## Tests

Tests live in two places. `src/**/tests/` holds unit tests with access to internals;
`tests/` exercises the crate as a downstream consumer sees it, which is the only vantage
point that can catch a type missing from the public exports or a `$crate` path breaking
in the `config!` macro.

`examples/` is a third vantage point, and the only one that runs the crate as a program:
`cargo build --examples` compiles all four, and each is written to run unattended —
`cargo run --example web_server` and friends create their own config files in a temporary
directory rather than expecting anything in the working directory.

## Documentation

Three surfaces, each with a different job:

| Surface | Holds |
| ------- | ----- |
| [README.md](README.md) | The landing page — what the crate is, installation, a first config, and links onward. Rendered on crates.io, so it stays short |
| [`docs/`](docs) | The full guide, one file per topic, for reading on GitHub or in a checkout |
| Doc comments in `src/` | The same material against the API, rendered on docs.rs, with the examples run as doctests |

Prose that describes one method belongs on that method. Prose that spans several — layering,
hot reload, error handling — belongs in a `docs/` guide, with the item docs linking to it.
When a guide gains a new file, add it to [docs/README.md](docs/README.md) and to the table
in the README, which are the two indexes a reader arrives through.

Code in `docs/` is not compiled. Anything load-bearing enough to break silently is better
written as a doctest in `src/` or as a program in `examples/`, and referenced from the
guide.

