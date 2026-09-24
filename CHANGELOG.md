# Changelog

All notable changes to Cribra are documented in this file.

Cribra follows semantic versioning from its first public release. Entries focus
on externally meaningful behavior, security properties, interoperability
contracts, and validated integration surfaces rather than internal development
history.

## [Unreleased]

## [0.4.7] - 2026-09-24

Cribra 0.4.7 completes the currently planned broad detector-expansion phase
with high-confidence personal-data, financial, OTP, Tailscale, NATS, and
system-security coverage, while preserving conservative classification,
deterministic ownership, and explicit opt-in boundaries for sensitive-data
catalogs.

### Added

- Added the opt-in `builtins::personal::CURRENT` catalog for structured
  personal identifiers.
- Added `personal.it-codice-fiscale` detection for Italian natural-person
  Codice Fiscale values, including structurally valid omocodic
  representations, positional semantics, encoded birth data, birthplace-code
  structure, and control-character validation.
- Added `personal.pl-pesel` detection with encoded century/date semantics,
  Gregorian date validation, and checksum validation.
- Added contextual `personal.uk-nhs-number` detection for compact and
  canonical 3-3-4 NHS Number representations with Modulus 11 validation.
- Added contextual `personal.us-ssn` detection for compact and canonical
  `AAA-GG-SSSS` representations with structural impossibility checks.
- Added `financial.card-verification-code` to the opt-in financial catalog for
  three- and four-digit payment-card verification codes under explicit
  card-verification context.
- Added contextual `mfa.otp-provisioning-secret` detection for TOTP/HOTP
  provisioning material, including `otpauth` provisioning and explicit
  OTP-secret configuration contexts.
- Added deterministic NATS NKey seed and encoded private-key detection with
  structural decoding, supported key-family validation, and checksum
  validation.
- Added dedicated Tailscale credential coverage for API access tokens, auth
  keys, OAuth client secrets, SCIM keys, and webhook keys.
- Added native C ABI composition of the personal-data catalog through
  `cribra_builder_add_personal_builtins`.
- Added WebAssembly composition of the personal-data catalog through
  `ScanEngineBuilder.addPersonalBuiltins()`.
- Added `docs/COVERAGE.md` as the canonical human-readable coverage,
  ownership, transitive-coverage, deferred-category, and refused-category
  manifest.

### Changed

- Completed the planned broad detector-expansion phase. Future built-in
  detector additions are evidence-driven rather than routine catalog growth.
- Formalized `DIRECT`, `TRANSITIVE`, `DEFERRED`, and `REFUSED` coverage states
  for reasoning about detector ownership and gaps.
- Kept personal and financial sensitive-data catalogs explicitly opt-in;
  `builtins::CURRENT` remains the default security and credential portfolio.
- Expanded the default security portfolio with authoritative OTP, NATS NKey,
  and Tailscale credential coverage.
- Strengthened semantic ownership and collision policy across generic,
  provider, ecosystem, financial, personal, and system-security rules.
- Preserved the distinction between credentials, private material, password
  verifiers, sensitive identifiers, public identifiers, and security-related
  configuration.
- Clarified Cribra's integration model: the engine provides application-
  agnostic detection, classification, explanation, and transformation
  primitives while surrounding tools retain workflow and policy authority.
- Established post-v0.4.7 development priorities around correctness,
  false-positive and evidence-backed false-negative hardening, performance,
  reliability, interface parity, compatibility, CLI/FFI maturity, bindings,
  packaging, distribution, and downstream integration requirements.

### Security

- Personal identifiers require structural or contextual authority appropriate
  to each representation and do not claim registry assignment, holder
  identity, or external record existence.
- NHS Number and US SSN classification require explicit identifier-specific
  context; structurally plausible bare numeric values are deliberately
  insufficient.
- Card verification codes require explicit payment-card verification context;
  bare three- or four-digit numeric values are not classified.
- OTP provisioning detection exposes only the shared secret as the sensitive
  span and rejects arbitrary bare Base32 material.
- NATS NKey secret detection validates encoded structure, supported key
  families, and checksums while deliberately excluding public NKeys from the
  sensitive-secret contract.
- Tailscale credentials retain dedicated ownership for authoritative
  credential formats rather than relying on weaker generic classification.
- Public SSH keys, `authorized_keys`, `known_hosts`, TLS certificates, public
  NATS NKeys, public blockchain addresses, and other intentionally public
  identifiers remain outside the sensitive-secret contract.
- Generic email addresses, phone numbers, physical addresses, arbitrary
  high-entropy blobs, and generic configuration values are not promoted to
  Findings merely because they are recognizable.
- Detection remains local and does not claim external ownership, account
  activity, compromise state, registry presence, or network authority.

### Transformation

