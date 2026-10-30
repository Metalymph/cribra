---
title: 'Swift integration'
description: 'Install the published Fuse SwiftPM package or build Core and Flow from source.'
order: 11
---

# Swift integration

> **Availability:** the official Fuse SwiftPM package **0.7.2** is published at
> [`Metalymph/cribra-swift`](https://github.com/Metalymph/cribra-swift) and has
> passed a clean-room remote consumer gate. Swift Package Index registration has
> not been submitted.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Official installation: Fuse (published)

The official Swift distribution is one SwiftPM package using the **Fuse**
profile. It exposes the product and module **`Cribra`** and the complete
supported Cribra binding API: scanning, configuration, batch operations, admitted
streaming, incremental snapshots, the four transformations, Share Bundles and
parallel scanning.

Published package: [`Metalymph/cribra-swift`](https://github.com/Metalymph/cribra-swift).
Release: [`0.7.2`](https://github.com/Metalymph/cribra-swift/releases/tag/0.7.2).

Add the remote package to `Package.swift`:

```swift
dependencies: [
    .package(
        url: "https://github.com/Metalymph/cribra-swift",
        exact: "0.7.2"
    )
]
```

Link the `Cribra` product from the application target:

```swift
.product(
    name: "Cribra",
    package: "cribra-swift"
)
```

Import the module:

```swift
import Cribra
```

Minimal use of the generated Fuse API:

```swift
import Cribra

let scanner = Cribra.Scanner()
let results = scanner.scanMany(sources: [
    ScanSource(id: SourceId(value: "config/.env"), text: "API_KEY=SECRET")
])

for report in results.reports {
    for finding in report.findings {
        print("\(report.sourceId.value): \(finding.ruleId.value) at byte \(finding.location.start)")
    }
}
```

`Cribra.Scanner()` uses the default built-in portfolio. Use
`Cribra.Scanner.configured(config:)` for explicit built-in or custom rule
selection. SwiftPM downloads `CribraFFI-fuse-0.7.2.xcframework.zip` from the
release and verifies the manifest checksum automatically.

The remote package does not require Rust, Cargo, UniFFI generators, generated
Xcode projects or local Cribra source files. The Swift source and native binary
come from the same qualified Fuse build, and the XCFramework is downloaded from
the immutable `0.7.2` release asset.

The published [Swift Core API reference](/api/0.7.2/swift/core/documentation/cribra/)
documents the generated **Core** profile for Cribra 0.7.2. It does not cover the
published Fuse package, and no Fuse DocC reference is published.

## Alternative build profiles

Core and Flow remain supported source-build alternatives. They are alternative
builds of the same engine, not add-on modules intended to coexist with the Fuse
package. Use exactly one profile per application or process.

| Profile | Distribution                        | Purpose                     |
| ------- | ----------------------------------- | --------------------------- |
| Fuse    | Official SwiftPM package (published) | Complete supported binding  |
| Flow    | Build from source                   | Core plus Flow capabilities |
| Core    | Build from source                   | Core capabilities           |

The profile roles and distribution policy are recorded in
[Ecosystem and scope](https://github.com/Metalymph/cribra/blob/main/docs/ECOSYSTEM.md#distribution-policy).
The detailed Swift packaging contract is in the
[Swift remote distribution contract](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#remote-swiftpm-distribution-contract-fuse)
and the authoritative consumer instructions are in
[Install Fuse from SwiftPM](https://github.com/Metalymph/cribra/blob/main/bindings/swift/README.md#install-fuse-from-swiftpm).

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
and pre-release qualification; that local build is not the published package.

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

Remote SwiftPM resolution for `0.7.2` passed: the exact version resolved from the
remote URL, the XCFramework was downloaded and checksum-validated, an external
macOS ARM64 consumer built and executed, and the same consumer compiled and
linked for `arm64-apple-ios16.0-simulator`.

**iOS Simulator compile/link is not Simulator execution or physical-device
runtime qualification**. Declared minimums macOS 13 / iOS 16 are not runtime
coverage of every OS release. Intel slices, Catalyst, tvOS, watchOS and visionOS
are not supported. No signing or notarization qualification was performed.

Swift Package Index registration has not been submitted. Do not assume Linux
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
