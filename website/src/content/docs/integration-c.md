---
title: "C ABI integration"
description: "Use the native C ABI 0.3.0 with explicit handle and error management."
order: 14
---

# C ABI integration

> **Source and distribution boundary:** This guide describes the repository
> contracts recorded in the W2.2 audit. Source implementation, local tests,
> CI qualification, and publication are distinct claims. Consult the linked
> adapter README and compatibility matrix before shipping binaries.

Cribra performs deterministic scanning in Rust Core. A **finding** is an
authoritative detection; a **sensitive candidate** is review-only and must not
be silently transformed. Spans are zero-based, half-open **UTF-8 byte ranges**.
Streaming admits only compatible rulesets; an ordinary default scanner is not
automatically streamable. Resource budgets are logical limits, not process-wide
heap guarantees.

## Distribution and contract

The `cribra-capi` adapter exposes a native C ABI with protocol version
**0.3.0**. It can be built from source as native libraries and ships a
checked-in header; its Cargo manifest sets `publish = false`. No binary
package publication is established by these facts.

## Start with the checked-in example

Use [`examples/c/scan.c`](https://github.com/Metalymph/cribra/blob/main/examples/c/scan.c) together with the checked-in header and
[`docs/INTEROP.md`](https://github.com/Metalymph/cribra/blob/main/docs/INTEROP.md). The C ABI has its own function signatures and resource
ownership; do not translate Rust method names directly into assumed C symbols.

## Ownership and errors

The C boundary uses opaque native handles and explicit release functions.
Follow each documented lifetime, especially for borrowed views and results.
Keep headers and native libraries from matching ABI builds together.

Version 0.3.0 exposes scanner metadata and structured construction-error
views. Prefer structured error fields to parsing diagnostic strings.

## Streaming and incremental documents

The native ABI supports Core-owned append-only byte streaming for admitted
rulesets and incremental UTF-8 documents with revisioned byte edits and
coherent snapshots. Its incremental API accepts four optional logical
budgets. Terminal Core failures preserve the previous committed snapshot;
a contained panic invalidates the adapter session. Neither budget category
is a total-process memory cap.

## Source references

- [`docs/INTEROP.md`](https://github.com/Metalymph/cribra/blob/main/docs/INTEROP.md)
- [`examples/c/scan.c`](https://github.com/Metalymph/cribra/blob/main/examples/c/scan.c)
- [`crates/cribra-capi/`](https://github.com/Metalymph/cribra/tree/main/crates/cribra-capi/)
- [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md)
