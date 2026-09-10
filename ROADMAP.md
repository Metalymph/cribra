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
- no requirement to send source material out of process.

The roadmap deliberately favors correctness and production reliability over
catalog breadth.

## Current release line

### v0.4.3 — High-Confidence Detection Coverage

Status: active.

Primary goal: close the remaining high-value detection gaps that can be covered
with strong structural or contextual evidence, without turning Cribra into a
generic PII/DLP classifier or an entropy-based scanner.

v0.4.3 is intended to substantially complete the current high-confidence
secret-detection surface. After this release, additional detector work should be
driven by concrete evidence rather than by catalog expansion.

### 0.4.3-A — Coverage Audit and Scope Freeze

Status: active.

- [x] Audit the complete current built-in detector catalog.
- [x] Map current deterministic and contextual detector coverage.
- [x] Audit provider-specific and generic validator boundaries.
- [x] Audit false-positive fixtures and placeholder rejection.
- [x] Audit adversarial detection coverage.
- [x] Audit collision and overlap normalization behavior.
- [x] Audit current private-key coverage.
- [x] Audit generic password, passphrase, credential and sensitive-hash
      coverage.
- [x] Confirm GitLab credentials as a significant missing provider family.
- [x] Confirm database connection credentials as a significant missing
      credential surface.
- [x] Confirm quoted passwords and passphrases containing spaces as a real
      contextual coverage gap.
- [x] Confirm HTTP Basic authentication as a high-confidence missing
      authentication surface.
- [x] Confirm PGP private keys as a high-confidence missing private-key family.
- [x] Confirm WireGuard `PrivateKey` and `PresharedKey` as high-confidence
      secret families when sufficient WireGuard context is present.
- [x] Confirm Docker registry authentication as a scoped tooling credential
      surface.
- [x] Confirm `.npmrc` registry authentication as a scoped tooling credential
      surface.
- [x] Confirm `.netrc` credentials as a scoped tooling credential surface.
- [x] Audit `/etc/shadow` and `.htpasswd` against the current
      `generic.sensitive-hash` detector.
- [x] Confirm that current sensitive-hash coverage does not cover modular
      password-verifier formats such as shadow or htpasswd records.
- [x] Identify provider-attribution ambiguity around generic field names such
      as `client_secret`.
- [x] Define deterministic attribution semantics for ambiguous provider-generic
      fields before expanding provider coverage.
- [x] Freeze the v0.4.3 detection scope.

Bare provider-generic fields such as `client_secret` are attributed to generic
credential detection unless the source provides sufficient provider-specific
evidence.

Provider attribution must follow available evidence. A generic field such as
`client_secret` must not be arbitrarily attributed to a specific provider when
the surrounding source does not establish that provider.

After this audit, the v0.4.3 detection scope is considered frozen. Additional
providers or secret families require concrete evidence of a significant,
high-confidence coverage gap and must not be added merely to increase catalog
breadth.

### 0.4.3-B — GitLab Credentials

- [x] Add a dedicated GitLab validator.
- [x] Cover stable, documented GitLab token families whose public format
      supports high-confidence structural recognition.
- [x] Evaluate documented prefixes including:
      - `glpat-`
      - `gloas-`
      - `gldt-`
      - `glrt-`
      - `glrtr-`
      - `glcbt-`
      - `glptt-`
      - `glft-`
      - `glimt-`
      - `glagent-`
      - `glwt-`
      - `glsoat-`
      - `glffct-`
- [x] Include only token families whose shape can be validated conservatively.
- [x] Reject placeholders, truncated values and malformed near-misses.
- [x] Add deterministic prefilter support.
- [x] Add provider-specific collision tests.
- [x] Document the GitLab Self-Managed custom-prefix limitation rather than
      weakening detection with broad heuristics.

GitLab support must not introduce generic opaque-token detection solely to
cover installations using custom prefixes.

### 0.4.3-C — Database Connection Credentials

- [x] Detect credentials embedded in recognized database connection URIs.
- [x] Cover PostgreSQL, MySQL, MariaDB, MongoDB and Redis URI families.
- [x] Project only the password component as the finding span.
- [x] Preserve percent-encoded source values without decoding or normalization.
- [x] Reject passwordless URIs, empty credentials, placeholders and malformed
      percent encoding.
