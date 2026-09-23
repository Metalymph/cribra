# Cribra Coverage Manifest

This document is the canonical human-readable statement of what Cribra
recognizes as sensitive material, which detector owns that material, what is
covered transitively by another detector, and which categories Cribra
deliberately defers or refuses to own.

It documents semantic coverage, not merely the current number of built-in
rules.

A missing provider-specific rule does not necessarily mean that the material
is undetected. A credential may already be covered by a more general owner.
Conversely, a recognizable string is not automatically within Cribra's
detection contract.

The manifest is intended for humans, downstream applications, documentation
generators, and automated agents that need to determine whether a class of
sensitive material is already covered before proposing another detector.

## Coverage states

Cribra uses four coverage states.

### DIRECT

Cribra owns an explicit detector for the material.

The detector has a defined matcher and validation contract and can produce a
confirmed `Finding` when that contract is satisfied.

### TRANSITIVE

The material is intentionally covered by another, more general detector.

For example, a provider may issue a standard credential representation already
owned by Cribra. A second provider-specific detector is unnecessary unless the
provider format adds meaningful validation or semantics.

Transitive coverage is first-class coverage. It is not considered a missing
detector.

### DEFERRED

The material is within the possible semantic scope of Cribra and may be
statically recognizable, but Cribra deliberately does not own a detector for
it in the current portfolio.

A deferred category is not a release commitment.

Deferred entries include a priority indicating how strongly they should be
reconsidered if production evidence, downstream demand, or stronger technical
authority appears:

- **HIGH** — strong candidate with a material use case or strong structure;
- **MEDIUM** — legitimate candidate whose incremental value or static
  discrimination is currently insufficient;
- **LOW** — valid sensitive material, but narrow, weakly differentiated, or
  substantially covered by existing detector families.

### REFUSED

Cribra deliberately does not own the category as a detection target.

Typical reasons include:

- the material is intentionally public;
- recognizing the syntax does not establish sensitive semantics;
- the category belongs to configuration auditing or general DLP rather than
  sensitive-material detection;
- there is no sufficiently reliable static representation;
- the category describes a container or transport rather than the sensitive
  value stored inside it.

`REFUSED` does not mean that sensitive content inside the refused category is
ignored.

For example, Docker secrets are not themselves a Cribra detection category,
but a supported private key stored inside a Docker secret remains detectable
by its private-key owner.

## Detection principles

Cribra detects secrets, credentials, authentication material, sensitive
verifiers, and selected statically recognizable sensitive personal or
financial material.

Cribra is not intended to become:

- a general-purpose DLP engine;
- a natural-language PII classifier;
- a vulnerability scanner;
- a dependency vulnerability database;
- a permissions auditor;
- an infrastructure or PAM configuration linter;
- a malware scanner;
- a generic entropy scanner that promotes arbitrary opaque data to findings.

Static recognizability is necessary but is not by itself sufficient for a
built-in detector.

A category should normally have:

1. material security or privacy impact;
2. an authoritative or defensible representation;
3. sufficiently strong static discrimination;
4. meaningful coverage not already owned by another rule;
5. acceptable false-positive behavior.

## Findings and candidates

A `Finding` represents material for which the active detector has enough
evidence to make a positive detection claim.

A `SensitiveCandidate` represents narrow, review-worthy material for which
Cribra intentionally lacks enough evidence to make that claim.

Candidates are not a fallback mechanism for weak detectors.

If a proposed detector cannot meet its Finding contract, Cribra does not
automatically downgrade arbitrary matches into candidates.

When a candidate overlaps an accepted Finding, the Finding owns the region and
the candidate is discarded.

## Ownership and collision policy

Every directly covered representation has a semantic owner.

When a specialized validated detector and a lower-priority generic detector
accept the exact same source span, the specialized detector has precedence.

Current normalization deliberately preserves partially overlapping Findings.
New detectors therefore must not rely on normalization to resolve ambiguous
semantic ownership.

Two specialized detectors should not intentionally produce different Findings
for the exact same representation merely to provide two classifications.

Where one structured representation contains several sensitive fields, Cribra
may assign ownership to the complete validated structure rather than emit a
cascade of overlapping Findings.

Rule identifiers are unique across a scanner.

## Default security and credential coverage

`builtins::CURRENT` owns the default credential and security-sensitive
portfolio.

The current portfolio includes direct coverage for the following major
families.

