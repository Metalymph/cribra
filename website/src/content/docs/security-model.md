---
title: Security Model
description: Trust boundaries, privacy, and limits of Cribra's security guarantees.
order: 19
---

# Security Model

Cribra is a **local-first, deterministic engine** for detecting, validating, reviewing and transforming sensitive data in caller-provided UTF-8 sources. It does not discover files, access networks, upload content or persist source material. The integrating application owns acquisition, I/O, retention, presentation, access controls and policy.

## Trust boundaries

- The Rust core owns rule semantics, validation, classification, normalization, ordering and transformation behavior. Adapters project these semantics; they are not independent detection engines.
- A `Finding` is a rule-backed classification. A `SensitiveCandidate` is review-only evidence and is **not automatically promoted** or transformed.
- Public findings and candidates contain coordinates and metadata, **not copies of matched secret values**. The caller still owns the original input and any transformed output.
- Whole-source input is caller-provided. Streaming sessions retain bounded source-local execution state; incremental sessions explicitly own the **entire mutable document**, caches and committed report.
- Host applications may log, transmit or persist inputs, outputs and diagnostic data. Cribra's no-network/no-persistence core contract does **not** make the host application private by default.

## What the engine does not guarantee

Cribra cannot guarantee detection of every secret, that ambiguous evidence is safe to ignore, or that transformed content is suitable for every recipient or threat model. A report is not a security certification. Redaction and synthesis are explicit operations over classified findings; review-only candidates require caller policy.

Dropping retained buffers is **not a secure-zeroization guarantee**. Streaming and incremental logical resource budgets are **not hard limits on process heap memory**. Integrators handling highly sensitive inputs should control host logging, copies, persistence, telemetry, process isolation and output destinations.

## Integration checklist

1. Decide which sources may enter the engine and which outputs may leave the host.
2. Treat findings and candidates as separate decision channels.
3. Review transformation policy and transformed output before sharing.
4. Set execution limits appropriate to the source and runtime; handle admission and terminal errors.
5. Avoid recording raw source, transformed source or secret-bearing diagnostics in application logs.

**Source of truth:** `docs/ARCHITECTURE.md`, `docs/INTEROP.md`, `docs/streaming.md`, `docs/incremental.md`.

## Related guides

- [Ownership and transformations](/docs/ownership-and-transformations/)
- [Resource limits and errors](/docs/resource-limits-and-errors/)
- [Troubleshooting](/docs/troubleshooting/)
