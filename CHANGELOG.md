# Changelog

All notable changes to Cribra are documented in this file.

The project follows semantic versioning from the first public release.

## [Unreleased]

Development toward `0.2.0` focuses on detection quality, explicit ambiguity
handling and presentation-agnostic explainability while preserving the
privacy-first, application-agnostic core architecture introduced in `0.1.0`.

### Added

#### Detection regression hardening

- Added a v0.2 corpus spanning representative positive detections, isolated
  contextual values, ambiguous review-only values and known false-positive
  families.
- Added adversarial coverage for malformed provider-shaped values, punctuation
  boundaries, opaque infrastructure identifiers and mixed contextual layouts.
- Added application-agnostic cross-format coverage across ENV, JSON, YAML,
  TOML, plain text and explicit CRLF input.
- Added regression coverage for source boundaries, Unicode surroundings,
  adjacent findings, deterministic collision ordering, partial overlaps and
  finding/candidate suppression semantics.
- Added collision-heavy serial/parallel equivalence coverage.
- Corpus expectations are treated as semantic contracts rather than generated
  snapshots.

#### Rule metadata and detection semantics

- Public `RuleMetadata` for presentation-safe rule introspection.
- Scanner-level rule metadata introspection backed by compiled rule metadata
  rather than duplicated public state.
- Public `DetectionMode` classification:
  - `MatcherOnly` for rules whose matcher is authoritative;
  - `Deterministic` for rules validated from candidate structure;
  - `Contextual` for rules whose validation depends on surrounding source
    context.
- Detection mode metadata derived from the existing validator authority.
- Regression coverage ensuring metadata and runtime detection semantics remain
  aligned.

#### Contextual detection quality

- Expanded contextual validation for generic credentials and sensitive values
  across common configuration syntaxes.
- More precise contextual treatment of generic authentication tokens,
  passwords, API keys and sensitive hashes.
- Shared placeholder rejection for contextual validators.
- Additional false-positive rejection for unrelated fields, placeholders and
  values that resemble credentials without sufficient semantic evidence.
- Format-invariance regression coverage across environment, JSON, TOML and YAML
  configuration forms.

#### Ambiguous sensitive values

- Public `SensitiveCandidate` model for structurally suspicious values that do
  not have enough evidence to become classified findings.
- Public candidate kind and evidence contracts for review-only results.
- Initial structural detection for narrowly defined recovery-like grouped
  values.
- Explicit separation between classified `Finding` values and ambiguous
  `SensitiveCandidate` values.
- Candidate emission integrated into the normal per-source scanning pipeline.
- Candidate storage in `ScanReport` as a separate immutable result collection.
- Batch-level candidate iteration through `ScanResults`.
- Candidate-aware report and result review helpers.
- Candidate aggregation in `ScanSummary`.
- Candidate-only source classification separate from failed and clean sources.
- Overlap suppression so an accepted finding takes precedence over an
  ambiguous candidate covering the same source span.
- Regression corpus for ambiguous recovery-like values and related
  false-positive families, including numeric groups, hexadecimal groups,
  placeholders, UUIDs, hashes, package-integrity values, version/date strings,
  case variants and malformed or extended grouped values.
- Serde support for the public candidate result contracts.

#### Explainability

- Public typed `Explanation` contract for presentation-agnostic result
  explainability.
- Classified explanations represented as
  `Explanation::Classified(DetectionMode)`.
- Ambiguous explanations represented as
  `Explanation::Ambiguous(CandidateEvidence)`.
- Finding explanation resolution through scanner-owned compiled rule metadata.
- Fail-closed finding explanation when the supplied scanner cannot resolve the
  finding unambiguously.
- Direct explanation projection for `SensitiveCandidate` from its existing
  candidate evidence.
- Convenience helpers for inspecting classified and ambiguous explanation
  variants.
- Optional Serde support for the standalone `Explanation` contract.
- Public regression coverage for classified/ambiguous separation, scanner
  authority, overlap suppression, serialization and source-value
  non-leakage.

#### Custom rules

- Scanner-wide uniqueness for `RuleId`, including duplicate custom IDs and
  custom/built-in collisions.
- Borrowed built-in catalogs such as `builtins::CURRENT` can be passed directly
  to `ScannerBuilder::builtins`.
- Public custom-rule regression contract covering literal, prefix and pattern
  rules, metadata, remediation, built-in composition and ambiguity promotion.
- Custom patterns that can produce zero-length matches are rejected at
  construction time with `RuleError::PatternMatchesEmpty`.
- Custom pattern coverage for Unicode, anchors, boundaries and alternation.
- Public custom patterns keep full-match span semantics; capture projection
  remains private to built-in rules.
- Identical matcher definitions remain valid when their stable rule IDs are
  distinct.
- Custom rules remain `MatcherOnly` and cannot select private built-in
  validators.

