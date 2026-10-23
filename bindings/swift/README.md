# Cribra for native Swift apps

See [Getting started](../../docs/GETTING_STARTED.md) and
[Compatibility and distribution status](../../docs/COMPATIBILITY.md) for the
cross-language entry points and evidence boundaries.

Local Swift Package distribution of Cribra Core **0.7.2**, adapter **0.1.0**,
UniFFI **=0.32.2**, Rust **1.97.0** and Swift **6.4**. The public Swift module and
library product are named **`Cribra`**. Deployment minimums are macOS 13 and iOS 16.

## Layout and profile selection

Choose exactly one package dependency per app/process:

| Profile | Package directory | Rust features | Available operations |
| --- | --- | --- | --- |
| Core | `bindings/swift` | `--no-default-features --features core` | Scanner, configuration, stream, incremental, atomic snapshot. |
| Flow | `bindings/swift/Profiles/Flow` | `--no-default-features --features flow` | Core plus four transforms and Share Bundles. |
| Fuse | `bindings/swift/Profiles/Fuse` | `--no-default-features --features fuse` | Flow plus parallel scanning. |

Each directory has a source-controlled `Package.swift` and its own ignored
`Generated/` tree. There is no environment-dependent package selection, and no
Swift define that pretends an unavailable Rust feature exists. Importing more
than one profile would duplicate the `Cribra` module and native symbols and is
unsupported. Build the selected profile before resolving its package.

```
bindings/swift/
  Package.swift                 # Core package, product/module Cribra
  Profiles/{Flow,Fuse}/Package.swift
  uniffi.toml                   # Cribra / CribraFFI generator configuration
  tools/apple.rs                # std-only reproducible Rust build driver
  Consumer/                     # portable external-consumer template
  Generated/                    # ignored Core output
    Sources/Cribra/{Cribra.swift,BuildInfo.swift}
    CribraFFI.xcframework/
      macos-arm64/libcribra_uniffi.a
      ios-arm64/libcribra_uniffi.a
      ios-arm64-simulator/libcribra_uniffi.a
      .../Headers/{CribraFFI.h,module.modulemap}
    BuildInfo.txt
    SourceInputs.sha256
    Artifacts.sha256
```

Flow and Fuse have the same generated layout under their own package directory.
The generated Swift file, headers/modulemap, archives and XCFramework are build
products and are not committed. Copy `Package.swift` and the complete matching
`Generated/` directory to distribute a standalone local package. The copied
package has no path dependency on the Rust repository, Rust toolchain, generator
or build script. This phase does not publish a remote package or signed release.

## Build

Prerequisites: native macOS ARM64, Xcode with macOS/iOS/iOS Simulator SDKs,
Swift 6.4, Rust 1.97.0 and `just`. Install the required Rust standard libraries:

```sh
rustup target add --toolchain 1.97.0 \
  aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim

just swift-apple core
just swift-apple flow
just swift-apple fuse
just swift-apple-inspect core
```

The driver compiles with `cargo +1.97.0 rustc --locked --release --lib
--crate-type staticlib` and the explicit profile/target. Rust release optimization
is enabled; stripping is explicitly disabled because Xcode 27's loader rejects
stripped host proc-macro dylibs produced by this toolchain with a mis-aligned
LINKEDIT string pool. This setting is build-local and does not modify Core or
workspace release profiles.

The native target triples are:

- `aarch64-apple-darwin`: real macOS ARM64 static library.
- `aarch64-apple-ios`: real iOS ARM64 device static library.
- `aarch64-apple-ios-sim`: real iOS Simulator ARM64 static library.

No macOS dylib is copied, renamed or used as an iOS slice. The adapter retains
its existing cdylib/rlib outputs and additionally supports staticlib. Generator
code runs on the host, independently of those native slices. UniFFI extracts
metadata directly from **each static archive**; all three generated Swift/header/
modulemap outputs must match byte-for-byte before assembly. The existing expected
API/DTO/error contract is checked for that profile. Runtime UniFFI checksums
remain enabled.

`xcodebuild -create-xcframework -library ... -headers ...` packages the three
archives separately. The driver inspects Info.plist platform identifiers,
Simulator variant, ARM64 architecture and static ar magic, plus Mach-O build
platform tags inside the archives. Headers are packaged beside each slice;
SwiftPM imports `CribraFFI` through `module.modulemap`, without manual compiler
or linker search-path flags in the consumer.

