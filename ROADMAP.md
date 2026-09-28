# Cribra Roadmap

Cribra is an application-agnostic, local-first secret and
sensitive-material detection engine.

The project prioritizes:

-   deterministic behavior;
-   high-confidence detection;
-   low false-positive rates;
-   exact source-span preservation;
-   explainable findings;
-   safe transformations;
-   semantic parity across supported interfaces;
-   no requirement to send source material out of process;
-   a small, stable public surface with minimal dependencies.

The roadmap deliberately favors correctness, compatibility, and
production reliability over catalog breadth.

## Stability and evolution policy

Cribra is intended to become a stable, battle-tested security component rather
than a continuously redesigned framework.

The existing core architecture and public contracts should remain unchanged
unless production evidence demonstrates a concrete limitation that cannot be
addressed safely within them. A cleaner abstraction, a more fashionable design,
or the possibility of greater generality is not by itself sufficient reason to
change the core.

Core evolution must therefore be evidence-driven:

- prefer additive changes over breaking changes;
- preserve established semantics and compatibility whenever practical;
- fix demonstrated limitations rather than redesigning speculative ones;
- require concrete use cases, correctness evidence, or operational constraints
  before introducing new core abstractions;
- keep experimental or application-specific policy outside the stable core;
- accept deliberate limitations when removing them would materially increase
  complexity, instability, or false-positive risk;
- treat accumulated production behavior, adversarial tests, and compatibility
  as assets that should not be discarded casually.

Breaking changes are exceptional. When one is genuinely necessary, the reason,
migration impact, and semantic benefit must be explicit and strong enough to
justify invalidating an established contract.

Cribra should mature primarily by becoming better tested, better validated, and
more reliable under real workloads, not by continuously expanding or
rearchitecting its core.

## Interface authority and parity

The Rust core is the semantic authority for detection, validation,
findings, candidates, explanations, queries, packs, and transformations.

Every public adapter is a projection of an authoritative lower-level
contract, never a second implementation of Cribra semantics:

-   the native C ABI and WebAssembly adapter maintain semantic parity
    with the applicable Rust core surface;
-   the canonical `cribra` CLI targets complete parity with every
    applicable public core capability;
-   native language bindings target complete parity with every
    applicable public `cribra-ffi` capability;
-   capabilities that are genuinely inapplicable to an adapter must be
    recorded explicitly as such rather than silently omitted.

Adapter ergonomics, packaging, acquisition, process policy, and
lifecycle may differ where required by the target environment, but
detection and classification semantics remain owned by Cribra.

## Release-line policy

Cribra's `0.4.x` line established the current detection, validation,
transformation, and interoperability foundation while preserving the original
whole-source scanning architecture and public semantic contracts.

The `0.5` line is the next architectural development line. Its defining change
is a bounded-memory streaming core for incremental source processing while
preserving Cribra's deterministic detection and validation semantics wherever
those semantics are applicable to streamed input.

CLI parity, native FFI parity, language bindings, packaging, distribution, and
developer integrations remain important project workstreams, but they do not
define the core release-line numbering. They may progress independently from
the architectural `0.5` work where their dependencies permit.

## Current architectural line

### v0.5 --- Streaming Core

Status: planned.

Goal: evolve Cribra from a whole-source UTF-8 scanning engine into an engine
that can process large or incrementally supplied sources with bounded memory,
without introducing a second detection implementation or weakening the
deterministic semantics of the existing scanner.

The existing whole-source API remains an important compatibility surface.
Streaming is an architectural extension of the authoritative Rust core, not a
separate scanner with independent detection semantics.

The v0.5 architecture is developed in ordered phases. Semantic equivalence and
explicit boundedness are established before public streaming APIs are frozen.

#### 0.5-A --- Streaming semantic contract

Status: design complete; implementation pending.

Goal: define the semantic invariants that every streaming implementation must
preserve before introducing chunked execution into the scanner.

The current whole-source scanner is the reference behavior for inputs whose
rules and validators are semantically applicable to streaming. Changing how
source bytes arrive must not silently change what Cribra detects or how an
accepted result is represented.

For a complete logical UTF-8 source `S`, a configured scanner `C`, and any
valid byte partition `P(S)`, streamed execution must be semantically equivalent
to whole-source execution:

```text
semantic_output(C.scan(S)) == semantic_output(C.stream(P(S)))
```

The notation above defines the architectural equivalence contract only. It does
not freeze a public streaming API or require the eventual API to expose
`stream`.

A valid partition may divide the source at any byte position. In particular,
transport reads may divide UTF-8 scalars, lines, tokens, matcher prefixes,
candidate values, contextual evidence, or logical matches. Partition placement
and chunk size are never part of Cribra's detection semantics.

Semantic output equivalence includes:

- confirmed findings and ambiguous `SensitiveCandidate` values;
- rule identity;
- severity and confidence;
- remediation;
- detection mode and explanation semantics;
- exact global half-open byte spans;
- one-based line and Unicode-scalar column coordinates;
- deterministic normalization and exact-span ownership;
- deterministic final ordering.

Required invariants:

-   [x] Define whole-source versus streamed equivalence for findings and
    sensitive candidates.
-   [x] Preserve rule identity, severity, confidence, remediation, detection
    mode, and explanation semantics independently of source partitioning.
-   [x] Preserve exact global half-open byte spans for accepted findings and
    candidates.
-   [x] Preserve one-based line and Unicode-scalar column coordinates relative
    to the complete logical source.
-   [x] Preserve deterministic finding and candidate ordering independently of
    chunk size and chunk boundaries.
-   [x] Preserve the existing deterministic normalization and exact-span
    ownership rules.
-   [x] Require a logical match crossing one or more chunk boundaries to be
    emitted exactly once by a conforming implementation.
-   [x] Require chunk-boundary placement to neither create an otherwise invalid
    finding nor suppress an otherwise valid finding.
-   [x] Preserve contextual-validator semantics when required evidence occurs
    before or after a candidate across chunk boundaries.
-   [x] Define UTF-8 transport semantics so reads may divide a multi-byte scalar
    while semantic processing still observes valid complete UTF-8.
-   [x] Distinguish transport/read boundaries from semantic source boundaries;
    callers are not required to align chunks to UTF-8 scalars, lines, tokens,
    matches, or validator context.
-   [x] Define end-of-stream as an explicit semantic boundary so pending
    matcher, boundary, and contextual state can be finalized deterministically.
-   [x] Define source-lifecycle isolation: completion, reset, or terminal
    failure must not permit retained state from one logical source to influence
    another source.
-   [x] Keep secret material caller-owned and avoid introducing public
    intermediate representations that expose matched values.
-   [x] Preserve the existing distinction between confirmed findings and
    ambiguous `SensitiveCandidate` values.
-   [x] Audit existing whole-source behavior for bounded-memory compatibility.
    No current matching or validation family has been identified as requiring
    inherently unbounded retained source material; implementation must preserve
    this conclusion or explicitly reclassify any discovered exception before
    claiming general streaming parity.

##### Retained-state boundedness contract

Streaming memory is bounded by semantic state, not by total logical-source
length and not by an arbitrary fixed overlap copied between chunks.

A conforming implementation may retain only state justified by the active
matching and validation semantics, including:

- incomplete UTF-8 transport bytes;
- incremental matcher state;
- bounded source history required by a matcher or validator;
- bounded lookahead required before a candidate can be finalized;
- unresolved bounded candidates and their required context;
- global byte and source-location accounting;
- pending normalization, collision, and deterministic-ordering state.

The architectural requirement is therefore that retained source-dependent state
does not grow as `O(total_source_length)` merely because the logical source is
large.

No implementation-wide overlap constant is part of the semantic contract.
Existing implementation window sizes or validator limits may inform retained
state, but they are not promoted to public streaming semantics.

Every matcher and validator family must have an explicit boundedness
classification before its streaming implementation is considered conforming.
The classification records, as applicable:

- maximum required history;
- maximum required lookahead;
- maximum unresolved candidate extent;
- dependence on a semantic boundary such as token, line, structural region, or
  end-of-stream;
- whether retained state can be represented incrementally instead of retaining
  the corresponding source text.

If implementation work discovers a rule or validator whose existing semantics
require potentially unbounded retained source material, that case must be made
explicit and redesigned, bounded, or classified as not yet stream-compatible.
It must not be hidden behind an arbitrary overlap size.

##### End-of-stream semantics

End-of-stream is a semantic event rather than an ordinary empty transport read.

A conforming streaming implementation must use end-of-stream to resolve any
state whose whole-source meaning depends on the absence of additional input.
This includes, where applicable:

- incomplete matcher candidates;
- trailing token or source boundaries;
- regex or captured-pattern matches requiring trailing evidence;
- contextual-validator evidence;
- pending normalization or ownership decisions.

A transport read boundary must never be interpreted as end-of-stream.

The exact public mechanism used to signal completion remains an API-design
decision for a later phase.

##### Source isolation and failure semantics

Streaming state belongs to exactly one logical source.

After successful completion, explicit reset, or terminal failure, state retained
for that source must not affect a subsequently scanned source. This includes
matcher state, UTF-8 carry, validator context, location accounting, unresolved
candidates, normalization state, and ordering state.

A terminal failure must not produce partially reused state through a later
source session. The concrete error and lifecycle API remains deliberately
unfrozen in this phase.

##### Conformance strategy

Whole-source execution remains the reference oracle while the streaming core is
introduced.

Streaming conformance tests must compare semantic output across adversarial
partitions rather than validate one preferred chunk size. Coverage must include,
where applicable:

- single-byte transport reads;
- boundaries at every byte position for bounded fixtures;
- splits inside multi-byte UTF-8 scalars;
- splits inside matcher prefixes and suffixes;
- splits inside captured values;
- splits between candidates and required contextual evidence;
- splits immediately before and after line, token, and structural boundaries;
- matches finalized only at end-of-stream;
- multiple chunk sizes and partition layouts producing identical final output.

The implementation may optimize this test space where exhaustive partitioning
would be impractical, but correctness must never depend on callers choosing a
particular chunk size.

Non-goals for this phase:

-   no public reader or chunk API;
-   no CLI streaming surface;
-   no C ABI, WebAssembly, or language-binding propagation;
-   no detector catalog expansion;
-   no parallelization of a single logical source;
-   no second streaming-specific rule or validator implementation.

Architectural constraints:

-   The compiled rule set remains the detection authority. Streaming may change
    execution strategy but must not fork rule semantics.
-   Validators remain authoritative for candidate acceptance. Streaming
    infrastructure supplies sufficient source context rather than weakening or
    duplicating validator policy.
-   Chunk boundaries are an implementation detail and must not become part of
    detection semantics.
-   Bounded memory must be demonstrated from explicit retained-state bounds,
    not inferred merely from processing the input in smaller reads.
-   Rules whose current semantics require potentially unbounded source material
    must be identified explicitly and redesigned, bounded, or classified before
    the streaming core can claim general bounded-memory behavior.
-   The existing `Scanner::scan` contract remains valid. v0.5 must not require
    existing callers with in-memory UTF-8 sources to adopt the streaming API.
-   No public streaming type, method naming, reader abstraction, callback
    model, synchronous/asynchronous policy, or chunk-size recommendation is
    frozen by this phase.

Acceptance gate:

-   [x] The streaming equivalence contract is documented precisely enough that
    implementation tests can determine pass or fail without relying on
    implementation-specific chunk sizes.
-   [x] Every current matching and validation family can be classified by the
    source history, lookahead, candidate extent, and semantic boundaries
    required to preserve its behavior.
-   [x] Unbounded or not-yet-bounded cases must be explicit rather than hidden
    behind an arbitrary overlap size; the current audit identified no
    inherently unbounded retained-source requirement in the existing
    portfolio.
-   [x] No public API has been frozen before these invariants and
    classifications were completed.

Design outcome:

- the whole-source scanner remains the semantic oracle for v0.5;
- source partitioning is semantically invisible;
- boundedness is a property of explicit retained semantic state rather than a
  fixed chunk overlap;
- end-of-stream is an explicit semantic boundary;
- streaming state is isolated per logical source;
- the current matcher and validator portfolio has a viable bounded-memory
  streaming model;
- public streaming API design remains intentionally deferred until the
  implementation architecture is specified.

#### 0.5-B --- Streaming execution architecture

Status: design complete; implementation pending.

Goal: define the internal execution architecture that can satisfy the streaming
semantic contract from 0.5-A with bounded retained state, without creating a
second detection engine or freezing the public streaming API prematurely.

