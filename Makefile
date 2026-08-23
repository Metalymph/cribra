# Cribra contributor/development commands.
# `just` is preferred locally; this Makefile mirrors the common recipes.

.PHONY: help fmt fmt-check check check-all capi capi-bench capi-header capi-header-check \
	capi-smoke capi-symbols capi-static capi-smoke-all wasm-adapter-check \
	wasm-adapter-build wasm-adapter-bindgen wasm-adapter wasm-opt-build \
	wasm-opt-validate wasm-opt-size wasm-opt-compress wasm-opt-prepare wasm-production \
	wasm-bench-prepare wasm-bench-serve wasm-opt-clean wasm-clean test test-serde \
	test-all test-doc clippy doc doc-all wasm wasm-serde msrv audit package \
	package-dirty publish-dry-run publish-dry-run-dirty gate release-gate bench \
	bench-v02 bench-diagnostics bench-all clean
help:
	@printf '%s\n' \
	  'Common Cribra targets:' \
	  '  make gate                 Local quality gate' \
	  '  make release-gate         Full release gate (requires cargo-audit)' \
	  '  make capi                 Check/lint/build native C adapter' \
	  '  make capi-smoke-all       Run complete local C ABI smoke surface' \
	  '  make wasm                 Check default WASM core contract' \
	  '  make wasm-serde           Check WASM + Serde core contract' \
	  '  make wasm-adapter         Build reusable browser JS/WASM adapter' \
	  '  make wasm-opt-prepare     Build/validate/measure base, -Os, -Oz, -O3' \
	  '  make wasm-production      Build the production Binaryen -Oz artifact' \
	  '  make wasm-parity          Compare production WASM adapter against Rust oracle' \
	  '  make wasm-parity-oracle   Generate the Rust-native semantic oracle for the WASM parity gate' \
	  '  make wasm-parity-prepare  Prepare the production artifact and Rust semantic oracle' \
	  '  make wasm-bench-prepare   Prepare browser benchmark variant directories' \
	  '  make wasm-bench-serve     Serve the real-browser benchmark with Node.js' \
	  '  make wasm-opt-clean       Remove generated optimization/benchmark outputs' \
	  '  make wasm-clean           Remove all generated WASM adapter outputs' \
	  '  make test-all             Test all native core features' \
	  '  make msrv                 Check/test Rust 1.97.0' \
	  '  make audit                Run cargo audit' \
	  '  make package              Verify crate package' \
	  '  make publish-dry-run      crates.io dry run' \
	  '  make bench-all            Run maintained native benchmarks' \
	  '  make clean                Remove Cargo build artifacts'

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

.PHONY: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen wasm-adapter wasm-parity-oracle wasm-parity-prepare wasm-parity

wasm-adapter-check:
	cargo check -p cribra-wasm --target wasm32-unknown-unknown
	cargo clippy -p cribra-wasm --target wasm32-unknown-unknown --all-targets -- -D warnings

wasm-adapter-build:
	cargo build -p cribra-wasm --target wasm32-unknown-unknown --release

wasm-adapter-bindgen:
	mkdir -p target/wasm
	wasm-bindgen \
	  --target web \
	  --out-dir target/wasm \
	  --out-name cribra \
	  target/wasm32-unknown-unknown/release/cribra_wasm.wasm

wasm-adapter: wasm-adapter-check wasm-adapter-build wasm-adapter-bindgen
	test -s target/wasm/cribra.js
	test -s target/wasm/cribra.d.ts
	test -s target/wasm/cribra_bg.wasm
	test -s target/wasm/cribra_bg.wasm.d.ts

wasm-opt-build: wasm-adapter
	mkdir -p target/wasm-opt
	cp target/wasm/cribra_bg.wasm target/wasm-opt/cribra_bg.base.wasm
	wasm-opt -Os target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.os.wasm
	wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.oz.wasm
	wasm-opt -O3 target/wasm/cribra_bg.wasm -o target/wasm-opt/cribra_bg.o3.wasm

wasm-opt-validate: wasm-opt-build
	@validator="$$(command -v wasm-validate || true)"; \
	if [ -z "$$validator" ] && [ -x /opt/homebrew/opt/wabt/bin/wasm-validate ]; then \
		validator=/opt/homebrew/opt/wabt/bin/wasm-validate; \
	fi; \
	if [ -z "$$validator" ]; then \
		echo "wasm-validate not found; install WABT or expose wasm-validate in PATH"; \
		exit 2; \
	fi; \
	for file in \
		target/wasm-opt/cribra_bg.base.wasm \
		target/wasm-opt/cribra_bg.os.wasm \
		target/wasm-opt/cribra_bg.oz.wasm \
		target/wasm-opt/cribra_bg.o3.wasm; do \
			"$$validator" "$$file"; \
	done

wasm-opt-size: wasm-opt-build
	wc -c \
		target/wasm-opt/cribra_bg.base.wasm \
		target/wasm-opt/cribra_bg.os.wasm \
		target/wasm-opt/cribra_bg.oz.wasm \
		target/wasm-opt/cribra_bg.o3.wasm

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

wasm-opt-prepare: wasm-opt-build wasm-opt-validate wasm-opt-size wasm-opt-compress

wasm-production: wasm-adapter
	mkdir -p target/wasm-production
	cp target/wasm/cribra.js target/wasm-production/cribra.js
	cp target/wasm/cribra.d.ts target/wasm-production/cribra.d.ts
	cp target/wasm/cribra_bg.wasm.d.ts target/wasm-production/cribra_bg.wasm.d.ts
	wasm-opt -Oz target/wasm/cribra_bg.wasm -o target/wasm-production/cribra_bg.wasm
	printf '%s\n' '{"type":"module"}' > target/wasm-production/package.json
	test -s target/wasm-production/cribra.js
	test -s target/wasm-production/cribra.d.ts
	test -s target/wasm-production/cribra_bg.wasm
	test -s target/wasm-production/cribra_bg.wasm.d.ts

wasm-parity-oracle:
	rm -rf target/wasm-parity
	mkdir -p target/wasm-parity
	cargo run -p cribra-wasm --example parity_oracle

wasm-parity-prepare: wasm-production wasm-parity-oracle
	test -s target/wasm-parity/oracle.json
	test -s target/wasm-production/cribra.js
	test -s target/wasm-production/cribra_bg.wasm

wasm-parity: wasm-parity-prepare
	node crates/cribra-wasm/tests/parity/parity.mjs

wasm-bench-prepare: wasm-opt-build
	rm -rf target/wasm-bench
	mkdir -p target/wasm-bench/base target/wasm-bench/os target/wasm-bench/oz target/wasm-bench/o3
	@for variant in base os oz o3; do \
		cp target/wasm/cribra.js "target/wasm-bench/$$variant/cribra.js"; \
	done
	cp target/wasm-opt/cribra_bg.base.wasm target/wasm-bench/base/cribra_bg.wasm
	cp target/wasm-opt/cribra_bg.os.wasm target/wasm-bench/os/cribra_bg.wasm
	cp target/wasm-opt/cribra_bg.oz.wasm target/wasm-bench/oz/cribra_bg.wasm
	cp target/wasm-opt/cribra_bg.o3.wasm target/wasm-bench/o3/cribra_bg.wasm

wasm-bench-serve:
	node crates/cribra-wasm/benches/web/serve.mjs

wasm-opt-clean:
	rm -rf target/wasm-opt target/wasm-production target/wasm-bench

wasm-clean:
	rm -rf target/wasm target/wasm-opt target/wasm-production target/wasm-bench target/wasm-parity

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