- Verified redact, template, pseudonymize, and synthesize behavior across the
  applicable v0.4.7 detector portfolio.
- Added deterministic synthesis semantics for newly supported structured
  personal identifiers while preventing synthesized values from remaining
  valid instances of the corresponding detector.
- Added invalid-by-construction synthesis for payment-card verification codes
  while preserving source width.
- Preserved deterministic transformation behavior and metadata-only,
  secret-safe public boundaries.

### Interoperability

- Rust remains the sole semantic authority for the new detector families.
- Native C ABI and WebAssembly consumers can explicitly compose the
  personal-data catalog without reimplementing detection semantics.
- C ABI semantic projections preserve the applicable v0.4.7 finding,
  remediation, contextual-detection, and explanation behavior.
- WebAssembly preserves the same catalog composition and detector semantics as
  the Rust core.
- Prepared `cribra-wasm` v0.4.5 with Cribra v0.4.7 semantic parity and the
  new personal-data catalog composition surface.
- Personal and financial opt-in catalogs compose with the default portfolio
  while retaining distinct rule ownership and stable rule IDs.

### Validation

- Added positive, negative, malformed, placeholder, documentation, boundary,
  exact-span, and adversarial regressions for the accepted v0.4.7 detector
  families.
- Added collision coverage across generic, provider, package-ecosystem,
  financial, personal, and system-security detectors.
- Added transformation regressions ensuring synthesized sensitive identifiers
  are not rediscovered as valid instances of their originating rules.
- Added Rust, C ABI, and WebAssembly semantic-parity coverage for applicable
  v0.4.7 capabilities.
- Re-audited the complete built-in and opt-in portfolio and documented
  materially relevant omitted, transitive, deferred, and refused categories.
- Verified the full Rust, native C ABI, WebAssembly, MSRV, dependency-audit,
  packaging, and publication dry-run release gates.

## [0.4.6] - 2026-09-19

Cribra 0.4.6 adds the first opt-in sensitive-data family
without broadening Cribra's default security and credential catalog.

### Added

- Added the opt-in `builtins::financial::CURRENT` catalog.
- Added `financial.iban` detection for compact electronic IBAN values using
  registered-country structure, exact country length, character constraints,
  check-digit structure, and MOD-97 validation.
- Added `financial.pan` detection for compact payment-card PAN values using
  numeric structure, supported 10–19 digit length, Luhn validity, and strong
  payment-card context.
- Added native C ABI composition of the financial catalog through
  `cribra_builder_add_financial_builtins`.
- Added WebAssembly composition of the financial catalog through
  `ScanEngineBuilder.addFinancialBuiltins()`.
- Added runnable Rust, browser/WebAssembly, JavaScript-runtime, C, and C++
  integration examples.
- Added concrete WebAssembly runtime examples validated with Node.js, Bun, and
  Deno.
- Added a native C example covering builder composition, scanning, finding
  traversal, redaction, and explicit ownership cleanup.
- Added a C++ example demonstrating application-local move-only RAII wrappers
  over the same public C ABI.

### Changed

- Established explicit built-in-pack composition as the sensitive-data
  extension model: `builtins::CURRENT` remains the default security and
  credential baseline while financial recognition is opt-in.
- Kept financial findings inside the existing `Finding`, metadata,
  explainability, remediation, and transformation contracts rather than adding
  a separate public sensitive-data result model.
- Kept `RuleKind` focused on matching strategy rather than using it as a
  sensitive-data taxonomy.
- Kept `SensitiveCandidate` reserved for deliberately review-only ambiguity;
  structurally invalid IBAN or PAN values are rejected rather than downgraded
  automatically into candidates.
- Kept generic redaction, templating, pseudonymization, and deterministic
  synthesis report-driven and reusable for financial findings without
  financial-specific transform paths.
- Documented financial detection as local recognition only: findings do not
  establish ownership, issuer or account assignment, account existence,
  activity, authorization, or compromise.
- Updated the root documentation to distinguish the default catalog from
  explicit opt-in catalogs and to point to executable integration examples.
- Replaced the historical native-interoperability design proposal with the
  implemented interoperability contract covering semantic authority, native
  ownership, consuming builder semantics, borrowed views, C/C++ consumption,
  WebAssembly composition, privacy boundaries, and cross-adapter invariants.
- Documented browser and server-side JavaScript consumption as two
  initialization paths over the same production WebAssembly artifact.

### Security

- PAN classification requires strong payment-card context in addition to
  numeric structure, supported length, and Luhn validity; broad numeric,
  account, or generic payment context is not sufficient.
- IBAN classification requires a registered country, the country's exact IBAN
  length, valid structure, and MOD-97 checksum.
- Financial rules do not perform network validation or claim external account
  authority.
- Financial detection remains disabled unless the financial catalog is
  explicitly selected.
