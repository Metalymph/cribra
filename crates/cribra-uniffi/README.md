# cribra-uniffi

See [Getting started](../../docs/GETTING_STARTED.md) and
[Compatibility](../../docs/COMPATIBILITY.md) for integration and distribution status.

NB-1A shared UniFFI foundation, based on Cribra Core v0.7.1. Requires Rust
1.97 and pins UniFFI to `=0.32.2`. The library produces `cdylib`, `rlib` and `staticlib`
artifacts using proc-macro scaffolding without UDL. A build script embeds the
Core package version.

Owned `Location`, `Severity`, `Confidence`, `RuleId`, and `Finding` DTOs
are exported. Coordinates use `u64`: zero-based UTF-8 byte offsets with an
exclusive end, and one-based lines and Unicode-scalar columns. Core-to-DTO
conversions preserve coordinates on supported 32-bit and 64-bit Rust targets.
Consumer-created records do not validate themselves. Flow/Share inputs are
transferred through Core serde; Core owns range and transformation validation. Findings never copy the matched sensitive text.

Apple Swift Package/XCFramework distribution is documented in
[`bindings/swift/README.md`](../../bindings/swift/README.md) (NB-1B.6B).

## Features

| Feature | Effect |
| --- | --- |
| `core` (default) | Enables Core conversions, the Scanner Object and temporary smoke scan. |
| `flow` | Enables `core`, `cribra/transform`, `cribra/serde`, the four explicit-finding Flow transforms, and in-memory Share Bundles. |
| `fuse` | Enables `flow`, `cribra/parallel`, and `Scanner.parallelScan(sources:)`. |
| `bindgen` | Enables the local UniFFI generator binary; disabled by default. |
| No default features | Exports the same DTOs and scaffolding, without Core conversions. |

`cribra` is a non-optional dependency with default features disabled. Flow enables
Core serde and an optional `serde_json` runtime dependency only for the explicit
DTO-to-Core report bridge; neither is requested by the default/core-only adapter.
Cargo feature unification can also enable Core features requested by other
workspace members. UniFFI default features are disabled. The opt-in `bindgen`
feature enables its CLI for the local `cribra-uniffi-bindgen` binary; it is not
needed by the runtime library. No async runtime or FFI tracing is enabled.

Redact, Template, Pseudonymize, Synthesize and Share Bundles are exposed from Flow. No logging,
networking or persistence is introduced.
Detection, validation, and transformations remain
owned by Core. Finding remediation and typed explanations are projected from Core.

## P0 metadata parity

With the Core profile alone (no Core Serde or Flow), `Finding.remediation` carries
Core's optional typed action. `Finding.explanation` carries optional
`Explanation::Classified { mode: DetectionMode }` facts. Ordinary scanner, smoke,
streaming, incremental and Fuse reports resolve the original Core finding through
`finding.explanation(&scanner)` before copying it. Core validates rule identity,
severity and remediation; incompatible authority returns `None`. No finding is
reconstructed and no adapter classification or validation rule is used.

Streaming and incremental sessions retain an immutable, source-free Core scanner
clone (shared compiled rules) for this resolution, independent of the foreign
Scanner lifetime. Ordering, UTF-8 coordinates, candidate counts, source privacy,
streaming admission/limits and incremental transaction semantics remain Core-owned.
The legacy Rust `From<&cribra::Finding>` and scanner-free report conversion leave
explanations unresolved; `Finding::from_core` accepts an original Core finding and
scanner for validated resolution. Foreign records are plain editable data, not
proof of scanner authority; no DTO-based finding explanation resolver is exposed.

`SensitiveCandidate` contains kind, location, evidence and an optional explanation
projected directly from Core. A future unsupported Core explanation fails closed
to `None`. Share's existing candidate types and bundle construction are unchanged;
`scan_for_share` uses the same Core-authorized finding projection as ordinary scans.

These record additions change the UniFFI wire contract: regenerate all bindings
and rebuild consumers with the matching native library. Optional finding fields
default to `None` in generated constructors; the added report candidate list defaults
to empty for manually constructed records. Rust struct literals must supply the
new fields. Counts remain Core-derived for scanner-produced reports; consumer-created
records do not validate or reconcile their fields.

## NB-1B.2: reusable Scanner

With `core`, `Scanner` is a UniFFI Object backed by one immutable Core scanner.
The default constructor uses `cribra::Scanner::default()` and returns `Arc<Scanner>`; the
generated Swift class manages native references through UniFFI. No manual
handle registry, explicit close method or retained source text is used.

| Rust API | Swift API | Result |
| --- | --- | --- |
| `Scanner::new()` | `Scanner()` | Reusable default-rule scanner |
| `scan(source: ScanSource)` | `scan(source:)` | `ScanReport` |
| `scan_many(sources: Vec<ScanSource>)` | `scanMany(sources:)` | `ScanResults` |

`ScanSource { id: SourceId { value: String }, text: String }` owns input data.
`ScanReport` returns `source_id`, `source_bytes`, the existing `Finding` DTOs,
`candidate_count`, ordered `candidates` and Core's `needs_review` value. `ScanResults` contains an
ordered `reports` vector and `ScanSummary`. The summary copies all twelve Core
counters, including candidate totals and each severity count. Both methods call
Core directly; neither uses the temporary `smoke_scan_text()` function.

Sources and reports preserve input order. IDs, including empty strings,
duplicates and Unicode sequences, are copied verbatim without normalization or
deduplication. Findings preserve Core order and coordinates. Empty sources still
produce reports; an empty batch has no reports and twelve zero summary counters.
Calls do not accumulate state. Reports remain valid after the Scanner is released.

This is an owned projection of Core, not its entire Rust API: source keys are
UTF-8 strings rather than generic Rust keys, and counts/coordinates are `u64`
rather than `usize`. Original source text and matched values are never returned.
Candidates expose `SensitiveCandidateKind`, UTF-8 `Location`, `CandidateEvidence`
and typed `Explanation::Ambiguous` facts. They have no finding severity, confidence
or remediation. Candidates are counted and affect `needs_review`, exactly as in Core: a report with no
findings is not necessarily clean. No detection, ordering or counting semantics
are changed. NB-1B.3 adds explicit custom-rule configuration and NB-1B.4A adds
streaming below. NB-1B.4B adds incremental documents, NB-1B.5A adds Redact/Template;
no Fuse-specific APIs are introduced.

