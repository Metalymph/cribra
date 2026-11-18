# Cribra React Native RN.1.3 — Qualification Record

Status: **RN.1.3 implemented and runtime-validated on the iOS simulator and the
Android emulator.** This is not a release-qualification claim, and it is not an
npm publication. Physical devices and release builds are `not-tested`.

Recorded: 2026-11-10. Package: `@metalymph/cribra-react-native@0.0.0` (private,
unpublished). ABI: `cribra-capi` 0.3.0 over Cribra Core 0.7.2.

RN.1.3 builds on the RN.1.2 native batch foundation (`feat/react-native-rn1-3`
from `676b93e`) and does not modify Cribra Core, the released C ABI, the
Swift/Kotlin/Python bindings, or the WebAssembly package. The vendored
`third_party/cribra/include/cribra.h` remains byte-identical to the canonical
`include/cribra.h`.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Jest facade batch/config contract and render | PASS | `npx jest --ci`; 3 suites, 89 tests |
| Host ABI, safe-integer, enum/explanation protocol | PASS | `npm run test:abi`; four `PASS` lines |
| Host scanner-configuration protocol | PASS | `Scanner configuration protocol: PASS` |
| C header matches canonical Core ABI header | PASS | `cmp include/cribra.h third_party/cribra/include/cribra.h` |
| iOS simulator build | PASS | `xcodebuild ... ** BUILD SUCCEEDED **` |
| iOS simulator runtime | PASS | rendered `config=ok` marker; ABI 0.3.0 |
| iOS physical-device runtime | not-tested | no device run |
| Android build, four ABIs | PASS | `BUILD SUCCESSFUL`; CMake for all four ABIs |
| Android emulator runtime (arm64-v8a) | PASS | logcat `config=ok` marker; ABI 0.3.0 |
| Android armeabi-v7a/x86/x86_64 runtime | not-tested | build-only ABIs |
| Android physical-device runtime | not-tested | no device run |
| Synthetic scan parity vs direct Core | PASS | full DTO tree comparison on both platforms |
| Configured metadata parity vs direct Core | PASS | 20-case config oracle on both platforms |
| Structured Core build failures | PASS | codes 1, 2, 4, 5, 6 with structured `ruleId` |
| Detached metadata after disposal | PASS | frozen snapshot readable after `dispose()` |
| Metadata-projection cleanup | PASS | mocked-DTO Jest cases; live count returns to baseline |
| Native batch semantics | PASS | empty batch, IDs/order, UTF-8 bytes, repeat determinism |
| Deterministic native negatives | PASS | invalid scanner ID and malformed batch elements |
| In-flight disposal and lifecycle | PASS | `lifecycle=ok`; batch completes before disposal |
| Runtime log privacy | PASS | no fixture secret, matcher payload, rule ID, or source ID |
| Release/npm qualification | not-tested | private package; no release build |

Runtime markers observed on both platforms:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:config=ok:negative=ok:privacy=ok
```

Android logcat:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:config=ok:negative=ok:privacy=ok
```

The iOS marker was rendered by the isolated harness and captured in
`/tmp/cribra-rn13-ios.png`. `simctl launch --console` does not capture React
Native `console.log` on this setup, so the on-screen marker is the iOS runtime
evidence. The Android marker was captured from logcat and matches the on-screen
marker in `/tmp/cribra-rn13-android.png`.

## Environment

| Component | Version |
| --- | --- |
| Host | macOS 27.2 (26B123) |
| Node / npm | 26.13.0 / 11.20.0 |
| Xcode | 27.2 (27B52) |
| iOS simulator runtime | iOS 27.0 (24A434), iPhone 18 Pro Max |
| Android emulator | `Pixel_10_clean`, Android 16, API 36, arm64-v8a |
| Android build tools / SDK | 37.0.0 / compileSdk 37, targetSdk 36, minSdk 24 |
| Android NDK | 27.1.12297006 |
| Gradle / Kotlin / JDK | 9.4.1 / 2.2.0 / 17.0.21 |
| Rust / cargo-ndk | 1.99.1 / 4.1.2 |
| React Native / React | 0.88.1 / 19.3.0 |
| React Native CLI | 20.2.0 |
| TypeScript | 6.0.3 |

The consumer enables the New Architecture (`newArchEnabled=true`) and Hermes
(`hermesEnabled=true`), and builds `armeabi-v7a,arm64-v8a,x86,x86_64`.

## Configured scanner implementation

`createScanner(config?)` is the only public factory. The previous
`createDefaultScanner` native method remains in the TurboModule spec for
compatibility but the facade no longer calls it.

1. The TypeScript facade validates only runtime shape: config object, catalog
   label, boolean flags, rule array, rule kind, string fields, severity label,
   and an input remediation. Rule semantics are not reimplemented.
2. The C++ method decodes the scalar configuration and copies every rule on the
   JS thread. No JSI object or borrow crosses the worker boundary.
3. One serial-worker job selects the requested Core catalogs, adds custom rules
   in caller order, builds the scanner, and projects every active rule's
   metadata into detached strings.
4. Metadata is validated and copied **before** the scanner ID is registered. A
   projection failure throws, the RAII scanner guard frees the unregistered
   scanner, and the JS thread receives a source-free error.
5. The facade validates the creation DTO. A success requires a positive scanner
   ID and empty failure fields. A failure requires ID 0 and empty rules.

The canonical default (`current` and no other catalog or rules) uses the
released `cribra_scanner_new_current` fast path, preserving RN.1.1/RN.1.2
behaviour and Core's precompiled current catalog. Every other configuration uses
`cribra_builder_*` with catalogs submitted current, financial, then personal.
Custom rules precede selected catalogs in the compiled metadata.

Builder ownership is explicit: a scope guard owns an unconsumed builder; early
catalog or rule errors free it; `cribra_builder_build` consumes it with a
writable `out_scanner` on success and failure, and the guard is marked consumed
immediately after that call so a consumed handle is never freed again.

## Configuration contract

| Field | Default | Meaning |
| --- | --- | --- |
| `builtins` | `'current'` | `'current'` or `'none'`; omitted or `{}` is the canonical current catalog |
| `financialBuiltins` | `false` | Opt-in financial catalog (3 rules) |
| `personalBuiltins` | `false` | Opt-in personal-identifier catalog (4 rules) |
| `rules` | `[]` | Custom rules in caller order |

Custom rule forms are `literal`, `prefix`, `suffix` (`value`), and `pattern`
(`pattern`, Rust `regex` syntax). Each has an `id`, `severity`, and optional
`remediation`. An omitted/null remediation means no remediation. The output-only
`'unknown'` remediation sentinel is rejected as input because Core rejects it.

Catalog sizes observed through Core: current 94, financial 3, personal 4, all
three 101. A current catalog plus one custom rule is 95. No catalog and no
rules is 0.

Core build failures are mapped from `cribra_error_build_info`, never by parsing
messages:

| Code | `CribraError.code` | Core-reachable through ABI 0.3.0 |
| --- | --- | --- |
| 1 | `invalid_pattern` | yes, `add_rule` |
| 2 | `pattern_matches_empty` | yes, `add_rule` |
| 3 | `missing_capture_group` | no; mapping-only (no public capture-rule construction) |
| 4 | `empty_rule_id` | yes, `build` |
| 5 | `duplicate_rule_id` | yes, `build` |
| 6 | `empty_matcher` | yes, `build` |
| 7 | `too_many_rules` | no; mapping-only today |
| 8 | `automaton_build` | no; mapping-only today |

An unknown or zero code fails closed as `scanner_create_failed`. When Core
reports a build error, `CribraError.ruleId` carries the structured caller rule
ID, or the public built-in ID for a catalog collision. The ID is never embedded
in the message or detail. Matcher, pattern, and source contents are never
exposed. The config oracle and Jest suites pin the reachable codes and rule IDs.

## Strict DTO and protocol validation

The RN.1.2 scan/enum validation is unchanged. RN.1.3 adds strict creation and
metadata validation:

