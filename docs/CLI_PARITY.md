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
    ├── --format human|json
    ├── --severity info|low|medium|high|critical
    ├── --min-severity info|low|medium|high|critical
    ├── --confidence low|medium|high
    ├── --min-confidence low|medium|high
    ├── --rule-id <ID>
    ├── --explain
    └── --sort rule-id|rule-id-desc|severity|severity-desc|
               confidence|confidence-desc|location|location-desc
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
| Candidate kind | EXPOSED | human and JSON output |
| Clean/review/findings status | EXPOSED | deterministic aggregate status |
| Metadata-only finding output | EXPOSED | matched source values are not intentionally rendered |
| Human representation | EXPOSED | default output format |
| JSON representation | EXPOSED | deterministic metadata representation |
| `ScanQuery` filtering | EXPOSED | composable finding filters delegated to `results.query()` |
| Exact severity filter | EXPOSED | `--severity` delegates to `query.severity()` |
| Minimum severity filter | EXPOSED | `--min-severity` delegates to `query.minimum_severity()` |
| Exact confidence filter | EXPOSED | `--confidence` delegates to `query.confidence()` |
| Minimum confidence filter | EXPOSED | `--min-confidence` delegates to `query.minimum_confidence()` |
| Rule-ID filter | EXPOSED | `--rule-id` delegates to `query.rule_id()` |
| Critical/high-priority/high-confidence query helpers | EXPOSED | equivalent constraints: `--severity critical`, `--min-severity high`, `--confidence high` |
| `ScanSort` | EXPOSED | `--sort` delegates to `query.sort()` for rule ID, severity, confidence, or location, in either direction |
| Aggregate `ScanSummary` | GAP | current CLI scans exactly one source |
| Finding explanation | EXPOSED | `--explain` resolves rendered findings through `finding.explanation(&scanner)` |
| Detection mode | EXPOSED | opt-in human `detection=` and JSON `detection_mode` from the public `Explanation` API |
| Candidate explanation | EXPOSED | existing candidate evidence projects `Explanation::Ambiguous` facts |
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

For review-only candidates, the current CLI projects location, kind, and evidence
in both human and JSON output formats.

The CLI derives one aggregate source status:

```text
findings   at least one classified finding
review     no findings and at least one review candidate
clean      no findings and no review candidates
```

Candidates therefore remain distinct from classified findings and do not
independently convert a successful scan into a finding-bearing result.

## Finding selection

Any finding filter or explicit sort activates selection, even when it selects
all findings or no findings. Neither `--format` nor `--explain` alone activates
selection. Without a filter or sort option, output contains no selection
metadata.

Filters compose through the public core `ScanQuery` API. Exact and minimum
constraints within the same dimension are mutually exclusive; duplicate
options are rejected. `--rule-id` requires a nonempty exact identifier.
Unsorted selection preserves core query iteration order. Explicit sorting uses
the core's ordering, including deterministic tie-breaking. Source/source-desc
sorting is not accepted because the CLI scans one source.

Selection changes only the rendered findings and their order. Source status,
`findings_count`, and `candidates_count` in JSON remain derived from the original
`ScanReport`. Human `findings:` and `candidates:` counters have the same original
report semantics. An empty selection therefore does not turn a finding-bearing
report into a clean report. Candidates remain unfiltered, in their original
order, with unchanged metadata.

With active selection, JSON additionally includes
`"selection":{"active":true,"findings_count":N}`. This nested count equals the
number of objects in the rendered `findings` array; the original top-level
fields and their types are retained. Human output adds `selected findings: N`
immediately after the original counters. Selection remains metadata-only and
does not change input handling or exit codes.

## Finding explainability

`--explain` is a valueless, opt-in scan flag; duplicate uses are rejected.
When absent, existing human and JSON output remain byte-for-byte unchanged,
including output with finding filters or sorting. With the flag present, each
rendered finding is resolved against the scanner that produced it through
`finding.explanation(&scanner)`. Detection mode comes exclusively from the
public `Explanation::detection_mode()` API.

Human finding lines append `detection=matcher-only`, `detection=deterministic`,
or `detection=contextual`. JSON finding objects append the `detection_mode`
field with the same string values after their existing metadata fields. If
explanation resolution returns `None` or yields no classified detection mode,
human output appends `detection=unknown` and JSON appends
`"detection_mode":null`. The CLI does not infer a mode from other finding
metadata.

Explanations are resolved only for rendered findings, after any filtering and
sorting. The flag alone does not activate selection, and it leaves original
status, counters, candidate metadata, input handling, and exit codes unchanged.
Clean and candidate-only scans therefore produce the same output with or
without `--explain`.

Candidates remain ambiguous review items. Existing human `evidence=structural`
and JSON `"evidence":"structural"` already project the evidence represented by
`Explanation::Ambiguous`; the flag adds no duplicate explanation or detection
fields to candidates. All explanation output remains metadata-only and never
contains matched values or source snippets.

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
- `CandidateEvidence`;
- `DetectionMode` (with `--explain`).

Some forward-compatible enum variants are currently rendered as `unknown`.

This means successful compilation against a newer core does not by itself prove
semantic CLI parity. A new public core variant can remain source-compatible
while losing specificity at the CLI presentation boundary.

Parity review must therefore include semantic projection changes, not only API
or build compatibility.

## Confirmed parity gaps

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
2. expose non-keyed transformations such as redaction and templating;
3. design keyed pseudonymization and synthesis with an explicit key-handling
   contract;
4. evaluate multi-source and share-bundle command semantics separately;
5. evaluate explicit CI failure policy without changing existing exit semantics.

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