- Findings, native projections, WebAssembly projections, examples, and
  diagnostics remain metadata-only with respect to matched sensitive values.
- Caller-owned source material remains outside adapter persistence and network
  behavior.

### Interoperability

- Rust, C ABI, and WebAssembly expose the same financial rule authority rather
  than reimplementing financial validation in adapters.
- Native scanner construction preserves explicit default and financial catalog
  composition.
- WebAssembly scanner construction preserves the same explicit catalog
  composition.
- The native builder consumption contract is documented explicitly:
  `cribra_builder_build` consumes a non-null builder on every build attempt.
- Native finding and output views remain borrowed from their owning report or
  transformed-output handle.
- C and C++ integration examples compile and link against the real
  `cribra-capi` library.
- The same optimized `wasm-bindgen --target web` production artifact is usable
  from browsers and from Node.js, Bun, and Deno; server-side runtimes initialize
  it from explicit Wasm bytes.
- Browser and JavaScript-runtime examples preserve the Rust core as the sole
  detection authority and do not route through the C ABI.

### Validation

- Added adversarial financial coverage for checksum failures, unknown IBAN
  countries, PAN context boundaries, numeric adjacency, punctuation, prose,
  and supported PAN length boundaries.
- Added composition coverage ensuring the default and financial catalogs retain
  distinct rule ownership and stable financial rule IDs.
- Added C ABI financial regressions.
- Added Rust/WebAssembly financial semantic-parity fixtures.
- Verified generic transforms over financial findings across applicable adapter
  surfaces.
- Verified the Rust financial example through compilation, execution, Clippy,
  and formatting checks.
- Verified the dependency-free browser example against the production Wasm
  artifact.
- Verified the JavaScript-runtime example with Node.js 26.9.0, Bun 1.4.2, and
  Deno 2.9.7.
- Verified the C example with C11 compilation, warnings-as-errors, dynamic
  linking, real scanning, finding traversal, and redaction.
- Verified the C++ example with C++17 compilation, warnings-as-errors,
  application-local RAII ownership, real scanning, finding traversal, and
  redaction.
- The full pre-release core, native, WebAssembly, MSRV, dependency-audit,
  packaging, and publish-dry-run gates pass for the implementation baseline.

## [0.4.5] - 2026-09-19

Cribra 0.4.5 expands high-confidence developer and package-ecosystem credential
coverage while preserving contextual validation, conservative false-positive
boundaries, transformation behavior, and Rust/C/WebAssembly semantic parity.

### Added

- Added Cargo registry credential detection, including supported registry token
  configuration and environment-variable forms.
- Added PyPI repository-token detection.
- Added NuGet package-source cleartext-password detection.
- Added Maven server-password detection.
- Added RubyGems API-key and host-scoped API-key detection.
- Added SwiftPM credential detection for supported registry tokens, registry
  passwords, source-control tokens, and SwiftPM netrc password material.
- Added Gradle repository-password and repository authentication-header-value
  detection for supported explicit configuration surfaces.
- Added Composer credential detection for supported HTTP Basic passwords,
  bearer tokens, Bitbucket consumer secrets, and Forgejo tokens.
- Expanded Rust, native C ABI, and WebAssembly parity coverage for the developer
  credential portfolio.

### Changed

- Hardened developer credential validators so similarly shaped values are not
  accepted outside their supported configuration authority.
- Kept developer/package detection contextual rather than adding broad token,
  password, or arbitrary encoded-value matching.
- Kept Composer subtype ownership explicit so one credential span does not
  acquire duplicate findings from overlapping validator identities.
- Kept SwiftPM credential recognition limited to supported explicit
  configuration surfaces rather than introducing broad Swift-related token
  heuristics.
- Kept Gradle credential recognition limited to supported repository
  configuration and explicit `ORG_GRADLE_PROJECT_` environment-variable
  authority.
- Audited Dart/pub credential surfaces without adding a detector where the
  available evidence did not justify a reliable new rule.
- Tightened Maven pattern prefiltering without changing accepted credential
  semantics.
- Updated the production WebAssembly optimization policy to retain the measured
  Binaryen `-Oz` artifact.

### Security

- Hardened contextual credential validation against documentation values,
  placeholders, malformed encodings, unrelated fields, and unsupported
  configuration shapes.
- Maven protected/encrypted password placeholders are rejected rather than
  classified as cleartext credentials.
- npm registry credentials remain restricted to registry-scoped supported
  authentication keys and valid supported encodings.
- Cargo token recognition remains restricted to supported Cargo registry
  configuration and environment-variable authority.
- Composer credential fixtures and test diagnostics avoid logging matched
  credential material.
- Dependency auditing runs on every pull request.

### Fixed

- Aligned transformation behavior with newly added built-in developer
  credentials.
- Corrected Composer validator identity so credential subtypes retain distinct
  rule ownership.
