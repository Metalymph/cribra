# Cribra Interoperability Architecture

Status: v0.3 design authority  
Scope: native C ABI and existing WASM/PWA interoperability  
Implementation status: specification only

## 1. Purpose

Cribra v0.3 establishes a deliberate interoperability boundary around the existing Rust core without expanding detection intelligence.

The Rust crate remains the only semantic authority for detection, validation, candidate promotion, explainability, remediation, ordering, and transformations. Native and WASM consumers receive projections of those semantics; they do not reimplement them.

The target architecture is:

```text
Cribra Core (`cribra`)
    ├── Rust API
    ├── C ABI adapter (`cribra-capi`)
    └── WASM/PWA adapter
```

The C ABI is the universal native-language protocol. It is intended to make Cribra embeddable from C/C++, Swift, JNI/Kotlin, Python, Go, .NET, and other FFI-capable runtimes without requiring v0.3 to ship complete language-specific wrappers.

Cribra already operates through WASM in the Silens Scan PWA. v0.3 therefore treats WASM as an existing working integration surface to audit and refine, not as a greenfield backend.

## 2. Current core facts that constrain interop

The current Rust core defines several important constraints that the adapters must preserve:

- `Scanner` is an immutable compiled scanner backed internally by shared compiled state and can be reused across inputs.
- `Scanner::builder()` creates an empty builder.
- `Scanner::default()` resolves to the current built-in scanner.
- The public Rust scanning API is batch-oriented: one source is represented as a one-element batch.
- Serial and optional Rayon-backed parallel scanning preserve input order and use the same per-source semantics.
- `Finding` stores metadata and coordinates, not the matched source value.
- `SensitiveCandidate` is a separate review-only model, not a weaker `Finding`.
- finding explainability is resolved against scanner-owned rule metadata rather than duplicated into each finding.
- candidate explainability derives from candidate evidence.
- transforms consume the original caller-owned source plus a `ScanReport`.
- custom rules expose literal, prefix, suffix, and full-match pattern semantics; capture projection remains an internal built-in capability.
- the core crate forbids unsafe Rust. Native FFI implementation must therefore live outside the core rather than weakening this invariant.

These are architectural contracts, not implementation details to rewrite for FFI convenience.

## 3. Non-goals

Cribra v0.3 interoperability work does not include:

- new provider detectors;
- new detection families;
- new validation families;
- a plugin system;
- a CLI;
- full Swift, Kotlin/JNI, Python, Go, or .NET bindings;
- exposing the Rust ABI;
- exposing Rust object layouts;
- routing WASM through the C ABI;
- redesigning the Rust query layer for FFI;
- exposing matched secret values;
- making Cribra an arbitrary binary scanner;
- changing detection semantics to reduce adapter complexity.

Language-specific wrappers may be built later on top of the stable-designed C protocol when justified.

## 4. Adapter architecture

The intended dependency direction is:

```text
cribra-capi ───────────▶ cribra
WASM/PWA adapter ──────▶ cribra

cribra-capi ✕ WASM/PWA adapter
```

Neither adapter is an authority for classification.

The existing published `cribra` package should remain Rust-native. When `cribra-capi` is introduced, the preferred repository evolution is for the current root package to also become the workspace root, unless implementation work reveals a concrete reason to move the core crate. Repository symmetry alone is not sufficient reason for churn.

A likely structure is:

```text
cribra/
├── Cargo.toml
├── src/
├── crates/
│   └── cribra-capi/
├── include/
├── docs/
├── tests/
└── ...
```

The exact workspace form is deferred to v0.3.2.

## 5. Core authority

Adapters MUST project existing Cribra semantics.

Adapters MUST NOT independently:

- classify a source span;
- implement provider validation;
- promote an ambiguous candidate into a finding;
- infer finding confidence;
- infer severity;
- derive remediation independently from core metadata;
- resolve overlap/collision semantics differently from the core;
- reorder results except where the public adapter contract explicitly defines a separate presentation ordering;
- create a second explainability authority.

When adapter ergonomics conflict with core semantics, core semantics win.

## 6. C ABI design principles

The C boundary MUST use:

- opaque Rust-owned handles for stateful/owned Rust objects;
- explicitly sized primitive representations;
- pointer-plus-length input buffers;
- explicit UTF-8 validation;
- explicit ownership and destruction;
- adapter-defined C DTO/view layouts where useful;
- panic containment at exported FFI entry points;
- status codes plus explicit error objects for fallible operations.

