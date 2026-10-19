# Cribra for Kotlin/JVM apps

Minimal Gradle Kotlin DSL consumer foundation for Cribra Core **0.7.1**, adapter
**0.1.0**, UniFFI **=0.32.2**, Rust **1.97.0**, Kotlin **2.4.21**, JNA **5.17.0**,
Gson **2.13.2** and Gradle **9.3.1**. The JVM baseline is **JDK 17**.

This phase delivers the **Core**, **Flow/Share** and **Fuse** profiles: a real JVM
consumer for each, replaying the shared repository fixtures and direct-Core oracle
outputs, one independently usable thin profile JAR per profile, and reproducible,
host-specific distribution bundles that pair a profile JAR with its native
library. Android and Kotlin Multiplatform are intentionally out of scope.

## Layout

```
bindings/kotlin/
  settings.gradle.kts            # project name
  build.gradle.kts               # Kotlin/JVM, JDK 17, run/runFlow/runFuse, profile JARs
  gradle/wrapper/                # pinned Gradle 9.3.1 wrapper
  gradlew, gradlew.bat
  src/main/kotlin/cribra/kotlin/smoke/Smoke.kt       # Core consumer
  src/main/kotlin/cribra/kotlin/profile/CoreProfile.kt
  src/flow/kotlin/cribra/kotlin/flow/FlowSmoke.kt    # Flow/Share consumer
  src/flow/kotlin/cribra/kotlin/profile/FlowProfile.kt
  src/fuse/kotlin/cribra/kotlin/fuse/FuseSmoke.kt    # Fuse consumer
  src/fuse/kotlin/cribra/kotlin/profile/FuseProfile.kt
  build_bundle.py                # deterministic native distribution bundles
  qualify_bundle_consumer.py     # isolated consumers of existing ZIP bundles
  verify/consumer/               # minimal standalone Core/Flow/Fuse JVM consumers
  verify/ClasspathCheck.java     # single-profile classpath verifier
```

One module, three source sets: `main` (Core), `flow` (Flow/Share) and `fuse`
(Fuse). Each source set compiles against exactly one generated binding because all
three bindings declare the `uniffi.cribra_uniffi` package and must never share a
classpath. JNA is the only runtime dependency of the generated bindings; the
`flow` and `fuse` smokes additionally use Gson to read the shared JSON fixtures
and oracle files, but Gson is not part of any profile JAR.

The generated bindings, oracle outputs, profile JARs and native library are
**build outputs** under the repository's ignored `target/` and
`bindings/kotlin/build/` trees. Nothing generated is committed:

```
target/uniffi-kotlin-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt        # Core binding
target/uniffi-kotlin-flow-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt   # Flow/Share binding
target/uniffi-kotlin-fuse-smoke/generated/uniffi/cribra_uniffi/cribra_uniffi.kt   # Fuse binding
target/uniffi-kotlin-flow-smoke/{flow-oracle.json,share-oracle.json}              # direct-Core oracles
target/uniffi-kotlin-fuse-smoke/parallel-oracle.json                              # direct-Core oracle
bindings/kotlin/build/libs/cribra-kotlin-{core,flow,fuse}-0.7.1.jar               # profile JARs
target/debug/libcribra_uniffi.{dylib,so,dll}                                      # native cdylib
target/uniffi-kotlin-bundles/dist/cribra-kotlin-{core,flow,fuse}-0.7.1-<platform>.zip  # bundles
target/uniffi-kotlin-bundles/{core,flow,fuse}/{cargo,generated,native,bundle}/         # per-profile isolation
```

Gradle's own `build/` and `.gradle/` directories stay ignored under
`bindings/kotlin/`.

## Prerequisites

- Rust **1.97.0** with the `cribra-uniffi` `core`, `flow` and `fuse` features
  buildable on the host.
- JDK **17** (the build pins `jvmToolchain(17)`; the wrapper matches `JAVA_HOME`).
- Network access on the first Gradle run: the wrapper downloads Gradle 9.3.1 and
  resolves the Kotlin plugin, JNA and Gson from Maven Central. Later runs are
  cached.
