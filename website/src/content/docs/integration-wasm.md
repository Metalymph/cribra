---
title: "WebAssembly integration"
description: "Build typed wasm-bindgen Core/Flow adapters without assuming source-to-registry parity."
order: 15
---

# WebAssembly integration

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

## Published package versus current source

The published `cribra-wasm` **0.4.7** registry contract depends on Rust Core
`^0.6`, while the current workspace source uses Core `^0.7`. A source API
being implemented or locally qualified does **not** establish its presence
in the published package. Use a matching source build for newer session,
batch, configuration, and Share Bundle APIs.

## Configurations

| WASM profile | Cargo flags | Capabilities |
| --- | --- | --- |
| Core | `--no-default-features` | Scan, report, source streaming/incremental, sequential batches |
| Flow (default) | `--no-default-features --features transform` | Core plus transformations and in-memory Share Bundles |
| Fuse | Unsupported | No browser Fuse profile |

Build for the browser-compatible target and then process with `wasm-bindgen`:

```sh
cargo build -p cribra-wasm --release --target wasm32-unknown-unknown --no-default-features
# Or Flow:
cargo build -p cribra-wasm --release --target wasm32-unknown-unknown --no-default-features --features transform
```

## Runtime and lifecycle

The typed adapter is a `wasm-bindgen` boundary, not the native C ABI or JSON
transport. Current source supports admitted byte streams, revisioned UTF-8
incremental documents, ordered sequential batches, and Flow-only in-memory
Share Bundles. Share Bundles are not archives or implicit persistence formats.

Follow generated JavaScript object ownership and `free()` requirements.
Browser storage, Workers, service workers, CSP and UI policy remain the
integrating application's responsibility.

The audit distinguishes local Node/Bun source tests, earlier browser checks,
and CI that predates newer session APIs. It does not establish universal
browser/runtime coverage or published parity for the new APIs.

## Source references

- `crates/cribra-wasm/README.md`
- `examples/wasm-browser/README.md`
- `examples/wasm-runtime/README.md`
- `docs/COMPATIBILITY.md`
