# Cribra Roadmap

Cribra is an application-agnostic, local-first Rust engine for detecting, reviewing,
and safely transforming secrets and sensitive material. The Rust core is the
semantic authority; adapters project applicable core behavior rather than
reimplementing detection policy.

**Current focus: v0.7.0 — final performance, configurability and integration-quality qualification.**

## Engineering principles

- Deterministic results, high-confidence detection, conservative false positives,
  exact UTF-8 source spans, explainable findings, and safe transformations.
- Whole-source scanning remains the semantic reference. New execution modes must
  preserve the complete applicable `ScanReport`, including findings, ambiguous
  `SensitiveCandidate` values, metadata, ordering, and coordinates.
- Preserve established public contracts. Prefer additive, evidence-driven changes;
  require a demonstrated correctness or operational need before rearchitecting.
- Keep source material local. Avoid unnecessary dependencies, external validation
  calls, application-specific policy, and independent detection implementations.
- Treat capability limits explicitly: unsupported configurations must fail clearly
  or take a documented exact fallback, never silently return partial results.
- Different execution models have different resource contracts. Logical accounting
  and bounded source retention are not claims of a total heap-memory cap.

## Release status

| Component / line | Status | Scope |
| --- | --- | --- |
| `cribra` **v0.5.0** | **Released — complete** | Bounded-source-retention streaming for explicitly admitted rulesets |
| `cribra-wasm` **v0.4.6** | **Released — complete** | Published compatibility update to core `^0.5`; existing JS/WASM API unchanged |
| `cribra-cli` v0.4.4 | Existing independent release | Canonical CLI; no new streaming or incremental interface implied |
| `cribra-c` v0.3.0 | Existing independent release | Native interoperability; no new streaming or incremental interface implied |
| `cribra` **v0.6.0** | **Released — complete** | Exact incremental scanning of mutable documents |
| `cribra-wasm` **v0.4.7** | **Released — complete** | Compatibility update to core `^0.6`; existing JS/WASM API unchanged |
| `cribra` **v0.7.0** | In development — locally qualified | Incremental performance, configurable profiles and integration quality |

Core release-line numbering does not force synchronized CLI, C, WASM, or
language-binding versions. Adapter parity applies to each adapter's documented,
applicable public surface; exposing a new core API requires separate design,
implementation, and qualification.

## v0.5.0 — Streaming Core — RELEASED

**Status: completed, qualified, merged, tagged, and released.**

v0.5 introduces synchronous chunked source processing without retaining the
entire input source. It extends the authoritative scanner; it does not replace
whole-source execution or broaden the streamable rule portfolio by implication.

Delivered:

- [x] Public `Scanner::stream()` and `stream_with_limits()` session creation.
- [x] `StreamSession::push()` for chunked input and consuming `finish()` for
      successful end-of-source report finalization.
- [x] Arbitrary transport partitioning, including UTF-8 scalar splits, with
      exact global byte spans and line/Unicode-scalar coordinates.
- [x] Complete `ScanReport` parity with whole-source scanning **for admitted
      configurations**, including findings, `SensitiveCandidate` values,
      validators, normalization, ordering, and overlap suppression.
- [x] Explicit upfront rejection of unsupported configurations; no silent
      degradation of matching or validation semantics.
- [x] Bounded retained source state and three independent optional controls:
      raw-candidate count, checked logical storage, and retained-result count.
- [x] Terminal error handling and successful-EOF-only complete report.
- [x] Rust, MSRV, C/WASM regression/parity, formatting, strict Clippy,
      documentation, security, packaging, and release qualification.
- [x] Core v0.5.0 release; WASM compatibility publication as
      `cribra-wasm` v0.4.6, retaining its established JavaScript API.

**Scope boundaries, not unfinished v0.5 commitments:**

- Default built-in Core, Personal, Financial, and Combined portfolios are not
  automatically streamable. Unsupported regex and semantic planning remain
  explicitly outside the admitted streaming subset.
