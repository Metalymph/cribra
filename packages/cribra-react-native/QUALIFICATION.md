# Cribra React Native RN.1.2 — Qualification Record

Status: **RN.1.2 implemented and runtime-validated on the iOS simulator and the
Android emulator.** This is not a release-qualification claim, and it is not an
npm publication. Physical devices and release builds are `not-tested`.

Recorded: 2026-11-10. Package: `@metalymph/cribra-react-native@0.0.0` (private,
unpublished). ABI: `cribra-capi` 0.3.0 over Cribra Core 0.7.2.

RN.1.2 builds on the RN.1.1 native TurboModule foundation
(`feat/react-native-rn1-2` from `main` at `109a19b`) and does not modify Cribra
Core, the C ABI, the Swift/Kotlin/Python bindings, or the WebAssembly package.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Jest facade batch contract and render | PASS | `npx jest --ci`; 2 suites, 22 tests |
| Host ABI, safe-integer, enum/explanation protocol | PASS | `npm run test:abi`; three `PASS` lines |
| C header matches canonical Core ABI header | PASS | `cmp include/cribra.h third_party/cribra/include/cribra.h` |
| iOS simulator build | PASS | `xcodebuild ... ** BUILD SUCCEEDED **` |
| iOS simulator runtime | PASS | rendered marker and screenshot; ABI 0.3.0 |
| iOS physical-device runtime | not-tested | no device run |
| Android build, four ABIs | PASS | `BUILD SUCCESSFUL`; CMake for all four ABIs |
| Android emulator runtime (arm64-v8a) | PASS | logcat marker; ABI 0.3.0 |
| Android armeabi-v7a/x86/x86_64 runtime | not-tested | build-only ABIs |
| Android physical-device runtime | not-tested | no device run |
| Synthetic scan parity vs direct Core | PASS | full DTO tree comparison on both platforms |
| Native batch semantics | PASS | empty batch, IDs/order, UTF-8 bytes, repeat determinism |
| Strict DTO/protocol/overflow failures | PASS | host protocol test and mocked-facade Jest cases |
| Deterministic native negatives | PASS | invalid scanner ID and malformed batch elements |
| In-flight disposal and lifecycle | PASS | `lifecycle=ok`; batch completes before disposal |
| Runtime log privacy | PASS | no fixture token, candidate value, rule ID, or source ID in logs |
| Release/npm qualification | not-tested | private package; no release build |