The same `just uniffi-swift-smoke` command below also tests this Object. Its
shared synthetic fixture covers duplicate/Unicode/empty IDs, empty and clean
sources, multiple rule IDs and severities, a candidate-only source, and CRLF,
emoji and combining-mark coordinates. A test-only Rust example calls Core
directly to write a JSON oracle under `target/`. Swift compares every returned
field and summary counter against that oracle, then checks repeated calls,
single/batch equivalence, reversed input order, empty batches, ARC release and
results surviving Scanner destruction. JSON is test tooling only; it adds no
serialization or persistence API to the adapter.

## NB-1B.3: configuration and active rule metadata

`Scanner::configured(config)` / `try Scanner.configured(config:)` is an
additional fallible UniFFI constructor. `Scanner()` retains its existing default
built-in behavior. Configuration is an owned enum so unused custom rules cannot
silently be ignored by a separate mode flag:

| `ScannerConfig` | Rule set |
| --- | --- |
| `BuiltInOnly` / `.builtInOnly` | Complete Core `builtins::CURRENT` catalog |
| `CustomOnly { rules }` / `.customOnly(rules:)` | Exactly the supplied rules, including zero rules |
| `BuiltInAndCustom { rules }` / `.builtInAndCustom(rules:)` | Supplied custom rules plus the complete built-in catalog |
| `Catalogs { current, financial, personal, rules }` / `.catalogs(current:financial:personal:rules:)` | Independently selected Core default, financial and personal catalogs plus custom rules |

`Catalogs` is available in Core, Flow and Fuse. Each boolean independently adds
the corresponding Core catalog; all false with no custom rules creates an empty
scanner. Financial and personal catalogs remain opt-in and are not added to
`Scanner()`, `BuiltInOnly`, `CustomOnly` or `BuiltInAndCustom`. Catalogs are
submitted to Core in current/financial/personal order. Core places caller-order
custom rules first, preserves each catalog's rule order, and owns all composition,
duplicate-ID detection and validation. The adapter does not merge, deduplicate,
sort or maintain a separate rule inventory.

```swift
let scanner = try cribra_uniffi.Scanner.configured(config: .catalogs(
    current: false, financial: true, personal: true, rules: []
))
```

The three legacy variants retain their fields and order; `Catalogs` is appended.
Regenerate bindings together with their matching native library, as usual for
UniFFI contract changes. Exhaustive consumer switches over `ScannerConfig` must
handle the new case. `ScannerBuildError.Rejected` and its structured fields and
Core diagnostics are unchanged, including collisions with either opt-in catalog.

`RuleSpec` is an owned FFI record (`id: RuleId`, `kind: RuleKind`, `value: String`,
`severity: Severity`, `remediation: Remediation?`). It is not Core's static
`RuleSpec`: it calls public owned `Rule` constructors without leaking strings.
`RuleKind` covers Literal, Prefix, Suffix and Pattern. The optional remediation
enum maps to all seven current public Core actions via `Rule::with_remediation`.

Literal matching imposes no token boundaries. Prefix and suffix use Core's
token boundaries and token extension, including its suffix-length bound.
Patterns use Core's regex dialect and full, non-overlapping, non-empty match
spans; named groups do not change the returned span. All regex parsing,
compilation, zero-length detection, ID/matcher/duplicate validation and detection
remain in Core. No text is trimmed or normalized: e.g. a whitespace-only rule
ID is accepted if Core accepts it. Custom rules remain `MatcherOnly`; an ID
resembling a built-in does not attach its validator. Candidate detection also
remains Core-owned, even for a zero-rule custom scanner.

```swift
let rule = RuleSpec(
    id: RuleId(value: "app.secret"), kind: .literal,
    value: "SYNTHETIC_VALUE", severity: .high, remediation: nil
)
let scanner = try cribra_uniffi.Scanner.configured(config: .customOnly(rules: [rule]))
let report = scanner.scan(source: ScanSource(
    id: SourceId(value: "editor/文書"), text: "SYNTHETIC_VALUE"
))
let rules = scanner.activeRules()
```

`active_rules()` / `activeRules()` returns owned `RuleMetadata` records with ID,
kind, detection mode, severity and optional Core remediation label/message.
Matcher payloads, regexes, validators and scanned source text are not returned.
Metadata is projected from `rule_metadata()` without sorting: caller-order
custom rules precede catalog-order built-ins, as in Core's builder. Findings
continue to follow Core source order independently of metadata order.

Construction throws `ScannerBuildError.Rejected(code:ruleId:detail:)` in Swift.
`BuildErrorCode` is the stable symbolic contract; Core diagnostic strings and
numeric enum discriminants are not machine-readable contracts. The optional
rule ID is included when provided by Core or known at the failing custom rule
constructor. For `EmptyRuleId` or errors without Core identity, it is absent.
Regex diagnostics can include configuration payloads, never scanned source text.

| Code | Core condition / validation exercised |
| --- | --- |
| `InvalidPattern` | Invalid syntax, unsupported lookaround, regex compilation size limit |
| `PatternMatchesEmpty` | Empty regex, nullable expressions and zero-width assertions |
| `EmptyRuleId` | Empty ID |
| `DuplicateRuleId` | Custom duplicates and custom/built-in collisions |
| `EmptyMatcher` | Empty Literal, Prefix and Suffix values |
| `TooManyRules` | Core capacity error mapped; conversion tested, not forced end-to-end |
| `MissingCaptureGroup` | Internal Core rule error mapped; conversion tested, unreachable through public custom patterns |
| `AutomatonBuild` | Core automaton error mapped; resource failure not forced in tests |

The shared configuration fixture has 21 successful cases and 19 failure
cases. It covers all eight catalog selections with/without custom rules, legacy
selection, empty rules, catalog collisions and Core error precedence. Rust and
the real Swift, Python and Kotlin consumers compare outcomes against direct Core
construction, including error precedence and exact current diagnostic detail.
Successful cases compare scan results, summary and every exposed metadata field
in order. Rust additionally verifies prefix/suffix boundaries, oversized suffix
tokens and full-match behavior with named regex groups.

Reproduce all paths with the same `cargo +1.97.0 test -p cribra-uniffi` and
`just uniffi-swift-smoke` commands in the macOS instructions below. Generated bindings and the expanded
Core oracle remain under ignored `target/`. This configuration API offers
complete catalogs and custom rules; selection of individual built-ins,
internal validators/capture projection and custom confidence settings are not
exposed. It adds no dependency or parallel validation implementation.

