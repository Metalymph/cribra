---
title: Compatibility & Distribution
description: Feature profiles, source-versus-release boundaries, and qualified platforms.
order: 22
---

# Compatibility & Distribution

**Implementation, local qualification, CI qualification and publication are distinct claims.** A passing build, local test or temporary CI artifact does not establish a public package or supported runtime. The canonical status record is `docs/COMPATIBILITY.md` (evidence snapshot dated 2026-10-20, with additional local source checks on 2026-10-22).

## Components and distribution

| Surface | Recorded source version | Distribution boundary |
| --- | --- | --- |
| Rust `cribra` | 0.7.2; MSRV Rust 1.97 | Core 0.7.2 published on crates.io. |
| CLI `cribra-cli` | 0.4.4 | Source build; Core publication does not establish CLI publication. |
| C ABI `cribra-capi` | 0.3.0 | Source-built native libraries and checked-in header; `publish = false`. |
| WASM `cribra-wasm` | 0.4.7 | Published 0.4.7 uses Core `^0.6`; workspace source uses Core `^0.7`. |
| Shared UniFFI adapter | 0.1.0 | Generator/adapter, `publish = false`. |
| Swift | Generated Core/Flow/Fuse packages | Local SwiftPM/XCFramework artifacts; no remote SwiftPM publication established. |
| Python | Fuse wheel `cribra` 0.7.2+fuse | Local/CI wheels; no PyPI publication established. |
| Kotlin/JVM | Core 0.7.2 profile JARs/ZIPs | Local/CI artifacts; no Maven publication established. |

## Feature boundaries

Rust **Core/Flow/Fuse** describe feature combinations, **not Cargo features** named `core`, `flow` or `fuse`. Core can be selected with `default-features = false`; default Rust `transform` enables Flow operations; Fuse additionally combines `serde` and `parallel`. UniFFI has explicit `core`, `flow`, `fuse` profiles. WASM supports Core and Flow, **not Fuse**.

Workspace WASM source adds streaming, incremental, batches and Flow Share Bundles, but these source checks **do not prove those APIs exist in the published 0.4.7 package**. Check the installed artifact and matching dependency versions, not just current source documentation.

## Qualified does not mean universal

Swift device/Simulator compile-and-link checks are not execution checks for every Apple target; no Intel slices or remote package distribution are established. Kotlin/JVM checks do not establish Android/KMP support. Python and Kotlin native bundles target recorded host/architecture combinations, not all platforms. WASM Node/Bun local checks do not establish browser, worker, CSP or publication qualification.

Never combine generated binding files with a different native library or profile. For C, match the header and binary; for WASM, match the generated JS glue and WASM binary.

**Source of truth:** `docs/COMPATIBILITY.md`, `docs/INTEROP.md`, `docs/WASM_INTEROP.md`, and binding-specific README files. Recheck these records when releasing.

## Related guides

- [Integration guides](/docs/)
- [Security model](/docs/security-model/)
- [Troubleshooting](/docs/troubleshooting/)
