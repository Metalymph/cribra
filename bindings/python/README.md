# Local Python Core consumer qualification (NB-2.2)

A minimal consumer of the existing `cribra-uniffi` crate and UniFFI **0.32.2**
generator. No PyO3, second Rust adapter, hand-written Python API, package install,
wheel, publishing or CI integration is introduced.

From the repository root, with Rust **1.97.0**, Python **3.13+** and `just`:

```sh
just uniffi-python-smoke
# Select the interpreter explicitly if needed:
just uniffi-python-smoke /path/to/python3.13
# Equivalent direct invocation:
python3.13 bindings/python/smoke.py
```

The script builds the **Core-only** cdylib, generates `cribra_uniffi.py`, places
the matching native library beside it for UniFFI's normal loader, runs the
existing direct Rust Core `scanner_oracle`, and executes standard-library
`unittest` consumer tests. It uses the same Python interpreter for the tests.
There are no pip dependencies. Missing tools, build failures or failing tests
return a nonzero exit status.

All generated bindings, native libraries, Cargo outputs and the oracle JSON stay
under ignored `target/uniffi-python-consumer/`. This separate Cargo directory
preserves existing Swift and Core/Flow/Fuse build outputs. No generated code or
native binary belongs in this source directory. Python bytecode writes are
disabled for the consumer run.

The **25 standard-library tests** reuse `scanner.json`, `configurations.json`
and `incremental.json` in `crates/cribra-uniffi/tests/fixtures/`. Expected reports,
summaries, rule metadata, error fields and incremental traces come from the
existing `scanner_oracle`, regenerated against Rust Core on every run. Python
only constructs DTOs, projects results and applies accepted byte edits to inputs
for full-scan comparisons; it does not detect secrets or validate edits/rules.

Qualified coverage:

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
- Core-only bindings exclude Flow/Share functions and representative DTOs/errors,
  plus Fuse parallel scanning.

Stable error codes covered are `DuplicateRuleId`, `EmptyRuleId`, `EmptyMatcher`,
`InvalidPattern`, `PatternMatchesEmpty`, `UnsupportedRuleSet`, `InvalidUtf8`,
`IncompleteUtf8`, `NotActive`, `CandidateLimitExceeded`, `ResultLimitExceeded`,
`StorageLimitExceeded`, `StaleRevision`, `InvalidRange`, `InvalidUtf8Boundary`,
`CacheLimitExceeded` and `TransactionStorageLimitExceeded`. Optional revision,
limit/required and terminal fields are checked where applicable. Human-readable
diagnostics are compared with the current oracle, not promised stable across
Core versions.

Qualified locally with **Python 3.13.7 on macOS ARM64**. The script recognizes
native library names on macOS, Linux and Windows; other platforms/interpreters
are not qualified by this step. Python Flow/Fuse consumers and distribution
packaging remain outside scope. Streaming supports only rulesets admitted by
Core; built-in scanning remains available through single/batch/incremental APIs.
Deliberate panic/mutex poisoning, revision overflow, allocator exhaustion, huge
rule-count/automaton failures and concurrent Python calls are not qualified here.
Incremental sessions have Active/Failed states, with no finish operation.
Garbage-collection checks establish usable owned results and session lifetimes,
not a native memory-leak or performance guarantee. Rust and Swift APIs/features
are unchanged; their separate regression suites are not run by this command.
