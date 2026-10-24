---
title: Explicit transformations
description: Create transformed derivatives with redaction, templates, keyed pseudonymization, synthesis, and Share Bundles.
order: 3
---

## Transform classified findings

Transformations are explicit caller operations. They require the original source and apply to classified finding spans. Review-only candidates are never transformed automatically.

Flow enables four operations:

- **Redact:** replace classified spans with a redaction marker.
- **Template:** replace findings with semantic placeholders; overlapping findings are rejected.
- **Pseudonymize:** use an explicit caller-owned key for reproducible pseudonyms.
- **Synthesize:** produce deterministic keyed replacements according to Core's documented semantics.

Transformations are enabled by default in Rust. They are absent from a build using `default-features = false`. See [profile and adapter coverage](/bindings/) before assuming an operation exists in another integration.

## Create a redacted derivative

From the existing [financial review example](https://github.com/Metalymph/cribra/blob/main/examples/financial_review.rs), after scanning caller-owned `source` with the selected scanner:

```rust
use cribra::transform::redact;

let results = scanner.scan([("checkout.env", source)]);
let report = results.single_report().expect("one source was scanned");
let safe_copy = redact(source, report)?;
```

This is an excerpt; the complete example includes scanner construction, explicit financial catalog selection, and synthetic input. Run `cargo run --example financial_review` from the repository root.

## Share Bundles

Share Bundles collect transformed sources and a Core-owned manifest. The [share_safe example](https://github.com/Metalymph/cribra/blob/main/examples/share_safe.rs) builds a redacted bundle from scan results. Where scanning and transformation do not need separate results, prefer the documented atomic scan-and-build path so the same source collection supplies both operations.

The Rust and adapter in-memory bundle APIs do not themselves define application storage or transport policy. The CLI has a separate explicit-file directory export contract on macOS/Linux.

## Keep output policy explicit

Candidates and undetected values may remain in transformed output. A derivative is not a guarantee of anonymity or secret-free content. Reusing a pseudonymization key enables correlation; applications own key generation, storage, access, and sharing decisions.

See the [transformation API](https://docs.rs/cribra/0.7.2/cribra/transform/index.html) and [CLI transformation/export contract](https://github.com/Metalymph/cribra/blob/main/docs/CLI_PARITY.md).