- The creation DTO must have string `failure`, `ruleId`, and `detail`, and a
  non-negative safe-integer `buildCode`.
- A success requires `failure === ''`, a positive safe-integer `scannerId`,
  `buildCode === 0`, `ruleId === ''`, and an array of valid metadata entries.
- A failure requires `scannerId === 0` and an empty rules array. A positive ID
  in a failure DTO is treated as a protocol violation and asynchronously
  disposed so it cannot become an orphan.
- Every metadata entry requires a non-empty `ruleId` and a documented `kind`,
  `detectionMode`, `severity`, and `remediation`. Unknown labels fail closed as
  `invalid_native_report` with a `rule.*` field path.
- If metadata projection fails after a positive scanner ID is received, the
  facade awaits `disposeScanner` before rethrowing the original projection
  error. Cleanup failure is swallowed so it cannot mask that error.
  `getLiveScannerCount()` returns to baseline in these tests.

Unknown creation failure categories are normalized to
`scanner_create_failed`; an ABI mismatch keeps `code === 'abi_mismatch'`.
Native promise rejections are normalized as in RN.1.2: ABI mismatch passes
through and other native failures fold into `scanner_create_failed` with a
privacy-safe detail.

## Builds

### Native Rust artifacts

Core and the C ABI were unchanged, so the RN.1.1/RN.1.2 static libraries were
reused. Their on-disk sizes remain:

| Artifact | On-disk size |
| --- | --- |
| iOS device `libcribra_capi.a` (arm64) | 23 MB |
| iOS universal simulator `libcribra_capi.a` (arm64 + x86_64) | 45 MB |
| `ios/CribraCAPI.xcframework` | 68 MB |
| Android `armeabi-v7a` | 25 MB |
| Android `arm64-v8a` | 30 MB |
| Android `x86` | 24 MB |
| Android `x86_64` | 30 MB |

These are unstripped static libraries and are not release-size claims.

### Codegen

`pod install` regenerated `CribraReactNativeSpecJSI.h` for the new spec. The
generated `__createScanner` invoker has four arguments and passes the catalog
string, two booleans, and a `jsi::Array`. The generated DTO parameter counts are
five for `NativeCribraReactNativeRuleSpecDto` and
`NativeCribraReactNativeRuleMetadataDto`, and six for
`NativeCribraReactNativeScannerCreationDto`; the C++ aliases in
`cpp/CribraBridge.h` match those signatures and member names.

### iOS

The Debug simulator build succeeded:

```sh
xcodebuild -workspace CribraConsumer.xcworkspace -scheme CribraConsumer \
  -configuration Debug -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro Max' build
# ** BUILD SUCCEEDED **
```

The RN.1.3 build compiled the new `createScanner` bridge, rule metadata
projection, and creation DTO without C++ corrections after the host ABI test
passed. The runtime harness rendered the `config=ok` marker.

### Android

Four ABIs are intentional: the consumer keeps the React Native 0.88 template's
default `reactNativeArchitectures=armeabi-v7a,arm64-v8a,x86,x86_64`. Only
`arm64-v8a` was exercised at runtime. The other three were compiled and linked
but are **build-only** in this record.

| ABI | Built | Runtime-qualified |
| --- | --- | --- |
| `arm64-v8a` | PASS | PASS (Pixel_10_clean emulator) |
| `armeabi-v7a` | PASS | not-tested |
| `x86` | PASS | not-tested |
| `x86_64` | PASS | not-tested |

`./gradlew assembleDebug --no-daemon` finished:

```text
BUILD SUCCESSFUL in 30s
95 actionable tasks: 14 executed, 81 up-to-date
```

The Debug APK is 160,994,329 bytes across four ABIs. After the build, every
ABI's `libappmodules.so` contains the
`CribraReactNative::createScanner` symbol and the generated
`NativeCribraReactNativeScannerCreationDto` and
`NativeCribraReactNativeRuleMetadataDto` instantiations, confirming that the
configured-scanner bridge was compiled and linked into all four ABIs rather than
merely typechecked.

On iOS, the device `arm64` slice is build-only; the simulator `arm64` slice was
runtime-qualified, and the `x86_64` simulator slice is build-only.

## ABI and parity proof

### ABI

Both runtime markers report ABI `0.3.0`. `getAbiVersion()` calls
`cribra_abi_version_major/minor/patch`. The vendored header is byte-identical to
the canonical `include/cribra.h`.

Before any scanner construction or scan, `AbiCompatibilityFailure()` compares
the linked runtime version with the vendored header's
`ABI_VERSION_MAJOR/MINOR/PATCH`. Compatibility requires equal major and minor; a
patch difference is tolerated. A mismatch returns
`cribra_native_error:abi_mismatch:expected=<contract>:actual=<linked>`. Scanner
construction therefore fails before `cribra_scanner_new_current` or
`cribra_builder_new`; disposal remains ungated so existing handles can always be
released.

`npm run test:abi` exercises the pure headers on the host:

```text
ABI compatibility gate: PASS
JS safe-integer boundary: PASS
Strict enum/explanation projection: PASS
Scanner configuration protocol: PASS
```

The configuration protocol check pins the rule-kind, detection-mode, severity,
and remediation input/output vocabularies, the numeric build-code constants,
the `unknown` output-only remediation rule, and the canonical-default predicate.
Codes 3, 7, and 8 are mapping-only today; the host test verifies their numeric
and label mapping even though Core cannot currently produce them through ABI
0.3.0.

### Fixtures and oracles

- Scan fixture: `fixtures/synthetic-source.json`, a synthetic five-source
  fixture (217 UTF-8 bytes) containing empty IDs, a duplicate ID, one empty
  source, non-ASCII text, a multi-finding source, and one candidate. It contains
  no real credentials.
- Scan oracle: `tools/oracle` links `cribra` Core directly, bypasses the C ABI
  and TurboModule, and emits source-free metadata to
  `consumer/src/fixtures/synthetic-oracle.json`.
- Config fixtures: `consumer/src/fixtures/rn13-config-cases.json` contains 20
  cases: default, explicit empty, financial-only, personal-only, all catalogs,
  each custom rule form, all custom forms together, custom-before-catalog
  ordering, and nine failure cases.
- Config oracle: `cargo run --release -- --config-cases ...` emits
  `consumer/src/fixtures/rn13-oracle.json` with per-case rule metadata, scan
  output where present, and structured Core failure codes/rule IDs.
- Re-running both oracle modes produced byte-identical committed JSON files.

The config oracle observed metadata counts 94 (default), 0 (explicit empty), 3
(financial), 4 (personal), 101 (all catalogs), 1 for each single custom rule, 4
for all custom forms, and 95 for current plus one custom rule. Failure cases
covered codes 1, 2, 4, 5, and 6, including duplicate empty IDs resolving to code
5 with an empty rule ID and a custom/built-in collision on `github.classic-pat`.

The harness compares configured `scanner.rules` and, where the case has scan
sources, the complete batch result tree against the config oracle. Comparison is
strict on values, array order, and key sets; JSON object key order is ignored
because `serde_json` sorts keys while the DTO projection preserves constructor
order.

## Batch, configuration, and lifecycle evidence

The harness reports `batch=ok`, `config=ok`, `negative=ok`, `lifecycle=ok`,
`parity=ok`, and `privacy=ok` only after all of the following:

1. Initial live native scanner count is 0.
2. `createScanner()` succeeds and the live count becomes 1.
3. The five-source `scanBatch()` result matches the direct-Core oracle.
4. Entry IDs match the input, including empty and duplicate IDs, in order.
5. Each `sourceBytes` value equals an independent UTF-8 byte count, not a
   JavaScript string length; the summary total also matches.
6. `scanBatch([])` returns no entries and an exact all-zero, clean summary.
7. Repeating the same batch produces an identical projected result tree.
8. Invalid JavaScript batch input is rejected with `invalid_argument` before a
   native call, and the scanner remains usable.