Runtime markers observed on both platforms:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:negative=ok:privacy=ok
```

Android logcat:

```text
11-10 17:27:13.893 I/ReactNativeJS(13549): CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:negative=ok:privacy=ok
```

The iOS marker was rendered by the isolated harness and captured in
`/tmp/cribra-rn12-ios.png`. `simctl launch --console` does not capture React
Native `console.log` on this setup, so the on-screen marker is the iOS runtime
evidence. The Android marker was captured from logcat and matches the on-screen
marker in `/tmp/cribra-rn12-android.png`.

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

## Native batch implementation

`scanBatch()` is **one TurboModule call and one serial-worker job**:

1. The TypeScript facade validates every `{ id, source }` entry, rejects a
   non-array or non-string field with `invalid_argument` before native work, and
   passes a fresh ordered array to the TurboModule.
2. The C++ method copies every `id` and `source` string on the JS thread. JSI
   objects never cross the worker boundary.
3. Exactly one job is posted to the existing serial worker. That job runs N
   serial `cribra_scanner_scan()` calls in input order using the existing full
   projection (findings, candidates, explanations, spans, lines, columns,
   severity, confidence, remediation).
4. The bridge aggregates the summary natively from Core-provided counts, source
   bytes, candidate counts, and projected severity ranks, then resolves one
   detached `ScanBatchDto` on the JS thread.

It deliberately does **not** call `cribra_scanner_scan_batch()`. The C ABI's
batch function exposes no finding/candidate explanation accessors, and its
`CribraBatchSummary` lacks the severity counts, `isClean`, and `hasCritical`
fields required by this API. Using N serial single-source scans is therefore
the only way to preserve full result fidelity through the released ABI.

Summary accumulation uses checked addition (`CheckedAdd`) against
`Number.MAX_SAFE_INTEGER`; every value converted to a JavaScript number also
passes `FitsJsSafeInteger`. An overflow fails closed with
`cribra_native_error:numeric_overflow:<field>` rather than returning a rounded
or partial summary.

A failure in any source rejects the whole promise. No partial entries, partial
summary, or report handle escape. An empty input array is still one native call
and returns an accurate all-zero summary with `isClean: true`.

## Strict DTO and protocol validation

The pure `cpp/CribraEnums.h` mapper is shared by the bridge and the host test.
Each ABI enum is mapped explicitly:

- Severity, confidence, and remediation codes have no implicit fallback. The
  documented `NONE`/`UNKNOWN` remediation sentinels map to the wire vocabulary;
  unrecognized numeric codes fail closed.
- Detection-mode, candidate-kind, candidate-evidence, and explanation-kind
  sentinels are accepted only in their documented contexts.
- The `CribraExplanationView` contract is enforced exactly: a `CLASSIFIED`
  explanation needs a concrete detection mode and must leave candidate evidence
  at `NONE`; an `AMBIGUOUS` explanation needs concrete evidence and must leave
  the detection mode at `NONE`; `NONE` must leave both payloads at `NONE`; and
  `UNKNOWN` accepts the released C ABI's both-`UNKNOWN` form (as well as the
  header's documented both-`NONE` form) but rejects mixed pairs.
- An unrecognized code or impossible combination throws a source-free
  `cribra_native_error:invalid_native_report:<field>:<code>` error. The facade
  maps both this and `numeric_overflow` to
  `CribraError.code === 'invalid_native_report'`.

C ABI failures use status-first text
(`cribra_native_error:<status>:<operation>`), so the facade preserves
`invalid_argument` and `invalid_utf8` instead of folding them into a generic
operation error. JSI input conversion failures are caught and replaced with
`cribra_native_error:invalid_input_string`; the original JSI/JavaScript
exception text is never forwarded because it may contain source fragments.

Scanner IDs are validated before casting: NaN, infinities, fractions, zero,
negatives, and values above `2^53 - 1` are rejected as `scanner_not_found`. The
same helper guards scan, batch, and disposal.

The facade additionally rejects malformed DTO shapes, unsafe or negative
integers, zero-based lines/columns, reversed spans, candidate-count mismatches,
unknown enum labels, explanation payload leaks, and summary invariants that
disagree with the projected entries. These checks validate native output; they
do not re-aggregate or reclassify findings.

## Builds

### Native Rust artifacts

The Rust static libraries were unchanged from RN.1.1 and were not rebuilt for
this qualification; their on-disk sizes remain:

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

`pod install` regenerated `CribraReactNativeSpecJSI.h`. The generated
`NativeCribraReactNativeScanSummaryDto` has fifteen parameters in the expected
wire order (`P0..P6` numeric, `P7` boolean, `P8..P12` numeric, `P13..P14`
boolean). `NativeCribraReactNativeScanBatchDto` has `P0 entries` and `P1
summary`. The C++ aliases in `cpp/CribraBridge.h` match those signatures, and
the generated `__scanBatch` invoker passes `double` and `jsi::Array`.

### iOS

The Debug simulator build succeeded:

```sh
xcodebuild -workspace CribraConsumer.xcworkspace -scheme CribraConsumer \
  -configuration Debug -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro Max' build