The C boundary MUST NOT expose:

- Rust references such as `&str`;
- Rust slices or `Vec` layout;
- Rust enum layout;
- Rust `bool` layout;
- Rust `String` layout;
- Rust `Scanner`, `ScanReport`, `Finding`, `SensitiveCandidate`, `Rule`, `Explanation`, or other Rust object layouts;
- Rust allocator ownership to a foreign allocator.

Normative ownership invariant:

> Every allocation returned across the ABI has exactly one documented destruction path.

## 7. Proposed opaque handles

The initial design should center on opaque handles conceptually equivalent to:

```c
typedef struct cribra_scanner cribra_scanner_t;
typedef struct cribra_builder cribra_builder_t;
typedef struct cribra_report cribra_report_t;
typedef struct cribra_buffer cribra_buffer_t;
typedef struct cribra_error cribra_error_t;
```

The exact C declarations are deferred to implementation.

Findings and candidates SHOULD NOT receive one heap allocation and one opaque handle per item. They are naturally owned by a report and should normally be projected through indexed report access into borrowed ABI views.

## 8. ABI-defined views and DTOs

Avoiding Rust layout exposure does not require every value to be opaque.

The adapter may intentionally define C-compatible POD/view types such as:

```text
cribra_bytes_view_t
cribra_string_view_t
cribra_finding_view_t
cribra_candidate_view_t
```

These are ABI contracts designed for C. They are projections from Rust values, not declarations that the Rust values have matching layout.

A view does not imply ownership. Every view type MUST document the owner that keeps its referenced memory alive.

## 9. Ownership model

The initial ownership model is:

| Resource | Owner | Destruction |
| --- | --- | --- |
| scanner handle | caller after successful creation | `cribra_scanner_free`-style function |
| builder handle | caller after successful creation | `cribra_builder_free`-style function |
| report handle | caller after successful scan | `cribra_report_free`-style function |
| transformed output buffer | caller after successful transform | `cribra_buffer_free`-style function |
| error object | caller when returned | `cribra_error_free`-style function |
| finding view | report | never freed directly |
| candidate view | report | never freed directly |
| explanation view/data | documented owner, likely scanner/report or explicit owned object depending final projection | defined by final API |
| input source | foreign caller | never freed or retained by Cribra unless a future API explicitly states otherwise |

Destroying a parent invalidates all borrowed views whose documented lifetime depends on that parent.

## 10. Lifetime model

Caller-provided input memory is borrowed only for the duration of the FFI call unless an API explicitly documents otherwise.

The scan boundary MUST NOT retain the source text merely to make later report traversal easier.

Borrowed report views are valid only while the report remains alive. If a view references scanner-owned metadata, both required owners must remain alive for the documented lifetime.

Using a borrowed view after its owner is destroyed is a caller contract violation.

Destroying an object while another thread is concurrently using it is a caller contract violation unless a later API explicitly provides synchronized ownership semantics.

## 11. Input representation

Cribra remains a UTF-8 text engine.

Native input SHOULD use pointer-plus-length representation rather than NUL-terminated strings as the primary contract.

Conceptually:

```c
const uint8_t *ptr;
size_t len;
```

The adapter validates UTF-8 before constructing a Rust `&str` view for the core.

Invalid UTF-8 MUST produce an explicit ABI error/status. v0.3 MUST NOT silently broaden the core into arbitrary binary scanning.

A null pointer with zero length may be accepted as an empty buffer if the final ABI defines it that way. A null pointer with non-zero length MUST be rejected before dereference.

## 12. Integer, boolean, and enum representation

ABI-facing integer widths MUST be explicit when semantic width matters.

Rust `bool` MUST NOT cross the ABI directly. Boolean-like values should use an explicitly documented fixed-width representation such as `uint8_t` with defined true/false constants.

Rust enums MUST NOT cross the ABI by layout assumption. ABI concepts such as severity, confidence, candidate kind, evidence, remediation, rule kind, detection mode, and transform mode should use fixed-width integer aliases/constants or deliberately designed C tagged representations.

Adding an internal Rust enum variant MUST NOT automatically become an ABI break.

## 13. Error model

The preferred design is a coarse status code plus an explicit error object for diagnostics.

A global or thread-local `last_error` mechanism is NOT the default design because it complicates reentrancy, thread semantics, nested calls, and message lifetime.