9. Two concurrent `dispose()` calls share one native disposal, both resolve,
   and the live count returns to 0.
10. A later `dispose()` is a no-op, and `scanBatch()` after disposal throws
    `CribraError` with `code === 'disposed'`.
11. Native `scanBatch(999999, [])` rejects with `scanner_not_found`; malformed
    batch elements on a live scanner reject with `invalid_argument`; the
    scanner still returns an empty batch afterwards.
12. A batch dispatched immediately before `dispose()` still resolves with the
    full fixture result, while disposal drains the registry to 0.
13. A second scanner can be created (live count 1) and disposed (live count 0).
14. An explicit-empty configuration creates a zero-rule scanner; its frozen
    rules remain readable after disposal; and the live count returns to
    baseline.
15. Each successful config-oracle case (default, empty, financial, personal,
    all catalogs, every custom form, custom-before-catalog) matches Core rule
    metadata and scan output, and disposes back to baseline.
16. Each failure config-oracle case throws the expected structured
    `CribraError.code`, carries the expected `ruleId` (or none), does not put the
    ID in the message, and leaves the live count at baseline.
17. The emitted marker contains no fixture secret, candidate value, matcher
    payload, custom rule ID prefix, or source ID.

Native ownership:

- `CribraScanner`, `CribraReport`, and `CribraError` handles are wrapped in
  RAII guards; only the guards free them.
- `CribraBuilder` is owned by a guard that frees only an unconsumed builder.
  `cribra_builder_build` is called with a writable `out_scanner`, so the guard
  marks it consumed on both success and failure.
- The registry maps JS-safe numeric IDs to `shared_ptr<ScannerHandle>`; raw
  pointers never cross into JavaScript. A new scanner is registered only after
  its metadata has been validated and copied.
- The serial worker receives plain C++ strings, promises, and value structs,
  never JSI runtime objects.
- In-flight batch jobs hold a shared scanner reference, so disposal removes the
  registry entry but does not free the handle until the queued batch finishes.
- On module teardown the worker is destroyed first (declared last), drains
  queued serial work while the scanner registry is still alive, and joins its
  thread before the registry is destroyed.

`cribra_scanner_rule_at` is queried index by index. That is O(n²) for a full
metadata walk, which is accepted for the current maximum of ~101 active rules
during construction; no scan path performs a metadata walk.

## Defects and corrections during RN.1.3

1. **Metadata diagnostics reused finding field names.** The shared severity and
   remediation parsers initially reported `finding.severity` and
   `finding.remediation` while projecting rule metadata. They now accept an
   explicit field prefix, so metadata failures report `rule.severity` and
   `rule.remediation` without duplicating the mapping.
2. **`unknown` remediation was initially accepted as input.** The output sentinel
   `CRIBRA_REMEDIATION_UNKNOWN` is not valid configuration input: Core's
   `remediation_from_code` rejects it. The TypeScript validator and the C++
   `MapRemediationCode` both reject it now, and the host configuration test pins
   that rejection.
3. **Creation DTO inconsistency needed explicit cleanup.** A native failure DTO
   that still carries a positive scanner ID is a protocol violation, but the
   scanner would otherwise leak. The facade now extracts any positive safe ID
   and awaits disposal before propagating the projection error, without masking
   cleanup failures.

## Dependencies

- The package has **no runtime npm dependencies**. It declares peer dependencies
  only: `react >= 19.0.0` and `react-native >= 0.88.0 < 0.90.0`.
- The isolated consumer uses React Native 0.88.1, React 19.3.0, CLI 20.2.0,
  TypeScript 6.0.3, and `react-native-safe-area-context` 5.11.1.
- Rust side: `cribra-capi` 0.3.0 over Core `cribra` 0.7.2. No UniFFI, no
  WebAssembly runtime, and no second scanner engine.

## Limitations and not-claimed items

- No physical iOS or Android device was exercised. Both runtime proofs are
  simulator/emulator only.
- Android runtime qualification covers `arm64-v8a` only. `armeabi-v7a`, `x86`,
  and `x86_64` are build-only.
- No release build, archive, App Store, Play Store, or npm publication was
  exercised. All binary sizes are Debug/unstripped artifacts.
- The ABI gate is policy-tested on the host, not by loading a deliberately
  mismatched shared library; the package links the 0.3.0 static library.
- Core-reachable build failures are exercised against real Core on both
  platforms. Codes 3 (`missing_capture_group`), 7 (`too_many_rules`), and 8
  (`automaton_build`) are mapping-only through ABI 0.3.0: the public rule API
  exposes no capture-aware construction, Core does not currently produce
  `TooManyRules`, and an automaton build failure cannot be induced reliably.
  Their numeric code and label mapping is pinned by the host protocol test.
- Malformed creation DTOs, inconsistent success/failure payloads, and
  metadata-projection cleanup cannot be injected into the real TurboModule
  without changing Core or the ABI. They are covered by mocked-facade Jest tests
  that assert the original error is preserved and the live-scanner count returns
  to baseline. Native metadata projection is validated and copied before
  registration, so the native path cannot register a scanner with malformed
  metadata.
- `scanBatch()` does not use `cribra_scanner_scan_batch()`. It is one
  TurboModule call and one worker job, but the job contains N serial
  `cribra_scanner_scan()` calls to preserve explanations and summary fields.
  It stops at the first error and returns no partial batch.
- Native `numeric_overflow`, `invalid_native_report`, and ABI-mismatch paths are
  covered by host protocol tests and mocked-facade tests, not by injecting
  malformed values into a real Core scan.
- No performance, memory, startup, background/foreground, cancellation, or
  long-running stress measurements were made. Disposal waits behind queued
  scans; cancellation/stress/reload is RN.1.6.
- No Unicode helper surface in RN.1.3 (the UTF-8/UTF-16 boundary layer is
  RN.1.4), flow transforms (RN.1.5), streaming, incremental documents, Share
  Bundles, parallel scanning, key-based transformations, or Amon integration.
- The `needsReview` projection is findings **or** candidates, matching Core.
  Absent remediation is an empty wire string and is converted to `null` by the
  facade; rule metadata follows the same convention.
- Rule IDs are user-visible structured metadata and are deliberately exposed on
  `scanner.rules` and `CribraError.ruleId`. They are not logged by the package or
  the harness. Matcher, pattern, and source contents are never exposed.
- The fixture's `ghp_...`, `sk_test_...`, and `ABCD-EFGH-...` values, plus the
  custom-rule matcher payloads, are synthetic and contain no real credentials.

## Evidence locations

Build and runtime logs are ephemeral and intentionally not committed:

- `/tmp/cribra-ios-build-rn13.log` — successful iOS simulator build
- `/tmp/cribra-android-build-rn13.log` — successful four-ABI Android build
- `/tmp/cribra-android-install-rn13.log` — emulator APK install
- `/tmp/cribra-android-logcat-rn13.log` — Android logcat marker and privacy grep
- `/tmp/cribra-rn13-oracle-regen.json` — regenerated config oracle (byte-identical)
- `/tmp/cribra-rn13-ios.png` — iOS harness `config=ok` marker and counts
- `/tmp/cribra-rn13-android.png` — Android harness `config=ok` marker and counts

No source text, matcher payload, rule ID, or per-finding data is written to
logs. The registered marker is aggregate-only and the harness fails if a fixture
secret, matcher value, rule ID, or source ID appears in it.

# Cribra React Native RN.1.4 — Unicode Boundary Qualification

Status: **RN.1.4 implemented and runtime-validated on the iOS simulator and the
Android ARM64 emulator.** Not a release-qualification claim and not an npm
publication. Physical devices and release builds are `not-tested`.

Recorded: 2026-11-10. Package: `@metalymph/cribra-react-native@0.0.0` (private,
unpublished). ABI: `cribra-capi` 0.3.0 over Cribra Core 0.7.2. Baseline:
`47dd41a`.

