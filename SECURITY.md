# Security Policy

Cribra is an embeddable, local-first Rust engine for detecting, reviewing, and
safely transforming secrets and sensitive data.

Security issues should be reported privately whenever possible.

## Supported Versions

| Version | Supported |
| --- | --- |
| 0.4.x | Yes — current release line |
| 0.3.x | No |
| 0.2.x | No |
| 0.1.x | No |
| < 0.1 | No |

Until Cribra reaches 1.0, security support follows the current active minor
release line unless a release announcement explicitly states otherwise.

## Reporting a Vulnerability

Do not open a public GitHub issue for a suspected vulnerability.

Use GitHub Private Vulnerability Reporting for the repository whenever
available.

A useful report should include:

- affected Cribra version or commit;
- affected surface (`cribra`, `cribra-cli`, native C ABI, or `cribra-wasm`);
- enabled crate features where relevant;
- operating system and architecture where relevant;
- a minimal synthetic reproducer;
- expected behavior;
- observed behavior;
- security or privacy impact;
- whether sensitive material may have been exposed;
- any known workaround.

Do not include real production credentials, private keys, customer data, or
other sensitive material. Use synthetic fixtures instead.

## Security-Relevant Areas

Reports are especially valuable when they concern:

- false negatives that can cause sensitive material to be treated as safe;
- finding/candidate classification that violates documented semantics;
- redaction, templating, pseudonymization, synthesis, or share-bundle behavior
  that leaks source material unexpectedly;
- span, overlap, or source/report consistency bugs that can produce unsafe
  transformed output;
- accidental serialization of original source material;
- secrets or source snippets appearing in errors, diagnostics, explanations,
  manifests, logs, CLI output, or public result objects;
- C ABI memory-safety, ownership, lifetime, pointer, destruction, or panic
  containment issues;
- WASM boundary issues that violate documented local-first/privacy guarantees;
- CLI behavior that prints matched secret material or source contents through
  success or failure paths;
- semantic divergence between Rust, C, WASM, and CLI projections;
- dependency or supply-chain vulnerabilities with practical impact.

## Privacy Boundary

Original source material remains caller-controlled.

Public findings, sensitive candidates, explanations, remediation metadata,
summaries, manifests, serialized metadata, and CLI result output must not
contain matched secret values or original source text unless an API explicitly
and intentionally returns transformed source content.

Cribra performs no network access to scan or transform data.

Low-level transformations that accept a source and an existing report require
the caller to preserve exact source/report pairing. Matching source length is a
sanity check, not proof of source identity.

When scanning and immediately producing share-safe output, prefer the atomic
scan-and-build path so scanning and transformation use the same borrowed source
values by construction.

## Canonical CLI

`cribra-cli` is a presentation and command adapter over the Cribra core.

Security invariants include:

- the CLI does not implement independent detection logic;
- matched source values are not intentionally rendered in human or JSON output;
- ambiguous candidates remain review-only;
- invalid UTF-8 is rejected rather than decoded lossily;
- input/execution diagnostics do not include source contents;
- the CLI performs no network access or remote credential validation;
- stdin and explicit-file scanning use the same core scanner semantics;
- the standalone binary remains a thin adapter over the reusable library.

Directory/repository traversal is not part of the v0.4.4 CLI contract.

## Native C ABI

The native C ABI is a dedicated interoperability adapter.

Important invariants include:

- Rust object layouts are never part of the public C ABI;
- caller input is length-delimited and validated where applicable;
- Rust panics must not unwind across exported FFI boundaries;
- Rust-owned allocations are released only through documented Cribra
  destruction functions;
- borrowed views are valid only for the documented owner lifetime;
- matched sensitive values must not appear in ABI metadata or diagnostics.

Use-after-free, double-free, forged handles, invalid pointer/length regions, and
concurrent destruction while an object is in use remain caller-contract
violations unless explicitly documented otherwise.

## WebAssembly

`cribra-wasm` is an independent typed adapter over the same Cribra core and does
not pass through the native C ABI.

Browser/WASM reports should include, where relevant:

- browser/runtime;
- bundler or loader;
- Web Worker usage;
- initialization path;
- serialization/projection path;
- CSP constraints;
- whether the issue reproduces through the Rust core.

## Semantic Parity

Equivalent supported operations must preserve logical semantics across exposed
interfaces.

Where applicable, parity includes:

- finding count and ordering;
- rule identifiers;
- byte spans;
- Unicode line/column coordinates;
- severity;
- confidence;
- remediation;
- candidate count and ordering;
- candidate kinds and evidence;
- explanation facts;
- supported transformed output.

Representations may differ. Classification semantics may not.

## Disclosure and Response

Confirmed vulnerabilities may result in:

- private fixes;
- regression tests;
- patched releases;
- GitHub Security Advisories;
- reporter credit when requested.

Please allow reasonable time for investigation and remediation before public
disclosure.

## Scope Notes

Detection-quality issues are not automatically security vulnerabilities.

A false positive may be a correctness issue. A false negative becomes directly
security-relevant when it causes sensitive material to cross a documented
safe-to-share boundary or otherwise violates an explicit security guarantee.

When uncertain, prefer private reporting.
