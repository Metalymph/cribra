# Cribra Roadmap
## v0.3 — Native Interoperability

Cribra v0.3 focuses on making the existing privacy-first Rust core universally embeddable without expanding detection intelligence.

Architectural target:

```text
Cribra Core
    ├── Rust API
    └── stable-designed C ABI
```

The C ABI is the universal native-language protocol. v0.3 does not ship complete language-specific wrappers.

WASM remains an independent adapter above the same Rust core. Its audit and refinement are intentionally deferred to v0.4 so that v0.3 remains a coherent native-interoperability release. WASM does not pass through the C ABI.

The interoperability architecture is specified in `docs/INTEROP.md`.
### 0.3.1 — Interop specification
- [x] Define scope and non-goals.
- [x] Freeze core-authority and adapter-separation rules.
- [x] Define ownership and lifetime model.
- [x] Define native input and stable primitive representation rules.
- [x] Define status/error model and panic containment.
- [x] Define scanner lifecycle and single-source boundary.
- [x] Define report/finding/candidate projection rules.
- [x] Define explainability and custom-rule authority boundaries.
- [x] Define transform ownership and source/report consistency.
- [x] Define batch/parallel design constraints.
- [x] Define thread-safety contract targets.
- [x] Define ABI versioning and symbol namespace policy.
- [x] Define native artifact/header/consumer validation requirements.
- [x] Define ABI performance policy.
- [x] Define adapter separation rules that keep WASM independent from the C ABI.
- [x] Define privacy/threat model and native semantic-parity expectations.
### 0.3.2 — `cribra-capi` skeleton and artifact model
- [x] Convert the repository root to an appropriate Cargo workspace form while preserving the published root `cribra` crate unless evidence justifies moving it.
- [x] Add `crates/cribra-capi/` as a dedicated native adapter crate.
- [x] Preserve `#![forbid(unsafe_code)]` in the core; confine required FFI `unsafe` to the adapter.
- [x] Configure native static/dynamic artifact production.
- [x] Establish initial adapter module boundaries without exposing public ABI functionality prematurely.
### 0.3.3 — ABI version, scanner lifecycle and single-source scan

- [x] Add explicit ABI protocol version querying independent from crate SemVer.
- [x] Add builder/scanner opaque lifecycle.
- [x] Preserve empty-builder versus current-builtins semantics.
- [x] Add UTF-8 validated pointer-plus-length single-source scan boundary.
- [x] Return an owned report handle.
- [x] Contain panics at exported FFI boundaries.
### 0.3.4 — Report and finding traversal

- [x] Add finding count and indexed traversal.
- [x] Project stable finding metadata without exposing Rust layout.
- [x] Preserve byte-span and Unicode coordinate semantics.
- [x] Preserve remediation optionality.
- [x] Keep matched source values outside the ABI result model.
### 0.3.5 — Ambiguity and explainability

- [x] Add candidate count and indexed candidate projection.
- [x] Preserve `SensitiveCandidate` as semantically distinct from `Finding`.
- [x] Project candidate evidence without inventing severity/confidence/remediation.
- [x] Add finding explanation resolution through scanner-owned metadata.
- [x] Preserve candidate explanation as evidence-derived.
### 0.3.6 — Custom-rule/configuration ABI

- [x] Configure rules through the native builder rather than exposing Rust `Rule` layout.
- [x] Support public literal, prefix, suffix and full-match pattern rule semantics.
- [x] Preserve rule-ID uniqueness and existing scanner-build validation.
- [x] Do not expose internal capture-projection capability as a custom-rule feature.
### 0.3.7 — Transform ABI and owned output buffers

- [x] Add explicit caller-source + report transform boundary.
- [x] Add Rust-owned output buffer handles with one destruction path.
- [x] Preserve redaction/template/pseudonymization/synthesis semantics where exposed.
- [x] Preserve overlap/span validation.
- [x] Define and test source/report consistency behavior.
### 0.3.8 — Batch API and optional parallel execution contract

- [x] Define stable batch input/key representation.
- [x] Define batch result ownership and partial-failure semantics.
- [x] Preserve input order.
- [x] Add an explicit amortization path for small inputs.
- [x] Keep Rayon an implementation detail rather than an ABI concept.
- [x] Preserve serial/parallel semantic equivalence.
- [x] Project `ShareBundle` only after the batch ownership model exists; do not invent a single-source substitute.
### 0.3.9 — Ownership, error, panic and thread hardening

- [x] Finalize coarse status codes and explicit error-object diagnostics.
- [x] Validate null/invalid argument behavior.
- [x] Audit every allocation/destruction pair.
- [x] Audit borrowed-view lifetimes.
- [x] Verify panic containment.
- [x] Freeze thread-safety guarantees per handle type.
- [x] Document unavoidable caller-side C memory contract violations.
### 0.3.10 — Generated C header, real consumer and cross-platform CI

- [x] Generate `include/cribra.h`, evaluating `cbindgen` as the default mechanism.
- [x] Compile and link a real C smoke consumer without Cargo knowledge.
- [x] Cover success and recoverable error paths.
- [x] Validate macOS.
- [x] Validate Linux.
- [x] Validate Windows.
- [x] Inspect public symbol/export hygiene where practical.
- [x] Add practical sanitizer/leak/native-memory validation.
### 0.3.11 — ABI performance benchmarks