RN.1.4 is TypeScript-only. It does not modify Cribra Core, the C ABI, the
vendored `third_party/cribra/include/cribra.h`, the TurboModule spec, the C++
module, or any platform project.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Jest suite, RN.1.1–RN.1.3 preserved | PASS | 6 suites, **184 tests** (89 existing + 95 new) |
| Deterministic Unicode offsets | PASS | direct expected literals, forward/reverse tables |
| Round-trip invariants | PASS | every scalar-boundary pair of 9 sample sources |
| Batch conversion order/duplicates/nesting | PASS | `byteSpansToUtf16Ranges` suite |
| Error precedence | PASS | malformed source wins over span/range errors in all three entry points |
| Malformed scan input invokes no native method | PASS | mocked TurboModule; `scan`/`scanBatch` assertions |
| Direct-Core span correspondence | PASS | every fixture finding and candidate |
| Host ABI protocol | PASS | 4 `PASS` lines, including scanner configuration |
| C header matches canonical Core ABI header | PASS | `cmp` identical |
| iOS simulator runtime | PASS | `offset=ok:probe=3/5` |
| Android ARM64 emulator runtime | PASS | `offset=ok:probe=3/5` |
| Hermes malformed-string behavior | RECORDED | numeric probe `3/5` (U+FFFD replacement) |
| JSC malformed-string behavior | source-derived | not runtime-tested |
| Runtime log privacy | PASS | `privacy=ok`; no source, matcher, rule ID, or source ID |
| Release/npm qualification | not-tested | private package |

Runtime markers observed:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:config=ok:negative=ok:offset=ok:probe=3/5:privacy=ok
```

## API surface

Added to `@metalymph/cribra-react-native`:

- `Utf16Range`
- `byteSpanToUtf16Range(source, span)`
- `utf16RangeToByteSpan(source, range)`
- `byteSpansToUtf16Ranges(source, spans)`
- `utf8ByteLength(source)`
- `CribraErrorCode` additions: `invalid_unicode_string`, `invalid_span_value`,
  `invalid_span_order`, `span_out_of_bounds`, `span_not_utf8_boundary`,
  `span_not_utf16_boundary`

`ByteSpan`, `Finding`, `Candidate`, `ScanResult`, and `ScanOptions` are
unchanged. No UTF-16 offsets are stored in native DTOs. There is no persistent
offset index, no grapheme segmentation, no normalization, and no native Unicode
operation.

`CribraError` and its messages moved to a native-independent `src/errors.ts`
verbatim apart from the additive codes; `src/index.ts` re-exports them.

## Preflight behavior

`scanner.scan()` and `scanner.scanBatch()` now reject any unpaired high or low
surrogate with `invalid_unicode_string` before a native call is made. Every
batch source is validated before the batch is dispatched. The mocked-TurboModule
tests assert `native.scan` and `native.scanBatch` are never called for malformed
input, including when the malformed source is last in a batch. Well-formed
supplementary characters still scan and report their exact UTF-8 length.

`report.sourceBytes === utf8ByteLength(source)` is documented as a necessary
consistency check, not proof that the content is unchanged. Consumers must
retain the exact string passed to the scan and compare it with the current
editor content. The test suite pins two different strings with the same UTF-8
byte length (`"é"` and `"Ā"`, both 2 bytes).

## Unicode findings

| Observation | Result |
| --- | --- |
| Hermes `"\uD83D"` through native `utf8()` | 3 bytes (U+FFFD replacement) |
| Hermes `"a\uD83Db"` through native `utf8()` | 5 bytes (U+FFFD replacement) |
| Public scan of the same malformed sources | rejected before native work |
| JSC | source-derived: strict conversion truncates at invalid UTF-16; not runtime-tested |

The probe is numeric-only; the synthetic text is never logged. `TextEncoder`
matches Hermes on unpaired surrogates but does not describe JSC, so it is not
used as an oracle.

Fixture correspondence is pinned directly: the first synthetic finding
`28..65` maps to UTF-16 `22..59`, and the slice has exactly 37 UTF-8 bytes.

## Performance

Preflight (`assertWellFormedUtf16`) is `O(n)`, allocation-free, and a
straightforward scalar walk. Measured with a 1 MiB ASCII source, warm, 5 runs:

| Environment | Preflight per MiB | Relative note |
| --- | --- | --- |
| Node 26.13 / V8 (host) | 1.66 ms | ~602 MiB/s |
| Android ARM64 emulator / Hermes debug | 33.2-33.7 ms | ~31 MiB/s |
| iOS simulator / Hermes debug | 54.6-60.4 ms | ~17-19 MiB/s |

Host measurements also covered 64 KiB ASCII (0.10 ms), 768 KiB UTF-8 CJK
(0.42 ms), and a 1.25 MiB mixed source (1.27 ms). Forward conversion of a 1 MiB
ASCII source costs 3.04 ms, reverse conversion of a 768 KiB UTF-8 CJK source
0.81 ms, and a 64-span batch over 1 MiB ASCII 3.30 ms on the host.

The preflight runs once per scanned source on the JS thread, before the async
native call. Large multi-megabyte documents will pay tens of milliseconds in
Hermes debug builds; typical credential-sized editor sources are in the low
milliseconds or below.

## Remaining limitations

- JSC is not runtime-tested. Its truncation behavior is derived from
  `ReactCommon/jsc/JSCRuntime.cpp` and is only relevant when Hermes is disabled.
- iOS physical devices, Android physical devices, release builds, and the
  armeabi-v7a/x86/x86_64 runtimes are `not-tested`.
- Preflight cost is linear and measurable on multi-megabyte sources.
- Grapheme clusters are intentionally unsupported; a range may split a
  combining sequence or a ZWJ emoji at a scalar boundary.
- No automatic redaction, highlighting, or flow transformation API is added.
- No persistent offset index is provided; batch conversion covers
  many-findings-per-source workloads.

## Evidence locations

- `/tmp/cribra-rn14-ios.png` — iOS harness `offset=ok:probe=3/5` marker
- `/tmp/cribra-ios-rn14.log` — iOS `log stream` capture with the numeric
  preflight observation
- `/tmp/cribra-metro-rn14.log` — Metro bundle log
- Android logcat (captured live) — `offset=ok:probe=3/5` and
  `CRIBRA_RN_PREFLIGHT:ms=33.201:bytes=1048576`

No source text, matcher payload, rule ID, or per-finding data appears in any
log or marker.

# Cribra React Native RN.1.5 — Flow Transformation Qualification

Status: **RN.1.5 implemented and runtime-validated on the iOS simulator and the
Android ARM64 emulator.** Not a release-qualification claim and not an npm
publication. Physical devices and release builds are `not-tested`.

Recorded: 2026-11-11. Package: `@metalymph/cribra-react-native@0.0.0` (private,
unpublished). ABI: `cribra-capi` 0.3.0 over Cribra Core 0.7.2. Branch:
`feat/react-native-rn1-5`. Baseline: `cafd6e3`.

RN.1.5 adds the four scan-coupled Core transformations to the native package.
It does not modify Cribra Core, the public C ABI, the vendored
`third_party/cribra/include/cribra.h`, the Swift/Kotlin/Python bindings, the
WebAssembly package, or CI. Transformations are delegated to the existing C ABI
entry points; neither TypeScript nor C++ contains replacement logic.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Jest suite, RN.1.1–RN.1.4 preserved | PASS | 8 suites, **263 tests** (184 baseline + 79 new) |
| Facade request validation and Core defaults | PASS | `transform.test.ts` |
| Key copying, caller immutability, JS temporary zeroization | PASS | `transform.test.ts` (success and rejection paths) |
| Malformed UTF-16 source and option preflight | PASS | `transform.test.ts`, `preflight.test.ts` |
| Exact fixed-diagnostic error classification | PASS | `transform.test.ts` |
| Direct-Core transform oracle integrity | PASS | `transform-oracle.test.ts`, 27 cases |
| Host ABI and transform signature protocol | PASS | `npm run test:abi`; five `PASS` lines |
| Vendored header matches canonical Core header | PASS | `cmp` identical |
| iOS simulator build and runtime | PASS | `transform=ok:key=ok` marker |
| Android ARM64 emulator build and runtime | PASS | `transform=ok:key=ok` marker |
| Android four-ABI build | PASS | all four `libappmodules.so` contain the transform path |
| Android non-ARM64 runtimes | build-only | armeabi-v7a/x86/x86_64 not executed |
| Runtime log privacy | PASS | `privacy=ok`; no output, key, source, matcher, or rule ID |
| Release/npm qualification | not-tested | private package |

Runtime markers observed (identical on both platforms):

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:config=ok:transform=ok:key=ok:negative=ok:offset=ok:probe=3/5:privacy=ok
```

