# Cribra contributor/development commands.
# `just` is preferred locally; this Makefile mirrors the common recipes.

.PHONY: help fmt fmt-check check check-all capi test test-serde test-all test-doc \
	clippy doc doc-all wasm wasm-serde msrv audit package package-dirty \
	publish-dry-run publish-dry-run-dirty gate release-gate bench bench-v02 \
	bench-diagnostics bench-all clean

help:
	@printf '%s\n' \
	  'Common Cribra targets:' \
	  '  make gate              Local quality gate' \
	  '  make release-gate      Full release gate (requires cargo-audit)' \
	  '  make capi              Check/lint/build native C adapter' \
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
