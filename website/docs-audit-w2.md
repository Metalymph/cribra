# W2 Documentation Audit

## Evidence baseline

- Audit date: 2026-10-24.
- Working branch: `feature/website-documentation-w2`.
- Rust Core: `cribra` 0.7.2.
- C ABI: `cribra-capi` 0.3.0.
- WASM: `cribra-wasm` 0.4.7.
- CLI: `cribra-cli` 0.4.4.
- UniFFI: `cribra-uniffi` 0.1.0.
- Rustdoc generation verified with `cargo doc -p cribra --no-deps --all-features`.

## Documentation sources

- Public exports in `src/lib.rs`.
- Scanner, builder, rule, report, finding, candidate, explanation, query and summary modules.
- Runnable Rust examples under `examples/`.
- Existing Astro documentation.
- `docs/COMPATIBILITY.md` and adapter qualification records.

## Confirmed inconsistency

The Rust crate documentation previously stated that adapters did not expose
session APIs. Current compatibility records describe session APIs in C ABI,
WASM and UniFFI source surfaces.

The crate-level statement was corrected without changing public APIs.

## W2.1 — Rust API documentation

Added:

- Rust API reference.
- Scanner and configuration.
- Rules and built-ins.
- Findings and reports.
- Queries and summaries.

Updated the documentation index and topic navigation.

Rustdoc remains the detailed method/type reference.

## Compatibility and distribution constraints

Implementation, local qualification, CI qualification and publication are
distinct claims.

- Swift, Python and Kotlin/JVM generated artifacts do not imply remote package publication.
- Kotlin/JVM qualification does not establish Android support.
- Swift compile/link qualification does not imply execution on every Apple target.
- WASM workspace source capabilities may differ from published distribution capabilities.
- C ABI and other adapters have independent version and ownership contracts.
- Rust Core/Flow/Fuse are feature combinations; WASM does not support Fuse.

Consult `docs/COMPATIBILITY.md` for recorded environments and evidence.

## Completed milestones

### W2.2 — Integration guides

Added Rust, Swift, Python, Kotlin/JVM, C ABI and WASM integration guides.
Each documents its implementation, consumer requirements, capabilities,
ownership and distribution boundaries.

### W2.3 — Advanced scanning

Added whole-source, streaming and incremental scanning guides, including
admission, lifecycle, resource limits and failure semantics.

### W2.4 — Security and contracts

Added security, ownership, transformation, resource-limit, error and
compatibility documentation. Canonical contracts remain in `docs/`.

### W2.5 — Developer experience

Added troubleshooting, cross-links and expanded documentation navigation.

### W2.6 — Integration qualification

Completed local qualification for the exercised Rust, Swift, Python,
Kotlin/JVM, C ABI and WASM integration surfaces.

The qualification environments, tested contracts and publication limits
are recorded in `docs/COMPATIBILITY.md`. These checks do not establish
universal platform support or complete cross-language API parity.

### W2.6.7 — Compatibility evidence alignment

Updated the canonical compatibility record, website binding matrix and
website compatibility guide with evidence reviewed on 2026-10-25.

## Validation

- `cargo doc -p cribra --no-deps --all-features`: passed during W2.1.
- `just website-check`: passed after W2.6.7.
- `just website-build`: passed after W2.6.7, including Pagefind generation.
- `git diff --check main...HEAD`: passed during W2.6.8.
- Documentation navigation diff reviewed during W2.6.9.
- Browser visual inspection and independent link-resolution audit are not
  established by these results.
