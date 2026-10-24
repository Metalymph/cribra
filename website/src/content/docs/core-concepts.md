---
title: Core concepts
description: Understand caller-owned data, deterministic detection, and the difference between findings and review candidates.
order: 2
---

## Application-controlled data

Cribra is an embeddable, local-first, synchronous Rust engine. Applications own source acquisition, filesystem discovery, networking, persistence, presentation, and policy. Cribra processes the data they supply.

The core owns compiled rules, detection, validation, normalization, reports, queries, explanations, and explicit transformations. It does not crawl repositories, upload sources, or make policy decisions for an application.

Compiled scanner configuration is immutable and reusable. Source-local state belongs to each execution or session. For the full boundary, read the [architecture](https://github.com/Metalymph/cribra/blob/main/docs/ARCHITECTURE.md).

## Detection and validation

`Scanner` and `ScannerBuilder` configure and execute detection. Rules may be literal, prefix, suffix, or regex based. Built-in catalogs add deterministic and contextual validation. Custom rules remain matcher-authoritative; they cannot select private built-in validators.

The current catalog is the default. Personal and financial catalogs are explicit opt-ins, not silently enabled categories. Use known formats and context rather than assuming Cribra detects every possible secret or sensitive value.

Whole-source scanning is the semantic reference. Equivalent supported operations preserve deterministic report ordering and classification; optional Rayon execution distributes independent sources without splitting one logical source across workers.

## Reporting and review

`ScanReport` describes one source; `ScanResults` preserves an ordered batch and exposes queries and aggregate summaries. Public findings contain rule identity, severity, confidence, remediation metadata, and coordinates rather than copies of matched values.

**Findings** are authoritative rule-backed classifications. **Sensitive candidates** are independent review evidence. They are not automatically promoted to findings, assigned finding semantics, or transformed. No findings does not necessarily mean no review is needed.

Spans are half-open UTF-8 byte ranges. Lines and columns are one-based; columns count Unicode scalars. Do not treat byte offsets as JavaScript string indices.

Explanations project existing classification facts rather than making another detection decision. A finding's explanation is resolved against its scanner's compiled metadata.

Continue with [explicit transformations](/docs/transformations/) or the runnable [query example](https://github.com/Metalymph/cribra/blob/main/examples/query_results.rs). API details are available in the [Rust documentation](https://docs.rs/cribra/0.7.2/cribra/).
