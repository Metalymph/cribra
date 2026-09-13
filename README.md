# Cribra

Embeddable, local-first Rust engine for detecting, reviewing, and safely
transforming secrets and sensitive data.

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

The current built-in catalog covers high-confidence families including:

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
- registry-scoped npm credentials;
- `.netrc` credentials;
- supported `/etc/shadow` password verifiers;
- supported `.htpasswd` password verifiers;
- contextual sensitive hashes;
- generic API keys, tokens, and secrets.

The canonical selectable built-in pack is `builtins::CURRENT`.

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

The dedicated `cribra-capi` adapter exposes a stable-designed native protocol
without exposing Rust object layouts.

Important invariants include explicit ownership, explicit destruction, UTF-8
validation, panic containment, metadata-only result projection, and semantic
parity with the Rust core.

See `docs/INTEROP.md`.

## WebAssembly

`cribra-wasm` is a typed `wasm-bindgen` adapter over the same Cribra core.

It does not route through the C ABI and does not maintain an independent
detector implementation.

The production browser artifact uses Binaryen `-Oz`. Rust/WASM parity gates
cover findings, spans, metadata, candidates, explanations, and supported
transformations.

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

`0.4.4` adds the canonical reusable Cribra CLI without changing the core
detection architecture. The project remains on `0.4.x` while changes are
additive and preserve the current public semantic contracts.

A future `0.5` is reserved for a deliberate architectural or public-contract
change significant enough to justify a new minor line.

## License

MIT
