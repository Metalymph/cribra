# CLI Capability Parity

## Purpose

This document records the capability relationship between the public `cribra`
Rust core and the canonical `cribra-cli` command surface.

CLI parity does not mean mechanically exposing every public Rust API as a
command-line option. It means that core capabilities which form useful,
coherent command-line operations have a canonical CLI projection, while
library-only APIs and intentionally excluded responsibilities remain explicit.

The `cribra` core remains the semantic authority. `cribra-cli` owns command
parsing, explicit input acquisition, command execution, and presentation. It
must not duplicate detection, validation, candidate classification, remediation,
explainability, or transformation semantics.

## Parity states

Capabilities in this document use the following states.

### EXPOSED

The capability has a canonical CLI projection backed by the corresponding core
semantics.

### PARTIAL

The capability is projected by the CLI, but some public semantic or
presentation information available from the core is not currently represented.

### GAP

The core exposes a capability that forms a useful command-line operation, but
the canonical CLI does not currently expose it.

A GAP identifies missing CLI surface. It does not by itself require a specific
syntax or guarantee inclusion in the next release.

### LIBRARY-ONLY

The capability is intentionally available through the Rust library without a
corresponding CLI surface.

Library-only capabilities are not parity defects.

### OUT OF SCOPE

The responsibility is intentionally outside the current CLI contract and is not
a parity defect.

## Current CLI contract

The canonical v0.4.4 command surface is intentionally small:

```text
cribra
├── --help
├── --version
└── scan
    ├── <FILE>
    ├── -
    └── --format
        ├── human
        └── json
```

`scan` accepts exactly one explicit UTF-8 filesystem path or standard input.

Input acquisition preserves valid UTF-8 text without newline normalization,
BOM stripping, lossy decoding, or encoding auto-detection. Invalid UTF-8 is
rejected.

The CLI does not currently perform:

- directory traversal;
- repository walking;
- glob expansion;
- automatic encoding conversion.

These are explicit boundary decisions rather than accidental omissions.

## Architecture boundary

The dependency direction is:

```text
standalone `cribra`
        │
        ▼
    cribra-cli
    ├── parser
    ├── command model
    ├── input acquisition
    ├── execution
    └── rendering
        │
        ▼
      cribra
      └── semantic authority
```

`cribra-cli` depends on the public `cribra` API and delegates scanning to
`Scanner`.

The CLI must not independently implement detector or validator behavior.

## Current capability matrix

| Core capability | CLI state | Current projection |
| --- | --- | --- |
| Default scanner | EXPOSED | `scan` uses `Scanner::default()` |
| Explicit UTF-8 file input | EXPOSED | `cribra scan <FILE>` |
| Standard input | EXPOSED | `cribra scan -` |
| Findings | EXPOSED | human and JSON output |
| Rule identity | EXPOSED | rule ID |
| Byte location | EXPOSED | start and end offsets |
| Line and column location | EXPOSED | one-based line and Unicode scalar column |
| Severity | EXPOSED | human and JSON output |
| Confidence | EXPOSED | human and JSON output |
| Remediation | EXPOSED | human and JSON output |
| Sensitive candidates | EXPOSED | represented separately from findings |
| Candidate evidence | EXPOSED | human and JSON output |
| Candidate kind | PARTIAL | JSON exposes kind; human output currently does not |
| Clean/review/findings status | EXPOSED | deterministic aggregate status |
| Metadata-only finding output | EXPOSED | matched source values are not intentionally rendered |
| Human representation | EXPOSED | default output format |
| JSON representation | EXPOSED | deterministic metadata representation |
| `ScanQuery` filtering | GAP | no CLI filtering surface |
| Exact severity filter | GAP | available in the core query API only |
| Minimum severity filter | GAP | available in the core query API only |
| Exact confidence filter | GAP | available in the core query API only |
| Minimum confidence filter | GAP | available in the core query API only |
| Rule-ID filter | GAP | available in the core query API only |
| Critical/high-priority/high-confidence query helpers | GAP | available in the core query API only |
| `ScanSort` | GAP | no CLI sorting surface |
| Aggregate `ScanSummary` | GAP | current CLI scans exactly one source |
| Finding explanation | GAP | `Explanation` is not projected |
| Detection mode | GAP | classified finding detection mode is not projected |
| Candidate explanation | PARTIAL | evidence is exposed, but the `Explanation` projection is not |
| Redaction | GAP | core transformation has no CLI operation |
| Template transformation | GAP | core transformation has no CLI operation |
| Pseudonymization | GAP | core transformation has no CLI operation |
| Synthesis | GAP | core transformation has no CLI operation |
| Share bundle | GAP | core transformation/batch capability has no CLI operation |
| Custom `Rule` construction | LIBRARY-ONLY | programmable Rust API |
| Custom `ScannerBuilder` construction | LIBRARY-ONLY | programmable Rust API |
| Parallel scanning | LIBRARY-ONLY | current CLI accepts one source |
| Directory traversal | OUT OF SCOPE | explicitly excluded from current CLI input contract |
| Repository walking | OUT OF SCOPE | explicitly excluded from current CLI input contract |
| Glob expansion | OUT OF SCOPE | explicitly excluded from current CLI input contract |
| Encoding conversion | OUT OF SCOPE | CLI accepts UTF-8 without automatic conversion |

