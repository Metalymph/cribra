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

### v0.4.5 — Developer Ecosystem Credential Coverage

Status: active.

Goal: consolidate the remaining high-value developer, package, runtime, build,
and systems credential surfaces into one evidence-driven release rather than
spreading additive detector work across several releases.

#### 0.4.5-A — Scope and coverage audit

Status: completed.

- [x] Audit high-value developer/package credential surfaces.
- [x] Prefer documented credential storage and configuration contracts.
- [x] Reuse generic, `.netrc`, HTTP-authentication, and provider detectors where
  they already provide equivalent semantic coverage.
- [x] Reject ecosystem-specific classification when it would add catalog breadth
  without stronger semantics.
- [x] Keep network validation and new parser/runtime dependencies out of scope.

#### 0.4.5-B — Developer package credentials

Status: active.

- [x] Cargo / Rust registry authentication.
- [x] Python / PyPI / `.pypirc` repository tokens.
- [ ] RubyGems credentials, including `GEM_HOST_API_KEY`, with deterministic
  collision handling against generic credential rules.
- [ ] NuGet / .NET cleartext package-source credentials.
- [ ] Maven repository credentials.
- [ ] Deno authentication tokens.

Audited surfaces may be rejected or deferred when they cannot satisfy Cribra's
confidence standard. Gradle, Go, Conan, vcpkg, Nix, and related ecosystems do
not require dedicated rules merely for catalog coverage when existing generic
or shared credential surfaces already provide equivalent semantics.

#### 0.4.5-C — Collision and normalization hardening

Status: pending.

- [ ] Verify provider/ecosystem-specific rules deterministically win valid
  collisions with generic credential rules.
- [ ] Preserve exact finding spans and stable rule attribution.
- [ ] Verify contextual prefilters remain consistent with validator semantics.
- [ ] Keep rule-ID and detection-mode contracts explicit as validator families
  expand.

#### 0.4.5-D — Adversarial corpus and false-positive hardening

Status: pending.

- [ ] Add positive, negative, malformed, placeholder, documentation, and
  cross-format fixtures for every accepted family.
- [ ] Add collision regressions for ecosystem-specific versus generic rules.
- [ ] Reject protected references, helper names, paths, registry names, and
  credential-store references that are not credential material.
- [ ] Preserve Cribra's preference for deliberate false negatives over noisy
  classification.

#### 0.4.5-E — Interface and parity validation

Status: pending.

- [ ] Validate Rust behavior for every accepted rule.
- [ ] Validate native C ABI exposure and semantic parity.
- [ ] Align `cribra-wasm` with the completed v0.4.5 core semantics.
- [ ] Validate WebAssembly parity for findings, spans, severity, confidence,
  remediation, candidates, and explanations.

#### 0.4.5-F — Cribra CLI distribution

Status: pending.

Goal: make the canonical `cribra` CLI straightforward to install on common
developer systems without changing CLI semantics or moving package-manager
policy into the core engine.

- [ ] Add Homebrew distribution/install support for `cribra-cli`.
- [ ] Add Debian/Ubuntu APT distribution/install support for `cribra-cli`.
- [ ] Keep Homebrew and APT packaging as distribution adapters over the same
  canonical `cribra` executable.
- [ ] Document installation, upgrade, and uninstall paths.
- [ ] Validate packaged binaries against the canonical CLI behavior and release
  version.

#### 0.4.5-G — Documentation and release gate

Status: pending.

- [ ] Update README, ROADMAP, CHANGELOG, rule documentation, and public coverage
  descriptions.
- [ ] Run formatting, workspace check/test, Clippy, docs, MSRV, RustSec, and
  packaging gates.
- [ ] Run C ABI release gates.
- [ ] Run WebAssembly adapter and parity gates.
- [ ] Run `cribra-cli` packaging/distribution validation, including Homebrew and
  APT artifacts.
- [ ] Verify a clean working tree and protected-main release workflow.
- [ ] Publish/tag v0.4.5 only after all release gates pass.

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

### v0.4.4 — Canonical Cribra CLI

Status: completed.

v0.4.4 established Cribra as the authoritative owner of its command-line
interface through the reusable `cribra-cli` crate and thin `cribra` executable.

Completed work included:

- canonical reusable command, input, execution, and output contracts;
- explicit UTF-8 file and stdin input with exact source preservation;
- deterministic human and JSON metadata-only output;
- stable `0`/`1`/`2` process exit semantics;
- privacy-safe diagnostics with no matched secret or candidate value leakage;
- executable integration and adversarial CLI coverage;
- zero new CLI framework, serialization, argument-parsing, or error dependency;
- documentation and package validation;
- full workspace, Clippy, docs, MSRV, RustSec, C ABI, and WebAssembly parity
  release gates;
- protected-main publication of `cribra-cli` and Cribra v0.4.4.

Filesystem traversal remains outside the core engine and was not required for
the canonical v0.4.4 CLI contract.


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