- Python **3.11+** for `just uniffi-kotlin-bundles` (the builder uses `tomllib`
  and `zipfile`; recent macOS and Linux hosts ship a suitable `python3`).

The Gradle wrapper is committed, so no system Gradle installation is required.

## Build and run

One reproducible command per profile, from the repository root:

```sh
just uniffi-kotlin-smoke        # Core
just uniffi-kotlin-flow-smoke   # Flow/Share
just uniffi-kotlin-fuse-smoke   # Fuse
just uniffi-kotlin-parallel-gating  # profile API gating
just uniffi-kotlin-jars         # package + verify all three profile JARs
just uniffi-kotlin-bundles      # build + verify the three host distribution bundles
```

`uniffi-kotlin-smoke` builds the Core native library, generates the Core binding
into `target/uniffi-kotlin-smoke/generated` (`--features bindgen,core`) and runs
`bindings/kotlin/gradlew -p bindings/kotlin run`.

`uniffi-kotlin-flow-smoke` builds the Flow library, generates the Flow binding
into `target/uniffi-kotlin-flow-smoke/generated` (`--features bindgen,flow`),
writes the `flow_oracle` and `share_oracle` example outputs into
`target/uniffi-kotlin-flow-smoke/`, and runs `bindings/kotlin/gradlew -p
bindings/kotlin runFlow`. The Rust oracle examples are the same ones the
qualified Swift consumer uses; Kotlin does not reimplement Core semantics.

`uniffi-kotlin-fuse-smoke` builds the Fuse library, generates the Fuse binding
into `target/uniffi-kotlin-fuse-smoke/generated` (`--features bindgen,fuse`),
writes the `parallel_oracle` example output into
`target/uniffi-kotlin-fuse-smoke/`, and runs `bindings/kotlin/gradlew -p
bindings/kotlin runFuse`. It exercises Core-owned parallel scanning without
reimplementing scheduling or scanning in Kotlin.

`uniffi-kotlin-parallel-gating` regenerates the Core, Flow and Fuse bindings and
asserts that Core-owned parallel scanning (`parallelScan` / `parallel_scan`) is
exported only by the Fuse profile.

`uniffi-kotlin-jars` deletes the three generated binding trees, regenerates them
for `core`, `flow` and `fuse`, runs `clean`, packages the three profile JARs,
verifies their contents/manifests/classfiles/API gating, and proves the
single-profile classpath guard with single and mixed classpaths.

The same steps by hand:

```sh
# Core
CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features core --lib
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,core \
  --bin cribra-uniffi-bindgen -- generate \
  --library target/debug/libcribra_uniffi.dylib --language kotlin \
  --out-dir target/uniffi-kotlin-smoke/generated
bindings/kotlin/gradlew -p bindings/kotlin --console=plain run

# Flow/Share
CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features flow --lib
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,flow \
  --bin cribra-uniffi-bindgen -- generate \
  --library target/debug/libcribra_uniffi.dylib --language kotlin \
  --out-dir target/uniffi-kotlin-flow-smoke/generated
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features flow \
  --example flow_oracle > target/uniffi-kotlin-flow-smoke/flow-oracle.json
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features flow \
  --example share_oracle > target/uniffi-kotlin-flow-smoke/share-oracle.json
bindings/kotlin/gradlew -p bindings/kotlin --console=plain runFlow

# Fuse
CARGO_TARGET_DIR=target cargo +1.97.0 build -p cribra-uniffi --no-default-features --features fuse --lib
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features bindgen,fuse \
  --bin cribra-uniffi-bindgen -- generate \
  --library target/debug/libcribra_uniffi.dylib --language kotlin \
  --out-dir target/uniffi-kotlin-fuse-smoke/generated
CARGO_TARGET_DIR=target cargo +1.97.0 run -p cribra-uniffi --no-default-features --features fuse \
  --example parallel_oracle > target/uniffi-kotlin-fuse-smoke/parallel-oracle.json
bindings/kotlin/gradlew -p bindings/kotlin --console=plain runFuse
```

Use `libcribra_uniffi.so` on Linux and `cribra_uniffi.dll` on Windows. Each task
loads the library through `jna.library.path`, so the library name must match the
host (`cribra_uniffi`). Paths are overridable for a relocated checkout:

```sh
bindings/kotlin/gradlew -p bindings/kotlin run \
  -Pcribra.kotlin.generated=/abs/path/to/generated -Pcribra.kotlin.native=/abs/path/to/native-dir
bindings/kotlin/gradlew -p bindings/kotlin runFlow \
  -Pcribra.kotlin.flow.smoke-dir=/abs/path/to/flow-smoke-dir -Pcribra.kotlin.native=/abs/path/to/native-dir
bindings/kotlin/gradlew -p bindings/kotlin runFuse \
  -Pcribra.kotlin.fuse.smoke-dir=/abs/path/to/fuse-smoke-dir -Pcribra.kotlin.native=/abs/path/to/native-dir
```

UniFFI formats the generated file with `ktlint` when available; the build still
succeeds and prints a warning when `ktlint` is not installed. This affects
formatting only, not the binding.

## Profile JARs

`just uniffi-kotlin-jars` produces three thin, independently usable JARs:

```
bindings/kotlin/build/libs/
  cribra-kotlin-core-0.7.1.jar   # Core
  cribra-kotlin-flow-0.7.1.jar   # Flow/Share
  cribra-kotlin-fuse-0.7.1.jar   # Fuse (parallel scanning)
```

Each JAR contains only:

- the generated `uniffi.cribra_uniffi` classes for that profile, plus their
  `META-INF/*.kotlin_module` metadata, and
- its `cribra.kotlin.profile.{Core,Flow,Fuse}Profile` metadata/guard class.