P0.6A local qualification runs these configuration cases through freshly
generated Core/Flow/Fuse Swift, Python and Kotlin bindings on macOS ARM64.
Swift's checked-in API contract pins the appended case and preserves existing
members. Python's existing consumer runner includes the expanded fixture:
`python3.13 bindings/python/smoke.py core` (also `flow` and `fuse`). The standalone
Kotlin consumer is `tests/kotlin/ConfigurationSmoke.kt`; compile it alongside the
profile's generated binding with the existing JNA/Gson test dependencies, then
run `ConfigurationSmokeKt <configurations.json> <scanner-oracle.json> <profile>`.
These are local source qualifications, not new package/platform release evidence.

## NB-1B.4A: byte streaming with terminal completion

The Scanner has two additional fallible methods:

- `stream(source_id)` / `stream(sourceId:)`: Core's default unlimited optional
  resource budgets.
- `stream_with_limits(source_id, limits)` / `streamWithLimits(sourceId:limits:)`:
  the same Core session with explicit `StreamLimits`.

Both return a UniFFI `StreamSession` Object. Core alone admits the ruleset;
unsupported configurations return `UnsupportedRuleSet` without creating an Object
or falling back to whole-source scanning. The complete built-in catalog is
currently unsupported by Core streaming. Use an admitted custom ruleset.
Bounded custom patterns can be supported; unbounded patterns such as `a+` are
rejected. Successful whole-source construction does not imply stream admission.

The session exposes `push(chunk: Vec<u8>) -> Result<(), StreamError>` (`Data` in
Swift), `finish() -> Result<ScanReport, StreamError>`, and `state() -> StreamState`.
The three states are Active, Finished and Failed. A mutex serializes operations;
callers are responsible for issuing chunks in source order. The session holds
its own Core execution and can outlive the Swift Scanner reference.

```swift
// scanner is the configured literal-rule Scanner from the previous example.
let session = try scanner.stream(sourceId: SourceId(value: "editor/文書"))
try session.push(chunk: Data([0xc3])) // first byte of é
try session.push(chunk: Data([0xa9])) // completes the scalar in Core
try session.push(chunk: Data("SYNTHETIC_VALUE".utf8))
let report = try session.finish()
precondition(session.state() == .finished)
```

Only a successful genuine EOF returns a complete existing `ScanReport` DTO.
The adapter uses `Option<CoreStreamSession>::take()` under the mutex to honor
Core's consuming `finish(self)`. Repeated finish and push after finish return
`NotActive` and leave the state Finished. Any Core push/EOF error discards the
session and working output, sets Failed, and returns no report; later operations
return `NotActive` and leave Failed. Dropping an Active Object discards it without
implicitly finishing. There is no partial-result or retained-source getter.

Chunk bytes are passed directly to Core without adapter UTF-8 decoding. Empty
chunks are no-ops while Active. UTF-8 scalars can span arbitrary chunks;
malformed sequences fail terminally, and incomplete scalars fail at EOF.
Report identity is preserved, byte length counts accepted input bytes, and
finding/candidate ordering and coordinates are projected from the final Core
report. Existing Scanner, configuration and result DTO behavior is unchanged.

`StreamLimits` has three optional `u64` fields (`nil` means unlimited):

| Field | Core meaning |
| --- | --- |
| `maxCandidates` | Cumulative raw rule candidates, including validator rejections |
| `maxStreamStorageBytes` | Checked static logical executor storage at admission |
| `maxRetainedResults` | Findings plus structural review records before normalization/suppression |

Zero is passed unchanged. Storage excludes allocator overhead/spare capacity,
compiled automata, regex/validator workspace, output records and FFI chunk
allocation; this is not a total heap budget. Result slots are not refunded when
final normalization or suppression reduces the report. The adapter neither
reinterprets these budgets nor retries a failed operation.

`StreamError.Rejected(code:limit:required:detail:)` exposes stable symbolic codes
and the original Core diagnostic. Resource errors preserve `limit` and, for
storage admission, `required`. Errors contain no scanned source text.

| Code | Verification |
| --- | --- |
| `UnsupportedRuleSet` | Default built-ins and unbounded custom pattern, Rust and Swift |
| `NotActive` | Repeated EOF, push after completion/failure, Rust and Swift |
| `InvalidUtf8` | Malformed UTF-8, Rust and Swift; additional overlong/surrogate/invalid-continuation cases in Rust |
| `IncompleteUtf8` | EOF inside scalar, Rust and Swift |
| `CandidateLimitExceeded` | During push and at EOF, Rust and Swift |
| `ResultLimitExceeded` | During push and candidate-only EOF, Rust and Swift; pre-suppression accounting in Rust |
| `StorageLimitExceeded` | Exact required/limit values, denied and exact-budget admission, Rust and Swift |
| `StorageRequirementOverflow`, `SourceOffsetOverflow`, `CoordinateOverflow`, `ExecutionInvariantViolated` | Core error conversion tests only; extreme/internal failures not forced |
| `SessionPoisoned` | Adapter fails closed after an earlier panic; injected mutex-poison test in Rust |
| `LimitOutOfRange` | Rejects a u64 budget not representable by target usize; not triggered on macOS ARM64 |
| `UnknownCoreError` | Fails closed for a future unknown Core error; unreachable with current Core variants |

The same `cargo +1.97.0 test -p cribra-uniffi` and `just uniffi-swift-smoke`
commands exercise this path. Rust checks whole-source/Core-stream/UniFFI parity
at every byte split and several chunk sizes, including CRLF, emoji, combining
marks and candidate-only output. Swift passes real `Data` chunks at every byte
split and compares results with the already Core-qualified whole-source API;
its error fields are compared against a direct Core oracle. Rust also tests
concurrent finish calls, where exactly one caller receives the report.

NB-1B.4B adds incremental documents and NB-1B.5A adds Redact/Template separately below.
Candidate details remain outside the
existing result DTOs; their counts and review status are preserved. The runtime
qualification here is native macOS ARM64 with Swift 6.4, not a distribution or
cross-platform qualification.

## NB-1B.4B: transactional incremental documents

