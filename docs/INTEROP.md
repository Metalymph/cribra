# Cribra Interoperability Contract

Status: implemented interoperability contract
Scope: Rust core, native C ABI, C/C++ consumers, and WebAssembly consumers

## 1. Purpose

Cribra exposes multiple integration surfaces while preserving one semantic
authority.

The Rust `cribra` crate owns detection, validation, candidate promotion,
explainability, remediation, ordering, and transformations. Native and
WebAssembly adapters project those semantics into foreign runtimes; they do not
reimplement them.

```text
Cribra core (`cribra`)
    ├── Rust API
    │
    ├── native adapter (`cribra-capi`)
    │     └── C ABI
    │           ├── C
    │           ├── C++
    │           └── other FFI-capable runtimes
    │
    └── WebAssembly adapter (`cribra-wasm`)
          ├── browser
          ├── Node.js
          ├── Bun
          └── Deno
```

The C ABI and WebAssembly adapter are independent projections over the same
core. WebAssembly does not route through the C ABI.

This document defines the interoperability contract. The generated
`include/cribra.h` header is the normative native API surface for exact
functions, types, constants, fields, and ownership requirements.

## 2. Core semantic authority

Adapters MUST preserve Cribra core semantics.

Adapters MUST NOT independently:

- classify source spans;
- implement provider or structural validation;
- promote an ambiguous candidate into a finding;
- infer severity or confidence;
- derive remediation independently from core metadata;
- change overlap or collision semantics;
- create an independent explainability authority;
- expose matched sensitive values as metadata;
- change logical result ordering because of execution strategy.

When adapter ergonomics conflict with core semantics, core semantics win.

The core remains responsible for:

- compiled scanner behavior;
- built-in and custom rule semantics;
- finding classification;
- sensitive-candidate classification;
- deterministic report ordering;
- byte spans and Unicode coordinates;
- finding and candidate explanations;
- transformations.

The core crate retains its `forbid(unsafe_code)` boundary. Native FFI-specific
`unsafe` belongs in `cribra-capi`.

## 3. Adapter architecture

The dependency direction is:

```text
cribra-capi ─────▶ cribra
cribra-wasm ─────▶ cribra

cribra-capi  ✕  cribra-wasm
```

Neither adapter depends on the other.

This prevents the C ABI from becoming an accidental semantic layer and avoids
requiring browser/Wasm consumers to pay for native-FFI abstractions they do not
use.

Host applications remain responsible for UI, filesystem policy, persistence,
networking, process lifecycle, and other runtime-specific behavior.

## 4. Privacy boundary

Cribra is local-first.

Across Rust, native, and WebAssembly integrations:

- source material is caller-owned;
- scans do not require network access;
- adapters do not upload source material;
- adapters do not persist source material;
- findings contain metadata and coordinates rather than matched source values;
- candidates contain review metadata rather than matched source values;
- transformations require the caller to provide the original source;
- transformed output is returned only to the caller.

A host application may choose to store, transmit, log, or otherwise process
source material. Such behavior belongs to that application and is outside the
Cribra core interoperability contract.

## 5. Native C ABI

`cribra-capi` exposes the native interoperability protocol.

The ABI uses:

- opaque Rust-owned handles for owned state;
- fixed ABI representations for enums and scalar protocol values;
- pointer-plus-length byte/string views;
- explicit UTF-8 validation;
- explicit status values;
- optional Rust-owned diagnostic errors;
- explicit destruction functions;
- generated public declarations in `include/cribra.h`.

Rust object layouts are never part of the ABI.

The C ABI is usable directly from C and as the foundation for C++, Swift,
JNI/Kotlin, Python, Go, .NET, or other FFI-capable environments.

Language-specific wrappers may provide host-language ergonomics, but they must
not become independent detection authorities.

## 6. Native ownership and lifetime model

Native ownership is explicit.

Rust-owned objects include scanner, stream, incremental-document, report, batch-result,
transformed-output,
and error handles where applicable. Every owned allocation has one documented
destruction path.

Passing null to destruction functions documented as null-safe is a no-op.

Borrowed views do not own their data.

Typical relationships are:

```text
CribraScanner
    │
    └── scan caller-owned source
            │
            ▼
       CribraReport
            │
            ├── borrowed finding views
            ├── borrowed candidate views
            └── explanation projections

caller-owned source + report
            │
            ▼
       CribraOutput
            │
            └── borrowed UTF-8 output view
```

A borrowed view becomes invalid when its owner is destroyed.

Caller-provided source memory is borrowed for the duration documented by the
operation and is not retained merely to support later report traversal.

