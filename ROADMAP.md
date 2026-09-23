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

Cribra remains on the `0.4.x` release line while changes are additive
and preserve the current core architecture and public semantic
contracts.

A future `0.5` release is reserved for a deliberate architectural or
public-contract change significant enough to justify a new minor line.
New detectors, hardening, CLI work, bindings, packaging, and additive
interoperability work do not by themselves require `0.5`.

## Current release line

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

Status: active.

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

-   [ ] Verify every accepted capability through the generic redact, template,
    pseudonymize, and synthesize contracts where applicable.
-   [ ] Add category-specific transformation semantics only when the generic
    contract is insufficient.
-   [ ] Maintain deterministic Rust behavior.
-   [ ] Maintain complete applicable C ABI semantic parity.
-   [ ] Maintain complete applicable WebAssembly semantic parity.
-   [ ] Preserve metadata-only and secret-safe public boundaries.

#### 0.4.7-F --- Portfolio completion gate

-   [x] Re-audit the complete built-in and opt-in detector portfolio after all
    accepted v0.4.7 work.
-   [ ] Verify that no known materially important detector family remains
    omitted without an explicit accepted reason.
-   [ ] Run full Rust, C ABI, WebAssembly, MSRV, security, packaging, and
    publication gates.
-   [ ] Document the post-v0.4.7 detector evolution policy.
-   [ ] Publish/tag the completed release only after all gates pass.

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

### v0.4.8 --- Canonical CLI Full Parity and Distribution

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

### v0.4.9 --- Binding-Ready Native FFI and Ecosystem Integrations

Status: planned.

Goal: complete Cribra's native interoperability foundation and expand Cribra
from a mature detection engine into a broadly consumable developer ecosystem
without moving application-specific product policy into the OSS core.

#### 0.4.9-A --- Binding-ready native FFI

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

#### 0.4.9-B --- GitHub integration

-   [ ] Provide an official GitHub Action over the canonical Cribra interfaces.
-   [ ] Preserve Cribra detection semantics rather than implementing
    Action-specific classification.
-   [ ] Define explicit CI/failure-policy behavior.
-   [ ] Keep source handling and diagnostics secret-safe.
-   [ ] Provide deterministic machine-readable findings suitable for CI.
-   [ ] Validate the Action against the shared Cribra conformance corpus.
-   [ ] Keep the integration usable without a Silens account or paid service.

#### 0.4.9-C --- Astro integration

-   [ ] Provide an official Astro integration for appropriate development,
    build, or CI scanning workflows.
-   [ ] Reuse Cribra's existing portable interfaces rather than implementing
    detection semantics in JavaScript/TypeScript.
-   [ ] Define explicit source and lifecycle boundaries.
-   [ ] Preserve local-first and secret-safe behavior.
-   [ ] Keep the integration fully usable without a Silens account or paid
    service.
-   [ ] Validate semantic equivalence against authoritative Cribra results.

#### 0.4.9-D --- Agent and AI interoperability

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

#### 0.4.9-E --- Application ecosystem boundary

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
