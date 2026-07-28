# Silens Scan

Privacy-first Rust engine for detecting secrets and sensitive data.

## Status

**S1.2 completed**.

### Features

-   Deterministic UTF-8 scanning
-   Compiled execution engine
-   Literal, prefix, suffix and regex rules
-   Immutable reports
-   Unicode-aware locations
-   Local-first, synchronous API

## Design Goals

-   Local-first
-   Deterministic
-   Privacy-first
-   WASM-friendly
-   Cross-platform
-   Single-thread optimized first

## Architecture

``` text
Scanner -> CompiledRuleSet -> InternalFinding -> Sort -> UTF-8 Positions -> ScanReport
```

## Public API

Scanner, ScannerBuilder, Rule, RuleSpec, RuleId, Finding, Location,
Severity, Confidence, Redaction, ScanReport.

## Coordinate Contract

-   start/end: zero-based UTF-8 bytes
-   line: one-based
-   column: one-based Unicode scalar

Matched secrets are never stored in public findings.

## Roadmap

✅ S0 ✅ S1.1 ✅ S1.2 🚧 S1.3 S1.4 S1.5 S2 S3 S4 S5

## License

MIT
