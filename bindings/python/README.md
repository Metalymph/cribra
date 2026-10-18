# Local Python Core/Flow/Fuse consumer qualification (NB-2.4)

A minimal consumer of the existing `cribra-uniffi` crate and UniFFI **0.32.2**
generator. No PyO3, second Rust adapter, hand-written Python API, package install,
wheel, publishing or CI integration is introduced.

From the repository root, with Rust **1.97.0**, Python **3.13+** and `just`:

```sh
just uniffi-python-smoke
just uniffi-python-flow-smoke
just uniffi-python-fuse-smoke
# Select the interpreter explicitly if needed:
just uniffi-python-smoke /path/to/python3.13
just uniffi-python-flow-smoke /path/to/python3.13
just uniffi-python-fuse-smoke /path/to/python3.13
# Equivalent direct invocations (core is the default):
python3.13 bindings/python/smoke.py
python3.13 bindings/python/smoke.py core
python3.13 bindings/python/smoke.py flow
python3.13 bindings/python/smoke.py fuse
```

The runner accepts only `core|flow|fuse`. It builds the existing cdylib with
`--no-default-features --features <profile>`, generates `cribra_uniffi.py`, and
places the matching native library beside it for UniFFI's normal loader. All
profiles regenerate the direct Rust Core `scanner_oracle` and run the 25 Core
consumer tests. Flow and Fuse additionally regenerate `flow_oracle` and
`share_oracle` and run 15 Flow/Share tests (40 tests total). Fuse also regenerates
`parallel_oracle` and runs three parallel-scanning tests (43 tests total).
All are standard-library
`unittest` tests, using the same Python interpreter as the runner. There are no
pip dependencies. Missing tools, build failures or failing tests return a nonzero
exit status.

All generated bindings, native libraries, Cargo outputs and the oracle JSON stay
under ignored `target/uniffi-python-consumer/<profile>/`, with separate `cargo/`
and `generated/` directories and oracle JSON files for each profile. Core, Flow
and Fuse never load one another's native library or generated module; each test
suite runs in a fresh Python process. Existing Swift and other Core/Flow/Fuse
build outputs are preserved. No generated code or native binary belongs in this
source directory. Python bytecode writes are disabled for the consumer run.

The **25 standard-library tests** reuse `scanner.json`, `configurations.json`
and `incremental.json` in `crates/cribra-uniffi/tests/fixtures/`. Expected reports,
summaries, rule metadata, error fields and incremental traces come from the
existing `scanner_oracle`, regenerated against Rust Core on every run. Python
only constructs DTOs, projects results and applies accepted byte edits to inputs
for full-scan comparisons; it does not detect secrets or validate edits/rules.

Core coverage (also exercised in the Flow and Fuse builds):

- Single and batch scans: order, duplicate/Unicode/empty source IDs, empty batches,
  empty/clean sources, all summary counters, single/batch parity and repeated calls.
  Candidate-only reports retain empty findings, their candidate count and review
  status; candidates are never counted as findings.
- Configuration: built-in-only, custom-only, combined and empty custom rules;
  Literal, Prefix, Suffix and Pattern; active metadata, deterministic order,
  severity and remediation. Custom-only adds no implicit built-ins.
- Configuration failures: duplicate IDs and built-in collisions, empty IDs and
  matchers, malformed/unsupported/oversized regexes, empty/nullable/zero-width
  patterns and error precedence. Codes, optional rule IDs and diagnostic details
  are compared to Core. A whitespace-only ID is accepted as Core specifies.
- Streaming: Python `bytes`, every two-chunk byte split of the Unicode fixture,
  one-byte and larger chunks, CRLF/emoji/combining marks, empty chunks/sources,
  findings/candidates, Active/Finished/Failed, repeated finish and push after
  completion/failure. Invalid UTF-8 and incomplete EOF sequences fail terminally
  without returning a partial report. Storage admission (including the exact
  reported requirement), candidate/result limits during push and EOF failures
  match Core. Built-ins and an unbounded regex are rejected without fallback;
  the bounded custom fixture and empty ruleset are supported.
- Incremental: initialization across rulesets, insert/delete/replace/no-op edits,
  byte ranges, revision increments, snapshots and legacy revision/report accessors.
  Stale revisions, reversed/out-of-range spans and UTF-8 scalar splits are
  recoverable and preserve the snapshot. Candidate/cache/result/transaction
  storage limits are tested at construction and during edits; terminal failure
  preserves the last valid snapshot and rejects subsequent edits. Every accepted
  fixture edit matches both the direct Core trace and a full scan.
- Ownership: repeated use, sessions surviving Scanner garbage collection,
  reports/snapshots surviving Scanner/session destruction, and detached DTO
  mutation leaving native state and other reports unchanged.
- Core-only bindings exclude every exported Flow/Share function, DTO and error
  type, plus Fuse parallel scanning.

