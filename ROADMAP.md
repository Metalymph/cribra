# Cribra Roadmap

Cribra is an application-agnostic, local-first secret and sensitive-material
detection engine.

The project prioritizes:

- deterministic behavior;
- high-confidence detection;
- low false-positive rates;
- exact source-span preservation;
- explainable findings;
- safe transformations;
- semantic parity across supported interfaces;
- no requirement to send source material out of process;
- a small, stable public surface with minimal dependencies.

The roadmap deliberately favors correctness, compatibility, and production
reliability over catalog breadth.

## Release-line policy

Cribra remains on the `0.4.x` release line while changes are additive and
preserve the current core architecture and public semantic contracts.

A future `0.5` release is reserved for a deliberate architectural or
public-contract change significant enough to justify a new minor line. New
detectors, hardening, CLI work, bindings, packaging, and additive
interoperability work do not by themselves require `0.5`.

## Current release line

### v0.4.4 — Canonical Cribra CLI

Status: active.

v0.4.4 establishes Cribra as the authoritative owner of its command-line
interface.

The release adds a reusable Rust CLI library plus a thin standalone `cribra`
executable while keeping detection, validation, findings, remediation, and
transformations authoritative in the `cribra` core.

The CLI is deliberately small and dependency-light. It must not become a second
scanner implementation or an application-specific policy layer.

Target structure:

```text
crates/
├── cribra/          # core engine
├── cribra-cli/      # reusable CLI library + `cribra` executable
└── cribra-wasm/     # WebAssembly adapter
```

Architectural ownership:

```text
cribra
└── semantic authority

cribra-cli
├── reusable command model
├── parser
├── execution surface
├── stable output contracts
└── thin `cribra` binary
```

Downstream products may embed `cribra-cli` when they need the canonical command
surface, but they must not reimplement Cribra CLI semantics independently.

Cribra itself contains no knowledge of Silens or any other downstream product.

### 0.4.4-A — CLI Foundation

Status: completed.

- [x] Add the reusable `cribra-cli` crate.
- [x] Add the standalone `cribra` executable as a thin wrapper around the
  reusable CLI library.
- [x] Keep command semantics independent from downstream products.
- [x] Preserve Cribra as the sole authority for its CLI behavior.
- [x] Avoid CLI framework dependencies unless they provide clear value over a
  small internal parser.
- [x] Keep the initial `cribra-cli` dependency surface limited to Cribra itself.

### 0.4.4-B — Input Contract

Status: completed.

- [x] Support one explicit file input.
- [x] Support stdin via `-`.
- [x] Define deterministic UTF-8 input behavior.
- [x] Preserve exact source bytes after UTF-8 validation.
- [x] Preserve CRLF and Unicode input without normalization.
- [x] Reject invalid UTF-8.
- [x] Preserve exact scanner semantics from the Cribra core.
- [x] Keep filesystem traversal, discovery, globbing, and repository walking
  outside the initial CLI contract.

### 0.4.4-C — Output Contract

Status: completed.

- [x] Add stable human-readable output.
- [x] Add machine-readable JSON output.
- [x] Preserve finding, candidate, and review-channel semantics.
- [x] Report source identity, status, counts, locations, rule IDs, severity,
  confidence, remediation, candidate kind, and candidate evidence using only
  public Cribra metadata.
- [x] Ensure matched secret material is never rendered.
- [x] Define the process exit-code contract:
  - `0` for successful command execution, including clean, review-only, and
    finding-bearing scans;
  - `1` for input/execution failures;
  - `2` for command-line usage or parse errors.
- [x] Keep output deterministic for equivalent inputs.

### 0.4.4-D — Reusable Command Surface

Status: completed.

- [x] Expose a reusable public `Command` model.
- [x] Expose `ScanCommand`, `ScanInput`, and `OutputFormat`.
- [x] Expose reusable command parsing.
- [x] Expose reusable execution without subprocess invocation.
- [x] Keep the standalone binary as a thin process adapter.
- [x] Separate command parsing, input acquisition, execution, rendering, and
  process I/O responsibilities.
- [x] Keep `ScanReport`, `Finding`, `SensitiveCandidate`, and detection authority
  owned by the Cribra core rather than duplicating them in the CLI layer.
- [x] Keep public CLI errors explicit while retaining internal input-acquisition
  details as implementation concerns where possible.
- [x] Prefer borrowed access such as `&str` where ownership is unnecessary while
  retaining owned `String`/`PathBuf` values where command or output lifetime
  requires ownership.
- [x] Add no new third-party CLI, error, serialization, or argument-parsing
  dependency.

### 0.4.4-E — CLI Hardening

Status: completed.