Conceptual status categories are:

```text
CRIBRA_OK
CRIBRA_INVALID_ARGUMENT
CRIBRA_INVALID_UTF8
CRIBRA_RULE_ERROR
CRIBRA_BUILD_ERROR
CRIBRA_TRANSFORM_ERROR
CRIBRA_OUT_OF_RANGE
CRIBRA_INTERNAL_ERROR
```

The exact numeric assignments are deferred to implementation and become ABI contracts once published.

Status values should remain coarse. Detailed diagnostics belong in `cribra_error_t` rather than mirroring every Rust error variant into the ABI.

For an operation with an output object and optional error output, the success contract should be unambiguous: success returns a valid requested output and no error object; failure returns no successful output and may return a diagnostic error object.

Error messages MUST NOT contain matched secret values or source snippets.

## 14. Panic containment

No Rust panic may cross an `extern "C"` boundary.

Every fallible exported function must be panic-contained at the ABI boundary.

Panic containment is a final safety net, NOT ordinary control flow. Normal invalid input must be validated and returned through the status/error model.

A contained unexpected panic should map to an internal-error status without exposing sensitive source data in diagnostics.

## 15. Caller contract and unavoidable C memory unsafety

The adapter should defend against inputs it can validate safely, including:

- null pointers where pointer validity can be checked before access;
- invalid UTF-8;
- invalid ABI enum/integer codes;
- out-of-range report indexes;
- malformed custom rule configuration;
- unsupported option combinations.

The adapter cannot make arbitrary foreign memory safe. The following remain caller contract violations and may cause undefined behavior:

- passing a dangling pointer;
- declaring a length larger than the accessible memory region;
- use-after-free of an opaque handle;
- double-free of an opaque handle;
- passing a forged pointer as a Cribra handle;
- concurrent destruction while another call uses the same object;
- concurrent destruction of an owner while another thread uses a borrowed view
  derived from that owner;
- concurrent mutation of a builder without caller-provided synchronization.

These remain caller-side undefined-behavior boundaries even when the underlying
immutable handle type is `Send + Sync`. The ABI does not provide automatic
reference counting or synchronization for foreign handles.

v0.3 SHOULD NOT introduce a global handle registry merely to simulate safety for these cases. Such a registry would add global mutable state, synchronization, lookup overhead, and lifecycle complexity without making arbitrary foreign memory trustworthy.

## 16. Scanner lifecycle

The native lifecycle should preserve the core separation between configuration and immutable execution:

```text
builder
   │ configure rules
   ▼
build
   │
   ▼
immutable scanner
   │
   ├── scan
   ├── scan
   └── scan
```

The ABI must preserve the semantic distinction already present in Rust:

- a new `ScannerBuilder` starts with no rules;
- `Scanner::default()` uses the current built-in scanner.

Therefore native convenience naming should be explicit. A function conceptually named `scanner_new_current` is preferable to an ambiguous ABI-level `default` if it means “current canonical built-ins”.

The scanner becomes immutable after successful construction.

## 17. Single-source scan boundary

Although the Rust API is intentionally batch-oriented, the first C ABI scan primitive should be single-source because it gives the smallest ownership and failure contract:

```text
scanner + caller-owned UTF-8 source
              │
              ▼
        owned report handle
```

The adapter may call the existing Rust batch API with one source internally. It MUST NOT create a separate detection implementation.

The eventual batch API is layered on top of the same semantics after the primitive lifecycle is proven.

## 18. Report traversal

A report owns immutable findings and ambiguous candidates.

The initial C report surface should provide concepts equivalent to:

- finding count;
- candidate count;
- review-needed state where justified by the current Rust contract;
- indexed finding access;
- indexed candidate access.

Rust iterators, generics, `ScanQuery`, and `SortedScanQuery` SHOULD NOT be mirrored directly into the initial C ABI.

A foreign consumer can filter or sort a modest report using ordinary host-language facilities. Replicating the entire Rust query DSL would unnecessarily expand and destabilize the ABI.

## 19. Finding projection

A finding projection may expose presentation-safe metadata already present in the Rust model, including:

- rule identifier;
- zero-based half-open byte span;
- one-based line;
- one-based Unicode-scalar column;
- severity;
- confidence;
- optional remediation state or code.

A finding projection MUST NOT expose the matched source value.

