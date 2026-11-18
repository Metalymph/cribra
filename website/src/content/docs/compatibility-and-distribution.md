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
| CLI `cribra-cli` | 0.4.4 | Source build and `cargo install --path crates/cribra-cli` from a checkout; crates.io returned 404 at this audit, so there is no `cargo install cribra-cli` registry path. Core publication does not establish CLI publication. |
| MCP `cribra-mcp` | 0.1.1; five native targets; stdio only | Published [`cribra-mcp-v0.1.1`](https://github.com/Metalymph/cribra/releases/tag/cribra-mcp-v0.1.1) with five archives and one `SHA256SUMS` manifest. M3 conformance **PARTIAL PASS**; M4 distribution **PASS**. Tool integration, not a language binding. |
| C ABI `cribra-capi` | 0.3.0 | Source-built native libraries and checked-in header; `publish = false`. |
| WASM `cribra-wasm` | 0.4.7 | Published 0.4.7 uses Core `^0.6`; workspace source uses Core `^0.7`. |
| Shared UniFFI adapter | 0.1.0 | Generator/adapter, `publish = false`. |
| Swift | Published Fuse SwiftPM package `cribra-swift` 0.7.2; Core/Flow source builds | Remote package and checksum-verified XCFramework asset published; clean-room macOS ARM64 consumer passed; iOS Simulator compile/link only; listed on the Swift Package Index for discovery; the listing is not Cribra compatibility-build evidence. |
| Python | Fuse wheel `cribra` 0.7.2+fuse | Local/CI wheels; no PyPI publication established. |
| Kotlin/JVM | Core 0.7.2 profile JARs/ZIPs | Local/CI artifacts; no Maven publication established. |
| TypeScript `@metalymph/cribra` | 0.1.0; Flow WebAssembly package; Node `>=22` declaration | **Not published.** Approved identity (`cribraRelease.state` `approved`); no registry record. Local source or packed-tarball installation only until a separately authorized release. |
| React Native `@metalymph/cribra-react-native` | 0.0.0; native New Architecture TurboModule; C ABI 0.3.0 over Core 0.7.2 | **Private and unpublished.** No registry record or npm installation instruction. Local source checkout with a `file:` dependency only. |

## Registry audit — 2026-11-17

The official registries were queried independently of GitHub. A registry `200`
is publication evidence; a `404` means no installation path exists yet. Neither
result is implementation or qualification evidence.

| Registry / package | Observed result |
| --- | --- |
| crates.io `cribra` | `200`; latest `0.7.2` |
| crates.io `cribra-wasm` | `200`; latest `0.4.7` |
| crates.io `cribra-cli` | `404`; crate does not exist |
| crates.io `cribra-capi` / `cribra-uniffi` | `404`; crates do not exist |
| npm `@metalymph/cribra` | `404 Not found` |
| npm `@metalymph/cribra-react-native` | `404 Not found` |
| GitHub `cribra-mcp-v0.1.1` | Published release; five archives plus `SHA256SUMS` |

The MCP server is distributed as a GitHub release, not as a Cargo or npm
package. Install the CLI with `cargo install --path crates/cribra-cli` from a
checkout; `cargo install cribra-cli` is not a registry path.

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

## React Native package status

The native React Native package is implemented and qualified through RN.1.6 as
`@metalymph/cribra-react-native@0.0.0`. It is **private and unpublished**: there
is no npm registry record, no public installation instruction and no publication
claim. It is a direct C ABI TurboModule over `cribra-capi` 0.3.0 and Core 0.7.2,
not a mode of the WebAssembly package and not a UniFFI generator experiment.

RN.1.6, recorded on 2026-11-12, verified 10 Jest suites and 281 tests, 15,736
host checks with 0 failures, host plain/ASan/UBSan/TSan runs, iOS Simulator
Debug and Release runtime, Android ARM64 emulator Debug and Release runtime,
Android four-ABI Debug and Release builds, and three full JS reload cycles per
platform. Physical devices, iOS device builds and macOS ASan leak detection are
not tested. Only the recorded RN 0.88.1 / React 19.3.0 / New Architecture /
Hermes consumer was qualified. See the [React Native integration
guide](/docs/integration-react-native/).

## Planned and demand-driven work

The Fuse-first Swift distribution is complete: `cribra-swift` 0.7.2 is published
and remotely qualified, and it is listed on the [Swift Package
Index](https://swiftpackageindex.com/Metalymph/cribra-swift) for discovery. The
listing is not Cribra compatibility-build evidence for additional platforms.
The approved `@metalymph/cribra@0.1.0` npm release remains an outstanding,
separately authorized release gate; the package is implemented and qualified but
unpublished. The next priorities are dependency and CI maintenance, evaluation of
Python and Kotlin/JVM documentation and official package distribution, and
evaluation of further bindings based on ecosystem demand. The TypeScript
WebAssembly package is implemented but unpublished. The React Native package is
implemented and qualified but private and unpublished. The native Node.js
backend is not implemented and remains separately gated; it is not the React
Native package. The published `cribra-mcp` 0.1.1 server is a tool integration,
not a language binding.

C#, Go and PHP are planned as **demand-driven / community contribution**
candidates. Ruby, Elixir and other candidates retain the same status.
Doxygen-generated C API documentation is likewise demand-driven and unscheduled.
Official GitHub/GitLab scanner integrations, Homebrew, Debian/Ubuntu APT and a
dedicated embedded Linux distribution workstream are not on the active roadmap.
Existing Linux support and public interoperability contracts remain in place.
These candidates are not actively scheduled and are not implemented, qualified or
published. No delivery dates are promised; the project will consider them only
with demonstrated real-world demand or a viable community contribution. See the
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
- [MCP integration](/docs/integration-mcp/)
- [TypeScript quick start](/docs/typescript/)
- [TypeScript runtime compatibility](/docs/typescript-runtime-compatibility/)
- [React Native integration](/docs/integration-react-native/)
- [Security model](/docs/security-model/)
- [Troubleshooting](/docs/troubleshooting/)
