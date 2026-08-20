# Cribra contributor/development commands.
#
# `just` is the preferred local task runner. The Makefile mirrors the most
# common commands for contributors who do not use just.

set shell := ["sh", "-cu"]

default:
    @just --list

# Show available recipes.
help:
    @just --list

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all -- --check

# Check the default core build.
check:
    cargo check

# Check every optional core feature.
check-all:
    cargo check --all-features

# Check, lint, and build the native C adapter artifact skeleton.
capi:
    cargo check -p cribra-capi
    cargo clippy -p cribra-capi --all-targets -- -D warnings
    cargo build -p cribra-capi

# Run the default test surface.
test:
    cargo test

# Run tests with Serde enabled.
test-serde:
    cargo test --features serde

# Run the complete native test surface.
test-all:
    cargo test --all-features

# Run doctests.
test-doc:
    cargo test --doc

# Run Clippy with warnings denied.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Build default-feature documentation.
doc:
    cargo doc --no-default-features --no-deps

# Build full-feature documentation.
doc-all:
    cargo doc --all-features --no-deps

# Check the supported browser/WASM default contract.
wasm:
    cargo check --target wasm32-unknown-unknown --no-default-features

# Check the supported browser/WASM Serde contract.
wasm-serde:
    cargo check --target wasm32-unknown-unknown --no-default-features --features serde

# Check and test against Cribra's declared MSRV.
msrv:
    cargo +1.97.0 check
    cargo +1.97.0 check --all-features
    cargo +1.97.0 test
    cargo +1.97.0 test --all-features

# Run the local RustSec audit. Requires `cargo-audit`.
audit:
    cargo audit

# Verify the crate package from a clean working tree.
package:
    cargo package

# Verify the crate package while release files are still uncommitted.
package-dirty:
    cargo package --allow-dirty

# Perform a crates.io publication dry run from a clean working tree.
publish-dry-run:
    cargo publish --dry-run

# Perform a publication dry run while release files are still uncommitted.
publish-dry-run-dirty:
    cargo publish --dry-run --allow-dirty

# Run the fast local quality gate used during normal development.
gate: fmt-check check-all capi test test-serde test-all test-doc clippy doc doc-all wasm wasm-serde
    git diff --check

# Run the release-oriented local gate. Requires `cargo-audit`.
release-gate: gate msrv audit package publish-dry-run

# Run the historical benchmark suite.
bench:
    cargo bench --bench scan --all-features

# Run the v0.2 semantic-path benchmark suite.
bench-v02:
    cargo bench --bench v02 --all-features

# Run diagnostic contextual matcher benchmarks.
bench-diagnostics:
    cargo bench --bench isolation --all-features
    cargo bench --bench contextual_tail --all-features

# Run every maintained benchmark suite.
bench-all: bench bench-v02 bench-diagnostics

# Remove Cargo build artifacts.
clean:
    cargo clean
