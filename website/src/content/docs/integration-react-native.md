---
title: 'React Native integration'
description: 'Use the private, native @metalymph/cribra-react-native TurboModule on the New Architecture, including async scanning, configuration, batch, offsets, transformations and lifecycle.'
order: 28
---

# React Native integration

> **Availability:** `@metalymph/cribra-react-native@0.0.0` is implemented and
> qualified through RN.1.6, but it is **private and unpublished**. There is no
> npm registry record and no public `npm install` instruction. Install it from a
> local repository checkout. This page records only the iOS Simulator and
> Android emulator qualification described below; it is not a physical-device,
> universal-platform or production-publication claim.

This is the native React Native backend. It is **not** the `@metalymph/cribra`
WebAssembly package, and it does not change Cribra Core, the released C ABI, the
WebAssembly TypeScript API, or the other language bindings. React Native work
runs asynchronously on one serial native worker: scanner creation, scanning,
batch scanning, transformation and disposal all return promises.

## What it is and is not

The package is a small TypeScript facade over a React Native Codegen
TurboModule:

```text
TypeScript facade (src/index.ts)
  -> Codegen spec (src/NativeCribraReactNative.ts)
  -> C++ TurboModule (cpp/CribraReactNative.*)
  -> RAII bridge (cpp/CribraBridge.*)
  -> cribra-capi 0.3.0 C ABI
  -> Cribra Core 0.7.2
```

It is deliberately a direct C ABI TurboModule, not a UniFFI or JSI generator
experiment. Core remains the only detection engine and the only authority for
rule validation, classification, spans and transformation semantics.

**Provided:** scanning, ordered batch scans, caller-configured scanners with
built-in catalogs and custom rules, detached rule metadata, exact UTF-8/UTF-16
offset conversion, and the four scan-coupled transformations (redaction,
templating, pseudonymization and synthesis). The runtime profile is Flow without
Rayon parallelism.

**Not provided:** streaming, incremental documents, Share Bundles, cancellation,
editor integration, parallel scanning, a key-derivation or key-storage service,
and npm publication. The package does not support Expo Go, and no arbitrary Expo
SDK version is qualified.

## Install from source

Add the package from a local checkout with a `file:` dependency. Do not present
this as a registry package:

```json title="package.json"
{
  "dependencies": {
    "@metalymph/cribra-react-native": "file:../cribra/packages/cribra-react-native"
  }
}
```

Build the native Rust static libraries before installing pods or building the
Android app. The generated binaries are gitignored build products:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
rustup target add armv7-linux-androideabi aarch64-linux-android i686-linux-android x86_64-linux-android
cargo install cargo-ndk --version 4.1.2