- [x] Add deterministic prefiltering for supported database URI schemes.
- [x] Add positive and adversarial corpus coverage.

Cribra must not decode and rewrite the surrounding connection string merely to
detect a credential.

### 0.4.3-D — Quoted Passwords and Passphrases

- [x] Allow quoted password, database-password, and passphrase values to contain
      internal whitespace while preserving exact value projection.
- [x] Keep existing contextual key validation and password validation unchanged.
- [x] Preserve conservative handling for unquoted values by excluding leading and
      trailing whitespace from the projected finding span.
- [x] Do not parse configuration-language syntax or introduce quote-pairing
      semantics in the detection engine.
- [x] Treat mismatched surrounding quotes as malformed source syntax rather than
      as a reason to suppress an otherwise high-confidence credential finding.
- [x] Never extend a credential finding across a newline. Multiline quoted-value
      parsing remains out of scope; when malformed quoted syntax reaches a
      newline, an otherwise valid candidate on the current line may still be
      reported rather than suppressed.

This slice closes a grammar gap in existing password detection rather than
broadening password classification semantics.

### 0.4.3-E — Adversarial Detection Corpus

- [x] Expand false-positive fixtures for all new v0.4.3 detector families.
- [x] Add false-prefix and truncated-token cases.
- [x] Add documentation and placeholder examples.
- [x] Add provider near-misses.
- [x] Add malformed database connection strings.
- [x] Add generic field names carrying clearly non-secret values.
- [x] Add Base64-looking values outside supported authentication context.
- [x] Add hash-like values outside password-verifier context.
- [x] Add collision-heavy cases between provider-specific and generic rules.
- [x] Verify exact-span normalization remains deterministic.
- [x] Verify serial and parallel scanning remain semantically equivalent.

Adversarial coverage is a release requirement, not optional test polish.

### 0.4.3-F — HTTP Basic Credentials

- [x] Detect explicit HTTP `Authorization: Basic` credentials.
- [x] Require explicit Basic authentication context.
- [x] Validate canonical standard Base64 structure locally.
- [x] Decode locally and require a credential structure containing `:`.
- [x] Keep the finding span on the encoded credential present in the source.
- [x] Reject invalid, truncated and non-canonical Base64.
- [x] Reject unrelated Base64 data.
- [x] Reject unsupported authentication schemes.
- [x] Reject empty username/password structures and common documentation examples.
- [x] Keep generic Base64 scanning out of scope.

The detector validates only explicit HTTP Basic authentication material. Decoding is
local and used solely to verify credential structure; decoded credentials are not
exposed as finding spans. Generic Base64 data is never scanned or promoted.

This detector must not introduce generic Base64 scanning.

### 0.4.3-G — PGP Private Keys

- [x] Detect ASCII-armored PGP private-key blocks.
- [x] Require exact private-key block delimiters.
- [x] Require a nontrivial body.
- [x] Reject PGP public-key blocks.
- [x] Reject incomplete or mismatched blocks.
- [x] Use Critical severity.
- [x] Use private-key replacement remediation.
- [x] Verify collision and transformation behavior.

PGP private-key detection is limited to complete ASCII-armored private-key
blocks with exact delimiters and a nontrivial body. Public-key, incomplete,
short, and mismatched blocks remain clean. The full armored block is treated as
the sensitive span and uses the same private-key replacement semantics as the
existing PEM private-key families.

### 0.4.3-H — WireGuard Credentials

- [x] Detect WireGuard `PrivateKey` values under sufficiently strong WireGuard
      configuration context.
- [x] Detect WireGuard `PresharedKey` values under sufficiently strong
      WireGuard configuration context.
- [x] Validate the encoded key structure locally.
- [x] Decode Base64 locally where required for structural validation.
- [x] Require the expected decoded WireGuard key length.
- [x] Project only the credential value.
- [x] Reject arbitrary `PrivateKey` fields outside WireGuard context.
- [x] Reject arbitrary Base64 values.
- [x] Reject malformed, short and placeholder values.
- [x] Add realistic `[Interface]` and `[Peer]` configuration fixtures.

