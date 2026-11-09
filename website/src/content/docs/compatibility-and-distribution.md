---
title: Compatibility & Distribution
description: Feature profiles, source-versus-release boundaries, and qualified platforms.
order: 26
---

# Compatibility & Distribution

**Implementation, local qualification, CI qualification and publication are distinct claims.** A passing build, local test or temporary CI artifact does not establish a public package or supported runtime. The canonical status record is [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md) (historical evidence from 2026-10-20 onward, supplemented by local six-integration qualification on 2026-10-25 and Swift Fuse 0.7.2 publication with remote qualification on 2026-10-29).

## Components and distribution

| Surface | Recorded source version | Distribution boundary |
| --- | --- | --- |
| Rust `cribra` | 0.7.2; MSRV Rust 1.97 | Core 0.7.2 published on crates.io. |
| CLI `cribra-cli` | 0.4.4 | Source build; Core publication does not establish CLI publication. |
| C ABI `cribra-capi` | 0.3.0 | Source-built native libraries and checked-in header; `publish = false`. |
| WASM `cribra-wasm` | 0.4.7 | Published 0.4.7 uses Core `^0.6`; workspace source uses Core `^0.7`. |
| Shared UniFFI adapter | 0.1.0 | Generator/adapter, `publish = false`. |
| Swift | Published Fuse SwiftPM package `cribra-swift` 0.7.2; Core/Flow source builds | Remote package and checksum-verified XCFramework asset published; clean-room macOS ARM64 consumer passed; iOS Simulator compile/link only; listed on the Swift Package Index for discovery; the listing is not Cribra compatibility-build evidence. |
| Python | Fuse wheel `cribra` 0.7.2+fuse | Local/CI wheels; no PyPI publication established. |
| Kotlin/JVM | Core 0.7.2 profile JARs/ZIPs | Local/CI artifacts; no Maven publication established. |
| TypeScript `@metalymph/cribra` | 0.1.0; Flow WebAssembly package; Node `>=22` declaration | **Not published.** Approved identity (`cribraRelease.state` `approved`); no registry record. Local source or packed-tarball installation only until a separately authorized release. |

## Feature boundaries

Rust **Core/Flow/Fuse** describe feature combinations, **not Cargo features** named `core`, `flow` or `fuse`. Core can be selected with `default-features = false`; default Rust `transform` enables Flow operations; Fuse additionally combines `serde` and `parallel`. UniFFI has explicit `core`, `flow`, `fuse` profiles. WASM supports Core and Flow, **not Fuse**.

Workspace WASM source adds streaming, incremental, batches and Flow Share Bundles, but these source checks **do not prove those APIs exist in the published 0.4.7 package**. Check the installed artifact and matching dependency versions, not just current source documentation.

## Local qualification update — 2026-10-25

The current checkout passed additional Rust public API and external Cargo
consumer checks; C ABI 0.3.0 header, symbol and independent C11 consumer checks;
Swift Core/Flow/Fuse independent SwiftPM consumer gates; Kotlin/JVM Core/Flow/Fuse
isolated bundle consumers; and installed Python Fuse wheel tests on macOS ARM64
with CPython 3.11–3.14.

WASM Core/Flow passed generated JavaScript/TypeScript runtime qualification
against direct Rust Core oracles. The local production package passed the
Binaryen `-Oz` release gate and package validation.

These results concern the exercised contracts and local artifacts. They do not
establish complete API parity, additional published packages, browser session
qualification or universal operating-system support.

## TypeScript package status

TypeScript is implemented against the WebAssembly backend as
`@metalymph/cribra@0.1.0`. The package identity is approved but the package is
unpublished, so the name is not yet an available registry installation
instruction. The package has no npm registry record; publication remains a
separately authorized step.

The W3.2C-1 installed-tarball matrix qualified Node 22.22.0, 24.22.2 and 26.13.0
for Flow and the test-only Core artifact; TypeScript 5.9.3 and 7.0.2 with
`NodeNext` and `Bundler`; Google Chrome session/task coverage, Playwright
Chromium, Playwright Firefox 157.0 and Playwright WebKit 27.2 for Flow; Chrome
for Core; and Vite 8.3.6 plus webpack 5.112.2. Native Safari remains unverified,
and Playwright WebKit is not Safari evidence. The `>=22` engine declaration is
not evidence for untested Node releases. See the [TypeScript quick
start](/docs/typescript/) and [runtime compatibility
guide](/docs/typescript-runtime-compatibility/).

## Planned and demand-driven work

The Fuse-first Swift distribution is complete: `cribra-swift` 0.7.2 is published
and remotely qualified, and it is listed on the [Swift Package
Index](https://swiftpackageindex.com/Metalymph/cribra-swift) for discovery. The
listing is not Cribra compatibility-build evidence for additional platforms.
The adoption-first priorities are the official `@metalymph/cribra@0.1.0` npm
release, native TypeScript / React Native integration, the native Rust MCP server
over stdio, CLI distribution through Homebrew and Debian/Ubuntu APT, and Python
and Kotlin/JVM documentation consolidation. The TypeScript WebAssembly package is
implemented but unpublished; the native backend remains planned and has three
separately gated milestones.

C#, Go and PHP are planned as **demand-driven / community contribution**
candidates. Ruby, Elixir and other candidates retain the same status.
Doxygen-generated C API documentation and GitHub/GitLab integrations are likewise
demand-driven and unscheduled. They are not actively scheduled and are not
implemented, qualified or published. No delivery dates are promised; the project
will consider them only with demonstrated real-world demand or a viable community
contribution. See the
[contribution guidelines](https://github.com/Metalymph/cribra/blob/main/CONTRIBUTING.md)
and the [roadmap priority order](https://github.com/Metalymph/cribra/blob/main/ROADMAP.md#development-priority-order-forward-looking).

## Qualified does not mean universal

Swift's published package is qualified by remote resolution and macOS ARM64
execution; its device/Simulator checks remain compile-and-link evidence, not
runtime execution, and no Intel slices exist. Its Swift Package Index listing is
discovery only and is not Cribra compatibility-build evidence. Kotlin/JVM checks do not establish Android/KMP support. Python and Kotlin native bundles target recorded host/architecture combinations, not all platforms. WASM Node/Bun local checks do not establish browser, worker, CSP or publication qualification.

Never combine generated binding files with a different native library or profile. For C, match the header and binary; for WASM, match the generated JS glue and WASM binary.

**Source of truth:** [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md), [`docs/INTEROP.md`](https://github.com/Metalymph/cribra/blob/main/docs/INTEROP.md), [`docs/WASM_INTEROP.md`](https://github.com/Metalymph/cribra/blob/main/docs/WASM_INTEROP.md), and binding-specific README files. Recheck these records when releasing.

## Related guides

- [Integration guides](/docs/)
- [TypeScript quick start](/docs/typescript/)
- [TypeScript runtime compatibility](/docs/typescript-runtime-compatibility/)
- [Security model](/docs/security-model/)
- [Troubleshooting](/docs/troubleshooting/)
