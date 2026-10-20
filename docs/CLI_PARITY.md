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
├── scan
│   ├── <FILE>
│   ├── -
│   ├── --format human|json
│   ├── --severity info|low|medium|high|critical
│   ├── --min-severity info|low|medium|high|critical
│   ├── --confidence low|medium|high
│   ├── --min-confidence low|medium|high
│   ├── --rule-id <ID>
│   ├── --explain
│   └── --sort rule-id|rule-id-desc|severity|severity-desc|
│              confidence|confidence-desc|location|location-desc
├── redact <FILE|->
├── template <FILE|->
├── pseudonymize <FILE|-> --key-file <PATH>
├── synthesize <FILE|-> --key-file <PATH>
└── bundle --mode <MODE> [--key-file <KEY>] --output <DIR> <FILE>...
    (parsing only; execution NOT IMPLEMENTED)
```

The five executable source commands accept exactly one explicit UTF-8
filesystem path or standard input. Keyed commands also require an explicit key
file. `bundle` accepts multiple explicit file paths for parsing only; its
execution is **NOT IMPLEMENTED** and it rejects stdin sources.

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
| Default scanner | EXPOSED | all five executable source commands use `Scanner::default()` |
| Explicit UTF-8 file input | EXPOSED | one explicit file per executable source command |
| Standard input | EXPOSED | `-` source input for the five executable commands; not accepted by `bundle` |
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
| Redaction | EXPOSED | `cribra redact` delegates to `cribra::transform::redact()` |
| Template transformation | EXPOSED | `cribra template` delegates to `cribra::transform::template()` |
| Pseudonymization | EXPOSED | `cribra pseudonymize` with mandatory `--key-file`, using Core defaults |
| Synthesis | EXPOSED | `cribra synthesize` with mandatory `--key-file`, using Core defaults |
| Share bundle | GAP | `bundle` parses requests only; execution NOT IMPLEMENTED |
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

## Redaction and semantic templating

```text
cribra redact <FILE|->
cribra template <FILE|->
```

Each command reads one explicit UTF-8 file or stdin through the same input
acquisition infrastructure as `scan`, scans with `Scanner::default()`, and
passes the original source and complete original report directly to the public
Core transformation API. All original findings are transformed without query
filters or sorting. The CLI does not implement replacement logic.

`redact` uses `cribra::transform::redact()` and replaces findings with
`[REDACTED]`. Core merges duplicate or overlapping spans into their complete
union before redaction; adjacent non-overlapping findings remain separate
replacements. `template` uses `cribra::transform::template()` and replaces each
finding with the default semantic placeholder `<CRIBRA:rule-id>`, without
numbering. Core rejects overlapping findings for templating because their
individual replacements would be ambiguous.

Successful commands emit only the exact Core-transformed UTF-8 text to stdout:
no metadata, summary, or added newline. Text outside transformed finding spans
is preserved byte-for-byte, including UTF-8, BOMs, CRLF, and trailing-newline
state. Empty and clean inputs remain unchanged. Neither command writes to the
input file or to an output file.

Candidates are not transformation targets and remain independent of findings.
Candidate-only sources remain unchanged; in mixed reports only finding spans
are transformed. **Output can still contain sensitive data**, including review
candidates and sensitive values the scanner did not classify. A successful
transformation is not a guarantee that all sensitive material was removed.

These commands accept no options or extra positional arguments. Scan-only
options (`--format`, `--explain`, filters, and sorting), custom replacement,
namespace, numbering, and output-file options are unsupported. Missing inputs,
unsupported options, or extra arguments return exit code `2`. Input and Core
transformation errors return `1` with empty stdout; diagnostics do not include
matched source values. Successful transformations return `0`.

## Keyed transformations

```text
cribra pseudonymize <FILE|-> --key-file <PATH>
cribra synthesize <FILE|-> --key-file <PATH>
```

`--key-file <PATH>` may appear before or after the source. Exactly one source
and one key-file option are required. Duplicate key options, missing values,
extra positional arguments, unknown options, and scan-only options are usage
errors. There are no inline, environment, generated, default, or stdin keys;
`--key-file -` is rejected. Keyed commands expose no custom prefix, marker,
filtering, sorting, or output-file options.

The key file must resolve to a regular file containing **exactly 32 raw binary
bytes**. Symlinks to regular files are permitted; directories, devices, sockets,
and named pipes are rejected. Key loading reads at most 33 bytes, rejecting
shorter or longer files. Hex and Base64 text are not decoded. Whitespace,
newlines, NULs, and arbitrary binary values are neither trimmed nor normalized;
a trailing newline is part of the key and counts toward the 32 bytes.

The key is read and validated before any source acquisition, including source
stdin. The CLI does not print or serialize key contents, store key bytes in the
public command model, or include them in diagnostics. It does not modify the
key file. Key bytes are held in local execution state and passed to Core options
without explicit cloning. **Memory zeroization is not guaranteed.**

Both commands scan with `Scanner::default()` and pass the complete original
report and original source to `cribra::transform::pseudonymize()` or
`cribra::transform::synthesize()`, using only
`PseudonymizationOptions::new([u8; 32])` or
`SynthesisOptions::new([u8; 32])`. The CLI does not implement replacement logic.
Core rejects overlapping findings for both keyed transformations. Successful
stdout contains only exact Core-transformed UTF-8 bytes, without headers,
summaries, or an added newline. Neither input nor key files are modified, and
the CLI writes no output file. Text outside finding spans retains its original
UTF-8, BOM, CRLF, and trailing-newline bytes.

Pseudonymization is deterministic for a matched value and key: repeated equal
values receive the same pseudonym even at different positions or in different
sources. Reusing a key therefore enables correlation across runs and datasets.
Synthesis is deterministic for the key, rule ID, and source span; repeated
equal source values at different positions need not receive equal replacements.
For supported providers, Core preserves recognizable structure while breaking
validation invariants; contextual/generic output uses synthetic markers. These
values do not retain the original matched secret and are not provider-verified
credentials.

Reusing a key preserves reproducibility; changing it changes pseudonyms and
suitable synthesized fixtures. Manage key creation and storage outside the CLI,
restrict file access, keep keys out of shared outputs and version control, and
use separate keys where cross-dataset correlation is unwanted. Losing a key
loses reproducibility for outputs made with it. Key reuse is an explicit choice,
not an assurance of anonymity.

Candidates are not transformed independently. Empty, clean, and candidate-only
sources remain unchanged; mixed sources transform only classified finding
spans. Output can still contain sensitive candidates and undetected values.
Key-file bytes independently present in unchanged source text also remain;
the CLI does not scrub arbitrary text outside findings.

Usage failures return `2`. Dedicated key-read/non-regular-file and invalid-key-
length errors, source input failures, and Core transformation failures return
`1` with empty stdout. Diagnostics contain no source values or key contents.
Successful execution returns `0`. Existing scan, redaction, and template output
contracts are unchanged.

## Proposed bundle syntax: parsing only

**Bundle execution is NOT IMPLEMENTED.** The public `BundleCommand` and
`BundleMode` model and CLI parser accept these proposed forms:

```text
cribra bundle --mode <MODE> --output <DIR> <FILE>...
cribra bundle --mode <MODE> --key-file <KEY> --output <DIR> <FILE>...
```

Modes are `redact`, `template`, `pseudonymize`, and `synthesize`. Exactly one
`--mode` and one `--output` are required, with at least one explicit source file
path. `pseudonymize` and `synthesize` require exactly one `--key-file`;
`redact` and `template` reject that option. Options may appear before, between,
or after source paths. Source argument order is preserved.

Parsing rejects duplicate options, duplicate source paths by `PathBuf` equality,
missing option values, empty paths, unknown modes, and unsupported flags.
Source stdin `-` is rejected. Source paths beginning with `-` must be expressed
as filesystem paths such as `./-name`. Key and output paths follow the existing
keyed-command path conventions: nonempty filesystem paths, with `-` rejected;
use an explicit relative path for a filename beginning with `-`.

Parsing performs no filesystem access, canonicalization, key-byte reading or
validation, recursive discovery, glob expansion, or implicit input selection.
Non-UTF-8 filesystem paths are retained where supported. `--output` is only a
recorded proposed directory path: this syntax does not promise any export
layout, manifest, output naming, or overwrite behavior.

Every parsed bundle request returns
`ExecuteError::UnsupportedOperation("bundle execution is not implemented")`.
The process exits `1` with empty
stdout and the safe diagnostic `bundle execution is not implemented`. It does
not read sources or keys, scan, call `ShareBundle`, create directories, or write
files. Usage errors return `2`. ShareBundle parity remains **GAP** until an
actual execution contract is implemented and validated.

## Exit semantics

The current process exit contract is:

| Code | Meaning |
| ---: | --- |
| `0` | successful execution, including clean, review-only, and finding-bearing scans |
| `1` | key, input, transformation, unavailable bundle execution, or other execution failure |
| `2` | command-line usage or parse failure |

Finding presence is deliberately not an execution failure.

Any future CI-oriented failure policy should therefore be explicit rather than
silently changing the meaning of the existing exit code `0`.

## Privacy invariant

`scan` presentation is metadata-only.

The command layer must not independently add matched secret values, candidate
source material, or source snippets to scan output or diagnostics.

Current tests cover source-value non-disclosure for classified findings,
review-only candidates, JSON output, and invalid UTF-8 diagnostics.

All four transformation commands emit transformed source as their primary
result. Core replaces classified findings; candidates and undetected material
may remain in that source. Errors emit no source text or partial transformed
stdout.

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

### Share bundles and CI exit policy

Core share bundles remain a separate CLI parity gap: `bundle` has a parsing and
public model surface, but execution is NOT IMPLEMENTED. The executable keyed
commands accept one source and do not add bundle orchestration or manifests. A
CI-oriented failure-on-findings policy also remains separate from the existing
execution exit codes. Future commands must preserve Core authority and the
transformation privacy model.

## Deliberate non-parity

Not every programmable core surface should become CLI syntax.

Custom rule construction and arbitrary `ScannerBuilder` composition are
currently treated as library responsibilities. They are suitable for embedded
consumers that require programmatic scanner construction and are not considered
CLI parity defects.

Likewise, parallel execution is not independently useful while executable source
commands each accept exactly one source.

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
2. evaluate multi-source and share-bundle command semantics separately;
3. evaluate explicit CI failure policy without changing existing exit semantics.

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