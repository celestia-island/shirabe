# shirabe — browser automation via CDP.

# Windows: PowerShell (the 5.1 floor ships with every Windows; pwsh 7 is
# NOT assumed). Linewise recipes must stay PS-5.1-safe: no `&&` chains,
# `cd X; cmd` instead of `cd X && cmd`. Bash-only recipes use
# [script('bash')] and need Git Bash (or WSL) when actually run.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command", "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $PSDefaultParameterValues['*:Encoding']='utf8';"]
set shell := ["bash", "-c"]
# On Windows just resolves recipe shebangs through the shell named here; without
# it just falls back to `cygpath`, which Git for Windows does not put on PATH,
# so every shebang recipe fails with "could not find cygpath executable".
# `set lists` enables which() (used by the imported celestia-devtools.just);
# `set unstable` gates it.
set unstable
set lists

# Repo definitions override the shared template's (imported above).
set allow-duplicate-recipes
set allow-duplicate-variables

import "./celestia-devtools.just"

default:
    @just --list

# Format all sources.
fmt:
    just fmt-toml
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Type-check all targets and features.
[unix]
check:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo check --all-targets --all-features

[windows]
check:
    $env:SHIRABE_SKIP_BROWSER_FETCH='1'; cargo check --all-targets --all-features

# Clippy with -D warnings.
[unix]
clippy:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings

[windows]
clippy:
    $env:SHIRABE_SKIP_BROWSER_FETCH='1'; cargo clippy --all-targets --all-features -- -D warnings

# Run the test suite.
[unix]
test:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features

[windows]
test:
    $env:SHIRABE_SKIP_BROWSER_FETCH='1'; cargo test --all-features

# Build all features.
[unix]
build:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo build --all-features

[windows]
build:
    $env:SHIRABE_SKIP_BROWSER_FETCH='1'; cargo build --all-features

# One-shot local gate: fmt-check + clippy + test.
ci:
    just fmt-check
    just clippy
    just test

# ── npx distribution (local dry-run) ─────────────────────────────────────────
#
# These wrap the shared recipes from celestia-devtools.just with shirabe's
# metadata. CI does the actual publish (see .github/workflows/npm-release.yml);
# locally these only stage ./dist and run `npm pack --dry-run`.
#
#   just npm-dist-local                              # reassemble root from existing dist/
#   just npm-dist-local 0.1.0 path/to/shirabe x86_64-unknown-linux-gnu
[unix]
npm-dist-local version='' binary='' target='':
    SHIRABE_SKIP_BROWSER_FETCH=1 just npm-dist shirabe {{version}} {{binary}} {{target}}

[windows]
npm-dist-local version='' binary='' target='':
    $env:SHIRABE_SKIP_BROWSER_FETCH='1'; just npm-dist shirabe {{version}} {{binary}} {{target}}
