# Cribra

Cribra is an embeddable, local-first, fully synchronous Rust engine for deterministic sensitive-data detection, validation, classification, and safe transformation.

It is not a file, repository, or vulnerability scanner. Cribra operates on caller-provided data and provides reusable security primitives that applications, CLIs, APIs, pipelines, WebAssembly hosts, and native integrations can compose into their own workflows.

Use it directly as a Rust crate, through the canonical CLI as a process boundary, via C ABI or WebAssembly, or through the Swift, Python and Kotlin/JVM UniFFI consumers.

*Cribra provides the engine; the surrounding application decides the workflow.*

See [Ecosystem and Scope](docs/ECOSYSTEM.md) for an overview of related
projects, integration models, and responsibility boundaries.
Start with [Getting started](docs/GETTING_STARTED.md); check
[Compatibility and distribution status](docs/COMPATIBILITY.md) before choosing
a platform or package.

Cribra is application-agnostic. Callers retain control of source acquisition,
I/O, storage, persistence, presentation, and policy. The core owns detection,
validation, deterministic normalization, metadata-only reporting, querying, and
explicit share-safe transformations.

Cribra’s core is fully synchronous. It requires no async runtime and performs
no I/O or network access. A logical source is processed serially and
deterministically. Independent sources can optionally be processed in parallel
with Rayon, while streaming execution allows a source to be supplied in
chunks without changing that execution model.

Matched secret values are not stored in public findings.

## Streaming core — available since 0.5.0

Streaming is synchronous and opt-in. Build an admitted ruleset, feed arbitrary
byte fragments, and consume the session at genuine EOF:

```rust
use cribra::{Rule, Scanner, Severity, StreamLimits};

let scanner = Scanner::builder()
    .rule(Rule::literal("credential", "SECRET", Severity::High))
    .build()?;
let limits = StreamLimits::default()
    .with_max_candidates(Some(10_000))
    .with_max_retained_results(Some(1_000));
let mut session = scanner.stream_with_limits(limits)?;
session.push(b"TOKEN=SEC")?;
session.push(b"RET")?;
let report = session.finish()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Successful reports preserve whole-source semantics, including structural review
candidates. Unsupported configurations fail before execution; `Scanner::default()`
and complete Core, Personal and Financial portfolios currently cannot stream.
Existing whole-source methods continue to support those portfolios.

Limits are optional and unlimited by default. Logical executor storage is separate
from retained results and is not a total heap-memory guarantee. Failure returns no
partial report and makes a session terminal.

See [streaming contracts and compatibility](docs/streaming.md) and
[release qualification](docs/0.5-qualification.md).

## Incremental scanning — new in 0.6.0

`Scanner::incremental_session(String)` owns a revisioned UTF-8 document.
`apply_edit(expected_revision, byte_range, replacement)` transactionally updates
its complete report with whole-source parity. Literal and structural matching
are localized; Prefix, Suffix and regex use authoritative whole-rule replay.
Built-in portfolios remain supported here independently of streaming admission.

Four optional limits govern new raw emissions, peak cache ownership, result
materialization and logical transaction payload. Resource failures are terminal
and preserve the previous complete snapshot. The full document is owned; this
API does not promise ruleset-bounded storage or a total heap-memory cap.
See [incremental contracts](docs/incremental.md).

## Capabilities

- deterministic sensitive-data detection over UTF-8 input;
- reusable compiled rule execution;
- bounded retained source state for admitted streaming configurations;
- exact owned-document incremental scanning with documented whole-rule fallbacks;
- literal, prefix, suffix, and regex rules;
- deterministic and contextual validators;
- selectable built-in detector catalog;
- immutable per-source `ScanReport`;
- ordered batch `ScanResults<K>`;
- lazy finding queries and aggregate summaries;
- separate review-only `SensitiveCandidate` results;
- presentation-safe rule metadata and typed explainability;
- remediation metadata;
- redaction;
- semantic templates;
- keyed pseudonymization;
- deterministic keyed synthesis;
- share-bundle construction;
- optional Serde support;
- optional Rayon parallel scanning;
- native C interoperability;
- typed WebAssembly interoperability;
- canonical reusable CLI via `cribra-cli`.

## Engine model

Cribra separates engine semantics from source acquisition and product workflow.

```text
caller-owned data
      │
      ▼