## API surface

Added to `@metalymph/cribra-react-native`:

- `scanner.transform(source, request): Promise<TransformResult>`
- `TransformResult` — `{ scan: ScanResult; output: string }`, frozen
- Request types: `RedactRequest`, `TemplateRequest`, `PseudonymizeRequest`,
  `SynthesizeRequest`, `TransformRequest`
- `CribraErrorCode` additions: `invalid_key_length`, `transform_invalid_span`,
  `transform_invalid_utf8_boundary`, `transform_overlapping_spans`,
  `transform_unknown`

Defaults match the WebAssembly adapter: `"[REDACTED]"`, `"CRIBRA"`, `false`,
`"cribra_pseudo_"`, `16`, `"cribra_synthetic"`. Omitted and explicit-empty
`replacement`, `prefix`, and `marker` are distinct; a supplied namespace is
forwarded verbatim. `digestBytes` accepts any nonnegative safe integer up to
`0xFFFFFFFF`; Core clamps it.

No Share Bundle, streaming, incremental, editor, or cancellation API is added.
The public signature intentionally differs from `@metalymph/cribra` only in
being asynchronous.

## Native execution and ownership

One `transform()` call performs one TurboModule call and one serial worker job:

1. The facade validates the request, resolves defaults, preflights the source
   and every option string for unpaired surrogates, and validates the key.
2. Keyed modes copy the 32 key bytes into a fresh JavaScript
   `Uint8Array`/`ArrayBuffer`; the caller's array and backing store are never
   mutated. The temporary is zeroed in a `finally` block after the promise
   settles, on both success and rejection.
3. The JS thread decodes the source and options to UTF-8, validates the native
   `jsi::ArrayBuffer` (`detached`, size exactly 32, non-null data), and copies
   the key into one `std::shared_ptr<KeyBytes>`. No JSI object is retained.
4. `TransformInput` is non-copyable and owns the text and that single key
   owner. The worker lambda captures `std::shared_ptr<TransformInput>`, so the
   job is copyable without duplicating key material. `KeyBytes` volatile-zeroes
   its 32 bytes on destruction; the input destructor best-effort wipes its
   copied source.
5. The worker calls `cribra_scanner_scan()` once for the exact source, projects
   the complete report, then calls exactly one `cribra_transform_*()` against
   that same live `CribraReport`. The report remains alive through the call.
6. The owned `CribraOutput` is wrapped in RAII, its UTF-8 view is copied into
   the DTO, and the handle is freed exactly once. Empty output is supported.

The C ABI configuration structs borrow the key pointer/prefix/marker for only
the synchronous transformation call. Only one native key buffer exists per
request. Zeroization is mandatory for the facade temporary and the native key
buffer, and best-effort for the native copied transformation source. It cannot
erase the caller's array, Hermes/JSC string representations, immutable JSI
strings, or Core's transient buffers. Option strings are not wiped; like rule
specs they are treated as caller configuration, not scanned content.

Redaction merges overlapping finding spans; template, pseudonymize, and
synthesize reject them. Ambiguous candidates are never transformed. The
returned `scan` is the full Core-derived projection of the transformed source,
identical to a detached `scan()` of that string, and contains no `sourceId`.

## Error classification

The C ABI reports every Core transformation failure as
`CRIBRA_TRANSFORM_ERROR` (5) with a fixed privacy-safe diagnostic. The bridge
maps status 5 to the `transform_failed` token and emits
`cribra_native_error:transform_failed:<operation>:<isolated diagnostic>`. The
facade requires that exact token, captures the diagnostic with
`^cribra_native_error:transform_failed:[a-z_]+:(.*)$/s`, and classifies only an
exact `===` match:

| Diagnostic | Code |
| --- | --- |
| `transform report contains an invalid source span` | `transform_invalid_span` |
| `transform report span is not aligned to UTF-8 boundaries` | `transform_invalid_utf8_boundary` |
| `transform requires non-overlapping finding spans` | `transform_overlapping_spans` |
| `transform source length does not match report` | `transform_unknown` |
| `source transformation could not be applied safely` | `transform_unknown` |

Prefixes, suffixes, whitespace, different casing, a missing diagnostic, a
reworded diagnostic, or a different native status all fall back to
`transform_unknown`. No substring parsing exists. Native key-length failures use
`cribra_native_error:invalid_key_length`, which maps to `invalid_key_length` and
is raised before any worker job. Tests cover all five diagnostics, adversarial
variants, both fallbacks, the key token, and an unrelated native failure.

## Direct-Core parity

`tools/oracle` was extended with `--transform-cases`, which uses Cribra Core
directly (no C ABI, no TurboModule) to produce
`consumer/src/fixtures/rn15-transform-oracle.json` from 27 synthetic cases:

- every mode with default options and with custom options;
- empty and multibyte replacement, prefix, marker, and namespace values;
- clean sources and candidate-only sources (candidates preserved verbatim);
- deterministic keyed repeats for pseudonymization and synthesis;
- digest lengths below and above Core's clamp (`1` and `64`);
- real overlapping findings (`secret-value` + `secret`), merged by redaction and
  rejected by the other three modes;
- exact full scan projections and exact output strings.

The runtime harness runs all 27 cases on device: the 24 successes compare the
entire `scan` tree and the exact `output` against the oracle, then compare the
embedded scan with a detached scan of the same source; the 3 overlap cases must
reject with `transform_overlapping_spans`. Deterministic repeats are compared
pairwise. No fixture source, finding, candidate, output, or key is logged.

## Jest and host checks

- `npx jest --ci`: 8 suites, 263 tests, 0 failures.
- `npm run test:abi`: `ABI compatibility gate: PASS`,
  `JS safe-integer boundary: PASS`, `Strict enum/explanation projection: PASS`,
  `Transformation ABI signatures: PASS`, `Scanner configuration protocol: PASS`.
  The new check pins all six transform entry-point signatures, status 5, the
  three config struct layouts, and the output view/free contract.
- `cmp include/cribra.h third_party/cribra/include/cribra.h`: identical.
- Package `npm run typecheck` and consumer `npx tsc --noEmit`: clean.

## Builds and runtime

Environment: macOS with Xcode 27.2 (iOS 27.0 simulator), JDK 17, Gradle 9.4.1,
NDK 27.1.12297006, Rust 1.99.1, Node 26.13.0.

iOS:

- `xcodebuild -workspace ios/CribraConsumer.xcworkspace -scheme CribraConsumer
  -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 18 Pro
  Max' -derivedDataPath /tmp/cribra-rn15-ios build` succeeded.
- Generated Codegen confirms `jsi::ArrayBuffer` arguments and the flat
  `TransformResultDto` bridging with six properties.
- Installed and launched on the booted simulator; the screen showed `PASS` and
  the marker above. Screenshot: `/tmp/cribra-rn15-ios.png`. Build log:
  `/tmp/rn15-ios-build.log`.

Android:

- `./gradlew assembleDebug` succeeded and produced all four ABIs; the APK
  contains `libappmodules.so` for `armeabi-v7a`, `arm64-v8a`, `x86`, and
  `x86_64`.
- All four libraries contain the fixed transform diagnostics, the
  `invalid_key_length` token, and the compiled C++ transform methods, proving
  the transform path is linked into each ABI.