## Result projection

For classified findings, the current CLI projects:

- rule ID;
- start byte offset;
- end byte offset;
- line;
- column;
- severity;
- confidence;
- remediation.

For review-only candidates, the current CLI projects location and evidence in
both output formats. JSON additionally projects candidate kind.

The CLI derives one aggregate source status:

```text
findings   at least one classified finding
review     no findings and at least one review candidate
clean      no findings and no review candidates
```

Candidates therefore remain distinct from classified findings and do not
independently convert a successful scan into a finding-bearing result.

## Exit semantics

The current process exit contract is:

| Code | Meaning |
| ---: | --- |
| `0` | successful execution, including clean, review-only, and finding-bearing scans |
| `1` | input or execution failure |
| `2` | command-line usage or parse failure |

Finding presence is deliberately not an execution failure.

Any future CI-oriented failure policy should therefore be explicit rather than
silently changing the meaning of the existing exit code `0`.

## Privacy invariant

CLI presentation is metadata-only.

The command layer must not independently add matched secret values, candidate
source material, or source snippets to normal output or diagnostics.

Current tests cover source-value non-disclosure for classified findings,
review-only candidates, JSON output, and invalid UTF-8 diagnostics.

Adding a new output mode or capability must preserve this boundary unless a
future operation explicitly requires transformed source output as its primary
result.

## Semantic projection and drift

The CLI owns a presentation projection from public core enums to stable textual
representations.

Current examples include:

- `Severity`;
- `Confidence`;
- `Remediation`;
- `SensitiveCandidateKind`;
- `CandidateEvidence`.

Some forward-compatible enum variants are currently rendered as `unknown`.

This means successful compilation against a newer core does not by itself prove
semantic CLI parity. A new public core variant can remain source-compatible
while losing specificity at the CLI presentation boundary.

Parity review must therefore include semantic projection changes, not only API
or build compatibility.

## Confirmed parity gaps

### Querying and sorting

The core provides composable `ScanQuery` filtering by severity, confidence, rule
identity, priority helpers, and explicit `ScanSort`.

The canonical CLI currently exposes none of these operations.

This is a genuine command-surface gap because filtering and sorting existing
scan metadata are coherent shell and pipeline operations and do not require the
CLI to duplicate core semantics.

### Explainability

The core distinguishes classified and ambiguous explanations through
`Explanation`, `DetectionMode`, and `CandidateEvidence`.

The CLI currently exposes finding metadata and candidate evidence, but does not
project the core explanation model.

Candidate explainability is therefore partial, while classified finding
explainability is currently absent.

### Transformations

The core exposes explicit share-safe transformations:

- redaction;
- templating;
- keyed pseudonymization;
- keyed synthesis;
- share bundles.

The canonical CLI currently exposes scanning only.

Transformation commands are genuine parity gaps, but their command design must
preserve the core privacy model and must not duplicate transformation logic in
`cribra-cli`.

## Deliberate non-parity

Not every programmable core surface should become CLI syntax.

Custom rule construction and arbitrary `ScannerBuilder` composition are
currently treated as library responsibilities. They are suitable for embedded
consumers that require programmatic scanner construction and are not considered
CLI parity defects.

Likewise, parallel execution is not independently useful while the CLI accepts
exactly one source.

These classifications may be revisited if the command model itself expands.

## Input orchestration is separate from semantic parity

Multi-source orchestration, directory discovery, repository walking, and glob
expansion must not be conflated.

A future CLI could support multiple explicit sources or a core `ShareBundle`
operation without assuming responsibility for recursive filesystem discovery.

Conversely, adding filesystem traversal would be a command-product decision,
not a requirement imposed merely by the existence of batch-oriented core APIs.

## Parity priorities

The current audit suggests the following implementation order if the gaps are
accepted for future CLI work:

1. complete small metadata projection gaps;
2. expose core-backed querying and sorting;
3. expose explainability;
4. expose non-keyed transformations such as redaction and templating;
5. design keyed pseudonymization and synthesis with an explicit key-handling
   contract;
6. evaluate multi-source and share-bundle command semantics separately;
7. evaluate explicit CI failure policy without changing existing exit semantics.

This ordering is advisory. Each capability should be implemented as an
independent, reviewable slice rather than as a broad CLI rewrite.

## Parity policy

Future public core capabilities should be reviewed against this document.

For each capability, maintainers should determine whether it is:

- already represented by the CLI;
- partially represented;
- a useful command-line operation and therefore a parity gap;
- intentionally library-only;
- outside the CLI responsibility boundary.

A core API addition does not automatically require CLI syntax.

Conversely, a capability classified as CLI-relevant should not be independently
reimplemented in `cribra-cli`; the CLI should project the authoritative core
behavior.

The objective is semantic capability parity where command-line use is
appropriate, not surface-area equality between Rust APIs and command-line
options.