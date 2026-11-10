# @metalymph/cribra-react-native

Private, unpublished native React Native backend for the Cribra sensitive-data
scanner. This is the RN.1.2 implementation: a direct C ABI TurboModule with a
small TypeScript facade and one asynchronous native batch operation, validated
in an isolated React Native 0.88.1 New Architecture/Hermes consumer.

This package is **not** the WebAssembly package `@metalymph/cribra` and is **not
published to npm**. It does not change the Cribra Core, the released C ABI, or
the WebAssembly TypeScript API.

## Architecture

```text
TypeScript facade (src/index.ts)
  -> Codegen spec (src/NativeCribraReactNative.ts)
  -> C++ TurboModule (cpp/CribraReactNative.*)
  -> RAII bridge (cpp/CribraBridge.*)
  -> cribra-capi 0.3.0 C ABI
  -> Cribra Core
```

Native work runs on one serial worker thread. JSI values are read or created
only on the JS thread; the worker only sees plain C++ values. Scanner, report,
and error handles are owned by RAII guards and never exposed to JavaScript.
JavaScript receives only numeric scanner IDs and source-free DTOs.

`scanBatch()` is one TurboModule call and one worker job. The worker runs N
serial `cribra_scanner_scan()` calls using the existing full finding/candidate
projection, then the bridge aggregates the summary natively. It deliberately
does **not** call `cribra_scanner_scan_batch()`: that ABI entry point exposes
neither finding/candidate explanations nor the severity summary fields. The
whole batch resolves as one detached DTO or rejects atomically; no partial
entries are returned.

Before creating a scanner or scanning, including every source in a batch, the
bridge compares the linked ABI (`cribra_abi_version_*`) with the vendored header
contract (`ABI_VERSION_*`).
A major or minor mismatch fails closed with a source-free structured error;
patch differences are tolerated because they are expected to stay compatible
within a minor version. Disposal is not gated so existing handles can always be
released.

## Layout

| Path | Purpose |
| --- | --- |
| `src/NativeCribraReactNative.ts` | Codegen TurboModule spec and wire DTOs |
| `src/index.ts` | Public TypeScript facade, DTO projection, lifecycle and errors |
| `cpp/CribraBridge.h/.cpp` | C ABI ownership, scan projection, native batch aggregation, ABI version |
| `cpp/CribraEnums.h` | Pure strict ABI enum, explanation, and sentinel mapping |
| `cpp/CribraAbi.h` | Major/minor gate and JavaScript safe-integer helpers |
| `cpp/CribraReactNative.h/.cpp` | Serial worker, scanner registry, async TurboModule methods |
| `tools/abi-check/` | Host protocol test for the ABI gate and strict enum mapping |
| `ios/OnLoad.mm` | iOS pure-C++ TurboModule registration |
| `CribraReactNative.podspec` | CocoaPods integration and xcframework linkage |
| `android/CMakeLists.txt` | Android target and prebuilt Rust static-library linkage |
| `third_party/cribra/include/cribra.h` | Vendored copy of the canonical C ABI header |
| `scripts/build-native.sh` | Builds iOS/Android Rust static libraries |
| `fixtures/` | Synthetic, non-secret scan fixture |
| `tools/oracle/` | Test-only direct-Core oracle generator |
| `consumer/` | Isolated RN 0.88.1 runtime/parity harness |

## API

The facade intentionally follows the `@metalymph/cribra` vocabulary where the
native backend allows it: `scan`, `scanBatch`, `dispose`, `Finding`,
`Candidate`, `ScanResult`, `ScanSummary`. The unavoidable difference is
asynchrony: scanner creation, scanning, batch scanning, and disposal all return
promises because native work runs on a worker and resolves on the JS thread.

```ts
import {
  createScanner,
  getAbiVersion,
  type ScanSource,
} from '@metalymph/cribra-react-native';

const abi = getAbiVersion(); // { major: 0, minor: 3, patch: 0 }

const scanner = await createScanner();
try {
  const one = await scanner.scan('source text', { id: 'one' });
  const batch = await scanner.scanBatch([
    { id: 'a', source: 'first' },
    { id: 'b', source: 'second' },
  ] satisfies ScanSource[]);

  console.log(batch.summary.totalFindings, one.findings.length);
} finally {
  await scanner.dispose();
}
```

### Async differences from the WebAssembly package

- `createScanner()` is a promise, not a synchronous factory after `init()`.
- `scan()` and `scanBatch()` are promises. `scanBatch()` is one TurboModule
  call and one native worker job that runs N serial C ABI scans; the batch
  resolves or rejects atomically with no partial result.
- `scanBatch()` preserves empty IDs, duplicate IDs, empty sources, and the
  exact input order. An empty input array is still one native call and returns
  an accurate zero summary.
- `dispose()` is a promise and is idempotent. Concurrent calls await one
  native disposal. A batch dispatched before disposal still completes, and any
  scan after disposal throws `CribraError` with `code === 'disposed'`.
- Findings and candidate explanations are always materialized. There is no
  `includeExplanations` option and no rule-metadata surface in RN.1.2.

### Errors

