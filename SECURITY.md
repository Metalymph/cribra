# Security Policy

Cribra is a privacy-first, local-first Rust engine for detecting, querying, and
safely transforming secrets and sensitive data.

Security issues should be reported privately whenever possible.

## Supported Versions

The following release lines are currently supported for security fixes:

| Version | Supported |
| --- | --- |
| 0.3.x | Yes — development line |
| 0.2.x | Yes |
| 0.1.x | No |
| < 0.1 | No |

Until Cribra reaches 1.0, support may move forward with active minor release
lines. Older unsupported releases may still receive documentation updates, but
security fixes are not guaranteed.

## Reporting a Vulnerability

Please do **not** open a public GitHub issue for a suspected vulnerability.

Use GitHub Private Vulnerability Reporting for this repository whenever
available.

A useful report should include:

- affected Cribra version or commit;
- affected feature set (`default`, `serde`, `parallel`, native C ABI, WASM);
- operating system and architecture where relevant;
- a minimal reproducer or test case;
- expected behavior;
- observed behavior;
- security or privacy impact;
- whether sensitive material may have been exposed;
- any known workaround.

Do not include real production credentials, private keys, customer data, or
other sensitive material in the report. Use synthetic test values instead.

## Security-Relevant Areas

Reports are especially valuable when they concern:

- false negatives that could cause sensitive material to be treated as safe;
- false candidate/finding promotion that violates documented classification
  semantics;
- redaction, templating, pseudonymization, synthesis, or `ShareBundle`
  behavior that leaks or preserves sensitive source material unexpectedly;
- span, overlap, or source/report consistency bugs that can produce unsafe
  transformed output;
- accidental serialization of original source material;
- violations of Cribra's metadata-only serialization boundary;
- secrets or sensitive values appearing in errors, diagnostics, explanations,
  manifests, logs, or public result objects;
- C ABI memory-safety issues, including invalid ownership, lifetime, buffer,
  pointer, or destruction behavior;
- Rust panics or unwinding crossing the C ABI boundary;
- ABI behavior that can cause undefined behavior for a conforming caller;
- WASM boundary issues that expose source material or violate documented
  local-first/privacy guarantees;
- concurrency or parallel-execution bugs that change semantic results or
  ordering;
- dependency or supply-chain vulnerabilities with practical impact on Cribra.

## Privacy Boundary

Cribra is designed so that original source material remains caller-controlled.

Public findings, sensitive candidates, explanations, remediation metadata,
summaries, manifests, and serialized metadata must not contain matched secret
values or original source text unless an API explicitly and intentionally
returns transformed source content.

Cribra itself does not require network access to scan or transform data.

A report showing that source material crosses one of these boundaries is
considered security-relevant.

## Native C ABI

The native ABI is experimental during the 0.3 series but is designed as a
long-lived interoperability boundary.

Important invariants include:

- Rust object layouts are never part of the public C ABI;
- caller input is length-delimited and explicitly validated where applicable;
- Rust panics must not unwind across an exported FFI boundary;
- Rust-owned allocations are released only through their documented Cribra
  destruction function;
- every successful allocation returned across the ABI has exactly one
  documented destruction path;
- borrowed views are valid only for the documented lifetime of their owner;
- matched sensitive values must not appear in ABI metadata or diagnostics.

Use-after-free, double-free, forged handles, invalid pointer/length regions, and
concurrent destruction while an object is in use remain violations of the
caller contract unless explicitly documented otherwise.

## WASM

WASM is an independent adapter over the same Cribra core and does not pass
through the C ABI.

Security reports concerning browser/WASM integration should include, where
relevant:

- browser/runtime;
- bundler or loader;
- Web Worker usage;
- initialization path;
- serialization path;
- CSP constraints;
- whether the issue is reproducible in the core Rust API.

## Disclosure and Response

Security reports will be assessed according to severity, exploitability,
privacy impact, and affected release lines.

When a vulnerability is confirmed, maintainers may:

- prepare a private fix;
- add regression tests;
- publish a patched release;
- publish a GitHub Security Advisory when appropriate;
- credit reporters who wish to be acknowledged.

Please allow reasonable time for investigation and remediation before public
disclosure.

## Scope Notes

Detection quality issues are not automatically security vulnerabilities.

For example, a false positive may be a correctness issue rather than a security
issue. However, a false negative that causes sensitive material to pass through
a documented safe-to-share transformation boundary may have direct security
impact and should be reported privately.

When in doubt, prefer private reporting.
