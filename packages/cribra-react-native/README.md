# @metalymph/cribra-react-native

Private, unpublished native React Native backend for the Cribra sensitive-data
scanner. This is the RN.1.3 implementation: a direct C ABI TurboModule with a
small TypeScript facade, asynchronous scanning, and caller-configurable scanner
construction from Core catalogs and custom rules, validated in an isolated
React Native 0.88.1 New Architecture/Hermes consumer.

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
builder, and error handles are owned by RAII guards and never exposed to
JavaScript. JavaScript receives only numeric scanner IDs and source-free DTOs.

`createScanner(config?)` copies the entire configuration on the JS thread, then
runs one worker job that selects Core catalogs, adds custom rules in caller
order, builds the scanner, and projects detached rule metadata. Omitted
configuration (or `{}`) uses the released canonical-current fast path;
`{ builtins: 'none' }` is an explicit empty scanner. Core remains the sole
authority for rule validation, ordering, and classification.

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
| `cpp/CribraBridge.h/.cpp` | C ABI ownership, scanner construction, rule metadata, scan projection, batch aggregation, ABI version |
| `cpp/CribraEnums.h` | Pure strict ABI enum, rule/build mapping, explanation, and sentinel helpers |
| `cpp/CribraAbi.h` | Major/minor gate and JavaScript safe-integer helpers |
| `cpp/CribraReactNative.h/.cpp` | Serial worker, scanner registry, async TurboModule methods |
| `tools/abi-check/` | Host protocol test for the ABI gate, strict enum mapping, and configuration protocol |
| `ios/OnLoad.mm` | iOS pure-C++ TurboModule registration |
| `CribraReactNative.podspec` | CocoaPods integration and xcframework linkage |
| `android/CMakeLists.txt` | Android target and prebuilt Rust static-library linkage |
| `third_party/cribra/include/cribra.h` | Vendored copy of the canonical C ABI header |
| `scripts/build-native.sh` | Builds iOS/Android Rust static libraries |
| `fixtures/` | Synthetic, non-secret scan fixture |
| `tools/oracle/` | Test-only direct-Core oracle generator for scans and configurations |
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
  type ScannerConfig,
} from '@metalymph/cribra-react-native';

const abi = getAbiVersion(); // { major: 0, minor: 3, patch: 0 }

const config: ScannerConfig = {
  builtins: 'current',
  financialBuiltins: true,
  personalBuiltins: false,
  rules: [
    {
      kind: 'prefix',
      id: 'acme.internal-token',
      value: 'acme_pat_',
      severity: 'high',
      remediation: 'revoke_and_rotate_credential',
    },
    {
      kind: 'pattern',
      id: 'acme.ticket',
      pattern: '\\bACME-[0-9]{4}\\b',
      severity: 'medium',
    },
  ],
};