- Runtime: `emulator-5554`, `arm64-v8a`, Android 16. Installed the APK,
  reversed Metro port 8081, launched `com.cribraconsumer/.MainActivity`, and
  observed the marker in logcat. Screenshot: `/tmp/cribra-rn15-android.png`.
  Build log: `/tmp/rn15-android-build.log`.

Android ABI matrix:

| ABI | Build | Runtime-qualified | Evidence |
| --- | --- | --- | --- |
| `arm64-v8a` | PASS | PASS | emulator marker `transform=ok:key=ok` |
| `armeabi-v7a` | PASS | build-only | transform diagnostics present in `libappmodules.so` |
| `x86` | PASS | build-only | transform diagnostics present in `libappmodules.so` |
| `x86_64` | PASS | build-only | transform diagnostics present in `libappmodules.so` |

## Privacy evidence

The harness asserts that the emitted marker contains no transformed output
fragment, fixture secret, candidate value, matcher payload, rule ID, or source
ID; it passes `privacy=ok` on both platforms. Only aggregate counts and fixed
pass/fail reasons are logged. The new C++ code adds no logging. Native
transform diagnostics are fixed strings and never include source text, matched
values, keys, or per-finding metadata.

## Limitations and not-claimed items

- Private, unpublished package; no release or npm qualification.
- iOS/Android physical devices and release builds are `not-tested`.
- Only `arm64-v8a` is runtime-qualified on Android; the other three ABIs are
  build-only for this record.
- JSC behavior is not runtime-tested (the harness runs Hermes debug builds).
- Zeroization is best-effort beyond the two mandatory buffers; no claim is made
  about the JavaScript engine, JSI, Core, or caller memory.
- Error classification is a compatibility mapping over fixed C ABI strings; any
  future rewording intentionally degrades to `transform_unknown`.
- Transformations are scan-coupled and one-shot. Streaming, incremental
  documents, Share Bundles, editor integration, cancellation, RN.1.6 work, and
  Amon are out of scope.
- Keys are raw 32 bytes; no KDF, key storage, or KMS integration is provided.
- Preflight remains `O(n)` on the JS thread for the source plus each option.

## Evidence locations

- `/tmp/cribra-rn15-ios.png` — iOS harness PASS marker
- `/tmp/rn15-ios-build.log` — iOS build log
- `/tmp/cribra-rn15-android.png` — Android harness PASS marker
- `/tmp/rn15-android-build.log` — Android Gradle build log
- `/tmp/cribra-rn15-ios/` — iOS derived data containing generated Codegen
- `consumer/src/fixtures/rn15-transform-cases.json` — 27 synthetic cases
- `consumer/src/fixtures/rn15-transform-oracle.json` — direct-Core oracle

No commit, push, merge, tag, or publication was performed.

# Cribra React Native RN.1.6 — Lifecycle and Stress Hardening Qualification

Status: **RN.1.6 implemented and runtime-validated on the iOS Simulator
(Debug and Release) and the Android ARM64 emulator (Debug and Release).** Not
a release-qualification claim and not an npm publication. Physical devices,
iOS device builds, and sanitizer-instrumented app builds are `not-tested`;
sanitizer leak detection on macOS is `not-tested`.

Recorded: 2026-11-12. Package: `@metalymph/cribra-react-native@0.0.0`
(private, unpublished). ABI: `cribra-capi` 0.3.0 over Cribra Core 0.7.2.
Branch: `feat/react-native-rn1-6`. Baseline: `e1d77f6` (merged RN.1.5).

RN.1.6 hardens the existing native worker, module teardown, and host-side
qualification. It does not modify Cribra Core, the released C ABI, the
vendored `third_party/cribra/include/cribra.h`, the Swift/Kotlin/Python
bindings, the WebAssembly package, CI, dependencies, or publication metadata.
It adds no scanner feature, cancellation API, streaming, Share Bundle, editor
integration, or Amon surface.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Jest suite, RN.1.1–RN.1.5 preserved | PASS | 10 suites, **281 tests** (263 baseline + 18 new) |
| JSI-free host lifecycle suite | PASS | 15,736 checks, 0 failures |
| Host ASan / UBSan / TSan | PASS | ASan, UBSan and TSan runs, 0 failures |
| Host 10,000-job burst | PASS | 10,000 FIFO jobs, 0 lost, ~1 ms plain |
| Host queue discard | PASS | 5,000 queued transform jobs, 0 executed, ~6.8 ms plain |
| Concurrent post/stop | PASS | Four posters × 500 jobs; TSan reports no data race |
| ABI gate and transform/config protocol | PASS | `npm run test:abi`; five `PASS` lines |
| Vendored header identity | PASS | `cmp include/cribra.h third_party/.../cribra.h` |
| Direct-Core oracle fixtures | PASS | scan, configuration and transform oracles byte-identical |
| iOS Debug runtime marker | PASS | `…:stress=ok:recovery=ok` |
| iOS three full JS reloads | PASS | Same PID, live scanners 1 → 0, probe cleared |
| iOS Release Simulator build and runtime | PASS | `BUILD SUCCEEDED`; release marker PASS |
| Android Debug ARM64 runtime marker | PASS | `…:stress=ok:recovery=ok` from logcat |
| Android three full JS reloads | PASS | Same PID, live scanners 1 → 0, probe cleared |
| Android four-ABI Debug APK | PASS | `armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64` |
| Android four-ABI Release APK | PASS | `assembleRelease`; standalone release runtime marker PASS |
| ASan leak detection | not-tested | macOS ASan reports `detect_leaks` unsupported |

## Shutdown contract

The implemented native contract is `Running → Stopping → Stopped`:

1. the module destructor closes the settlement gate **before** stopping the
   worker, so no new JavaScript settlement can be scheduled;
2. `stop()` marks the worker stopping under the queue mutex and notifies it;
3. the worker finishes the job that is already running, including its native
   ABI work;
4. every queued job is destroyed on the worker without executing Core work;
5. the shutdown hook runs on the worker and clears the scanner registry, so
   the last scanner handles and discarded-job captures are released there;
6. the worker thread is joined. It is never detached, and the hook is never
   re-run on the caller's thread.

`CribraSerialWorker` is a plain header with no JSI or React Native dependency;
`CribraOwnership.h` is likewise JSI-free. The production module and the host
tests share `PostResult`, `PostFailureMessage`, `Settle`, `RejectNotAccepted`,
`CribraSerialWorker` and the ownership primitives, so the tests exercise the
same code the TurboModule ships.

`Settle` holds the gate mutex only across React Native's non-blocking
`invokeAsync` scheduling call. It never runs JavaScript under the mutex. This
is not a proof of runtime safety: React Native owns invalidation of callbacks
that were already scheduled when a runtime is destroyed.

### Refusal and enqueue failure

`post()` distinguishes three outcomes:

| Outcome | Meaning | Promise settlement |
| --- | --- | --- |
| `Accepted` | Stored in the queue; runs before shutdown completes | Job settles itself |
| `Refused` | Worker is stopping or stopped; job destroyed without running | Rejected on the JS thread |
| `EnqueueFailed` | The job could not be stored (allocation/synchronization) | Rejected on the JS thread |

All six production job methods (`createDefaultScanner`, `createScanner`,
`scan`, `scanBatch`, `runTransform` via the four transform entry points, and
`disposeScanner`) use the shared `RejectNotAccepted` helper. The fixed details
are `cribra_native_error:worker_stopped` and
`cribra_native_error:worker_unavailable`; both map to the existing public
`native_error` code. No new public error code was added.

The rejected lambda is destroyed before the caller rejects, so its scanner
reference or `TransformInput` (and therefore its key copy) is released first.
A job whose enqueue fails is never executed, and an accepted job is never
silently discarded unless shutdown has begun. Accepted jobs queued behind a
shutdown are discarded by design; their pending promises are abandoned (see
limitations).

## Findings fixed during RN.1.6