### Provider and platform credentials

Direct provider-specific coverage includes credential families for:

- GitHub;
- GitLab;
- Stripe;
- Cloudflare;
- Slack;
- Telegram;
- AWS;
- Azure;
- GCP.

Provider-specific ownership is used when the provider exposes a sufficiently
distinct representation or when provider-aware validation materially improves
the detection contract.

A provider does not receive a dedicated detector merely because it can consume
a generic password, JWT, private key, bearer token, or other representation
already owned elsewhere.

### Developer and package ecosystem credentials

Direct coverage includes credential material associated with:

- npm;
- Cargo registries;
- PyPI;
- NuGet;
- Maven;
- RubyGems;
- SwiftPM;
- Gradle;
- Composer.

The portfolio intentionally prefers credential representation and configuration
semantics over broad detection of arbitrary values appearing in package-manager
files.

### Generic authentication material

Direct contextual or structural coverage includes families such as:

- passwords;
- database passwords;
- database connection credentials;
- passphrases;
- API keys;
- authentication tokens;
- bearer authentication material;
- HTTP Basic authentication material;
- generic secrets;
- selected sensitive password/hash representations.

Generic detectors are deliberately conservative and do not imply provider
ownership.

### Cryptographic private material

Direct coverage includes supported private-key representations, including
families such as:

- PKCS#8 private keys;
- encrypted private keys;
- RSA private keys;
- EC private keys;
- OpenSSH private keys;
- PGP private-key material.

Public cryptographic material is intentionally treated separately and is
generally outside the sensitive-secret contract.

### System authentication material

Direct coverage includes:

- Unix/system password verifiers recognized by the system-password-verifier
  contract;
- htpasswd password verifiers;
- WireGuard private keys;
- WireGuard preshared keys;
- netrc password material.

System configuration itself is not automatically sensitive material.

### Signed JWTs

Structurally valid signed JWT material has a direct owner.

Systems that use standard signed JWTs for authentication may therefore receive
transitive coverage without requiring a provider-specific JWT detector.

## Financial coverage

Financial detection is opt-in through:

```rust
builtins::financial::CURRENT
```

Financial material is separated from the default credential portfolio because
its presence is sensitive under a different semantic contract.

### IBAN

**Owner:** `financial.iban`

**Coverage:** DIRECT

Recognizes supported registered-country IBAN structures with country-specific
length validation, character validation, check-digit requirements, and MOD-97
validation.

An account identifier represented as a valid supported IBAN is covered by this
owner.

The detector does not establish:

- account existence;
- account ownership;
- account activity;
- compromise;
- authorization to use the account.

### Payment card PAN

**Owner:** `financial.pan`

**Coverage:** DIRECT

Recognizes payment-card primary account numbers only when numeric structure,
supported length, Luhn validity, and sufficiently strong payment-card context
agree.

The detector is network-agnostic and does not claim:

- card-network ownership;
- issuer assignment;
- account existence;
- account activity;
- compromise.

Weak generic terms such as `number` or `account` do not independently establish
PAN semantics.

### Card verification codes

**Planned for v0.4.7**

Card verification values such as CVV/CVC/CID have passed the v0.4.7 coverage
audit as sensitive authentication data.

The intended owner is:

`financial.card-verification-code`

Detection must be contextual because a bare three- or four-digit value cannot
reliably establish card-verification semantics.

Until the implementation lands and passes its validation gates, this entry
must not be interpreted as DIRECT coverage.

## Structured personal-data coverage

Structured personal-data detection is being introduced in v0.4.7 through an
explicit opt-in portfolio.

### Italian Codice Fiscale — DIRECT

Owner:

- `personal.it-codice-fiscale`

Direct coverage:

- canonical 16-character Italian Codice Fiscale for natural persons;
- ordinary and structurally valid omocodic representations;
- positional structure;
- encoded month and day/sex fields;
- birthplace/cadastral-code structure;
- final control-character validation.

The rule reports only the authoritative 16-character identifier span.

The detector establishes structural validity only. It does not establish that
the identifier was assigned, that the represented person exists, that the
identifier belongs to a particular person, or that it is currently valid in an
authoritative registry.

Deliberate exclusions:

- 11-digit numeric Italian tax identifiers;
- Italian VAT numbers as a personal-data category;
- generic national identifiers;
- registry or identity lookups.