# ** BUILD SUCCEEDED **
```

The rebuilt app bundle is 85 MB, including React Native development overhead.
The first RN.1.2 native build failed on a C++ typo in the new severity
projection; after the fix, the incremental build succeeded.

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

`./gradlew assembleDebug --no-daemon` ran `configureCMakeDebug` and
`buildCMakeDebug` for all four ABIs and finished:

```text
> Task :app:configureCMakeDebug[arm64-v8a]
> Task :app:buildCMakeDebug[arm64-v8a]
> Task :app:configureCMakeDebug[armeabi-v7a]
> Task :app:buildCMakeDebug[armeabi-v7a]
> Task :app:configureCMakeDebug[x86]
> Task :app:buildCMakeDebug[x86]
> Task :app:configureCMakeDebug[x86_64]
> Task :app:buildCMakeDebug[x86_64]
...
BUILD SUCCESSFUL in 30s
```

The Debug APK is 160,090,137 bytes across four ABIs. After the build,
`libappmodules.so` for each of the four ABIs contains the
`CribraReactNative::scanBatch` symbol and the
`cribra_native_error:numeric_overflow` and
`cribra_native_error:invalid_native_report` strings, confirming that the new
bridge was compiled and linked into every ABI rather than merely typechecked.

On iOS, the device `arm64` slice is build-only; the simulator `arm64` slice was
runtime-qualified, and the `x86_64` simulator slice is build-only.

## ABI and parity proof

### ABI

Both runtime markers report ABI `0.3.0`. `getAbiVersion()` calls
`cribra_abi_version_major/minor/patch`. The vendored header is byte-identical to
the canonical `include/cribra.h`.

Before scanner creation and before every scan, including every source in a
batch, `AbiCompatibilityFailure()` compares the linked runtime version with the
vendored header's `ABI_VERSION_MAJOR/MINOR/PATCH`. Compatibility requires equal
major and minor; a patch difference is tolerated. A mismatch returns
`cribra_native_error:abi_mismatch:expected=<contract>:actual=<linked>`:

- `CreateDefaultScanner` fails before `cribra_scanner_new_current`.
- `Scan` throws before `cribra_scanner_scan`.
- `disposeScanner` is intentionally not gated, so an already-created handle can
  always be freed even if the linked ABI is later found to be incompatible.
- The facade normalizes the text to `CribraError` with
  `code === 'abi_mismatch'`; scanner construction does not fold it into
  `scanner_create_failed`.

`npm run test:abi` exercises the pure headers on the host:

```text
ABI compatibility gate: PASS
JS safe-integer boundary: PASS
Strict enum/explanation projection: PASS
```

The runtime mismatch path itself cannot be triggered against the correctly
linked 0.3.0 library; it is guarded by the host test and the compile-time
`static_assert`s.

### Fixture and oracle

- Fixture: `fixtures/synthetic-source.json`, a synthetic five-source fixture
  (217 UTF-8 bytes) containing empty IDs, a duplicate ID, one empty source,
  non-ASCII text, a multi-finding source, and one candidate. It contains no real
  credentials.
- Oracle: `tools/oracle` links `cribra` Core directly, bypasses the C ABI and
  TurboModule, and emits source-free metadata to
  `consumer/src/fixtures/synthetic-oracle.json`.
- Re-running the oracle in RN.1.2 produced a byte-identical JSON file, so the
  committed oracle still matches Core.
- Oracle summary: 5 sources, 217 scanned bytes, 2 reports with findings, 3
  without, 3 findings, 1 candidate, 2 critical, 1 high, needs review.

The harness compares the complete `entries` and `summary` trees against the
oracle, including:

- entry order, IDs, source byte counts;
- every finding's rule ID, byte span, line, scalar column, severity,
  confidence, remediation, and explanation;
- every candidate's kind, evidence, byte span, line, scalar column, and
  explanation;
- finding/candidate counts, `needsReview`, `hasCritical`, and every summary
  field.

Comparison is strict on values, array order, and key sets. It ignores JSON
object key order because `serde_json` sorts keys in the oracle while the DTO
projection preserves constructor order. Both platforms passed.

## Batch and lifecycle evidence

The harness reports `batch=ok`, `negative=ok`, `lifecycle=ok`, `parity=ok`, and
`privacy=ok` only after all of the following:

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
14. The emitted marker contains no fixture secret, candidate value, or source
    ID.

Native ownership remains unchanged from RN.1.1:

- `CribraScanner`, `CribraReport`, and `CribraError` handles are wrapped in
  RAII guards; only the guards free them.
- The registry maps JS-safe numeric IDs to `shared_ptr<ScannerHandle>`; raw
  pointers never cross into JavaScript.
- The serial worker receives plain C++ strings, promises, and value structs,
  never JSI runtime objects.
- In-flight batch jobs hold a shared scanner reference, so disposal removes the
  registry entry but does not free the handle until the queued batch finishes.
- On module teardown the worker is destroyed first (declared last), drains
  queued serial work while the scanner registry is still alive, and joins its
  thread before the registry is destroyed.

## Defects and corrections during RN.1.2

1. **Native build failure in the new severity projection.** The first bridge
   used `std::string(*severity->label)` even though `label` is already a
   `std::string_view`. The iOS build caught the invalid indirection before any
   runtime test; the line now uses `std::string(severity->label)`.
2. **Explanation payload validation was initially too permissive.** An early
   mapper treated any `UNKNOWN` sentinel as an absent payload in every context.
   The final mapper requires the exact `*_NONE` sentinel for unused payloads and
   a concrete authority for `CLASSIFIED`/`AMBIGUOUS`, and the host test covers
   the rejected combinations.
3. **Error text order.** Status-first text was adopted so the facade's stable
   `cribra_native_error:<code>` parser preserves `invalid_argument` and
   `invalid_utf8`; the operation name is diagnostic context only.
4. **Strict summary integrity.** The facade now cross-checks every native
   summary field against the projected entries and rejects a mismatch as
   `invalid_native_report` instead of trusting a malformed DTO.

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
- `scanBatch()` does not use `cribra_scanner_scan_batch()`. It is one
  TurboModule call and one worker job, but the job contains N serial
  `cribra_scanner_scan()` calls to preserve explanations and summary fields.
  It stops at the first error and returns no partial batch.
- The native `numeric_overflow`, `invalid_native_report`, and ABI-mismatch paths
  are covered by host protocol tests and mocked-facade tests, not by injecting
  malformed values into a real Core scan.
- Native malformed-argument coverage uses deterministic invalid scanner IDs and
  non-string batch elements. Lone UTF-16 surrogate rejection is not used as the
  sole negative native test; comprehensive Unicode validation is RN.1.4 work.
- No performance, memory, startup, background/foreground, cancellation, or
  long-running stress measurements were made. Disposal waits behind queued
  scans; cancellation/stress/reload is RN.1.6.
- Explanations are always materialized; there is no `includeExplanations`
  option. Rule/config metadata is not exposed in RN.1.2 (RN.1.3).
- No streaming, incremental documents, Share Bundles, parallel scanning,
  key-based transformations, or Amon integration.
- The `needsReview` projection is findings **or** candidates, matching Core.
  Absent remediation is an empty wire string and is converted to `null` by the
  facade.
- The fixture's `ghp_...`, `sk_test_...`, and `ABCD-EFGH-...` values are
  synthetic and already present in Cribra's committed test fixtures; they are
  not real credentials.

## Evidence locations

Build and runtime logs are ephemeral and intentionally not committed:

- `/tmp/cribra-ios-build-rn12.log` — successful iOS simulator build
- `/tmp/cribra-android-build-rn12.log` — successful four-ABI Android build
- `/tmp/cribra-android-install-rn12.log` — emulator APK install
- `/tmp/cribra-android-logcat-rn12.log` — Android logcat marker and privacy grep
- `/tmp/cribra-metro-rn12.log` — Metro bundle and server log
- `/tmp/cribra-oracle-rn12.json` — regenerated direct-Core oracle (byte-identical)
- `/tmp/cribra-rn12-ios.png` — iOS harness marker and counts
- `/tmp/cribra-rn12-android.png` — Android harness marker and counts

No source text or per-finding data is written to logs. The registered marker is
aggregate-only and the harness fails if a fixture secret, candidate value, or
source ID appears in it.