Rust-owned memory MUST be released through Cribra's destruction functions,
never through the caller allocator.

### Error-output slots

An optional `out_error` slot is initialized to null before each operation
that supports diagnostic errors.

The caller MUST release any previously returned `CribraError` using
`cribra_error_free` before reusing the same output slot. Clearing the slot
does not release an existing error handle.

An error handle is owned by the caller until explicitly released.
Borrowed diagnostic message views remain valid only while their error
handle remains alive.

### Builder consumption

`cribra_builder_build` consumes a non-null builder after validating
`out_scanner`, regardless of whether scanner construction succeeds.

If `out_scanner` is null, the call returns `CRIBRA_INVALID_ARGUMENT` without
consuming the builder.

Once the builder has been consumed, the caller MUST NOT reuse or free it
independently.

This consuming ownership rule is part of the public ABI contract.

### Streaming lifecycle and budgets (ABI 0.2.0)

`cribra_scanner_stream_new` opens an unlimited Core stream;
`cribra_scanner_stream_new_with_limits` copies a `CribraStreamLimits` configuration.
Complete built-in portfolios and unsupported custom patterns fail admission with
`CRIBRA_STREAM_UNSUPPORTED_RULE_SET`. There is no whole-source fallback or adapter
source buffer. Streaming eligibility is exactly Core's; see
[streaming](streaming.md) and its [compatibility inventory](streaming-compatibility.md).

`cribra_stream_push` accepts caller-owned bytes in source order, including partial
UTF-8 scalars across chunks. No caller borrow survives the call. Null plus zero
length is an empty chunk; embedded NUL is a source byte. Definite malformed UTF-8
fails during push with `CRIBRA_STREAM_INVALID_UTF8`; EOF inside a scalar fails with
`CRIBRA_STREAM_INCOMPLETE_UTF8`. These are distinct from the existing whole-source
`CRIBRA_INVALID_UTF8` status, whose assignment and meaning remain unchanged.

`cribra_stream_finish` signals genuine EOF and consumes the Core session, returning
an ordinary independently owned `CribraReport` only on success. Existing report,
explanation and transformation APIs apply unchanged, including UTF-8 byte offsets,
Core finding/candidate order, review-only semantics and source-length validation.
The stream handle remains valid until `cribra_stream_free`, even after finish or
failure. Repeated finish and every later push (including empty chunks) return
`CRIBRA_STREAM_NOT_ACTIVE`. A failed operation never transfers a partial report.
Freeing an active stream cancels it; freeing null is a no-op. A stream may outlive
its scanner, and a report may outlive both.

`CribraStreamLimits` exposes the three Core limits with separate `uint32_t` enabled
flags and `size_t` values. Each flag must be 0 (unlimited, ignore the value) or 1
(use the value). Enabled zero is a real zero budget; no value is a sentinel.
Zero-initialization makes all budgets unlimited. The limits cover cumulative raw
rule candidates, logical executor storage at admission, and retained finding plus
structural review records before normalization/suppression. Normalization does
not refund slots. Logical storage excludes allocator overhead, spare capacity,
compiled rules, validator workspace and result records; it is not an RSS cap.

Each current Core streaming error has a distinct `CRIBRA_STREAM_*` status,
including resource limits, arithmetic overflows and execution invariant failure.
Future unrecognized Core errors return `CRIBRA_STREAM_UNKNOWN_ERROR`. All Core
execution failures discard the session. Adapter source-byte accounting is checked
and advances only after a successful push; overflow also makes the stream terminal.
Diagnostics are fixed privacy-safe text, with no source, rule text or panic payload.
Unwinding execution panics are contained as `CRIBRA_INTERNAL_ERROR` and leave the
session inactive; process aborts and invalid-memory caller violations cannot be
contained by this ABI.

Null required pointers, invalid enabled flags and invalid buffer descriptors return
`CRIBRA_INVALID_ARGUMENT`; they are not Core execution errors. Invalid buffer
arguments and a null `out_report` do not consume an active stream. A valid report
output slot is cleared before validation. Optional diagnostic slots obey the
error ownership rules above and must be freed before reuse.

Streams require exclusive access: callers must serialize push, finish and free;
there is no adapter mutex or concurrent use guarantee. Caller buffers and output
slots must not alias the stream. Existing immutable scanner rules remain unchanged.

### Incremental documents and snapshots (ABI 0.3.0)

