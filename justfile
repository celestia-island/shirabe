# shirabe — browser automation via CDP.

set shell := ["bash", "-c"]

import "./celestia-devtools.just"

default:
    @just --list

# Format all sources.
fmt:
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Type-check all targets and features.
check:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo check --all-targets --all-features

# Clippy with -D warnings.
clippy:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings

# Run the test suite.
test:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features

# Build all features.
build:
    SHIRABE_SKIP_BROWSER_FETCH=1 cargo build --all-features

# One-shot local gate: fmt-check + clippy + test.
ci:
    just fmt-check
    just clippy
    just test
