# Cribra for native Swift apps

See [Getting started](../../docs/GETTING_STARTED.md) and
[Compatibility and distribution status](../../docs/COMPATIBILITY.md) for the
cross-language entry points and evidence boundaries.

Local Swift Package distribution of Cribra Core **0.7.2**, adapter **0.1.0**,
UniFFI **=0.32.2**, Rust **1.97.0** and Swift **6.4**. The public Swift module and
library product are named **`Cribra`**. Deployment minimums are macOS 13 and iOS 16.

**Current status:** the repository builds local SwiftPM packages for Core, Flow
and Fuse. W3.1C adds a local packaging command that produces an **unpublished
staging directory** for the planned Fuse remote package: an XCFramework ZIP,
SwiftPM checksum, generated sources, manifest and provenance metadata. No
distribution repository, remote package, release asset, published checksum or
Swift Package Index entry exists yet. See
[Remote SwiftPM distribution contract](#remote-swiftpm-distribution-contract-fuse)
for the target contract and
[Local Fuse distribution packaging](#local-fuse-distribution-packaging-w31c) for
the implemented staging workflow.

## Layout and profile selection

Choose exactly one package dependency per app/process:

| Profile | Package directory | Rust features | Available operations | Packaging role |
| --- | --- | --- | --- | --- |
| Core | `bindings/swift` | `--no-default-features --features core` | Scanner, configuration, stream, incremental, atomic snapshot. | Source-build alternative only. |
| Flow | `bindings/swift/Profiles/Flow` | `--no-default-features --features flow` | Core plus four transforms and Share Bundles. | Source-build alternative only. |
| Fuse | `bindings/swift/Profiles/Fuse` | `--no-default-features --features fuse` | Flow plus parallel scanning. | Planned official remote package; local source builds remain supported. |

Fuse is the intended public distribution profile because it exposes the complete
supported binding feature set. Core and Flow remain supported source-build
alternatives; they are alternative builds of the same engine, not separate
registry packages or add-on modules intended to coexist with Fuse. Their role is
recorded in the [ecosystem distribution policy](../../docs/ECOSYSTEM.md#distribution-policy).

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

## Remote SwiftPM distribution contract (Fuse)

**Status: contract implemented for local staging only.** This section defines the
planned official remote SwiftPM distribution for Swift. W3.1C produces an
unpublished local staging directory that matches this contract, but it does not
describe a package that consumers can resolve today. The repository does not yet
contain a committed or published distribution manifest, live remote URL, release
asset, published SwiftPM checksum or Swift Package Index registration. Until the
publication milestone completes, the local integration path below is the only
usable installation method.

### Public package and profile policy

The official public Swift distribution is one SwiftPM package using the **Fuse**
profile. It must expose the complete supported Cribra binding API. Core and Flow
remain source-build alternatives and are not published as separate official
registry packages.

| Item | Contract value |
| --- | --- |
| Distribution repository | `Metalymph/cribra-swift` (provisional until explicitly created) |
| SwiftPM package identity (proposed) | `cribra-swift`, derived from the distribution URL |
| Manifest package name (proposed) | `Cribra` |
| Public product | `Cribra` |
| Consumer import | `import Cribra` |
| Swift module | `Cribra` |
| Native binary target and module | `CribraFFI` |
| XCFramework | `CribraFFI.xcframework` |
| Initial distribution version | `0.7.2` |
| Distribution tag format | Bare semantic version, for example `0.7.2` |

The `Cribra` product and module and the `CribraFFI` binary module are confirmed
from the existing manifests and `uniffi.toml`. The package identity and manifest
package name remain provisional with the distribution repository. The remote
package must expose one public library product, `Cribra`, and must not rename
generated modules or change the UniFFI configuration.

The package must not require consumers to install Rust, Cargo, UniFFI
generators, Xcode build plugins, custom build tools or Cribra source files. The
Swift source and native binary must originate from the same qualified build.

Intended distribution repository layout:

```text
cribra-swift/
├── Package.swift
├── Sources/
│   └── Cribra/
│       ├── Cribra.swift
│       └── BuildInfo.swift
├── README.md
└── LICENSE
```

`Package.swift` uses a remote binary target for the prebuilt Fuse XCFramework.
The XCFramework must be hosted as an immutable GitHub Release asset and must not
be committed to the distribution repository.

### Versions

| Component | Recorded version |
| --- | --- |
| Rust Core `cribra` | `0.7.2` |
| Shared UniFFI adapter `cribra-uniffi` | `0.1.0` |
| UniFFI toolchain and Swift generator | `0.32.2` |
| Rust toolchain used to build native slices | `1.97.0` |
| Swift compiler used to build and qualify generated source | `6.4` |
| Initial Swift distribution package | `0.7.2` |
| XCFramework deployment minimums | macOS `13.0`, iOS `16.0` |

The Swift distribution is versioned independently, even though its first
version aligns with Core `0.7.2`. A packaging-only correction or enhancement
increments the Swift distribution version without requiring a new Rust Core
release. A change to the native contract must still be traced to the matching
Core and adapter versions recorded in `BuildInfo.swift`.

Use bare semantic-version tags in the Swift distribution repository. Do not
reuse the Rust Core repository's `v0.7.2` tag, and do not infer that a Swift
package tag changes any Core, adapter or UniFFI version.

### Swift toolchain compatibility

The existing evidence establishes the following:

- Every current Swift package and consumer manifest declares
  `// swift-tools-version: 6.4`.
- The Apple build driver rejects a compiler whose `swiftc --version` does not
  contain `Swift version 6.4`.
- The generated binding source is compiled in Swift 6 language mode, and the
  local and CI qualification environments use Swift 6.4.

The roles are distinct:

- **Minimum required consumer toolchain:** Swift 6.4, enforced by the manifest
  tools version. Compatibility below 6.4 is not established.
- **Native artifact build toolchain:** Rust 1.97.0 with the three Apple targets
  and Apple SDK deployment minimums, plus UniFFI 0.32.2 for Swift source
  generation. The XCFramework slices are Rust static libraries.
- **Swift build and CI qualification toolchain:** Swift 6.4, including the
  `xcode-27` CI environment and local macOS ARM64 qualification.

This contract does not establish that Swift 6.4 is a language-level minimum
independent of the manifest: no evidence shows that the generated source
compiles with an earlier 6.x toolchain, and none shows that
`swift-tools-version` can be lowered safely.

Do not lower `swift-tools-version` without a compatibility matrix covering the
consumer toolchain and generated source. The earliest supported consumer
toolchain below 6.4 remains unresolved.

### Supported platforms

| Platform | Architecture | Minimum |
| --- | --- | --- |
| macOS | ARM64 | macOS 13 |
| iOS device | ARM64 | iOS 16 |
| iOS Simulator | ARM64 | iOS 16 |

The distribution contains exactly these three slices. Do not claim support for
Intel macOS, Intel iOS Simulator, Mac Catalyst, tvOS, watchOS or visionOS.
Those targets may be evaluated later from demonstrated demand, but this
milestone does not expand the architecture matrix.

Deployment minimums are declared compatibility floors, not runtime
qualification on every OS release.

### Binary artifact contract

Publish one Fuse XCFramework archive per Swift package release:

```text
CribraFFI-fuse-0.7.2.xcframework.zip
```

The archive must contain the complete, qualified `CribraFFI.xcframework` with
the `macos-arm64`, `ios-arm64` and `ios-arm64-simulator` slices, headers and
`module.modulemap`. The release asset is the only native binary input to the
remote package.

The remote `Package.swift` must reference:

- an immutable GitHub Release asset URL; and
- the exact SwiftPM checksum of the final archive.

The intended URL pattern is:

```text
https://github.com/Metalymph/cribra-swift/releases/download/0.7.2/CribraFFI-fuse-0.7.2.xcframework.zip
```

That URL is planned, not live, until the distribution repository and release
exist. The illustrative manifest shape is:

```swift
.binaryTarget(
    name: "CribraFFI",
    url: "https://github.com/Metalymph/cribra-swift/releases/download/0.7.2/CribraFFI-fuse-0.7.2.xcframework.zip",
    checksum: "<computed-at-release>"
)
```

Compute the SwiftPM checksum from the final archive with:

```sh
swift package compute-checksum CribraFFI-fuse-0.7.2.xcframework.zip
```

This archive-level checksum is distinct from `SourceInputs.sha256`,
`Artifacts.sha256` and the per-file SHA-256 provenance hashes. They verify
different things and must not be substituted for one another.

Published release assets are immutable. Do not overwrite an existing archive. If
a published artifact requires correction, release a new Swift distribution
version with a new asset.

### Publication procedure

Run the procedure from a clean checkout of the intended source revision:

1. Record the exact Core and adapter source revision, plus the versions in the
   table above. Confirm that the adapter still declares Rust `1.97` and UniFFI
   `=0.32.2`.
2. Build the Fuse package from that revision:
   `just swift-apple fuse`.
3. Inspect the XCFramework slices and metadata:
   `just swift-apple-inspect fuse`.
4. Run the local independent consumer gate:
   `just swift-apple-consumer fuse`.
5. Produce and locally qualify the unpublished staging package:
   `just swift-apple-dist fuse`. The command verifies the Fuse generated
   artifacts and provenance, creates
   `CribraFFI-fuse-0.7.2.xcframework.zip`, computes and records the SwiftPM
   checksum, stages the matching sources and manifest, validates the archive and
   manifest, and runs the existing consumer against the staged sources and
   archive. Output is written under
   `target/swift-distribution/fuse-0.7.2/`. This step never publishes.
6. Review the staging output, especially `distribution.json` and the SwiftPM
   checksum. Copy `Package.swift`, `Sources/`, `README.md` and `LICENSE` into the
   distribution repository. Upload the ZIP as the release asset; do not commit it.
7. Commit the distribution `Package.swift` containing the exact asset URL and
   checksum. Confirm that `Sources/Cribra/Cribra.swift` and
   `Sources/Cribra/BuildInfo.swift` come from the same Fuse build.
8. Create a draft `0.7.2` GitHub Release in `Metalymph/cribra-swift` targeting
   that distribution commit, upload the archive, and verify that its asset URL
   downloads the exact uploaded bytes.
9. Publish the release only after the asset exists, which creates the bare
   `0.7.2` tag. Then run the
   [remote-consumer acceptance gate](#remote-consumer-acceptance-gate) against
   the published tag, not a local path dependency.
10. Keep the release asset and tag immutable. Correct a packaging defect with a
    new Swift distribution version.

The generated source and native binary are tied together because the driver
extracts bindings from each of the three static archives, requires the generated
Swift, header and module map files to match byte-for-byte, and records the
complete generated set in `Artifacts.sha256`. `SourceInputs.sha256` detects
changes to the Rust and UniFFI inputs across the build. These are provenance and
integrity records, not signed release attestations or SwiftPM checksums.

### Remote-consumer acceptance gate

The remote gate must run outside the Cribra checkout, in a clean consumer
directory, with no local path dependency and without Rust, Cargo, UniFFI
generators or Cribra source files available. It must resolve the published
package from its remote Git URL and select an exact published semantic version.

Acceptance requires all of the following:

1. SwiftPM resolves the exact package version from the remote URL.
2. SwiftPM downloads the Fuse XCFramework and verifies the manifest checksum.
3. The consumer compiles with `import Cribra` and sees one public `Cribra`
   product.
4. Representative scanning works: `Scanner`, `scanMany(sources:)`, an admitted
   stream and an incremental document with an atomic snapshot.
5. Fuse transformations work: `redact`, `template`, `pseudonymize` and
   `synthesize`.
6. Share Bundle construction works through the documented Fuse API.
7. `Scanner.parallelScan(sources:)` works, preserves declared behavior and
   `CribraBuild.profile` reports `fuse`.
8. The executable builds and runs on macOS ARM64.
9. The same consumer source compiles and links for
   `arm64-apple-ios16.0-simulator`; this is compile/link evidence, not simulator
   launch or device execution.
10. The run does not invoke Cargo, `rustc`, UniFFI generators or a Cribra
    checkout.

Additional Fuse-specific checks should compare `parallelScan` with sequential
results, exercise all four transformations with explicit caller-owned keys where
required, and inspect the archive's three platform slices and deployment
minimums. The gate must fail if it falls back to a local package path, a branch,
a version range or an unpublished asset.

### Swift Package Index

Swift Package Index is a discovery and documentation service, not a binary
hosting service. The package must first be remotely installable and pass the
clean-room gate. Index registration is a later milestone; W3.1B does not create
or submit an SPI entry. Do not assume Linux compatibility for an Apple-only
binary package.

### Unresolved distribution questions

- The `Metalymph/cribra-swift` repository, package name, URL, tag and release
  assets are provisional and do not exist under this contract.
- The earliest consumer Swift toolchain below 6.4 has not been established.
- Signing, notarization, App Store submission and binary provenance attestations
  are not defined by this contract.
- Distribution release automation and immutable-asset policy enforcement are
  future implementation work.
- Kotlin/JVM and Python have the same Fuse-first policy but require separate
  distribution contracts; Swift packaging details do not define Maven or PyPI
  mechanics.

## Local Fuse distribution packaging (W3.1C)

`just swift-apple-dist fuse` implements the local staging half of the remote
contract above without publishing anything. It consumes an existing Fuse
`Generated/` tree produced by `just swift-apple fuse`; it does not rebuild with
different settings. Run the build first, or the command fails with an actionable
error.

The command:

1. Rejects profiles other than `fuse`, checks the adapter MSRV/UniFFI pins and
   requires Rust 1.97.0 and Swift 6.4.
2. Reads `BuildInfo.txt` and requires profile `fuse`, Core `0.7.2`, adapter
   `0.1.0`, UniFFI `0.32.2`, Rust `1.97.0`, macOS `13.0` and iOS `16.0`.
3. Verifies `Artifacts.sha256` and requires the recorded `SourceInputs.sha256` to
   match the current Rust/UniFFI build inputs. A mismatch tells you to rebuild
   rather than mixing artifacts from different revisions.
4. Reuses the existing XCFramework inspection: exactly three ARM64 static
   slices, macOS/device/Simulator platform tags, deployment minimums, headers and
   module maps.
5. Archives only `CribraFFI.xcframework` at the archive root with `/usr/bin/zip`
   and `-X` (no `Generated/`, resource forks or unrelated products).
6. Computes the SwiftPM checksum twice with
   `swift package compute-checksum`, and separately records the archive SHA-256.
7. Stages the exact generated `Cribra.swift` and `BuildInfo.swift`, compares their
   hashes with the Fuse build, and refuses Core/Flow metadata.
8. Writes a remote-manifest `Package.swift` with one `Cribra` product, a
   `CribraFFI` remote binary target, macOS 13/iOS 16 and tools version 6.4, and
   validates it with `swift package dump-package`.
9. Extracts the final ZIP, re-inspects the extracted XCFramework, and compares
   every extracted file hash with the generated XCFramework.
10. Qualifies the staged sources and archive with a temporary local-binary
    manifest and the existing external consumer: the macOS executable runs, and
    the same consumer compiles and links for
    `arm64-apple-ios16.0-simulator`. The staged remote manifest is not modified.
11. Writes `distribution.json`, re-checks that inputs did not change, and only
    then promotes the staging directory to its final name.

### Output layout

```text
target/swift-distribution/fuse-0.7.2/
├── Package.swift
├── Sources/Cribra/{Cribra.swift,BuildInfo.swift}
├── CribraFFI-fuse-0.7.2.xcframework.zip
├── CribraFFI-fuse-0.7.2.xcframework.zip.checksum
├── CribraFFI-fuse-0.7.2.xcframework.zip.sha256
├── distribution.json
├── README.md
└── LICENSE
```

`*.zip.checksum` contains the exact SwiftPM checksum used by the manifest.
`*.zip.sha256` records the archive digest in `shasum -c` format. Both are
archive-level values and do not replace the per-file `SourceInputs.sha256` or
`Artifacts.sha256`.

The manifest uses the planned URL
`https://github.com/Metalymph/cribra-swift/releases/download/0.7.2/CribraFFI-fuse-0.7.2.xcframework.zip`
and the computed checksum. It is explicitly labelled as an unpublished staging
manifest; the URL is not live. `distribution.json` records the profile, Swift
distribution version, Core/adapter/UniFFI/Rust/Swift versions, source Git
revision and dirty flag, generated-source hashes, input metadata hashes, archive
name/size/SHA-256/SwiftPM checksum, platform table, timestamp and the
qualification that ran.

### Failure handling and atomicity

Packaging shares `target/swift-apple/build.lock` with the build driver, so a
concurrent build cannot replace `Generated/` while it is inspected. Output is
assembled in `fuse-0.7.2.staging-<pid>` and promoted only after every validation
and the local consumer qualification pass. A previous recognized
`fuse-0.7.2` directory is renamed aside and restored if promotion fails; an
unrecognized directory is never overwritten. Failed staging directories keep
their clear `.staging-` name for diagnosis and are ignored by Git. The command
does not delete unrelated build artifacts.

### Qualification scope and limitations

The macOS consumer executes scanning, batch scanning, streaming, incremental
scanning, all four transformations, Share Bundles and parallel scanning. The iOS
Simulator build is compile/link evidence only, not simulator launch or device
execution. Qualification uses a temporary local binary-target manifest, so it
does not exercise remote download, URL resolution or published checksum
verification. It also does not establish bit-for-bit reproducibility: ZIP
timestamps and host toolchain details can change. Signing, notarization, release
publication, the remote-consumer acceptance gate and Swift Package Index remain
later milestones.

Packaging validators can be exercised without building the distribution:

```sh
just swift-apple-packaging-test
```

## Integrate from an external app

This is the current source-based path. It does not use the planned remote
distribution contract and remains valid for Core, Flow and Fuse development.

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
x86_64 Simulator, Catalyst, tvOS/watchOS/visionOS and device/simulator runtime
qualification remain outside the qualified matrix. The remote contract is only
locally staged, not implemented as a published package: there is no live remote
package, release asset, published SwiftPM checksum or SPI entry. Release
signing/notarization, App Store submission and automated distribution publication
are also outside this phase. If an SDK or Rust target is missing, the build fails
explicitly and keeps the previous complete package; it does not fabricate a slice
or fall back to macOS. Core, C API, WASM and CLI are unchanged. Do not commit
generated trees/binaries or distribution staging output.