- Hardened password/source handling in the developer credential implementation.

### Dependencies

- Updated `blake3` from 1.8.6 to 1.8.7.
- Updated `aho-corasick` from 1.1.4 to 1.1.5.
- Updated `base64` from 0.22.1 to 0.23.1.

### Validation

- Added Rust contract coverage for the developer credential families.
- Added native C ABI developer-credential parity coverage.
- Added WebAssembly semantic-parity coverage for the v0.4.5 portfolio.
- Expanded adversarial false-positive tests across supported package and
  developer credential formats.
- Completed PHP/Composer and Dart credential parity/audit coverage.
- Validated transformations for applicable new built-in findings.
- Release preparation completed for `cribra 0.4.5` and
  `cribra-wasm 0.4.3`.

## [0.4.4] - 2026-09-13

Cribra 0.4.4 establishes the canonical reusable command-line architecture
without changing core detection semantics.

### Added

- Added the dedicated `cribra-cli` crate containing the reusable CLI library
  and thin standalone `cribra` executable.
- Added explicit UTF-8 file scanning and stdin scanning through
  `cribra scan -`.
- Added stable human-readable output and deterministic metadata-only JSON
  output.
- Added reusable public command types and parsing/execution APIs for downstream
  Rust consumers.
- Added executable integration tests, file/stdin equivalence coverage, and
  clean, review-only, finding-bearing, invalid-input, and exit-code
  regressions.

### Changed

- Established Cribra as the authority for canonical CLI semantics while keeping
  detection, validation, findings, candidates, remediation, and transformations
  authoritative in the core.
- Kept the standalone executable as a thin adapter over `cribra-cli`.
- Kept CLI dependencies intentionally minimal.
- Kept recursive traversal, repository discovery, globbing, and directory
  walking outside the v0.4.4 CLI contract.

### CLI contract

- `cribra scan <FILE>` scans one explicit UTF-8 file.
- `cribra scan -` scans UTF-8 from stdin.
- `--format human` is the default presentation.
- `--format json` emits stable machine-readable metadata.
- Exit code `0` means command execution succeeded, including clean,
  review-only, and finding-bearing scans.
- Exit code `1` represents input or execution failure.
- Exit code `2` represents command-line usage or parse failure.

### Security

- CLI output contains classification metadata and coordinates rather than
  matched secret values.
- Review candidates remain metadata-only and are not promoted into findings by
  the CLI.
- Invalid UTF-8 is rejected deterministically.
- CLI diagnostics do not include source contents or matched values.
- The CLI adds no network access, persistence, uploads, or remote validation.

### Validation

- Formatting, check, test, Clippy, executable integration, human-output, and
  JSON-output gates cover `cribra-cli`.
- Core Rust, native C ABI, and WebAssembly release gates remain authoritative
  for their interoperability surfaces.

## [0.4.3] - 2026-09-12

Cribra 0.4.3 substantially completed the high-confidence secret-detection
baseline while preserving conservative false-positive requirements and
semantic parity across Rust, C, and WebAssembly.

### Highlights

- GitLab credential families.
- Database connection passwords.
- Quoted password/passphrase support.
- HTTP Basic authentication.
- PGP private keys.
- WireGuard credentials.
- Docker registry authentication.
- npm registry credentials.
- `.netrc` credentials.
- `/etc/shadow` and `.htpasswd` password verifiers.
- Password-verifier remediation semantics.
- Expanded adversarial corpus.
- Rust/C/WebAssembly parity validation.

## [0.4.2] - 2026-09-09

Security-hardening release.

### Highlights

- Atomic scan-and-build construction for share-safe bundles.
- Exact source/report pairing guidance.
- Transformation-to-rescan safety coverage.
- PKCS#8, encrypted PKCS#8, RSA, EC, and OpenSSH private keys.
- HTTP Bearer authentication.
- JSON-escaped GCP service-account private keys.
- Rust/C/WebAssembly release validation.

## [0.4.1] - 2026-08-24

Published `cribra-wasm` as an independently consumable WebAssembly
interoperability adapter with packaging, semantic parity, and browser
validation.

## [0.4.0] - 2026-08-24

Introduced the typed WebAssembly interoperability layer while preserving the
Rust core as semantic authority and keeping WebAssembly independent from the
native C ABI.

## [0.3.x]

Introduced native interoperability through the dedicated `cribra-capi` adapter,
including explicit ownership and lifetime rules, generated C headers, real C
consumer validation, cross-platform gates, and Rust/C/WebAssembly semantic
parity.

## [0.2.x]

Established the Cribra name and the current detection, ambiguity,
explainability, query, remediation, and share-safe transformation model.

## [0.1.x]

Initial public release line, originally published under the `silens-scan` crate
name before the reusable engine was renamed Cribra.