WireGuard credential detection is limited to explicit WireGuard configuration
semantics. `PrivateKey` is accepted only under `[Interface]`, while
`PresharedKey` is accepted only under `[Peer]`. Candidates must use canonical
standard Base64 and decode to exactly 32 bytes. Only the encoded credential
value is projected as the finding span.

WireGuard support remains contextual and does not introduce generic
`PrivateKey=<base64>` or generic Base64 scanning.

### 0.4.3-I — Docker Registry Credentials

- [ ] Detect Docker registry credentials stored in Docker configuration.
- [ ] Require sufficiently strong Docker `auths` structure.
- [ ] Recognize supported `auth` credential fields.
- [ ] Validate Base64 structure locally.
- [ ] Decode locally and require the expected credential structure.
- [ ] Project only the encoded credential value.
- [ ] Reject unrelated `auth` fields.
- [ ] Reject arbitrary Base64.
- [ ] Do not treat credential-helper names or external credential-store
      configuration as secrets.
- [ ] Add realistic Docker `config.json` fixtures.

### 0.4.3-J — npm Registry Credentials

- [ ] Detect strongly contextual npm registry credentials in `.npmrc` syntax.
- [ ] Support registry-scoped `_authToken` credentials.
- [ ] Project only the credential value.
- [ ] Reject placeholders and documentation examples.
- [ ] Reject unrelated `_authToken` text outside recognizable npm
      configuration syntax.
- [ ] Evaluate documented `_auth` and `_password` forms where structural
      validation can remain conservative.
- [ ] Do not treat `certfile` or `keyfile` path values as secret material.
- [ ] Do not introduce generic Base64 detection.
- [ ] Add realistic scoped-registry and private-registry fixtures.

### 0.4.3-K — `.netrc` Credentials

- [ ] Implement a narrow `.netrc` credential grammar.
- [ ] Recognize `machine`, `login` and `password` relationships.
- [ ] Project only password values.
- [ ] Avoid treating unrelated `password` text as `.netrc` credentials.
- [ ] Handle common whitespace and line-layout forms conservatively.
- [ ] Reject incomplete records.
- [ ] Reject documentation placeholders.
- [ ] Add realistic multi-machine fixtures.

`.netrc` support must not require broadening the generic password parser merely
to accommodate this file format.

### 0.4.3-L — System Password Verifiers

- [ ] Add dedicated structural coverage for high-confidence system password
      verifier formats where justified.
- [ ] Cover common `/etc/shadow` record forms conservatively.
- [ ] Evaluate supported modular crypt families such as:
      - SHA-512 crypt;
      - SHA-256 crypt;
      - yescrypt;
      - bcrypt where applicable.
- [ ] Cover common `.htpasswd` verifier families where structure is
      sufficiently distinctive.
- [ ] Evaluate Apache APR1.
- [ ] Require appropriate record or field context.
- [ ] Do not classify arbitrary `$...$` strings as password verifiers.
- [ ] Do not classify arbitrary hashes as sensitive material.
- [ ] Keep generic checksum and package-integrity fixtures clean.
- [ ] Define remediation semantics appropriate for password verifiers rather
      than treating them as plaintext credentials.

This slice is separate from `generic.sensitive-hash`. The existing generic rule
intentionally recognizes contextual hexadecimal digests only and must not be
expanded into an unrestricted password-hash detector.

### 0.4.3-M — Cross-Interface Semantic Parity

- [ ] Verify every new built-in rule through the Rust interface.
- [ ] Verify public metadata for every new built-in rule.
- [ ] Verify findings and exact source spans through the C interface.
- [ ] Verify findings and exact source spans through the WebAssembly interface.
- [ ] Verify severity, confidence, rule identifiers and remediation metadata
      remain semantically equivalent.
- [ ] Verify serial and parallel Rust scanning remain equivalent.
- [ ] Verify deterministic ordering remains stable across interfaces.
- [ ] Verify transformations behave consistently for all newly supported
      finding families.

C and WebAssembly adapters must remain projections of the same Cribra semantics,
not independent detector implementations.

### 0.4.3-N — Documentation and Release Gate