The rule is not part of `builtins::CURRENT`. Consumers opt into it through
`builtins::personal::CURRENT`.

### Polish PESEL — DIRECT

Owner:

- `personal.pl-pesel`

Direct coverage:

- canonical 11-digit Polish PESEL identifiers;
- encoded century and birth-month semantics for the supported PESEL range
  1800–2299;
- Gregorian birth-date validity, including leap-year handling;
- final checksum validation.

The rule reports only the authoritative 11-digit identifier span.

The detector establishes structural validity only. It does not establish that
the identifier was assigned, that the represented person exists, that the
identifier belongs to a particular person, or that it is currently present in
the PESEL register.

The sex-encoding digit is structurally accepted for both valid parity classes;
Cribra does not use the encoded sex as an additional sensitive-classification
condition.

Deliberate exclusions:

- arbitrary 11-digit numeric identifiers;
- PESEL-like values with invalid encoded dates or checksums;
- registry or identity lookups.

The rule is not part of `builtins::CURRENT`. Consumers opt into it through
`builtins::personal::CURRENT`.

### UK NHS Number — DIRECT

Owner:

- `personal.uk-nhs-number`

Direct coverage:

- compact 10-digit NHS Numbers;
- canonical 3-3-4 representations using ASCII spaces or hyphens;
- Modulus 11 check-digit validation;
- explicit NHS-number field context.

The rule reports the complete source representation of the NHS Number while
excluding surrounding field context from the finding span.

A checksum-valid numeric sequence alone is not sufficient evidence of
sensitive NHS data. Classification therefore requires explicit NHS-number
context and is reported as contextual detection.

Structural validity does not establish that the number was assigned, that a
patient exists, that the number belongs to a particular person, or that it is
present in an authoritative NHS registry.

Deliberate exclusions:

- bare checksum-valid 10-digit sequences;
- generic patient, account, reference, or other numeric identifiers;
- malformed or noncanonical NHS Number representations;
- known placeholder values;
- identifiers from other UK healthcare numbering systems.

The rule is not part of `builtins::CURRENT`. Consumers opt into it through
`builtins::personal::CURRENT`.

### US Social Security Number — DIRECT

Owner:

- `personal.us-ssn`

Direct coverage:

- compact nine-digit US Social Security Number representations;
- canonical `AAA-GG-SSSS` representations;
- current SSA structural impossibility constraints;
- explicit SSN or Social Security Number field context.

The rule reports the complete source representation of the SSN while excluding
surrounding field context from the finding span.

A structurally possible nine-digit value alone is not sufficient evidence of
sensitive SSN data. Classification therefore requires explicit SSN-specific
context and is reported as contextual detection.

Structural validity does not establish that the number was assigned, that a
holder exists, that the number belongs to a particular person, or that it is
present in authoritative SSA records.

Deliberate exclusions:

- bare structurally possible nine-digit values;
- generic tax, national, employee, person, or other numeric identifiers;
- area `000`, `666`, and `900`–`999`;
- group `00`;
- serial `0000`;
- historical geographic inference or High Group validation as a current
  validity test;
- assignment or identity verification.

The rule is not part of `builtins::CURRENT`. Consumers opt into it through
`builtins::personal::CURRENT`.

### Remaining v0.4.7 personal-data scope — PLANNED

The remaining frozen personal-data implementation scope includes:

- ICAO machine-readable travel-document zones.

This family remains planned until its implementation and validation gates land.

It becomes DIRECT only when the corresponding implementation and tests land.

### ICAO MRZ ownership

The intended MRZ detector owns the complete validated machine-readable zone
rather than independently promoting every embedded document number, date, or
other field to overlapping Findings.

Generic passport-number detection is deliberately refused.

## Cryptographic wallet and recovery material

Cribra distinguishes public cryptocurrency identifiers from private credential
material.

Public wallet addresses are not secrets and are deliberately outside the
built-in sensitive credential contract.

Private or recovery material capable of granting control is different.

The v0.4.7 frozen implementation scope includes:

- BIP39 mnemonic recovery material;
- BIP32 extended private keys.

These are security credentials and belong to the default security portfolio,
not the financial portfolio.

They become DIRECT only after implementation and validation.

## MFA material

### TOTP/HOTP provisioning secrets — DIRECT

**Owner:** `mfa.otp-provisioning-secret`

TOTP/HOTP shared provisioning secrets are authentication credentials directly
covered by the default security portfolio.

