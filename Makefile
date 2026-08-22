# Cribra contributor/development commands.
# `just` is preferred locally; this Makefile mirrors the common recipes.

.PHONY: help fmt fmt-check check check-all capi capi-header capi-header-check \
	capi-smoke capi-symbols capi-static capi-smoke-all test test-serde test-all test-doc \
	clippy doc doc-all wasm wasm-serde msrv audit package package-dirty \
	publish-dry-run publish-dry-run-dirty gate release-gate bench bench-v02 \
	bench-diagnostics bench-all clean

help:
	@printf '%s\n' \
	  'Common Cribra targets:' \
	  '  make gate              Local quality gate' \
	  '  make release-gate      Full release gate (requires cargo-audit)' \
	  '  make capi              Check/lint/build native C adapter' \
	  '  make capi-header       Generate include/cribra.h with cbindgen' \
	  '  make capi-header-check Verify committed C header is deterministic' \
	  '  make capi-smoke        Build/run the real C ABI smoke consumer' \
	  '  make capi-symbols      Verify generated/native ABI symbol parity' \
	  '  make capi-static       Verify native static library artifact' \
	  '  make capi-smoke-all    Run the complete local C ABI smoke surface' \
	  '  make wasm             Check, build, and bind Cribra WASM' \
	  '  make wasm-adapter       Check and lint the wasm32 adapter' \
	  '  make wasm-adapter-build       Build the release wasm32 adapter' \
	  '  make wasm-adapter-bindgen     Generate web JS/WASM artifacts' \
	  '  make test-all          Test all native core features' \
	  '  make wasm              Check default WASM contract' \
	  '  make wasm-serde        Check WASM + Serde contract' \
	  '  make msrv              Check/test Rust 1.97.0' \
	  '  make audit             Run cargo audit' \
	  '  make package           Verify crate package' \
	  '  make publish-dry-run   crates.io dry run' \
	  '  make bench-all         Run maintained benchmarks' \
	  '  make clean             Remove Cargo build artifacts'

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

check:
	cargo check

check-all:
	cargo check --all-features

# Check that the native C adapter can be statically linked.
capi-static:
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

# Check that the committed native C header matches the generated one.
capi-header-check:
	mkdir -p target/c-smoke
	cp include/cribra.h target/c-smoke/cribra.h.expected
	cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output target/c-smoke/cribra.h.generated
	diff -u target/c-smoke/cribra.h.expected target/c-smoke/cribra.h.generated

# Smoke test the native C adapter.
capi-smoke: capi-header
	cargo build -p cribra-capi
	mkdir -p target/c-smoke
	@if [ "$$(uname -s)" = "Darwin" ]; then \
		clang -std=c11 -Wall -Wextra -Werror -Iinclude \
		  crates/cribra-capi/tests/c/smoke.c \
		  -Ltarget/debug -lcribra_capi \
		  -Wl,-rpath,@loader_path/../debug \
		  -o target/c-smoke/cribra-smoke; \
	elif [ "$$(uname -s)" = "Linux" ]; then \
		cc -std=c11 -Wall -Wextra -Werror -Iinclude \
		  crates/cribra-capi/tests/c/smoke.c \
		  -Ltarget/debug -lcribra_capi \
		  -Wl,-rpath,'$$ORIGIN/../debug' \
		  -o target/c-smoke/cribra-smoke; \
	else \
		echo "capi-smoke local target currently supports macOS and Linux; Windows is covered in CI"; \
		exit 2; \
	fi
	target/c-smoke/cribra-smoke

# Check that the native C adapter exports the expected symbols.
capi-symbols: capi-header
	cargo build -p cribra-capi
	sh crates/cribra-capi/tests/c/check-capi-symbols.sh

# Check that the native C adapter can be statically linked.
capi-static:
	cargo build -p cribra-capi
	test -f target/debug/libcribra_capi.a

capi-smoke-all: capi-header-check capi-smoke capi-symbols capi-static

.PHONY: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen wasm-adapter

wasm-adapter-check:
	cargo check -p cribra-wasm --target wasm32-unknown-unknown
	cargo clippy -p cribra-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings

wasm-adapter-build:
	cargo build -p cribra-wasm --target wasm32-unknown-unknown --release

wasm-adapter-bindgen:
	mkdir -p target/wasm
	wasm-bindgen --target web --out-dir target/wasm --out-name cribra target/wasm32-unknown-unknown/release/cribra_wasm.wasm

wasm-adapter: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen
	test -s target/wasm/cribra.js
	test -s target/wasm/cribra_bg.wasm

test:
	cargo test

test-serde:
	cargo test --features serde

test-all:
	cargo test --all-features

test-doc:
	cargo test --doc

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

doc:
	cargo doc --no-default-features --no-deps

doc-all:
	cargo doc --all-features --no-deps

# Check that the WASM adapter exports the expected symbols.
wasm:
	cargo check --target wasm32-unknown-unknown --no-default-features

# Check that the WASM adapter exports the expected symbols.
wasm-symbols:
	cargo check --target wasm32-unknown-unknown --no-default-features

# Check that the WASM adapter can be used with serde.
wasm-serde:
	cargo check --target wasm32-unknown-unknown --no-default-features --features serde

# Check that the MSRV requirements are met.
msrv:
	cargo +1.97.0 check
	cargo +1.97.0 check --all-features
	cargo +1.97.0 test
	cargo +1.97.0 test --all-features

audit:
	cargo audit

package:
	cargo package

package-dirty:
	cargo package --allow-dirty

publish-dry-run:
	cargo publish --dry-run

publish-dry-run-dirty:
	cargo publish --dry-run --allow-dirty

# Gate the release process.
gate: fmt-check check-all capi test test-serde test-all test-doc clippy doc doc-all wasm wasm-serde
	git diff --check

release-gate: gate msrv audit package publish-dry-run

bench:
	cargo bench --bench scan --all-features

bench-v02:
	cargo bench --bench v02 --all-features

bench-diagnostics:
	cargo bench --bench isolation --all-features
	cargo bench --bench contextual_tail --all-features

bench-all: bench bench-v02 bench-diagnostics

clean:
	cargo clean