const scanner = await createScanner(config);
try {
  const one = await scanner.scan('source text', { id: 'one' });
  const batch = await scanner.scanBatch([
    { id: 'a', source: 'first' },
    { id: 'b', source: 'second' },
  ] satisfies ScanSource[]);

  console.log(scanner.rulesCount, batch.summary.totalFindings);
  console.log(one.findings.length);
} finally {
  await scanner.dispose();
}
```

### Scanner configuration

`createScanner(config?)` accepts:

| Field | Default | Meaning |
| --- | --- | --- |
| `builtins` | `'current'` | `'current'` selects the canonical catalog; `'none'` disables it |
| `financialBuiltins` | `false` | Opt-in financial catalog |
| `personalBuiltins` | `false` | Opt-in personal-identifier catalog |
| `rules` | `[]` | Custom rules in caller order |

Omitting the argument or passing `{}` selects the canonical current catalog,
exactly like the previous default factory. An explicit empty scanner is
`{ builtins: 'none' }`. Custom rules are placed before selected catalogs by
Core, and catalogs are submitted current, financial, then personal.

Each custom rule has `id`, `severity`, optional `remediation`, and one of
`literal`/`prefix`/`suffix` (`value`) or `pattern` (`pattern`, Rust `regex`
syntax). Rule semantics are not reimplemented in TypeScript or C++: empty IDs,
empty matchers, invalid patterns, zero-length patterns, duplicate IDs, and
catalog collisions all come from Core. `remediation: null`/`undefined` means no
remediation; the output-only `'unknown'` label is rejected as input.

`scanner.rules` is a frozen, detached snapshot of `RuleMetadata` and
`scanner.rulesCount` its length. Both remain readable after `dispose()`. The
snapshot never contains matcher, pattern, or source contents.

### Async differences from the WebAssembly package

- `createScanner()` is a promise, not a synchronous factory after `init()`.
- `createScanner(config?)` is the only public factory. Omitted configuration
  preserves the canonical-current default.
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
  `includeExplanations` option. Detached rule metadata is available in RN.1.3.

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

Malformed UTF-16 in a scanned source is rejected locally, before any native
call, as `CribraError` with `code === 'invalid_unicode_string'`. Offset
conversions use the additional stable codes `invalid_span_value`,
`invalid_span_order`, `span_out_of_bounds`, `span_not_utf8_boundary`, and
`span_not_utf16_boundary`. See [Offsets](#offsets).

An incompatible linked ABI is reported as `CribraError` with
`code === 'abi_mismatch'` and a detail of the form
`cribra_native_error:abi_mismatch:expected=0.3.0:actual=X.Y.Z`. This check runs
before scanner construction and before every `cribra_scanner_scan`, so an
incompatible library cannot reach scanner code.

Core scanner-build failures are reported through structured
`cribra_error_build_info` metadata, never by parsing human-readable messages:

| Code | `CribraError.code` |
| --- | --- |
| 1 | `invalid_pattern` |
| 2 | `pattern_matches_empty` |
| 3 | `missing_capture_group` |
| 4 | `empty_rule_id` |
| 5 | `duplicate_rule_id` |
| 6 | `empty_matcher` |
| 7 | `too_many_rules` |
| 8 | `automaton_build` |

An unknown future code fails closed as `scanner_create_failed`. When Core
reports one, `CribraError.ruleId` carries the caller's rule identifier (or a
public built-in ID for a catalog collision). The identifier is never embedded
in the error message or detail. Matcher, pattern, and source contents are never
reported. The builder is released on every early error, and a scanner whose
metadata projection fails is disposed before the projection error is surfaced.

### Offsets

Core reports zero-based half-open UTF-8 byte spans (`finding.span`,
`candidate.span`). JavaScript strings are indexed in UTF-16 code units, so use
the pure conversion utilities rather than byte arithmetic:

```ts
import {
  byteSpanToUtf16Range,
  byteSpansToUtf16Ranges,
  utf16RangeToByteSpan,
  utf8ByteLength,
} from '@metalymph/cribra-react-native';

const range = byteSpanToUtf16Range(source, finding.span);
const matched = source.slice(range.start, range.end);

