# Cribra WASM Interoperability Audit

Status: v0.4.2 capability parity and Binaryen production-profile decision
Branch: `v0.4-wasm-interoperability`

## Purpose

Cribra v0.4 audits and refines the already-working browser/WASM integration
without changing core semantics merely to create a new wrapper.

The v0.4 rule remains evidence before implementation and optimization: preserve
the Rust core as semantic authority, measure the real browser boundary, and add
or optimize only what concrete consumers justify.

## Architecture

```text
Cribra Core
    ├── Rust API
    ├── C ABI
    └── WASM adapter
            │
            └── JavaScript / TypeScript consumer
```

The WASM adapter is independent from `cribra-capi`. The C ABI is not an
implementation layer for browser WASM.

The core remains browser-agnostic. The dedicated `cribra-wasm` crate owns the
typed `wasm-bindgen` projection. DOM APIs, Worker orchestration, service-worker
integration, caching, CSP, source lifetime, session lifetime, storage, and UI
remain consumer responsibilities.

## v0.4.1 measured baseline

The initial reusable `cribra-wasm` adapter was built in release mode for
`wasm32-unknown-unknown` and processed with `wasm-bindgen --target web`.

| Artifact | Size |
| --- | ---: |
| `cribra.js` | 5,937 bytes |
| `cribra.d.ts` | 1,698 bytes |
| `cribra_bg.wasm` | 1,335,066 bytes |
| Total | 1,342,701 bytes |

This was a structural baseline before the substantive scan/result/transform
surface existed.

## v0.4.2 capability-parity outcome

The reusable `cribra-wasm` adapter now exposes the justified browser-facing
surface derived from the working Silens Scan integration while remaining
independent from application policy, DOM APIs, and the native C ABI.

The typed WebAssembly surface includes:

- `ScanEngine` and metadata-only single-source scanning;
- separate confirmed-finding and ambiguous-candidate projections;
- typed severity, confidence, remediation, candidate-kind, and
  candidate-evidence values;
- typed finding and candidate explanations;
- redact, template, pseudonymize, and synthesize transformations;
- exact 32-byte `Uint8Array` keys for keyed transformations;
- a generic `ScanEngineBuilder`;
- literal, prefix, suffix, and pattern custom rules;
- optional composition of custom rules with the authoritative current built-ins.

Application-specific concepts such as Silens policy IDs, revisions, entitlement
semantics, Worker ownership, service workers, caching, and browser UI remain
outside `cribra-wasm`.

### Typed projection decision

The completed adapter does not use `serde_json`, `JsValue`, JSON serialization,
or deserialization for scan/result transport.

The only explicit owned-copy behavior observed in the adapter is small boundary
material such as owned rule identifiers. Rust error values are converted to
strings only when crossing the JavaScript error boundary. The generated
TypeScript declarations expose typed classes and enums rather than serialized
generic payloads, and keyed transformations use `Uint8Array`.

Typed projection is therefore the retained public boundary.

### Artifact size after capability parity

Release artifacts produced with `wasm-bindgen --target web` after completing
the v0.4.2 surface measure:

| Artifact | v0.4.1 baseline | v0.4.2 |
| --- | ---: | ---: |
| `cribra.js` | 5,937 bytes | 30,668 bytes |
| `cribra.d.ts` | 1,698 bytes | 11,501 bytes |
| `cribra_bg.wasm` | 1,335,066 bytes | 1,502,228 bytes |
| Total | 1,342,701 bytes | 1,544,397 bytes |

The unoptimized v0.4.2 WASM binary is the common input for the Binaryen
comparison below.

### Error and ownership boundary

Rust validation remains independently testable on native targets. JavaScript
error construction occurs only at the exported WASM boundary.

The adapter retains scan metadata but does not retain source text or
transformation keys. Transform operations require the source to be supplied
explicitly for each call. Ambiguous candidates remain review-only and are
never promoted to transform authority.

## Binaryen production-profile gate

The same 1,502,228-byte `wasm-bindgen` output was used as the sole input for
four comparable artifacts:

```text
base
Binaryen -Os
Binaryen -Oz
Binaryen -O3
```

No Rust profile, adapter API, fixture, or JavaScript glue change was introduced
between variants.

### Size results

| Variant | Raw WASM | gzip -9 | Brotli q11 |
| --- | ---: | ---: | ---: |
| base | 1,502,228 B | 443,167 B | 301,865 B |
| `-Os` | 1,198,407 B | **419,063 B** | **284,364 B** |
| `-Oz` | **1,178,110 B** | 419,168 B | 284,822 B |
| `-O3` | 1,201,557 B | 419,332 B | 284,877 B |

`-Oz` reduces the raw WASM artifact by 324,118 bytes, about 21.6%, relative to
the unoptimized baseline. After Brotli q11, all three Binaryen variants are
effectively equivalent for transfer size; the complete spread from `-Os` to
`-O3` is only 513 bytes.

All four artifacts were validated with WABT `wasm-validate`.

