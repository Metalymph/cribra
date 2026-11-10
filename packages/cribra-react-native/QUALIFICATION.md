# Cribra React Native RN.1.1 — Qualification Record

Status: **RN.1.1 implemented and runtime-validated on the iOS simulator and the
Android emulator.** This is not a release-qualification claim, and it is not an
npm publication. Physical devices and release builds are `not-tested`.

Recorded: 2026-11-09; pre-commit review revalidation: 2026-11-10. Package:
`@metalymph/cribra-react-native@0.0.0` (private, unpublished). ABI:
`cribra-capi` 0.3.0.

## Result summary

| Check | Result | Evidence |
| --- | --- | --- |
| Package and consumer TypeScript | PASS | `npm run typecheck`; `npx tsc --noEmit` |
| Consumer Jest render test | PASS | `npm test` (native facade mocked) |
| ABI compatibility policy host test | PASS | `npm run test:abi`; `ABI compatibility gate: PASS` |
| C header matches canonical Core ABI header | PASS | `cmp include/cribra.h third_party/cribra/include/cribra.h` |
| iOS simulator build | PASS | `xcodebuild ... ** BUILD SUCCEEDED **` |
| iOS simulator runtime | PASS | rendered marker and screenshot; ABI 0.3.0 |
| iOS physical-device runtime | not-tested | no device run |
| Android build, four ABIs | PASS | `BUILD SUCCESSFUL`; CMake for all ABIs |
| Android emulator runtime (arm64-v8a) | PASS | logcat marker; ABI 0.3.0 |
| Android armeabi-v7a/x86/x86_64 runtime | not-tested | build-only ABIs |
| Android physical-device runtime | not-tested | no device run |
| Synthetic scan parity vs direct Core | PASS | full DTO tree comparison on both platforms |
| Scanner lifecycle | PASS | harness checks; `lifecycle=ok` |
| Release/npm qualification | not-tested | private package; no release build |

Runtime markers observed:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok
```

The Android marker was captured from logcat after the ABI-gate rebuild:

```text
11-10 15:54:40.500 12645 12724 I ReactNativeJS: CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok
```

The iOS marker was rendered by the isolated harness and captured in
`/tmp/cribra-ios-final-review.png`. `simctl launch --console` does not capture
React Native `console.log`, so the harness displays the same marker on screen
for source-free evidence. The rebuilt iOS `CribraConsumer.debug.dylib` also
contains the `cribra_native_error:abi_mismatch` string, confirming the gate was
compiled into the runtime artifact.

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

## Builds

### Native Rust artifacts

`scripts/build-native.sh` builds `cribra-capi` 0.3.0 (Core 0.7.2) as static
libraries. It does not use `cribra-uniffi`.

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

### iOS

`pod install` integrated 88 pods and generated `CribraReactNativeSpec`; the
single `CribraReactNative` pod was autolinked. The Debug simulator build
succeeded:

```sh
xcodebuild -workspace CribraConsumer.xcworkspace -scheme CribraConsumer \
  -configuration Debug -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro Max' build
