# Cribra WASM Interoperability Audit

Status: v0.4.1 baseline audit  
Branch: `v0.4-wasm-interoperability`

## Purpose

Cribra v0.4 audits and refines the already-working browser/WASM integration
without changing core semantics merely to create a new wrapper.

The first rule of the v0.4 audit is evidence before implementation:

> Do not add a new WASM adapter until the existing Silens Scan integration has
> been identified and its real boundary has been measured.

## Current repository baseline

The Cribra repository currently exposes a Rust-native core that is compatible
with `wasm32-unknown-unknown`.

The repository-level WASM contract is presently validated through:

```text
cargo check --target wasm32-unknown-unknown --no-default-features
cargo check --target wasm32-unknown-unknown --no-default-features --features serde
```

These checks are exposed as the `wasm` and `wasm-serde` contributor commands.

The core itself remains browser-agnostic:

- no `wasm-bindgen` dependency;
- no `js-sys` dependency;
- no `web-sys` dependency;
- no DOM/browser API ownership;
- no JavaScript runtime ownership;
- no TypeScript package surface in this repository;
- no browser worker implementation in this repository;
- no WASM-through-C layering.

This is consistent with the existing architectural boundary: callers own I/O,
storage, presentation and runtime integration.

## Important distinction

Today, "Cribra supports WASM" means two different things that must not be
conflated.

### 1. Core WASM compatibility

The Rust core can be compiled for the browser-oriented
`wasm32-unknown-unknown` target.

This is a property of the Cribra core and is already covered by repository
checks.

### 2. JavaScript/TypeScript WASM adapter

A browser/PWA application still needs a concrete boundary that converts its
JavaScript/TypeScript inputs and result expectations into calls to the Rust
core.

That adapter is not currently represented as a dedicated public crate or
package in the Cribra repository.

The existing Silens Scan integration therefore remains the authoritative
evidence for how Cribra is currently exposed to a browser consumer.

## v0.4 architectural invariant

The intended layering remains:

```text
Cribra Core
    ├── Rust API
    ├── C ABI
    └── WASM adapter
            │
            └── JavaScript / TypeScript consumer
```

The WASM adapter must remain independent from `cribra-capi`.

The C ABI is not an implementation layer for browser WASM.

## What must be audited in Silens Scan

Before adding or changing any Cribra-side WASM API, identify the exact current
consumer boundary and record:

1. where the WASM artifact is built;
2. whether `wasm-bindgen`, `wasm-pack`, raw wasm exports, or another wrapper is
   used;
3. where initialization occurs;
4. what JavaScript/TypeScript functions are currently exposed;
5. how source strings cross the JS/WASM boundary;
6. whether scan results cross as typed values, serialized JSON, or custom
   projection objects;
7. whether the wrapper exposes only findings or also candidates;
8. whether remediation and explanations are projected;
9. whether transforms are currently exposed;
10. whether custom rules are currently supported;
11. how errors are represented;
12. whether batching exists;
13. whether the PWA executes scanning on the main thread or in a Web Worker;
14. how the `.wasm` asset is loaded and cached;
15. what CSP requirements the current loading path has;
16. whether initialization or scanning performs avoidable copies.

## Current capability gap cannot yet be inferred

The Rust core currently exposes substantially more than a simple scanner,
including:

- findings;
- ambiguous `SensitiveCandidate` values;
- candidate evidence;
- explainability;
- remediation;
- query/result summaries;
- custom rules;
- redaction;
- semantic templates;
- pseudonymization;
- synthesis;
- share bundles;
- batch scanning.

The existence of these core capabilities does **not** imply that every one of
them should be exported to JavaScript.

v0.4 will add WASM-facing surface only when the real Silens Scan integration or
another concrete browser consumer benefits from it.

## Likely adapter shape, not yet a decision

If the current consumer audit shows that a reusable Cribra-owned JavaScript
boundary is justified, the preferred architecture is likely a dedicated
adapter crate such as:

```text
crates/
    cribra-capi/
    cribra-wasm/
```

rather than adding `wasm-bindgen` exports to the root core crate.

This is intentionally not yet a roadmap commitment.

A dedicated adapter would preserve:

- `cribra` as a browser-agnostic Rust core;
- `cribra-capi` as the native ABI adapter;
- `cribra-wasm` as a JS/WASM projection layer;
- independent versioning of adapter implementation details where useful;
- no browser/runtime dependencies in the core.

The existing Silens Scan boundary must be inspected before this structure is
accepted or rejected.

## v0.4.1 gate

v0.4.1 is complete only when:

- the current Silens Scan integration has been inspected;
- the real JS/WASM boundary has been documented;
- current exported capabilities have been inventoried;
- initialization and error representation have been recorded;
- current copy/serialization behavior is understood;
- batch and worker behavior are known;
- bundle size can be measured;
- CSP/browser loading constraints are known;
- the team can decide whether Cribra needs a dedicated WASM adapter at all.

Until then, no new `wasm-bindgen` API should be added to the core.

## v0.4.1 measured baseline

The initial reusable `cribra-wasm` adapter was built in release mode for
`wasm32-unknown-unknown` and processed with `wasm-bindgen --target web`.

The resulting baseline artifacts are:

| Artifact | Size |
| --- | ---: |
| `cribra.js` | 5,937 bytes |
| `cribra.d.ts` | 1,698 bytes |
| `cribra_bg.wasm` | 1,335,066 bytes |
| Total | 1,342,701 bytes |

The initial WASM binary is therefore approximately 1.27 MiB before any
WASM-specific size optimization such as `wasm-opt`.

This is a structural baseline, not a final v0.4 size target. The adapter
currently exposes only the minimal `ScanEngine` construction and
`rulesCount()` contract, so bundle size must be re-measured after the justified
scan, result-projection and transform surface has been added.

### Generated TypeScript contract

`wasm-bindgen` currently generates TypeScript declarations for the public
adapter surface.

The generated declaration includes:

```ts
export class ScanEngine {
    rulesCount(): number;
}
```

The generated JavaScript also exposes the corresponding ScanEngine lifecycle
and rulesCount() binding.

This establishes that Cribra can provide its browser-facing API as generated,
typed ES-module artifacts without maintaining handwritten TypeScript
declarations for the Rust-owned surface.

Current copy and serialization baseline

The initial cribra-wasm source contains no explicit use of:

* serde_json;
* JsValue;
* JavaScript typed arrays;
* js_sys;
* web_sys;
* explicit string serialization/deserialization;
* adapter-level cloning.

No application-level serialization layer or secondary adapter copy has
therefore been introduced by the initial WASM boundary.

The current API does not yet transfer source text or result projections across
the JS/WASM boundary, so this finding must not be generalized to the future
scan API. String-transfer, result-projection and transformation costs must be
measured again when those capabilities are implemented.

v0.4.1 outcome

The audit establishes the following architecture:

* cribra remains the semantic Rust authority and remains browser-agnostic;
* cribra-wasm is a dedicated JS/WebAssembly projection layer;
* cribra-wasm does not route through cribra-capi;
* the adapter is DOM-independent and suitable for use inside a dedicated Web
    Worker;
* Worker ownership, service-worker integration, caching, CSP, source lifetime,
    session lifetime and browser orchestration remain consumer responsibilities;
* generated ES-module and TypeScript artifacts are viable;
* the existing Silens Scan adapter is the reference consumer contract for
    justified v0.4 capability parity;
* performance, copy behavior and bundle size will be re-measured after the
    substantive adapter surface exists.