- [ ] Update README detector coverage.
- [ ] Update CHANGELOG.
- [ ] Update security documentation where new credential families affect the
      documented threat model.
- [ ] Ensure roadmap scope and completed work match the shipped release.
- [ ] Run formatting checks.
- [ ] Run Clippy with all targets and features.
- [ ] Run workspace tests.
- [ ] Run all-feature tests.
- [ ] Run documentation tests.
- [ ] Run MSRV validation.
- [ ] Run RustSec audit.
- [ ] Run package validation.
- [ ] Run publish dry-run.
- [ ] Run C ABI header, smoke, symbol and static-library gates.
- [ ] Run WebAssembly adapter, parity and package gates.
- [ ] Verify a clean working tree before release.
- [ ] Publish `cribra` v0.4.3 only after the release branch is merged into the
      authoritative `main`.

## After v0.4.3

No feature release is currently scheduled.

After v0.4.3, Cribra enters a maintenance and security-hardening phase.

Expected work includes:

- bug fixes;
- security fixes;
- false-positive reduction;
- false-negative fixes backed by concrete evidence;
- compatibility fixes;
- performance improvements supported by measurement;
- documentation improvements;
- maintenance of Rust, C and WebAssembly parity;
- updates for major provider credential formats when upstream formats change;
- narrowly scoped new detectors when a significant production gap is
  demonstrated.

A future release number or feature line should not be created merely because
v0.4.3 has shipped.

## Explicit Non-Goals

Cribra is not intended to become a broad DLP, PII classification or content
classification platform.

The following are outside the current roadmap unless the project receives new,
concrete requirements that justify revisiting them:

- generic entropy-based secret detection;
- arbitrary Base64 detection;
- arbitrary hexadecimal or cryptographic hash detection;
- broad PII classification;
- email-address detection;
- phone-number detection;
- payment-card/PAN scanning;
- IBAN, BIC, SWIFT or generic bank-account classification;
- generic financial-data classification;
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

Cribra should prefer a deliberate false negative over a high-noise detector when
the available evidence is insufficient to distinguish secret material from
ordinary data.

## Historical Releases

### v0.4.2 — Security Hardening

Status: completed.

v0.4.2 strengthened Cribra's safe-processing boundary and expanded
high-confidence secret coverage without changing the core architecture.

Completed work included:

- atomic scan-and-build construction for share-safe bundles;
- documentation of exact source/report pairing requirements for lower-level
  transformation paths;
- transformation-to-rescan semantic safety regression coverage;
- generic PKCS#8 private-key detection;
- encrypted PKCS#8 private-key detection;
- RSA private-key detection;
- EC private-key detection;
- OpenSSH private-key detection;
- HTTP Authorization Bearer detection;
- GCP JSON-escaped private-key detection;
- separation of literal multiline and JSON-escaped GCP private-key forms;
- documentation and security-boundary alignment;
- complete Rust, C ABI and WebAssembly release validation.

### v0.4.1 — WebAssembly Publication

Status: completed.

v0.4.1 published the WebAssembly adapter with production packaging and semantic
parity validation.

Completed work included:

- typed wasm-bindgen interface;
- browser-oriented WebAssembly packaging;
- optimized production artifact generation;
- semantic parity between native Rust and WebAssembly scanning;
- deterministic browser benchmark coverage;
- Web Worker validation;
- release packaging and Trusted Publishing support;
- correction of native-64 versus wasm32 coordinate-dependent behavior.

### v0.4.0 — WebAssembly Interoperability

Status: completed.

v0.4 established WebAssembly as a supported interoperability surface while
preserving the Rust engine as the semantic authority.

The WebAssembly adapter does not maintain an independent detector
implementation.

### v0.3 — Native Interoperability

Status: completed.

v0.3 introduced the dedicated native C interoperability adapter.

The C ABI is maintained as a separate interoperability surface with its own
compatibility and release considerations. It remains a projection of the Cribra
core semantics rather than an independent scanner implementation.

### v0.2 — Detection and Transformation Baseline

Status: completed.

v0.2 established the core detection, contextual-validation, ambiguity,
explainability and safe-transformation contracts that later releases harden and
extend.