#### Performance regression hardening
- Added v0.2-specific Criterion coverage for candidate density, realistic mixed
  sources, typed explainability and ambiguity promotion.
- Added diagnostic isolation suites for matcher families, contextual-pattern
  scaling and the contextual rule tail.
- Profiled the expanded v0.2 built-in portfolio and isolated repeated
  full-source contextual regex work as the dominant regression.
- Added conservative contextual prefilters and localized contextual regex
  execution without changing matcher/validator authority.
- Added a shared contextual prefilter gate so clean-path cost no longer scales
  linearly with the contextual-pattern portfolio.
- Preserved rule-local authoritative validation after prefilter dispatch.
- Restored the clean 64 KiB no-finding path to approximately the historical
  v0.1 baseline while retaining the expanded v0.2 detection model.
- Recorded the final v0.2 engineering baseline in `docs/PERFORMANCE.md`.

### Changed

#### Result semantics

- `ScanReport` now models classified findings and ambiguous candidates as
  separate result channels.
- `ScanResults` now distinguishes:
  - failed sources containing classified findings;
  - review-only sources containing candidates but no findings;
  - clean sources containing neither.
- Clean/review semantics now account for ambiguous candidates without
  promoting them to findings.
- `ScanSummary` now reports candidate counts and candidate-bearing reports in
  addition to classified finding statistics.
- Explainability is derived from existing classification authorities rather
  than stored as duplicate state in findings, candidates, reports or batch
  results.

#### Privacy and transformation boundaries

- Ambiguous candidates remain metadata-only and never retain matched source
  values.
- Candidate evidence is explicitly distinct from finding `Confidence`,
  `Severity` and `Remediation`.
- Ambiguous candidates are excluded from `ScanQuery`.
- Share-safe transformations continue to operate only on classified findings;
  ambiguous candidates are never automatically redacted, templated,
  pseudonymized or synthesized.
- Serialized scan results do not embed derived explanation state.
- Explanation payloads contain typed metadata only and never source snippets or
  matched sensitive values.

#### Documentation

- Expanded the documented detection pipeline to show classified and structural
  review paths separately.
- Documented the distinction between `Finding`, `SensitiveCandidate`,
  `DetectionMode`, `CandidateEvidence` and `Explanation`.
- Documented scanner-owned metadata as the authority for finding
  explainability.
- Clarified that explainability provides typed facts to presentation layers
  rather than human-facing copy.
- Clarified the application boundary: Cribra itself performs no network
  access and operates exclusively on caller-provided input.
- Expanded regression and corpus documentation for contextual and ambiguous
  detection behavior.

## [0.1.0] - 2026-08-10

First public release of the reusable Cribra core.

### Added

#### Scanning and rules

- Deterministic UTF-8 scanning engine.
- Compiled rule execution.
- Literal, prefix, suffix and regex rules.
- Capture-aware regex projection.
- Public custom rule builder.
- Selectable built-in rule catalog.
- Deterministic and contextual validators.
- Deterministic finding normalization.
- Cached default built-in scanner.
- Ordered batch scanning.
- Optional Rayon-backed parallel batch scanning with serial/parallel result
  equivalence.

#### Results and querying

- `Finding`, `Location`, `Severity`, `Confidence` and `RuleId`.
- Immutable `ScanReport`.
- Generic keyed `ScanEntry<K>` and `ScanResults<K>`.
- Lazy `ScanQuery` filters for severity, confidence and rule identifiers.
- Explicit result sorting through `ScanSort`.
- `ScanSummary` aggregation and plain-text display.
- Optional remediation metadata with presentation-safe labels and messages.
- Optional Serde support for public data contracts.

#### Share-safe transformations

- Conservative redaction.
- Configurable redaction replacement.
- Semantic template generation.
- Deterministic keyed pseudonymization.
- Deterministic keyed synthetic replacements.
- Share bundles with explicit transformation mode, transformed keyed sources
  and manifest metadata.
- Strict span validation for transformations.
- Overlap rejection where semantic transformation would be ambiguous.

#### Privacy and source coordinates

- Public findings do not retain matched secret values.
- Zero-based half-open UTF-8 byte spans.
- One-based line coordinates.
- One-based Unicode scalar columns.
- Local-first serial core suitable for native and WASM/PWA integration.

#### Testing and performance

- Unit and doctest coverage across the public surface.
- Canonical input/output fixture corpus.
- Golden verification against outputs generated by the real crate.
- Serial/parallel equivalence tests.
- Transformation determinism and leakage checks.
- UTF-8, CRLF, empty-input, dense-finding and large-line hardening.
- Criterion benchmark suite for build cost, input size, match density, rule
  count, pipeline composition and batch parallelism.

### Release policy

The `0.1.0` core capability set is frozen for publication. Manual SIMD
optimization is deferred until real-world profiling demonstrates a concrete
bottleneck.