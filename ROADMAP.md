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

### v0.4.5 --- Developer Ecosystem Credential Coverage

Status: completed.

Goal: consolidate the remaining high-value developer, package, runtime,
build, and systems credential surfaces into one evidence-driven release
rather than spreading additive detector work across several releases.

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
publication gate and remains planned under the canonical CLI milestone
below.

### v0.4.6 --- Sensitive Data Foundation

Status: next.

Goal: extend Cribra from a high-confidence secret and credential scanner
into a high-confidence sensitive-data detection engine without weakening
its false-positive resistance, deterministic semantics, or
embeddability.

-   [ ] Define the sensitive-data taxonomy and public category model.
-   [ ] Define opt-in pack semantics without indiscriminately broadening
    `builtins::CURRENT`.
-   [ ] Add financial-data detection where strong structural validation
    is available.
-   [ ] Add IBAN detection with country-aware structural and checksum
    validation.
-   [ ] Add payment-card PAN detection with structural and checksum
    validation.
-   [ ] Research additional financial identifiers only where
    classification can remain high-confidence.
-   [ ] Define when sensitive data is a classified `Finding` versus a
    `SensitiveCandidate`.
-   [ ] Extend metadata, explanation, confidence, severity, and
    remediation semantics where required.
-   [ ] Preserve exact authoritative source spans.
-   [ ] Preserve metadata-only public findings; never expose matched
    sensitive values through result metadata.
-   [ ] Extend synthesis, redaction, template, and pseudonymization
    support where new categories require it.
-   [ ] Maintain deterministic Rust behavior and full C ABI/WebAssembly
    semantic parity.
-   [ ] Add adversarial false-positive and false-negative regression
    coverage.
-   [ ] Run full Rust, C ABI, WebAssembly, MSRV, security, and packaging
    gates.

Principles:

-   Cribra may intentionally recognize multiple classes of sensitive
    data, but it must not become a broad low-confidence regex-based DLP
    classifier.
-   Strong validation is preferred over superficial pattern matching.
-   Checksums are valuable validation signals but do not by themselves
    justify classification outside the intended data contract.
-   Ambiguous sensitive material remains reviewable through candidate
    semantics rather than being promoted to a finding without sufficient
    evidence.
-   Sensitive-data packs outside the default credential baseline are
    opt-in unless a later release explicitly changes that contract.

### v0.4.7 --- Sensitive Data / Expose Readiness

Status: planned.

Goal: expand the validated sensitive-data model where it materially
improves Cribra as an embeddable engine for exposure analysis, including
future Silens Expose use cases.

Research and implementation candidates:

-   [ ] Physical-address recognition with contextual validation and
    conservative candidate semantics where authoritative classification
    is not possible.
-   [ ] Email-address exposure semantics.
-   [ ] Telephone-number exposure semantics with region-aware validation
    where practical.
-   [ ] Country-specific tax, national, or account identifiers only
    where structural validation is sufficiently strong.
-   [ ] Additional financial/account identifiers justified by real
    exposure use cases.
-   [ ] Evaluate cryptographic wallet/account identifiers separately
    from financial identity data.
-   [ ] Define contextual classification boundaries for web pages,
    documents, messages, email, logs, and local text.
-   [ ] Expand adversarial corpora for public, example, and reference
    data that must not be misclassified as personal exposure.
-   [ ] Maintain Rust, C ABI, and WebAssembly semantic parity for every
    exposed capability.

Principles:

-   Recognition and sensitive classification are separate decisions.
-   Public, example, organizational, and contextual identifiers must not
    be treated as personal exposure merely because their syntax is
    recognizable.
-   `SensitiveCandidate` remains the preferred boundary for useful but
    ambiguous evidence.
-   Cribra remains deterministic and application-agnostic; exposure
    correlation, crawling, identity ownership, breach intelligence, and
    higher-level exposure policy belong to consumers such as Silens
    Expose.

### v0.4.8 --- Unix/System Security Coverage Gap Closure

Status: planned.

Goal: audit and close the remaining high-value Unix and system-security
credential coverage gaps without duplicating the system, generic,
provider, or package-ecosystem surfaces already supported by Cribra.

Existing baseline includes Unix shadow password verifiers, htpasswd
password verifiers, `.netrc` passwords, OpenSSH and common private-key
formats, WireGuard private and preshared keys, generic
password/passphrase/auth surfaces, HTTP authentication, and relevant
container/provider credentials.

Remaining work:

-   [ ] Perform a complete Unix/Linux credential-surface gap audit
    against the existing built-in catalog.
-   [ ] Audit additional system/service authentication formats only
    where the credential or verifier can be identified from source
    content with strong structural or contextual evidence.
-   [ ] Audit SSH authentication material not already covered by OpenSSH
    and generic private-key rules.
-   [ ] Audit common daemon and infrastructure authentication formats
    not already owned by generic, provider-specific, package-ecosystem,
    or container rules.
-   [ ] Audit shell and system configuration credential conventions only
    where source content itself establishes the security contract.
-   [ ] Audit additional password-verifier/hash formats separately from
    generic sensitive hashes.
-   [ ] Document rejected Unix/system candidates where reliable static
    classification is not possible.
-   [ ] Add collision and ownership tests for any new system-specific
    rules.
-   [ ] Add adversarial false-positive coverage for system-like
    configuration that does not contain credentials.
-   [ ] Maintain Rust, C ABI, and WebAssembly semantic parity for every
    new capability.
-   [ ] Run full release gates.

Principles:

-   Existing `.netrc`, shadow, htpasswd, private-key, WireGuard,
    generic-auth, HTTP-auth, container, provider, and package-ecosystem
    coverage must not be duplicated.
-   Filesystem paths are not semantic authority; Cribra classifies
    source content.
-   Password hashes, password verifiers, credentials, keys, and merely
    security-related configuration remain distinct concepts.
-   Unix/System coverage is credential and sensitive-material detection,
    not host posture, vulnerability, or configuration-compliance
    scanning.

### v0.4.9 --- Canonical CLI Full Parity and Distribution

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

### v0.4.10 — Binding-Ready Native FFI

Status: planned.

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
