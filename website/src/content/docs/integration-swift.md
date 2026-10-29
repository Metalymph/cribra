---
title: 'Swift integration'
description: 'Plan the official Fuse SwiftPM package and build Core or Flow from source.'
order: 11
---

# Swift integration

> **Availability boundary:** The official remote SwiftPM package is planned, not
> published. The current repository builds local packages; the commands below
> remain source-build instructions. Do not add a remote dependency or claim
> Swift Package Index registration until the package is released and qualified.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Official installation: Fuse (planned)

The official Swift distribution target is one SwiftPM package using the **Fuse**
profile. It will expose the product and module **`Cribra`** and the complete
supported Cribra binding API: scanning, configuration, batch operations, admitted
streaming, incremental snapshots, the four transformations, Share Bundles and
parallel scanning.

**This package is not published.** The provisional distribution repository is
`Metalymph/cribra-swift`, the initial package version is `0.7.2`, and the planned
native asset is `CribraFFI-fuse-0.7.2.xcframework.zip`. The repository, URL, tag,
asset and SwiftPM checksum do not exist yet.

When released, consumers will add the remote Git URL as a SwiftPM dependency,
pin an exact published semantic version, and link the `Cribra` product. The
planned form is:

```swift
// Planned form only. This dependency does not resolve until the package is published.
.package(url: "<future cribra-swift repository URL>", exact: "0.7.2")
```

```swift
import Cribra
```

The remote package will not require Rust, Cargo, UniFFI generators, generated
Xcode projects or local Cribra source files. The Swift source and native binary
will come from the same qualified Fuse build, and the XCFramework will be
downloaded from an immutable release asset.

The published [Swift Core API reference](/api/0.7.2/swift/core/documentation/cribra/)
documents the generated **Core** profile for Cribra 0.7.2. It does not cover the
planned Fuse package, and no Fuse DocC reference is published.

## Alternative build profiles

Core and Flow remain supported source-build alternatives. They are alternative
builds of the same engine, not add-on modules intended to coexist with the Fuse
package. Use exactly one profile per application or process.

| Profile | Distribution                       | Purpose                     |
| ------- | ---------------------------------- | --------------------------- |
| Fuse    | Official SwiftPM package (planned) | Complete supported binding  |
| Flow    | Build from source                  | Core plus Flow capabilities |
| Core    | Build from source                  | Core capabilities           |

The profile roles and distribution policy are recorded in
[Ecosystem and scope](https://github.com/Metalymph/cribra/blob/main/docs/ECOSYSTEM.md#distribution-policy).
The detailed Swift packaging contract is in the
[Swift remote distribution contract](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#remote-swiftpm-distribution-contract-fuse).

## Build Core or Flow from source

On native macOS ARM64, with Xcode's macOS/iOS/Simulator SDKs, Swift 6.4, Rust
1.97.0 and `just`, follow the
[official Swift build instructions](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#build):

```sh
rustup target add --toolchain 1.97.0 \
  aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim

just swift-apple core
just swift-apple-consumer core
# Or replace core with flow in both commands.
```

The profile directories and their exact capability boundaries are documented in
[layout and profile selection](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#layout-and-profile-selection).
Fuse can also be built from source with `just swift-apple fuse` for development
and pre-release qualification; that local build is not the remote package.

After generation, add the chosen directory as a **local** Swift package in Xcode
or the consuming `Package.swift`. For example, a Core consumer uses
`.package(path: "/path/to/cribra/bindings/swift")` and links product `Cribra`.
This is not remote installation.

```swift
import Cribra
let scanner = Cribra.Scanner()
let results = scanner.scanMany(sources: [
    ScanSource(id: SourceId(value: "example"), text: "example input")
])
```

The generated interface is authoritative for exact types and error signatures.
Flow and Fuse source consumers use the same `import Cribra`; they must not mix
packages, generated bindings or native libraries from different profiles.

## Qualification limits

The remote package has not been published or qualified through a clean-room
consumer. The repository now includes an unpublished local Fuse distribution
staging and qualification workflow (W3.1C), but it does not create a remote
package, release asset, published checksum or Swift Package Index entry. Current
evidence covers local macOS ARM64 external consumers and
compiled/linked iOS Simulator ARM64 consumers. **Simulator compile/link is not
Simulator execution or physical-device runtime qualification**. Declared
minimums macOS 13 / iOS 16 are not runtime coverage of every OS release. Intel
slices, Catalyst, tvOS, watchOS and visionOS are not supported.

Swift Package Index registration is not established. Do not assume Linux
compatibility for this Apple-only binary package.

Do not convert Swift string/UTF-16 offsets into incremental edit positions
without explicit UTF-8 byte conversion. Keep the complete matching generated
package and native library together; transformation keys are caller-owned.

## Source references

- [`bindings/swift/README.md`](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md)
- [Swift remote distribution contract](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#remote-swiftpm-distribution-contract-fuse)
- [`crates/cribra-uniffi/README.md`](https://github.com/Metalymph/cribra/blob/main/crates/cribra-uniffi/README.md)
- [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md)
- [Compatibility and distribution](/docs/compatibility-and-distribution/)
