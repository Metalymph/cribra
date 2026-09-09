# Cribra Roadmap

Cribra is a small, application-agnostic security engine. The roadmap favors
correctness, high-confidence detection, semantic parity across interfaces and
safe transformation behavior over feature breadth.

Cribra does not schedule major versions merely to create feature churn.
After v0.4.3, development moves to maintenance and hardening mode until a
concrete security, correctness, interoperability or broadly reusable capability
requirement justifies another feature release.

---

## v0.4.2 — Security Hardening

Status: in progress.

v0.4.2 hardens security-sensitive boundaries in the existing Cribra core
without introducing a new architecture.

### 0.4.2-A — Source/result transformation consistency

- [x] Reproduce the same-length stale-source transformation hazard.
- [x] Add an atomic scan-and-build path that scans and transforms the same
  borrowed source values.
- [x] Preserve the existing lower-level `build` API for compatibility.
- [x] Document that source count and byte length are sanity checks, not source
  identity proof.
- [x] Avoid retaining source material or introducing source fingerprints into
  public metadata.

### 0.4.2-B — Transformation semantic safety

- [x] Add transform-to-rescan regression coverage.
- [x] Verify redaction output does not remain review-worthy under the same
  canonical scanner.
- [x] Verify template output does not remain review-worthy.
- [x] Verify pseudonymized output does not remain review-worthy.
- [x] Verify synthesized output does not remain review-worthy.
- [x] Preserve deterministic transformation behavior.

### 0.4.2-C — Generic private-key detection

- [x] Detect PKCS#8 private-key PEM blocks.
- [x] Detect encrypted PKCS#8 private-key PEM blocks.
- [x] Detect RSA private-key PEM blocks.
- [x] Detect EC private-key PEM blocks.
- [x] Detect OpenSSH private-key blocks.
- [x] Reject incomplete, mismatched, public-key and certificate blocks.
- [x] Preserve provider-specific priority when a generic private-key detector
  overlaps stronger contextual evidence.

### 0.4.2-D — HTTP Bearer credentials

- [x] Detect explicit `Authorization: Bearer` credentials.
- [x] Project only the credential value.
- [x] Preserve contextual detection semantics.
- [x] Reject Basic authentication, missing Bearer schemes, short values,
  placeholders and misleading header names such as `Proxy-Authorization`.

### 0.4.2-E — GCP escaped private keys

- [x] Detect JSON-escaped GCP service-account private keys.
- [x] Preserve exact raw-source byte projection without decoding JSON strings.
- [x] Keep literal multiline PEM and escaped JSON representations semantically
  distinct.
- [x] Reject incomplete and mismatched escaped private-key structures.

### 0.4.2-F — Documentation and security alignment

- [ ] Align the roadmap with the current maintenance line.
- [ ] Update supported security release lines.
- [ ] Document the new security-sensitive transformation boundary.
- [ ] Update the built-in detection catalog where documented.
- [ ] Record v0.4.2 changes in the changelog.
- [ ] Remove speculative future-version commitments.

### 0.4.2-G — Final regression and release gate

- [ ] Run the complete Rust default-feature test suite.
- [ ] Run the complete all-features test suite.
- [ ] Validate Serde contracts.
- [ ] Validate serial/parallel semantic equivalence.
- [ ] Validate native C ABI regression coverage.
- [ ] Validate WASM build and browser-facing adapter compatibility.
- [ ] Re-run Rust/WASM semantic parity.
- [ ] Run formatting, lint, doctest, audit and package validation.
- [ ] Validate release metadata and packaged artifacts.
- [ ] Finalize v0.4.2 release notes.

No additional detection families are planned for v0.4.2.

---

## v0.4.3 — High-Confidence Detection Coverage

Status: planned.

v0.4.3 expands detection only where strong structure or context provides
useful security coverage without turning Cribra into a broad DLP engine.

### 0.4.3-A — GitLab credentials

- [ ] Add high-confidence GitLab token families with documented prefixes.
- [ ] Cover personal/project/group access-token families where the public shape
  is sufficiently stable.
- [ ] Cover high-confidence OAuth, deploy, runner and CI token families where
  appropriate.
- [ ] Add provider-specific validation and placeholder rejection.
- [ ] Document the limitation that GitLab Self-Managed deployments may use
  customized token prefixes.

