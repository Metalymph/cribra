# Architecture

## Scope

`cribra` is a reusable, privacy-first Rust library. Callers own input, I/O,
storage, persistence and presentation.

It owns:

- scanning orchestration;
- compiled detection rules and built-in detector knowledge;
- deterministic and contextual validation;
- classified `Finding` results;
- review-only `SensitiveCandidate` results;
- source locations, severity, confidence and remediation metadata;
- presentation-safe rule metadata and typed explainability;
- deterministic result normalization;
- share-safe transformation contracts.

It does not own:

- filesystem or repository traversal;
- network access or uploads;
- terminal, browser or desktop presentation;
- authentication or cloud synchronization;
- persistence or subscription logic;
- Silens Siren monitoring.

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

The built-in pipeline groups deterministic matcher work and uses conservative
prefiltering for contextual pattern rules:

```text
UTF-8 source
    │
    ├── deterministic matcher groups
    │
    └── shared contextual prefilter gate
             │
             ▼
       potentially active rules
             │
             ▼
       rule-local matcher / validator
             │
             ▼
       deterministic normalization
```

The shared contextual gate is an execution optimization only. A prefilter may
skip a rule that cannot match; it cannot create a finding. Rule-local matching
and validation remain authoritative.

A single source is scanned serially. With the optional `parallel` feature,
independent sources are distributed through Rayon while sharing the same
immutable scanner. Serial and parallel execution preserve the same per-source
semantics and input ordering.

## Privacy boundary

Public findings and candidates contain metadata and source coordinates, not
copies of matched secret values. Cribra itself performs no network access.

Share-safe transformations are explicit caller operations and apply to
classified findings. Ambiguous candidates are not automatically redacted,
templated, pseudonymized or synthesized.

## Consumers

```text
Cribra
├── Generic consumers: Rust apps, middleware, services, custom tooling
├── Silens Scan / Scan+: WASM/PWA
└── Silens Studio: desktop application
```

## Dependency direction

Consumers may depend on `cribra`.

Cribra must not depend on those consumers or on Silens Siren.
