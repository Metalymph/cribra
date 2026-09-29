# Architecture

## Scope

`cribra` is a reusable, local-first Rust engine for deterministic sensitive-data
detection, validation, classification, review, and safe transformation.

Cribra is application-agnostic. Callers own source acquisition, I/O, storage,
persistence, presentation, and policy. The engine operates on caller-provided
data and does not define the surrounding product workflow.

It owns:

- compiled detection rules and built-in detector knowledge;
- deterministic matcher execution;
- deterministic and contextual validation;
- classified `Finding` results;
- review-only `SensitiveCandidate` results;
- source locations, severity, confidence, and remediation metadata;
- deterministic result normalization;
- presentation-safe rule metadata and typed explainability;
- querying and aggregate result semantics;
- explicit share-safe transformation contracts;
- source-local execution state required by incremental processing.

It does not own:

- filesystem, directory, glob, or repository traversal;
- network access or uploads;
- source persistence;
- terminal, browser, mobile, or desktop presentation;
- application policy;
- authentication or cloud synchronization;
- subscription or entitlement logic;
- Silens Siren monitoring.

`Scanner` and `ScannerBuilder` are the primary Rust APIs for configuring and
executing Cribra's detection capability. They are not the architectural boundary
of the project: detection is one capability of the engine alongside validation,
review, reporting, querying, explainability, and transformation.

## Engine model

```text
caller-owned data
        │
        ▼
┌──────────────────────────────────────────┐
│                  Cribra                  │
│                                          │
│  detection ──────▶ validation            │
│      │                 │                 │
│      └─────────────────┴──▶ normalization│
│                              │           │
│                              ▼           │
│                       reports / queries  │
│                              │           │
│                              ▼           │
│                         transformation   │
└──────────────────────────────────────────┘
        │
        ▼
caller-owned policy / storage / output
```

Source acquisition and destination policy remain outside the engine. This keeps
the same core reusable inside Rust applications, services, CLIs, WebAssembly
hosts, native bindings, and higher-level security products.

## Detection and review authority

```text
caller-owned UTF-8 source
        │
        ├──────── compiled rule pipeline ────────┐
        │                                        ▼
        │                               matcher / validator
        │                                        │
        │                                        ▼
        │                                     Finding
        │
        └──────── structural review path ────────┐
                                                 ▼
                                      SensitiveCandidate
```

`Finding` is authoritative rule-backed classification. `SensitiveCandidate` is
review evidence only and is never silently promoted into finding semantics.

`DetectionMode` is derived from compiled rule authority. `CandidateEvidence`
belongs to the independent ambiguity channel. `Explanation` projects these
existing facts without becoming a second classification authority.

## Compiled execution

Configuration is compiled into immutable reusable rule state before execution.
Source-local mutable state is kept separate from that shared configuration.

```text
compiled configuration
        │
        ├── shared multi-pattern matcher
        ├── suffix matchers
        ├── contextual pattern rules
        ├── contextual prefilter gate
        └── immutable rule metadata
                    │
                    ▼
              source execution
                    │
                    ├── matcher candidates
                    ├── validation
                    ├── normalization
                    └── report materialization
```

The shared contextual gate is an execution optimization only. A prefilter may
skip a rule that cannot match; it cannot create a finding. Rule-local matching
and validation remain authoritative.

Compiled configuration contains no mutable state belonging to an individual
source and can therefore be reused across independent source executions.

## Whole-source and incremental execution

Whole-source and incremental processing are execution strategies of the same
engine. They must not become independent detection implementations.

Whole-source execution remains the semantic reference path. Incremental
execution moves source-local state into a `SourceSession` while reusing the same
compiled rule authority.

```text
immutable CompiledRuleSet
          │
          ├──────────────┐
          ▼              ▼
   whole source      SourceSession
                         │
                         ├── UTF-8 transport state
                         ├── absolute source position
                         └── matcher-local streaming state
```

Incremental execution is introduced matcher family by matcher family. A matcher
is considered stream-capable only when its chunked execution preserves the
corresponding whole-source detection semantics.

Literal and prefix matching support incremental execution with bounded
source-local state. Cross-chunk matches preserve absolute UTF-8 byte offsets,
prefix token semantics, and end-of-source finalization.

The multi-pattern streaming state retains only the bounded overlap required by
the longest compiled needle plus active prefix candidates whose token boundary
has not yet been observed. It does not retain complete historical chunks.

Arbitrary byte fragmentation is handled separately by the incremental UTF-8
transport. Incomplete UTF-8 scalar bytes may be retained only until the scalar
can be reconstructed or end-of-source proves the input incomplete.

Other matcher, validation, review, normalization, and public reporting stages
remain subject to their existing whole-source semantics until their incremental
execution slice is explicitly implemented and validated.

## Parallel execution

A single logical source is processed serially.

With the optional `parallel` feature, independent sources may be distributed
through Rayon. Each source owns independent execution state while sharing the
same immutable compiled configuration.

Parallel execution does not split an individual source into worker-owned
fragments and does not change per-source detection semantics or deterministic
input ordering.

## Privacy boundary

Public findings and candidates contain metadata and source coordinates, not
copies of matched secret values. Cribra itself performs no network access.

Source-local incremental state is bounded execution state rather than
source-sized persistence.

Share-safe transformations are explicit caller operations and apply to
classified findings. Ambiguous candidates are not automatically redacted,
templated, pseudonymized, or synthesized.

## Consumers

```text
Cribra
├── Generic consumers: Rust apps, middleware, services, custom tooling
├── cribra-cli: canonical process boundary
├── cribra-capi: native C ABI
├── cribra-wasm: WebAssembly integration
├── Silens Scan / Scan+: WASM/PWA
└── Silens Studio: desktop application
```

Consumers may compose Cribra capabilities into broader workflows, but those
workflows do not become core engine responsibilities.

## Dependency direction

Consumers may depend on `cribra`.

Cribra must not depend on its consumers, product-specific workflow, or Silens
Siren.