bash packages/cribra-react-native/scripts/build-native.sh ios
bash packages/cribra-react-native/scripts/build-native.sh android
```

React Native autolinking consumes the package's `react-native.config.js` entry
for the pure C++ TurboModule. iOS also needs `pod install`; Android uses the
package CMake target and the prebuilt static libraries.

### Verified prerequisites

| Requirement | Verified in the RN.1.6 qualification |
| --- | --- |
| React Native | 0.88.1 consumer; peer range `>=0.88.0 <0.90.0` |
| React | 19.3.0 consumer; peer range `>=19.0.0` |
| TypeScript | 6.0.3 in the isolated consumer |
| Architecture | React Native New Architecture with Codegen and Hermes |
| Node.js | `>=22.11.0` in the consumer harness |
| Rust | 1.97+ with the Apple and Android targets |
| Android toolchain | NDK `27.1.12297006`, build-tools `37.0.0`, JDK 17, `cargo-ndk` 4.1.2 |
| iOS toolchain | Xcode with CocoaPods and the vendored `CribraCAPI.xcframework` |

The peer range and the verified consumer version are different facts. Only the
recorded RN 0.88.1 / React 19.3.0 / TypeScript 6.0.3 environment was qualified;
do not infer support for every version inside the declared range.

### iOS and Android native integration

On iOS the podspec compiles `ios/**`, `cpp/**` and the vendored C header, and
links the locally built `ios/CribraCAPI.xcframework`. On Android the package
links the C ABI static libraries under
`android/src/main/cpp/prebuilt/` through CMake. React Native's default four ABIs
(`armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64`) remain enabled. Qualification
exercised only `arm64-v8a` at runtime on an emulator; the other three were
build-only.

No signing, notarization, App Store, Play Store, Expo EAS or device-build claim
is made.

## Create and dispose a scanner

`createScanner()` is asynchronous because native construction runs on the
worker. Omitting the configuration, or passing `{}`, selects Cribra's current
built-in catalog. Custom rules are placed before selected catalogs by Core.

```ts
import { createScanner } from '@metalymph/cribra-react-native';

const scanner = await createScanner();
try {
  const result = await scanner.scan(
    'ACME_API_KEY=acme_live_example_only\n',
    { id: 'config.env' },
  );

  console.log(result.findingCount, result.candidateCount);
} finally {
  await scanner.dispose();
}
```

`dispose()` is idempotent. Concurrent callers await the same native disposal,
and a failed disposal can be retried. After disposal, `scanner.disposed` is
`true`, scan methods throw `CribraError` with `code === 'disposed'`, and the
detached `scanner.rules` metadata remains readable.

## Scan one source or a batch

`scan(source, options?)` returns a detached report. `options.id` is preserved
verbatim and may be empty, non-ASCII or duplicated. Findings and candidate
explanations are always materialized; there is no `includeExplanations` option.

`scanBatch(sources)` is one TurboModule call and one native worker job that runs
N serial scans. It preserves input order, empty IDs, duplicate IDs and empty
sources. An empty array is still one native call and returns an accurate zero
summary. The whole batch resolves or rejects atomically; no partial entries are
returned.

```ts
import {
  createScanner,
  type ScanSource,
} from '@metalymph/cribra-react-native';

const scanner = await createScanner();
try {
  const sources: ScanSource[] = [
    { id: 'first', source: 'token = acme_pat_example\n' },
    { id: 'second', source: 'no sensitive fixture here\n' },
  ];

  const batch = await scanner.scanBatch(sources);

  console.log(batch.summary.scannedSources);
  console.log(batch.summary.totalFindings);
  for (const entry of batch.entries) {
    console.log(entry.id, entry.sourceBytes, entry.result.needsReview);
  }
} finally {
  await scanner.dispose();
}
```

## Configure scanners and custom rules

`ScannerConfig` accepts four fields:

| Field | Default | Meaning |
| --- | --- | --- |
| `builtins` | `'current'` | `'current'` selects the canonical catalog; `'none'` disables it |
| `financialBuiltins` | `false` | Opt-in financial catalog |
| `personalBuiltins` | `false` | Opt-in personal-identifier catalog |
| `rules` | `[]` | Custom rules in caller order |

An explicit empty scanner is `{ builtins: 'none' }`. Each custom rule has `id`,
`severity`, optional `remediation`, and one of `literal`/`prefix`/`suffix`
(`value`) or `pattern` (`pattern`, Rust `regex` syntax). Core validates empty
IDs, empty matchers, invalid or zero-length patterns, duplicate IDs and catalog
collisions; TypeScript and C++ do not reimplement rule semantics.

```ts
import {
  createScanner,
  type ScannerConfig,
} from '@metalymph/cribra-react-native';

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
  console.log(scanner.rulesCount);
  console.log(scanner.rules.map((rule) => rule.ruleId));
} finally {
  await scanner.dispose();
}
```

`scanner.rules` is a frozen, detached snapshot of `RuleMetadata`. It never
contains matcher payloads, patterns or scanned source.

## UTF-8 byte spans versus JS UTF-16 indices

Core reports zero-based, half-open **UTF-8 byte** spans. JavaScript strings are
indexed in **UTF-16 code units**. Never use a byte offset directly with
`slice()`; convert it with the pure helpers, which work without a scanner:

```ts
import {
  byteSpanToUtf16Range,
  byteSpansToUtf16Ranges,
  createScanner,
  utf16RangeToByteSpan,
  utf8ByteLength,
} from '@metalymph/cribra-react-native';

const scanned = 'price: 12€';
const scanner = await createScanner();

try {
  const report = await scanner.scan(scanned);
  const firstFinding = report.findings[0];

  if (firstFinding) {
    const range = byteSpanToUtf16Range(scanned, firstFinding.span);
    console.log(scanned.slice(range.start, range.end));
  }

  const ranges = byteSpansToUtf16Ranges(
    scanned,
    report.findings.map((finding) => finding.span),
  );
  const spanAgain = utf16RangeToByteSpan(scanned, ranges[0]!);

  console.log(utf8ByteLength(scanned), report.sourceBytes, spanAgain);
} finally {
  await scanner.dispose();
}
```

Conversions are exact or throw. They never clamp, round, substitute U+FFFD, or
split a surrogate pair or UTF-8 scalar. Grapheme clusters are not respected and
no Unicode normalization is applied. `byteSpansToUtf16Ranges` converts many
spans from one source in a single walk and preserves input order.

Malformed UTF-16 in a scanned source — an unpaired surrogate — is rejected before
any native call with `invalid_unicode_string`. Keep the exact string you scanned.
If editor content changes, rescan or use `transform()` on the retained value;
do not apply stale spans to a different string.

## Transform with all four modes

`scanner.transform(source, request)` performs exactly one Core scan and one Core
transformation against the report from that same scan. It resolves with both the
detached scan and the transformed output. Review candidates are preserved in the
scan and are never rewritten in the output.

| Kind | Required | Options | Core default |
| --- | --- | --- | --- |
| `redact` | — | `replacement` | `"[REDACTED]"`; `""` is valid |
| `template` | — | `namespace`, `numbered` | `"CRIBRA"`, `false` |
| `pseudonymize` | `key` | `prefix`, `digestBytes` | `"cribra_pseudo_"`, `16` |
| `synthesize` | `key` | `marker` | `"cribra_synthetic"` |

```ts
import {
  createScanner,
} from '@metalymph/cribra-react-native';

const scanner = await createScanner();
try {
  const redacted = await scanner.transform(
    'ACME_API_KEY=acme_live_example_only\n',
    { kind: 'redact', replacement: '[HIDDEN]' },
  );
  console.log(redacted.scan.findings.length, redacted.output);

  const templated = await scanner.transform(
    'contact: alice@example.invalid\n',
    { kind: 'template', namespace: 'ACME', numbered: true },
  );
  console.log(templated.output);

  // Supply exactly 32 bytes from your own secret store; this is illustrative.
  const key = new Uint8Array(32);

  const pseudonymized = await scanner.transform(
    'contact: alice@example.invalid\n',
    {
      kind: 'pseudonymize',
      key,
      prefix: 'acme_pseudo_',
      digestBytes: 16,
    },
  );
  console.log(pseudonymized.output);

  const synthesized = await scanner.transform(
    'contact: alice@example.invalid\n',
    { kind: 'synthesize', key, marker: 'acme_synthetic' },
  );
  console.log(synthesized.output);
} finally {
  await scanner.dispose();
}
```

Redaction merges overlapping finding spans; templating, pseudonymization and
synthesis reject overlapping spans. `digestBytes` accepts any nonnegative
integer up to `0xFFFFFFFF`; Core clamps the effective digest length to its
supported range.

### Key contract and zeroization limits

Keyed transformations require a `Uint8Array` of exactly 32 raw bytes. A wrong
length fails with `invalid_key_length` before Core is called. The facade copies
the key into a fresh, exactly sized JavaScript temporary and zeroes that
temporary in a `finally` block; the native side copies it once more into one
non-copyable buffer and volatile-zeroes that buffer when its last owner is
destroyed. The caller's `Uint8Array` is never mutated.

These steps erase the copies Cribra controls. They cannot guarantee erasure of
your original array, JavaScript engine string representations, immutable JSI
strings, or Core's transient buffers. The package provides no key derivation,
key storage or KMS integration. Do not log keys, and treat any copy you make as
your own responsibility.

## Candidates are never auto-transformed

A **finding** is an authoritative detection; a **sensitive candidate** is
review-only. The native report always materializes candidate explanations.
Candidates remain in `scan.candidates` and `scan.needsReview`, and `transform()`
never rewrites a candidate in `output`. An application must review ambiguous
candidates explicitly rather than relying on automatic redaction.

## Errors and privacy

Bridge failures are normalized to `CribraError` with a stable `code` and an
optional privacy-safe `detail`. Messages are fixed constants. Scanned source,
matched values, matcher payloads, rule identifiers and offsets are never included
in error messages. Key construction failures may expose only the caller's rule
identity through `CribraError.ruleId`.

Common codes include:

| Code | Meaning |
| --- | --- |
| `disposed` | The scanner has already been disposed |
| `invalid_argument` | A public argument has the wrong type |
| `invalid_unicode_string` | An unpaired UTF-16 surrogate was preflighted |
| `invalid_key_length` | A transformation key is not exactly 32 bytes |
| `invalid_span_value`, `invalid_span_order`, `span_out_of_bounds` | Offset input is malformed |
| `span_not_utf8_boundary`, `span_not_utf16_boundary` | An endpoint splits a scalar |
| `invalid_pattern`, `pattern_matches_empty`, `empty_rule_id`, `duplicate_rule_id`, `empty_matcher`, `too_many_rules`, `automaton_build` | Core scanner-build failure |
| `transform_invalid_span`, `transform_invalid_utf8_boundary`, `transform_overlapping_spans`, `transform_unknown` | Core transformation failure |
| `abi_mismatch` | The linked C ABI major/minor does not match the vendored header |
| `native_error` | A native operation failed, including worker shutdown refusal |

An incompatible ABI fails closed before scanner construction and before every
native scan. Disposal is not gated so existing handles can always be released.
The package does not log, persist or upload source text, findings,
transformation keys or transformed output.

## Lifecycle and teardown

The native module has a one-way lifecycle: `Running → Stopping → Stopped`.
Destroying the TurboModule closes a settlement gate, stops the serial worker,
lets the running job finish, destroys queued jobs on the worker without running
Core work, releases remaining scanner handles and copied keys on the worker,
and joins the thread. The worker is never detached, and shutdown never reads or
creates a JSI value.

A job that reaches a stopping or stopped worker is rejected on the JS thread
with `CribraError` code `native_error` and a fixed detail
(`cribra_native_error:worker_stopped` or
`cribra_native_error:worker_unavailable`). A full JavaScript reload destroys the
old runtime without running promise continuations, so promises still in flight
at that moment are abandoned rather than settled. Application code should
dispose scanners during normal screen or module teardown and must not depend on
pending promises surviving a reload.

## Qualification matrix

RN.1.6 was recorded on 2026-11-12 against `cribra-capi` 0.3.0 over Cribra Core
0.7.2.

| Check | Result |
| --- | --- |
| Package and consumer TypeScript | PASS |
| Jest suites RN.1.1–RN.1.5 preserved | PASS — 10 suites, 281 tests |
| JSI-free host lifecycle suite | PASS — 15,736 checks, 0 failures |
| Host plain / ASan / UBSan / TSan | PASS |
| Host 10,000-job burst and queue discard | PASS |
| ABI gate, enum/explanation and config/transform protocol | PASS |
| Vendored header identity and direct-Core oracle fixtures | PASS |
| iOS Simulator Debug runtime | PASS |
| iOS Simulator Release build and runtime | PASS |
| iOS three full JS reload cycles | PASS |
| Android ARM64 emulator Debug runtime | PASS |
| Android ARM64 emulator Release runtime | PASS |
| Android three full JS reload cycles | PASS |
| Android four-ABI Debug and Release builds | PASS |
| Physical iOS/Android devices | **not tested** |
| iOS device builds | **not tested** |
| macOS ASan leak detection | **not tested** |

Build-only ABI results are not runtime qualification. The four Android ABIs were
built, but only `arm64-v8a` ran on an emulator. No performance, battery, thermal,
accessibility or long-running background claim is derived from this matrix.

## Limitations

- Private and unpublished: no npm registry package, public installation command
  or version-support commitment.
- New Architecture and Hermes only; Expo Go and arbitrary Expo SDK versions are
  not supported claims.
- No physical-device, iOS device-build, production-store or universal-platform
  qualification.
- No streaming, incremental documents, Share Bundles, cancellation, editor
  integration or parallel scanning.
- Scanner, scan, batch, transform and disposal APIs are asynchronous; they differ
  from the synchronous WebAssembly facade after `init()`.
- Keys are raw 32-byte values; key management and complete memory erasure are
  application responsibilities beyond the documented zeroization.

## Related documentation

- [React Native package README](https://github.com/Metalymph/cribra/blob/main/packages/cribra-react-native/README.md)
- [RN.1.6 qualification record](https://github.com/Metalymph/cribra/blob/main/packages/cribra-react-native/QUALIFICATION.md)
- [C ABI integration guide](/docs/integration-c/)
- [Compatibility and distribution](/docs/compatibility-and-distribution/)
- [TypeScript runtime compatibility](/docs/typescript-runtime-compatibility/)
- [Ownership and transformations](/docs/ownership-and-transformations/)