- [x] Add integration tests against the real `cribra` executable.
- [x] Add explicit file-input integration coverage.
- [x] Add stdin integration coverage.
- [x] Add stdin/file semantic-equivalence coverage.
- [x] Add human-output contract coverage.
- [x] Add JSON-output contract coverage.
- [x] Add clean, review-only, and finding-bearing scan coverage.
- [x] Add exit-code regression coverage for `0`, `1`, and `2`.
- [x] Verify privacy-safe failure behavior.
- [x] Verify invalid UTF-8 produces a stable privacy-safe diagnostic.
- [x] Verify missing-file failures do not expose source material.
- [x] Verify finding output does not contain matched secret values.
- [x] Verify candidate output does not contain candidate source values.
- [x] Verify the standalone binary remains only a thin adapter over the reusable
  library.

### 0.4.4-F — Documentation, Packaging, and Release Gate

Status: active.

- [x] Update the root README with canonical CLI installation and usage.
- [x] Add `cribra-cli` package documentation.
- [x] Document file/stdin behavior, UTF-8 requirements, output formats, and exit
  codes.
- [x] Document the reusable Rust command surface.
- [x] Document the privacy boundary for CLI output and diagnostics.
- [x] Ensure ROADMAP and CHANGELOG match the intended release.
- [ ] Run formatting checks.
- [ ] Run workspace checks.
- [ ] Run workspace tests.
- [ ] Run Clippy with all relevant targets/features.
- [ ] Run documentation tests.
- [ ] Run MSRV validation.
- [ ] Run RustSec audit.
- [ ] Run `cribra-cli` package validation.
- [ ] Run `cribra-cli` publish dry-run.
- [ ] Run existing C ABI release gates.
- [ ] Run existing WebAssembly adapter and parity gates.
- [ ] Verify a clean working tree before release.
- [ ] Merge through the protected `main` workflow.
- [ ] Publish `cribra-cli` only after the authoritative release commit is on
  `main`.
- [ ] Tag and publish Cribra v0.4.4 only after all release gates pass.

Directory or repository traversal may be added later under Cribra CLI authority
if a concrete use case justifies it. Downstream consumers must not
independently fork CLI semantics merely to add traversal convenience.

## After v0.4.4

The post-v0.4.4 roadmap remains additive and stability-first.

The core architecture, public result model, transformation model, C ABI
principles, and WebAssembly semantic authority are considered stable unless
concrete evidence justifies an architectural change.

### v0.4.5 — Developer Ecosystem Credential Coverage

Goal: consolidate the remaining high-value developer, package, runtime, build,
and systems credential surfaces into one evidence-driven release rather than
spreading additive detector work across several releases.

Primary audit candidates:

- Cargo / Rust registry authentication;
- Python / PyPI / `.pypirc`;
- Deno authentication tokens;
- RubyGems credentials where sufficiently deterministic;
- Maven repository authentication;
- Gradle repository credentials;
- NuGet / .NET package-source credentials;
- additional JVM or .NET credential surfaces only where strongly contextual;
- Nix access tokens;
- Go module and proxy authentication;
- Conan, vcpkg, and other C/C++ package-tool credentials;
- common Unix and systems developer tooling with documented credential formats;
- security and infrastructure tooling only where a concrete high-confidence
  credential surface is identified.

Principles:

- prefer documented credential storage and configuration surfaces;
- reuse existing generic detectors where they already provide equivalent
  semantic coverage;
- do not create ecosystem-specific rules merely for catalog breadth;
- project only credential material;
- keep network validation out of scope;
- distinguish exposed credentials from encrypted/protected references;
- do not classify paths, provider names, helper names, or credential-store
  references as secrets;
- reuse `.netrc`, HTTP authentication, and existing provider detectors where
  appropriate;
- require adversarial false-positive coverage for every accepted family;
- reject audited candidates that cannot satisfy Cribra's confidence standard;
- preserve Rust, C, and WebAssembly semantic parity for every accepted rule;
- introduce no architectural changes to the core.

v0.4.5 is intended as a consolidated enrichment release. It does not imply that
every audited ecosystem will receive a dedicated detector.

### v0.4.6 — Native Language Bindings

Goal: provide ergonomic bindings for high-value native/security ecosystems
without expanding or weakening the Cribra core.

Initial targets:

- Swift;
- Kotlin;
- Python.

Principles:

- bindings live outside the Cribra core crate;
- bindings project existing Cribra semantics rather than implementing detection;
- the native C ABI remains the preferred interoperability foundation where
  appropriate;
- binding packages own language ergonomics, packaging, and lifecycle only;
- no matched source value is added to public metadata;
- ownership, lifetime, error, and destruction behavior must be explicit;
- parity tests must verify findings, locations, severity, confidence,
  remediation, candidates, explanations, and supported transformations;
- dependencies must remain minimal and justified.

No dedicated JavaScript/TypeScript binding is planned while `cribra-wasm`
provides the portable browser/JavaScript WebAssembly boundary adequately.
A separate JS/TS binding should be considered only if a concrete runtime cannot
use the existing WASM contract safely or ergonomically.