`Scanner.incremental(source:)` and `incrementalWithLimits(source:limits:)` create
an owned UniFFI `IncrementalSession`. The existing `ScanSource` supplies the
unchanged source ID and initial text. Core owns the editable UTF-8 document;
the adapter does not expose its source getter or retain another source copy.
No existing Scanner/configuration/report/streaming API is replaced.

| Swift API | Result |
| --- | --- |
| `try session.revision()` | Last committed Core revision, initially 0 |
| `try session.report()` | Owned existing `ScanReport` DTO, without source text |
| `try session.snapshot()` | NB-1B.6A atomic `IncrementalSnapshot { revision, report }` |
| `try session.applyEdit(expectedRevision:start:end:replacement:)` | Newly committed `UInt64` revision |
| `session.state()` | `.active` or `.failed` |

Start and end are **UTF-8 byte offsets** in the zero-based half-open range
`[start, end)`, never Swift character, grapheme or UTF-16 indices. The replacement
is a valid UTF-8 string lowered by UniFFI. Core alone validates the expected
revision, range and scalar boundaries, and performs the transaction. Inserts,
deletes, replacements, empty edits and unchanged replacements all retain their
Core semantics: every successful edit increments the revision once, including
no-ops. The adapter never adds an unchanged-edit shortcut.

```swift
let document = try scanner.incremental(source: ScanSource(
    id: SourceId(value: "editor/文書"), text: "éSYNTHETIC_VALUE"
))
let revision = try document.revision()
let next = try document.applyEdit(
    expectedRevision: revision, start: 0, end: 2, replacement: "🙂"
)
let report = try document.report()
```

After an edit error, the adapter reads Core's `is_active()` to populate the
error's `terminal` flag and the observable state. It does not maintain a second
error-code classification. Recoverable errors leave revision/report unchanged
and permit subsequent valid edits. Terminal Core errors retain the previous
complete snapshot, expose Failed, and make subsequent edits return NotActive;
`revision()` and `report()` remain available. A failed construction creates no
Object and is reported with `terminal: true`.

The Object serializes operations with one mutex around its Core session. There
are no foreign callbacks, nested session locks or foreign calls under that lock.
The report is cloned under the lock, then projected into DTOs after releasing
it; UniFFI lowering also occurs after the Rust method releases the lock. Each
getter/edit is individually synchronized. Separate `revision()` and `report()`
calls can observe intervening edits by another caller; no atomic pair of separate
reads is promised. NB-1B.6A adds `snapshot()` for a coherent pair. `expectedRevision` provides Core's optimistic edit conflict check.
The session can outlive its foreign Scanner wrapper, and returned DTOs can
outlive the session.

`IncrementalLimits` contains four optional `u64` budgets (None/nil unlimited,
zero passed unchanged): `maxCandidates`, `maxRetainedCacheRecords`,
`maxMaterializedResults` and `maxTransactionStorageBytes`. These map directly
to Core's per-operation limits during construction and edits. Cache and storage
count simultaneously owned old/staged data; logical storage is not a total heap
budget. FFI input allocations, caller/report clones, allocator spare capacity,
shared compiled configuration and opaque regex/validator workspace remain outside
Core's accounting contract. No budget is reinterpreted or retried.

`IncrementalError.Rejected` contains `code`, `terminal`, optional
`expectedRevision`/`actualRevision`, optional `limit`/`required`, and diagnostic
`detail`. Codes are stable symbolic values; Core diagnostic text and numeric
enum discriminants are not parsing contracts. Errors contain no scanned source.

| Codes | Verification |
| --- | --- |
| `StaleRevision`, `InvalidRange`, `InvalidUtf8Boundary` | Real Rust/Swift errors, unchanged snapshot and successful later edits |
| `CandidateLimitExceeded`, `CacheLimitExceeded`, `ResultLimitExceeded`, `TransactionStorageLimitExceeded` | Real Rust/Swift construction and edit failures, exact Core fields, Failed and last snapshot preservation |
| `NotActive` | Real later edits after terminal errors; precedence over stale revision/invalid range |
| `RevisionOverflow`, `DocumentLengthOverflow`, `ResourceAccountingOverflow`, `ExecutionInvariantViolated` | Conversion tests only; extreme/internal Core failures not forced |
| `SessionPoisoned` | Injected mutex-poison test in Rust; panic recovery is outside Core's contract |
| `LimitOutOfRange`, `OffsetOutOfRange` | Adapter representation failures for targets whose usize cannot represent a u64 input; unreachable on qualified ARM64 |
| `UnknownCoreError` | Future non-exhaustive Core error mapping, not exercised with current Core |

Revision/document-length overflow are preflight errors in the current Core;
their code alone must not be interpreted as terminal. The runtime flag/state
always comes from Core activity. A poisoned mutex reports Failed and blocks all
access with SessionPoisoned, including the otherwise readable getters: preserving
snapshots after a panic is not claimed. Normal returned Core errors do not poison
the lock. On targets narrower than 64 bits, an unrepresentable edit endpoint has
an explicit adapter OffsetOutOfRange error instead of Core range validation;
these targets have not been qualified.

Run `cargo +1.97.0 test -p cribra-uniffi` and `just uniffi-swift-smoke` as below.
The shared incremental fixture covers Unicode/CRLF, insert/delete/replace,
two kinds of no-op, stale and invalid ranges/boundaries, and recovery. Its
direct Core oracle records revisions, full report projections, state and error
fields after every operation; valid reports also match whole-source scanning.
Both Rust and Swift test competing edits against one revision. Swift uses actual
concurrent calls and weak references to check ARC ownership/release.

There is no adapter fallback to whole-source scanning. Core's existing internal
incremental replay strategy for non-literal matchers remains unchanged. Flow
Redact/Template is added separately below; no Fuse-specific API or packaging is introduced.
Qualification is native macOS ARM64, Rust 1.97,
UniFFI 0.32.2 and Swift 6.4; the expanded oracle and generated outputs remain
ignored under `target/`.

## NB-1B.5A: explicit-finding Flow Redact and Template

All functions, options, results and error DTOs in this section are compiled and
exported **only** with `flow` (also enabled by `fuse`). They are absent from the
default/core-only and no-default-features interfaces. Existing APIs are unchanged.

| Rust / Swift function | Inputs | Result |
| --- | --- | --- |
| `redact` | `source: String`, `findings: Vec<Finding>` / `[Finding]`, `options: RedactOptions` | Owned `TransformResult { content }` |
| `template` | Same explicit source/findings, `options: TemplateOptions` | Owned `TransformResult { content }` |

