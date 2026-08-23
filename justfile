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

# Check the reusable JS/WASM adapter.
wasm-adapter-check:
    cargo check -p cribra-wasm --target wasm32-unknown-unknown
    cargo clippy -p cribra-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings

# Build the reusable JS/WASM adapter.
wasm-adapter-build:
    cargo build -p cribra-wasm --target wasm32-unknown-unknown --release

# Generate browser-native ES module glue, TypeScript declarations, and WASM.
wasm-adapter-bindgen:
    mkdir -p target/wasm
    wasm-bindgen \
      --target web \
      --out-dir target/wasm \
      --out-name cribra \
      target/wasm32-unknown-unknown/release/cribra_wasm.wasm

# Validate the complete reusable JS/WASM adapter artifact.
wasm-adapter: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen
    test -s target/wasm/cribra.js
    test -s target/wasm/cribra.d.ts
    test -s target/wasm/cribra_bg.wasm
    test -s target/wasm/cribra_bg.wasm.d.ts

# Produce the reproducible Binaryen comparison matrix from one identical
# wasm-bindgen baseline. These are benchmark inputs, not release artifacts.
wasm-opt-build: wasm-adapter
    mkdir -p target/wasm-opt
    cp target/wasm/cribra_bg.wasm target/wasm-opt/cribra_bg.base.wasm
    wasm-opt -Os target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.os.wasm
    wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.oz.wasm
    wasm-opt -O3 target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.o3.wasm

# Validate every Binaryen comparison artifact. Prefer a linked WABT executable,
# but support an unlinked Apple Silicon Homebrew installation when WABT and
# Binaryen expose conflicting tool names.
wasm-opt-validate: wasm-opt-build
    validator="$(command -v wasm-validate || true)"; \
    if [ -z "$validator" ] && [ -x /opt/homebrew/opt/wabt/bin/wasm-validate ]; then \
      validator=/opt/homebrew/opt/wabt/bin/wasm-validate; \
    fi; \
    if [ -z "$validator" ]; then \
      echo "wasm-validate not found; install WABT or expose wasm-validate in PATH"; \
      exit 2; \
    fi; \
    for file in \
      target/wasm-opt/cribra_bg.base.wasm \
      target/wasm-opt/cribra_bg.os.wasm \
      target/wasm-opt/cribra_bg.oz.wasm \
      target/wasm-opt/cribra_bg.o3.wasm; do \
        "$validator" "$file"; \
    done

# Print raw Binaryen comparison sizes.
wasm-opt-size: wasm-opt-build
    wc -c \
      target/wasm-opt/cribra_bg.base.wasm \
      target/wasm-opt/cribra_bg.os.wasm \
      target/wasm-opt/cribra_bg.oz.wasm \
      target/wasm-opt/cribra_bg.o3.wasm

# Produce transfer-size evidence for the comparison matrix.
wasm-opt-compress: wasm-opt-build
    gzip -9 -c target/wasm-opt/cribra_bg.base.wasm > target/wasm-opt/cribra_bg.base.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.os.wasm > target/wasm-opt/cribra_bg.os.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.oz.wasm > target/wasm-opt/cribra_bg.oz.wasm.gz
    gzip -9 -c target/wasm-opt/cribra_bg.o3.wasm > target/wasm-opt/cribra_bg.o3.wasm.gz
    brotli -f -q 11 target/wasm-opt/cribra_bg.base.wasm -o target/wasm-opt/cribra_bg.base.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.os.wasm -o target/wasm-opt/cribra_bg.os.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.oz.wasm -o target/wasm-opt/cribra_bg.oz.wasm.br
    brotli -f -q 11 target/wasm-opt/cribra_bg.o3.wasm -o target/wasm-opt/cribra_bg.o3.wasm.br
    wc -c target/wasm-opt/*.wasm.gz
    wc -c target/wasm-opt/*.wasm.br

# Reproduce the complete Binaryen validation/size comparison surface.
wasm-opt-prepare: wasm-opt-build wasm-opt-validate wasm-opt-size wasm-opt-compress

# Build the single production WASM profile selected by the v0.4.2 browser
# benchmark gate: Binaryen -Oz.
wasm-production: wasm-adapter
    mkdir -p target/wasm-production
    cp target/wasm/cribra.js target/wasm-production/cribra.js
    cp target/wasm/cribra.d.ts target/wasm-production/cribra.d.ts
    cp target/wasm/cribra_bg.wasm.d.ts target/wasm-production/cribra_bg.wasm.d.ts
    wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-production/cribra_bg.wasm
    test -s target/wasm-production/cribra.js
    test -s target/wasm-production/cribra.d.ts
    test -s target/wasm-production/cribra_bg.wasm
    test -s target/wasm-production/cribra_bg.wasm.d.ts

# Prepare isolated base/-Os/-Oz/-O3 directories for the real-browser
# regression benchmark harness.
wasm-bench-prepare: wasm-opt-build
    rm -rf target/wasm-bench
    mkdir -p \
      target/wasm-bench/base \
      target/wasm-bench/os \
      target/wasm-bench/oz \
      target/wasm-bench/o3
    for variant in base os oz o3; do \
      cp target/wasm/cribra.js "target/wasm-bench/$variant/cribra.js"; \
    done
    cp target/wasm-opt/cribra_bg.base.wasm target/wasm-bench/base/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.os.wasm target/wasm-bench/os/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.oz.wasm target/wasm-bench/oz/cribra_bg.wasm
    cp target/wasm-opt/cribra_bg.o3.wasm target/wasm-bench/o3/cribra_bg.wasm

# Serve the browser benchmark harness with dependency-free Node.js.
wasm-bench-serve:
    node crates/cribra-wasm/benches/web/serve.mjs

# Remove Binaryen comparison, production, and browser-benchmark artifacts only.
wasm-opt-clean:
    rm -rf target/wasm-opt target/wasm-production target/wasm-bench

# Remove all generated reusable WASM artifacts while preserving unrelated Cargo
# outputs.
wasm-clean:
    rm -rf target/wasm target/wasm-opt target/wasm-production target/wasm-bench

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