# ** BUILD SUCCEEDED **
```

The Debug simulator app bundle is 85 MB, including React Native development
overhead.

### Android

Four ABIs are intentional: the consumer keeps the React Native 0.88 template's
default `reactNativeArchitectures=armeabi-v7a,arm64-v8a,x86,x86_64`. Only
`arm64-v8a` was exercised at runtime. The other three were compiled and linked
but are **build-only** in this record; no emulator or device ran them.

| ABI | Built | Runtime-qualified |
| --- | --- | --- |
| `arm64-v8a` | PASS | PASS (Pixel_10_clean emulator) |
| `armeabi-v7a` | PASS | not-tested |
| `x86` | PASS | not-tested |
| `x86_64` | PASS | not-tested |

On iOS, the device `arm64` slice is build-only; the simulator `arm64` slice was
runtime-qualified, and the `x86_64` simulator slice is build-only.

`./gradlew assembleDebug --no-daemon` succeeded after CMake debug configuration
and compilation for all four ABIs:

```text
> Task :app:configureCMakeDebug[arm64-v8a]
> Task :app:buildCMakeDebug[arm64-v8a]
...
> Task :app:assembleDebug
BUILD SUCCESSFUL in 47s
95 actionable tasks: 58 executed, 37 up-to-date
```

The Debug APK is 136 MB across four ABIs. `lib/x86_64/libappmodules.so` contains
the generated `NativeCribraReactNative...` bridging symbols and
`facebook::react::CribraReactNative` symbols, confirming that the C++ bridge was
compiled and linked into the app rather than merely typechecked.

## ABI and parity proof

### ABI

Both runtime markers report ABI `0.3.0`. `getAbiVersion()` is implemented by
calling `cribra_abi_version_major/minor/patch`. The vendored header is
byte-identical to the canonical `include/cribra.h`.

Before scanner creation and before every scan, `AbiCompatibilityFailure()`
compares the linked runtime version with the vendored header's
`ABI_VERSION_MAJOR/MINOR/PATCH`. Compatibility requires equal major and minor;
a patch difference is tolerated. A mismatch returns
`cribra_native_error:abi_mismatch:expected=<contract>:actual=<linked>`:

- `CreateDefaultScanner` fails before `cribra_scanner_new_current`.
- `Scan` throws before `cribra_scanner_scan`.
- `disposeScanner` is intentionally not gated, so an already-created handle can
  always be freed even if the linked ABI is later found to be incompatible.
- The facade normalizes the text to `CribraError` with
  `code === 'abi_mismatch'`; scanner construction does not fold it into
  `scanner_create_failed`.

The policy is checked on the host with `npm run test:abi`, which exercises the
pure `cpp/CribraAbi.h` header for equal, patch-different, and incompatible
major/minor combinations. The runtime mismatch path itself cannot be triggered
against the correctly linked 0.3.0 library; it is guarded by that host test and
the compile-time `static_assert`s.

### Fixture and oracle

- Fixture: `fixtures/synthetic-source.json`, a synthetic five-source fixture
  (217 UTF-8 bytes) copied from the existing Cribra test fixture. It contains
  no real credentials.
- Oracle: `tools/oracle` links `cribra` Core directly, bypasses the C ABI and
  TurboModule, and emits source-free metadata to
  `consumer/src/fixtures/synthetic-oracle.json`.
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

Comparison is strict on values, array order, and key sets. It is intentionally
insensitive to JSON object key order because `serde_json` sorts keys in the
oracle while the DTO projection preserves constructor order.

Both platforms passed this comparison.

## Lifecycle evidence

The harness checks and reports `lifecycle=ok` only after all of the following:

1. Initial live native scanner count is 0.
2. `createScanner()` succeeds and the live count becomes 1.
3. `scanBatch()` completes and parity passes.
4. Two concurrent `dispose()` calls share one native disposal, both resolve,
   and the live count returns to 0.
5. A later `dispose()` is a no-op.
6. A scan after disposal throws `CribraError` with `code === 'disposed'`.
7. A second scanner can be created (live count 1) and disposed (live count 0).

Native ownership evidence:

- `CribraScanner`, `CribraReport`, and `CribraError` handles are wrapped in
  RAII guards; only the guards free them.
- The registry maps JS-safe numeric IDs to `shared_ptr<ScannerHandle>`; raw
  pointers never cross into JavaScript.
- One serial worker executes scanner creation, scans, and disposal. The worker
  receives plain C++ strings and promises, not JSI runtime objects.
- JSI string conversion and promise conversion happen on the JS thread;
  resolution is scheduled through React Native's `CallInvoker`.
- `AsyncPromise` is copied into each worker job; `bridging::toJs` runs after
  posting with the still-valid local promise.
- In-flight scans hold a shared scanner reference, so disposal removes the
  registry entry but does not free the handle until the queued work finishes.
- The facade stores the in-flight disposal promise, so concurrent `dispose()`
  calls await the same native operation. A real disposal failure clears the
  cached promise for retry while scans remain blocked.
- On module teardown the worker is destroyed first (declared last), drains
  queued serial work while the scanner registry is still alive, and joins its
  thread before the registry is destroyed.

## Defects found during runtime validation

These were caught by execution, not compilation:

1. **iOS crash on scan.** The first implementation moved the `AsyncPromise` into
   the worker capture before converting it to a JS value, leaving a moved-from
   promise and a `SIGSEGV` in `AsyncPromise::get`. The worker now captures a
   copy, and the promise is converted while valid. Re-validated on both
   platforms.
2. **Harness fixture key mismatch.** The Core fixture uses `text`, while the
   facade's `ScanSource` uses `source`; the harness now maps the field. This
   produced `CribraError('invalid_argument')`, not a native defect.
3. **False parity failure from key order.** The first comparator used
   `JSON.stringify` equality, which is key-order sensitive; the direct-Core
   oracle sorts keys via `serde_json`. The comparator now ignores object key
   order while remaining strict on values and arrays.

Pre-commit review corrections:

4. **Missing ABI gate.** The first implementation exposed the ABI version but
   did not compare it before scanner creation or scanning. The bridge now checks
   major/minor against the vendored header, fails closed with `abi_mismatch`,
   and preserves that code through the facade. `npm run test:abi` covers the
   policy; both native targets were rebuilt and revalidated.
5. **Consumer Jest test could not run.** `App.test.tsx` imported the facade,
   whose `TurboModuleRegistry.getEnforcing` throws when no native binary is
   present. The test now mocks the facade package and renders the screen under
   Jest; the real native path remains the simulator/emulator harness.

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
- No performance, memory, startup, background/foreground, or long-running
  stress measurements were made.
- `scanBatch()` is a serial loop of `scan()` calls, not a native batch API. It
  stops at the first error and returns no partial batch.
- Explanations are always materialized; there is no `includeExplanations`
  option. Rule metadata (`rules`, `rulesCount`) is not exposed in RN.1.1.
- No streaming, incremental documents, Share Bundles, parallel scanning,
  key-based transformations, or Amon integration.
- No cancellation API; disposal waits behind queued scans.
- Invalid UTF-8 and malformed input behavior was not separately exercised at
  runtime; the C ABI remains the authority for those failures.
- The `needsReview` projection is findings **or** candidates, matching Core.
  Absent remediation is an empty wire string and is converted to `null` by the
  facade.

## Evidence locations

Build and runtime logs are ephemeral and intentionally not committed:

- `/tmp/cribra-ios-build-review.log` — successful iOS simulator build after the
  ABI-gate change
- `/tmp/cribra-android-build-review.log` — successful four-ABI Android build
  after the ABI-gate change
- `/tmp/cribra-pod-install.log` — CocoaPods/codegen integration
- `/tmp/cribra-android-marker.log` — Android logcat parity marker
- `/tmp/cribra-ios-final.png` — iOS harness marker and counts
- `/tmp/cribra-android-final.png` — Android harness marker and counts

The fixture values (`ghp_...`, `sk_test_...`, `ABCD-EFGH-...`) are synthetic
and already present in Cribra's existing committed test fixtures; they are not
real credentials. No source text or per-finding data is written to logs.

The isolated harness regenerates these results; no source text or per-finding
data is written to logs.