Stable error codes covered are `DuplicateRuleId`, `EmptyRuleId`, `EmptyMatcher`,
`InvalidPattern`, `PatternMatchesEmpty`, `UnsupportedRuleSet`, `InvalidUtf8`,
`IncompleteUtf8`, `NotActive`, `CandidateLimitExceeded`, `ResultLimitExceeded`,
`StorageLimitExceeded`, `StaleRevision`, `InvalidRange`, `InvalidUtf8Boundary`,
`CacheLimitExceeded` and `TransactionStorageLimitExceeded`. Optional revision,
limit/required and terminal fields are checked where applicable. Human-readable
diagnostics are compared with the current oracle, not promised stable across
Core versions.

Flow/Share coverage reuses `flow.json`, `keyed-flow.json`, `keyed-options.json`
and `share.json`, plus the same public synthetic key vectors as the Rust/Swift
tests. Python constructs DTOs and compares semantic oracle results; it implements
no transformation, hashing or synthesis algorithm.

- Redact and Template: 23 fixtures, 46 oracle outcomes covering default/custom
  replacement, deletion, namespace normalization, numbering, unsorted/adjacent/
  duplicate/overlapping spans, empty input and Unicode/CRLF/emoji/combining marks.
  Redact merges overlaps; Template rejects them, exactly as Core does.
- Pseudonymize and Synthesize: 27 inputs × 9 option sets × 2 transforms = 486
  oracle outcomes, each called twice to check deterministic output. This includes
  different explicit keys, prefixes, synthesis markers, provider synthesis and
  Core's content semantics. Digest sizes 0, 1, 8, 16, 32, 33 and `u64::MAX` follow
  Core's clamping to 8..32 on the qualified 64-bit target; 33 is not an error.
- Structured `TransformError` codes and fields: `InvalidSpan`,
  `InvalidUtf8Boundary`, `OverlappingSpans`, plus `InvalidKeyLength` for lengths
  0, 1, 31, 33, 64 and 1024. Invalid keys take precedence over invalid spans.
  Outputs/errors do not echo the public key vectors. All transforms leave the
  source untouched when no findings are supplied; no implicit scan is requested.
- Share `scan_for_share`, `scan_and_build` and `build`: all four modes, five
  batches, both build paths (40 oracle outcomes), and 23 explicit-finding inputs
  across all modes (92 oracle outcomes). Tests compare source order, Unicode and
  duplicate IDs, transformed bytes, manifest mode and every summary counter.
  Source-free Share reports preserve finding and candidate metadata; candidate-only
  reports remain distinct and candidate text is not transformed as a finding.
- Each successful bundle timestamp is checked for valid seconds/nanoseconds and
  the current call's time window. Independent timestamps are never compared for
  equality, including when checking atomic/precomputed paths against the oracle.
- Structured `ShareError`: `MissingShareMode`, `SourceCountMismatch`,
  `SourceLengthMismatch` (UTF-8 byte counts and batch index), and nested transform
  failures, including invalid keys in both build paths. Missing-mode/count/length
  and invalid-key cases use the existing Swift reference expectations because
  the Core oracles do not emit those cases. Core checks byte lengths, not source
  identity: equal-length changed content is accepted. Explicit reports are used
  without rescanning.
- Feature gating: Core excludes Flow/Share APIs and `Scanner.scan_for_share`;
  Flow and Fuse expose them and retain all Core APIs. `parallel_scan` is present
  exclusively in Fuse.

Fuse parallel coverage reuses `parallel.json`, `parallel_oracle`, the existing
Core report projection and configuration helpers. It exercises the same 44
scenarios as the Rust/Swift consumers: built-in-only, custom-only, combined and
empty rulesets, each across an empty batch, seven single sources, the original
batch, reversed order and a 64-source repeated batch.

- Every scenario compares complete parallel reports and all summary counters
  with both the direct Core oracle and sequential `scan_many`, repeated eight
  times on the same Scanner (352 parallel/sequential comparisons).
- Checks preserve source order, duplicate/empty/Unicode IDs (including distinct
  composed/decomposed UTF-8 encodings), source byte lengths, finding identity,
  severity/confidence, UTF-8 coordinates, candidate counts and review status.
  The mixed fixture exercises every severity counter and candidate-only reports
  without promoting candidates to findings.
- Owned parallel results remain usable after Scanner garbage collection.
  Python calls the native synchronous API directly; the tests introduce no
  Python scheduling, thread pool or parallelism.

Qualified locally with **Python 3.13.7 on macOS ARM64**. The script recognizes
native library names on macOS, Linux and Windows; other platforms/interpreters
are not qualified by this step. Distribution packaging remains outside scope.
Parallel tests qualify observable results and repeatability, not speedup, worker
counts, scheduling or concurrent Python callers.
Representation/unknown-future-Core transform errors are
not deliberately forced, and 32-bit digest conversion is not qualified.
Streaming supports only rulesets admitted by Core; built-in scanning remains
available through single/batch/incremental APIs.
Deliberate panic/mutex poisoning, revision overflow, allocator exhaustion, huge
rule-count/automaton failures and concurrent Python calls are not qualified here.
Incremental sessions have Active/Failed states, with no finish operation.
Garbage-collection checks establish usable owned results and session lifetimes,
not a native memory-leak or performance guarantee. Rust and Swift APIs/features
are unchanged; their separate regression suites are not run by this command.
