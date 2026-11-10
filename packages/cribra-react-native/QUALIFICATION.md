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
- No Unicode helper surface (RN.1.4), flow transforms (RN.1.5), streaming,
  incremental documents, Share Bundles, parallel scanning, key-based
  transformations, or Amon integration.
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