The coordinate contract remains the existing Cribra contract. The adapter MUST NOT reinterpret column as byte index, UTF-16 code unit, grapheme cluster, or zero-based column.

## 20. SensitiveCandidate projection

`SensitiveCandidate` remains a separate review-only channel.

The ABI projection should preserve, as applicable:

- candidate kind;
- source location/span;
- candidate evidence.

Candidates MUST NOT gain finding severity, finding confidence, or remediation merely to simplify C representation.

Candidates MUST NOT participate in automatic transforms unless the Rust core changes that semantic contract in a future release.

## 21. Explainability

Explainability must preserve the current two-authority model:

```text
Finding
  + Scanner rule metadata
        │
        ▼
Explanation::Classified(DetectionMode)

SensitiveCandidate
  + CandidateEvidence
        │
        ▼
Explanation::Ambiguous(...)
```

Finding explanation is not independent state stored in the finding. It is resolved against scanner-owned rule metadata and may fail closed when the scanner cannot resolve the finding unambiguously.

The C ABI therefore SHOULD NOT simply embed duplicated detection-mode authority into every finding view.

A likely native shape is an explicit explanation query that receives the scanner plus report/finding identity or index. The exact call shape is deferred to v0.3.5.

Candidate explanation may be projected directly from candidate evidence because that is already the core semantic source.

Human-facing explanation strings remain a consumer concern.

## 22. Custom rules

The native ABI should configure custom rules through the builder rather than expose Rust `Rule` layout.

The intended native configuration surface covers the same public custom-rule families as Rust:

- literal;
- prefix;
- suffix;
- pattern with full-match span semantics.

The adapter must route construction through existing Rust rule constructors and scanner build validation wherever possible rather than duplicate semantic validation.

Rule IDs remain scanner-wide stable identities and duplicate IDs remain build errors.

Capture projection MUST NOT be exposed as a custom-rule feature while it remains an internal built-in capability in the Rust public contract.

## 23. Transform API

Transform operations preserve the current source-plus-report model:

```text
caller-owned original source
            +
          report
            │
            ▼
     Rust-owned output buffer
```

The initial native transform surface should eventually cover the v0.2 transform capabilities where practical, including redaction, templating, pseudonymization, synthesis, and later share-bundle projection if the ownership model remains clear.

Owned transform output MUST be destroyed only through the documented Cribra buffer destruction function. Foreign code MUST NOT pass Rust-owned memory to `free`, `delete`, or another host allocator.

Existing span validation, overlap behavior, and transformation semantics remain authoritative.

## 24. Source/report consistency

Transforms are semantically valid only when the supplied report belongs to the same logical source used for transformation.

The ABI documentation must state this precondition explicitly.

The adapter SHOULD evaluate retaining non-sensitive source metadata such as source byte length inside the native report wrapper so obvious mismatches can be rejected cheaply. This is an implementation option, not yet a normative requirement.

v0.3 MUST NOT introduce mandatory source hashing solely for FFI without measured evidence that the extra cost and state are justified.

## 25. Batch scanning

Batch scanning is a later native layer built after the single-source lifecycle is proven.

v0.3.8 must resolve:

- source key representation;
- input array/view representation;
- output ownership;
- per-input ordering;
- partial failure semantics;
- serial versus optional parallel execution;
- FFI-call amortization for small inputs.

The batch contract must preserve the Rust guarantee that input order is preserved.

## 26. Parallel execution

Rayon is an implementation choice, not an ABI semantic concept.

Where native batch scanning allows execution strategy selection, serial and parallel modes must produce the same logical per-source results and preserve input ordering.

The ABI MUST NOT expose Rayon types, thread-pool internals, or Rust parallel iterators.

Cribra currently does not split one source into chunks for parallel scanning and does not create a private Rayon pool. v0.3 should preserve that behavior unless separately changed in the core for justified reasons.

## 27. Thread safety

The native ABI uses a per-handle thread contract. The immutable owned handle
types listed below are verified by compile-time Rust assertions to satisfy
`Send + Sync` in the adapter implementation.

| Handle | Native concurrency contract |
| --- | --- |
| `CribraScanner` | concurrent immutable scan/read use is supported |
| `CribraReport` | concurrent read-only traversal is supported |
| `CribraBatchResults` | concurrent read-only traversal is supported |
| `CribraOutput` | concurrent read-only view access is supported |
| `CribraShareBundle` | concurrent read-only traversal is supported |
| `CribraError` | concurrent read-only status/message access is supported |
| `CribraBuilder` | mutable configuration is externally synchronized; concurrent mutation is not supported |

