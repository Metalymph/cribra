# Architecture

## Scope

`cribra` is a reusable Rust library.

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
Cribra
├── Generic consumers: Rust apps, middleware, services, custom tooling
├── Silens Scan / Scan+: WASM/PWA
└── Silens Studio: desktop application
```

## Dependency direction

Consumers may depend on `cribra`.

Cribra must not depend on those consumers or on Silens Siren.