`RedactOptions.replacement` is optional: None/nil selects Core's `[REDACTED]`,
while Some is passed verbatim, including an empty string. `TemplateOptions`
contains optional `namespace` and `numbered: bool`; namespace defaults,
normalization, fallback and per-rule numbering belong entirely to Core.

```swift
let redacted = try redact(
    source: text, findings: report.findings,
    options: RedactOptions(replacement: nil)
)
let templated = try template(
    source: text, findings: report.findings,
    options: TemplateOptions(namespace: "APP", numbered: true)
)
```

There is no Scanner call, hidden rescan, membership check against active rules,
finding normalization or source retention in these functions. The caller supplies
the source corresponding to the explicit findings. Both results preserve text
outside those ranges; with no findings, the output is an owned unchanged source,
even if a scanner would detect secrets in it. Returned content alone therefore
does not establish that it is secret-free.

Core's public transform APIs accept `ScanReport`, whose finding/report
constructors are private. The adapter transfers the existing finding DTOs in
unchanged input order through Core's **public serde deserialization model**,
with an empty candidate list, then calls only `redact`/`redact_with` or
`template_with`. This in-memory metadata bridge is why Flow forwards
`cribra/serde` and uses optional `serde_json`; it performs no I/O and serializes
no source text. Coordinates, rule IDs, severity and confidence are transferred;
remediation and explanation fields are not used by these transforms.
Core remains the sole span validator and transformation implementation.

Ranges are zero-based, half-open UTF-8 byte offsets. Input order is preserved for
Core's validation/error precedence. Core then sorts spans deterministically by
start/end/rule ID for output. Redact merges overlapping and duplicate ranges,
leaving adjacent spans separate. Template rejects overlaps/duplicates, keeps
adjacent spans independent, and numbers per raw rule ID in sorted source order.
The adapter does not reproduce sorting, merging, placeholder normalization or
replacement logic. Line/column metadata is carried unchanged; it does not replace
Core's byte-range validation.

Errors are `TransformError.Rejected(code:span:otherSpan:sourceLen:detail:)`.
`ByteSpan` holds only `start` and `end` u64 byte offsets. Codes are stable symbols;
diagnostic strings and numeric discriminants are not machine-readable contracts:

- `InvalidSpan`: empty, reversed or out-of-source range; carries span/sourceLen.
- `InvalidUtf8Boundary`: endpoint inside a scalar; carries span.
- `OverlappingSpans`: Template-only; carries both Core-sorted overlapping spans.
- `InputConversion`: Core serde could not represent finding metadata (e.g. u64
  offsets outside usize on narrower targets); not exercised on qualified ARM64.
- `UnknownCoreError`: Core error outside the exposed transform contract; not forced.

Errors return no partial transformed output and contain no source text. Core's
existing ability to transform externally supplied spans, including unknown rule
IDs and arbitrary line/column metadata, is preserved. No additional matching or
source/finding correspondence check is invented.

Reproduce on the macOS ARM64 Rust 1.97 / Swift 6.4 setup described below:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features fuse
cargo +1.97.0 check -p cribra-uniffi --no-default-features
just uniffi-swift-smoke               # unchanged core-only API smoke
just uniffi-swift-flow-smoke          # Flow cdylib, bindings, Core oracle, real Swift
just uniffi-swift-flow-smoke fuse     # same transform smoke with fuse gating
```

The separate Flow output directory is ignored `target/uniffi-flow-smoke/`.
The shared fixture checks 23 inputs and 46 direct Core outcomes: one/multiple
findings, Unicode/CRLF/emoji/combining marks, unchanged/empty sources, unknown
IDs, unsorted/adjacent/duplicate/nested/overlapping spans, custom/empty markers,
namespace normalization and numbering, error precedence and maximum u64 offset
transport. Swift compares successful output as UTF-8 bytes (not canonically
equivalent String equality) and all exposed structured error fields. Additional
tests explicitly show no rescanning and results surviving input ownership.

NB-1B.5B below adds Pseudonymize and Synthesize; NB-1B.5C adds Share Bundles.
NB-1B.5D adds fuse-only parallel scanning; packaging remains outside scope.
Core, C API, WASM and CLI are not modified. Validation here does
not qualify other platforms or total heap memory use: FFI inputs and the serde
metadata bridge allocate owned data outside Core's transformation logic.

## NB-1B.5B: explicit-key Pseudonymize and Synthesize

Two additional functions reuse the existing `Finding`, DTO-to-Core report bridge,
owned `TransformResult` and structured `TransformError`:

| Swift / Rust function | Options DTO |
| --- | --- |
| `pseudonymize(source:findings:options:)` | `PseudonymizationOptions { key, prefix, digestBytes / digest_bytes }` |
| `synthesize(source:findings:options:)` | `SynthesisOptions { key, marker }` |

All new functions and DTOs are exported only under `flow` or `fuse`; core-only
builds expose none of them. Both APIs require an explicit **32-byte** key
(`Vec<u8>` in Rust, `Data` in Swift). There is no default key, random generation,
key store, persistence or logging path. Source and findings are explicit inputs;
the adapter never scans or chooses rules internally.

```swift
// callerKey is a caller-owned Data value containing exactly 32 bytes.
let pseudonyms = try pseudonymize(
    source: text, findings: report.findings,
    options: PseudonymizationOptions(key: callerKey, prefix: nil, digestBytes: nil)
)
let synthetic = try synthesize(
    source: text, findings: report.findings,
    options: SynthesisOptions(key: callerKey, marker: nil)
)
```

Pseudonymization options map directly to Core. A nil prefix selects Core's
`cribra_pseudo_`; a supplied prefix is passed verbatim, including empty text.
Nil digest length selects Core's default **16 bytes**. Core's own `digest_bytes`
method clamps any representable request into **8...32 bytes**; zero, one and
values above 32 are therefore accepted, not new adapter validation errors.
The output digest is Core's lowercase hex encoding. Hashing and all digest
logic remain exclusively in Core. Equal matched bytes plus the same key have
the same pseudonym regardless of rule metadata or position; prefix selection
does not affect that digest identity.

Synthesis options map to Core's mandatory key and optional marker. Nil marker
uses `cribra_synthetic`; supplied markers, including empty or unsupported text,
are normalized/fallback-selected by Core. No new marker restriction is imposed.
Core owns provider-aware strategies and the custom-rule strategy. Its output
depends on the key, rule ID and byte geometry rather than the original matched
bytes, and is deterministic for the same inputs/options. No synthetic-generation
or hashing algorithm is implemented in this adapter.

Both transforms delegate byte-range validation, source-order sorting and overlap
rejection to Core. Duplicate or overlapping spans fail; adjacent spans remain
independent. Empty sources with no findings succeed with valid keys; no findings
return the original source unchanged, including text a scanner would detect.
Caller-provided source/option text outside transformed spans retains the same
Core semantics; the adapter does not add an independent scrubber.

The existing `InvalidSpan`, `InvalidUtf8Boundary`, `OverlappingSpans`,
`InputConversion` and `UnknownCoreError` codes are reused. `InvalidKeyLength` is
added to `TransformErrorCode`: it reports only the required and received lengths
in `detail`, with no key bytes or partially transformed output. This shape check
precedes DTO report conversion because Core requires a `[u8; 32]` value. Even an
empty source/report does not bypass the explicit key requirement. On narrower
targets, a digest length not representable by usize returns the existing
`InputConversion` code; these targets are not qualified here.

The key is never added to results, Core error mapping, error diagnostics, or
the in-memory serde report bridge. Rust Debug for the new input DTOs redacts
the key field. Input DTOs necessarily contain caller-owned key bytes; no secure
memory/zeroization guarantee beyond Core/UniFFI's existing ownership is claimed.
There is no added retention after the call beyond caller inputs and owned output.

The existing recipes now test all four Flow transforms:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features fuse --test flow
just uniffi-swift-flow-smoke
just uniffi-swift-flow-smoke fuse
just uniffi-swift-smoke   # also regenerate/check the core-only interface
```