- The API returns a complete report at EOF; it does not promise pre-EOF ordered
  emission or a total heap-memory cap. Result metadata can accumulate until
  EOF, subject to the optional retained-result limit.
- Existing `Scanner::scan()` behavior and Rayon parallel whole-source scanning
  are preserved. Rayon was already implemented and tested before v0.5; it is
  **not** an outstanding performance milestone.
- Streaming bindings for CLI, C, WASM, and native languages are not implied by
  the Rust-core release or by the WASM dependency compatibility update.
- Broad portfolio admission, speculative regex frontiers, and whole-source
  executor migration are not retroactive release requirements.

Authoritative implementation and qualification records:
`docs/streaming.md`, `docs/streaming-compatibility.md`,
`docs/0.5-qualification.md`, and `docs/0.5-release.md`.

### Historical v0.5 phase closure

The original v0.5-A/B/C/D design and implementation phases are **closed for the
released admitted-subset contract**. Historical design notes and broader
aspirations must not be read as claims that every builtin rule is streamable or
that pre-EOF finalization is implemented. Any remaining broader capability is a
new, separately justified proposal, not an unfinished v0.5 release gate.

## v0.6.0 — Incremental Scanning Core — RELEASED

**Goal:** after edits to an already scanned UTF-8 document, produce exactly the
same complete `ScanReport` as a fresh authoritative whole-source scan of the
updated document, while avoiding demonstrably unnecessary rescanning when safe.

Incremental scanning is **not** append-only streaming. Streaming eligibility
must not be reused as an incremental-locality proof: even bounded-length regex
matches can propagate non-overlapping match selection across an arbitrarily
long suffix after an insertion.

### Architectural contract

- Owned UTF-8 document and explicit revisioned replacement edits `[start, end)`;
  insertion and deletion are special cases.
- Immutable compiled scanner shared with a document-local incremental session.
- Per-rule cache before global normalization, and structural candidate cache
  before overlap suppression. Preserve enough information to restore candidates
  previously rejected, suppressed, or displaced by priority.
- Reuse authoritative matcher, validator, normalization, coordinate, and
  suppression logic. Do not introduce a second detection engine.
- Initially replay all rules to establish a transactional exact baseline; then
  introduce local invalidation only for families with a defensible proof.
- For rules whose selection or dependencies cannot be safely localized, rescan
  the **whole rule** against the updated document. Whole-document fallback is
  permitted when needed to preserve exactness.
- Recompute final normalization, source coordinates, and structural suppression
  from the updated candidate multiset until profiling justifies a more complex
  indexed representation.
- Validate edit ranges and UTF-8 boundaries, use checked arithmetic and revision
  checks, and publish a new revision atomically. Recoverable errors must leave
  the previous complete revision available.
- Make ownership, temporary allocations, cache growth, and logical limits
  explicit; never label logical accounting as a total heap cap.

### D1 — Semantic and dependency audit — COMPLETE

- [x] Inventory whole-source pipeline, report equality, ordering, exact-span
      ownership, structural candidates, and overlap suppression.
- [x] Classify literal, prefix, suffix, regex, capture, prefilter, and validator
      dependencies; identify non-local match-selection behavior.
- [x] Establish independent incremental eligibility and explicit fallback policy.
- [x] Compare flat owned-document/cache architecture with indexed document and
      checkpoint alternatives; select the smallest exact design.
- [x] Define revision/edit semantics, correctness argument, differential test
      strategy, resource accounting, and representative benchmark workloads.

**D1 decision:** use an owned `String`, per-rule pre-normalization caches,
selective invalidation with conservative fallback, and global authoritative
finalization. No generic bounded regex invalidation is claimed. D1 was a
read-only architectural audit; it did not implement or qualify incremental
execution.

### D2 — Exact session foundation — IMPLEMENTED, LOCALLY QUALIFIED

- [x] Owned source, revision, per-rule pre-normalization caches, unsuppressed
      structural cache and complete report.