`Send + Sync` describes the implementation capability of the immutable owned
handles; it does not create lifetime management for foreign callers. A caller
must keep every handle alive for the full duration of every concurrent operation
that uses it.

No handle may be destroyed while another thread is using that handle or a
borrowed view whose lifetime depends on it. Destruction functions do not perform
reference counting, locking, handle registration, or synchronization on behalf
of the caller.

Borrowed views inherit the concurrency and lifetime contract of their owner:

- report finding/candidate views require the report to remain alive;
- batch entry/finding/candidate views require the batch results to remain alive;
- output string views require the output handle to remain alive;
- share-bundle entry views require the share bundle to remain alive;
- error message views require the error handle to remain alive.

The adapter does not expose Rayon or a thread-pool contract.
`CRIBRA_BATCH_EXECUTION_AUTO` may use the core parallel implementation when the
adapter is built with the optional `parallel` feature, while preserving the same
logical results and input ordering as serial execution.

## 28. ABI versioning

Cribra crate SemVer and the native ABI protocol version are distinct concepts.

A future native library should expose an explicit queryable ABI version without requiring string parsing. A likely form is separate major/minor/patch accessors or one version struct with fixed-width fields.

Compatibility policy:

```text
ABI major  -> breaking contract change
ABI minor  -> additive compatible change
ABI patch  -> compatible fix or clarification
```

The v0.3 ABI is experimental but compatibility-conscious. Cribra does not promise permanent ABI stability before a later explicit stability milestone.

A crate release does not automatically require an ABI version change, and an ABI change must not be inferred only from crate SemVer.

## 29. Symbol namespace

All public native symbols and public C type names MUST use the `cribra_` namespace.

The initial experimental ABI SHOULD NOT prematurely encode a permanent ABI major into every symbol name. If a future stable ABI requires parallel incompatible symbol generations, versioned symbols can be evaluated then.

## 30. Native artifact model

The future `cribra-capi` crate is expected to produce native artifacts suitable for static and dynamic linking, likely using Cargo crate types equivalent to:

```text
staticlib
cdylib
```

Expected platform forms include:

```text
macOS   libcribra.a / libcribra.dylib
Linux   libcribra.a / libcribra.so
Windows cribra.dll plus the appropriate import/static libraries
```

A generated public header is expected at a stable repository path such as:

```text
include/cribra.h
```

Exact packaging and release distribution are deferred beyond v0.3.2/v0.3.10.

## 31. Header generation

`cbindgen` is the preferred tool to evaluate for producing `include/cribra.h` from intentionally FFI-shaped Rust definitions.

Header generation must not accidentally expose internal Rust implementation details.

The public C contract, not incidental generator output, remains the compatibility authority.

The generated header is not considered valid merely because generation succeeds. A real C compiler must compile a consumer against the header and produced library.

## 32. Export hygiene

Where platform and toolchain behavior permit, native release validation should inspect exported symbols and verify that the intended public `cribra_*` surface is visible without unintentionally advertising implementation details as supported ABI.

Potential validation tools include `nm`, `objdump`/`readelf`, and Windows equivalents. The exact gate is deferred to v0.3.10.

## 33. Real C consumer validation

The C ABI is not considered validated by Rust tests alone.

A real C smoke consumer must compile and link using only the public header and native artifact. It must not depend on Cargo internals.

The minimum success-path scenario should cover:

1. query ABI version;
2. create a scanner using current built-ins;
3. scan known UTF-8 input;
4. inspect finding count;
5. inspect finding rule ID, coordinates, severity, confidence, and remediation metadata where present;
6. inspect candidate count and candidate metadata;
7. resolve explanation data;
8. execute at least one transform;
9. inspect transformed bytes;
10. destroy the transformed buffer;
11. destroy the report;
12. destroy the scanner.

Error-path coverage should include at least:

- invalid UTF-8;
- invalid index;
- invalid custom pattern;
- duplicate rule ID;
- invalid/null argument combinations that are defined as recoverable.

## 34. Cross-platform CI

Native ABI validation must run on at least:

- macOS;
- Linux;
- Windows.

The gate must exercise actual C compilation and linking against the produced artifact.

A successful `cargo build` is insufficient evidence of a usable C ABI.

