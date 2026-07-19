# Silens Scan

Privacy-first Rust engine for detecting secrets and sensitive data.

Silens Scan is the reusable open-source scanning core of the Silens ecosystem. It is designed to power:

- `silens scan` in Silens CLI
- the client-only Silens Scan WASM/PWA on `silenshq.com`
- Silens Scan+ inside Silens Studio

## Status

Early project bootstrap. The public API is not stable yet.

## Principles

- Local-first and network-free by default
- No account or authentication requirement
- Deterministic structured findings
- Precise source locations
- Safe redaction by default
- Reusable across native and WebAssembly targets
- Independent from Silens Siren

## Repository role

This repository contains the scanner engine only. Terminal, browser, desktop, authentication, and cloud concerns belong to separate clients or services.

## License

MIT