- [x] Checked insert/delete/replace, UTF-8 boundaries, stale revisions and atomic
      snapshot publication; shared authoritative validation and finalization.
- [x] Initial full replay and complete-report differential qualification,
      including hidden/rejected candidates, captures and contextual validators.

### D3 — Proven selective invalidation — IMPLEMENTED, LOCALLY QUALIFIED

- [x] Local literal and structural start invalidation with boundary ownership,
      checked span shifting and selective contextual revalidation.
- [x] Whole-rule Prefix, Suffix and regex/prefilter fallback.
- [x] Adversarial edit differential parity and private work counters.

Global finalization and document copying remain size dependent; localized matcher
work is not a claim of sublinear end-to-end updates.

### D4 — Resource contracts, public API and benchmarks — IMPLEMENTED, LOCALLY QUALIFIED

- [x] Preventive fallible raw emission, including whole-rule fallback.
- [x] Four optional limits: raw candidates, simultaneous cache records,
      materialized results and logical transaction payload.
- [x] Opaque Rust session with owned document, revisioned edits, borrowed source
      and complete-report access, recoverable inputs and terminal execution errors.
- [x] Built-in/custom differential tests, resource boundaries, Rust 1.97 MSRV,
      strict Clippy, documentation and relevant C/WASM regressions.
- [x] Equivalent edit-plus-update versus edit-plus-scan benchmarks, including
      dense and fallback-heavy cases where incremental execution is slower.

Contracts: [incremental](docs/incremental.md). Dated evidence:
[D4 qualification](docs/0.6-d4-qualification.md). Local qualification does not
establish Linux/Windows CI or publication readiness by itself.

### D5 — Final documentation, audit and release readiness — COMPLETE

- [x] Align roadmap, README, architecture and rustdoc with implemented contracts.
- [x] Preserve relevant historical decisions without reinstating obsolete plans.
- [x] Complete current local regression and documentation qualification.
- [x] Complete remote release validation and publication.
- [x] Prepare version/changelog, packaging and tag-driven release qualification
      in the separately authorized release campaign.

Cribra v0.6.0 and the independently versioned cribra-wasm v0.4.7 are
published. The incremental API remains Rust-only; no new CLI, C or WASM
incremental interface is implied. Qualification evidence is recorded in
[D5 qualification](docs/0.6-d5-qualification.md).

### Deferred pending evidence

Regional prefix/suffix replay, regex restart/checkpoints, document ropes,
indexed coordinate trees, incremental normalization, and adapter exposure are
not part of the minimum v0.6 commitment. They require their own correctness
proofs, profiling evidence, or consumer demand.

## v0.7.0 — Performance, Configurability and Integration Quality — IN PROGRESS

**Status:** implementation integrated and locally qualified; final documentation,
integration-quality review and release qualification remain open.

v0.7 consolidates the existing architecture. It does not introduce a new
detection engine, change semantic authority or require architectural redesign.

### Delivered performance optimizations

- [x] P0.3 — Profile incremental edit phases and identify dominant costs.
- [x] P1.1 — Ordered heap merge for eligible literal candidate caches,
      preserving authoritative ordering and retention.
- [x] P2.1 — Remove redundant old-source emission validation during cache
      transfer without weakening transactional or resource contracts.
- [x] Integrate the performance changes with the configurable build profiles.
- [x] Pass combined Rust Core/Flow/Fuse and adapter regression.

Measured incremental edit latency versus the v0.6 baseline:

| Workload | v0.7 | Change |
| --- | ---: | ---: |
| `dense-8-64k` | 19.654 ms | -46.62% |
| `mixed-64k` | 488.09 us | -8.76% |
| `sparse-64-64k` | 33.813 us | -2.02% |
| `sparse-4-1k` | 783.20 ns | +3.60% |

These measurements were collected on the performance branch before
integration with P5. The combined implementation subsequently passed local
regression, but was not independently rebenchmarked. Results are not
cross-platform guarantees. The small-input regression is retained explicitly.

