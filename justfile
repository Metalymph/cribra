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

# Run benchmarks for the native C adapter.
capi-bench:
	cargo bench -p cribra-capi --bench abi

# Generate the committed native C header from the Rust ABI adapter.
capi-header:
    mkdir -p include
    cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output include/cribra.h

# Verify that regenerating the native C header is deterministic and leaves the
# committed header unchanged.
capi-header-check:
    mkdir -p target/c-smoke
    cp include/cribra.h target/c-smoke/cribra.h.expected
    cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output target/c-smoke/cribra.h.generated
    diff -u target/c-smoke/cribra.h.expected target/c-smoke/cribra.h.generated

# Build and run the real C consumer against the native dynamic library.
#
# macOS and Linux use an executable rpath so the smoke test does not require
# DYLD_LIBRARY_PATH / LD_LIBRARY_PATH. Windows is exercised by CI.
capi-smoke: capi-header
    cargo build -p cribra-capi
    mkdir -p target/c-smoke
    if [ "$(uname -s)" = "Darwin" ]; then \
        clang -std=c11 -Wall -Wextra -Werror -Iinclude \
          crates/cribra-capi/tests/c/smoke.c \
          -Ltarget/debug -lcribra_capi \
          -Wl,-rpath,@loader_path/../debug \
          -o target/c-smoke/cribra-smoke; \
    elif [ "$(uname -s)" = "Linux" ]; then \
        cc -std=c11 -Wall -Wextra -Werror -Iinclude \
          crates/cribra-capi/tests/c/smoke.c \
          -Ltarget/debug -lcribra_capi \
          -Wl,-rpath,'$ORIGIN/../debug' \
          -o target/c-smoke/cribra-smoke; \
    else \
        echo "capi-smoke local recipe currently supports macOS and Linux; Windows is covered in CI"; \
        exit 2; \
    fi
    target/c-smoke/cribra-smoke

# Verify that the generated header and the native dynamic library expose the
# same public `cribra_*` function set on macOS/Linux.
capi-symbols: capi-header
    cargo build -p cribra-capi
    sh crates/cribra-capi/tests/c/check-capi-symbols.sh

# Build the static ABI artifact and verify that it is produced.
#
# Full static linking is platform-specific because Rust staticlibs require
# target-native system libraries at the final link step; cross-platform static
# linking is therefore validated in CI rather than guessed here.
capi-static:
    cargo build -p cribra-capi
    test -f target/debug/libcribra_capi.a

# Run the complete local native-C interoperability smoke surface.
capi-smoke-all: capi-header-check capi-smoke capi-symbols capi-static

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
