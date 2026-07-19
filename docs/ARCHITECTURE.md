# Architecture

## Scope

`silens-scan` is a reusable Rust library.

It owns:

- scanning orchestration
- detection rules
- findings
- source locations
- severity and confidence metadata
- safe redaction contracts

It does not own:

- terminal presentation
- browser UI
- desktop UI
- authentication
- cloud synchronization
- Silens Siren monitoring

## Consumers

```text
silens-scan
├── Silens CLI: `silens scan`
├── Silens Scan WASM/PWA
└── Silens Studio: Scan+
```

## Dependency direction

Consumers may depend on `silens-scan`.

The scanner must not depend on those consumers or on Silens Siren.