Direct coverage includes:

- shared secrets carried by `otpauth://totp` provisioning material;
- shared secrets carried by `otpauth://hotp` provisioning material;
- explicit OTP-secret configuration fields.

Detection is contextual. Base32 structure alone is insufficient to establish
OTP credential semantics.

The finding projects only the shared-secret value rather than the complete
provisioning URI or surrounding configuration field.

Bare arbitrary Base32 strings and Base32 material in unrelated secret contexts
are deliberately rejected.

The specialized MFA owner takes precedence over generic secret ownership for
the same accepted source span.

## Tailscale credentials

Tailscale credentials with authoritative capability-specific prefixes are
directly covered by the default credential portfolio.

Current direct coverage includes:

- `tskey-api-` as `tailscale.api-access-token`;
- `tskey-auth-` as `tailscale.auth-key`;
- `tskey-client-` as `tailscale.oauth-client-secret`;
- `tskey-scim-` as `tailscale.scim-key`;
- `tskey-webhook-` as `tailscale.webhook-key`.

These formats are structurally classified from the credential value itself and
expose deterministic detection metadata.

Capability-specific semantic ownership is preserved rather than collapsing
these formats into a single generic Tailscale credential rule.

Malformed values, unsupported prefixes, case-changed prefixes, and obvious
documentation placeholders are rejected.

Cribra does not infer undocumented Tailscale credential formats from opaque
values.

## Transitive coverage index

The following table records important cases where absence of a dedicated rule
does not imply absence of coverage.

| Material / ecosystem | Coverage owner | Status | Notes |
|---|---|---|---|
| Kubernetes ServiceAccount signed JWT | signed JWT detector | TRANSITIVE | A provider-specific rule is unnecessary when the credential is already a supported signed JWT |
| TLS private key in a supported encoding | corresponding private-key detector | TRANSITIVE | TLS usage does not create a different private-key secret |
| SSH private key in supported OpenSSH/private-key encoding | corresponding private-key detector | TRANSITIVE | Ownership follows the private representation |
| Account identifier represented as a valid supported IBAN | `financial.iban` | TRANSITIVE | Domestic account semantics do not require a second Finding |
| Provider credential represented only as a supported generic bearer token | generic bearer/auth ownership | TRANSITIVE | Unless a stronger provider-specific representation exists |
| Password stored in a supported netrc representation | netrc/password ownership | DIRECT / representation-specific | The configuration representation provides additional semantic evidence |
| Sensitive material stored inside a Docker secret | owner of the contained material | TRANSITIVE when recognizable | Docker Secret is a container, not a secret format |
| Sensitive material supplied through systemd credentials | owner of the contained material | TRANSITIVE when recognizable | The credential transport does not define the material's semantic type |

This index is intentionally illustrative during v0.4.7 development. The
portfolio-completion gate must reconcile it against the complete built-in
catalog.

## Deferred coverage

`DEFERRED` categories remain possible Cribra territory but are not part of the
current frozen implementation scope.

Priority is a reconsideration priority, not a release commitment.

| Category | Reason | Priority |
|---|---|---|
| German Steuer-ID | Strong structured personal identifier, but the authoritative validation contract still requires closure before implementation | HIGH |
| Brazil CPF | Strong structured personal identifier; authoritative checksum implementation evidence was not sufficiently closed during the current audit | HIGH |
| UK National Insurance Number | Important personal identifier but weaker static validation and likely contextual semantics | MEDIUM |
| Canada Social Insurance Number | Important personal identifier but bare numeric representation has insufficient discrimination for the current portfolio | MEDIUM |
| US Medicare Beneficiary Identifier | Structured healthcare identifier, but lower incremental value after the selected v0.4.7 personal-data portfolio | MEDIUM |
| Bitcoin WIF private key | Legitimate private wallet material, but incremental coverage after BIP39/BIP32 is lower | MEDIUM |
| BIP38 encrypted private key | Legitimate protected wallet material with lower immediate value than unencrypted recovery/private material | LOW |
| OpenVPN-specific static secret material | Legitimate security material, but comparatively narrow and partially overlapping existing private-key coverage | LOW |
| French numéro fiscal | Personal identifier, but current audit did not establish sufficient static discrimination and validation authority for inclusion | LOW |

A deferred detector should be reconsidered when there is concrete production
evidence, downstream demand, newly authoritative validation information, or a
material coverage gap that existing families cannot represent safely.

