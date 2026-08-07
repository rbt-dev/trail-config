<#
.SYNOPSIS
    Runs the full pre-release check: tests, doctests and clippy across every feature
    combination.

.DESCRIPTION
    This project has no CI by design, so the four feature combinations, the doctests 
    and the clippy runs are verified by hand. Doing that by hand is twelve invocations 
    and easy to half-finish; this is one.

    Each combination is a separate compilation of the crate: `json` and `toml` are
    additive feature gates, so code that compiles with both enabled can still fail to
    compile with neither, and a test that only exists under one feature is only run
    under that one.

.PARAMETER Msrv
    Also check the crate against the MSRV declared in Cargo.toml, which requires that
    toolchain to be installed (`rustup toolchain install 1.85`). This is `cargo check`
    rather than `cargo test`: dev-dependencies have a higher floor than the library
    (criterion 0.8 needs 1.86), so the tests cannot be built with it.

.PARAMETER Bench
    Also run the criterion benchmarks. Off by default — they take several minutes, and
    criterion's run-to-run comparison is not trustworthy on a loaded machine.

.PARAMETER Docsrs
    Also build the docs the way docs.rs does — nightly, `--all-features` and
    `--cfg docsrs` — which requires that toolchain (`rustup toolchain install nightly`).
    This is the only configuration in which the `#[cfg_attr(docsrs, doc(cfg(...)))]`
    feature labels compile at all, so a mistake in one is otherwise invisible until the
    crate is published and the rendered page is wrong.

.EXAMPLE
    .\check.ps1
    .\check.ps1 -Msrv
    .\check.ps1 -Docsrs
#>
[CmdletBinding()]
param(
    [switch]$Msrv,
    [switch]$Bench,
    [switch]$Docsrs
)

$ErrorActionPreference = 'Continue'

# The MSRV declared in Cargo.toml. Kept here so -Msrv checks the version the package
# actually claims rather than a number that drifted out of step with it.
$MsrvVersion = '1.85'

$combinations = @(
    @{ Name = 'default';             Args = @() },
    @{ Name = 'no default features'; Args = @('--no-default-features') },
    @{ Name = 'json only';           Args = @('--no-default-features', '--features', 'json') },
    @{ Name = 'toml only';           Args = @('--no-default-features', '--features', 'toml') },
    @{ Name = 'all features';        Args = @('--all-features') }
)

$failures = New-Object System.Collections.Generic.List[string]

function Invoke-Step {
    param(
        [string]$Label,
        [string[]]$Arguments
    )

    Write-Host ''
    Write-Host "==> $Label" -ForegroundColor Cyan
    Write-Host "    cargo $($Arguments -join ' ')" -ForegroundColor DarkGray

    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        Write-Host "    FAILED ($Label)" -ForegroundColor Red
        $script:failures.Add($Label)
    }
}

foreach ($combination in $combinations) {
    $name = $combination.Name
    $flags = $combination.Args

    # --all-targets so the integration tests and benches are linted too, not just the lib
    Invoke-Step "clippy [$name]" (@('clippy', '--all-targets') + $flags + @('--', '-D', 'warnings'))
    Invoke-Step "test [$name]"   (@('test') + $flags)
}

# Doctests run once: they are feature-independent, and `cargo test` above already ran
# them for each combination that compiles them. This pins the count in the summary.
Invoke-Step 'doctests' @('test', '--all-features', '--doc')

Invoke-Step 'docs' @('doc', '--all-features', '--no-deps')

# What `cargo publish` would actually upload. `exclude` in Cargo.toml keeps the review
# notes and this script out of the tarball; nothing else enforces that, and the failure is
# invisible until the crate is on crates.io, where a published version cannot be replaced.
# `--allow-dirty` so the check is usable mid-change: it inspects the file list, not the
# VCS state, and a dirty tree is the normal case when running this script.
Write-Host ''
Write-Host '==> package contents' -ForegroundColor Cyan
Write-Host '    cargo package --list --allow-dirty' -ForegroundColor DarkGray

$packaged = & cargo package --list --allow-dirty
if ($LASTEXITCODE -ne 0) {
    Write-Host '    FAILED (package contents)' -ForegroundColor Red
    $failures.Add('package contents')
} else {
    $unwanted = $packaged | Where-Object { $_ -like 'IMPROVEMENTS_*.md' -or $_ -eq 'check.ps1' }
    if ($unwanted) {
        Write-Host '    FAILED (package contents): excluded files are in the crate' -ForegroundColor Red
        foreach ($file in $unwanted) {
            Write-Host "      $file" -ForegroundColor Red
        }
        Write-Host '    check `exclude` in Cargo.toml' -ForegroundColor DarkGray
        $failures.Add('package contents')
    } else {
        Write-Host "    $($packaged.Count) file(s), none excluded-but-present" -ForegroundColor DarkGray
    }
}

if ($Msrv) {
    $installed = & rustup toolchain list
    if ($installed -match [regex]::Escape($MsrvVersion)) {
        # `check`, not `test`: dev-dependencies require a newer toolchain than the library
        Invoke-Step "MSRV $MsrvVersion [all features]" @("+$MsrvVersion", 'check', '--all-features')
        Invoke-Step "MSRV $MsrvVersion [no default features]" @("+$MsrvVersion", 'check', '--no-default-features')
    } else {
        Write-Host ''
        Write-Host "==> MSRV $MsrvVersion skipped: toolchain not installed" -ForegroundColor Yellow
        Write-Host "    rustup toolchain install $MsrvVersion" -ForegroundColor DarkGray
        $failures.Add("MSRV $MsrvVersion (toolchain missing)")
    }
}

if ($Docsrs) {
    $installed = & rustup toolchain list
    if ($installed -match 'nightly') {
        # Mirrors [package.metadata.docs.rs] in Cargo.toml. `doc_cfg` is a nightly
        # feature requested behind the `docsrs` cfg, so nothing below this line is
        # exercised by any other step in this script.
        $previous = $env:RUSTDOCFLAGS
        $env:RUSTDOCFLAGS = '--cfg docsrs'
        try {
            Invoke-Step 'docs.rs docs [nightly, --cfg docsrs]' @('+nightly', 'doc', '--all-features', '--no-deps')
        } finally {
            if ($null -eq $previous) {
                Remove-Item Env:\RUSTDOCFLAGS -ErrorAction SilentlyContinue
            } else {
                $env:RUSTDOCFLAGS = $previous
            }
        }
    } else {
        Write-Host ''
        Write-Host '==> docs.rs docs skipped: nightly toolchain not installed' -ForegroundColor Yellow
        Write-Host '    rustup toolchain install nightly' -ForegroundColor DarkGray
        $failures.Add('docs.rs docs (nightly missing)')
    }
}

if ($Bench) {
    Invoke-Step 'bench' @('bench', '--all-features')
}

Write-Host ''
if ($failures.Count -eq 0) {
    Write-Host 'All checks passed.' -ForegroundColor Green
    exit 0
}

Write-Host "$($failures.Count) check(s) failed:" -ForegroundColor Red
foreach ($failure in $failures) {
    Write-Host "  - $failure" -ForegroundColor Red
}
exit 1