- [x] Benchmark minimal FFI call overhead.
- [x] Benchmark native scan versus equivalent Rust-native scan.
- [x] Benchmark report count/index traversal.
- [x] Benchmark complete report traversal.
- [x] Benchmark transform allocation/copy cost.
- [x] Benchmark batch amortization.
- [x] Document overhead policy without sacrificing semantics for micro-optimizations.
### 0.3.12 — Documentation, examples, packaging and release gate

- [x] Finalize native integration documentation.
- [x] Provide a minimal native C example.
- [x] Document ownership/lifetime/error/thread contracts prominently.
- [x] Document ABI experimental compatibility policy.
- [x] Link the native ABI performance reference from the primary documentation.
- [x] Validate generated header and native package/release artifacts.
- [x] Run formatting, lint, test, doctest, feature, MSRV, audit and package gates.
- [x] Confirm the cross-platform native C ABI CI matrix is green.
- [x] Finalize v0.3 release notes and versioning metadata.

## v0.3 non-goals

Do not add during this release solely as part of interoperability work:

- new provider detectors;
- new detector families;
- plugin architecture;
- CLI functionality;
- complete language-specific wrapper ecosystems;
- Rust ABI exposure;
- WASM-through-C layering;
- matched secret values in public result models.

---

## v0.4 — WASM Interoperability

Cribra v0.4 focuses on auditing and refining the existing WASM/PWA adapter without changing the native C ABI architecture and without expanding detection intelligence merely for interoperability work.

WASM is an independent adapter over the same Rust core:

```text
Cribra Core
    ├── Rust API
    ├── C ABI
    └── WASM API
```

The existing WASM integration already runs in Silens Scan. v0.4 therefore starts from validation and evidence, not from a rewrite.

### 0.4.1 — Existing WASM boundary audit

- [ ] Validate the current Cribra WASM integration in the Silens Scan PWA.
- [ ] Audit the currently exposed capabilities against the Rust core.
- [ ] Audit initialization and error representation.
- [ ] Audit TypeScript declarations.
- [ ] Audit JS/WASM copy behavior and avoidable secondary copies.
- [ ] Audit serialization overhead and typed projection opportunities.
- [ ] Audit batch ergonomics.
- [ ] Verify Web Worker friendliness.
- [ ] Measure bundle size.
- [ ] Document CSP/browser constraints.
- [ ] Do not route WASM through the C ABI.

### 0.4.2 — Capability parity and justified refinements

- [ ] Expose candidates only where the working integration needs them.
- [ ] Expose explanations only where the working integration needs them.
- [ ] Expose remediation only where the working integration needs it.
- [ ] Expose transforms only where the working integration needs them.
- [ ] Expose custom rules only where the working integration needs them.
- [ ] Prefer typed projections over serialization when evidence shows a measurable benefit.
- [ ] Avoid API churn when the existing boundary is already sufficient.

### 0.4.3 — Rust/WASM semantic parity gate

- [ ] Reuse canonical/golden fixtures where practical.
- [ ] Compare finding count/order and rule IDs.
- [ ] Compare spans and Unicode coordinates.
- [ ] Compare severity/confidence/remediation.
- [ ] Compare candidate count/order/kind/evidence where exposed.
- [ ] Compare explanation facts where exposed.
- [ ] Compare transformed output for equivalent supported operations.
- [ ] Require semantic parity while allowing representation differences.

### 0.4.4 — WASM performance and browser validation

- [ ] Measure boundary call overhead where meaningful.
- [ ] Measure source copy behavior.
- [ ] Measure serialization/projection overhead.
- [ ] Measure batch amortization.
- [ ] Validate representative browser execution.
- [ ] Validate Web Worker execution.
- [ ] Record bundle-size reference measurements.
- [ ] Document the WASM performance policy without sacrificing semantics for micro-optimizations.

### 0.4.5 — Documentation, packaging and release gate

- [ ] Finalize WASM integration documentation.
- [ ] Document initialization, errors, workers and browser constraints.
- [ ] Document supported capability parity with the Rust core.
- [ ] Validate generated TypeScript declarations and package artifacts.
- [ ] Run the complete WASM/browser/test/package release gate.
- [ ] Finalize v0.4 release notes and versioning metadata.

## v0.4 non-goals

Do not add during this release solely as part of WASM interoperability work:

- new provider detectors;
- new detector families;
- plugin architecture;
- CLI functionality;
- complete native-language binding ecosystems;
- WASM-through-C layering;
- matched secret values in public result models;
- a rewrite of the working WASM adapter without measured justification.

---

## Historical benchmark foundation
### S1.3.1 — Benchmark foundation

- [x] Add Criterion as a development dependency
- [x] Add a dedicated `scan` benchmark target
- [x] Measure 1 KiB, 64 KiB, and 1 MiB inputs
- [x] Measure 4, 64, and 512-rule sets
- [x] Measure zero, sparse, and dense findings
- [x] Measure literal, prefix, suffix, regex, and mixed workloads
- [ ] Record the first serial baseline
- [ ] Document benchmark environment and commands
- [ ] Define a performance-regression policy before optimization work
