# Cribra WASM Interoperability Audit

Status: v0.4.5 release-hardening documentation and packaging gate
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

## Initialization, errors, Workers, and browser constraints

`cribra-wasm` is generated with `wasm-bindgen --target web`. The generated ES
module initializes the `.wasm` module explicitly before constructing
`ScanEngine` or `ScanEngineBuilder` instances.

The adapter maps validation and transformation failures to JavaScript errors at
the exported boundary. Invalid regular expressions, zero-length-capable
patterns, duplicate rule IDs, invalid transformation keys, out-of-range result
indexes, and incompatible explanation lookups fail closed rather than
returning partial semantic state.

The reusable crate intentionally imports no DOM APIs. It can therefore execute
inside a dedicated Web Worker. Worker creation, termination, crash recovery,
session lifetime, service-worker integration, cache invalidation, and UI
coordination remain application responsibilities. Silens Scan exercises this
model by owning source text and transformation keys in a dedicated Worker while
Cribra retains metadata-only scan results.

Browser consumers must serve the generated `.wasm` with the
`application/wasm` MIME type and use a CSP that permits the application-owned
Worker and WASM execution path. The adapter itself requires no network access,
remote storage, DOM access, or global browser state.

The v0.4 validation matrix exercised current stable representatives of the three
major browser-engine families:

- Chrome / V8 / Chromium;
- Safari / WebKit / JavaScriptCore;
- Firefox / Gecko / SpiderMonkey.

Browser-specific scheduler and JIT behavior can affect absolute benchmark
numbers. Semantic parity is required across targets; performance measurements
are interpreted as regression and architecture evidence.

## Supported Rust/WASM capability parity

The WASM surface is intentionally narrower than the complete generic Rust API,
but every exposed capability delegates to the same Rust core semantics.

| Capability | Rust core | `cribra-wasm` | v0.4 parity status |
| --- | --- | --- | --- |
| current built-ins | yes | yes | semantic parity verified |
| literal/prefix/suffix/pattern custom rules | yes | yes | semantic parity verified |
| finding count/order/rule ID | yes | yes | semantic parity verified |
| byte spans and Unicode coordinates | yes | yes | semantic parity verified |
| severity/confidence/remediation | yes | yes | semantic parity verified |
| ambiguous candidate count/order/kind/evidence | yes | yes | semantic parity verified |
| typed finding/candidate explanations | yes | yes | semantic parity verified |
| redact / custom redaction | yes | yes | semantic parity verified where exposed |
| template / custom template options | yes | yes | semantic parity verified where exposed |
| keyed pseudonymization | yes | yes | semantic parity verified |
| keyed deterministic synthesis | yes | yes | semantic parity verified |
| generic ordered multi-source `ScanResults<K>` | yes | no | consumer performs repeated single-source scans |
| Rayon parallel scan | optional native feature | no | not a browser/WASM contract |
| Serde transport | optional | not used by adapter transport | typed projections retained |
| raw WASM memory / allocator API | n/a | no | intentionally not exposed |

Representation differences are allowed. Semantic differences in exposed
capabilities are not.

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

Micro-workloads such as the 64-byte scan are measured in repeated batches and
normalized per operation so browser timer quantization does not collapse the
median to zero. Larger scan, traversal, transform, and boundary-floor workloads
remain the primary architectural evidence.

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

### WASM performance policy

Cribra treats semantic correctness, privacy, deterministic behavior, and a
stable typed boundary as stronger requirements than isolated microbenchmark
wins.

The v0.4 browser benchmark establishes the following policy:

- Binaryen `-Oz` is the single production WASM profile.
- The unoptimized, `-Os`, `-Oz`, and `-O3` artifacts remain a regression
  comparison matrix, not separate product variants.
- Typed `wasm-bindgen` projections are retained. Their measured traversal and
  explanation overhead is too small to justify JSON serialization or bulk
  projection APIs.
- JS string input has a material source-transfer/boundary cost for larger
  inputs. A zero-rule scanner measures a practical boundary floor of roughly
  0.185 ms for 64 KiB and 2.93 ms for 1 MiB in the recorded Chrome/V8
  reference run.
- Repeated 64 KiB scans show no meaningful per-source amortization between
  one, eight, and thirty-two serial sources. A WASM `scanBatch()` API is
  therefore not justified for performance in v0.4.
- Cribra does not expose raw WASM memory, allocator handles, or alternate
  byte-input APIs solely to avoid measured copies. Such an API requires
  evidence from real workloads that the existing safe boundary is a product
  bottleneck.
- Browser benchmark results are interpreted as regression and architectural
  evidence rather than absolute performance guarantees.
- Semantics must never be changed to improve a microbenchmark.

| Chrome/V8 reference (`-Oz`) | Median |
| --- | ---: |
| WASM initialization | 2.295 ms |
| Worker to ready | 6.260 ms |
| 64 KiB scan | 0.295 ms |
| 1 MiB scan | 4.720 ms |
| zero-rule 64 KiB boundary floor | 0.185 ms |
| zero-rule 1 MiB boundary floor | 2.930 ms |
| 256-finding typed traversal | 0.060 ms |
| 256-finding explanation traversal | 0.025 ms |

## v0.4.5 release-hardening contract

The final release gate treats the generated JavaScript glue, TypeScript
declarations, optimized `.wasm` artifact, semantic parity oracle, browser
benchmark harness, and Rust package contents as independently validated release
surfaces.

The root `cribra` crate remains the crates.io package. `cribra-wasm` is an
internal workspace adapter (`publish = false`) for producing the browser
artifact set; it is not a second crates.io package in v0.4.

The release gate must verify that generated TypeScript declarations describe the
same typed public classes/enums that were validated semantically, and that the
production artifact is generated from the same release build before Binaryen
`-Oz` optimization.