The existing compiled rule set remains immutable scanner configuration.
Streaming introduces source-local execution state around that compiled
configuration rather than moving mutable stream state into `CompiledRuleSet`.

The architectural separation is:

```text
Scanner
  |
  +-- CompiledRuleSet              immutable detection authority
  |
  +-- source execution session     mutable state for one logical source
        |
        +-- UTF-8 transport state
        +-- global source-position state
        +-- matcher execution state
        +-- validator/context state
        +-- unresolved candidate state
        +-- normalization/finalization state
```

The exact internal type names are intentionally not frozen by this roadmap.
The implementation may split or combine these responsibilities where doing so
improves clarity or performance, provided their ownership and lifecycle remain
explicit.

Required architecture:

-   [x] Keep `CompiledRuleSet` immutable after scanner construction.
-   [x] Keep rule metadata and compiled matching configuration shared across
    source sessions.
-   [x] Introduce mutable execution state scoped to exactly one logical source.
-   [x] Separate transport chunk boundaries from semantic processing
    boundaries.
-   [x] Decode arbitrarily partitioned UTF-8 transport input without requiring
    callers to provide scalar-aligned chunks.
-   [x] Track global byte offsets, line numbers, and Unicode-scalar columns
    incrementally.
-   [x] Represent matcher continuation explicitly where a logical candidate may
    cross transport chunks.
-   [x] Retain only bounded history and lookahead justified by matcher or
    validator semantics.
-   [x] Keep unresolved candidates explicit until sufficient evidence exists to
    accept, reject, or classify them.
-   [x] Delay externally observable emission until the result is semantically
    final with respect to later source input.
-   [x] Preserve exact-span ownership and deterministic normalization across
    incrementally discovered candidates.
-   [x] Treat end-of-stream as an explicit finalization event.
-   [x] Make successful completion, reset, and terminal failure destroy or
    invalidate all source-local execution state.
-   [x] Preserve `Scanner::scan` as a whole-source compatibility path using the
    same semantic authority as streamed execution.

##### Immutable compiled configuration

`CompiledRuleSet` continues to own the scanner's immutable rule metadata and
compiled matching configuration.

Source-local mutable state must not be stored in a way that makes the compiled
rule set itself specific to one active source. A scanner must remain capable of
creating independent source executions from the same compiled configuration.

The current internal execution groups remain useful architectural inputs:

- literal and prefix rules use shared multi-pattern matching;
- suffix rules have reverse token-boundary semantics;
- pattern rules use compiled regular expressions;
- contextual pattern rules may use rule-specific prefilters;
- the shared contextual-pattern gate avoids unnecessary rule execution.

v0.5 may replace or specialize the implementation of any of these groups where
incremental execution requires it. Their current concrete representation is not
a compatibility contract.

What remains authoritative is the rule behavior observed through the semantic
contract, not the particular whole-source algorithm currently used to produce
it.

##### Source execution state

One execution session owns all mutable state required to process one logical
source.

At minimum, the architecture must account explicitly for:

- incomplete UTF-8 transport bytes;
- total accepted source-byte position;
- current one-based line and Unicode-scalar column state;
- matcher continuation state;
- bounded history needed to determine leading boundaries;
- bounded lookahead needed to determine trailing boundaries;
- unresolved matches or captured spans;
- validator-specific contextual evidence;
- pending `Finding` and `SensitiveCandidate` classification;
- normalization and exact-span collision state;
- deterministic output-order state;
- end-of-stream and terminal lifecycle state.

The implementation should prefer compact semantic state over retaining source
text. Source bytes may be retained only when a matcher or validator genuinely
requires those bytes to preserve existing semantics.

No state belonging to one logical source may be reused implicitly by another.

##### UTF-8 transport and source coordinates

Transport input is a byte stream. Semantic matching continues to operate on
valid UTF-8 text.

A chunk may terminate after any byte, including inside a multi-byte UTF-8
scalar. The execution session therefore owns a small UTF-8 carry containing
only the incomplete trailing scalar bytes required to join the next transport
chunk.

Complete valid UTF-8 is then presented to semantic execution independently of
the original transport partition.

Invalid UTF-8 handling must remain deterministic and explicit. Streaming must
not silently replace malformed input, reinterpret bytes, or make validity
depend on chunk placement.

Global source coordinates are maintained incrementally:

- byte spans are offsets in the complete logical source;
- lines remain one-based;
- columns remain one-based Unicode-scalar positions;
- a scalar divided across transport reads advances the column exactly once;
- newline handling must produce the same location as whole-source execution.

Transport-local offsets must never escape into final findings or candidates.

##### Matcher continuation model

Each matching family requires an incremental strategy derived from its actual
semantics rather than from a common copied overlap window.

Literal and prefix matching may preserve automaton continuation state across
input segments. Prefix findings additionally require enough boundary state to
establish the leading token boundary and must remain unresolved while the token
can still extend.

Suffix matching requires enough token history to recover the candidate start
when the suffix is recognized and enough trailing information to establish the
suffix boundary.

Pattern matching must use a streaming strategy whose retained state is derived
from the pattern's bounded language requirements. Existing built-in patterns
already impose bounded candidate or contextual extents where required by the
0.5-A audit.

Captured-pattern execution must preserve the exact projected capture span even
when the complete regex match and its capture cross different transport
chunks.

A matcher result is therefore not necessarily a finalized finding. Discovery
and finalization are separate execution events.

##### Validator and contextual state

Validators remain the authority for accepting matcher candidates.

Streaming infrastructure must provide each validator with semantically
equivalent evidence to the whole-source scanner. It must not weaken validation
because some evidence arrived in an earlier or later transport chunk.

Validator execution may therefore use:

- the candidate span and candidate bytes while unresolved;
- bounded source history;
- bounded lookahead;
- compact parser-like or structural state;
- explicit semantic-boundary state;
- end-of-stream when absence of further evidence is significant.

Where contextual evidence can be represented incrementally, the implementation
should retain that semantic state instead of the complete source region.

Validator-specific retained-state bounds belong to internal implementation
contracts and tests. They are not public chunk-size requirements.

##### Finalization frontier

Streaming execution needs an explicit notion of which portion of the logical
source can no longer be affected by future input.

Call this architectural concept the finalization frontier. The name does not
require a public type or API.

A candidate may cross the frontier only when later bytes can no longer:

- extend or invalidate its matcher span;
- change a required token, line, or structural boundary;
- provide validator evidence that changes acceptance or rejection;
- alter exact-span ownership;
- create a higher-priority collision affecting normalization;
- change whether the result is a confirmed finding or an ambiguous candidate;
- change deterministic ordering relative to another unresolved result.

Once all semantic decisions affecting a result are final, source bytes retained
solely for that result may be released.

The frontier may advance at different rates for different matching or
validation families. The implementation must not force all rules to retain the
largest possible common history if their semantics allow earlier release.

End-of-stream advances the frontier through all remaining resolvable state.

##### Incremental normalization and ordering

Whole-source execution currently discovers internal findings before applying
validation, exact-span ownership, normalization, and deterministic result
ordering.

Streaming must preserve the resulting semantics without requiring all source
text or all raw matcher candidates to remain resident until end-of-stream.

Pending results may therefore remain buffered while another unresolved
candidate can still affect their ownership or ordering.

Once a result lies behind the finalization frontier and no unresolved candidate
can alter its semantic outcome, it may be committed to finalized result state.

Exact-span collisions continue to use the existing rule-priority semantics.
Provider-specific validated rules must not lose ownership merely because a
generic candidate happened to be discovered or finalized from a different
transport chunk.

Final ordering must be identical to whole-source ordering for the same logical
source regardless of partition layout.

The implementation may initially retain finalized metadata until source
completion if required by the existing report API. Bounded-memory claims apply
to retained source-dependent material; compact result metadata necessarily
scales with the number of reportable results unless a later public incremental
result API defines different ownership.

##### SensitiveCandidate integration

`SensitiveCandidate` remains a first-class semantic output and is not treated
as an implementation fallback for streaming uncertainty.

Temporary uncertainty caused only by incomplete input remains internal pending
state.

A pending matcher or validator candidate becomes a public
`SensitiveCandidate` only when the same complete logical source would produce
that candidate under whole-source semantics.

Chunk boundaries must therefore never create additional ambiguous candidates.

##### End-of-stream and lifecycle

End-of-stream performs semantic finalization, not merely buffer flushing.

Completion must:

1. resolve or reject incomplete UTF-8 according to the defined input-validity
   contract;
2. finalize matcher state that depends on source termination;
3. provide end-of-source evidence to validators that require it;
4. resolve remaining normalization and exact-span ownership decisions;
5. produce deterministic final result ordering;
6. transition the source session into a completed state from which additional
   input cannot be accepted accidentally.

Reset abandons all unresolved state and returns execution to a clean
source-independent state.

Terminal failure invalidates the current source session. Reusing partially
processed matcher, validator, location, or normalization state after terminal
failure is forbidden.

The concrete public methods and error types implementing these lifecycle events
remain deferred.

##### Whole-source compatibility path

The existing whole-source API remains supported.

Architecturally, whole-source and streamed execution must converge on the same
semantic machinery rather than evolve as independent scanners.

The implementation may migrate `Scanner::scan_source` and related whole-source
paths onto the new execution session once parity is demonstrated. It may also
retain optimized whole-buffer entry paths where they reuse the same compiled
rules and semantic decisions.

What is forbidden is maintaining two independently evolving definitions of
matching, validation, normalization, or ownership.

##### Implementation sequence

The internal streaming core should be introduced in narrow, testable slices:

1. source-session lifecycle and global position accounting;
2. UTF-8 transport carry and adversarial partition tests;
3. literal and prefix incremental execution;
4. suffix incremental execution;
5. deterministic pattern execution with bounded retained state;
6. captured-pattern projection;
7. contextual prefilter and validator integration;
8. incremental finalization and normalization;
9. `SensitiveCandidate` parity;
10. whole-source compatibility integration;
11. complete cross-family partition-equivalence suite.

A later slice may reorder implementation details where dependencies require it,
but semantic parity must be demonstrated after each matching family is moved
onto the streaming path.

##### Memory model

The primary bounded-memory requirement is:

```text
retained_source_state =
    utf8_carry
  + matcher_state
  + bounded_history
  + bounded_lookahead
  + unresolved_candidate_material
  + validator_context
  + normalization_pending_state
```

None of those source-dependent components may grow merely because the logical
source continues indefinitely.

Finalized report metadata may grow with the number of findings and candidates
required by the existing report-returning API. That growth is distinct from
retaining the scanned source and does not justify retaining matched secret
material.

A future incremental-consumer API may permit finalized metadata to be handed
off earlier, but such an API is not required to establish the internal
streaming architecture.

Non-goals for this phase:

-   no public streaming API design;
-   no `Read`, async-reader, iterator, callback, or channel commitment;
-   no CLI streaming interface;
-   no C ABI, WebAssembly, Swift, Kotlin, Python, or other binding surface;
-   no source-level parallelism within one logical source;
-   no detector-catalog expansion;
-   no arbitrary fixed chunk overlap presented as the streaming architecture;
-   no weakening of validators to simplify incremental execution;
-   no requirement to expose internal pending candidates or retained context.

Architectural constraints:

-   One immutable compiled rule configuration may serve multiple independent
    source executions.
-   Mutable execution state is source-local.
-   Source bytes remain caller-owned except for bounded internal material whose
    retention is semantically required.
-   Internal retained secret material must be released as soon as its semantic
    dependency is finalized.
-   Chunk size and chunk placement must not affect semantic output.
-   The implementation must be explainable in terms of explicit state bounds.
-   Existing rule identifiers, metadata, validation authority, normalization,
    and result semantics remain authoritative.
-   Performance optimizations must preserve the 0.5-A equivalence contract.

Acceptance gate:

-   [x] Immutable compiled configuration and mutable per-source execution state
    have distinct ownership.
-   [x] UTF-8 carry and global coordinate accounting have an explicit
    partition-independent model.
-   [x] Every matcher family has an incremental continuation strategy without
    relying on one implementation-wide overlap constant.
-   [x] Validator context can be supplied from bounded retained or incremental
    semantic state.
-   [x] Discovery, validation, normalization, and finalization are modeled as
    distinct concerns.
-   [x] A finalization frontier defines when later input can no longer change a
    result.
-   [x] `SensitiveCandidate` semantics distinguish genuine source ambiguity from
    temporary streaming incompleteness.
-   [x] End-of-stream, reset, completion, and terminal failure have explicit
    architectural semantics.
