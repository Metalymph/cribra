# Ecosystem and Scope

## Purpose

Cribra is an embeddable, local-first Rust engine for deterministic
sensitive-data detection, validation, classification, review, reporting,
and explicit transformation.

This document explains Cribra's responsibility boundary in relation to
other projects operating in adjacent areas of sensitive-data detection
and secret scanning.

It is an ecosystem orientation document, not a competitive evaluation.

The projects discussed here serve different purposes and may be used
together. Their inclusion does not imply equivalent functionality,
interchangeability, or direct competition.

## Methodology

The comparison is organized around publicly documented responsibilities
and typical integration models.

It considers:

- the responsibility owned by each project;
- the surrounding workflow or application environment;
- the boundary between detection engines and source acquisition;
- integration and deployment models;
- privacy considerations and responsibility for data handling;
- explicit limitations of Cribra's own scope.

The document does not compare:

- detection accuracy or false-positive rates;
- execution speed, throughput, or memory consumption;
- binary size or dependency footprint;
- overall security or reliability;
- product quality or suitability rankings.

Absence of a capability from this document must not be interpreted as
evidence that another project does not support it.

Project capabilities and integration surfaces can change independently.
Readers should consult the linked primary documentation for current
details.

## Cribra's responsibility boundary

Cribra operates on caller-provided data.

Its Rust core owns:

- compiled detection rules and deterministic execution;
- contextual and deterministic validation;
- classified findings and separate review-only candidates;
- source coordinates and classification metadata;
- result normalization, querying, and reporting;
- typed explanation of existing classification evidence;
- explicit redaction and transformation operations;
- whole-source scanning;
- admitted-configuration streaming;
- exact incremental scanning of owned revisioned documents;
- optional parallel execution across independent sources.

The core does not own:

- filesystem or repository discovery;
- remote source acquisition;
- network communication;
- source persistence;
- application policy;
- user interfaces;
- repository hosting or managed monitoring services.

Source acquisition, data retention, presentation, and destination
policy remain the responsibility of the integrating application.

### Integration surfaces

Cribra provides several independently versioned interfaces:

| Interface | Role |
| --- | --- |
| Rust Core | Authoritative engine; Core, Flow, and Fuse build profiles |
| C ABI | Native interoperability through a documented adapter contract |
| WebAssembly | Core and Flow profiles for supported JavaScript/WASM consumers |
| Swift | UniFFI Core, Flow and Fuse source packages and XCFrameworks; planned official Fuse SwiftPM package |
| Kotlin/JVM | UniFFI Core, Flow and Fuse profile JARs and host bundles; planned official Fuse Maven artifact |
| Python | UniFFI Core, Flow and Fuse consumers and Fuse wheel packaging; planned official Fuse PyPI package |
| TypeScript | Planned shared isomorphic API contract; WebAssembly wrapper first, future native backend separately gated |
| MCP | Planned native Rust binary using MCP over stdio and reusing Cribra Core |
| CLI | Explicit UTF-8 file or standard-input scanning |

Streaming and incremental sessions are Core-owned APIs. They are exposed by the
C ABI, the WebAssembly source adapter and the UniFFI consumers, while the CLI
does not expose them. The published `cribra-wasm 0.4.7` package predates the
newer workspace session, batch and Share Bundle additions; a source build is
required for those APIs.

The CLI intentionally does not perform directory traversal,
repository walking, or glob expansion.

Build profiles describe supported configurations of the same engine,
not separate detection implementations.

### Binding support policy

Official integrations are maintained by the Cribra project and carry an ongoing
responsibility for compatibility, testing, packaging, documentation, and
maintenance. Official status does **not** imply that every integration is
published to a package registry; registry publication is tracked separately in
[Compatibility](COMPATIBILITY.md).

| Integration | Status | Boundary |
| --- | --- | --- |
| Rust Core | Official, implemented | Authoritative engine |
| C ABI | Official, implemented | Source-built libraries and checked-in header; `publish = false` |
| WebAssembly | Official, implemented | Core and Flow source; published adapter release has a separate Core contract |
| Swift | Official, implemented; distribution in progress | Local SwiftPM packages and XCFrameworks, plus unpublished local Fuse distribution staging; planned Fuse SwiftPM package; no remote publication |
| Kotlin/JVM | Official, implemented; distribution in progress | Local/CI profile JARs and host bundles; planned Fuse Maven artifact; no Maven publication |
| Python | Official, implemented; distribution in progress | Core/Flow/Fuse source consumers and a local/CI Fuse wheel; planned Fuse PyPI package; no PyPI publication |
| TypeScript shared API | Official, planned | One isomorphic public API contract for the WebAssembly and future native backends; contract precedes implementation |
| TypeScript (WebAssembly) | Official, planned | Higher-level wrapper over typed WASM exports for browsers and compatible JS runtimes; primary motivation is Crystal |
| TypeScript (native) | Official, planned; separately gated | Future UniFFI-backed backend against the shared contract; Node.js requirements need investigation, and UniFFI does not by itself provide a working React Native binding |
| MCP server | Official, planned | Ultra-lightweight native Rust binary over stdio reusing Cribra Core; no Node.js/Python runtime, database or external service; no arbitrary filesystem access by default |
| C#, Go, PHP, Ruby, Elixir | Planned — demand-driven / community contribution | Not actively scheduled; considered only with demonstrated real-world demand or a viable community contribution |
| Community integrations | Community-maintained | May use public interoperability surfaces; not officially tested, packaged or supported |