### 0.4.3-B — Database connection credentials

- [ ] Detect credential-bearing PostgreSQL connection strings.
- [ ] Detect credential-bearing MySQL connection strings.
- [ ] Detect credential-bearing Redis/Rediss connection strings.
- [ ] Detect credential-bearing MongoDB/MongoDB SRV connection strings.
- [ ] Project only the sensitive credential span rather than the complete URI.
- [ ] Preserve exact raw-source offsets when percent-encoded credentials are
  present.
- [ ] Reject connection strings without credentials.

### 0.4.3-C — Quoted password and passphrase values

- [ ] Detect strongly contextual quoted credential values containing
  whitespace.
- [ ] Preserve existing conservative unquoted credential rules.
- [ ] Project only the quoted value contents.
- [ ] Reject placeholders and weak/unrelated fields.

### 0.4.3-D — Adversarial detection corpus

- [ ] Add false-prefix and truncated-token cases.
- [ ] Add documentation/example placeholder cases.
- [ ] Add provider near-misses.
- [ ] Add connection-string near-misses.
- [ ] Add malformed and ambiguous credential forms.
- [ ] Preserve deterministic collision and normalization behavior.

### 0.4.3-E — HTTP Basic authentication

- [ ] Detect explicit `Authorization: Basic` credentials.
- [ ] Require structurally valid Base64.
- [ ] Validate locally that decoded material has credential structure.
- [ ] Keep the finding projected onto the encoded source credential.
- [ ] Do not introduce generic Base64 detection.

### 0.4.3-F — PGP private keys

- [ ] Detect armored PGP private-key blocks.
- [ ] Require matching private-key delimiters and non-trivial body content.
- [ ] Reject public-key blocks and malformed structures.
- [ ] Use critical private-key remediation semantics.

### 0.4.3-G — Docker registry credentials

- [ ] Detect Docker configuration `auth` credentials only under sufficiently
  strong Docker configuration context.
- [ ] Validate Base64 credential structure locally.
- [ ] Avoid generic Base64 classification.
- [ ] Project only the encoded credential value.

### 0.4.3-H — `.netrc` credentials

- [ ] Evaluate a narrow `machine` / `login` / `password` grammar.
- [ ] Add detection only if false-positive behavior remains acceptably strict.
- [ ] Do not broaden generic configuration-key parsing solely for `.netrc`.

This slice is optional and may be omitted from v0.4.3 if the required grammar
would weaken Cribra's contextual precision.

### 0.4.3-I — Cross-interface semantic parity

- [ ] Validate the complete new detector corpus through the Rust API.
- [ ] Validate native C ABI compatibility.
- [ ] Validate WebAssembly compatibility.
- [ ] Preserve equivalent findings, spans, metadata and ordering across
  supported interfaces.

### 0.4.3-J — Documentation and release gate

- [ ] Update the public detector catalog and limitations.
- [ ] Update security and interoperability documentation where necessary.
- [ ] Run the complete release gate.
- [ ] Publish v0.4.3 release notes.

---

## After v0.4.3

No feature release is currently scheduled.

Cribra enters maintenance and security-hardening mode after v0.4.3.

Future releases should be driven by concrete evidence such as:

- a correctness or security issue;
- a high-confidence broadly useful credential family;
- an interoperability requirement;
- a measurable performance problem;
- a privacy or transformation-safety improvement;
- a reusable capability that remains application-agnostic.

The following remain intentionally outside the planned scope unless future
evidence changes the trade-off:

- generic entropy scanning;
- generic Base64 scanning;
- broad PII or DLP classification;
- credit-card detection;
- arbitrary hash classification;
- AI-based secret classification;
- provider network validation;
- filesystem or repository traversal inside the core;
- cloud SDK dependencies;
- application-specific policy engines;
- speculative plugin architecture.

---

## Historical releases

### v0.3 — Native Interoperability

Completed in v0.3.0.

[Keep the existing detailed v0.3 checklist below this heading.]

### v0.4 — WASM Interoperability

Completed in v0.4.0, followed by the independently published
`cribra-wasm 0.4.1` distribution release.

[Keep the existing detailed v0.4 checklist below this heading.]