The keyed test matrix has **27 inputs × 9 option sets × 2 transforms = 486
direct Core outcomes** in addition to the Redact/Template fixture. Rust and
Swift check every outcome and repeat calls for determinism. Cases include
Unicode/CRLF/emoji/combining marks, provider and custom IDs, multiple/unsorted
findings, equality across rule IDs, changed keys, digest defaults/bounds,
invalid marker text, invalid ranges/boundaries/overlaps, empty input, no implicit
scan, and unchanged-length source content changes for synthesis.

Separate tests reject key lengths 0, 1, 31, 33, 64 and 1024 and verify diagnostics
contain only lengths. Result/error snapshots are checked for absence of the
public synthetic test-vector key values; oracle output contains only option-set
labels, never keys or option DTO dumps. No runtime key is generated or saved.

Existing Redact/Template, Scanner, streaming and incremental API behavior is
unchanged. Share bundles are covered below; no packaging is exposed. Native qualification is
macOS ARM64, Rust 1.97, UniFFI 0.32.2 and Swift 6.4; conversion/future-unknown
errors and other target architectures remain outside end-to-end qualification.

## NB-1B.5C: in-memory Share Bundles

Share APIs and all their DTOs exist only with `flow` (also enabled by `fuse`):

| Swift API | Contract |
| --- | --- |
| `scanAndBuild(scanner:sources:options:)` | Delegates the complete scan/transform operation to Core `ShareBundleBuilder::scan_and_build`. |
| `Scanner.scanForShare(sources:)` | One explicit Core scan returning owned source-free `ShareReport` records for later builds. |
| `build(reports:sources:options:)` | Delegates precomputed reports and positional text to Core `ShareBundleBuilder::build`, without scanning. |

Rust uses `scan_and_build` and `scan_for_share`. Existing Scanner methods and
Flow functions keep their signatures and behavior. NB-1B.5D below adds the
parallel scanning API. No archive, filesystem write, packaging, network or
persistence is introduced.

`ShareBundleOptions.mode` is an explicit optional `ShareMode`: Redact, Template,
Pseudonymize with existing `PseudonymizationOptions`, or Synthesize with existing
`SynthesisOptions`. None preserves Core's `MissingShareMode` failure. Redact and
Template use the defaults offered by the Core share builder; individual transform
options are not added where the builder does not support them. Keyed modes
require the same caller-owned 32-byte `Data` keys and Core prefix/digest/marker
options as NB-1B.5B. Key material is not added to any output/diagnostic, and input
options' Rust Debug redacts the key. No secure-memory/zeroization guarantee is added.

```swift
let options = ShareBundleOptions(mode: .redact)
let sources = [ScanSource(id: SourceId(value: "documento😀"), text: text)]
let atomic = try scanAndBuild(scanner: scanner, sources: sources, options: options)
let reports = try scanner.scanForShare(sources: sources)
let precomputed = try build(reports: reports, sources: sources.map(\.text), options: options)
```

`ShareReport` owns source ID, original UTF-8 byte length, existing Finding DTOs
and full `ShareCandidate` metadata (kind, location, evidence). The older
`ScanReport` projection only has a candidate count: accepting it would lose
candidate counters or require inventing candidate records. The new DTO avoids
both, without modifying that earlier API. As in the existing Flow bridge, Core's
public serde model transfers metadata into its private-constructor report types.
It never serializes source text or cryptographic keys. Core computes all summary
counts, sorts/validates spans and performs transformations. Candidate observations
remain review-only, never promoted to transformed findings.

`ShareBundle.sources` contains owned `TransformedSource { sourceId, content }`
records in Core input order, preserving Unicode, empty and duplicate IDs. It
retains only transformed content. Text outside findings (or an entire source
without findings) remains unchanged; no secret-free guarantee is added.
`ShareManifest` holds mode kind, existing `ScanSummary`, and `ShareTimestamp`
with Unix seconds and nanoseconds copied from the actual Core manifest.
**The timestamp is not deterministic between calls.** Tests compare manifest
mode/counters/content to an independent Core call and validate each call's time
semantically against its own invocation interval, not against oracle bytes.

`build` preserves Core's positional contract: number of sources and each recorded
UTF-8 byte length are checked by Core, but equal-length changed text is accepted.
Source IDs do not establish source identity. Callers must preserve pairing;
`scanAndBuild` uses identical inputs for both operations by construction.