Demand-driven candidates have no delivery dates and are not implemented,
qualified or published. Community contributions are welcome under the repository
[contribution guidelines](../CONTRIBUTING.md). Official support requires an
ongoing maintenance commitment, accurate documentation, consumer testing and CI
coverage; the existence of an adapter surface or an open request does not by
itself commit the project to a binding. Do not create placeholder packages or
empty integration guides for these candidates.

Community integrations do not become officially maintained by using those
surfaces. Adding a new official binding requires demonstrated demand, a credible
maintenance commitment, and a concrete technical justification.

### Distribution policy

The primary language bindings target one official public package per ecosystem.
Each official package exposes the complete supported **Fuse** binding profile:

| Ecosystem | Official distribution target | Default profile |
| --- | --- | --- |
| Swift | SwiftPM package, discoverable through Swift Package Index | Fuse |
| Kotlin/JVM | Maven Central artifact | Fuse |
| Python | PyPI package | Fuse |

These are intended distribution targets, not claims that registry publication
has occurred. Approval, qualification and release are separate gates.

Swift is the immediate priority. W3.1C implements local Fuse distribution
staging and qualification, but no distribution repository, remote package,
release asset, published checksum or Swift Package Index entry exists yet.
Kotlin/JVM and Python remain official priorities with local/CI artifacts only.
None of these three ecosystems has a published public package.

The public package must expose the complete functionality implemented by the
Fuse binding. It does not claim parity with Rust-only APIs that are absent from
the foreign adapter.

Core and Flow remain available for source-based builds. They are not published
as separate official registry packages:

- **Core** provides scanning and the related Core capabilities.
- **Flow** provides Core plus the transformations and Share Bundle capabilities
  implemented by that profile.
- **Fuse** provides the complete supported binding feature set, including the
  parallel capabilities available in that profile.

Core and Flow are alternative builds of the same engine, not additional modules
intended to coexist with the Fuse package. Use exactly one matching profile per
application or process.