- **Shutdown hook could run on the caller thread.** An earlier `stop()`
  fallback re-ran the hook when a second call found the worker already
  stopping. That would have cleared the scanner registry off the worker.
  The fallback was removed: only the first caller stores the hook, and only the
  worker executes it. `TestDoubleStop` proves one call and no caller-thread run.
- **Enqueue failure was indistinguishable from shutdown refusal and could
  leave the promise unsettled.** `post()` now returns `Refused` or
  `EnqueueFailed`; every caller rejects through `RejectNotAccepted` with the
  fixed message before returning to JavaScript.
- **A null settlement gate could be dereferenced.** `Settle` now returns
  without action when the gate is absent; the destructor closes the gate under
  a contained lock.
- **Queue discard and registry cleanup were not provably on the worker.**
  `ClearScanners()` was moved into the worker shutdown hook, is `noexcept`,
  and swaps the registry under its mutex before releasing handles on the
  worker.
- **Compile failure in the hook aggregate.** `ShutdownHook` now has explicit
  `constexpr` constructors, fixing the complete-class-context default member
  initializer error without changing the allocation-free layout.
- **Host test probe counted a destroyed temporary.** The queue-discard probe
  originally copied a non-copyable guard into a shared pointer, incrementing
  the discard counter before shutdown. It now owns one heap probe per job.
- **Harness recovery check compared different strings.** The recovery phase
  compared the byte length of two different probe strings, producing a false
  failure. It now scans the same `recovery probe` string before and after each
  isolated native rejection.
- **Queued-key test kept an extra strong reference.** The test now drops its
  own `shared_ptr` and observes expiry, proving the queued job owned the last
  reference when the worker discarded it.
- **Host burst and creation/disposal waits could lose the final notification.**
  A worker job notified its completion condition variable without holding the
  waiter's mutex, so the last transition could be missed between the predicate
  check and the sleep. Both waits now notify under that mutex. The product code
  was unaffected; only the test synchronization was wrong.
- **The "no JSI crosses the worker" claim was imprecise.** The queue does hold
  the `AsyncPromise` settlement handle. Documentation now states the real
  guarantee: no raw JSI value is dereferenced on the worker, and settlement is
  scheduled through the `CallInvoker` with RN's weak callback wrappers.

## Thread and ownership analysis

- **JS thread** reads JSI values, copies configuration/strings/keys into plain
  C++ values, constructs `AsyncPromise`, and returns the JS promise. It never
  runs Core work and never holds a raw C ABI handle.
- **One serial worker thread** is created when the TurboModule is constructed.
  Every Core and C ABI call runs there. No job spawns a thread. The host suite
  asserts each job runs on the captured worker thread.
- **Queue ownership**: each `std::function` owns plain inputs, a
  `shared_ptr<ScannerHandle>`, and (for transforms) a shared
  `shared_ptr<TransformInput>` whose `KeyBytes` owner is unique. It also holds
  the `AsyncPromise` settlement handle. No raw JSI value is dereferenced on the
  worker: `AsyncPromise` keeps only weak callback wrappers and schedules
  settlement through the `CallInvoker`.
- **Registry ownership**: `scanners_` is guarded by `scannersMutex_`.
  Registration and disposal happen on the worker; lookup copies a
  `shared_ptr` on the JS thread. The module destructor's hook swaps and clears
  the map on the worker, so the final frees occur there. The host registry test
  creates and frees 200 real scanners on the worker with `offThreadFrees == 0`.
- **Hook allocation**: the shutdown hook is a plain function pointer plus a
  `void*` context, so shutdown cannot fail on `std::function` allocation.
- **Ordering**: `settlementGate_` is declared before `worker_`, and the worker
  is declared last, so the worker is joined during the destructor body before
  the gate and registry members are destroyed. A second `stop()` (from
  `~CribraSerialWorker`) only joins an already-finished thread.
- **No JSI in teardown**: the destructor and worker only touch the gate,
  queue, registry, and C ABI handles. They never read or create a JSI value.

## Key evidence and limitations

Key semantics preserved from RN.1.5:

| Path | Evidence |
| --- | --- |
| Caller's `Uint8Array` never mutated | Jest 1,000 keyed transforms; runtime probes report `keyIntact: true` |
| JS temporary zeroed after normal settle | Jest `transform.test.ts` and 1,000-transform stress |
| JS temporary zeroed after native rejection/refusal | Jest lifecycle and refused-transform stress tests |
| Native `KeyBytes` zeroed after run, throw, or queue discard | Host slab test and custom-deleter discard test |
| Copied source text wiped when `TransformInput` dies | Host allocation-watch test |
| No duplicate native key copies | One non-copyable `KeyBytes` per job, shared by the copyable lambda |
| Discard happens on the worker | Host queued-key discard test checks the worker thread ID |

Limitations, stated without over-claiming:

- On a full JavaScript reload, the old runtime is destroyed without running
  promise continuations or `finally` blocks. A JavaScript key temporary and
  any in-flight promise are abandoned; they cannot be explicitly zeroed or
  settled from the SDK. Native queue discard and key zeroization still run on
  the worker, but the process-level runtime teardown is outside SDK control.
- Zeroization is best effort. Compiler reordering is prevented with a volatile
  loop, but Core, Hermes and the operating system may retain copies outside
  the buffers this package owns.
- `TransformInput` wipes the copied source and the key; `option` strings
  (replacement, namespace, prefix, marker) are not wiped because they are
  caller-supplied, source-free transform metadata.
- Accepted work queued behind a shutdown is deliberately discarded (D1); a
  caller awaiting that promise after a reload will never observe a settlement.
- Queue discard is O(N). A very large queued backlog lengthens the destructor
  call. There is no hard latency bound.

## Sanitizer results

`bash tools/host-tests/run.sh` supports `plain`, `asan`, `ubsan` and `tsan`.
All four completed on the qualification host:

| Mode | Result | Checks | Burst | Shutdown measurement |
| --- | --- | --- | --- | --- |
| plain | PASS | 15,736 / 0 failures | 10,000 jobs in 0.5 ms | idle 0.010 ms; in-flight + 5,000 queued in 6.161 ms |
| asan | PASS | 15,736 / 0 failures | 10,000 jobs in 2.3 ms | in-flight + 5,000 queued in 13.461 ms |
| ubsan | PASS | 15,736 / 0 failures | 10,000 jobs in 0.9 ms | in-flight + 5,000 queued in 8.721 ms |
| tsan | PASS | 15,736 / 0 failures | 10,000 jobs in 14.5 ms | in-flight + 5,000 queued in 243.444 ms |

- The Rust `libcribra_capi.a` is not instrumented; only the C++ test binary,
  `CribraSerialWorker.h` and `CribraOwnership.h` are.
- macOS ASan prints `detect_leaks is not supported on this platform`; leak
  detection is therefore `not-tested`, not passed. The run uses
  `abort_on_error=1`.
- The simulator and emulator applications are not sanitizer builds.

## Regression evidence

| Check | Result |
| --- | --- |
| Jest | 10 suites, **281 tests**, 0 failures |
| Preserved baseline tests | 263 tests from RN.1.1–RN.1.5 |
| New lifecycle tests | `lifecycle.test.ts`, 11 tests |
| New stress tests | `stress.test.ts`, 7 tests |
| Package typecheck | PASS |
| Consumer typecheck | PASS |
| ABI host gate | 5 × PASS |
| Vendored header | byte-identical |
| Oracle fixtures | scan, config, transform byte-identical after regeneration |

The new host suite groups:

1. shutdown ordering and hook observation;
2. post-after-stop refusal and fixed message;
3. enqueue copy failure, fixed message and worker recovery;
4. per-job exception recovery;
5. registry cleanup on the worker with 200 real scanners;
6. `SecureZero` and `KeyBytes` destruction;
7. queued key discard zeroization on the worker;
8. transformation-source wipe via allocation watch;
9. 10,000-job burst in FIFO order with no off-thread execution;
10. 200 real scanner create/dispose rounds;
11. concurrent post/stop with four posters;
12. double stop idempotence;
13. idle and loaded shutdown latency.