`cribra_scanner_incremental_new` creates an unlimited Core-owned UTF-8 document;
`cribra_scanner_incremental_new_with_limits` also accepts `CribraIncrementalLimits`.
Construction validates UTF-8 and copies the initial bytes once into Core-owned
storage. Core exclusively owns the editable source, caches, revision and report.
There is no adapter source copy, matching logic or edit engine. Complete built-in
scanners are supported by Core incremental scanning; streaming admission rules do
not restrict incremental documents. See [Core incremental semantics](incremental.md).

The initial revision is zero. `cribra_incremental_apply_edit` accepts an
`expected_revision`, a half-open UTF-8 **byte** range `[start, end)`, and replacement
UTF-8 bytes. Successful insertion, replacement, deletion and no-op edits each
increment the revision once. Endpoints must fall on scalar boundaries. Core alone
validates revisions, ranges, boundaries and transactions after ABI input validation.
Replacement bytes are validated before Core is invoked; malformed UTF-8 returns
the existing `CRIBRA_INVALID_UTF8` status without changing the session. Null plus
zero length means empty input; embedded NUL is allowed. Nonempty buffers must be
readable and at most `PTRDIFF_MAX` bytes. Invalid buffer/output arguments leave
the document unchanged. The required edit revision output is zero on failure.

Each current Core error has a distinct `CRIBRA_INCREMENTAL_*` status. Stale revision,
invalid range, invalid UTF-8 boundary, revision overflow and document-length overflow
are recoverable. Core resource/accounting/invariant failures terminate further edits,
which then return `CRIBRA_INCREMENTAL_NOT_ACTIVE`, but preserve the last committed
snapshot. Core defines error precedence for valid UTF-8 requests, including inactive
before stale revision or bad range. A future unrecognized Core error maps to
`CRIBRA_INCREMENTAL_UNKNOWN_ERROR` while preserving Core's actual state. Error
messages are fixed privacy-safe text and never contain document, replacement,
matcher text, or panic payloads.

`CribraIncrementalLimits` carries four pairs of `uint32_t` enabled flags and `size_t`
values. Each flag must be 0 (unlimited; ignore the value) or 1 (use the value).
Enabled zero is a real budget; there are no sentinel values. Zero-initialization
means unlimited budgets. Core charges newly emitted raw candidates per operation,
simultaneously owned old/staged cache records, materialized finding/structural
records per operation, and conservative peak logical transaction bytes. Rejected
and hidden cache records count; suppression refunds no result tickets. Logical
storage excludes allocator capacity/overhead, shared configuration, opaque
regex/validator workspace, FFI input conversion and caller-owned report clones;
it is not a total heap or allocation-failure guarantee.

`cribra_incremental_snapshot` fills one `CribraIncrementalSnapshot` with matching
revision, UTF-8 source byte count and an independently owned complete `CribraReport`.
It remains available after ordinary Core terminal failures. The cloned report
supports all existing report/candidate/explainability/transform APIs, with unchanged
Core ordering, UTF-8 coordinates and review-only semantics. It remains valid through
later edits and after freeing the document or scanner. There is no source getter.
The caller must call `cribra_report_free(snapshot.report)` exactly once, including
before reusing the same output record; copying the C record does not clone ownership.
Snapshot failure clears all fields to zero/null and transfers no partial report.

Every handle operation requires exclusive access, including snapshot and activity
queries. Callers must serialize calls; there is no adapter mutex. Coherence covers
one complete snapshot call, not separate calls made across intervening edits.
Inputs, output slots and handles must not alias. The document may outlive its
scanner. `cribra_incremental_free` releases active, terminal or panic-invalidated
handles; null is a no-op. Snapshot reports remain independently owned.

An unwinding panic during guarded Core/report operations is contained as
`CRIBRA_INTERNAL_ERROR` and permanently removes the adapter's Core session. Later
edits/snapshots return `CRIBRA_INCREMENTAL_SESSION_INVALIDATED`; panic recovery is
not supported and no potentially invalid committed snapshot is exposed.
`cribra_incremental_is_active` succeeds with a `uint32_t` 1 for active documents,
or 0 after either an ordinary terminal failure or panic invalidation. Previously
returned reports remain valid. Allocation aborts and invalid-memory caller-contract
violations are outside panic containment. Optional error-output slots follow the
same ownership and clearing conventions as other C ABI functions.

## 7. Native input contract

Native source input is length-delimited rather than NUL-terminated.

This permits embedded NUL bytes at the ABI transport level without relying on C
string termination. The source must nevertheless satisfy Cribra's UTF-8 input
contract.

Invalid UTF-8 produces an explicit failure rather than silently broadening
Cribra into an arbitrary binary scanner.