-   [x] Whole-source compatibility converges on the same semantic authority.
-   [x] Bounded-memory claims distinguish source-dependent retained material
    from compact finalized report metadata.
-   [x] No public streaming API has been frozen by this phase.

Design outcome:

- `CompiledRuleSet` remains immutable scanner configuration;
- one logical source owns one isolated mutable execution session;
- transport partitioning is absorbed below semantic matching;
- UTF-8 and global location accounting are incremental and partition-independent;
- matcher and validator continuation uses explicit bounded semantic state;
- discovery does not imply immediate emission;
- the finalization frontier determines when results and retained source material
  become irrevocable;
- normalization and exact-span ownership remain deterministic across chunks;
- temporary streaming uncertainty never leaks as a `SensitiveCandidate`;
- whole-source scanning remains compatible and converges on the same semantic
  engine;
- the architecture is sufficiently specified to begin implementation without
  prematurely committing Cribra to a public streaming API shape.

#### 0.5-C --- Public streaming API contract

Status: design complete; implementation pending.

Goal: define the public ownership, lifecycle, input, completion, error, and
compatibility contract for streaming scans without exposing internal matcher
state or coupling Cribra to a particular I/O or asynchronous runtime.

The public streaming model follows directly from the semantic contract in 0.5-A
and the execution architecture in 0.5-B:

```text
immutable Scanner
      |
      +-- create source-local streaming session
                    |
                    +-- accept arbitrary byte chunks
                    +-- retain bounded semantic state
                    +-- explicit end-of-stream finalization
                    |
                    +-- ScanReport
```

The exact public type and method names remain implementation decisions until the
first implementation slice demonstrates that the contract can be expressed
without unnecessary complexity.

The architectural API shape, however, is fixed by this phase.

Required public contract:

-   [x] Keep `Scanner` immutable and reusable across whole-source and streaming
    scans.
-   [x] Represent one active streamed logical source with a separate
    source-local session object.
-   [x] Accept transport input as bytes rather than requiring every individual
    chunk to be valid `&str`.
-   [x] Permit transport chunks to end at arbitrary byte positions, including
    inside a multi-byte UTF-8 scalar.
-   [x] Make end-of-stream an explicit operation rather than inferring it from
    an empty chunk.
-   [x] Make successful finalization consume or otherwise irreversibly complete
    the source session.
-   [x] Prevent additional input after successful completion.
-   [x] Treat terminal input or execution errors as terminating the current
    logical-source session.
-   [x] Return the same semantic `ScanReport` produced by whole-source scanning
    for the equivalent complete UTF-8 source.
-   [x] Keep transport adapters such as `std::io::Read` above the primitive
    byte-oriented streaming contract.
-   [x] Keep asynchronous runtime integration outside the core streaming
    contract.
-   [x] Avoid exposing matcher, validator, normalization, or retained-source
    internals through the public API.
-   [x] Avoid requiring callers to choose a semantically significant chunk
    size.
-   [x] Keep chunk memory caller-owned except for bounded material that Cribra
    must retain to preserve detection semantics.
-   [x] Do not require a public reset operation for the initial streaming API;
    a new logical source uses a new source session.

##### Scanner and session ownership

`Scanner` remains immutable scanner configuration.

Creating a streaming execution must not move, mutate, or otherwise make the
scanner unavailable for additional scans. Multiple independent source sessions
may therefore originate from the same scanner configuration.

Conceptually:

```rust
let scanner = Scanner::default();

let mut first = scanner.stream();
let mut second = scanner.stream();
```

The notation illustrates ownership only. It does not freeze `stream` as the
eventual public method name.

Each session owns the mutable state for exactly one logical source. It may
borrow immutable scanner configuration or share it through the scanner's
existing compiled-rule ownership model.

The public contract must permit independent sessions to coexist without their
source-local state influencing one another.

Whether a particular session type is `Send` or `Sync` must follow from its
actual implementation and safety properties rather than being promised by the
roadmap. The immutable scanner should retain its existing ability to be reused
across independent scans.

##### Primitive input contract

The primitive streaming operation accepts a borrowed byte slice representing
the next transport fragment of the same logical source.

Conceptually:

```rust
session.push(bytes)?;
```

The exact method name remains unfrozen.

`&[u8]` is the primitive input rather than `&str` because transport boundaries
are not semantic UTF-8 boundaries. Requiring every chunk to be independently
valid UTF-8 would contradict the partition invariance established in 0.5-A.

A call may contain:

- zero bytes;
- one byte;
- one or more complete UTF-8 scalars;
- a complete logical source;
- the beginning or end of a multi-byte scalar;
- bytes completing a scalar begun by an earlier call;
- arbitrary divisions of matcher, token, line, capture, or contextual regions.

An empty input chunk is a no-op transport event. It does not represent
end-of-stream and must not cause pending semantic state to finalize.

No minimum, preferred, or correctness-critical chunk size is part of the public
contract.

Implementations and documentation may recommend chunk sizes for throughput, but
changing the chunk size must not change semantic output.

##### Borrowing and retained input

Passing a byte slice does not transfer ownership of the caller's input buffer to
Cribra.

After the input operation returns, the caller may reuse, mutate, or release its
buffer according to normal Rust borrowing rules.

The streaming implementation may internally copy only material that must survive
the call in order to preserve semantics, such as:

- incomplete UTF-8 bytes;
- bounded matcher history;
- unresolved candidate material;
- bounded validator evidence;
- other explicitly classified source-dependent state from 0.5-A and 0.5-B.

The API does not promise zero-copy processing across calls. Such a promise would
conflict with the need to retain bounded fragments whose lifetime exceeds the
borrowed input slice.

Likewise, the API must not require retaining caller-owned chunks until
end-of-stream.

##### Completion contract

End-of-stream is explicit.

Conceptually:

```rust
let report = session.finish()?;
```

The preferred Rust ownership model is consuming finalization:

```rust
fn finish(self) -> Result<ScanReport, StreamError>;
```

The names and concrete error type shown above are illustrative, but the
ownership property is architectural: successful completion must make accidental
continued use of the same source execution impossible or structurally
unrepresentable.

Completion performs all semantic work defined for end-of-stream in 0.5-A and
0.5-B, including:

1. resolving any pending UTF-8 transport state;
2. finalizing matcher state that depends on source termination;
3. resolving pending token, line, and structural boundaries;
4. providing end-of-source evidence to validators;
5. resolving remaining findings and `SensitiveCandidate` values;
6. completing exact-span ownership and deterministic normalization;
7. producing deterministic final ordering;
8. materializing the final `ScanReport`.

An empty logical source is valid:

```text
create session
finish session
```

Its report must be semantically equivalent to whole-source scanning of the empty
UTF-8 string.

##### UTF-8 validity and errors

Transport chunks are arbitrary bytes, but the complete logical source scanned by
the UTF-8 scanner must satisfy the scanner's UTF-8 input contract.

A byte sequence split across calls is not invalid merely because an intermediate
chunk ends inside a UTF-8 scalar.

The implementation must distinguish:

```text
temporarily incomplete UTF-8
```

from:

```text
definitively invalid UTF-8
```

Temporarily incomplete trailing bytes remain pending until additional input or
end-of-stream.

A definitively malformed sequence produces a deterministic terminal input error.

If end-of-stream occurs while an incomplete scalar remains pending, completion
fails deterministically rather than replacing, dropping, or reinterpretating
the bytes.

Chunk partitioning must not change whether a complete byte sequence is accepted
as valid UTF-8.

The public error surface must not expose retained secret material or include
matched source contents merely to explain the failure.

##### Terminal failure semantics

An unrecoverable streaming error terminates the current logical-source
execution.

After terminal failure, the session must not silently resume using partially
retained:

- UTF-8 state;
- matcher state;
- validator state;
- unresolved candidates;
- source coordinates;
- normalization state.

The initial API does not provide recovery by resetting a failed session.

The caller creates a new session for a new logical source.

This intentionally keeps source isolation structural and avoids a public
lifecycle in which callers must know which internal state is safe to reset.

##### Report equivalence

The initial public streaming API is report-oriented.

For every valid complete logical UTF-8 source `S` and every valid partition
`P(S)`:

```text
semantic_output(scanner.scan(S))
    ==
semantic_output(scanner.stream(P(S)).finish())
```

The notation is architectural rather than literal Rust syntax.

Streaming therefore returns the existing semantic result model rather than
introducing a second report type.

The resulting `ScanReport` must preserve:

- findings;
- `SensitiveCandidate` values;
- rule identifiers;
- severity;
- confidence;
- remediation;
- exact global byte spans;
- one-based line and Unicode-scalar columns;
- normalization and ownership decisions;
- deterministic ordering.

Source length recorded by higher-level identified-source APIs must represent the
complete logical source length, not the size of the final transport chunk.

##### Result emission policy

The first public streaming surface is intentionally completion-oriented.

It does not require findings to be emitted incrementally while chunks are being
accepted.

This separates two concerns:

```text
bounded-memory source processing
```

from:

```text
incremental result delivery
```

They are related but not equivalent.

A scanner can process source material with bounded retained source state while
still returning one final compact `ScanReport`.

Introducing callbacks, iterators, channels, polling, event streams, or
backpressure before the finalization semantics are proven would unnecessarily
expand the first public contract.

A later API may expose results that have crossed the finalization frontier
defined in 0.5-B, but doing so must preserve the same ownership,
normalization, ordering, and security guarantees.

##### `std::io::Read` integration

`std::io::Read` is an adapter opportunity, not the primitive semantic API.

A convenience operation may repeatedly:

1. read bytes into a caller- or Cribra-owned bounded buffer;
2. submit the bytes to the primitive source session;
3. signal completion after the reader reaches genuine EOF;
4. return the final `ScanReport`.

Conceptually:

```text
Read
  |
bounded buffer
  |
streaming session
  |
ScanReport
```

Reader buffer size is therefore a throughput choice, not a detection-semantic
choice.

I/O failures and scanner failures must remain distinguishable enough for callers
to handle them correctly without exposing source contents.

The exact adapter shape is deferred until the primitive session API exists.

##### Async integration

The core streaming scanner remains runtime-independent.

The primitive operation accepts already-available bytes and performs scanner
work synchronously. It does not depend on Tokio, async-std, futures executors,
or another asynchronous runtime.

Async applications can perform asynchronous I/O externally and submit completed
byte buffers to the same streaming session:

```text
async transport
      |
      | await read
      v
available bytes
      |
      v
Cribra streaming session
```

This keeps detection semantics independent of scheduling and avoids introducing
a runtime dependency into the core library.

A future optional async adapter may improve ergonomics, but it must remain an
adapter over the same semantic engine.

##### Concurrency contract

One source session represents one sequential logical byte stream.

Calls contributing bytes to that session have a defined source order and must
not be interpreted as independently reorderable work items.

Parallelism across independent logical sources remains compatible with the
architecture:

```text
Scanner
  |
  +-- source A session
  +-- source B session
  +-- source C session
```

The implementation may later optimize work inside one session where doing so
preserves deterministic stream order and semantics, but the public API does not
require or expose intra-source parallel execution.

The existing whole-source parallel scanning model remains a separate
higher-level capability.

##### Identified-source integration

The current `Scanner::scan` API associates each complete source with a
caller-owned key and returns `ScanResults<K>` preserving input order.

Streaming must not force source identity into the low-level matcher state.

The primitive source session scans one logical source and produces one
`ScanReport`.

Higher-level APIs may associate a key, path, identifier, or source length with
that completed report without changing detection semantics.

This preserves the current separation:

```text
source identity / collection orchestration
                |
                v
         per-source scanning
                |
                v
           ScanReport
```

and avoids coupling the streaming core to filesystem paths, network resources,
or application-specific identifiers.

##### Security properties

The public streaming surface must preserve Cribra's existing secret-handling
direction.

In particular:

- input buffers remain caller-owned;
- source contents are not exposed through streaming-state inspection;
- internal pending candidates are not public findings;
- error messages must not include matched secret values;
- debug representations of public streaming state must not expose retained
  source material;
- dropping a session must not cause retained source material to become publicly
  observable;
- transport adapters must not introduce logging of scanned chunks;
- no public intermediate representation may require callers to handle raw
  matched secret values.

Memory clearing guarantees, if introduced, must be explicit and technically
enforceable rather than implied merely because a session has been dropped.

##### API evolution constraints

The first implementation should expose the smallest surface capable of
expressing the contract.

The likely minimum conceptual operations are:

```text
Scanner
  -> create source session

source session
  -> accept bytes
  -> finish into ScanReport
```

This phase deliberately does not freeze:

- exact type names;
- exact method names;
- concrete streaming error names or variants;
- a `Read` convenience method;
- async adapters;
- callbacks;
- incremental result iterators;
- event streams;
- result channels;
- user-configurable chunk sizes;
- explicit reset;
- pause/resume persistence;
- serialization of active source state.

Those features may be evaluated only after the primitive session implementation
demonstrates semantic parity.

##### Implementation sequence

The public API should be introduced only after the corresponding internal
execution slices from 0.5-B exist.

Recommended sequence:

1. implement an internal source session;
2. prove arbitrary byte-partition and UTF-8 carry behavior;
3. migrate matching families incrementally;
4. prove validator and normalization parity;
5. prove `SensitiveCandidate` parity;
6. prove end-of-stream behavior;
7. expose the minimal public byte-oriented session;
8. add compile-time and runtime lifecycle tests;
9. integrate whole-source compatibility where appropriate;
10. evaluate `Read` convenience only after the primitive API is stable.

This prevents public API pressure from dictating an incorrect internal
architecture.

##### Required conformance tests

The public streaming contract requires tests covering at least:

- empty logical source;
- one chunk containing the complete source;
- one-byte chunks;
- empty chunks interspersed with non-empty chunks;
- every byte boundary for bounded fixtures;
- UTF-8 scalars divided across calls;
- incomplete UTF-8 completed by a later call;
- invalid UTF-8 detected independently of partition layout;
- incomplete UTF-8 at end-of-stream;
- matches crossing one or multiple chunk boundaries;
- contextual evidence crossing chunk boundaries;
- captured spans crossing chunk boundaries;
- findings finalized only at end-of-stream;
- `SensitiveCandidate` parity;
- exact-span ownership parity;
- deterministic ordering parity;
- multiple independent sessions created from one scanner;
- dropping an unfinished session without affecting later sessions;
- terminal failure followed by creation of a clean new session;
- equivalence between whole-source and streamed reports.

Where lifecycle invalidity can be made impossible through Rust ownership, a
compile-fail test or equivalent API-level proof is preferable to a runtime
state check.

##### Non-goals for this phase

- freezing exact public type and method names before implementation validates
  them;
- asynchronous I/O in the core scanner;
- choosing an async runtime;
- making filesystem or network I/O part of detection semantics;
- exposing internal matcher or validator state;
- exposing partially validated candidates;
- promising zero-copy processing across calls;
- requiring caller-managed overlap buffers;
- introducing a public reset protocol;
- serializing or resuming active stream sessions;
- incremental public finding delivery;
- changing `ScanReport` semantics;
- changing existing whole-source `Scanner::scan` behavior.

##### Acceptance gate

-   [x] The ownership relationship between immutable scanner configuration and
    mutable per-source execution is explicit.
-   [x] The primitive input is byte-oriented and compatible with arbitrary
    transport partitioning.
-   [x] Empty chunks and end-of-stream have distinct semantics.
-   [x] Successful completion has an irreversible lifecycle transition.
-   [x] Terminal failure cannot permit state reuse across logical sources.
-   [x] UTF-8 validity is independent of transport partitioning.
-   [x] Input-buffer ownership and retained-data expectations are explicit.
-   [x] The initial output remains the existing `ScanReport`.
-   [x] Whole-source versus streaming report equivalence is explicit.
-   [x] `Read` and async I/O are adapters rather than semantic authorities.
-   [x] No async runtime dependency is required by the core contract.
-   [x] Incremental result delivery is explicitly separated from bounded-memory
    source processing.
-   [x] The initial API does not require reset, callbacks, channels, or
    caller-managed overlap.
-   [x] Security constraints prevent the streaming API from becoming a new
    secret-exposure surface.
-   [x] Exact public naming remains deferred until implementation validates the
    minimal surface.

Design outcome:

- `Scanner` remains immutable, reusable compiled configuration;
- each streamed logical source has isolated mutable execution state;
- arbitrary `&[u8]` transport fragments are the primitive input model;
- UTF-8 reconstruction belongs to the source session rather than the caller;
- end-of-stream is explicit and finalization is irreversible;
- the first streaming API remains report-oriented;
- whole-source and streamed execution share one semantic authority;
- I/O adapters remain layered above the core;
- async runtime policy remains outside Cribra's detection engine;
- public API size remains deliberately minimal until implementation proves the
  architecture.

#### 0.5-D --- Implementation and conformance gate

Status: design complete; implementation pending.

Goal: turn the semantic, execution, and public API contracts defined by 0.5-A
through 0.5-C into an ordered implementation plan with explicit conformance
gates, so the streaming core can be introduced incrementally without weakening
whole-source semantics, bounded-memory guarantees, source isolation, or public
compatibility.

This phase does not introduce a fourth definition of streaming behavior.
0.5-A remains the semantic authority, 0.5-B defines the internal execution
architecture, and 0.5-C defines the public ownership and lifecycle contract.

0.5-D defines how those contracts are implemented and proven.

The implementation rule is:

```text
implement one semantic slice
        |
        v
prove whole-source parity
        |
        v
prove partition invariance
        |
        v
prove retained-state bounds
        |
        v
advance to the next slice
```

A later slice must not compensate for an unresolved semantic defect in an
earlier slice.

The implementation may refine internal type names, module boundaries, and local
algorithms where required by evidence from the code, but any change to the
semantic or architectural contracts established in 0.5-A through 0.5-C must be
made explicitly in the roadmap rather than introduced accidentally during
implementation.

##### 0.5-D1 --- Source-session foundation

Goal: introduce the internal lifecycle and state ownership required for one
streamed logical source without yet requiring every matcher family to execute
incrementally.

Required work:

-   [x] Introduce an internal source-local execution session owned separately
    from immutable scanner configuration.
-   [x] Keep `CompiledRuleSet` and rule metadata immutable and reusable across
    independent sessions.
-   [x] Represent source lifecycle explicitly, including active, successfully
    completed, and terminally failed execution.
-   [x] Track total accepted source bytes independently of individual transport
    chunk sizes.
-   [x] Establish source-local ownership for matcher, validator, candidate,
    normalization, and location state.
-   [x] Permit multiple independent source sessions to originate from one
    scanner configuration.
-   [x] Ensure dropping an unfinished session cannot affect later scans or
    another active session.
-   [x] Keep all new execution machinery internal until the primitive semantics
    are proven.

Acceptance gate:

-   [x] Two or more independent sessions can coexist without state leakage.
-   [x] Completion, abandonment, and terminal failure cannot cause state from
    one logical source to influence another.
-   [x] No mutable source state has been moved into shared compiled scanner
    configuration.
-   [x] Existing whole-source behavior remains unchanged.

##### 0.5-D2 --- UTF-8 transport and source coordinates

Goal: make arbitrary byte partitioning semantically invisible before detection
families are migrated onto incremental execution.

Required work:

-   [ ] Accept arbitrary byte fragments internally.
-   [ ] Retain only incomplete trailing UTF-8 bytes required to reconstruct a
    scalar divided across transport calls.
-   [ ] Distinguish temporarily incomplete UTF-8 from definitively malformed
    UTF-8.
-   [ ] Reject malformed complete input deterministically and independently of
    partition layout.
-   [ ] Reject incomplete UTF-8 deterministically at end-of-stream.
-   [ ] Track global byte position incrementally.
-   [ ] Track one-based line numbers incrementally.
-   [ ] Track one-based Unicode-scalar columns incrementally.
-   [ ] Ensure a scalar divided across chunks advances source coordinates
    exactly once.
-   [ ] Treat empty chunks as transport no-ops rather than semantic completion.

Acceptance gate:

-   [ ] Valid UTF-8 has identical validity under every tested partition.
-   [ ] Invalid UTF-8 has identical failure semantics under every tested
    partition.
-   [ ] Single-byte transport reads preserve source coordinates.
-   [ ] Splits inside multi-byte scalars preserve source coordinates.
-   [ ] Empty chunks cannot advance coordinates or finalize pending state.
-   [ ] UTF-8 carry remains bounded independently of total source length.

##### 0.5-D3 --- Literal and prefix streaming parity

Goal: migrate literal and prefix matching onto incremental execution while
preserving whole-source matching, boundary, validation, span, and ordering
semantics.

Required work:

-   [ ] Preserve literal matcher continuation across arbitrary transport
    boundaries.
-   [ ] Detect literal matches crossing one or multiple chunks exactly once.
-   [ ] Preserve prefix leading-boundary semantics.
-   [ ] Keep prefix candidates unresolved while later bytes can still extend or
    invalidate the relevant token.
-   [ ] Preserve exact global byte spans.
-   [ ] Preserve whole-source validation behavior.
-   [ ] Preserve rule identity and metadata.
-   [ ] Avoid a generic copied overlap window when compact matcher state is
    sufficient.

Acceptance gate:

-   [ ] Literal fixtures produce semantic output identical to whole-source
    execution for adversarial partitions.
-   [ ] Prefix fixtures produce semantic output identical to whole-source
    execution for adversarial partitions.
-   [ ] Matcher prefixes divided at every relevant byte position are handled
    correctly.
-   [ ] Logical matches are neither duplicated nor suppressed by chunk
    placement.
-   [ ] Retained literal and prefix source-dependent state has an explicit
    bound.

##### 0.5-D4 --- Suffix streaming parity

Goal: migrate suffix matching while preserving reverse token-boundary semantics
without retaining an unbounded token or source prefix.

Required work:

-   [ ] Preserve enough bounded token history to recover candidate starts when a
    suffix is recognized.
-   [ ] Preserve trailing-boundary semantics independently of transport
    boundaries.
-   [ ] Keep unresolved suffix candidates pending while later input can change
    their validity.
-   [ ] Preserve exact global spans and source coordinates.
-   [ ] Preserve validator behavior and rule metadata.
-   [ ] Make any maximum retained token or candidate extent explicit and
    testable.

Acceptance gate:

-   [ ] Suffix matches crossing arbitrary chunk boundaries are equivalent to
    whole-source results.
-   [ ] Leading and trailing token-boundary cases preserve existing behavior.
-   [ ] Chunk boundaries cannot create false suffix findings.
-   [ ] Chunk boundaries cannot suppress valid suffix findings.
-   [ ] Suffix retained source-dependent state has an explicit semantic bound.

##### 0.5-D5 --- Pattern and captured-pattern streaming parity

Goal: migrate pattern-based matching, including projected capture spans, onto a
bounded incremental execution strategy.

Required work:

-   [ ] Implement bounded incremental execution for the existing pattern
    portfolio.
-   [ ] Preserve whole-match semantics where validators depend on the complete
    pattern match.
-   [ ] Preserve projected capture spans for captured-pattern rules.
-   [ ] Support matches whose complete match and capture cross different
    transport chunks.
-   [ ] Preserve exact global byte spans after capture projection.
-   [ ] Preserve source coordinates after projection.
-   [ ] Make matcher-specific history, lookahead, and unresolved candidate
    bounds explicit.
-   [ ] Do not introduce an implementation-wide arbitrary overlap constant as
    the correctness mechanism.

If implementation discovers a current pattern whose existing semantics cannot
be represented with bounded retained source state, implementation of that case
must stop and the exception must be made explicit before general streaming
parity is claimed.

Acceptance gate:

-   [ ] Every stream-compatible pattern rule produces whole-source-equivalent
    results under adversarial partitions.
-   [ ] Captured-pattern spans remain exact across chunk boundaries.
-   [ ] End-of-source pattern finalization preserves existing semantics.
-   [ ] Pattern execution has explicit retained-state bounds.
-   [ ] No unsupported unbounded case is hidden behind a large overlap buffer.

##### 0.5-D6 --- Contextual validation and prefilter parity

Goal: preserve validator authority and contextual detection semantics when
candidate evidence and contextual evidence arrive in different transport
chunks.

Required work:

-   [ ] Preserve the existing validator as the authority for candidate
    acceptance or rejection.
-   [ ] Supply validators with equivalent bounded evidence independently of
    chunk placement.
-   [ ] Preserve contextual evidence that occurs before a candidate.
-   [ ] Preserve contextual evidence that occurs after a candidate.
-   [ ] Preserve rule-specific contextual prefilter behavior.
-   [ ] Preserve the shared contextual-pattern gate semantics.
-   [ ] Prefer compact incremental semantic state where complete source regions
    need not be retained.
-   [ ] Define and test validator-specific retained history and lookahead bounds.
-   [ ] Preserve end-of-source evidence where absence of additional input is
    semantically significant.