The Swift technical contract is
[the Swift remote distribution contract](../bindings/swift/README.md#remote-swiftpm-distribution-contract-fuse).
Kotlin/JVM and Python require separate distribution contracts; Swift packaging
details do not define their registry mechanics.

This one-package, Fuse-first policy applies to Swift, Kotlin/JVM and Python
distribution. It does not automatically define the packaging architecture of the
C ABI, the raw WebAssembly crate, Rust feature selection, the MCP server or the
planned TypeScript runtime integrations. For TypeScript, the shared public API
contract is distinct from the distribution mechanism used by each runtime;
WebAssembly and native backends are distinct targets, not Core/Flow/Fuse package
variants, and package names are not decided prematurely.

## Related projects and workflows

The following descriptions identify primary responsibilities rather
than exhaustively cataloging features.

| Project | Documented area of responsibility | Typical integration context |
| --- | --- | --- |
| Cribra | Embedded detection, validation, review, reporting, and explicit transformation | Applications and services integrating the Rust core or its adapters |
| Gitleaks | Detection of secrets in development and Git-related content | CLI, development workflows, and automation |
| TruffleHog | Discovery and verification of secrets across supported sources | Security workflows, CLI, and integrations |
| detect-secrets | Detection of potential secrets and management of review baselines | Development workflows and Python-based tooling |
| Microsoft Presidio | Analysis and anonymization of sensitive information | Applications, services, and data-processing workflows |
| GitHub Secret Scanning | Secret detection integrated with GitHub repositories and platform workflows | GitHub-hosted development and security workflows |
| Talisman | Preventive secret-related checks in Git development workflows | Git hooks and developer tooling |

These are overlapping areas of work, not mutually exclusive product
categories.

### Gitleaks

Gitleaks focuses on finding secrets in development-oriented content,
including Git repositories, files, directories, and standard input.
It also documents baseline and pre-commit workflows.

Its source acquisition and command-oriented usage are distinct from
Cribra's caller-provided-data engine boundary.

Official documentation:
https://github.com/gitleaks/gitleaks

### TruffleHog

TruffleHog focuses on discovering secrets and, where supported,
verifying detected credentials against relevant services or providers.

Verification capabilities and their operational implications depend
on the selected detectors, sources, and configuration.

Cribra's built-in validation is local and deterministic; it does not
perform network-based credential verification.

Official documentation:
https://github.com/trufflesecurity/trufflehog

### detect-secrets

detect-secrets provides secret-detection tooling and baseline-oriented
review workflows for development environments.

Its baseline and development-tooling responsibilities differ from
Cribra's metadata reporting and application-owned workflow model.

Official documentation:
https://github.com/Yelp/detect-secrets

### Microsoft Presidio

Microsoft Presidio provides components for detecting and anonymizing
sensitive information, including personally identifiable information.

There is functional overlap in sensitive-data recognition and
transformation, but the projects have different documented detection
scope and integration contracts.

Cribra intentionally does not claim broad PII classification.

Official documentation:
https://github.com/data-privacy-stack/presidio

### GitHub Secret Scanning

GitHub Secret Scanning is a platform-integrated capability for
detecting supported secrets in GitHub-managed development workflows.

Its managed-service context differs from Cribra's embedded engine,
which does not acquire or upload source data.

Official documentation:
https://docs.github.com/en/code-security/concepts/secret-security/secret-scanning

### Talisman

Talisman provides preventive checks intended for Git development
workflows, including hook-based use.

Its workflow integration is a different responsibility from
Cribra's reusable detection and transformation engine.

Official documentation:
https://github.com/thoughtworks/talisman

## Integration and privacy considerations

Privacy cannot be accurately reduced to a single local-versus-cloud
label for all these projects.

For Cribra, the Rust core performs no filesystem I/O or network
communication. The integrating application decides where source data
comes from, whether it is persisted, and where outputs are sent.

Cribra's public finding and review metadata does not intentionally
contain copies of matched secret values. Incremental sessions
separately own the complete source document; explicit transformation
operations require access to caller-provided source material.

For command-line tools, data handling depends on the selected command,
inputs, configuration, and enabled integrations.

For libraries and self-hosted services, the application and deployment
environment determine important parts of the data-handling boundary.

For managed platform services, processing takes place within the
service's documented operational and security model.

No project-wide privacy or security ranking is inferred from these
differences.

## Detection and transformation scope

Cribra's default built-in catalog concentrates on supported
credential and security-sensitive patterns.

Additional financial recognition is available through an explicit
opt-in pack.

Cribra deliberately does not perform:

- generic entropy scanning;
- arbitrary Base64 detection;
- unrestricted hash detection;
- broad PII classification;
- network-based credential verification.

A classified Finding and a review-only SensitiveCandidate have
different semantics.

Review-only candidates are not silently promoted into classified
findings or automatically transformed.

Transformations are explicit and operate under the integrating
application's source and destination policy.

These boundaries are part of Cribra's design contract, not claims
about the capabilities or quality of other projects.

## Choosing an integration model

The relevant distinction is often which component should own the
workflow.

An embedded engine is appropriate when an application already owns
source acquisition, user interaction, persistence, and policy, and
needs reusable detection or transformation primitives.

A repository-oriented scanner is appropriate when the desired
operation includes inspecting development content through its
supported repository or filesystem workflows.

A development-workflow tool is appropriate when checks or review
baselines need to participate directly in developer operations.

A managed platform capability is appropriate when the organization
wants detection integrated with that platform's supported workflows.

A sensitive-information processing framework is appropriate when
the application requires its documented recognition and
anonymization capabilities.

These models can coexist. Selecting one does not inherently exclude
the others.

## Limitations of this document

This document describes project scope and integration responsibilities.

It is not:

- a feature-by-feature conformance test;
- an independent audit of the referenced projects;
- a comparative security assessment;
- a performance benchmark;
- a substitute for current upstream documentation.

The descriptions of external projects are intentionally concise.
They do not assert that undocumented capabilities are absent.

Cribra's supported behavior is defined by its own public contracts
and qualification evidence, not by this ecosystem overview.

## Primary sources

Cribra:

- README.md
- docs/ARCHITECTURE.md
- docs/INTEROP.md
- docs/WASM_INTEROP.md
- docs/CLI_PARITY.md
- docs/streaming.md
- docs/incremental.md

External projects:

- Gitleaks: https://github.com/gitleaks/gitleaks
- TruffleHog: https://github.com/trufflesecurity/trufflehog
- detect-secrets: https://github.com/Yelp/detect-secrets
- Microsoft Presidio: https://github.com/data-privacy-stack/presidio
- GitHub Secret Scanning: https://docs.github.com/en/code-security/concepts/secret-security/secret-scanning
- Talisman: https://github.com/thoughtworks/talisman

Primary-source review: 2026-10-15.

The descriptions were checked against the official project repositories
and documentation available on that date. They describe documented
responsibilities, not a pinned release-by-release feature inventory.

External project capabilities may change; consult upstream documentation
for current behavior.