## 35. Native memory validation

FFI hardening should use practical platform tooling where reliable, including as appropriate:

- AddressSanitizer;
- UndefinedBehaviorSanitizer where meaningful;
- leak detection;
- Valgrind on Linux as supplemental validation when useful.

The project should prefer deterministic, maintainable CI checks over adding fragile tooling for appearance alone.

## 36. ABI performance policy

Interop benchmarks must isolate boundary costs instead of reporting only one end-to-end number.

At minimum measure:

- a minimal ABI call;
- a report-count accessor;
- indexed finding traversal;
- complete report traversal;
- representative scan sizes;
- transform allocation/copy cost;
- batch amortization;
- equivalent Rust-native operations.

The performance goal is not mathematically zero overhead.

For medium and large scans, native boundary overhead should be immaterial relative to core scan cost. For tiny inputs, the batch API should provide an amortization path when call overhead becomes proportionally visible.

Correctness, privacy, and semantic parity take priority over micro-optimizations.

## 37. Existing WASM/PWA architecture

Cribra already runs correctly through WASM in the Silens Scan PWA. The serial core is deliberately suitable for WASM/PWA use, while optional Rayon parallelism is a native caller choice.

v0.3 therefore does NOT assume that WASM needs a rewrite.

The WASM work is:

1. integrate and validate Cribra v0.2 in the existing Silens Scan PWA;
2. treat successful integration as evidence that the current execution boundary remains stable;
3. audit the exposed surface against the richer v0.2 public capabilities;
4. remove unnecessary copies/serialization or awkward API projection only where evidence justifies the change;
5. preserve the direct WASM-to-core architecture.

WASM MUST NOT call the C ABI.

## 38. WASM audit topics

The later v0.3 WASM audit should inspect:

- which v0.2 capabilities are actually exposed;
- JS-to-WASM input copy/UTF-8 behavior;
- avoidable secondary copies;
- serialization overhead;
- batch ergonomics;
- error representation;
- TypeScript declarations;
- initialization ergonomics;
- Web Worker compatibility;
- bundle size;
- CSP/browser constraints;
- candidates;
- explanations;
- remediation;
- custom rules;
- transforms;
- source privacy.

JSON/Serde should not automatically become the transport for every operation merely because `serde` exists. The audit should prefer typed projection where it measurably improves ergonomics or cost while preserving the current working integration.

## 39. Web Worker and browser constraints

The WASM adapter should remain usable from a Web Worker without depending on DOM-only APIs.

The later audit should verify initialization, scan execution, result transfer, transform execution where exposed, and deterministic semantics in a worker context.

Browser deployment documentation should record actual CSP and WASM initialization requirements rather than assuming one bundler or framework.

Threaded WASM is not a v0.3 requirement.

## 40. Privacy boundary

Interop must preserve Cribra's privacy-first contract.

Across Rust, native ABI, and WASM:

- the application owns source I/O;
- Cribra operates on caller-provided UTF-8 text;
- public findings do not contain matched source values;
- ambiguous candidates do not contain matched source values;
- explanation data does not contain source snippets or matched source values;
- metadata serialization must not accidentally serialize original source material;
- transforms are explicit operations;
- adapters perform no network access;
- errors must not leak matched values or source snippets.

A wrapper that silently uploads, persists, logs, or exports source material is outside the Cribra core contract and must not be introduced by v0.3 adapters.

## 41. Threat model

### 41.1 Erroneous or hostile caller

The native adapter must consider:

- null pointers;
- invalid lengths;
- invalid UTF-8;
- invalid numeric enum values;
- out-of-range indexes;
- malformed custom rules;
- incompatible option combinations;
- source/report mismatch during transforms;
- resource exhaustion attempts.

The adapter should reject safely checkable invalid state before entering core operations.

### 41.2 Implementation failure

The adapter must consider:

- unexpected Rust panic;
- internal conversion/projection errors;
- allocation failure behavior within realistic Rust/platform limits;
- accidental sensitive-data inclusion in diagnostics;
- ownership-path leaks;
- inconsistent projection of Rust semantics.

### 41.3 Lifetime misuse

The contract must explicitly document:

- use-after-free;
- double-free;
- borrowed view after owner destruction;
- concurrent destruction while in use;
- forged or foreign handles.

These cannot all be made recoverable by an FFI wrapper and remain caller responsibility where memory cannot safely be inspected.