Acceptance gate:

-   [ ] Contextual findings are identical to whole-source findings under
    adversarial partitions.
-   [ ] Splitting a candidate from its contextual evidence cannot change its
    semantic result.
-   [ ] Prefilter behavior cannot suppress an otherwise valid streamed finding.
-   [ ] Validators are not duplicated or weakened for streaming.
-   [ ] Validator source-dependent state has explicit bounds.

##### 0.5-D7 --- Finalization, normalization, and SensitiveCandidate parity

Goal: introduce the finalization frontier required to release source-dependent
state while preserving exact-span ownership, normalization, classification, and
deterministic ordering.

Required work:

-   [ ] Represent unresolved matcher and validator candidates separately from
    finalized semantic results.
-   [ ] Advance results across the finalization frontier only when later input
    can no longer alter their semantic outcome.
-   [ ] Preserve exact-span collision ownership.
-   [ ] Preserve rule-priority semantics when generic and provider-specific
    detections collide.
-   [ ] Preserve deterministic normalization.
-   [ ] Preserve deterministic final ordering.
-   [ ] Preserve the distinction between confirmed `Finding` values and
    ambiguous `SensitiveCandidate` values.
-   [ ] Ensure temporary uncertainty caused solely by incomplete transport input
    never becomes a public `SensitiveCandidate`.
-   [ ] Release source material when no unresolved semantic decision still
    requires it.
-   [ ] Finalize all remaining resolvable state deterministically at
    end-of-stream.

Acceptance gate:

-   [ ] Finding parity is exact between whole-source and streamed execution.
-   [ ] `SensitiveCandidate` parity is exact between whole-source and streamed
    execution.
-   [ ] Exact-span ownership is independent of candidate discovery order and
    chunk layout.
-   [ ] Final result ordering is independent of chunk layout.
-   [ ] Pending source material does not remain retained after its semantic
    dependencies have been finalized.
-   [ ] End-of-stream resolves all remaining valid pending state exactly once.

##### 0.5-D8 --- Public streaming surface

Goal: expose the smallest public Rust API capable of expressing the proven
streaming contract without leaking internal execution details.

Required work:

-   [ ] Expose creation of a source-local streaming session from an immutable
    scanner.
-   [ ] Expose borrowed byte-oriented input.
-   [ ] Keep empty input distinct from end-of-stream.
-   [ ] Expose explicit finalization producing the existing `ScanReport`.
-   [ ] Prefer consuming or otherwise irreversible successful finalization.
-   [ ] Prevent accidental input after successful completion.
-   [ ] Make terminal streaming errors invalidate the current source execution.
-   [ ] Keep matcher, validator, normalization, and retained-source internals
    private.
-   [ ] Keep caller buffers caller-owned after each input operation returns.
-   [ ] Avoid exposing raw matched secret material through public streaming
    state, errors, or debug output.
-   [ ] Keep the core API synchronous and runtime-independent.
-   [ ] Do not require caller-managed overlap, reset, callbacks, channels,
    iterators, or incremental result delivery.

Exact public type, method, and error names must be chosen from the implementation
that satisfies these requirements rather than copied mechanically from roadmap
examples.

Acceptance gate:

-   [ ] The minimal public surface expresses the 0.5-C ownership and lifecycle
    contract without unnecessary operations.
-   [ ] Rust ownership prevents or clearly rejects invalid post-completion use.
-   [ ] Public errors do not expose scanned secret material.
-   [ ] The API has no dependency on an asynchronous runtime.
-   [ ] Chunk size remains a performance choice rather than a correctness
    parameter.
-   [ ] Existing whole-source callers are not required to migrate.

##### 0.5-D9 --- Whole-source convergence

Goal: ensure whole-source and streaming execution use one semantic authority
rather than becoming independently evolving scanners.

Required work:

-   [ ] Integrate the existing whole-source path with the new execution
    machinery where doing so removes duplicated semantic authority.
-   [ ] Preserve existing `Scanner::scan` behavior and result types.
-   [ ] Preserve identified-source orchestration and input ordering.
-   [ ] Preserve source-length semantics.
-   [ ] Preserve existing parallelism across independent whole sources.
-   [ ] Keep source identity outside low-level matcher and streaming state.
-   [ ] Remove or isolate superseded detection paths that would otherwise create
    two definitions of matching, validation, normalization, or ownership.
-   [ ] Retain optimized whole-buffer entry paths only where they share the same
    semantic decisions as streaming execution.

Acceptance gate:

-   [ ] Whole-source regression tests remain green.
-   [ ] Streaming and whole-source reports are semantically identical for the
    same logical source.
-   [ ] No matching or validation family has two independently maintained
    semantic implementations.
-   [ ] Existing public whole-source contracts remain source-compatible unless a
    separately justified breaking change is explicitly documented.
-   [ ] Multi-source ordering and source identity remain unchanged.

##### 0.5-D10 --- Adversarial partition and bounded-memory conformance

Goal: prove the complete streaming implementation against the partition
invariance and boundedness contracts rather than against a small set of
preferred chunk sizes.

Required conformance coverage:

-   [ ] Empty logical source.
-   [ ] One chunk containing the complete source.
-   [ ] One-byte chunks.
-   [ ] Empty chunks interspersed with non-empty chunks.
-   [ ] Every byte boundary for bounded representative fixtures.
-   [ ] Splits inside multi-byte UTF-8 scalars.
-   [ ] Splits inside matcher prefixes and suffixes.
-   [ ] Splits inside complete pattern matches.
-   [ ] Splits inside captured values.
-   [ ] Splits between candidates and required contextual evidence.
-   [ ] Splits immediately before and after token boundaries.
-   [ ] Splits immediately before and after line boundaries.
-   [ ] Findings requiring end-of-stream finalization.
-   [ ] Incomplete UTF-8 completed by later input.
-   [ ] Definitively invalid UTF-8.
-   [ ] Incomplete UTF-8 at end-of-stream.
-   [ ] Exact-span collision and ownership cases.
-   [ ] `SensitiveCandidate` cases.
-   [ ] Multiple independent sessions from one scanner.
-   [ ] Abandoned unfinished sessions.
-   [ ] Terminal failure followed by a clean independent session.
-   [ ] Multiple partition layouts producing identical final semantic output.

Bounded-memory proof must classify retained source-dependent state by family:

```text
utf8_carry
+ matcher_continuation
+ bounded_history
+ bounded_lookahead
+ unresolved_candidate_material
+ validator_context
+ normalization_pending_state
```

For each component, tests or implementation invariants must demonstrate that
retention is derived from semantic bounds rather than total logical-source
length.

Compact finalized report metadata may scale with the number of reportable
results required by `ScanReport`. That is not equivalent to retaining the
logical source and does not weaken the bounded source-state requirement.

Acceptance gate:

-   [ ] Partition-equivalence tests cover every implemented matching and
    validation family.
-   [ ] No correctness test depends on one preferred transport chunk size.
-   [ ] Source-dependent retained state does not grow as
    `O(total_source_length)` merely because a source is long.
-   [ ] Every retained-state bound is attributable to an explicit matcher,
    validator, transport, or normalization requirement.
-   [ ] No arbitrary global overlap size acts as an undocumented semantic
    dependency.
-   [ ] Large-source tests demonstrate that processing does not require retaining
    the complete source.

##### 0.5-D11 --- Release compatibility and v0.5 closure

Goal: close the architectural release only after the implementation proves all
contracts established by 0.5-A through 0.5-D.

Required release gate:

-   [ ] All 0.5-A semantic invariants are implemented and covered by
    conformance tests.
-   [ ] All 0.5-B execution-architecture requirements are represented by the
    implementation or explicitly superseded by an equivalent documented design.
-   [ ] The public streaming surface satisfies 0.5-C.
-   [ ] D1 through D10 acceptance gates are green.
-   [ ] Whole-source scanner regressions are green.
-   [ ] Streaming versus whole-source semantic parity is green across all
    supported rule and validator families.
-   [ ] UTF-8 validity and source coordinates are partition-invariant.
-   [ ] Exact spans, rule identity, severity, confidence, remediation,
    detection mode, explanations, normalization, ownership, candidates, and
    ordering preserve existing semantics.
-   [ ] Source-session isolation is demonstrated across success, abandonment,
    and terminal failure.
-   [ ] Bounded retained source state is demonstrated rather than inferred from
    chunked input.
-   [ ] No second independently evolving detection engine exists.
-   [ ] Public streaming state and errors introduce no new secret-exposure
    surface.
-   [ ] Public API documentation describes ownership, input, completion,
    errors, UTF-8 behavior, and compatibility.
-   [ ] Relevant examples demonstrate streaming without teaching callers to
    depend on a particular chunk size.
-   [ ] Formatting, linting, unit tests, integration tests, documentation tests,
    and the repository's supported CI matrix are green.
-   [ ] The release notes describe the streaming core as an architectural
    extension while preserving the existing whole-source compatibility surface.

The following remain outside the v0.5 closure gate unless implementation
evidence establishes that one is required for correctness:

- async runtime integration;
- async reader adapters;
- incremental public result delivery;
- callbacks or result channels;
- serialization or persistence of active sessions;
- pause/resume;
- public reset;
- intra-source parallelism;
- C ABI propagation;
- WebAssembly propagation;
- language-binding propagation;
- CLI streaming UX;
- detector catalog expansion unrelated to streaming;
- packaging or distribution work unrelated to the core architectural change.

A synchronous `std::io::Read` convenience adapter may be evaluated after the
primitive byte-oriented public session is stable. It is not permitted to become
a second semantic execution path.

##### Implementation discipline

The implementation branch should progress through D1 to D11 in small,
independently reviewable slices.

Each slice should normally follow:

```text
implementation
    |
targeted tests
    |
whole-source parity tests
    |
partition-equivalence tests where applicable
    |
boundedness/lifecycle gate where applicable
    |
commit
```

A slice may be divided into smaller commits when that improves reviewability,
but a later semantic family should not be migrated merely to hide a failing
acceptance gate in an earlier family.

If implementation evidence invalidates an architectural assumption, the correct
response is to update the relevant 0.5 contract explicitly and review that
change before continuing. The implementation must not silently redefine the
roadmap.

##### Final acceptance gate

v0.5 architecture is implementation-complete only when:

```text
whole-source semantics
        ==
streaming semantics under arbitrary valid partitioning
```

for every supported streaming-compatible rule and validator family, while
source-dependent retained memory remains bounded by explicit semantic state
rather than complete logical-source length.

At that point:

- source partitioning is semantically invisible;
- whole-source and streaming execution share one detection authority;
- streamed input preserves exact global source coordinates;
- validator authority is unchanged;
- normalization and exact-span ownership are unchanged;
- `SensitiveCandidate` semantics are unchanged;
- end-of-stream finalization is deterministic;
- source sessions are isolated;
- the public API remains minimal and runtime-independent;
- existing whole-source callers retain their compatibility surface;
- bounded-memory streaming is a demonstrated property of the core rather than a
  consequence assumed from reading input in chunks.

Design outcome:

- 0.5-A defines what streaming must mean;
- 0.5-B defines how streaming execution is structured;
- 0.5-C defines how callers interact with it;
- 0.5-D defines how the implementation is introduced and proven;
- D1 through D11 provide the ordered implementation gates for the v0.5
  development branch;
- passing D11 closes the v0.5 Streaming Core architectural line.

## Completed 0.4 release line

### v0.4.6 --- Sensitive Data Foundation

Status: completed.

Goal: extend Cribra from a high-confidence secret and credential scanner
into a high-confidence sensitive-data detection engine without weakening
its false-positive resistance, deterministic semantics, or
embeddability.

#### 0.4.6-A --- Sensitive-data architecture and authority

Status: completed.

-   [x] Audit the existing public contracts for representing sensitive
    data without redesigning the core.
-   [x] Confirm that `RuleSpec`, validators, `Finding`,
    `SensitiveCandidate`, `DetectionMode`, `Confidence`, `Severity`,
    `Remediation`, and the existing exact-span pipeline are sufficient
    for the foundation scope.
-   [x] Keep `RuleKind` as matching-strategy authority rather than
    introducing a semantic sensitive-data taxonomy into it.
-   [x] Reject a new public category model until a concrete consumer
    requirement demonstrates that rule identity and pack membership are
    insufficient.
-   [x] Define opt-in pack semantics through existing static
    `RuleSpec` collections and `ScannerBuilder::builtins()`.