`ShareError.Rejected` exposes stable `ShareErrorCode` plus optional `expected`,
`actual`, zero-based `index` and Core diagnostic. Codes cover `MissingShareMode`,
`SourceCountMismatch`, `SourceLengthMismatch`, and adapter `InputConversion`.
`ShareError.Transform` wraps the unchanged `TransformError`: invalid spans,
UTF-8 boundaries, overlap, wrong key lengths and other existing codes. Errors
return no partial bundle. Key shape conversion precedes Core construction;
Core owns missing-mode/count/length/span error precedence thereafter. Redact
merges overlaps; Template/Pseudonymize/Synthesize reject them, as in Core.

Reproduce on native macOS ARM64 with Rust 1.97.0, UniFFI =0.32.2 and Swift 6.4:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features fuse --test share
just uniffi-swift-share-smoke flow
just uniffi-swift-share-smoke fuse
just uniffi-swift-smoke
cargo +1.97.0 check -p cribra-uniffi --no-default-features
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features fuse
cargo +1.97.0 check --workspace
cargo fmt --all -- --check
git diff --check
```

The Share recipe builds the cdylib, generates bindings, compiles and executes
both the existing Flow smoke and `tests/swift/ShareSmoke.swift`. Five batches ×
four modes × two paths give 40 Core outcomes, and 23 explicit finding fixtures ×
four modes give another 92. Further Rust/Swift checks exercise missing modes,
source count/length errors, same-length changed content, and key lengths
0/1/31/33/64. Fixtures include empty/single/multiple sources, Unicode/duplicate
IDs, CRLF/emoji/combining marks, ambiguous candidates and built-in findings.
Generation outputs/oracles/executables stay under ignored `target/`.

Core-only bindings must contain none of `ShareBundle`, `ShareReport`,
`ShareManifest`, `ShareMode`, `ShareCandidate`, `ShareError`, `TransformedSource`,
`scanForShare`, `scanAndBuild`, or `build`. Core, C API, WASM and CLI remain
unchanged. Native ARM64 is the qualified target; narrower-target integer
conversion, pre-Unix-epoch host clocks, and future Core candidate/mode variants
return representation errors and are outside runtime qualification.

## NB-1B.5D: fuse-only parallel scanning

`Scanner::parallel_scan(Vec<ScanSource>) -> ScanResults`, exported as
`Scanner.parallelScan(sources:) -> ScanResults` in Swift, is available **only**
with `fuse`. It calls `cribra::Scanner::parallel_scan` directly. The adapter
adds no scheduling, pool, Swift concurrency, source chunking or sequential
fallback. Core uses the current Rayon pool to scan independent sources through
its existing pipeline; the UniFFI call is synchronous and blocks its caller.
No performance improvement or particular worker count is promised.

```swift
// Generate/build with --no-default-features --features fuse.
let scanner = Scanner()
let parallel = scanner.parallelScan(sources: sources)
let sequential = scanner.scanMany(sources: sources)
```

The existing owned `ScanSource`, `ScanResults`, `ScanReport` and `ScanSummary`
DTOs include the P0 metadata additions above. Source/report association is positional; input
order and exact UTF-8 source-ID bytes are preserved, including Unicode, empty
and duplicate IDs. Findings preserve Core order, rule IDs, severity, confidence,
UTF-8 half-open byte ranges and one-based scalar line/column coordinates.
Candidate-only reports retain `candidateCount` and `needsReview`, alongside the
ordered review-only candidate metadata. All twelve summary counters come from Core,
including candidates and per-severity counts. Outputs retain no source text
and remain valid after releasing the Scanner. Existing Scanner and Flow APIs
keep their signatures and behavior.

Native macOS ARM64 qualification, using Rust 1.97.0, UniFFI =0.32.2 and Swift 6.4:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features fuse
just uniffi-swift-parallel-smoke
just uniffi-swift-parallel-gating
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features core
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 check -p cribra-uniffi --no-default-features --features fuse
cargo +1.97.0 check -p cribra-uniffi --no-default-features
cargo +1.97.0 check --workspace
cargo fmt --all -- --check
git diff --check
```

The smoke builds the fuse cdylib, generates actual bindings, compiles and
executes Swift. A direct public-Core oracle compares Core parallel/sequential
results independently of adapter calls/conversions. Rust and Swift cover
**44 scenarios × 8 repeats**: four rule configurations (built-in, custom,
mixed, empty) and eleven batches (empty, seven single-source inputs, multiple,
reversed, and 64-source inputs). Fixtures include Unicode/duplicate/empty IDs,
canonically equivalent but byte-distinct IDs, CRLF/emoji/combining marks, clean
and empty sources, candidate-only reports, and nonzero counts at every severity.
Swift compares exact UTF-8 snapshot bytes for all exposed fields/counters,
checks ARC ownership and compares both scanning methods against the Core oracle.
No Swift task, dispatch queue or test-controlled thread pool is introduced.

`uniffi-swift-parallel-gating` regenerates default/core and flow bindings and
fails if either contains `parallelScan` or its Rust symbol name. The method is
gated at the adapter feature boundary even when another workspace crate enables
Core's `parallel` feature through Cargo feature unification. Generated files,
oracles and executables stay in ignored `target/`; no distribution packaging or
other platforms are qualified. Core, C API, WASM and CLI are unchanged.

## NB-1B.6A: incremental snapshot and native contract consolidation

`IncrementalSession.snapshot() throws -> IncrementalSnapshot` is additive to the
Core profile (also present in Flow/Fuse). The owned record contains `revision`
and the existing `ScanReport`. One acquisition of the existing session mutex
captures Core revision, source byte length and a clone of its report together.
DTO projection and UniFFI lowering occur after releasing that mutex; no foreign
call, callback, nested lock or source-text copy is introduced. Edits can proceed
while the captured immutable report is projected, without changing its revision.

```swift
let snapshot = try document.snapshot()
let revision = snapshot.revision
let reportAtRevision = snapshot.report
```

`revision()` and `report()` retain their signatures, behavior and errors.
Their separate calls still permit intervening edits. Snapshot reads include
successful no-op revisions, remain available with the last committed data after
terminal Core errors, and fail with the existing `SessionPoisoned` diagnostic
when the adapter mutex is poisoned. State remains a separate observation, not
an additional atomic field. Snapshot DTOs outlive the session; they do not retain
a foreign/native Object reference.

