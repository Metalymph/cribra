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

## Remaining milestones

### W2.2 — Integration guides

Rust, Swift, Python, Kotlin/JVM, C ABI and WASM consumer procedures, ownership,
build requirements, capability boundaries and distribution status.

### W2.3 — Advanced scanning

Whole-source, streaming, incremental, admission rules, lifecycle, budgets and
failure semantics, using runnable examples.

### W2.4 — Security and contracts

Security model, source ownership, transformation limitations, guarantees,
resource limits, error handling and compatibility.

### W2.4 generated documentation

- Security model and trust boundaries (`security-model.md`).
- Ownership and explicit transformations (`ownership-and-transformations.md`).
- Streaming/incremental limits and error contracts (`resource-limits-and-errors.md`).
- Compatibility and distribution status (`compatibility-and-distribution.md`).

These pages summarize the recorded source contracts; canonical detailed evidence remains in `docs/`.

### W2.5 — Developer experience

Troubleshooting, cross-links, examples, navigation and final validation.

## Validation

Validation results must be recorded after executing the project checks.

## W2.5 — Developer experience and final review

- Added troubleshooting covering streaming admission/EOF, incremental revision and terminal errors, resource budgets, candidates, adapter distribution, and output policy.
- Connected previously isolated W2.4 contracts to related guides and improved scanning-mode discovery.
- Expanded documentation topic navigation without changing public Rust APIs.
- Verify build, Pagefind, link resolution, examples, and desktop/mobile light/dark presentation before final PR.
- This entry records planned checks, not their successful execution.
