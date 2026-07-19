# Roadmap

Silens Scan has an independent lifecycle from Silens Siren and Silens CLI.

## S0 — Foundation

- [x] Initialize the public Rust library repository
- [x] Define scope, ownership, and privacy constraints
- [x] Add baseline validation and continuous integration

## S1 — Core Scanner

- [ ] Define input, rule, finding, location, severity, confidence, and redaction models
- [ ] Implement deterministic in-memory scanning
- [ ] Add an initial high-confidence secret rule set
- [ ] Add false-positive controls
- [ ] Add safe output contracts
- [ ] Add focused unit and integration tests

## S2 — Stable Integration API

- [ ] Stabilize the API consumed by Silens CLI
- [ ] Add filesystem adapters outside the pure scanning layer
- [ ] Define machine-readable serialization contracts
- [ ] Prepare the first crates.io release

## S3 — WebAssembly and PWA

- [ ] Validate supported WASM targets
- [ ] Add a thin WASM binding layer
- [ ] Integrate with the client-only Silens Scan PWA
- [ ] Preserve offline operation in the browser

## S4 — Studio Integration

- [ ] Integrate the engine into Silens Studio
- [ ] Support Scan+ workflows without weakening the open core

## Permanent constraints

- The core must not depend on Silens Siren.
- Local scanning must not require authentication.
- Network access must never become mandatory.
- Frontends must consume the engine rather than reimplement detection.
