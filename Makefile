# Cribra contributor/development commands.
# `just` is preferred locally; this Makefile mirrors the common recipes.

.PHONY: help fmt fmt-check check check-all capi capi-header capi-header-check \
	capi-smoke capi-static capi-smoke-all test test-serde test-all test-doc \
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
	  '  make capi-static       Verify native static library artifact' \
	  '  make capi-smoke-all    Run the complete local C ABI smoke surface' \
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

capi:
	cargo check -p cribra-capi
	cargo clippy -p cribra-capi --all-targets -- -D warnings
	cargo build -p cribra-capi

capi-header:
	mkdir -p include
	cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output include/cribra.h

capi-header-check:
	mkdir -p target/c-smoke
	cp include/cribra.h target/c-smoke/cribra.h.expected
	cbindgen --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output target/c-smoke/cribra.h.generated
	diff -u target/c-smoke/cribra.h.expected target/c-smoke/cribra.h.generated

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

capi-static:
	cargo build -p cribra-capi
	test -f target/debug/libcribra_capi.a

capi-smoke-all: capi-header-check capi-smoke capi-static

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

wasm:
	cargo check --target wasm32-unknown-unknown --no-default-features

wasm-serde:
	cargo check --target wasm32-unknown-unknown --no-default-features --features serde

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