┌──────────────────────────────────────┐
│                Cribra                │
│                                      │
│  detection ──▶ validation            │
│      │              │                │
│      └──────────────┴─▶ normalize    │
│                         │            │
│                         ▼            │
│                 reports / queries    │
│                         │            │
│                         ▼            │
│                    transform         │
└──────────────────────────────────────┘
      │
      ▼
caller-owned policy / storage / output
```

The engine does not discover files, walk repositories, own persistence, make
network requests, or decide application policy.

Scanner and ScannerBuilder are Cribra’s Rust detection API. They do not
define the scope of the overall project: scanning is one capability of the
engine, alongside validation, reporting, querying, explainability, and
transformation.

Whole-source scanning is authoritative. Admitted streaming configurations and
successful incremental edits preserve its complete report semantics. Incremental
sessions own the document; streaming sessions retain bounded source history.
These session APIs are Rust-core capabilities and are not exposed by the current
CLI, C or WASM adapters.

## Privacy boundary

Cribra performs no network access.

The caller owns source acquisition and destination policy. Public findings,
review candidates, explanations, summaries, and serialized metadata contain
coordinates and classification metadata rather than copies of matched source
material.

Transformations are explicit operations and require caller-owned source input.

## Safe Rust boundary

Cribra keeps its detection and transformation engine entirely in safe Rust.

- `cribra` — `#![forbid(unsafe_code)]`
- `cribra-wasm` — `#![forbid(unsafe_code)]`
- `cribra-cli` — `#![forbid(unsafe_code)]`

Unsafe Rust is intentionally isolated to `cribra-capi`, where it is required at
the C ABI/FFI boundary. The core engine itself does not permit unsafe Rust.

## Install

### Rust core

