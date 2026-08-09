# Canonical Silens Scan fixture corpus

This directory is the canonical, committed example corpus used by Silens Scan
documentation and golden tests.

## Safety

Every sensitive-looking value in `inputs/` is deliberately synthetic and uses
Silens-specific demo token families. These files must never contain real
provider credentials.

The corpus is designed to demonstrate engine behaviour without relying on live
GitHub, Stripe, AWS, Slack, or other provider token formats.

## Layout

- `inputs/` contains hand-maintained source fixtures.
- `outputs/reports/` contains structured JSON produced by the real crate.
- `outputs/redacted/` contains `transform::redact` output.
- `outputs/templates/` contains `transform::template` output.
- `outputs/pseudonymized/` contains deterministic keyed pseudonyms.
- `outputs/synthesized/` contains deterministic synthetic replacements.

Files under `outputs/` are generated artifacts. Do not edit them manually.

Regenerate intentionally with:

```text
cargo run --example generate_fixtures --features serde
```

Golden tests added in the next hardening step compare fresh crate output against
these committed files and never overwrite them.
