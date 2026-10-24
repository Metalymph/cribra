---
title: "Swift integration"
description: "Use locally generated SwiftPM packages with matching UniFFI native artifacts."
order: 11
---

# Swift integration

> **Source and distribution boundary:** This guide describes the repository
> contracts recorded in the W2.2 audit. Source implementation, local tests,
> CI qualification, and publication are distinct claims. Consult the linked
> adapter README and compatibility matrix before shipping binaries.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Distribution and profiles

The public Swift product and module are **`Cribra`**. These are **local**
SwiftPM packages, not a verified remotely published SwiftPM dependency.
Generated sources, static archives and XCFrameworks are build outputs.

| Profile | Local package directory | Operations |
| --- | --- | --- |
| Core | `bindings/swift` | Scan, configure, stream, incremental, snapshot |
| Flow | `bindings/swift/Profiles/Flow` | Core plus transformations and Share Bundles |
| Fuse | `bindings/swift/Profiles/Fuse` | Flow plus parallel scan |

Choose exactly **one** profile per application/process. All packages expose
the same module and native symbols; mixing them is unsupported.

## Build and consume

On native macOS ARM64, with Xcode/SDKs, Swift 6.4, Rust 1.97 and `just`:

```sh
rustup target add --toolchain 1.97.0 aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim
just swift-apple core
# Or select flow / fuse instead of core.
```

After generation, add the chosen directory as a **local** Swift package in
Xcode or the consuming `Package.swift`. For example, a Core consumer uses
`.package(path: "/path/to/cribra/bindings/swift")` and links product `Cribra`.

```swift
import Cribra
let scanner = Cribra.Scanner()
let results = scanner.scanMany(sources: [
    ScanSource(id: SourceId(value: "example"), text: "example input")
])
```

The generated interface is authoritative for exact types and error signatures.

## Qualification limits

macOS ARM64 external consumers have been run. iOS ARM64 device and Simulator
archives were built; **Simulator compile/link is not Simulator execution or
physical-device runtime qualification**. Declared minimums macOS 13 / iOS 16
are not runtime coverage of every OS release. Intel slices are absent.

Do not convert Swift string/UTF-16 offsets into incremental edit positions
without explicit UTF-8 byte conversion. Keep the complete matching generated
package and native library together; transformation keys are caller-owned.

## Source references

- `bindings/swift/README.md`
- `crates/cribra-uniffi/README.md`
- `docs/COMPATIBILITY.md`
