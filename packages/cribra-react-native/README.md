# @metalymph/cribra-react-native

Private, unpublished native React Native backend for the Cribra sensitive-data
scanner. This is the RN.1.1 implementation: a direct C ABI TurboModule with a
small TypeScript facade, validated in an isolated React Native 0.88.1 New
Architecture/Hermes consumer.

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

Before creating a scanner or scanning, the bridge compares the linked ABI
(`cribra_abi_version_*`) with the vendored header contract (`ABI_VERSION_*`).
A major or minor mismatch fails closed with a source-free structured error;
patch differences are tolerated because they are expected to stay compatible
within a minor version. Disposal is not gated so existing handles can always be
released.

## Layout

| Path | Purpose |
| --- | --- |
| `src/NativeCribraReactNative.ts` | Codegen TurboModule spec and wire DTOs |
| `src/index.ts` | Public TypeScript facade, DTO projection, lifecycle and errors |
| `cpp/CribraBridge.h/.cpp` | C ABI ownership, scan projection, enum labels, ABI version |
| `cpp/CribraReactNative.h/.cpp` | Serial worker, scanner registry, async TurboModule methods |
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
- `scan()` and `scanBatch()` are promises. `scanBatch()` awaits one serial
  native scan per source; there is no single native batch call and no partial
  result on failure.
- `dispose()` is a promise and is idempotent. Concurrent calls await one
  native disposal. A scan after disposal throws `CribraError` with
  `code === 'disposed'`.
- Findings and candidate explanations are always materialized. There is no
  `includeExplanations` option and no rule-metadata surface in RN.1.1.

### Errors

Bridge failures are normalized to `CribraError` with a stable `code` and an
optional privacy-safe `detail` derived from Cribra's ABI metadata. Source text,
matched values, and per-finding data are never included in errors.

An incompatible linked ABI is reported as `CribraError` with
`code === 'abi_mismatch'` and a detail of the form
`cribra_native_error:abi_mismatch:expected=0.3.0:actual=X.Y.Z`. This check runs
before `cribra_scanner_new_current` and before every `cribra_scanner_scan`, so
an incompatible library cannot reach scanner code.

## Privacy

The native package does not log, persist, or upload source text or findings. The
runtime harness logs and displays only the aggregate marker
`CRIBRA_RN_SMOKE:PASS:abi=...:entries=...:findings=...:candidates=...:lifecycle=ok:parity=ok`
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
`tools/oracle`. It also checks scanner counts, idempotent disposal, and
post-disposal behavior.

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
npm run test:abi           # host test for the ABI compatibility policy
cd consumer
npx tsc --noEmit           # consumer harness types
npm test                   # Jest render test with a mocked native module
```

`npm run typecheck` reads React Native's generated types from the consumer's
`node_modules`, so install the consumer first. The Jest test replaces the
facade with a fake because unit tests have no native TurboModule; the real
native path is the simulator/emulator harness above.

## Scope

RN.1.1 deliberately excludes streaming, incremental documents, Share Bundles,
parallel scanning, key-based transformations, Amon integration, and npm
publication. It does not link `cribra-uniffi`, does not introduce a second
scanner engine, and does not modify the WebAssembly API, Core, or the released C
ABI protocol.

See [QUALIFICATION.md](./QUALIFICATION.md) for the build, ABI, parity,
lifecycle, size, and limitation record.