-   [x] Keep `builtins::CURRENT` as the existing default credential and
    security baseline; sensitive financial-data rules remain opt-in.
-   [x] Reject a new `Pack` type, registry, trait, or runtime pack
    authority while the existing composition contract is sufficient.
-   [x] Define classified-versus-ambiguous semantics:
    accepted validated material becomes a `Finding`, failed validation
    is rejected, and `SensitiveCandidate` remains an explicit
    review-only path rather than a fallback for rejected matches.
-   [x] Confirm that deterministic validation does not imply a fixed
    confidence level; confidence remains evidence-driven and independent
    from `DetectionMode`.
-   [x] Confirm that existing severity semantics and
    `Remediation::RemoveSensitiveValue` are sufficient for the initial
    financial-data scope.
-   [x] Preserve metadata-only results and exact authoritative source
    spans without introducing matched sensitive values into public
    metadata.

#### 0.4.6-B --- Financial-data foundation

Status: completed.

-   [x] Define the authoritative financial-data detector portfolio.
-   [x] Add financial-data detection only where strong structural
    validation is available.
-   [x] Add IBAN detection with country-aware structural and checksum
    validation.
-   [x] Add payment-card PAN detection with structural and checksum
    validation.
-   [x] Research additional financial identifiers only where
    classification can remain high-confidence.
-   [x] Keep financial-data rules outside `builtins::CURRENT` and expose
    them through an explicit opt-in built-in pack.
-   [x] Preserve exact authoritative source spans.
-   [x] Preserve metadata-only public findings; never expose matched
    sensitive values through result metadata.

#### 0.4.6-C --- Transformation and interface parity

Status: completed.

-   [x] Extend synthesis, redaction, template, and pseudonymization
    support where new sensitive-data rules require it.
-   [x] Expose the opt-in financial built-in pack through the C ABI and
    WebAssembly interfaces.
-   [x] Maintain deterministic Rust behavior and C ABI/WebAssembly
    semantic parity for financial-pack selection.
-   [x] Verify explanations, severity, confidence, remediation, and
    pack behavior across supported interfaces.
-   [x] Verify transformation behavior for financial findings across
    Rust, C ABI, and WebAssembly.

#### 0.4.6-D --- Adversarial validation and release gate

Status: completed.

-   [x] Add adversarial false-positive and false-negative regression
    coverage for every accepted sensitive-data family.
-   [x] Validate malformed, checksum-invalid, boundary-invalid,
    placeholder, example, and cross-format cases.
-   [x] Verify deterministic collision and ownership behavior against
    existing generic and specialized rules.
-   [x] Run full Rust, C ABI, WebAssembly, MSRV, security, and packaging
    gates.
-   [x] Complete release documentation and publication preparation.

Release outcome:

-   [x] Published Cribra v0.4.6.
-   [x] Published `cribra-wasm` v0.4.4 with the v0.4.6 financial
    capability surface.

### v0.4.7 --- Sensitive Data and System Security Completion

Status: completed.

Goal: complete Cribra's planned high-confidence detection portfolio across
sensitive personal data and remaining Unix/system-security material, then move
the project from planned detector expansion to evidence-driven maintenance and
ecosystem development.

This milestone is a coverage-completion release, not a mandate to maximize the
number of built-in rules. Every candidate remains subject to Cribra's existing
structural, contextual, false-positive, ownership, and exact-span requirements.

#### A. Final coverage audit — DONE

- [x] Audit the existing built-in portfolio and identify genuine residual coverage gaps.
- [x] Audit structured identity and personal-data identifiers.
- [x] Audit residual financial and cryptocurrency-sensitive material.
- [x] Audit Unix and system-security residuals.
- [x] Consolidate candidates into IMPLEMENT / COVERED / DEFERRED / REFUSED.
- [x] Freeze semantic ownership, collision, precedence, and span policy.
- [x] Establish the canonical Coverage Manifest contract, including direct,
      transitive, deferred, and refused coverage.

Outcome:

- the final v0.4.7 detector-expansion scope is frozen;
- broad speculative catalog expansion is explicitly out of scope;
- deferred categories require evidence-driven reconsideration;
- refused categories are documented as intentional non-ownership;
- `docs/COVERAGE.md` is the canonical human-readable coverage and ownership
  authority and must be reconciled against the released portfolio at the
  v0.4.7 completion gate.

#### B. Sensitive personal-data completion — DONE

-   [x] Audit physical-address recognition with conservative contextual or
    candidate semantics.
-   [x] Audit email-address sensitive-data semantics without turning Cribra
    into a generic email harvester.
-   [x] Audit telephone-number sensitive-data semantics with region-aware
    validation where practical.
-   [x] Implement accepted structured identity/personal identifiers from the
    final coverage audit:
    - [x] Italian Codice Fiscale.
    - [x] Polish PESEL.
    - [x] UK NHS Number.
    - [x] US Social Security Number under strong contextual semantics.
    - [-] ICAO machine-readable travel-document zones — DEFERRED / LOW.
-   [x] Keep recognition and sensitive classification as separate decisions.
-   [x] Prefer `SensitiveCandidate` when evidence is useful but insufficient
    for authoritative classification.
-   [x] Preserve exact source spans and metadata-only public results for the
    implemented personal-data portfolio.
-   [x] Add adversarial coverage for examples, documentation, public
    identifiers, malformed values, and ordinary non-sensitive data.

Progress:

- Italian Codice Fiscale detection is complete and exposed through the
  explicit opt-in `builtins::personal::CURRENT` portfolio.
- `personal.it-codice-fiscale` owns canonical 16-character natural-person
  identifiers, including structurally valid omocodic representations.
- Validation is deterministic and structural: positional semantics, encoded
  birth fields, birthplace-code structure, and control character are
  authoritative; registry assignment or holder identity is not claimed.
- Codice Fiscale remains outside `builtins::CURRENT`.
- Default and personal portfolios compose without duplicate ownership.
- `docs/COVERAGE.md` records the implemented Codice Fiscale family as DIRECT
  coverage.
- Polish PESEL detection is complete and exposed through the explicit opt-in
  `builtins::personal::CURRENT` portfolio.
- `personal.pl-pesel` validates the canonical 11-digit representation,
  including encoded century/date semantics, Gregorian calendar validity, and
  checksum.
- PESEL validation is deterministic and structural; assignment, registry
  presence, and holder identity are not claimed.
- `docs/COVERAGE.md` records PESEL as DIRECT coverage.
- UK NHS Number detection is complete and exposed through the explicit opt-in
  `builtins::personal::CURRENT` portfolio.
- `personal.uk-nhs-number` supports compact and canonical 3-3-4
  representations, requires Modulus 11 validity and explicit NHS-number
  context, and preserves the exact source representation as the finding span.
- NHS Number classification is contextual: a checksum-valid bare 10-digit
  sequence is deliberately insufficient for classification.
- Structural validation does not claim assignment, patient identity, or
  authoritative registry presence.
- `docs/COVERAGE.md` records NHS Number as DIRECT coverage.
- US Social Security Number detection is complete and exposed through the
  explicit opt-in `builtins::personal::CURRENT` portfolio.
- `personal.us-ssn` supports compact and canonical `AAA-GG-SSSS`
  representations, applies current SSA structural impossibility constraints,
  and requires explicit SSN-specific field context.
- SSN classification is contextual: a structurally possible bare nine-digit
  value is deliberately insufficient for classification.
- Structural validation does not claim assignment, holder identity, or
  authoritative SSA record presence.
- `docs/COVERAGE.md` records US SSN as DIRECT coverage.

#### 0.4.7-C --- Unix and system-security completion — DONE

Existing baseline includes Unix shadow password verifiers, htpasswd password
verifiers, `.netrc` passwords, OpenSSH and common private-key formats,
WireGuard private and preshared keys, generic password/passphrase/auth
surfaces, HTTP authentication, relevant container/provider credentials, and
validated NATS NKey secret material.

-   [x] Perform a complete Unix/Linux credential-surface gap audit against the
    existing built-in catalog.
-   [x] Audit additional system/service authentication formats only where the
    credential or verifier can be identified from source content with strong
    structural or contextual evidence.
-   [x] Audit SSH authentication material not already covered by OpenSSH and
    generic private-key rules:
    - [x] retain private-key ownership with the corresponding OpenSSH, PKCS#8,
          RSA, EC, or other supported private-key representation rather than
          adding SSH-specific duplicate Findings;
    - [x] keep SSH public keys, `authorized_keys`, `known_hosts`, and public SSH
          certificate material outside the sensitive-secret contract;
    - [x] treat `IdentityFile`, `IdentityAgent`, and related SSH configuration
          as references or configuration rather than credential material;
    - [x] do not add an SSH-specific detector where source content provides no
          stronger secret semantics than an existing private-key or passphrase
          owner.
-   [x] Audit common daemon and infrastructure authentication formats not
    already owned by generic, provider-specific, package-ecosystem, or
    container rules:
    - [x] add deterministic NATS NKey seed detection with structural decoding,
          supported key-family validation, and checksum validation;
    - [x] add deterministic NATS encoded private-key detection with structural
          decoding and checksum validation;
    - [x] keep NATS public operator, account, user, server, cluster, and curve
          NKeys outside the sensitive-secret contract;
    - [x] reject malformed, corrupted, non-canonical, and unsupported NKey
          material.
-   [x] Audit shell and system configuration credential conventions only where
    source content itself establishes the security contract.
-   [x] Audit additional password-verifier/hash formats separately from generic
    sensitive hashes:
    - [x] retain dedicated structured ownership for supported `/etc/shadow`
          SHA-256 crypt, SHA-512 crypt, and yescrypt verifiers;
    - [x] retain dedicated structured ownership for supported `.htpasswd`
          APR1 and bcrypt verifiers;
    - [x] keep `generic.sensitive-hash` limited to explicit sensitive-hash
          context rather than treating arbitrary password-hash syntax as a
          Finding;
    - [x] do not expand the built-in portfolio merely to recognize additional
          crypt, Argon2, PBKDF2, scrypt, or other verifier syntax without a
          stronger source-level security contract.
-   [x] Implement TOTP/HOTP shared provisioning-secret detection:
    - [x] support `otpauth://totp` and `otpauth://hotp` provisioning material;
    - [x] support explicit OTP-secret configuration fields;
    - [x] expose only the shared-secret value as the sensitive span;
    - [x] reject arbitrary bare Base32 values;
    - [x] keep contextual ownership distinct from generic secret detection;
    - [x] define explicit remediation, precedence, metadata, and synthesis
          semantics.
-   [x] Implement accepted Tailscale credential formats with specific semantic
    ownership for authoritative credential prefixes.
-   [x] Document rejected Unix/system candidates where reliable static
    classification is not possible.
-   [x] Do not duplicate existing `.netrc`, shadow, htpasswd, private-key,
    WireGuard, generic-auth, HTTP-auth, container, provider, or
    package-ecosystem ownership.

#### 0.4.7-D --- Ownership, ambiguity, and adversarial hardening

-   [x] Define deterministic ownership for every newly accepted rule.
-   [x] Add collision regressions against existing generic, provider,
    ecosystem, financial, and system rules.
-   [x] Expand positive, negative, malformed, placeholder, example, and
    documentation corpora.
-   [x] Verify boundary and exact-span behavior for every accepted family.
-   [x] Preserve the distinction between credentials, sensitive identifiers,
    password verifiers, hashes, keys, and merely security-related
    configuration.
-   [x] Prefer deliberate false negatives over noisy classification.

#### 0.4.7-E --- Transformation and interface parity

-   [x] Verify every accepted capability through the generic redact, template,
    pseudonymize, and synthesize contracts where applicable.
-   [x] Add category-specific transformation semantics only when the generic
    contract is insufficient.
-   [x] Maintain deterministic Rust behavior.
-   [x] Maintain complete applicable C ABI semantic parity.
-   [x] Maintain complete applicable WebAssembly semantic parity.
-   [x] Preserve metadata-only and secret-safe public boundaries.

#### 0.4.7-F --- Portfolio completion gate

-   [x] Re-audit the complete built-in and opt-in detector portfolio after all
    accepted v0.4.7 work.
-   [x] Verify that no known materially important detector family remains
    omitted without an explicit accepted reason.
-   [x] Run full Rust, C ABI, WebAssembly, MSRV, security, packaging, and
    publication gates.
-   [x] Document the post-v0.4.7 detector evolution policy.
-   [x] Publish/tag the completed release only after all gates pass.

Principles:

-   Cribra detects secrets, credentials, and statically recognizable sensitive
    material; it is not a general-purpose DLP or content-classification
    platform.
