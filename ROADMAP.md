# Cribra Roadmap

## v0.3 — Native Interoperability

Cribra v0.3 focuses on making the existing privacy-first Rust core universally embeddable without expanding detection intelligence.

Architectural target:

```text
Cribra Core
    ├── Rust API
    ├── stable-designed C ABI
    └── existing WASM/PWA API
```

The C ABI is the universal native-language protocol. v0.3 does not ship complete language-specific wrappers.

WASM already operates in the Silens Scan PWA. The v0.3 WASM work audits and refines that working boundary after Cribra v0.2 integration validation; WASM does not pass through the C ABI.

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
- [x] Define existing-WASM audit scope.
- [x] Define privacy/threat model and semantic parity gate.

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

### 0.3.12 — Existing WASM boundary audit and refinement

- [ ] Validate Cribra v0.2 in the existing Silens Scan PWA before deeper adapter changes.
- [ ] Audit exposed v0.2 capabilities.
- [ ] Audit JS/WASM copy behavior and avoidable secondary copies.
- [ ] Audit serialization overhead and typed projection opportunities.
- [ ] Audit batch ergonomics and errors.
- [ ] Audit TypeScript declarations and initialization.
- [ ] Verify Web Worker friendliness.
- [ ] Measure bundle size.
- [ ] Document CSP/browser constraints.
- [ ] Expose candidates, explanations, remediation, transforms and custom rules only where the working integration needs refinement.
- [ ] Do not route WASM through the C ABI.

### 0.3.13 — Rust/C/WASM semantic parity gate

- [ ] Reuse canonical/golden fixtures where practical.
- [ ] Compare finding count/order and rule IDs.
- [ ] Compare spans and Unicode coordinates.
- [ ] Compare severity/confidence/remediation.
- [ ] Compare candidate count/order/kind/evidence.
- [ ] Compare explanation facts.
- [ ] Compare transformed output for equivalent supported operations.
- [ ] Require semantic parity while allowing representation differences.

### 0.3.14 — Documentation, examples, packaging and release gate

- [ ] Finalize native integration documentation.
- [ ] Finalize WASM integration documentation after audit results.
- [ ] Provide minimal native C example.
- [ ] Document ownership/lifetime/error/thread contracts prominently.
- [ ] Document ABI experimental compatibility policy.
- [ ] Validate package/release artifacts.
- [ ] Run the complete test/feature/audit/package release gate.

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