### v0.4.7 — Financial Data Pack (Opt-In)

Goal: add the first optional sensitive-data pack outside the default
secret/credential baseline.

Research candidates:

- IBAN with structural and checksum validation;
- payment-card PAN with structural and checksum validation;
- other financial identifiers only where validation is sufficiently strong.

Principles:

- financial detection is explicitly opt-in;
- `builtins::CURRENT` remains focused on secrets and credentials;
- BIC/SWIFT or generic account numbers are not findings merely by appearance;
- no broad numeric-pattern scanning;
- checksum validation is necessary where applicable but is not by itself
  sufficient reason to classify unrelated numeric data;
- exact source spans remain authoritative;
- false-positive resistance is a release requirement;
- financial detection must not turn Cribra into a general PII/DLP classifier;
- Rust, C, and WebAssembly behavior must remain semantically equivalent where
  the pack is exposed.

## Maintenance and Hardening

Alongside planned feature releases, Cribra continues continuous maintenance and
security hardening.

Expected work includes:

- bug fixes;
- security fixes;
- false-positive reduction;
- false-negative fixes backed by concrete evidence;
- compatibility fixes;
- performance improvements supported by measurement;
- documentation improvements;
- maintenance of Rust, C, WebAssembly, CLI, and future binding parity;
- updates for major provider credential formats when upstream formats change;
- narrowly scoped new detectors when a significant production gap is
  demonstrated.

Feature scope remains evidence-driven and may be reduced, deferred, or
reordered when an audited candidate cannot satisfy Cribra's confidence
requirements.

## Explicit Non-Goals

Cribra is not intended to become a broad DLP, PII classification, or content
classification platform.

The following remain outside the roadmap unless new concrete requirements
justify revisiting them:

- generic entropy-based secret detection;
- arbitrary Base64 detection;
- arbitrary hexadecimal or cryptographic hash detection;
- broad PII classification;
- email-address detection;
- phone-number detection;
- payment-card/PAN scanning in the default built-in secret pack;
- IBAN, BIC, SWIFT, or generic bank-account classification in the default
  built-in secret pack;
- generic financial-data classification without explicit opt-in;
- arbitrary Kubernetes `data:` or `Secret` Base64 scanning;
- generic encrypted-blob detection;
- DPAPI blob detection;
- public certificate detection as secret material;
- AI or model-based secret classification;
- provider network calls to validate whether credentials are active;
- cloud SDK dependencies for credential verification;
- filesystem or repository traversal inside the core engine;
- application-specific policy engines;
- speculative plugin architecture;
- adding provider detectors solely to increase the number of supported
  services.

Cribra should prefer a deliberate false negative over a high-noise detector
when the available evidence is insufficient to distinguish sensitive material
from ordinary data.

## Historical Releases

### v0.4.3 — High-Confidence Detection Coverage

Status: completed.

v0.4.3 substantially completed the current high-confidence secret-detection
baseline while preserving conservative false-positive requirements and semantic
parity across Rust, C, and WebAssembly interfaces.

Completed work included:

- GitLab credential-family coverage;
- database connection passwords for PostgreSQL, MySQL, MariaDB, MongoDB, and
  Redis;
- quoted password/passphrase support with exact span preservation;
- HTTP Basic credentials;
- ASCII-armored PGP private keys;
- contextual WireGuard credentials;
- Docker registry credentials;
- npm registry credentials;
- `.netrc` credentials;
- `/etc/shadow` and `.htpasswd` password verifiers;
- dedicated password-verifier remediation semantics;
- adversarial corpus expansion;
- Rust/C/WebAssembly parity validation;
- release documentation, packaging, and publication gates.

Provider attribution remains evidence-driven. Bare provider-generic fields such
as `client_secret` are attributed generically unless surrounding context
establishes a specific provider.

### v0.4.2 — Security Hardening

Status: completed.

v0.4.2 strengthened Cribra's safe-processing boundary and expanded
high-confidence secret coverage without changing the core architecture.

Completed work included atomic scan-and-build share-safe construction, stronger
private-key coverage, contextual HTTP Bearer detection, JSON-escaped GCP
private-key detection, and complete Rust/C/WASM release validation.

### v0.4.1 — WebAssembly Publication

Status: completed.

v0.4.1 published the WebAssembly adapter with production packaging, semantic
parity validation, deterministic browser benchmarking, and Trusted Publishing.

### v0.4.0 — WebAssembly Interoperability

Status: completed.

v0.4 established WebAssembly as a supported interoperability surface while
preserving the Rust engine as the semantic authority.

### v0.3 — Native Interoperability

Status: completed.

v0.3 introduced the dedicated native C interoperability adapter and stable
interoperability contracts.

### v0.2 — Detection and Transformation Baseline

Status: completed.

v0.2 established the core detection, contextual-validation, ambiguity,
explainability, and safe-transformation contracts that later releases harden
and extend.