The Flow/Share serde bridge now **moves already-built Value subtrees** into
report, candidate and batch containers. In pinned serde_json, `json!(value)`
serializes through `to_value(&value)`, recursively copying a supplied Value
array/object. These container-level copies are removed. The same Core serde
deserializers, constructors, field values, order and error mapping still run.
Share deliberately retains its initial Core finding-report deserialization and
subsequent serialization: removing that boundary could change which public
conversion error/detail is returned, especially on narrower targets. There is
no manual Core deserializer, new validation algorithm, dependency, or scan.
No throughput or total-heap improvement is claimed; this is a structural copy
reduction, qualified by the unchanged direct-Core outcome/error matrix.

The public error audit keeps all existing variants and associated fields:

| Error family | Stable machine-readable context | Diagnostic contract |
| --- | --- | --- |
| Scanner construction | `BuildErrorCode`, optional rule ID | Core rule/regex diagnostic; can contain caller configuration, not scanned text. |
| Streaming | `StreamErrorCode`, optional limit/required bytes | Core or existing adapter lifecycle/representation diagnostic; failures remain terminal. |
| Incremental | `IncrementalErrorCode`, terminal flag, revisions, resource fields | Lifecycle comes from Core after failure; snapshot adds no new error code. |
| Flow | `TransformErrorCode`, spans, optional source length | Core transform diagnostic, or existing DTO/key representation diagnostic. |
| Share | `ShareErrorCode`, expected/actual/index, or nested `TransformError` | Core count/length/mode diagnostic; same-length changed text remains accepted. |

Code names and structured fields are the programmatic contract. Numeric enum
discriminants and human-readable diagnostic text are not promised stable across
Core updates. Unknown-Core paths and target-width representation failures retain
the existing codes; no new recoverable/terminal classification is invented.
Keys are not inserted into diagnostics/results; existing key Debug redaction
and memory/zeroization limitations remain unchanged.

`tests/fixtures/api-contract.json` is a reviewable expected adapter surface,
not generated binding code. `tests/swift/APIContract.swift` checks actual
UniFFI-generated bindings against it: exact type, function, method and
constructor sets; ordered DTO field shapes; enum cases; and structured error
payload shapes. Generator support helpers/converters are excluded. Tests do not
regenerate or silently accept the expected fixture.

The profiles are cumulative and preserve the existing always-exported DTOs:

| Profile | Exposed adapter capabilities |
| --- | --- |
| No default features | Shared base DTOs and `cribraVersion` only. |
| `core` | Base plus Scanner/configuration, streaming, incremental and atomic snapshot. |
| `flow` | Core plus four transforms and in-memory Share Bundles. |
| `fuse` | Flow plus `Scanner.parallelScan`. |

Regress on native macOS ARM64, Rust 1.97.0, UniFFI =0.32.2 and Swift 6.4:

```sh
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features core
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features flow
cargo +1.97.0 test -p cribra-uniffi --no-default-features --features fuse
just uniffi-swift-smoke core
just uniffi-swift-smoke flow
just uniffi-swift-smoke fuse
just uniffi-swift-share-smoke flow
just uniffi-swift-share-smoke fuse
just uniffi-swift-parallel-smoke
just uniffi-swift-api-contract
cargo +1.97.0 check --workspace
cargo fmt --all -- --check
git diff --check
```

The core smoke recipe now accepts an optional profile, defaulting to `core`;
existing no-argument commands retain their behavior. Both Rust and real Swift
race four snapshot readers with 128 edits alternating insertion/no-op, checking
revision-derived byte lengths, full finding geometry, Unicode/CRLF, legacy
getters, terminal snapshots and owned results. Existing Flow/Share fixtures
still compare content and structured errors to independent public-Core calls;
manifest timestamps remain semantic, not byte-identical across calls.

No Apple packaging, ABI/distribution compatibility guarantee, other architecture
qualification, logging, filesystem/network API or persistence is added. New
bindings must be generated for the additive snapshot API; the profile/shape
inventory is not a promise that old compiled bindings link against changed
libraries. Core, C API, WASM and CLI remain unchanged.

## NB-1B.1: Swift smoke path on macOS ARM64

Two functions are exported using UniFFI proc macros:

- `cribra_version() -> String` (`cribraVersion()` in Swift) returns the Core
  package version. The adapter build script reads `../../Cargo.toml` and embeds
  `package.version`; it does not use the adapter's package version or change Core.
- `smoke_scan_text(source: String) -> Vec<Finding>`
  (`smokeScanText(source:)` in Swift) requires `core`. It constructs
  `cribra::Scanner::default()`, scans exactly one source and projects classified
  findings through the existing DTO conversions. This is an experimental smoke
  entry point, not a definitive Scanner API. It returns neither candidates nor
  matched source text.

Prerequisites: native macOS ARM64, Rust toolchain `1.97.0`, Apple Swift **6.4**
and its macOS SDK (select the corresponding Xcode toolchain), and `just`.
The first Cargo run needs access to dependencies unless already cached.
From the repository root, verify the selected tools and run:

```sh
uname -sm                         # Darwin arm64
rustc +1.97.0 --version
swiftc --version                  # Apple Swift version 6.4, arm64 target
cargo +1.97.0 test -p cribra-uniffi
just uniffi-swift-smoke
cargo +1.97.0 check --workspace
cargo fmt --all -- --check
```

`just uniffi-swift-bindings` can also be run separately. It builds the native
`cdylib` with Rust 1.97 and runs the UniFFI **0.32.2** generator against the
actual library. The smoke recipe then compiles the generated Swift source into
a module/library, compiles `tests/swift/Smoke.swift` with `-swift-version 6`,
and executes it against the Rust library. It checks the Core version against
`cargo pkgid -p cribra`, empty and clean input, a synthetic built-in GitHub PAT
detection, RuleId, severity, confidence, and half-open UTF-8 offsets. CRLF,
emoji and combining scalars distinguish byte offsets and scalar columns from
UTF-16 or grapheme indexing. Failed checks terminate the executable.

Outputs are confined to ignored `target/debug` and `target/uniffi-swift-smoke`:
generated Swift/header/modulemap, Swift module/library, executable and module
cache. Recipes explicitly use the repository's `target/` directory; do not
version these outputs. Relative executable/library rpaths avoid requiring
`DYLD_LIBRARY_PATH`. No Swift package, framework, XCFramework, distribution or
other language binding is provided or qualified here.

The generated-file and library workflow follows the
[UniFFI Swift documentation](https://mozilla.github.io/uniffi-rs/latest/swift/overview.html).
