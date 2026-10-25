# Compatibility and distribution status

Evidence reviewed on 2026-10-20; P0.4/P0.5 WASM source streaming and incremental
scanning, P0.6A/P0.6B/P0.6C adapter configuration/metadata/batch scanning and
P0.6D Flow Share Bundles locally checked on 2026-10-22.
This is a snapshot of distinct contracts:
**implemented** means an API/build path exists; **locally qualified** means a
consumer ran on the recorded host; **CI-qualified** means the named remote job
completed successfully. Neither a successful build nor an uploaded artifact is
package publication. See [Getting started](GETTING_STARTED.md) for entry points.

## Versions and publication

| Component | Source contract | Verified publication / distribution |
| --- | --- | --- |
| Rust Core `cribra` | 0.7.2; MSRV 1.97 | Latest [Core GitHub release](https://github.com/Metalymph/cribra/releases/tag/v0.7.2); independently verified 0.7.2 on crates.io |
| CLI `cribra-cli` / executable `cribra` | 0.4.4; workspace currently uses Core `^0.7` | Source build available; no CLI record found in the official registry index at this audit; Core releases do not establish CLI publication |
| C adapter `cribra-capi` | 0.3.0; native ABI protocol 0.3.0 | Source-built dynamic/static libraries and checked-in header; Cargo `publish = false` |
| WASM `cribra-wasm` | 0.4.7; workspace currently uses Core `^0.7` | [Independent GitHub release](https://github.com/Metalymph/cribra/releases/tag/cribra-wasm-v0.4.7); [registry 0.4.7](https://github.com/rust-lang/crates.io-index/blob/master/cr/ib/cribra-wasm) depends on Core `^0.6`. Published and workspace dependency contracts differ |
| Shared `cribra-uniffi` | 0.1.0; Core 0.7.2; UniFFI exactly 0.32.2; Rust 1.97 | Cargo `publish = false`; shared generator/adapter, not a second detection engine |
| Swift | Product/module `Cribra`; Core/Flow/Fuse local packages | Generated Swift/headers/static archives/XCFrameworks; local SwiftPM dependency, no remote SwiftPM publication established |
| Python | Fuse wheel `cribra` 0.7.2+fuse; module `cribra_uniffi` | Local/CI wheels; no PyPI publication established |
| Kotlin/JVM | Profile JAR/ZIP names use Core 0.7.2; adapter 0.1.0 | Local and CI host ZIPs; no Maven Central or other package publication established |

Crates.io publication was checked separately from GitHub: Core via the sparse
index, WASM via the official Git index; the CLI index path returned 404.
GitHub Core v0.7.0 and v0.7.1 releases have no attached binary assets. Generated
binding outputs are ignored build products; source-controlled packaging does not
make a checkout immediately usable as a prebuilt binary dependency. CI diagnostic
artifacts have retention limits and are not permanent release downloads.

The TypeScript WASM wrapper, React Native UniFFI integration, and C#,
Go and PHP C ABI bindings are planned workstreams. They are not included
among the implemented or qualified adapter surfaces above.

## Feature and capability boundaries

| Surface | Core | Flow | Fuse |
| --- | --- | --- | --- |
| Rust | Scan/report, built-ins/custom rules, stream and incremental | Default `transform`: Core plus four transforms and Share Bundles | Flow plus `serde` and `parallel` |
| UniFFI → Swift/Python/Kotlin | `core`: Scanner/configuration, batch, stream, incremental/atomic snapshot | `flow`: Core plus Redact, Template, Pseudonymize, Synthesize and Share Bundles | `fuse`: Flow plus Core-owned parallel scanning |
| WASM | `--no-default-features`: scanning/reporting, sequential multi-source batches, admitted byte streaming and incremental documents/snapshots | Default `transform`: transformations plus Core-backed in-memory Share Bundles; published distribution profile | Not supported |

Rust Core/Flow/Fuse are feature combinations, not Cargo features named `core`,
`flow` and `fuse`; those names are explicit UniFFI features. The C ABI 0.3.0
adds Core-owned incremental documents, UTF-8 byte edits, four optional budgets
and coherent revision/byte-count/report snapshots. Ordinary Core terminal failures
preserve the committed snapshot; contained panics invalidate the adapter session.
The 0.2.0 Core-backed byte streaming API and its admission rules remain unchanged.
WASM source now exposes Core-owned byte streaming in both Core and Flow, with
three independently optional budgets and typed terminal errors. Core-owned
incremental WASM documents add exact bigint revisions, UTF-8 byte edits, four
independent budgets and owned committed snapshots, available after terminal
Core errors. WASM sequential batches add one Core-owned ordered `Scanner::scan()`
invocation with owned entries and Core summary projections. These source
additions do not establish published session APIs.
The CLI retains its own surface. See
[CLI parity](CLI_PARITY.md), [C ABI](INTEROP.md),
[WASM](../crates/cribra-wasm/README.md) and [UniFFI](../crates/cribra-uniffi/README.md).

P0.6A source configuration adds independently selected current/financial/personal
catalogs to UniFFI Core/Flow/Fuse and optional Core remediation to all four WASM
custom-rule families in Core/Flow. Legacy configuration variants, builder methods
and WASM enum values remain present. UniFFI's appended `ScannerConfig.Catalogs`
case requires regenerated matching bindings/library and a new branch in exhaustive
consumer switches. Core exclusively owns composition, ordering, collisions and
rule validation. No default catalog selection changes.

Local configuration qualification uses generated Swift 6.4, Python 3.13.7 and
Kotlin 2.4.21/JDK 17 consumers for each UniFFI profile (21 successful configurations,
19 Core errors per profile/language). WASM Core/Flow run on Node 26.12.0 and Bun
1.4.3, comparing 132 scan/stream/incremental reports across 44 remediation
configurations per variant/runtime against Core. These are macOS ARM64 source
checks, not new CI, browser, platform or published-package qualifications.

P0.6B source adds native C scanner metadata count/index views and a structured
construction-error view on the existing opaque error handle. Protocol 0.3.0,
prior symbols, status numbers, existing layouts and messages are retained; the
known remediation code `ReviewPasswordVerifier = 7` now replaces the prior
`Unknown` projection for that Core value. New C views borrow their scanner/error
owner. WASM Core/Flow add owned `activeRules()` views and Core-derived `code` /
optional `ruleId` properties on the existing garbage-collected JS build Errors;
legacy signatures, messages and enum values remain. Regenerate bindings/header
from matching source. No Core, UniFFI or runtime dependency changes are involved.
P0.6B local checks cover C11 plus C/C++ examples, header compatibility/determinism
and symbol parity; Node/Bun exercise 16 metadata configurations and 14 structured
construction failures per WASM variant/runtime, alongside existing regressions.
These macOS ARM64 checks do not qualify Linux/Windows or browser sessions.

P0.6C adds the additive WASM `ScanEngine.scanBatch()` API in Core and Flow. Its
input is a type-only array of explicitly identified UTF-8 sources; the adapter
delegates the complete ordered batch to one Core `Scanner::scan()` call. New
owned `ScanBatchResult`, `ScanBatchEntry` and `ScanSummaryView` objects expose
entries, existing `ScanResult` projections and every Core summary field with
independent `free()` ownership. Existing WASM exports, enum values, signatures
and defaults are unchanged, and no dependency, thread or parallel path is added.
Local Node/Bun qualification compares eight batch fixtures and 20 ordered entries
per variant/runtime against a direct-Core oracle, including single-source
`scan()` parity, empty/Unicode/duplicate IDs, UTF-8 byte lengths, findings,
candidates, all summary fields, invalid inputs, repeated scans and independent
lifetimes. These are macOS ARM64 generated-runtime checks, not new CI, browser or
published-package qualifications.

P0.6D adds Flow-only WASM Share Bundles through an additive `ShareBundleBuilder`
and owned `ShareBundle`, `ShareBundleEntry` and `ShareManifestView` objects.
Core WASM omits these classes and `ShareModeKind`. The adapter delegates the
complete ordered source collection to Core
`ShareBundleBuilder::scan_and_build_with_generated_at()`, preserving Core modes,
defaults, validation, source pairing, transformations, summary and manifest
semantics. Redact, default template, keyed pseudonymization and keyed synthesis
are exposed; Core has no per-bundle template options or archive/serialization
operation, so the adapter does not invent them.

`wasm32-unknown-unknown` has no working `SystemTime::now()`. Core adds the
additive timestamp-injecting entry point while retaining the original
`scan_and_build()` and `build()` behavior and timing. Flow obtains the host clock
through `Date.now()` and passes it to Core, which stores it verbatim. No manifest
or transformation logic is duplicated in the adapter. Bundle outputs own cloned
IDs, transformed content and metadata, retain no original source text, and
outlive builders, engines and bundles with explicit `free()` lifetimes.

Local Node/Bun qualification compares 11 Share Bundle collections, 18 ordered
entries and 11 manifests per Flow runtime against a direct-Core oracle,
including all modes, keyed defaults/clamping, clean/finding/candidate-only
sources, Unicode IDs/content, empty and duplicate IDs, missing mode, invalid keys
and inputs, repeated construction and independent lifetimes. Timestamps are
checked against the invocation window rather than compared across independent
runs; Core WASM is checked for export absence. These are macOS ARM64
generated-runtime checks, not new CI, browser or published-package
qualifications.

Use one matching generated binding/native library/profile set per process.
Kotlin additionally requires one profile JAR per classpath; Swift requires one
package exposing `Cribra`. Runtime checksums and profile guards do not authorize
mixing builds. All surfaces preserve UTF-8 byte spans and the distinction between
findings and review-only candidates. Streaming admission and resource limits
remain Core-owned; a default scanner is not automatically streamable.

## W2.6.6 local integration qualification — 2026-10-25

The following checks were completed against the local source checkout.
They supplement the historical CI receipts above and below; they do not
establish new registry publication or universal platform compatibility.

| Surface | Newly verified local evidence |
| --- | --- |
| Rust | Public API contract tests passed with minimal, default and all-features configurations. Three independent external Cargo consumers passed for the exercised APIs. |
| C ABI | Header regeneration matched the checked-in header; C11 smoke, static-library and symbol gates passed. The macOS ARM64 dynamic library exposed 57 checked symbols. An external C11 consumer compiled and ran using a copied public header and dynamic library, confirming ABI 0.3.0. |
| WASM | `wasm-release-gate` passed, including Rust/WASM parity and validation of the Binaryen `-Oz` production artifact. Node.js 26.12.1 Core/Flow runtime qualification passed generated JS/TypeScript contracts and direct-Core oracles for streaming, incremental documents, batch scans, configuration, remediation and metadata; Flow additionally passed Share Bundle checks. The five-file local production package totaled 1,551,414 bytes. Browser session execution and publication of these workspace additions were not established. |
| Swift | Core/Flow/Fuse Apple build, inspection and independent SwiftPM consumer gates passed on macOS ARM64. iOS Simulator compilation/linking was checked, not Simulator or device execution. |
| Python | A locally built macOS ARM64 Fuse wheel `cribra` 0.7.2+fuse passed isolated installed-wheel tests on CPython 3.11.17, 3.12.15, 3.13.7 and 3.14.8. This does not establish PyPI publication or execution on every macOS deployment target. |
| Kotlin/JVM | Core/Flow/Fuse generated bindings, JARs and macOS ARM64 host bundles passed the repository gates. Isolated bundle consumers passed three positive profile checks and nine expected rejection checks with Kotlin 2.4.21/JDK 17. |

These results qualify the tested interfaces and environments, not complete
cross-language API equivalence. Core/Flow/Fuse remain capability profiles
with adapter-specific availability and distribution boundaries.

## Qualification and platform limits

These are recorded environments, not inferred compatibility ranges. The linked
consumer guides retain the detailed fixture, ownership, error and parity scope.

| Surface | Local evidence | Completed CI evidence / limits |
| --- | --- | --- |
| Rust / C | Existing release qualification and native examples | [Engine CI](https://github.com/Metalymph/cribra/actions/runs/39411373054): Linux full gate, native Linux ARM64, RISC-V64 under QEMU, macOS/Windows tests; native C ABI gates on Ubuntu/macOS/Windows. This does not qualify every language binding on those platforms |
| WASM | Existing production browser and Node 26.9.0 / Bun 1.4.2 / Deno 2.9.7 consumers. P0.4/P0.5 source sessions, P0.6C batches and P0.6D Flow Share Bundles: local macOS ARM64, Node 26.12.0 / Bun 1.4.3, generated Core/Flow contracts; 216 streaming reports plus 77 incremental snapshots across 68 edit attempts plus 8 batch cases / 20 ordered entries per variant/runtime, plus 11 Share Bundle collections / 18 ordered entries per Flow runtime, against direct-Core oracles | [WASM CI](https://github.com/Metalymph/cribra/actions/runs/39411373136): earlier wasm32 gate and Core/Flow JS/TypeScript contracts; predates P0.4/P0.5 sessions. No Fuse; session browser execution and Deno are not qualified by this local run. Browser lifecycle, Workers and CSP remain caller concerns |
| Swift | macOS ARM64, Swift 6.4; external macOS consumer runs; real ARM64 device and Simulator archives built | [Swift Apple integration](https://github.com/Metalymph/cribra/actions/runs/39411373180) passed all three packages/external consumers. iOS Simulator compile/link only; no simulator launch or device execution claim. Declared macOS 13/iOS 16 deployment minimums are not runtime tests on every OS. Intel slices are absent |
| Python | macOS 27.0.1 ARM64; installed Fuse wheel on CPython 3.11.17/3.12.15/3.13.7/3.14.8; source consumers Core 25, Flow 40, Fuse 43 tests | Linux x86_64 and repaired `manylinux_2_28_x86_64` installed-wheel gates passed in [engine CI](https://github.com/Metalymph/cribra/actions/runs/39411373054); native Windows x86_64 CPython 3.11–3.14 passed in [engine CI](https://github.com/Metalymph/cribra/actions/runs/39411373054). No musl, ARM64 Linux/Windows, Intel macOS or older macOS runtime qualification |
| Kotlin/JVM | macOS ARM64 fixture/oracle smokes and isolated bundle consumers | [NB-3.6A CI](https://github.com/Metalymph/cribra/actions/runs/39411373200), commit `0c7ac77f4e3eb959929977f3ecadbb9263824dcf`: macOS ARM64 (`macos-15`), Linux x86_64 (`ubuntu-24.04`), Windows x86_64 (`windows-2022`); Rust 1.97.0, Kotlin 2.4.21, JDK 17 |

Each Kotlin CI job builds/verifies separate Core/Flow/Fuse ZIPs and runs three
positive consumers plus nine expected rejections (missing native library,
invalid library, mixed-profile classpath). Uploaded `cribra-kotlin-<platform>`
artifacts contain bundles, logs and machine-readable qualification evidence.
This supersedes the historical macOS-only/mocked-Windows status. It does not
qualify Android, Kotlin Multiplatform, other architectures/JDK versions or
external Gradle application integration.

P0.7B.2 adds checked-in Kotlin configuration and session conformance to the three
Just smoke recipes and bundle-builder Gradle gate, using existing direct-Core
oracles. Each profile executes 21 successful configurations / 19 Core errors,
224 streaming report comparisons and 12 incremental trace snapshots across
seven accepted / four rejected edits, plus five explicit CRLF edits and all
session budgets/lifecycle checks. Generated interface signatures, DTO fields,
enum cases and profile gating are checked without public API changes. Local
macOS ARM64 execution is distinct from the historical CI receipts above; new
Linux/Windows CI execution and published-package qualification are not asserted.

For Python, the raw `linux_x86_64` wheel is distinct from the repaired manylinux
wheel; do not generalize it to all Linux distributions. macOS wheel metadata
names macOS 13 as the deployment minimum, but runtime was qualified on macOS 27.
Installed wheels require no Cargo/source checkout. Swift artifacts and Kotlin
ZIPs also have their own dependency/loader requirements; ad-hoc macOS signing is
not notarization or a signed distribution guarantee.

Detailed build/consumer procedures and remaining limits:
[Swift](../bindings/swift/README.md), [Python](../bindings/python/README.md),
[Kotlin/JVM](../bindings/kotlin/README.md). Historical release-preparation notes
remain historical evidence, not current publication statements. No PyPI, Maven
Central, remote SwiftPM or new release publication is asserted by this alignment.