Evidence: [performance](docs/PERFORMANCE.md) and
[historical phase profile](docs/0.7-p03-incremental-phase-profile.md).

### Delivered configurable profiles

- [x] P5.1 — Rust Core, Flow and Fuse profiles using existing Cargo features.
- [x] P5.2 — WASM Core and Flow; Flow remains the default and official profile.
- [x] P5.3 — Automated JS/TypeScript API contract and Node.js runtime qualification.
- [x] P5.4 — Measure binary sizes, compression and dependency differences.
- [x] P5.5 — Consolidate naming, documentation and regression.

Profiles:

| Profile | Rust | WASM |
| --- | --- | --- |
| Core | Scanner without default features | Scanning/reporting API |
| Flow | Default; transformations enabled | Default; transformations enabled |
| Fuse | All features; parallel and Serde | Not supported |

Evidence: [configuration qualification](docs/0.7-configuration-qualification.md).

### Final qualification checklist

- [x] Q1 — Qualify WASM Core/Flow compilation and JS runtime contracts.
- [x] Q2 — Integrate performance changes and pass combined regression.
- [x] Q3 — Consolidate configuration documentation and roadmap.
- [ ] Q4 — Publish reproducible performance, footprint and dependency
      measurements in README with explicit limitations.
- [ ] Q5 — Document a source-verified comparison with relevant scanners
      covering privacy, responsibility, embeddability, size, portability,
      scanning capabilities and limitations.
- [ ] Q6 — Audit existing Rust, C, WASM and CLI adapter boundaries without
      architectural redesign.
- [ ] Q7 — Evaluate whether a capability manifest adds concrete value;
      document the decision without assuming implementation is required.
- [ ] Q8 — Evaluate precompiled WASM distribution and packaging;
      document the decision and any independently scheduled follow-up.
- [ ] Q9 — Verify C ABI compatibility, ownership, errors and existing
      conformance coverage; make only justified compatibility corrections.

Final release gates remain separate: strict CI, packaging, versioning,
publication and release evidence must be qualified explicitly.

### Next workstream

After v0.7 closure, prioritize independently versioned Swift and Python
bindings over the existing authoritative Rust core and applicable C ABI.
No automatic streaming or incremental adapter parity is implied.

## Road to 1.0 — Maturity, not feature expansion

**Core feature-complete target: v0.7.0.**

Cribra has implemented the planned core execution architecture:

- Three scanning modes: whole-source, admitted-configuration streaming,
  and exact incremental scanning.
- Synchronous execution and optional Rayon parallel scanning of independent
  inputs; parallelism is not promised for every scanning mode.
- Three Rust build profiles: Core, Flow and Fuse.
- Two WASM build profiles: Core and Flow, exposing their documented
scanning/reporting surface rather than Rust streaming/incremental APIs.
- An independently versioned C ABI adapter exposing its documented
  applicable surface, not automatic parity with every Rust profile.

Feature-complete means the planned core capability set is implemented.
It does not imply universal streaming support, complete adapter parity,
zero false positives, unrestricted input sizes or production qualification
on every platform.

### v1.0 maturity gates

- [ ] **Correctness:** deterministic semantic parity and sustained regression
      qualification across supported execution modes.
- [ ] **API stability:** documented public contracts, compatibility policy
      and justified handling of breaking changes.
- [ ] **Portability:** explicit platform support matrix backed by CI or
      reproducible qualification evidence.
- [ ] **Performance stability:** reproducible representative baselines,
      evaluated regressions and documented resource limitations.
- [ ] **Integration maturity:** qualified adapter contracts, ownership,
      error handling and independently versioned binding compatibility.
- [ ] **Release maturity:** repeatable packaging, security maintenance,
      release validation and documented support policy.

A v1.0 release requires evidence that these gates are satisfied.
It does not require adding new scanning modes or redesigning the engine.

### Post-v0.7 versioning policy