## Runtime and reload evidence

iOS:

- Debug build installed on the booted iPhone 18 Pro Max simulator; PID 9067.
- Harness marker:
  `CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:config=ok:transform=ok:key=ok:negative=ok:offset=ok:probe=3/5:privacy=ok:stress=ok:recovery=ok`
- Three full `Page.reload` cycles through the Metro inspector. Each created a
  scanner (id 49), submitted an in-flight scan and a queued batch without
  awaiting them, then reloaded. After each reload the same process remained,
  `getLiveScannerCount()` returned 0 and the `globalThis` probe was undefined.
- Release Simulator build succeeded and the installed release app reached the
  same PASS marker. Screenshot: `/tmp/cribra-rn16-ios-release.png`.

Android:

- The ARM64 emulator (`Pixel_10_clean`) ran the four-ABI debug APK and logged
  the same marker with `stress=ok:recovery=ok`.
- Three inspector `Page.reload` cycles with the same abandoned scanner id 49,
  in-flight scan and queued batch. Each reload kept the process (PID 19141),
  returned live count 0 and cleared the probe. Screenshots:
  `/tmp/cribra-rn16-android-reload-1.png` … `-3.png`.
- The release APK built successfully with the embedded JavaScript bundle and
  all four ABIs, and the installed release app reached the same PASS marker
  standalone (no Metro): `CRIBRA_RN_STRESS:scans=1000:ms=17`,
  `keyed=200:ms=2`, then `…:stress=ok:recovery=ok`. Screenshot:
  `/tmp/cribra-rn16-android-release.png`.
- During an ad-hoc inspector probe, calling the **raw** TurboModule
  `transformPseudonymize` with a `Uint8Array` instead of an `ArrayBuffer`
  tripped React Native codegen's debug assertion
  (`runtime.isArrayBuffer(*this)` failed). The documented public facade always
  passes an `ArrayBuffer`; Jest and the runtime harness exercised that path
  thousands of times. This probe mistake is recorded as a remaining risk of
  calling the private TurboModule directly, not as an RN.1.6 product failure.

## Measurements

Runtime counts and latency (Debug builds; values are indicative):

| Platform | Submitted | Settled | Failed | Elapsed |
| --- | --- | --- | --- | --- |
| Android, 20,000 `scan()` | 20,000 | 20,000 | 0 | 3,533 ms |
| Android, 3,000 keyed transforms | 3,000 | 3,000 | 0 | 140 ms |
| iOS, 20,000 `scan()` | 20,000 | 20,000 | 0 | 18,682 ms |
| iOS, 3,000 keyed transforms | 3,000 | 3,000 | 0 | 194 ms |

- Every run ended with live scanner count 0.
- The harness's own stress phase logged
  `CRIBRA_RN_STRESS:scans=1000:ms=34` and
  `CRIBRA_RN_STRESS:keyed=200:ms=5` on the Android Debug build, and
  `scans=1000:ms=17`, `keyed=200:ms=2` on the Android Release build.

Memory (sampled every 250 ms, so the peak is a lower bound):

| Platform / phase | Before | High-water | After | Threads |
| --- | --- | --- | --- | --- |
| Android, 20,000 scans | PSS 311,883 KB | PSS 321,093 KB | PSS 320,714 KB | 44 → 42 |
| Android, 3,000 keyed | PSS 320,175 KB | PSS 319,673 KB | PSS 318,371 KB | 41 → 42 |
| iOS, 20,000 scans + 3,000 keyed | RSS 34,128 KB | RSS 145,696 KB | RSS 46,000 KB | 15 → 15 |

- The Android figures are `dumpsys meminfo` total PSS for
  `com.cribraconsumer`; iOS figures are host `RSS` for the simulator process,
  not device memory.
- Thread count did not grow with 20,000 submitted jobs. The Android count
  includes transient React Native, network and OkHttp threads; the native
  worker is one persistent thread.
- The 10% / 8 MB guidance was not used as a pass criterion. The observed
  Android PSS increase during the 20,000-scan burst is about 9.2 MB at the
  sampled peak and does not return fully to baseline in the same process;
  the keyed phase shows no growth. The iOS simulator RSS rises sharply during
  the concurrent burst (promise/queue backlog in the debug runtime) and falls
  after settlement.
- Across the three Android reloads, PSS moved from 296–322 MB without process
  replacement; this is debug-runtime churn, not a native queue leak: live
  scanner count and abandoned work returned to 0 every time.

## Environments and not-tested items

- iOS Simulator: iPhone 18 Pro Max, Xcode release corresponding to the
  simulator runtime; Debug and Release Simulator builds.
- Android: `Pixel_10_clean`, API 36, ARM64; Debug runtime and four-ABI Debug
  and Release builds with the debug keystore.
- `not-tested`: physical iOS/Android devices; iOS device builds;
  sanitizer-instrumented app builds; ASan leak detection on macOS; long-duration
  soak beyond 20,000 submitted jobs; cancellation/drain (intentionally
  unsupported); Rust static-library instrumentation.

## Remaining risks

1. A full JS reload abandons in-flight promises and the JS key temporary; the
   SDK cannot settle or zero memory owned by a destroyed runtime.
2. Shutdown is O(N) in queued jobs and blocks the calling thread while the
   worker discards the queue and joins. No hard latency bound exists.
3. The settlement gate prevents new scheduling but cannot revoke a callback
   already handed to React Native; RN's weak callback invalidation is part of
   the contract and was not instrumented.
4. Direct calls to the private TurboModule with the wrong JavaScript argument
   type can trip React Native codegen assertions before package code runs. The
   public TypeScript facade is the supported boundary.
5. `CribraSerialWorker` construction can throw if the OS rejects thread
   creation; this path is not handled and is `not-tested`.
6. `stop()` must not be called from the worker thread (it joins) and is not
   designed for concurrent callers; production calls it once from the module
   destructor, followed by the worker's own destructor on the same thread.
7. Zeroization is best effort; Core, Hermes and the OS may retain copies
   outside package-owned buffers.
8. No production lifecycle logging or metrics were added (D8); diagnosis on a
   physical device relies on host tests, memory/thread deltas and reload
   behavior.
9. Settlement marshalling is owned by React Native: `AsyncCallback::call` is
   `noexcept` and allocates for its argument tuple, so an allocation failure
   there can terminate the process regardless of the SDK's `Settle` catch.
   The SDK's catch covers its own pre-scheduling work only.

## Readiness

RN.1.6 is implemented and qualified within the scope above, with the stated
`not-tested` items and remaining risks. It is not a release qualification and
not a publication. No commit, push, merge, tag, or publication was performed.

## Evidence locations (RN.1.6)

- `/tmp/cribra-rn16-ios-1.png`, `-2.png` — iOS Debug PASS markers
- `/tmp/cribra-rn16-ios-reload-1.png` … `-reload-3.png`,
  `/tmp/cribra-rn16-ios-abandon-running.png`,
  `/tmp/cribra-rn16-ios-reload-probe.png` — iOS reload evidence
- `/tmp/cribra-rn16-ios-release.png` — iOS Release Simulator runtime marker
- `/tmp/rn16-ios-release-build.log` — iOS Release build log
- `/tmp/cribra-rn16-android-1.png` — Android Debug PASS marker
- `/tmp/cribra-rn16-android-reload-1.png` … `-3.png` — Android reload evidence
- `/tmp/cribra-rn16-android-release.png` — Android Release runtime marker
- `/tmp/rn16-android-release-build.log` — Android Release build log
- `tools/host-tests/run.sh` — plain/ASan/UBSan/TSan host runner
- `tools/host-tests/main.cpp` — 13 host lifecycle/stress groups
- `consumer/__tests__/lifecycle.test.ts`, `stress.test.ts` — RN.1.6 Jest suites
- `/tmp/cribra-android-pss.txt`, `/tmp/cribra-android-keyed-pss.txt`,
  `/tmp/cribra-ios-rss.txt` — sampled PSS/RSS traces