const ranges = byteSpansToUtf16Ranges(
  source,
  findings.map((finding) => finding.span),
);
const span = utf16RangeToByteSpan(source, range);
```

They are pure, synchronous and native-independent: they work without a scanner.

Conversion rules:

- Exact or throw. There is no clamping, rounding, U+FFFD substitution, or
  selection of half a surrogate pair.
- Ranges are half-open in both directions. Byte offset `0`, UTF-16 index `0`,
  and the exclusive end of a non-empty source are valid boundaries.
- Grapheme clusters are not respected. Combining marks and ZWJ emoji are
  separate scalars and may be split at any scalar boundary. No Unicode
  normalization is applied.
- `byteSpansToUtf16Ranges` converts many spans from one source in a single walk
  (`O(n + k log k)` time, `O(k)` memory) and preserves input order. Prefer it
  over converting findings one at a time, which costs `O(k * n)`.

Validation is deterministic and source-free, in this order:

| # | Failure | `CribraError.code` |
| --- | --- | --- |
| 1 | source is not a string | `invalid_argument` |
| 2 | unpaired UTF-16 surrogate in the source | `invalid_unicode_string` |
| 3 | span/range is not an object, or an endpoint is not a safe non-negative integer | `invalid_span_value` |
| 4 | `start > end` | `invalid_span_order` |
| 5 | endpoint beyond the source length | `span_out_of_bounds` |
| 6 | endpoint inside a UTF-8 scalar (forward) | `span_not_utf8_boundary` |
| 6 | endpoint inside a surrogate pair (reverse) | `span_not_utf16_boundary` |

Errors never contain source text, matched values, offsets, matchers, or rule
IDs.

#### Malformed UTF-16 and scan preflight

Hermes replaces an unpaired surrogate with U+FFFD before scanning, and JSC
silently truncates at it. Either way, native spans could refer to text that is
not the caller's string. `scan()` and `scanBatch()` therefore reject any
unpaired high or low surrogate with `invalid_unicode_string` before dispatching
native work. Every batch source is validated before the batch is sent, so no
partial or rewritten source reaches Core. Well-formed strings, including
supplementary characters and emoji, are unaffected. The conversion utilities
independently reject malformed input.

#### Source drift

Spans are only meaningful for the exact string value passed to the scan. Keep
that value. JavaScript strings are immutable, so "modified after scanning"
means the caller substituted a different value. Before applying highlights or
redactions, compare the retained value with the current editor content:

```ts
const scanned = source;
const report = await scanner.scan(scanned, { id });
if (currentText !== scanned || report.sourceBytes !== utf8ByteLength(scanned)) {
  // Stale result: rescan. Do not index, highlight, or redact.
}
```

The byte-length check is a necessary consistency check but not proof of
identity: different strings can share a UTF-8 byte length (`"é"` and `"Ā"` are
both 2 bytes). Never apply redaction or highlighting to a changed source merely
because the length matches. RN.1.4 provides no automatic redaction API.

The preflight is `O(n)` with no allocation. Measured on the qualification
matrix: 33-34 ms per MiB on the Android ARM64 emulator and 55-60 ms per MiB on
the iOS simulator, both Hermes debug builds. Typical editor sources are far
smaller (about 2-4 ms per 64 KiB). Work happens on the JS thread before the
async native call.

## Privacy

The native package does not log, persist, or upload source text, matcher
payloads, rule identifiers, or findings. The runtime harness logs and displays
only the aggregate marker
`CRIBRA_RN_SMOKE:PASS:abi=...:entries=...:findings=...:candidates=...:lifecycle=ok:parity=ok:batch=ok:config=ok:negative=ok:offset=ok:probe=...:privacy=ok`
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
and malformed batch elements), and the RN.1.3 configuration surface.

The configuration checks build default, explicit-empty, financial, personal,
combined, and custom-rule scanners; compare their detached `scanner.rules`
metadata (and scan output where present) against the direct-Core config oracle;
exercise every Core-reachable build-error code and its structured `ruleId`; and
confirm the live-scanner count returns to baseline after every failure. They
also verify that emitted markers contain no fixture secret, candidate value,
matcher payload, rule ID, or source ID.

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

Regenerate the scan oracle after changing the fixture or Core:

```sh
cd tools/oracle
cargo run --release -- ../../fixtures/synthetic-source.json \
  > ../../consumer/src/fixtures/synthetic-oracle.json
```

Regenerate the configuration oracle after changing the configuration fixture
or Core:

```sh
cd tools/oracle
cargo run --release -- --config-cases \
  ../../consumer/src/fixtures/rn13-config-cases.json \
  > ../../consumer/src/fixtures/rn13-oracle.json
```

Checks:

```sh
npm run typecheck          # package facade (uses the consumer's TypeScript)
npm run test:abi           # host test for the ABI gate, safe-integer boundary,
                           # strict enum/explanation mapping, and config protocol
cd consumer
npx tsc --noEmit           # consumer harness types
npm test                   # Jest facade batch/config contracts plus App render
```

`npm run typecheck` reads React Native's generated types from the consumer's
`node_modules`, so install the consumer first. Jest mocks the native
TurboModule because unit tests have no native binary; it pins the JavaScript
contract (one native call, copied inputs, strict DTO/summary/metadata
validation, structured build errors, scanner cleanup, and source-free error
normalisation). The real native path is the simulator/emulator harness above.

## Scope

RN.1.3 exposes scanner-builder configuration and detached rule metadata. It
deliberately excludes Unicode helper surfaces (RN.1.4), flow transforms
(RN.1.5), cancellation/stress/reload (RN.1.6), streaming, incremental
documents, Share Bundles, parallel scanning, and npm publication. It does not
link `cribra-uniffi`, does not introduce a second scanner engine, and does not
modify the WebAssembly API, Core, or the released C ABI protocol.

See [QUALIFICATION.md](./QUALIFICATION.md) for the build, ABI, parity,
lifecycle, size, and limitation record.
