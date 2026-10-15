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
- source-local streaming state and owned revisioned incremental documents.

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

The shared `PatternPrefilterGate` is an optimization that skips impossible rules.
Per-rule prefilters are different: their enumeration can change match sets and
remains authoritative. Streaming rejects them; incremental edits replay those
rules against the complete updated source. Validation remains shared.

Compiled configuration contains no mutable state belonging to an individual
source and can therefore be reused across independent source executions.

## Whole-source and streaming execution

Whole-source scanning remains the semantic reference. Append-only streaming
moves chunk-local execution into source sessions while reusing compiled rules,
validation and normalization. It does not implement mutable-document edits.

Literal and prefix streaming retain overlap and active token candidates rather
than historical chunks.

The multi-pattern streaming state retains only the bounded overlap required by
the longest compiled needle plus active prefix candidates whose token boundary
has not yet been observed. It does not retain complete historical chunks.

UTF-8 transport delivers only complete scalars to matching and coordinates.
Incomplete scalar bytes are retained until reconstruction or genuine EOF proves
the input incomplete.

The public `StreamSession` composes the annotated validated session with bounded
structural review detection. Admitted suffix and bounded regex engines, including
projected captures, reuse compiled rules. Validator dispatch, exact coordinates,
normalization and review suppression preserve whole-source semantics.

Owned bounded history and pending snapshots provide validator evidence; prefix
coordinate pins preserve long-lived starts. Checked planning precedes allocation.
Raw candidate and retained-result limits remain distinct from logical executor
storage admission, which is not a total heap-memory cap.

Only successful consuming `finish` constructs the complete `ScanReport`. All
execution failures are terminal. Complete built-in portfolios, per-rule regex
prefilters and other unsupported configurations fail admission explicitly. See
[the streaming contract](streaming.md) for exact restrictions and ownership.
This chunked execution does not implement incremental rescanning after edits.

## Incremental rescanning — available since v0.6.0

`IncrementalSession` owns a revisioned UTF-8 `String` and shares immutable
compiled scanner configuration. It retains per-rule raw candidates and validator
outcomes before normalization, including rejected and lower-priority records,
and structural observations before overlap suppression. Public reports retain
metadata only; the session separately owns the full document.

Replacement edits use checked byte ranges, UTF-8 boundaries and expected
revisions. Local literal enumeration uses the existing overlapping matcher on
the complete haystack with restricted start ownership. Structural token starts
are invalidated with their boundary dependencies. Unaffected cache spans shift
with checked arithmetic; contextual outcomes are reused only when their exact
clipped source windows remain unchanged, otherwise authoritative dispatch runs
again. Prefix, Suffix and all regex rules, including per-rule prefilters, use
whole-rule replay: streaming eligibility is not a locality proof.

Global normalization, Unicode coordinates, structural suppression and report
construction remain shared with whole-source scanning. A new source, caches and
report are staged beside the old snapshot; only complete success publishes the
new revision. Invalid inputs and edit overflows are recoverable. Resource or
internal invariant failures leave the previous snapshot readable but make the
session terminal. No staged partial report is exposed.

Four optional preventive limits cover raw emissions per operation, simultaneous
cache ownership, result materialization and conservative logical transaction
payload. Old/staged documents and separate cache storage coexist and count.
Spare capacity, allocator overhead, shared compiled state, opaque regex/validator
workspace and caller-owned clones are excluded; this is not a total heap cap.
Document copying, cache traversal and global finalization remain source/result
size dependent. See [incremental contracts](incremental.md) for exact limits,
error precedence and ownership, and [D4 qualification](0.6-d4-qualification.md)
for differential and measured evidence.

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

Streaming retains bounded source-local state. Incremental sessions explicitly
own their full mutable document and caches; they do not promise ruleset-bounded
retention. Neither execution mode performs filesystem discovery or network I/O.

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
