#!/usr/bin/env bash
#
# Runs the full pre-release check on Linux: tests, doctests and clippy across every
# feature combination. The counterpart to check.ps1, which is the same gate on Windows.
#
# Why both exist. This project has no CI, so check.ps1 is the gate — but it can only run
# on the machine it is invoked from, and that machine is Windows. This crate reads files
# by path, derives format from extensions and creates files exclusively, so the platform
# axis is where the untested surface is. Run this under WSL and the two together cover it:
# Windows supplies a case-insensitive filesystem, Linux supplies Unix error kinds, a
# case-sensitive filesystem and Unix path handling. macOS adds essentially nothing on top
# of those two for this crate.
#
# WSL is enough for that despite the source tree living on /mnt. Every filesystem test
# goes through tempfile::tempdir(), which resolves to /tmp — inside WSL2 that is the ext4
# VHD, not the Windows drive, so create_new, the error-kind mapping and the load_or_create
# race all get genuine Linux semantics wherever the checkout sits.
#
# Usage:
#   ./check.sh
#   ./check.sh --msrv
#   ./check.sh --docsrs
#   ./check.sh --bench
#
#   --msrv    Also check against the MSRV declared in Cargo.toml, which needs that
#             toolchain installed *in this environment* — a rustup in WSL is a separate
#             install from the one on Windows. `cargo check`, not `cargo test`:
#             dev-dependencies have a higher floor than the library (criterion 0.8 needs
#             1.86), so the tests cannot be built with it.
#   --bench   Also run the criterion benchmarks. Off by default — they take several
#             minutes, and criterion's run-to-run comparison is not trustworthy on a
#             loaded machine, which a WSL VM sharing a host generally is.
#   --docsrs  Also build the docs the way docs.rs does — nightly, --all-features and
#             --cfg docsrs. The only configuration in which the
#             #[cfg_attr(docsrs, doc(cfg(...)))] labels compile at all, so a mistake in
#             one is otherwise invisible until the crate is published.

set -uo pipefail

# Deliberately not `set -e`. Every step runs and the failures are listed at the end, which
# is what makes one invocation worth more than twelve: a run tells you everything that is
# broken rather than the first thing.

# Windows and Linux builds both land in target/debug — cargo namespaces by profile, not by
# host triple — so sharing the directory makes each toolchain invalidate the other's
# fingerprints and rebuild the world on every switch. A separate directory costs disk and
# saves that. Respects an existing CARGO_TARGET_DIR if one is already set.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-linux}"

# The MSRV declared in Cargo.toml. Kept here, as in check.ps1, so --msrv checks the version
# the package actually claims rather than a number that drifted out of step with it.
MSRV_VERSION='1.85'

# Files `exclude` in Cargo.toml is supposed to keep out of the published crate.
UNWANTED_PATTERNS=('IMPROVEMENTS_*.md' 'check.ps1' 'check.sh')

run_msrv=0
run_bench=0
run_docsrs=0

for arg in "$@"; do
    case "$arg" in
        --msrv)   run_msrv=1 ;;
        --bench)  run_bench=1 ;;
        --docsrs) run_docsrs=1 ;;
        -h|--help)
            # The header comment, unprefixed: everything after the shebang up to the
            # first line that is not a comment. A hardcoded line range goes stale the
            # first time the header grows.
            awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
            exit 0
            ;;
        *)
            echo "unknown option: $arg" >&2
            echo "usage: $0 [--msrv] [--bench] [--docsrs]" >&2
            exit 2
            ;;
    esac
done

# Colours, but only when stdout is a terminal — piping this into a file or a pager should
# not fill it with escape sequences.
if [ -t 1 ]; then
    CYAN=$'\033[36m'; GREY=$'\033[90m'; RED=$'\033[31m'; GREEN=$'\033[32m'
    YELLOW=$'\033[33m'; RESET=$'\033[0m'
else
    CYAN=''; GREY=''; RED=''; GREEN=''; YELLOW=''; RESET=''
fi

failures=()

step() {
    local label="$1"; shift

    echo
    echo "${CYAN}==> ${label}${RESET}"
    echo "${GREY}    cargo $*${RESET}"

    if ! cargo "$@"; then
        echo "${RED}    FAILED (${label})${RESET}"
        failures+=("$label")
    fi
}

# Each combination is a separate compilation of the crate: `json` and `toml` are additive
# feature gates, so code that compiles with both enabled can still fail to compile with
# neither, and a test that only exists under one feature is only run under that one.
#
# Name and flags are packed into one string because bash has no array of arrays; the flags
# are word-split deliberately on use.
combinations=(
    'default|'
    'no default features|--no-default-features'
    'json only|--no-default-features --features json'
    'toml only|--no-default-features --features toml'
    'all features|--all-features'
)

