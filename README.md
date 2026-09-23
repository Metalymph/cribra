# Cribra

Embeddable, local-first Rust engine for detecting, classifying, and safely transforming secrets and sensitive data. Use it directly as a Rust crate, through the CLI as a process boundary, via C ABI or WebAssembly, or through language-specific bindings. Integrate it into scripts and pipelines, use it inside APIs before data is stored, logged, or transmitted, or build complete security and data-handling products on top of it.

*Cribra provides the primitives; the surrounding tool decides the workflow*ch

Cribra is application-agnostic. Callers provide UTF-8 text and retain control
of I/O, storage, persistence, presentation, and policy. The core owns detection,
validation, deterministic normalization, metadata-only reporting, and explicit
share-safe transformations.

Matched secret values are not stored in public findings.

## Capabilities

- deterministic UTF-8 scanning;
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

## Privacy boundary

Cribra performs no network access.

The caller owns source acquisition and destination policy. Public findings,
review candidates, explanations, summaries, and serialized metadata contain
coordinates and classification metadata rather than copies of matched source
material.

Transformations are explicit operations and require caller-owned source input.

## Install

### Rust core

```toml
[dependencies]
cribra = "0.4"
```

Optional features:

```toml
cribra = { version = "0.4", features = ["serde", "parallel"] }
```

| Feature | Default | Purpose |
| --- | --- | --- |
| `serde` | no | Serialize supported public data contracts |
| `parallel` | no | Scan independent inputs concurrently with Rayon |

### Canonical CLI

```text
cargo install cribra-cli
```

The installed executable is `cribra`.

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

Cribra separates classified findings from ambiguous review candidates.

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

Additional built-in families may be exposed through explicit opt-in packs rather
than broadening the default scanner. The financial pack is available as
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

For findings, explanation is resolved against scanner-owned rule metadata. For
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

Cribra produces one optimized WebAssembly artifact using Binaryen `-Oz`.
Browser consumers can use the generated URL/fetch initialization path, while
server-side JavaScript runtimes can initialize the same artifact explicitly
from its Wasm bytes.

The production artifact has been exercised directly from a browser and from
Node.js 26.9.0, Bun 1.4.2, and Deno 2.9.7. These versions record the validated
example environment rather than define a compatibility range.

Rust/WASM parity gates cover findings, spans, metadata, candidates,
explanations, transformations, and the opt-in financial built-ins.

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

## Release status

The active release line is `0.4.x`.

The current development line extends Cribra conservatively through additive
detectors, opt-in detection packs, interoperability improvements, and
downstream-consumer readiness while preserving the existing public semantic
contracts.

A future `0.5` is reserved for a deliberate architectural or public-contract
change significant enough to justify a new minor line.

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