## 42. Cross-adapter semantic parity

Equivalent supported operations must preserve semantics across Rust, C ABI, and WASM/PWA adapters.

For equivalent scanner configuration and source, parity fixtures should compare, where the adapter exposes the capability:

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

The adapters should reuse canonical/golden fixtures where practical rather than maintain unrelated duplicated corpora.

## 43. Normative interoperability invariants

The following invariants govern v0.3 implementation:

**I1.** Cribra core remains the only detection authority.

**I2.** C ABI and WASM/PWA are independent adapters over the Rust core.

**I3.** No Rust object layout is part of the C ABI.

**I4.** Every FFI-owned allocation has exactly one documented destruction path.

**I5.** Borrowed views are valid only for the documented lifetime of their owner.

**I6.** Caller-provided source memory is never retained beyond the call unless explicitly documented by a future API.

**I7.** No sensitive matched value is returned through findings, candidates, errors, explanations, or metadata serialization.

**I8.** Native C input is length-delimited and explicitly UTF-8 validated.

**I9.** No Rust panic crosses an FFI boundary.

**I10.** ABI primitive representations are explicitly sized where required by the contract.

**I11.** Rust `bool` and Rust enum layouts never become ABI contracts.

**I12.** Report ordering preserves Rust semantic ordering.

**I13.** `SensitiveCandidate` remains semantically distinct from `Finding`.

**I14.** Explainability never becomes an independent classification authority.

**I15.** Transform operations require caller-provided original source.

**I16.** Rust-owned allocation is never freed with the caller allocator.

**I17.** No global mutable state is required for normal ABI operation.

**I18.** Scanner objects are immutable after successful construction.

**I19.** ABI protocol versioning is independent from crate SemVer.

**I20.** Rust/C/WASM must satisfy semantic parity fixtures where equivalent APIs exist.

**I21.** Neither interop adapter performs network access.

**I22.** Interoperability convenience must not alter detection semantics.

**I23.** The core crate's `forbid(unsafe_code)` boundary remains intact; required FFI `unsafe` belongs in the dedicated native adapter crate.

**I24.** Parallel execution strategy is not observable as a change in logical scan results or input ordering.

## 44. Planned v0.3 implementation sequence

This document is the design authority for the following roadmap:

```text
0.3.1  Interop specification, invariants, ownership, lifetime, threading,
       error model, panic containment, versioning, threat model

0.3.2  Workspace/root structure + cribra-capi skeleton + artifact model

0.3.3  ABI version API + builder/scanner lifecycle + single-source scan

0.3.4  Report/finding traversal ABI

0.3.5  Candidate + explainability ABI

0.3.6  Custom-rule/configuration builder ABI

0.3.7  Transform ABI + owned output buffers

0.3.8  Batch API + optional parallel execution contract

0.3.9  Ownership/error/panic/thread hardening

0.3.10 Generated C header + real C consumer + cross-platform CI +
       native memory validation

0.3.11 ABI performance benchmarks

0.3.12 Existing WASM boundary audit/refinement after v0.2 PWA validation

0.3.13 Rust/C/WASM semantic parity gate

0.3.14 Documentation/examples/packaging/release gate
```

## 45. Deferred implementation choices

The following remain intentionally open until the relevant implementation step provides evidence:

- exact C function names;
- exact numeric status/enum values;
- exact POD field ordering;
- exact explanation projection ownership;
- whether source length is retained in the native report wrapper;
- exact batch key type and batch output handle shape;
- exact mechanism for selecting serial versus parallel batch execution;
- exact workspace configuration at the root;
- exact `cbindgen` configuration;
- exact sanitizer matrix per platform;
- exact packaging/distribution format for prebuilt native libraries;
- whether the existing WASM surface needs structural API changes at all.

These choices are deferred deliberately. They must be resolved by measurement, consumer ergonomics, and the existing core contract rather than guessed in the architecture document.

## 46. Release principle

v0.3 succeeds when Cribra becomes substantially easier to embed without becoming a larger or less coherent detection engine.

Interop is successful only if it preserves the properties that make the core useful:

- local-first operation;
- privacy-first boundaries;
- deterministic semantics where possible;
- contextual detection where required;
- explainable classification;
- explicit ambiguity;
- safe transformations;
- small reusable core;
- no unnecessary coupling to UI, CLI, network, filesystem, or host-runtime policy.