- v0.7 closes the planned core feature set and completes its qualification.
- v0.8 is conditional: create it only for demonstrated, justified changes
  that warrant a new core minor release.
- v0.9 is not a mandatory milestone.
- Progression directly from v0.7 to v1.0 is permitted when maturity gates
  are satisfied.
- Swift, Python and other language bindings evolve as independent
  interoperability workstreams; their releases do not automatically require
  a new core minor version.

## Independent interoperability and ecosystem workstreams

These workstreams remain separate from the v0.6/v0.7 core milestones. New core
features are not automatically present in any adapter.

### Canonical CLI parity and distribution

- Maintain the canonical `cribra-cli` contract, deterministic human/JSON output,
  explicit input ownership, privacy-safe diagnostics, and documented exit codes.
- Evaluate additional applicable core capabilities and distribution channels
  independently; preserve existing CLI compatibility. Track exact filters,
  sorting, pack selection, transformations, multi-source/parallel applicability
  and explicit failure policy in [CLI parity](docs/CLI_PARITY.md).
- Homebrew and Debian/Ubuntu APT distribution remain planned independently;
  packaged binaries must preserve canonical CLI semantics and version identity.

### Native FFI and bindings

- Maintain the C ABI and its applicable parity contract, ownership rules,
  errors, and conformance corpus.
- Binding readiness still requires a versioned Rust-to-C capability matrix,
  explicit gaps/N/A decisions, ABI compatibility guarantees, stable lifetime/error
  contracts and a shared adversarial conformance corpus. Existing C interoperability
  does not by itself qualify every future binding. See [interop](docs/INTEROP.md).
- Planned binding families: Swift; shared JVM Kotlin/Java; Python; Go;
  .NET/C#; and PHP. Each needs complete applicable FFI parity, lifetime/error
  semantics, adversarial conformance, packaging, and consumer tests before being
  marked supported.
- JavaScript/TypeScript remains covered by `cribra-wasm` for its documented
  applicable surface; a separate native binding requires concrete need.

### Developer integrations

- GitHub integration, Astro integration, agent/AI interoperability, and other
  application ecosystem integrations remain independently scoped.
- Repository traversal, application exposure policy, correlation, crawling,
  ownership decisions, and network validation are not core scanner features.

## Maintenance, detection policy, and security

The v0.4.x line established the conservative detection, contextual validation,
transformation, sensitive-data, and C/WASM interoperability foundation. Planned
catalog expansion was considered complete after v0.4.7; future detectors require
a significant, documented production gap rather than a desire to grow counts.

Ongoing work includes bug and security fixes, evidence-backed false-negative
improvements, false-positive hardening, compatibility with changed provider
formats, semantic and adapter parity, documentation, and measured performance.

Cribra does **not** target generic entropy classification, arbitrary Base64 or
hash harvesting, broad undifferentiated PII/DLP classification, speculative
provider detectors, AI-based secret classification, live credential verification,
cloud SDK dependencies, or application-specific policy engines. High confidence
and source-local semantics take precedence over speculative coverage.

## Historical release summary

| Release | Completed milestone |
| --- | --- |
| v0.4.7 | Sensitive-data and system-security completion; final built-in portfolio and hardening |
| v0.4.6 | Sensitive-data foundation and financial opt-in semantics |
| v0.4.5 | Developer ecosystem credential coverage and transformation parity |
| v0.4.4 | Canonical Cribra CLI |
| v0.4.3 | High-confidence detection coverage |
| v0.4.2 | Security hardening |
| v0.4.1 | WebAssembly publication |
| v0.4.0 | WebAssembly interoperability |
| v0.3 | Native C interoperability |
| v0.2 | Detection and transformation baseline |

Important retained detector decisions and historical source references are
collected in [architecture decision notes](docs/HISTORICAL_DECISIONS.md).

Historical detailed checklists and release-specific evidence belong in their
respective release documentation and Git history. This roadmap tracks current
commitments and verified completion; it does not turn superseded exploratory
phase notes into outstanding work.
