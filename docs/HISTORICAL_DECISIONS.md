# Retained architectural decisions

The concise roadmap replaces the historical checklist, not its semantic policy.
This D5 review compared it with `main` at
[`4408050`](https://github.com/Metalymph/cribra/blob/4408050fa722f83d21653c10ae7faab1ea9d14c8/ROADMAP.md).
The old document remains available in Git history. These decisions still matter:

- **Conservative detector evolution:** after v0.4.7, routine portfolio growth is
  complete. New detectors require a demonstrated production gap and strong
  structural/contextual evidence. Deliberate false negatives are preferable to
  noisy classification. Financial and personal-data packs remain explicit opt-ins;
  public certificates, arbitrary encrypted/DPAPI blobs and generic Kubernetes
  Base64 are not secret classifications merely because of their representation.
- **Deferred developer credentials:** `DENO_AUTH_TOKENS` multi-entry parsing and
  `NuGetPackageSourceCredentials_{name}` exact password/source association were
  deferred; no dedicated rules were promised. Their source rationale is in
  [v0.4.5 developer coverage](https://github.com/Metalymph/cribra/blob/4408050fa722f83d21653c10ae7faab1ea9d14c8/ROADMAP.md#L2660-L2679).
- **Shared ownership over artificial language detectors:** Gradle/Maven/AWS and
  provider-specific rules own applicable Kotlin credential semantics. SwiftPM
  reuses `.netrc` when equivalent. Dart/pub URL configuration and arbitrary
  environment-variable references do not establish credential attribution; no
  Dart/pub-specific detector was justified. OS credential-store extraction,
  language runtimes and package-manager parsers were not introduced.
- **Personal-data evidence:** Codice Fiscale, PESEL, NHS Number and contextual
  SSN validation do not establish registry assignment or holder identity. ICAO
  MRZ remains deferred; structural recognition is not automatic sensitive-data
  classification. [Coverage](COVERAGE.md) retains the authoritative category
  inventory and rationale.
- **Adapter authority:** C/WASM and future native bindings project Rust semantics.
  Missing applicable capabilities are recorded gaps; genuinely inapplicable
  surfaces are explicit N/A. Binding readiness needs versioned conformance,
  lifetime/error contracts, ABI guarantees and consumer tests. CLI parity and
  Homebrew/APT distribution remain independent commitments, tracked in the
  [roadmap](../ROADMAP.md) and [CLI matrix](CLI_PARITY.md).
- **Streaming scope:** the original v0.5 phase notes explored broader admission,
  pre-EOF delivery, reader/async helpers and whole-source executor migration.
  They are not outstanding requirements of the released admitted-subset contract.
  The authoritative current restrictions are [streaming](streaming.md) and
  [compatibility](streaming-compatibility.md); any expansion needs new evidence.

Historical release and qualification documents retain their original dates and
results. They are evidence snapshots, not claims that later edits or every
platform were verified. Current mutable-document contracts are documented in
[incremental](incremental.md), independently of streaming eligibility.