for combination in "${combinations[@]}"; do
    name="${combination%%|*}"
    flags="${combination#*|}"

    # shellcheck disable=SC2086  # word splitting of $flags is the point
    # --all-targets so the integration tests and benches are linted too, not just the lib
    step "clippy [$name]" clippy --all-targets $flags -- -D warnings
    # shellcheck disable=SC2086
    step "test [$name]" test $flags
done

# Doctests run once: they are feature-independent, and `cargo test` above already ran them
# for each combination that compiles them. This pins the count in the summary.
step 'doctests' test --all-features --doc

step 'docs' doc --all-features --no-deps

# What `cargo publish` would actually upload. `exclude` in Cargo.toml keeps the review
# notes and the two check scripts out of the tarball; nothing else enforces that, and the
# failure is invisible until the crate is on crates.io, where a published version cannot be
# replaced. `--allow-dirty` so the check is usable mid-change: it inspects the file list,
# not the VCS state, and a dirty tree is the normal case when running this script.
echo
echo "${CYAN}==> package contents${RESET}"
echo "${GREY}    cargo package --list --allow-dirty${RESET}"

if ! packaged="$(cargo package --list --allow-dirty)"; then
    echo "${RED}    FAILED (package contents)${RESET}"
    failures+=('package contents')
else
    unwanted=()
    while IFS= read -r file; do
        [ -n "$file" ] || continue
        for pattern in "${UNWANTED_PATTERNS[@]}"; do
            # shellcheck disable=SC2053  # unquoted $pattern so the glob is a glob
            if [[ $file == $pattern ]]; then
                unwanted+=("$file")
            fi
        done
    done <<< "$packaged"

    if [ ${#unwanted[@]} -gt 0 ]; then
        echo "${RED}    FAILED (package contents): excluded files are in the crate${RESET}"
        printf "${RED}      %s${RESET}\n" "${unwanted[@]}"
        echo "${GREY}    check \`exclude\` in Cargo.toml${RESET}"
        failures+=('package contents')
    else
        echo "${GREY}    $(printf '%s\n' "$packaged" | grep -c .) file(s), none excluded-but-present${RESET}"
    fi
fi

if [ "$run_msrv" -eq 1 ]; then
    if rustup toolchain list 2>/dev/null | grep -qF "$MSRV_VERSION"; then
        # `check`, not `test`: dev-dependencies require a newer toolchain than the library
        step "MSRV $MSRV_VERSION [all features]" "+$MSRV_VERSION" check --all-features
        step "MSRV $MSRV_VERSION [no default features]" "+$MSRV_VERSION" check --no-default-features
    else
        echo
        echo "${YELLOW}==> MSRV $MSRV_VERSION skipped: toolchain not installed${RESET}"
        echo "${GREY}    rustup toolchain install $MSRV_VERSION${RESET}"
        echo "${GREY}    (rustup in WSL is a separate install from the one on Windows)${RESET}"
        failures+=("MSRV $MSRV_VERSION (toolchain missing)")
    fi
fi

if [ "$run_docsrs" -eq 1 ]; then
    if rustup toolchain list 2>/dev/null | grep -q 'nightly'; then
        # Mirrors [package.metadata.docs.rs] in Cargo.toml. `doc_cfg` is a nightly feature
        # requested behind the `docsrs` cfg, so nothing here is exercised by any other step.
        # Scoped to the one command rather than exported, so nothing later inherits it.
        echo
        echo "${CYAN}==> docs.rs docs [nightly, --cfg docsrs]${RESET}"
        echo "${GREY}    RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --all-features --no-deps${RESET}"
        if ! RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --all-features --no-deps; then
            echo "${RED}    FAILED (docs.rs docs)${RESET}"
            failures+=('docs.rs docs')
        fi
    else
        echo
        echo "${YELLOW}==> docs.rs docs skipped: nightly toolchain not installed${RESET}"
        echo "${GREY}    rustup toolchain install nightly${RESET}"
        failures+=('docs.rs docs (nightly missing)')
    fi
fi

if [ "$run_bench" -eq 1 ]; then
    step 'bench' bench --all-features
fi

echo
if [ ${#failures[@]} -eq 0 ]; then
    echo "${GREEN}All checks passed.${RESET}"
    exit 0
fi

echo "${RED}${#failures[@]} check(s) failed:${RESET}"
printf "${RED}  - %s${RESET}\n" "${failures[@]}"
exit 1