### Real-browser benchmark method

The repository benchmark harness executes each variant in its own dedicated
Web Worker and measures the same typed adapter operations. Startup uses 20
fresh-worker samples. Runtime measurements include clean 64 KiB and 1 MiB
scans, 256-finding traversal, 256-finding redaction, and construction of a
four-rule custom scanner.

The 64-byte scan remains below the useful timer resolution of the current
browser harness and is not used for optimization decisions.

The decision uses medians for startup and representative runtime comparisons;
isolated scheduler outliers in startup p95 values are not treated as product
performance regressions.

### Cross-engine results

Representative 1 MiB clean-scan throughput:

| Engine / browser | base | `-Os` | `-Oz` | `-O3` |
| --- | ---: | ---: | ---: | ---: |
| Safari / WebKit | **285.7 MiB/s** | 264.6 MiB/s | 266.0 MiB/s | 267.4 MiB/s |
| Firefox / SpiderMonkey | **239.2 MiB/s** | **239.2 MiB/s** | 235.8 MiB/s | 237.0 MiB/s |
| Chrome / V8 | 194.9 MiB/s | 214.4 MiB/s | **215.1 MiB/s** | 213.9 MiB/s |

Representative WASM initialization medians:

| Engine / browser | base | `-Os` | `-Oz` | `-O3` |
| --- | ---: | ---: | ---: | ---: |
| Safari / WebKit | 6.98 ms | **4.60 ms** | 4.94 ms | 4.74 ms |
| Firefox / SpiderMonkey | 7.00 ms | 6.62 ms | 6.46 ms | **6.36 ms** |
| Chrome / V8 | 6.415 ms | 3.370 ms | **2.985 ms** | 3.365 ms |

The engines react differently to Binaryen output. WebKit preserves a measurable
large-input throughput advantage for the unoptimized baseline, SpiderMonkey is
nearly neutral, and V8 materially improves both startup and scan throughput
after Binaryen optimization.

No optimized profile is universally fastest. However, `-Oz` provides the best
overall browser-product trade-off:

- smallest raw artifact;
- about 21.6% raw-size reduction;
- approximately 284.8 KiB Brotli-q11 comparison artifact;
- substantial startup improvement on all three tested engine families;
- materially higher representative scan throughput on V8;
- near-neutral representative throughput on SpiderMonkey;
- a bounded large-input throughput cost on WebKit;
- no meaningful transfer-size reason to maintain separate `-Os` or `-O3`
  production variants.

### Production decision

**Cribra v0.4 uses one production browser/WASM artifact: Binaryen `-Oz`.**

The repository retains the base/`-Os`/`-Oz`/`-O3` matrix only as a reproducible
regression benchmark. It does not define separate Free/Plus, browser-specific,
or performance/compact production binaries.

This avoids duplicate cache identities, entitlement-dependent binary selection,
double release validation, and service-worker/versioning complexity without
giving up a demonstrated product-level benefit.

The production pipeline is:

```text
Rust `wasm32-unknown-unknown` release
        ↓
wasm-bindgen --target web
        ↓
Binaryen wasm-opt -Oz
        ↓
single production JS/TypeScript/WASM artifact set
```

The optimization choice is evidence-driven rather than permanent ABI policy.
Future Cribra releases should re-run the maintained browser matrix when code
shape, Rust/LLVM, wasm-bindgen, Binaryen, or browser engines change materially.

## Reproducible contributor commands

```text
just wasm-adapter
just wasm-opt-prepare
just wasm-production
just wasm-bench-prepare
just wasm-bench-serve
```

`just wasm-opt-prepare` reproduces the base/`-Os`/`-Oz`/`-O3` validation and
size evidence. `just wasm-production` emits only the selected `-Oz` production
artifact set. `just wasm-bench-prepare` prepares the four isolated benchmark
directories, and `just wasm-bench-serve` serves the dependency-free browser
harness through Node.js.

`make` mirrors the same contributor targets.

## v0.4.2 gate outcome

v0.4.2 is complete:

- capability parity is intentionally scoped to concrete browser needs;
- typed projections remain preferred over serialization;
- source and transformation-key lifetime remain consumer-owned;
- the WASM adapter remains independent from the C ABI;
- the comparison matrix is reproducible;
- Safari/WebKit, Firefox/SpiderMonkey, and Chrome/V8 were exercised;
- Binaryen `-Oz` is the single production optimization profile.

## v0.4.3 parity gate outcome

### Target-independent deterministic synthesis

The Rust/WASM semantic parity gate identified a target-width dependency in
deterministic synthesis. Span offsets were previously hashed as `usize`, which
encoded them as 64-bit values on common native targets but as 32-bit values on
`wasm32`.

Span hashing is now normalized to `u64`. This preserves the existing output on
64-bit native targets while making deterministic synthesis stable across native
and WebAssembly builds.

The parity gate verifies transformed output across both targets, including
redaction, templating, pseudonymization, and deterministic synthesis.