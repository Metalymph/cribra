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

Rust-owned objects include scanner, report, batch-result, transformed-output,
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

### Builder consumption

`cribra_builder_build` consumes a non-null builder on every build attempt.

After calling it, the caller MUST NOT reuse or free that builder independently,
regardless of whether scanner construction succeeds.

This consuming ownership rule is part of the public ABI contract.

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
SemVer.

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