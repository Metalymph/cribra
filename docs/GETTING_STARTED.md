# Getting started

These entry points use the existing examples and qualified consumers. Run commands
from the repository root unless stated otherwise. Use Rust 1.97 or newer for the
workspace; UniFFI build recipes pin Rust 1.97.0 and UniFFI 0.32.2. Install `just`
for the recipes. Check [Compatibility](COMPATIBILITY.md) for platform evidence,
profile selection and publication status; a build artifact is not a registry package.

## Rust

Core v0.7.2 is independently verified in the crates.io registry index:

```toml
[dependencies]
cribra = "0.7.2"
```

This selects Flow (transformations enabled). For Core use
`cribra = { version = "0.7.2", default-features = false }`; Fuse adds
`features = ["serde", "parallel"]` to the default configuration.
The [root quick start](../README.md#quick-start) uses the public Rust API:

```rust
use cribra::Scanner;

let results = Scanner::default().scan([("settings.toml", "log_level = \"info\"")]);
for (source, finding) in results.findings() {
    println!("{source}: {}", finding.rule_id());
}
```

For a complete source consumer with synthetic financial findings and redaction,
see [financial_review.rs](../examples/financial_review.rs).

## CLI

Build the existing `cribra-cli` workspace binary (registry publication is not
assumed), then use the documented single-file/stdin interface:

```sh
cargo build -p cribra-cli
# macOS/Linux; use target/debug/cribra.exe on Windows:
target/debug/cribra scan config.env --format json
```

Supply an existing UTF-8 file. The executable is named `cribra`; findings and
review-only results still return exit code 0. See the [CLI guide](../crates/cribra-cli/README.md)
and [capability parity](CLI_PARITY.md). There is no directory traversal.

## C / C++

```sh
cargo build -p cribra-capi
just capi-smoke-all
```

The smoke recipe requires `cbindgen` and a C compiler on macOS/Linux. Follow the
[C](../examples/c/README.md) or [C++](../examples/cpp/README.md) guide for the
exact platform compiler/linker commands for their standalone examples. C++ uses
the same C ABI with local RAII wrappers. The [header](../include/cribra.h)
defines ownership; borrowed finding views must not outlive their report.
Windows ABI qualification uses the CI commands, not the POSIX local smoke recipe.

## WebAssembly / JavaScript

Use the Rust `wasm32-unknown-unknown` target, a `wasm-bindgen-cli` matching
`Cargo.lock`, and Binaryen `wasm-opt` on PATH:

```sh
just wasm-production
node examples/wasm-runtime/scan.mjs
```

This builds the default Flow artifact under `target/wasm-production/`.
The [runtime guide](../examples/wasm-runtime/README.md) includes Bun and Deno;
the [browser guide](../examples/wasm-browser/README.md) serves the same artifact
locally. For explicit Core/Flow contract checks with Node.js 22+:

```sh
node crates/cribra-wasm/tests/runtime/qualify.mjs --install-bindgen
```

See the [WASM adapter guide](../crates/cribra-wasm/README.md) for feature selection.
There is no WASM Fuse profile; Core/Flow expose sequential batch scanning,
admitted byte streaming and incremental document interfaces, and Flow adds
Core-backed in-memory Share Bundles.

## Swift

On native macOS ARM64 with Swift 6.4, Xcode's macOS/iOS/Simulator SDKs, Rust
1.97.0 and the three Apple Rust targets installed as in the [Swift guide](../bindings/swift/README.md):

```sh
just swift-apple core
just swift-apple-consumer core
```

This builds an XCFramework and qualifies an external local Swift Package
consumer. Use `flow` or `fuse` in both commands to select that profile. Integrate
exactly one generated local package and `import Cribra`; the Swift guide provides
`Package.swift`, Scanner and atomic incremental snapshot examples. The iOS
Simulator consumer is compiled/linked, not launched. Device execution is not
established by building the device slice.

## Python

With Python 3.13+, Rust 1.97.0 and `just`:

```sh
just uniffi-python-smoke
just uniffi-python-flow-smoke
just uniffi-python-fuse-smoke
```

The runner generates and loads real matching bindings/native libraries under
`target/uniffi-python-consumer/<profile>/`. The public generated module is
`cribra_uniffi`; there is no hand-written Python facade. For installation outside
the checkout, follow the [Python guide's Fuse wheel build and offline install](../bindings/python/README.md).
It documents the pinned build environment, exact wheel paths and installed
consumer commands for each qualified platform. No PyPI installation is implied.

## Kotlin/JVM

With Rust 1.97.0, JDK 17, Python 3.11+, Kotlin compiler 2.4.21 and `just`:

```sh
just uniffi-kotlin-bundles
just uniffi-kotlin-bundle-consumer
```

The first command builds/verifies all three host ZIPs; the second compiles and
runs isolated consumers of those ZIPs. First builds require network access for
the pinned Gradle wrapper and dependencies. For a native Windows shell use the
same cross-platform drivers directly:

```sh
python bindings/kotlin/build_bundle.py
python bindings/kotlin/qualify_bundle_consumer.py
```

Each classpath must contain exactly one profile JAR plus JNA and the Kotlin
standard library, with its matching native library loadable by JNA. See the
[Kotlin guide](../bindings/kotlin/README.md) for dependency setup, startup profile
guards, classpaths and expected negative-test rejections. Generated APIs use
`uniffi.cribra_uniffi`; no Maven Central package is published by these commands.

Across all adapters, offsets are half-open UTF-8 byte ranges, while line and
column are one-based (columns count Unicode scalars). Findings and review
candidates remain distinct; candidates are not automatically transformed.
