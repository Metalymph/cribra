---
title: Ownership & Transformations
description: Input ownership, result lifetimes, and explicit share-safe transformations.
order: 20
---

# Ownership & Transformations

## Source and result ownership

The caller supplies source bytes or text. Ordinary scan reports expose metadata and UTF-8 byte spans rather than matched secret strings. Spans are **half-open byte ranges**, not character indices; Unicode line/column metadata uses a different coordinate system. Never slice UTF-8 text with unchecked offsets.

Streaming `push` accepts ordered byte fragments; the caller may reuse the fragment buffer after the call. `finish` consumes a successful session and returns a complete report. No partial result should be treated as a finished report.

An incremental session owns its document and cached state. `source()`, `revision()` and `report()` describe the last committed snapshot. Edits require an expected revision and valid UTF-8 byte boundaries. On successful edits, the revision advances even for a no-op. Old and staged snapshots can coexist during an edit; a caller-owned clone adds separate memory use.

## Explicit transformation policy

Cribra provides redaction, templating, pseudonymization and synthesis in Flow-enabled surfaces. Transformations are caller-requested and operate on **classified findings**, not automatically on `SensitiveCandidate` review evidence. A report alone does not contain original secret values: transformation requires source access. Treat transformed output as potentially sensitive until your application's policy validates it.

Keyed transformation modes require correct key management by the caller. A deterministic or repeatable replacement is not an anonymity guarantee. Review output for residual context, indirect identifiers, candidate-only material and downstream sharing requirements.

## Adapter lifetimes

- **Rust:** ownership and borrows are enforced by the API; cloned reports belong to the caller.
- **C ABI:** opaque handles own native state; borrowed pointer-plus-length views are valid only while their documented owner remains alive. Follow the checked-in [`include/cribra.h`](https://github.com/Metalymph/cribra/blob/main/include/cribra.h) contract and release owned handles.
- **WASM:** generated JavaScript objects and owned results have explicit `free()` lifetimes; do not use them after release.
- **UniFFI Swift/Python/Kotlin:** use a matching generated binding/native-library/profile set. Do not mix profiles or independently built artifacts in one process.

**Source of truth:** [`docs/INTEROP.md`](https://github.com/Metalymph/cribra/blob/main/docs/INTEROP.md), [`docs/ARCHITECTURE.md`](https://github.com/Metalymph/cribra/blob/main/docs/ARCHITECTURE.md), [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md), [`docs/streaming.md`](https://github.com/Metalymph/cribra/blob/main/docs/streaming.md), [`docs/incremental.md`](https://github.com/Metalymph/cribra/blob/main/docs/incremental.md).

## Related guides

- [Findings and reports](/docs/findings-reports/)
- [Explicit transformations](/docs/transformations/)
- [Security model](/docs/security-model/)