String and byte views are borrowed only for the lifetime documented by the
function receiving them.

The caller must provide readable memory for every non-empty input view.

## 8. Errors and panic containment

Expected native failures use explicit status values.

Where an API accepts an error-output parameter:

- success returns the requested successful output and no diagnostic error;
- failure returns no successful output and may return a Rust-owned diagnostic
  error.

Diagnostic errors must not expose matched sensitive values.

Rust panics MUST NOT cross the FFI boundary. Unexpected internal failures are
contained by the native adapter and projected through the ABI failure model.

The adapter validates safely checkable caller state where practical. It cannot
make arbitrary invalid foreign pointers, use-after-free, double-free, or
concurrent destruction memory-safe.

Those remain caller contract violations inherent to a native FFI boundary.

### P0.7 — Raw Pointer Safety and CodeQL Audit

An output-slot address is distinct from the pointer stored in that slot. For
example, `CribraReport **out_report` addresses caller-owned pointer storage;
`*out_report` may initially be null without `out_report` itself being null.
Required output slots must be valid, correctly aligned, writable for the complete
output value, and live throughout the call. Callers must obey documented
exclusive-access and non-aliasing requirements for slots, inputs and handles.
Checking a pointer for null does not validate arbitrary foreign memory.

Optional diagnostic slots may be null. `ErrorSlot` checks its slot address for
null before reading or writing through it. A non-null diagnostic slot must meet
its documented readable/writable lifetime and exclusive-access requirements;
its stored error pointer may initially be null.