The smoke consumers (`cribra.kotlin.smoke`, `cribra.kotlin.flow`,
`cribra.kotlin.fuse`), native libraries and third-party dependencies are not
packaged. These are thin binding JARs, not fat JARs. The matching native
`libcribra_uniffi.{dylib,so,dll}` stays external and is loaded by JNA from
`jna.library.path` (or the platform loader's search path).

A consumer supplies:

- the Kotlin standard library (bindings are compiled with Kotlin **2.4.21**),
- JNA **5.17.0** (`net.java.dev.jna:jna`),
- the matching Cribra native library from the selected Rust profile.

Every class targets **JDK 17** (classfile major version 61) and the JARs are
reproducible (fixed entry order, no entry timestamps). Manifest metadata:

| Attribute | Value |
| --- | --- |
| `Implementation-Title` | `Cribra Kotlin/JVM <profile> profile` |
| `Implementation-Version` | `0.7.1` (Cribra Core) |
| `Implementation-Vendor` | `Cribra` |
| `Cribra-Profile` | `core`, `flow` or `fuse` |
| `Cribra-Core-Version` | `0.7.1` |
| `Cribra-Adapter-Version` | `0.1.0` |
| `Cribra-Uniffi-Version` | `0.32.2` |
| `Cribra-Native-Library` | `external` |
| `Cribra-Classpath-Rule` | `exactly-one-profile` |
| `Automatic-Module-Name` | `cribra.kotlin.<profile>` |

### Classpath rule

All three JARs deliberately keep the same `uniffi.cribra_uniffi` package. **Use
exactly one Cribra profile JAR per consumer classpath.** Combining two profiles
would put duplicate classes on the classpath, and classpath order would silently
choose which profile wins (Core could hide `parallelScan`, for example). The
packaging task fails if a foreign profile class or marker leaks into an artifact.

Each JAR ships a dependency-free guard to turn a mix-up into an immediate
failure. Call it once during startup:

```kotlin
import cribra.kotlin.profile.CoreProfile

CoreProfile.requireExclusiveClasspath() // IllegalStateException if Flow/Fuse is present
println("${CoreProfile.ID} ${CoreProfile.CORE_VERSION}")
```

`verifyProfileJars` checks the classfile level, manifests, profile API gating and
that no smoke, native or dependency entries are present. `uniffi-kotlin-jars`
additionally exercises the guard with single-profile and mixed classpaths. On the
JPMS module path, the distinct `Automatic-Module-Name` values also make accidental
combinations fail as split packages.

## Distribution bundles

`just uniffi-kotlin-bundles` builds one reproducible bundle per profile for the
current host:

```
target/uniffi-kotlin-bundles/dist/
  cribra-kotlin-core-0.7.1-macos-arm64.zip
  cribra-kotlin-flow-0.7.1-macos-arm64.zip
  cribra-kotlin-fuse-0.7.1-macos-arm64.zip
```

Each ZIP contains exactly this layout at its root:

```
README.md
lib/cribra-kotlin-<profile>-0.7.1.jar
lib/jna-5.17.0.jar
native/libcribra_uniffi.{dylib,so,dll}
metadata/build-info.json
metadata/SHA256SUMS
```

The profile JAR follows the same packaging rules as `uniffi-kotlin-jars`, and
native libraries are never embedded in a JAR. The native library is built
separately for each profile in its own Cargo target directory
(`target/uniffi-kotlin-bundles/<profile>/cargo`), so one profile's `cdylib` can
never be packaged by another. On macOS the Mach-O install name is set to
`@rpath/libcribra_uniffi.dylib` (and on Linux the SONAME to
`libcribra_uniffi.so`) so the shipped library never records an absolute
repository or build path. The macOS build also pads the symbol string table
and code signature to the alignment the current dyld requires and re-signs the
library ad-hoc; the pass is a no-op when the linker already emits aligned
structures.

`metadata/build-info.json` records the profile, Cribra Core, adapter, UniFFI,
Kotlin, JNA and Rust versions, the JDK baseline, host OS/architecture, Rust
target triple, native library path and the SHA-256 and size of every payload.
`metadata/SHA256SUMS` covers every file except itself. Both are deterministic:
no timestamps and no absolute paths. ZIP entries are sorted, use the 1980 ZIP
epoch, Unix `0644` modes and a fixed deflate level, so a clean rebuild of the
same host and toolchain produces the same archive SHA-256.

Verify a bundle from its root:

```sh
shasum -a 256 -c metadata/SHA256SUMS
```

The recipe runs the same checks before it writes the archive: exact layout,
profile manifest, exactly one native library, JNA present, no Gson or smoke
classes, no embedded native library, valid checksums, no repository path in any
payload and, on macOS, a valid ad-hoc signature plus a real dyld load of the
native library.

### Using a bundle

1. Put `lib/cribra-kotlin-<profile>-0.7.1.jar` and `lib/jna-5.17.0.jar` on the
   application classpath.
2. Provide the Kotlin standard library (2.4.21) and JDK 17+.
3. Make `native/libcribra_uniffi.{dylib,so,dll}` loadable by JNA, for example
   with `-Djna.library.path=<bundle>/native`.
4. Use exactly one Cribra profile bundle per classpath and call
   `cribra.kotlin.profile.{Core,Flow,Fuse}Profile.requireExclusiveClasspath()`
   once at startup.

### Bundle boundaries

- macOS ARM64 is the qualified host. Linux x86_64 and Windows x86_64 host
  detection, library naming and ZIP platform naming are implemented but not
  executed on a native host in this phase.
- The macOS library is ad-hoc signed, which is sufficient for local loading but
  not for notarized distribution. The current dyld's LINKEDIT alignment rules
  are handled by the builder; older macOS versions are not exercised.
- The Kotlin standard library is not bundled; only the profile JAR and JNA are.
- Bundles are local build artifacts. There is no publishing, distribution
  signing, fat JAR or CI integration in this phase.
- Reproducibility is byte-for-byte for clean rebuilds on the same host and
  toolchain. It is not a cross-platform or cross-toolchain guarantee; the
  deflate stream depends on the host zlib.

### Qualify existing bundles in isolation (NB-3.5B.2)

From the repository root:

```sh
just uniffi-kotlin-bundle-consumer
# Override Python and/or the existing ZIP directory:
just uniffi-kotlin-bundle-consumer python3.13 /absolute/path/to/dist
# Direct equivalent, including report/temporary-directory options:
python3.13 bindings/kotlin/qualify_bundle_consumer.py --dist /absolute/path/to/dist \
  --report target/uniffi-kotlin-bundle-consumer/qualification.json --keep
```

Prerequisites are Python **3.11+**, JDK **17+**, the **Kotlin 2.4.21** command-line
compiler (`kotlinc`) with its matching `kotlin-stdlib.jar`, and one already-built
ZIP per Core/Flow/Fuse profile for the current host. The compiler version must
match the bundles' metadata. Put `java` and `kotlinc` on PATH, or set `JAVA` and
`KOTLINC` to absolute executable paths. Set `JAVA_HOME` consistently for the
compiler launcher; `KOTLIN_HOME` can locate a matching stdlib installation.
For example, with Homebrew's JDK 17 on macOS ARM64:

```sh
export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
export PATH="$JAVA_HOME/bin:$PATH"
just uniffi-kotlin-bundle-consumer python3.13
```

This command does **not** run Cargo, UniFFI generation, Gradle or the bundle
builder, and does not modify or rebuild the existing ZIPs, JARs or native
libraries. It requires no dependency downloads: JNA comes from each bundle and
the Kotlin stdlib is staged from the local compiler installation.

The driver verifies ZIP layout/checksums and payload metadata, extracts each
bundle into a fresh temporary directory outside the checkout, re-verifies the
extracted files and compiles the minimal `verify/consumer` Kotlin sources.
Fresh JVMs load only the extracted profile JAR/JNA, the staged stdlib and the
compiled consumer classes. Each positive consumer proves the actual native
library path: Core scans a synthetic token, Flow applies Redact to scanner
findings, and Fuse compares parallel and sequential scans. This is a focused
distribution-consumer check, separate from the full fixture/oracle smokes.

All three profiles must also reject these deliberate negative cases:

- Missing native library: nonzero exit with `UnsatisfiedLinkError` and
  `cribra_uniffi` in the diagnostic.
- Invalid native library: the same expected loader rejection.
- Mixed profile classpath (Core+Flow, Flow+Fuse, Fuse+Core): nonzero exit with
  the guard diagnostics `cannot share a classpath` and `exactly one Cribra`.

These nine failures are **expected successes of the qualification**. Normal
output prints one rejection summary per case instead of Java stack traces.
Unexpected success, wrong diagnostics, signal termination or a failed positive
consumer fails the command; unexpected failure diagnostics remain visible.

On complete success, the driver atomically writes machine-readable evidence to
ignored `target/uniffi-kotlin-bundle-consumer/qualification.json` (overridable
with `--report`, outside the ZIP directory). It preserves bundle/native hashes,
tool versions, source hashes, classpaths, loaded native paths and exit codes,
plus full stdout/stderr and matched markers for every expected rejection.
The report represents the last successful run; a failed run does not replace
it. Temporary directories are removed on success unless `--keep` is supplied;
they are retained on failure. Paths recorded for cleaned temporary files are
historical evidence, not persistent artifacts.

Qualified on **macOS ARM64** with Kotlin **2.4.21** and JDK **17**. Other hosts,
JDK versions, Android, Gradle integration and publishing are not qualified by
this command. Runtime classpaths and working directories are isolated; this is
not an OS sandbox denying all repository access.

### Multiplatform CI (NB-3.6A)

`.github/workflows/uniffi-kotlin.yml` builds and qualifies bundles on separate
native runners: `macos-15` (ARM64), `ubuntu-24.04` (x86_64) and `windows-2022`
(x86_64). It runs on relevant pull requests, relevant pushes to `main`, and
manual `workflow_dispatch`. Each job installs Rust **1.97.0**, Temurin JDK **17**,
Python **3.13** and the checksum-pinned official Kotlin **2.4.21** compiler ZIP.

Every job calls `build_bundle.py` for all three profiles, then
`qualify_bundle_consumer.py`. Native libraries and generated bindings remain in
the existing separate Core/Flow/Fuse Cargo trees. Windows uses the checked-in
`gradlew.bat`; Python enables shell handling only for Windows `.bat`/`.cmd`
launchers, including `kotlinc.bat`. Native executables and POSIX launchers retain
shell-free execution. The existing macOS Mach-O alignment pass is unchanged.

The workflow uploads `cribra-kotlin-<platform>` diagnostic artifacts even when a
job fails: available distribution ZIPs, the successful qualification JSON, and
`build.log`/`consumer.log` under `target/uniffi-kotlin-bundle-consumer/`. A build
failure prevents consumer execution; a failed qualification may have a log but
no JSON report. It does not publish a release or package.

Local macOS checks are evidence only for the local host. The new workflow's
GitHub-hosted macOS, Linux and Windows jobs must actually complete before those
runners are considered qualified; Windows batch dispatch checks on macOS are
mock-based and do not replace Windows execution.

## What the smokes exercise

`Smoke.kt` (Core) runs against the real native library (JNA direct mapping, not a
mock):

- `cribraVersion()` and the top-level `smokeScanText()` Core entry points.
- Built-in detection of a synthetic GitHub classic PAT with exact UTF-8 byte
  offsets and Unicode-scalar line/column over a CRLF + multi-byte prefix.
- A reusable `Scanner()` object: `scan`/`scanMany` parity, Unicode and duplicate
  source IDs, empty input and reuse without retained state.
- `Scanner.configured(ScannerConfig.CustomOnly(...))`: custom-only rules,
  `activeRules()` metadata and a real scan.
- Candidate-only review semantics (a candidate is not a clean source).

`FlowSmoke.kt` (Flow/Share) replays the checked-in fixtures against the Rust
oracle, comparing outputs structurally (numbers by value, text by UTF-8 bytes):

- **Redact/Template**: `crates/cribra-uniffi/tests/fixtures/flow.json` (23 cases)
  against the `flow_oracle` `redactTemplate` output; UTF-8 boundaries, overlap
  merging/rejection, empty and reversed spans, `u64` max ranges, replacement and
  namespace/numbered options, and structured `TransformException` errors.
- **Keyed**: the 23 cases plus `keyed-flow.json` (27 inputs) across all 9
  `keyed-options.json` configurations (486 pseudonymize/synthesize outcomes),
  with repeated-call determinism, invalid key lengths, digest clamping to 8..32,
  key sensitivity, content independence, no key echo and no implicit rescan.
- **Share**: `crates/cribra-uniffi/tests/fixtures/share.json` (5 batches) across
  all four `ShareMode`s and both the atomic `scanAndBuild` and explicit `build`
  paths, plus the 23 explicit `flow.json` cases (40 + 92 outcomes). It checks
  transformed sources, `ShareModeKind`, the full `ScanSummary` counters, the
  in-range manifest timestamp, review-only candidates, Unicode/duplicate IDs and
  structured `ShareException` errors (`SourceCountMismatch`,
  `SourceLengthMismatch`, `MissingShareMode`, `InvalidKeyLength`).

`FuseSmoke.kt` (Fuse) replays `crates/cribra-uniffi/tests/fixtures/parallel.json`
against the `parallel_oracle` example output, exercising Core-owned parallel
scanning across all four configurations (`builtInOnly`, `customOnly`,
`builtInAndCustom`, `emptyRules`) and the fixture's empty/single/multiple/reversed/
64-source batches (44 scenarios × 8 repeats). For each run it checks
`parallelScan`/`scanMany` sequential-parity, deterministic ordering, every report
and finding field, the UTF-8 byte length of each source, duplicate-Unicode-ID
ordering, candidate-only review semantics, the full `ScanSummary` counters,
UTF-8 coordinates, and that results stay usable after the `Scanner` is closed.

Any failed `check` throws and the JVM exits non-zero, so the Gradle task fails the
build. To confirm the library is genuinely loaded, point JNA away from it:

```sh
bindings/kotlin/gradlew -p bindings/kotlin runFlow -Pcribra.kotlin.native=/nonexistent-dir
# java.lang.UnsatisfiedLinkError: Unable to load library 'cribra_uniffi'
```

## Boundaries

Only the Core, Flow/Share and Fuse profiles are generated, exercised and packaged.
There is no Android, Kotlin Multiplatform or Maven Central (or other) publishing:
the profile JARs and CI bundles are build/diagnostic artifacts. They are
thin JARs, so native libraries and third-party dependencies stay external. Fuse
scheduling is Core-owned: the Kotlin consumer calls `parallelScan` and never
implements thread pools, chunking or a sequential fallback. The generated
bindings, oracle outputs, JARs and native library are never vendored or committed;
regenerate them with the `just` recipes. The Rust Core and UniFFI API are
unchanged.