-   Strong validation and contextual authority take precedence over catalog
    breadth.
-   Recognition does not by itself imply sensitive classification.
-   Filesystem paths, external ownership, account activity, compromise state,
    and network validation are not semantic authority.
-   Application-specific correlation, crawling, ownership, and policy remain
    outside the Cribra core.
-   v0.4.7 is intended to complete the currently planned detector-expansion
    phase. Subsequent built-in detector additions must be justified by
    demonstrated production gaps, important new credential formats, or
    downstream requirements rather than routine catalog growth.

## CLI Core Parity and Distribution

Status: planned; parity foundation implemented ahead of milestone.

Goal: make `cribra-cli` the canonical command-line projection of every
applicable public Cribra capability, then distribute that same
executable through common developer package channels.

Parity foundation already implemented:

-   [x] Canonical reusable command model and thin process adapter.
-   [x] Explicit UTF-8 file input with exact source preservation.
-   [x] Standard-input scanning with exact source preservation.
-   [x] Deterministic human metadata-only output.
-   [x] Deterministic JSON metadata-only output.
-   [x] Finding reporting with rule ID, byte span, line/column,
    severity, confidence, and remediation.
-   [x] Candidate reporting with kind and evidence.
-   [x] Finding detection/explanation mode reporting.
-   [x] Secret-safe presentation regression coverage.
-   [x] Minimum-severity query filtering through core `ScanQuery`.
-   [x] Minimum-confidence query filtering through core `ScanQuery`.
-   [x] Successful scans retain exit status 0 by default regardless of
    findings.

Remaining parity work:

-   [ ] Audit every public Cribra capability and maintain an explicit
    CLI parity matrix.
-   [ ] Add exact severity filtering.
-   [ ] Add exact confidence filtering.
-   [ ] Add exact rule-ID filtering.
-   [ ] Add applicable finding sorting through core `ScanSort`.
-   [ ] Complete explainability projection where richer public
    explanation semantics exist.
-   [ ] Expose applicable rule and pack selection without reimplementing
    core policy.
-   [ ] Expose redact transformation.
-   [ ] Expose template transformation.
-   [ ] Expose pseudonymize transformation.
-   [ ] Expose synthesize transformation.
-   [ ] Expose `ShareBundle` functionality.
-   [ ] Define and implement explicit CI/failure-policy exit semantics
    while preserving successful-scan exit 0 by default.
-   [ ] Evaluate multi-source CLI input where required for complete
    applicable core capability coverage.
-   [ ] Expose parallel scanning when multi-source execution makes it
    applicable.
-   [ ] Classify purely programmatic core APIs explicitly as CLI N/A
    rather than silently omitting them.
-   [ ] Add parity/conformance tests covering every applicable
    capability.

Distribution:

-   [ ] Add Homebrew distribution/install support for `cribra-cli`.
-   [ ] Add Debian/Ubuntu APT distribution/install support for
    `cribra-cli`.
-   [ ] Keep Homebrew and APT packaging as distribution adapters over
    the same canonical `cribra` executable.
-   [ ] Document installation, upgrade, and uninstall paths.
-   [ ] Validate packaged binaries against canonical CLI behavior and
    release version.
-   [ ] Add packaging/distribution release gates.

## Bindings and Integrations

Status: planned.

Goal: complete Cribra's native interoperability foundation and expand Cribra
from a mature detection engine into a broadly consumable developer ecosystem
without moving application-specific product policy into the OSS core.

### Binding-ready native FFI

Goal: make `cribra-ffi` a complete, stable, versioned interoperability
foundation from which native language bindings can project the full applicable
Cribra capability surface without reimplementing Cribra semantics.

- [ ] Audit the complete public Rust capability surface against `cribra-ffi`.
- [ ] Define an explicit versioned Rust → C ABI conformance matrix.
- [ ] Close every applicable FFI capability gap.
- [ ] Expose findings, locations, severity, confidence, remediation, candidates,
  and explanations completely through the FFI.
- [ ] Expose applicable query and filtering semantics.
- [ ] Expose rule and pack selection required by the public core contract.
- [ ] Expose applicable redact, template, pseudonymize, synthesize, and
  `ShareBundle` transformations.
- [ ] Define stable ownership, borrowing, allocation, destruction, and error
  contracts for binding consumers.
- [ ] Define ABI/version compatibility guarantees.
- [ ] Provide binding-oriented conformance fixtures and test vectors.
- [ ] Verify metadata-only and secret-safe boundaries across the complete FFI.
- [ ] Verify deterministic equivalence against authoritative Rust results.
- [ ] Document explicitly any Rust capability that is genuinely inapplicable to
  the native ABI.
- [ ] Freeze the binding-ready FFI contract only after complete parity gates
  pass.

Principles:

- Rust remains the semantic authority.
- `cribra-ffi` projects Rust semantics and never reimplements detection or
  classification.
- Native bindings consume `cribra-ffi`; binding-specific policy does not enter
  the core or FFI.
- A missing applicable capability is an FFI parity gap, not an accepted binding
  limitation.

### GitHub integration

-   [ ] Provide an official GitHub Action over the canonical Cribra interfaces.
-   [ ] Preserve Cribra detection semantics rather than implementing
    Action-specific classification.
-   [ ] Define explicit CI/failure-policy behavior.
-   [ ] Keep source handling and diagnostics secret-safe.
-   [ ] Provide deterministic machine-readable findings suitable for CI.
-   [ ] Validate the Action against the shared Cribra conformance corpus.
-   [ ] Keep the integration usable without a Silens account or paid service.

### Astro integration

-   [ ] Provide an official Astro integration for appropriate development,
    build, or CI scanning workflows.
-   [ ] Reuse Cribra's existing portable interfaces rather than implementing
    detection semantics in JavaScript/TypeScript.
-   [ ] Define explicit source and lifecycle boundaries.
-   [ ] Preserve local-first and secret-safe behavior.
-   [ ] Keep the integration fully usable without a Silens account or paid
    service.
-   [ ] Validate semantic equivalence against authoritative Cribra results.

### Agent and AI interoperability

-   [ ] Define a minimal official MCP surface over authoritative Cribra
    capabilities.
-   [ ] Keep detection and classification deterministic and owned by Cribra;
    language models may orchestrate or explain results but do not become
    detection authority.
-   [ ] Define structured, secret-safe findings, candidates, explanations, and
    transformation contracts for agent consumers.
-   [ ] Separate read-only scanning capabilities from explicit transformation
    or action capabilities.
-   [ ] Provide an official ChatGPT/Codex-compatible integration over the
    shared agent contract.
-   [ ] Provide an official Claude-compatible integration over the shared
    agent contract.
-   [ ] Avoid provider-specific detection semantics in the Cribra core.
-   [ ] Keep the developer-level Cribra agent integration usable independently
    of Silens commercial services.

### Application ecosystem boundary

Cribra developer integrations and Silens application integrations are distinct
layers.

Cribra-owned developer surfaces such as the CLI, native FFI, WebAssembly,
GitHub Action, Astro integration, and agent/MCP interoperability remain
open-source engine and developer integrations.

End-user application integrations are products of the relevant Silens service,
not premium Cribra editions. They may embed or consume Cribra as their
authoritative detection engine while providing separate application workflows,
user experience, account integration, and commercial capabilities.

For example, a WordPress end-user integration belongs to Silens Scan / Scan+
and may identify Cribra as its underlying detection engine. Cribra itself does
not withhold built-in detector semantics in order to create application-tier
gating.

This separation allows broad Cribra adoption to strengthen the visibility and
trust of products built on it without coupling the OSS engine to commercial
entitlements.

On market demand:

-   Drupal application integration.
-   Joomla application integration.
-   Strapi application integration.
-   Additional CMS, framework, or application integrations justified by
    demonstrated demand.

Principles:

-   Engine and developer integrations remain Cribra.
-   End-user application integrations belong to the corresponding Silens
    service.
-   Application integrations are access surfaces, not new subscription
    products.
-   Commercial entitlements belong to Silens application capabilities, not to
    artificially restricted Cribra detector semantics.
-   Shared infrastructure may be reused across application integrations without
    collapsing independent Silens services into a single embedded product.

## Native Bindings

Native bindings are independent interoperability subprojects maintained within
this repository and built on the binding-ready `cribra-ffi` contract. They do
not define Cribra core release milestones and may progress, version, and reach
parity independently after the required FFI surface is available.

A binding is considered supported only after complete applicable FFI parity has
been verified. Partial implementations may exist as work in progress but are
not considered parity-complete bindings.

### Swift

Status: planned.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Define idiomatic Swift ownership and lifetime handling.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define Swift Package Manager packaging.
- [ ] Verify package installation and supported-platform builds.
- [ ] Mark parity checked.

### JVM — Kotlin / Java

Status: planned.

One shared JVM interoperability implementation should serve both Kotlin and
Java rather than maintaining two independent native bindings.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Provide idiomatic Kotlin and Java-facing APIs over the shared binding.
- [ ] Define native ownership, lifecycle, and error handling.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define Maven Central packaging.
- [ ] Verify Kotlin and Java consumer projects.
- [ ] Mark parity checked.

### Python

Status: planned.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Define Python ownership, lifecycle, and exception semantics.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define PyPI packaging and supported wheels.
- [ ] Verify package installation on supported platforms.
- [ ] Mark parity checked.

### Go

Status: planned.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Define Go ownership, lifecycle, and error semantics.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define Go module and native-library distribution strategy.
- [ ] Verify supported-platform consumers.
- [ ] Mark parity checked.

### .NET / C#

Status: planned.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Define safe managed/native ownership and disposal semantics.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define NuGet packaging and native runtime assets.
- [ ] Verify supported .NET targets and platforms.
- [ ] Mark parity checked.

### PHP

Status: planned.

- [ ] Project the complete applicable `cribra-ffi` surface.
- [ ] Define PHP/native ownership, lifecycle, and error semantics.
- [ ] Verify findings, candidates, explanations, queries, packs, and
  transformations.
- [ ] Run the shared cross-language conformance corpus.
- [ ] Verify secret-safe/error behavior.
- [ ] Define Composer/extension packaging and supported PHP runtimes.
- [ ] Verify supported-platform installation and consumers.
- [ ] Mark parity checked.

### JavaScript / TypeScript

Status: covered by `cribra-wasm`.

No dedicated JavaScript/TypeScript native binding is planned while
`cribra-wasm` provides the required portable browser/JavaScript interoperability
contract with semantic parity.

A separate native JS/TS binding should be considered only if a concrete runtime
cannot use the WebAssembly contract safely or ergonomically.

## Maintenance and Hardening

Alongside planned feature releases, Cribra continues continuous
maintenance and security hardening.

Expected work includes:

-   bug fixes;
-   security fixes;
-   false-positive reduction;
-   false-negative fixes backed by concrete evidence;
-   compatibility fixes;
-   performance improvements supported by measurement;
-   documentation improvements;
-   maintenance of Rust, C, WebAssembly, CLI, and future binding parity;
-   updates for major provider credential formats when upstream formats
    change;
-   narrowly scoped new detectors when a significant production gap is
    demonstrated.

Feature scope remains evidence-driven and may be reduced, deferred, or
reordered when an audited candidate cannot satisfy Cribra's confidence
requirements.

### Post-v0.4.7 detector evolution

After v0.4.7, planned catalog expansion is considered complete.

New built-in detection should be added only when at least one of the following
is demonstrated:

-   a materially important credential or sensitive-material class is missing;
-   real downstream or production usage exposes a significant coverage gap;
-   a newly important technology introduces a distinct credential format not
    represented by existing rules;
-   an existing rule family cannot safely represent the material without a
    narrowly justified semantic extension.

Routine catalog growth, speculative provider coverage, and detector-count
expansion are not roadmap goals.

Post-v0.4.7 development should primarily focus on:

-   bug and security fixes;
-   false-positive and evidence-backed false-negative hardening;
-   performance and reliability;
-   semantic and adapter parity;
-   compatibility with evolving upstream formats;
-   CLI, FFI, bindings, packaging, distribution, and ecosystem integrations;
-   concrete requirements discovered by downstream consumers.

## Explicit Non-Goals

Cribra is not intended to become a broad, low-confidence DLP or general
content classification platform. Sensitive-data detection is
intentionally supported where Cribra can establish a sufficiently strong
structural or contextual contract.

The following remain outside the roadmap unless new concrete
requirements justify revisiting them:

-   generic entropy-based secret detection;
-   arbitrary Base64 detection;
-   arbitrary hexadecimal or cryptographic hash detection;
-   broad undifferentiated PII classification;
-   broad email-address harvesting without exposure semantics;
-   broad phone-number harvesting without region/context semantics;
-   payment-card/PAN scanning in the default built-in secret pack;
-   IBAN, BIC, SWIFT, or generic bank-account classification in the
    default built-in secret pack;
-   generic financial-data classification without explicit opt-in;
-   arbitrary Kubernetes `data:` or `Secret` Base64 scanning;
-   generic encrypted-blob detection;
-   DPAPI blob detection;
-   public certificate detection as secret material;
-   AI or model-based secret classification;
-   provider network calls to validate whether credentials are active;
-   cloud SDK dependencies for credential verification;
-   filesystem or repository traversal inside the core engine;
-   application-specific exposure, correlation, crawling, or ownership
    policy;
-   application-specific policy engines;
-   speculative plugin architecture;
-   adding provider detectors solely to increase the number of supported
    services.

Cribra should prefer a deliberate false negative over a high-noise
detector when the available evidence is insufficient to distinguish
sensitive material from ordinary data.

## Historical Releases

### v0.4.5 --- Developer Ecosystem Credential Coverage

Status: completed.

v0.4.5 consolidated the remaining high-value developer, package, runtime,
build, and systems credential surfaces into one evidence-driven release
while preserving Cribra's conservative detection and interoperability
contracts.

#### 0.4.5-A --- Scope and coverage audit

Status: completed.

-   [x] Audit high-value developer/package credential surfaces.
-   [x] Prefer documented credential storage and configuration
    contracts.
-   [x] Reuse generic, `.netrc`, HTTP-authentication, and provider
    detectors where they already provide equivalent semantic coverage.
-   [x] Reject ecosystem-specific classification when it would add
    catalog breadth without stronger semantics.
-   [x] Keep network validation and new parser/runtime dependencies out
    of scope.

#### 0.4.5-B --- Developer package credentials

Status: completed.

-   [x] Cargo / Rust registry authentication.
-   [x] Python / PyPI / `.pypirc` repository tokens.
-   [x] RubyGems credentials, including `GEM_HOST_API_KEY`, with
    deterministic collision handling against generic credential rules.
-   [x] NuGet / .NET cleartext package-source credentials in
    `nuget.config`; `NuGetPackageSourceCredentials_{name}` environment
    credentials remain deferred because exact password projection and
    source association require additional parsing beyond the bounded B4
    matcher.
-   [x] Maven repository credentials.
-   [x] Deno authentication tokens audited and deferred.
    `DENO_AUTH_TOKENS` is a documented security-relevant credential
    surface, but its multi-entry representation requires reliable
    per-entry credential discovery and exact span projection that the
    current rule execution model does not provide without broader
    parsing or overly generic scanning. No dedicated Deno rule is added
    in v0.4.5.

Audited surfaces may be rejected or deferred when they cannot satisfy
Cribra's confidence standard. Gradle, Go, Conan, vcpkg, Nix, and related
ecosystems do not require dedicated rules merely for catalog coverage
when existing generic or shared credential surfaces already provide
equivalent semantics.

#### Transformation Semantic Parity

Status: completed.

-   [x] Audit every rule in `builtins::CURRENT`.
-   [x] Give every built-in an explicit synthesis strategy while keeping
    custom rule fallback separate.
-   [x] Keep structured and encoded synthetic values deliberately
    invalid under their corresponding detector contracts.
-   [x] Verify redact, template, and pseudonymization parity through
    their generic span/metadata contracts.
-   [x] Complete the repository validation gates before marking this
    step complete.

#### 0.4.5-C --- Collision and normalization hardening

Status: completed.

-   [x] Verify provider/ecosystem-specific rules deterministically win
    valid collisions with generic credential rules.
-   [x] Preserve exact finding spans and stable rule attribution.
-   [x] Verify contextual prefilters remain consistent with validator
    semantics.
-   [x] Keep rule-ID and detection-mode contracts explicit as validator
    families expand.

#### 0.4.5-D --- Adversarial corpus and false-positive hardening

Status: completed.

-   [x] Add positive, negative, malformed, placeholder, documentation,
    and cross-format fixtures for every accepted family.
-   [x] Add collision regressions for ecosystem-specific versus generic
    rules.
-   [x] Reject protected references, helper names, paths, registry
    names, and credential-store references that are not credential
    material.
-   [x] Preserve Cribra's preference for deliberate false negatives over
    noisy classification.

#### 0.4.5-E --- Interface and parity validation

Status: completed.

-   [x] Validate Rust behavior for every accepted rule.
-   [x] Validate native C ABI exposure and semantic parity.
-   [x] Align `cribra-wasm` with the completed v0.4.5 core semantics.
-   [x] Validate WebAssembly parity for findings, spans, severity,
    confidence, remediation, candidates, and explanations.

#### 0.4.5-F --- Swift Package Manager credential coverage

Status: completed.

Goal: audit Swift Package Manager credential surfaces and add dedicated
detection only where SwiftPM provides stronger, documented semantics
than existing shared credential rules.

-   [x] Audit SwiftPM registry authentication and documented credential
    storage.
-   [x] Audit `SWIFTPM_REGISTRY_TOKEN`, `SWIFTPM_REGISTRY_PASSWORD`,
    `SWIFTPM_SOURCE_CONTROL_TOKEN`, and `SWIFTPM_NETRC_DATA`.
-   [x] Reuse the existing `.netrc` detector wherever it already
    provides equivalent semantics.
-   [x] Preserve exact credential-value spans and deterministic
    collision behavior.
-   [x] Add synthesis semantics and adversarial/collision tests for
    accepted rules.
-   [x] Do not extract credentials from Keychain or other OS credential
    stores.
-   [x] Do not add a SwiftPM parser, Swift runtime dependency, network
    validation, or adapter-specific logic to the core.

#### 0.4.5-G --- Kotlin / Gradle credential audit

Status: completed.

Goal: cover credentials encountered in Kotlin development without
inventing a Kotlin package-manager category. Repository authentication
is primarily owned by Gradle, Maven, AWS, and shared credential
contracts.

-   [x] Audit Gradle Groovy and Kotlin DSL repository credential
    configuration.
-   [x] Audit `PasswordCredentials`, including repository-derived Gradle
    properties.
-   [x] Audit `HttpHeaderCredentials` and existing generic/HTTP
    coverage.
-   [x] Audit `AwsCredentials` only for gaps not already covered by AWS
    rules.
-   [x] Audit Kotlin Multiplatform repository/dependency workflows.
-   [x] Prefer `gradle.*`, `maven.*`, `aws.*`, or existing
    generic/shared ownership over `kotlin.*` rule IDs.
-   [x] Add rules only when documented context materially improves
    confidence, attribution, or exact-span semantics.
-   [x] Allow this step to complete with no new detector if existing
    coverage is sufficient.

#### 0.4.5-H --- PHP / Composer and Dart / pub credential coverage

Status: completed.

Goal: audit the remaining high-value PHP and Dart package/developer
credential surfaces, adding dedicated detection only where Composer or
pub provides documented semantics stronger than existing shared
credential rules.

-   [x] Audit Composer authentication surfaces, including `auth.json`,
    `composer.json` authentication where applicable, and documented
    environment configuration.
-   [x] Audit Composer `http-basic`, `bearer`, `github-oauth`,
    `gitlab-oauth`, `gitlab-token`, and other documented authentication
    families.
-   [x] Reuse existing HTTP, GitHub, GitLab, `.netrc`, and generic
    credential rules wherever they already provide equivalent semantic
    coverage.
-   [x] Audit Dart/pub credential surfaces.
    -   `PUB_HOSTED_URL` is repository configuration, not credential
        material.
    -   `dart pub token add` accepts the secret out-of-band; there is no
        reliable static source representation to detect.
    -   `dart pub token add --env-var` persists/references an arbitrary
        environment variable name; the corresponding environment
        assignment cannot be attributed to Dart/pub from the assignment
        alone.
    -   Recognizable provider credentials remain owned by their
        provider-specific rules.
    -   Decision: no Dart/pub-specific detector is justified for v0.4.5.
-   [x] Audit authenticated custom package repositories and documented
    token configuration used by `dart pub`.
-   [x] Prefer `composer.*`, `pub.*`, provider-specific, or existing
    generic/shared ownership over artificial `php.*` or `dart.*` rule
    IDs.
-   [x] Preserve exact credential-value spans and deterministic
    collision behavior for every accepted rule.
-   [x] Add synthesis semantics and adversarial/collision tests for
    accepted rules.
-   [x] Do not extract credentials from OS credential stores or other
    protected external storage.
-   [x] Do not add PHP/Dart runtimes, package-manager parsers, network
    validation, or adapter-specific logic to the core.

#### 0.4.5-I --- Documentation and release gate

Status: completed.

-   [x] Update release documentation and public coverage descriptions.
-   [x] Run formatting, workspace check/test, Clippy, docs, MSRV,
    RustSec, and package publication gates.
-   [x] Run C ABI release gates.
-   [x] Run WebAssembly adapter, optimization, and parity gates.
-   [x] Verify a clean working tree and protected-main release workflow.
-   [x] Publish/tag Cribra v0.4.5 and `cribra-wasm` v0.4.3 after release
    gates pass.

CLI package-manager distribution was not part of the final v0.4.5
publication gate and remains planned under the canonical CLI milestone.

### v0.4.4 --- Canonical Cribra CLI

Status: completed.

v0.4.4 established Cribra as the authoritative owner of its command-line
interface through the reusable `cribra-cli` crate and thin `cribra`
executable.

Completed work included:

-   canonical reusable command, input, execution, and output contracts;
-   explicit UTF-8 file and stdin input with exact source preservation;
-   deterministic human and JSON metadata-only output;
-   stable `0`/`1`/`2` process exit semantics;
-   privacy-safe diagnostics with no matched secret or candidate value
    leakage;
-   executable integration and adversarial CLI coverage;
-   zero new CLI framework, serialization, argument-parsing, or error
    dependency;
-   documentation and package validation;
-   full workspace, Clippy, docs, MSRV, RustSec, C ABI, and WebAssembly
    parity release gates;
-   protected-main publication of `cribra-cli` and Cribra v0.4.4.

Filesystem traversal remains outside the core engine and was not
required for the canonical v0.4.4 CLI contract.

### v0.4.3 --- High-Confidence Detection Coverage

Status: completed.

v0.4.3 substantially completed the current high-confidence
secret-detection baseline while preserving conservative false-positive
requirements and semantic parity across Rust, C, and WebAssembly
interfaces.

Completed work included:

-   GitLab credential-family coverage;
-   database connection passwords for PostgreSQL, MySQL, MariaDB,
    MongoDB, and Redis;
-   quoted password/passphrase support with exact span preservation;
-   HTTP Basic credentials;
-   ASCII-armored PGP private keys;
-   contextual WireGuard credentials;
-   Docker registry credentials;
-   npm registry credentials;
-   `.netrc` credentials;
-   `/etc/shadow` and `.htpasswd` password verifiers;
-   dedicated password-verifier remediation semantics;
-   adversarial corpus expansion;
-   Rust/C/WebAssembly parity validation;
-   release documentation, packaging, and publication gates.

Provider attribution remains evidence-driven. Bare provider-generic
fields such as `client_secret` are attributed generically unless
surrounding context establishes a specific provider.

### v0.4.2 --- Security Hardening

Status: completed.

v0.4.2 strengthened Cribra's safe-processing boundary and expanded
high-confidence secret coverage without changing the core architecture.

Completed work included atomic scan-and-build share-safe construction,
stronger private-key coverage, contextual HTTP Bearer detection,
JSON-escaped GCP private-key detection, and complete Rust/C/WASM release
validation.

### v0.4.1 --- WebAssembly Publication

Status: completed.

v0.4.1 published the WebAssembly adapter with production packaging,
semantic parity validation, deterministic browser benchmarking, and
Trusted Publishing.

### v0.4.0 --- WebAssembly Interoperability

Status: completed.

v0.4 established WebAssembly as a supported interoperability surface
while preserving the Rust engine as the semantic authority.

### v0.3 --- Native Interoperability

Status: completed.

v0.3 introduced the dedicated native C interoperability adapter and
stable interoperability contracts.

### v0.2 --- Detection and Transformation Baseline

Status: completed.

v0.2 established the core detection, contextual-validation, ambiguity,
explainability, and safe-transformation contracts that later releases
harden and extend.