Cargo caches and staging work are isolated by profile under
`target/swift-apple/`. A build lock prevents concurrent packaging invocations;
staged output is promoted only after all targets, binding comparisons and
inspections pass. Existing recognized generated output is replaced as a complete
set, not updated one native library at a time. Rust input hashes are checked
before/after the build. Failed stages remain ignored for diagnosis and are not
promoted. After an interrupted process, inspect `target/swift-apple/build.lock`
and its PID before manually removing a stale lock.

`BuildInfo.swift` exposes `CribraBuild.profile`, `coreVersion`, `adapterVersion`,
`uniffiVersion` and `rustVersion`. `SourceInputs.sha256` records build inputs;
`Artifacts.sha256` detects accidental edits/mixing in the complete generated set.
These hashes are provenance/integrity metadata, not signed release attestations.

## Integrate from an external app

Add the selected directory as a **local Swift package** in Xcode, or declare:

```swift
// In the consumer's Package.swift. Pick one profile directory.
dependencies: [.package(path: "/path/to/cribra/bindings/swift")]
// Then use the Cribra product in the app target's dependencies.
```

For a standalone copied package named `CribraDistribution`:

```swift
.package(path: "../CribraDistribution")
.product(name: "Cribra", package: "CribraDistribution")
```

App source:

```swift
import Cribra

let scanner = Cribra.Scanner() // qualified if Foundation.Scanner is also imported
let results = scanner.scanMany(sources: [
    ScanSource(id: SourceId(value: "editor/文書"), text: text)
])
let document = try scanner.incremental(source: ScanSource(
    id: SourceId(value: "editor/文書"), text: text
))
let snapshot = try document.snapshot()
```

The consumer does not invoke Cargo, use a build plugin, copy loose native
libraries, or set a DYLD library path. Core/Flow/Fuse behavior and ownership remain
those of the existing UniFFI adapter. Keyed operations still require explicit
caller-owned keys; packaging adds no key generation, storage or logging.

## Reproduce external integration checks

```sh
# Build the selected profile first, then create a fresh consumer outside this repo.
just swift-apple-consumer core
just swift-apple-consumer flow
just swift-apple-consumer fuse

# Optionally choose a new absolute external directory (it must not already exist).
just swift-apple-consumer core /private/tmp/cribra-core-consumer
```

The driver verifies artifact hashes before and after copying **only the standalone package**
and consumer template into a new temporary directory outside the checkout. It
resolves the dependency with SwiftPM, compiles and runs the macOS executable,
then compiles the consumer with SwiftPM for `arm64-apple-ios16.0-simulator` using
the iOS Simulator SDK. The same consumer source has a SwiftUI `App` entry point
on iOS. This compiles native app integration code and links the Simulator archive;
it is not a simulator launch or physical-device execution claim.

The executable checks version/profile metadata, Scanner and duplicate/Unicode
IDs, UTF-8/CRLF/emoji coordinates, streaming Data and atomic incremental snapshots.
Flow/Fuse variants additionally invoke transformations and Share Bundles; Fuse
invokes parallel scanning. The full existing generated-binding contract check
runs during each package build, including absence of Flow/Fuse APIs from Core
and parallelScan from Flow.

SwiftPM state/build/module caches stay in the external consumer tree. The test
driver disables SwiftPM's nested manifest sandbox to work under the agent's
filesystem sandbox; all dependencies are local and no packages are fetched.
This flag is not required in ordinary Xcode integration and is not a package
build setting. Consumer directories are retained and printed for inspection.

Additional regressions from the repository root:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features core
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features fuse
just uniffi-swift-smoke core
just uniffi-swift-share-smoke flow
just uniffi-swift-share-smoke fuse
just uniffi-swift-parallel-smoke
cargo +1.97.0 check --workspace
cargo fmt --all -- --check
rustfmt +1.97.0 --edition 2024 --check bindings/swift/tools/apple.rs
git diff --check
```

## Boundaries

Only ARM64 macOS, iOS device and iOS Simulator slices are provided. Intel Mac,
x86_64 Simulator, Catalyst, tvOS/watchOS/visionOS, remote binary downloads,
checksums for remote SwiftPM targets, release signing/notarization, App Store
submission and device/simulator runtime qualification are outside this phase.
If an SDK or Rust target is missing, the build fails explicitly and keeps the
previous complete package; it does not fabricate a slice or fall back to macOS.
Core, C API, WASM and CLI are unchanged. Do not commit generated trees/binaries.