Bridge failures are normalized to `CribraError` with a stable `code` and an
optional privacy-safe `detail` derived from Cribra's ABI metadata. Source text,
matched values, and per-finding data are never included in errors.

C ABI failures use status-first text (`cribra_native_error:<status>:<operation>`)
so `invalid_argument` and `invalid_utf8` keep their meaning. Unexpected ABI
enum values and impossible explanation kind/payload combinations fail closed as
`invalid_native_report`; summary accumulation uses checked integer addition and
reports `numeric_overflow`. Both map to `CribraError` with
`code === 'invalid_native_report'`. JSI string conversion failures are sanitized
to `invalid_input_string` without forwarding the JavaScript/JSI exception text.

An incompatible linked ABI is reported as `CribraError` with
`code === 'abi_mismatch'` and a detail of the form
`cribra_native_error:abi_mismatch:expected=0.3.0:actual=X.Y.Z`. This check runs
before `cribra_scanner_new_current` and before every `cribra_scanner_scan`, so
an incompatible library cannot reach scanner code.

## Privacy

The native package does not log, persist, or upload source text or findings. The
runtime harness logs and displays only the aggregate marker
`CRIBRA_RN_SMOKE:PASS:abi=...:entries=...:findings=...:candidates=...:lifecycle=ok:parity=ok:batch=ok:negative=ok:privacy=ok`
or a source-free failure reason. Error messages come from the C ABI's
privacy-safe diagnostic metadata.

## Building

Prerequisites:

- Rust 1.97+ with the Apple and Android targets used below.
- `cargo-ndk` 4.1.2 for Android.
- Xcode with CocoaPods for iOS.
- Android SDK with NDK `27.1.12297006`, build-tools `37.0.0`, and JDK 17.

Build the native Rust static libraries (outputs are gitignored):

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
rustup target add armv7-linux-androideabi aarch64-linux-android i686-linux-android x86_64-linux-android
cargo install cargo-ndk --version 4.1.2

bash scripts/build-native.sh ios
bash scripts/build-native.sh android
```

iOS:

```sh
cd consumer
npm install
cd ios
pod install
xcodebuild -workspace CribraConsumer.xcworkspace \
  -scheme CribraConsumer -configuration Debug -sdk iphonesimulator \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro Max' build
```

Android:

```sh
cd consumer
npm install
cd android
printf 'sdk.dir=%s\n' "$HOME/Library/Android/sdk" > local.properties
./gradlew assembleDebug
```

The Android build deliberately keeps React Native's default four ABIs
(`armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64`). The qualification record
exercised only `arm64-v8a` on an emulator; the other three are build-only.

Generated binaries, Codegen output, Pods, Gradle intermediates, the APK, and the
isolated consumer's build artifacts are ignored by Git.

## Runtime harness

The isolated consumer in `consumer/` scans the synthetic fixture and compares
the complete native result tree against a direct-Core oracle generated by
`tools/oracle`. It also checks the RN.1.2 batch behaviours (empty batch, ID and
order preservation, independent UTF-8 byte lengths, repeat determinism), the
scanner lifecycle (counts, idempotent disposal, post-disposal rejection,
in-flight disposal), deterministic native negative cases (invalid scanner ID
and malformed batch elements), and that emitted markers contain no fixture
secret, candidate value, or source ID.

```sh
# Terminal 1
cd consumer
npx react-native start --port 8081

# iOS
xcrun simctl install <udid> <path-to-CribraConsumer.app>
xcrun simctl launch --console <udid> org.reactjs.native.example.CribraConsumer

# Android
adb reverse tcp:8081 tcp:8081
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.cribraconsumer/.MainActivity
adb logcat | grep CRIBRA_RN_SMOKE
```

Regenerate the oracle after changing the fixture or Core:

```sh
cd tools/oracle
cargo run --release -- ../../fixtures/synthetic-source.json \
  > ../../consumer/src/fixtures/synthetic-oracle.json
```

Checks:

```sh
npm run typecheck          # package facade (uses the consumer's TypeScript)
npm run test:abi           # host test for the ABI gate, safe-integer boundary,
                           # and strict enum/explanation mapping
cd consumer
npx tsc --noEmit           # consumer harness types
npm test                   # Jest facade batch contract plus App render mock
```

`npm run typecheck` reads React Native's generated types from the consumer's
`node_modules`, so install the consumer first. Jest mocks the native
TurboModule because unit tests have no native binary; it pins the JavaScript
contract (one native call, copied inputs, strict DTO/summary validation, and
source-free error normalisation). The real native path is the
simulator/emulator harness above.

## Scope

RN.1.2 deliberately excludes config/rule metadata (RN.1.3), Unicode helper
surfaces (RN.1.4), flow transforms (RN.1.5), cancellation/stress/reload
(RN.1.6), streaming, incremental documents, Share Bundles, parallel scanning,
and npm publication. It does not link `cribra-uniffi`, does not introduce a
second scanner engine, and does not modify the WebAssembly API, Core, or the
released C ABI protocol.

See [QUALIFICATION.md](./QUALIFICATION.md) for the build, ABI, parity,
lifecycle, size, and limitation record.
