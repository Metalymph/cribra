---
title: "Python integration"
description: "Use local UniFFI consumers or verified platform-specific Fuse wheels."
order: 12
---

# Python integration

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

## Distribution and profiles

The generated Python module is `cribra_uniffi`. The repository provides
local Core, Flow and Fuse source consumers; its wheel packaging path produces
**Fuse** wheels. **PyPI publication is not established.** Do not present
`pip install cribra` from the public index as a verified installation path.

| Profile | Capabilities |
| --- | --- |
| Core | Scan, configure, batch, stream, incremental and snapshots |
| Flow | Core plus transforms and Share Bundles |
| Fuse | Flow plus parallel scan |

## Reproduce source consumer checks

From the repository root, with Rust 1.97, Python **3.13+** and `just`:

```sh
just uniffi-python-smoke
just uniffi-python-flow-smoke
just uniffi-python-fuse-smoke
```

Each runner generates the profile-specific `cribra_uniffi.py` and matching
native library in ignored `target/` output and tests them in isolation.
Generated bindings from one profile must not load another profile's library.

```python
import cribra_uniffi
```

The import requires a generated module and its matching native binary to be
on the consumer's loading path; the import alone is not an installation step.
See the README's installed-wheel workflow for producing and verifying a wheel.

## Wheel qualification

Installed Fuse wheels were tested with CPython 3.11–3.14 on the recorded
platforms. Source smoke requirements differ from installed-wheel interpreter
requirements. Linux `linux_x86_64` and repaired `manylinux_2_28_x86_64` wheels
are distinct artifacts. Do not infer musl, ARM64 Linux/Windows, Intel macOS,
or every older macOS runtime from these results.

Incremental edits are UTF-8 byte-based; coherent snapshots should be used when
a revision/report pair must describe one committed state.

## Source references

- [`bindings/python/README.md`](https://github.com/Metalymph/cribra/blob/main/bindings/python/README.md)
- [`crates/cribra-uniffi/README.md`](https://github.com/Metalymph/cribra/blob/main/crates/cribra-uniffi/README.md)
- [`docs/COMPATIBILITY.md`](https://github.com/Metalymph/cribra/blob/main/docs/COMPATIBILITY.md)