## Refused coverage

`REFUSED` categories are deliberately not detector backlog.

They should not be proposed as built-in Findings merely to increase detector
count.

| Category | Reason |
|---|---|
| Generic passport numbers | Country and document-generation variability prevents a reliable generic ownership contract; structured MRZ is preferred |
| Generic national-ID numbers | No coherent universal validation contract |
| VAT/business tax identifiers as personal PII | They may identify businesses or taxable entities and do not inherently establish personal sensitive material |
| Generic bank account numbers | No sufficiently reliable universal static representation; IBAN is owned where applicable |
| ABA routing numbers | Identify financial institutions and are publicly distributed routing identifiers |
| SWIFT/BIC | Institution/business identifiers rather than account secrets |
| Public cryptocurrency wallet addresses | Public payment identifiers, not private credential material |
| Other public blockchain addresses | Same public-identifier principle |
| Generic seed phrases | Arbitrary word sequences do not establish wallet recovery semantics; validated formats such as BIP39 require their own contract |
| Generic email addresses as standalone Findings | Syntactic recognizability does not establish sufficiently strong sensitive-material semantics |
| Generic phone numbers as standalone Findings | Syntactic recognizability and substantial ambiguity do not establish the required Finding contract |
| Physical addresses as standalone Findings | Requires semantic/NLP classification beyond Cribra's static-material scope |
| Standalone card expiration dates | Insufficiently discriminating and not independently useful as a Cribra Finding |
| Standalone cardholder names | Generic personal information rather than strongly structured sensitive material |
| SSH public/authorized keys | Deliberately public cryptographic authentication material; corresponding private material is owned separately |
| SSH known-host keys | Public host identity material |
| TLS certificates | Deliberately public cryptographic material; corresponding private keys are covered separately |
| `/etc/passwd` entries, usernames, UID/GID | System identity metadata rather than authentication secrets |
| PAM configuration | Configuration/security auditing rather than sensitive-material detection |
| Docker Secret as a category | Secret container/transport rather than a semantic secret format; recognizable contents remain detectable |
| systemd credential as a category | Credential transport/container rather than a semantic secret format |
| Kubernetes ServiceAccount token as a separate provider family | Supported signed JWT representations are already transitively covered; provider duplication is unnecessary |
| Generic recovery codes | No universal authoritative representation; narrow ambiguous recovery-like structures may remain `SensitiveCandidate`s |
| Generic high-entropy blobs | Entropy alone does not establish secret semantics |
| Generic configuration values | Configuration presence does not establish sensitive material |

## Coverage evolution policy

v0.4.7 is intended to complete Cribra's planned broad detector-expansion phase.

After the v0.4.7 portfolio-completion gate, new built-in detector families
should be exceptional rather than routine.

A new built-in detector should normally require at least one of:

1. a materially important sensitive class is demonstrably missing;
2. a real downstream or production workload exposes a meaningful gap;
3. a newly important technology introduces a distinct credential or sensitive
   representation;
4. an existing detector family cannot safely represent the material.

Routine provider catalog growth, speculative ecosystem coverage, and detector
count are not project goals.

Post-v0.4.7 development should primarily emphasize:

- false-positive and false-negative hardening;
- security fixes;
- correctness;
- performance;
- reliability;
- deterministic behavior;
- API and interface parity;
- compatibility;
- CLI and FFI maturity;
- bindings;
- packaging and distribution;
- downstream integration requirements.

A `DEFERRED` category may become `DIRECT` when evidence justifies it.

A `REFUSED` category requires a stronger change in evidence or semantics. Mere
demand for a larger detector catalog is not sufficient.

## Documentation requirements for detectors

Every new built-in detector should make the following coverage contract
recoverable from project documentation:

- semantic owner;
- direct coverage;
- important transitive coverage;
- deliberate exclusions;
- overlap or precedence behavior where relevant;
- validation authority or technical basis.

The final v0.4.7 portfolio-completion gate must audit the complete built-in
catalog against this manifest and ensure that significant existing detector
families are represented.

## Version authority

This manifest describes the Cribra coverage model during development of
v0.4.7.

Entries explicitly described as planned do not constitute current DIRECT
coverage until the corresponding implementation and tests have landed.

At the v0.4.7 completion gate this document must be reconciled against the
actual built-in portfolio and updated to describe the released state rather
than the development plan.