Core v0.7.2 is available on [crates.io](https://crates.io/crates/cribra/0.7.2)
(verified against the registry index). Rust 1.97 is the MSRV.

```toml
[dependencies]
cribra = "0.7.2"
```

Flow enables transformations by default. Core keeps scanner, built-ins,
streaming, incremental sessions and reporting without the transformation layer
or its BLAKE3 dependency:

```toml
cribra = { version = "0.7.2", default-features = false }
```

The standalone `Redaction` marker type remains available in Core. Enable
`transform` explicitly to restore transformation APIs. Fuse combines Flow
with `serde` and `parallel`:

```toml
cribra = { version = "0.7.2", features = ["serde", "parallel"] }
```

| Feature | Default | Purpose |
| --- | --- | --- |
| `transform` | yes | Redaction, templates, keyed pseudonymization, synthesis and share bundles |
| `serde` | no | Serialize supported public data contracts |
| `parallel` | no | Scan independent inputs concurrently with Rayon |

### v0.7 performance and footprint

Cribra v0.7 introduces measurement-led incremental optimizations and
configurable build profiles. These results are local qualification data,
not cross-platform performance or release-size guarantees.

**Incremental edit latency versus v0.6:**

| Workload | v0.7 | Change |
| --- | ---: | ---: |
| Dense, 8 rules, 64 KiB | 19.654 ms | -46.62% |
| Mixed, 64 KiB | 488.09 us | -8.76% |
| Sparse, 64 rules, 64 KiB | 33.813 us | -2.02% |
| Sparse, 4 rules, 1 KiB | 783.20 ns | +3.60% |

The small-input regression is included deliberately. These performance
measurements precede integration with the configurable build profiles;
the combined implementation subsequently passed regression testing.

**WASM footprint (optimized, Brotli level 11):**

| Profile | Compressed WASM |
| --- | ---: |
| Core | 294,690 bytes |
| Flow | 311,469 bytes |

Core is approximately 5.39% smaller than Flow in this measurement.

**Rust dependency footprint:**

| Profile | Distinct normal dependency packages |
| --- | ---: |
| Core | 6 |
| Flow | 10 |
| Fuse | 24 |

Counts exclude Cribra itself. Native executable sizes were nearly
identical for the scanning-only consumer used in qualification; fewer
dependencies do not imply proportionally smaller linked executables.

Measurements: macOS ARM64, Rust 1.99.0. See
[performance methodology](docs/PERFORMANCE.md) and
[configuration qualification](docs/0.7-configuration-qualification.md).

### Canonical CLI

```text
cargo build -p cribra-cli
target/debug/cribra scan config.env --format json
```

Build from the repository root; the executable is `target/debug/cribra`
(`target/debug/cribra.exe` on Windows). The usage below assumes that executable
is on PATH. CLI registry publication is not established by the Core release;
see [distribution status](docs/COMPATIBILITY.md).

```text
cribra scan config.env
cribra scan config.env --format json
cat config.env | cribra scan -
```

The CLI accepts one explicit UTF-8 file or stdin. It does not perform directory,
repository, glob, or filesystem discovery in v0.4.4.

Exit codes:

| Code | Meaning |
| ---: | --- |
| `0` | command executed successfully, including clean, review-only, or finding-bearing scans |
| `1` | input or execution failure |
| `2` | command-line usage or parse error |

CLI output is metadata-only. Matched secret values are not printed.

## Quick start

```rust
use cribra::Scanner;

let scanner = Scanner::default();

let results = scanner.scan([
    ("config.env", "GITHUB_TOKEN=ghp_example"),
    ("settings.toml", "log_level = \"info\""),
]);

for (source, finding) in results.findings() {
    println!(
        "{source}:{}:{} {}",
        finding.location().line(),
        finding.location().column(),
        finding.rule_id(),
    );
}
```

Cribra scans a single source serially. With the optional `parallel` feature,
independent sources can be distributed through Rayon while preserving the same
per-source semantics and deterministic input ordering.

## Detection model

Detection is one capability of the Cribra engine. The `Scanner` API provides
the primary Rust interface for executing compiled detection rules.

```text
caller-owned UTF-8 input
        │
        ├── compiled rule pipeline ───────────────▶ Finding
        │                                          ├─ rule id
        │                                          ├─ location
        │                                          ├─ severity
        │                                          ├─ confidence
        │                                          └─ remediation
        │
        └── structural review path ───────────────▶ SensitiveCandidate
                                                   ├─ kind
                                                   ├─ location
                                                   └─ evidence
```

A `Finding` means the configured rule pipeline has sufficient evidence to
classify the span.

A `SensitiveCandidate` is review-only evidence. It has no finding severity,
confidence, or remediation and is never automatically transformed.

## Built-in detection

The default built-in catalog covers high-confidence security and credential
families including:

- GitHub and GitLab credentials;
- Stripe, Cloudflare, Slack, and Telegram credentials;
- JWT/JWS compact values;
- AWS, Azure, and GCP credentials;
- generic PKCS#8, encrypted PKCS#8, RSA, EC, OpenSSH, and PGP private keys;
- contextual password, database-password, and passphrase fields;
- passwords embedded in supported database connection URIs;
- explicit HTTP Bearer and HTTP Basic authentication;
- contextual WireGuard credentials;
- Docker registry authentication;
- developer and package-registry credentials for supported Cargo, PyPI, NuGet,
  Maven, RubyGems, SwiftPM, Gradle, and Composer surfaces;
- registry-scoped npm credentials;
- `.netrc` credentials;
- supported `/etc/shadow` password verifiers;
- supported `.htpasswd` password verifiers;
- contextual sensitive hashes;
- generic API keys, tokens, and secrets.

The canonical default pack is `builtins::CURRENT`.

Additional built-in families may be exposed through explicit opt-in packs rather than broadening the default detection catalog. The financial pack is available as
`builtins::financial::CURRENT` and currently contains:

- `financial.iban` — registered-country IBAN structure, exact country length,
  and MOD-97 validation;
- `financial.pan` — contextual payment-card PAN recognition using numeric
  structure, supported length, Luhn validity, and strong payment-card context.

Compose it explicitly when financial recognition is required:

```rust
use cribra::Scanner;

let scanner = Scanner::builder()
    .builtins(cribra::builtins::CURRENT)
    .builtins(cribra::builtins::financial::CURRENT)
    .build()?;
# Ok::<(), cribra::ScannerBuildError>(())
```

Financial findings identify locally recognizable data. They do not establish
ownership, issuer or account assignment, account existence, activity,
authorization, or compromise.

Cribra intentionally does not perform generic entropy scanning, arbitrary
Base64 detection, unrestricted hash detection, or broad PII classification.

## Custom rules

Applications can add domain knowledge without changing Cribra's built-in
classification authority.

```rust
use cribra::{Remediation, Rule, Scanner, Severity};

let scanner = Scanner::builder()
    .builtins(cribra::builtins::CURRENT)
    .rule(
        Rule::prefix("acme.api-key", "acme_", Severity::Critical)
            .with_remediation(Remediation::RotateCredential),
    )
    .build()?;

# Ok::<(), cribra::ScannerBuildError>(())
```

Custom rules remain matcher-authoritative and cannot select private built-in
validators.

## Explainability

`Explanation` projects existing classification facts rather than introducing a
second classification authority.

For findings, explanation is resolved against compiled rule metadata. For
review candidates, explanation derives from candidate evidence.

The core intentionally does not provide human-facing explanation copy.

## Transformations

Cribra supports explicit share-safe transformations over classified findings:

- redaction;
- semantic templates;
- deterministic keyed pseudonymization;
- deterministic keyed synthesis.

Ambiguous candidates are never transformed automatically.

When scan results do not need to exist independently from transformation,
prefer the atomic scan-and-build path so scanning and transformation use the
same borrowed sources by construction.

## Native C interoperability

The dedicated `cribra-capi` adapter exposes Cribra through a native C ABI
without exposing Rust object layouts.

The ABI preserves the Rust core as semantic authority while providing explicit
ownership and destruction, UTF-8 validation, panic containment, metadata-only
result projection, scanner construction, report traversal, transformations,
and opt-in built-in composition.

C applications can consume the ABI directly. C++ applications use the same ABI
and may layer their own ownership ergonomics over it; Cribra does not maintain
a separate C++ semantic implementation.

Runnable C and C++ consumers are available under `examples/c/` and
`examples/cpp/`.

See `docs/INTEROP.md` for the native interoperability contract.

## WebAssembly

`cribra-wasm` is a typed `wasm-bindgen` adapter over the same Cribra core. It
does not route through the C ABI or maintain an independent detector
implementation.

Cribra supports two WASM build profiles: Core for scanning/reporting and
Flow for scanning/reporting plus transformations and in-memory Share Bundles.
Flow is the default and official distribution profile. Each selected profile can
produce an optimized WebAssembly artifact using Binaryen `-Oz`.

Browser consumers can use the generated URL/fetch initialization path, while
server-side JavaScript runtimes can initialize the selected artifact explicitly
from its Wasm bytes.

The production artifact has been exercised directly from a browser and from
Node.js 26.9.0, Bun 1.4.2, and Deno 2.9.7. These versions record the validated
example environment rather than define a compatibility range.

Rust/WASM parity gates cover findings, spans, metadata, candidates,
explanations, transformations, the opt-in financial built-ins, sequential batch
entries and aggregate summaries, and Flow Share Bundle content/manifests.

Runnable browser and JavaScript-runtime consumers are available under
`examples/wasm-browser/` and `examples/wasm-runtime/`.

See `docs/WASM_INTEROP.md`.

## Canonical CLI architecture

Cribra owns its command semantics.

```text
cribra
└── cribra-cli
    ├── reusable command model
    ├── parser
    ├── execution API
    ├── stable human/JSON rendering
    └── depends on cribra
```

The standalone binary is a thin adapter over the reusable library surface.
Downstream consumers may embed `cribra-cli` directly when they need canonical
Cribra CLI behavior; they should not independently reimplement the same command
semantics.

The capability relationship between the public Rust core and the canonical CLI,
including explicit parity gaps and deliberate non-parity, is documented in
[`docs/CLI_PARITY.md`](docs/CLI_PARITY.md).

## Coordinate contract

Locations use:

- `start`: zero-based UTF-8 byte offset, inclusive;
- `end`: zero-based UTF-8 byte offset, exclusive;
- `line`: one-based;
- `column`: one-based Unicode scalar position.

## Scope

Cribra owns reusable scanning and transformation semantics.

It does not own:

- repository or filesystem traversal inside the core;
- remote repository loading;
- uploads or network access;
- authentication;
- persistence;
- subscriptions;
- UI;
- application-specific policy.

## Development

Preferred local commands:

```text
just gate
just release-gate
just capi-smoke-all
just wasm-release-gate
```

Useful direct checks for the CLI:

```text
cargo check -p cribra-cli
cargo test -p cribra-cli
cargo clippy -p cribra-cli --all-targets -- -D warnings
cargo package -p cribra-cli
cargo publish -p cribra-cli --dry-run
```

See `CONTRIBUTING.md`, `SECURITY.md`, and `ROADMAP.md` for the broader project
contracts.

## Language bindings and interoperability

Cribra uses one Rust detection engine across native and cross-language
integration surfaces. Adapter availability does not imply identical packaging
or platform qualification.

| Language / platform | Integration | Status |
| --- | --- | --- |
| Rust | Native Core crate | AVAILABLE |
| C / C++ | Native C ABI | AVAILABLE |
| Swift | UniFFI Core / Flow / Fuse | AVAILABLE |
| Python | UniFFI Core / Flow / Fuse | AVAILABLE |
| Kotlin/JVM | UniFFI Core / Flow / Fuse | AVAILABLE |
| JavaScript / TypeScript | `wasm-bindgen` adapter and generated TypeScript declarations | AVAILABLE |
| TypeScript | Higher-level WebAssembly wrapper | PLANNED |
| TypeScript / React Native | UniFFI integration | PLANNED |
| C# / .NET | C ABI integration | PLANNED |
| Go | C ABI integration (cgo) | PLANNED |
| PHP | C ABI integration (FFI) | PLANNED |

The planned TypeScript wrapper is distinct from the existing typed WASM
exports. React Native, C#, Go and PHP bindings are not yet implemented or
qualified.

See [Interoperability](docs/INTEROP.md) for the implemented adapter capability
matrix and [Compatibility](docs/COMPATIBILITY.md) for versions, distribution
and qualification boundaries.

## Release status

The completed `0.4.x` line established Cribra's current detection, validation,
transformation, C ABI, WebAssembly, and downstream-consumer foundation.

Core **[v0.7.2](https://github.com/Metalymph/cribra/releases/tag/v0.7.2)**
is the latest GitHub release and is separately verified on crates.io.
It completes the P0.7 adapter conformance and release alignment.
**v0.7.1** fixed htpasswd CRLF detection. **v0.7.0** introduced incremental
performance improvements and configurable Rust/WASM profiles.

The historical **v0.6.0** release introduced
incremental scanning of revisioned UTF-8 documents, transactional edits,
selective invalidation and conservative fallback with complete whole-source
report parity.

Streaming, introduced in **v0.5.0**, remains available for explicitly admitted
configurations. Complete built-in portfolios remain unsupported for streaming,
but are supported by the incremental API.
See [the D4 evidence](docs/0.6-d4-qualification.md) and
[the D5 qualification record](docs/0.6-d5-qualification.md).

Existing Rayon execution of independent sources remains available. Swift,
Python and Kotlin/JVM integrations now have qualified consumers and local or CI
artifacts; their packaging and publication status is documented separately in
[Compatibility](docs/COMPATIBILITY.md).

CLI, C, WASM, native bindings and distribution remain independently versioned
workstreams with their own documented capability surfaces.

See `ROADMAP.md` for the architectural roadmap and the independent interface
and ecosystem workstreams.

## Road to 1.0

Cribra's released v0.7 line implements the planned core feature set: whole-source,
streaming and incremental scanning; synchronous execution and optional
parallel scanning of independent inputs; Core/Flow/Fuse Rust profiles;
Core/Flow WASM profiles; and a separately qualified native C ABI.

**Feature-complete is not the same as maturity-complete.**

Cribra v1.0 will be determined by demonstrated correctness, API stability,
portability, performance stability, integration quality and repeatable
release/security processes, rather than by adding more core features.

A v0.8 core release is not scheduled automatically. It requires a
demonstrated reason to change the core. CLI parity, C ABI, WASM and
Swift/Python/Kotlin bindings evolve independently when they can reuse
existing core capabilities. Adapter feature parity is not a prerequisite
for core v1.0.

See [Road to 1.0](ROADMAP.md#road-to-10--maturity-not-feature-expansion)
for the maturity gates and versioning policy.

## Examples

Cribra includes runnable integration examples rather than documentation-only
snippets:

- **Rust** — `examples/financial_review.rs` composes the default and opt-in
  financial packs, inspects finding metadata, and creates a redacted derivative.
- **Browser / WebAssembly** — `examples/wasm-browser/` scans and redacts
  caller-owned text locally in a dependency-free browser application.
- **Node.js / Bun / Deno** — `examples/wasm-runtime/` loads the same production
  WebAssembly artifact from a server-side JavaScript runtime.
- **C** — `examples/c/` demonstrates scanner construction, opt-in pack
  composition, report traversal, transformation, and explicit ABI cleanup.
- **C++** — `examples/cpp/` consumes the C ABI through small local move-only
  RAII wrappers without introducing a separate Cribra binding.

## License

MIT