`clear_out()` writes null into a pointer slot; `clear_value()` writes a default
record, including any null pointer fields. Neither dereferences nor frees a
pointer previously stored there. Callers MUST release previously owned outputs
before reusing their slots. Successful handle/report-producing calls transfer
Rust allocations through `Box::into_raw()`; each must be released exactly once
using its corresponding Cribra destruction function, never the caller allocator.
See [§6, Native ownership and lifetime model](#6-native-ownership-and-lifetime-model).

Incremental snapshot reports and finished-stream reports are independently
owned. They remain valid after subsequent edits and destruction of their
originating session or scanner, until `cribra_report_free()` releases them.
Copying a snapshot record or raw pointer does not duplicate ownership. Borrowed
views still require their owner to remain alive; APIs requiring a scanner still
require a live scanner.

Negative C ABI tests may store null or `ptr::dangling_mut()` sentinels in valid
output slots to verify clearing or replacement. Passing the address of such a
slot is different from passing a dangling slot address. Sentinels must never be
dereferenced or freed, and successful outputs must be checked before use.

**Historical audit.** The read-only audit of PR #53 at commit
`d412cee1da90bf087171645a4cc8717fdbed6fc0` reviewed CodeQL Rust analysis
`2004343041`, CodeQL version 2.27.3, rule `rust/access-invalid-pointer`.
The 39 PR alerts (#59–#97) were individually classified as false positives,
including review of 48 explicit PR code flows. Three separate existing
`ErrorSlot` alerts (#4, #23, #24) were also classified as false positives. No
genuine memory-safety defect was identified among the reported paths.

Observed alert paths propagated initial null or sentinel values past checked
successful output writes and complete snapshot replacement, or conflated stored
pointer values with slot addresses and included guarded null diagnostic slots.
These observations describe the exported analysis; they do not establish
CodeQL's internal implementation or the precise cause of its results.

The conclusions apply only to the audited revision and paths, depend on the
documented unsafe FFI caller contracts, and do not prove whole-library memory
safety. They do not automatically classify future alerts or justify disabling
or suppressing the CodeQL rule. Future alerts require independent review. See
the [§23 threat model](#23-threat-model) for caller-memory and lifetime limitations.

## 9. Scanner construction

The native builder mirrors Cribra's explicit scanner-construction model.

The current default security/credential catalog can be added through:

```text
cribra_builder_add_current_builtins
```

The opt-in financial catalog can be added through:

```text
cribra_builder_add_financial_builtins
```

These catalogs remain semantically distinct. Adding financial detection does
not implicitly broaden Cribra's default built-in scanner.

The financial pack currently includes the core-owned rules:

```text
financial.iban
financial.pan
```

Custom public rule families are configured through the builder rather than by
exposing Rust `Rule` layouts.

Rule-ID uniqueness and scanner-wide validation remain core authority.

Once successfully constructed, a scanner is immutable and reusable.

### Scanner rule metadata (P0.6B)

The public native header is `include/cribra.h`, generated from `cribra-capi`
with its existing cbindgen configuration. `cribra_scanner_rule_count` and
`cribra_scanner_rule_at` enumerate Core's active rule metadata directly. Indexes
follow `Scanner::rule_metadata()` exactly: caller-order custom rules precede
the catalogs in Core builder order. The adapter keeps no second rule inventory.

`CribraRuleMetadataView` has exactly five fields: `rule_id`, `kind`,
`detection_mode`, `severity` and `remediation`. It uses existing ABI enum types;
`CRIBRA_REMEDIATION_NONE` represents absent remediation. The newly assigned
`CRIBRA_REMEDIATION_REVIEW_PASSWORD_VERIFIER = 7` projects Core's existing action
through metadata and report views and is accepted in rule configuration.
Previously this action projected as `CRIBRA_REMEDIATION_UNKNOWN`; all existing
numeric assignments remain unchanged.

`rule_id` is a borrowed, non-NUL-terminated UTF-8 view into immutable scanner
storage. It survives scans and report destruction and expires at scanner free.
Do not free it. Copy the bytes before freeing the scanner if needed longer.
No field exposes matcher payloads, regexes, validators or scanned source.
Null inputs/outputs return `CRIBRA_INVALID_ARGUMENT`; invalid indexes return
`CRIBRA_OUT_OF_RANGE`. Writable outputs are cleared before handle/index checks.
Pointer validity, non-aliasing and live-owner obligations follow the existing ABI.

### Structured construction diagnostics (P0.6B)

The existing owned `CribraError` remains opaque. `cribra_error_status`,
`cribra_error_message`, optional error-output slots and `cribra_error_free`
keep their existing behavior. Construction still returns coarse
`CRIBRA_RULE_ERROR` or `CRIBRA_BUILD_ERROR`; fixed sanitized messages are unchanged.
No new general status codes or changes to public struct layouts are introduced.

`cribra_error_build_info(error, out_info)` fills a new `CribraBuildErrorView`:
`code` is a stable Core-derived category and `rule_id` is an optional borrowed
identity copied into the error at failure. A null ID pointer means absent;
otherwise its UTF-8 bytes remain valid until error free, independently of the
builder/scanner lifetime. Empty IDs may be present in constructor context;
Core's `EmptyRuleId` build error itself has no supplied identity. Neither view
owns memory. Non-construction errors return code `CRIBRA_BUILD_CODE_NONE` and
no ID. Null arguments fail and clear writable output just like other accessors.

| Category suffix (`CRIBRA_BUILD_CODE_*`) | Value | Core origin |
| --- | ---: | --- |
| `NONE` | 0 | Non-construction status/error |
| `INVALID_PATTERN` | 1 | `RuleError::InvalidPattern` |
| `PATTERN_MATCHES_EMPTY` | 2 | `RuleError::PatternMatchesEmpty` |
| `MISSING_CAPTURE_GROUP` | 3 | `RuleError::MissingCaptureGroup` |
| `EMPTY_RULE_ID` | 4 | `ScannerBuildError::EmptyRuleId` |
| `DUPLICATE_RULE_ID` | 5 | `ScannerBuildError::DuplicateRuleId` |
| `EMPTY_MATCHER` | 6 | `ScannerBuildError::EmptyMatcher` |
| `TOO_MANY_RULES` | 7 | `ScannerBuildError::TooManyRules` |
| `AUTOMATON_BUILD` | 8 | `ScannerBuildError::AutomatonBuild` |

Categories come from Rust enum matches, never diagnostic parsing. Core still
owns pattern construction, catalog composition, ordering and validation error
precedence. Invalid patterns leave the builder usable; a validated build attempt
consumes it on success or failure, as before. C diagnostics remain fixed and
source-free. WASM retains Core's human-readable regex diagnostics, which may
contain caller-supplied configuration patterns; those are not scanned source.

The native regression suite compares all catalog selections and four custom
families against Core, checks borrowed ownership/UTF-8 IDs, optional remediation,
structured categories and null/index handling. Header contract fixtures pin
172 prior declarations and 16 additions (types, fields, codes and signatures).
The C11 smoke exercises the new views and errors; C/C++ examples compile and run.
Rare capacity/capture errors are conversion checks; automaton resource failure
is mapped but not forced. The ABI protocol remains 0.3.0 with additive symbols.

Regenerate with `just capi-header` and check `just capi-header-check`. If Cargo's
unfiltered metadata discovery needs uncached dependencies for another platform,
the equivalent host-filtered generation is:

```sh
cbindgen --only-target-dependencies --config crates/cribra-capi/cbindgen.toml --crate cribra-capi --output include/cribra.h
```

Repeat that command with a temporary output and compare bytes for determinism.
Filtering dependency discovery does not change the C ABI source/configuration.

## 10. Scanning

The native API supports single-source scanning and batch-oriented operations
where exposed by the current header.

Single-source scanning borrows caller-owned UTF-8 source bytes and returns a
Rust-owned report.

Batch scanning associates caller inputs with report results without changing
per-source detection semantics.

Where serial and parallel batch execution are available, execution strategy
must not change:

- per-source findings;
- per-source candidates;
- semantic ordering;
- input ordering.

Parallelism is an execution property, not a classification property.

## 11. Finding projection

Findings are projected as report-owned metadata views.

The native projection preserves the applicable core facts, including:

- stable rule ID;
- byte span;
- one-based Unicode line and column;
- severity;
- confidence;
- remediation.

Finding projections do not expose the matched source value.

A finding means the configured core pipeline had sufficient evidence to
classify the span. The adapter does not independently strengthen or weaken that
decision.

## 12. Sensitive-candidate projection

`SensitiveCandidate` remains distinct from `Finding`.

Candidates represent review-only ambiguity and therefore do not receive finding
severity, confidence, or remediation merely for ABI convenience.

Candidate projections preserve the applicable core facts, including:

- candidate kind;
- byte span and coordinates where exposed;
- evidence;
- deterministic ordering.

Candidates do not participate in automatic transformations unless the Rust core
itself changes that semantic contract.

## 13. Explainability

Explainability is projected from core-owned semantics.

Finding explanations derive from scanner/rule authority. Candidate explanations
derive from candidate evidence.

Adapters may change representation for host-language ergonomics, but they MUST
NOT independently infer why something was classified.

Explainability is descriptive. It is not a second classifier.

## 14. Transformations

Native transformations operate from:

```text
caller-owned original source
        +
classified Cribra report
        │
        ▼
Rust-owned transformed output
```

The native adapter exposes applicable core transformation capabilities,
including:

- redaction;
- custom redaction;
- templating;
- pseudonymization;
- deterministic synthesis.

Transforms operate on classified finding spans. Review-only candidates are not
automatically transformed.

The returned transformed output is Rust-owned. A borrowed output view remains
valid only until its owning output handle is destroyed.

Adapters MUST NOT reconstruct sensitive values from finding metadata.

## 15. C and C++ consumption

C consumes `include/cribra.h` directly.

A runnable C integration is maintained under:

```text
examples/c/
```

It demonstrates:

```text
builder
  → current built-ins
  → opt-in financial built-ins
  → scanner
  → scan
  → finding traversal
  → redaction
  → explicit cleanup
```

C++ uses the same C ABI. Cribra does not maintain a separate C++ semantic
implementation.

The runnable example under:

```text
examples/cpp/
```

adds small application-local move-only RAII wrappers around Rust-owned handles.
Those wrappers demonstrate normal C++ ownership ergonomics but are not a
separate official binding.

## 16. ABI representation

Rust-specific layouts are not exported.

Rust `bool` layout is not an ABI contract.

Rust enum layout is not an ABI contract.

ABI-visible protocol values use deliberately defined C representations whose
numeric assignments become part of the published ABI once exposed.

Exact declarations and numeric values are defined by `include/cribra.h`, not
duplicated in this architecture document.

This separation prevents documentation prose from becoming a second,
potentially stale header.

## 17. ABI versioning

Cribra exposes an explicit native ABI version independently from Rust crate
SemVer. The current native protocol is **0.3.0**, adding Core-owned incremental
documents and coherent snapshots alongside 0.2.0 streaming. Existing entry-point
signatures and numeric statuses remain unchanged.

Crate version and ABI protocol version answer different questions:

```text
crate SemVer
    → Rust package evolution

ABI version
    → foreign native protocol evolution
```

A crate release therefore does not imply that the ABI protocol version must
change.

Compatibility policy must be based on the published ABI contract rather than
Rust implementation details.

## 18. Header authority

`include/cribra.h` is the committed public native header.

The header is generated from `cribra-capi` through `cbindgen` and checked
against the committed copy.

The repository gate requires generated and committed declarations to remain in
sync.

The header owns exact native details such as:

- function names;
- structs;
- enum/constants;
- numeric representations;
- argument types;
- ownership notes;
- lifetime requirements;
- safety requirements.

This document owns architecture and invariants.

## 19. Native artifact model

`cribra-capi` provides the native library artifacts required by the supported
build and validation paths.

Cribra validates the C ABI using real foreign consumers rather than Rust tests
alone.

Repository validation includes C compilation/linking smoke coverage and checks
for the generated header contract. Static-library production is also validated
by the repository's native gates.

A successful Rust build alone is not considered sufficient evidence of usable
native interoperability.

## 20. WebAssembly adapter

`cribra-wasm` is an independent typed `wasm-bindgen` adapter directly over
`cribra`.

```text
JavaScript / TypeScript
          │
          ▼
     cribra-wasm
          │
          ▼
        cribra
```

It does not call `cribra-capi`.

The core remains browser-agnostic, and the adapter does not depend on DOM APIs
for its detection semantics.

The production Wasm artifact is optimized with Binaryen `-Oz`.

Detailed Wasm architecture, optimization evidence, initialization behavior,
browser validation, and parity results are documented in
`docs/WASM_INTEROP.md`.

## 21. Browser and JavaScript-runtime consumption

The same production WebAssembly artifact can be consumed across the validated
JavaScript environments.

Browser integrations use the generated WebAssembly URL/fetch initialization
path.

Server-side JavaScript runtimes may load the Wasm file themselves and pass its
bytes explicitly to the generated initialization function.

The maintained examples demonstrate:

```text
cribra.js + cribra_bg.wasm
        │
        ├── browser
        │     └── URL / fetch initialization
        │
        ├── Node.js
        │     └── explicit Wasm bytes
        ├── Bun
        │     └── explicit Wasm bytes
        └── Deno
              └── explicit Wasm bytes
```

These are integration paths over one semantic implementation and one production
Wasm artifact, not separate Cribra engines.

The concrete runtime versions exercised by the examples are evidence of tested
environments, not a permanent compatibility-range promise.

## 22. Web Worker and host policy

The Wasm adapter remains independent from DOM-only detection behavior and is
suitable for ownership by a Web Worker.

Host applications own:

- Worker lifecycle;
- message transport;
- UI;
- persistence;
- service-worker behavior;
- caching policy;
- filesystem access where available;
- network policy.

Cribra does not require these facilities for local scanning.

Threaded WebAssembly is not part of the current interoperability contract.
Native optional parallelism and browser Worker concurrency are separate
execution concerns.

## 23. Threat model

### 23.1 Hostile or erroneous native caller

The native adapter can validate protocol state that is safely inspectable, but
cannot make arbitrary foreign memory trustworthy.

Examples of caller contract violations include:

- invalid non-null pointers;
- insufficient readable input memory;
- use-after-free;
- double-free;
- using a consumed builder;
- using borrowed views after their owner is destroyed;
- concurrent destruction while an object is in use;
- violating documented mutability or synchronization requirements.

Cribra avoids unnecessary global state or handle registries merely to simulate
memory safety for arbitrary invalid foreign pointers.

### 23.2 Implementation failure

Unexpected Rust panics are contained at FFI boundaries.

Errors and panic diagnostics must not disclose matched sensitive values.

Adapter failures must not silently reinterpret data as a different
classification result.

### 23.3 Lifetime misuse

Ownership and lifetime rules are part of the public protocol.

Opaque handles prevent foreign callers from depending on Rust layout, but they
do not eliminate the caller's responsibility to obey destruction and borrowing
rules.

Higher-level language wrappers may encode these rules more safely through RAII,
garbage collection, or other host-language ownership mechanisms.

## 24. Cross-adapter semantic parity

Equivalent supported operations preserve semantics across Rust, native C ABI,
and WebAssembly adapters.

For equivalent scanner configuration and source, parity validation covers
applicable facts including:

- finding count;
- finding order;
- rule IDs;
- byte spans;
- one-based Unicode line/column coordinates;
- severity;
- confidence;
- remediation;
- candidate count;
- candidate order;
- candidate kinds;
- candidate evidence;
- explanation facts;
- transformed output.

Representations may differ. Semantics may not.

The opt-in financial catalog participates in adapter parity where the adapter
exposes built-in pack composition.

Canonical or shared fixtures are preferred over unrelated duplicated corpora.

### Adapter capability matrix

The following matrix summarizes adapter capabilities. A check mark indicates
an exposed capability, not identical APIs or universal platform qualification.

| Capability | C ABI | Swift | Kotlin/JVM | Python | WebAssembly |
| --- | :---: | :---: | :---: | :---: | :---: |
| Scanner and configuration | ✓ | ✓ | ✓ | ✓ | ✓ |
| Findings, candidates and coordinates | ✓ | ✓ | ✓ | ✓ | ✓ |
| Metadata, remediation and explanations | ✓ | ✓ | ✓ | ✓ | ✓ |
| UTF-8 streaming | ✓ | ✓ | ✓ | ✓ | ✓ |
| Incremental scanning | ✓ | ✓ | ✓ | ✓ | ✓ |
| Batch scanning | ✓ | ✓ | ✓ | ✓ | ✓ |
| Transformations | ✓ | ✓ | ✓ | ✓ | ✓ |
| In-memory Share Bundles | ✓ | ✓ | ✓ | ✓ | ✓ |
| Parallel scanning | ✓ | ✓ | ✓ | ✓ | —* |

**Notes**

- **C ABI parallel scanning:** `CRIBRA_BATCH_EXECUTION_AUTO` may use parallel
  execution when the `cribra-capi/parallel` feature is enabled; otherwise
  execution remains serial.
- **Swift, Kotlin/JVM and Python:** transformations and Share Bundles require
  Flow or Fuse; parallel scanning requires Fuse.
- **WebAssembly:** transformations and Share Bundles require Flow.
  Rayon parallel scanning is intentionally outside the WebAssembly contract.
- **Streaming:** only Core-admitted scanner configurations support streaming.
- **Semantic parity:** equivalent supported operations preserve Core semantics.
  This matrix does not imply identical APIs, identical profile availability,
  or qualification on every operating system and runtime.

See [compatibility and qualification](COMPATIBILITY.md),
[WebAssembly interoperability](WASM_INTEROP.md), and the individual adapter
consumer guides for evidence and platform boundaries.

## 25. Normative interoperability invariants

The following invariants govern Cribra interoperability.

**I1.** The Cribra core is the only detection authority.

**I2.** `cribra-capi` and `cribra-wasm` are independent adapters over the Rust
core.

**I3.** No Rust object layout is part of the C ABI.

**I4.** Every FFI-owned allocation has exactly one documented destruction path.

**I5.** Borrowed views are valid only for the documented lifetime of their
owner.

**I6.** Caller-provided source memory is never retained beyond the documented
operation unless an API explicitly states otherwise.

**I7.** No sensitive matched value is returned through findings, candidates,
errors, explanations, or metadata projections.

**I8.** Native source input is length-delimited and explicitly UTF-8 validated.

**I9.** No Rust panic crosses an FFI boundary.

**I10.** ABI primitive representations are explicitly defined where required by
the contract.

**I11.** Rust `bool` and Rust enum layouts never become ABI contracts.

**I12.** Report ordering preserves Rust semantic ordering.

**I13.** `SensitiveCandidate` remains semantically distinct from `Finding`.

**I14.** Explainability never becomes an independent classification authority.

**I15.** Transform operations require caller-provided original source.

**I16.** Rust-owned allocation is never freed with the caller allocator.

**I17.** No global mutable state is required for normal ABI operation.

**I18.** Scanner objects are immutable after successful construction.

**I19.** ABI protocol versioning is independent from crate SemVer.

**I20.** Rust, C ABI, and WebAssembly satisfy semantic parity where equivalent
APIs exist.

**I21.** Neither interoperability adapter performs network access as part of
scanning.

**I22.** Interoperability convenience must not alter detection semantics.

**I23.** The core crate's `forbid(unsafe_code)` boundary remains intact;
required FFI `unsafe` belongs in the dedicated native adapter crate.

**I24.** Parallel execution strategy is not observable as a change in logical
scan results or input ordering.

**I25.** Built-in pack selection is explicit: adapter convenience must not
silently promote opt-in catalogs into the default catalog.

**I26.** A consumed native builder is never reused or independently destroyed.

## 26. Validation contract

Interoperability is validated through actual consumer boundaries, not only
through Rust unit tests.

The maintained validation surface includes:

- generated-header synchronization;
- C ABI Rust tests;
- real C compilation and linking;
- native smoke tests;
- static native artifact validation;
- Wasm adapter tests;
- Rust/Wasm semantic parity fixtures;
- optimized production Wasm generation;
- real-browser validation;
- runnable Node.js, Bun, and Deno consumers;
- runnable C and C++ consumers.

Examples are intentionally executable integration evidence rather than
documentation-only pseudocode.

## 27. Release principle

Interoperability work must make Cribra easier to embed without turning adapters
into separate detection engines.

The boundary preserves:

- local-first operation;
- privacy-first behavior;
- deterministic semantics where applicable;
- contextual classification where required;
- explicit ambiguity;
- explainability;
- safe transformations;
- a reusable Rust core;
- no unnecessary coupling to UI, filesystem, network, or host-runtime policy.

New adapters or wrappers should be added only when they can preserve these
properties and have a concrete consumer need.
