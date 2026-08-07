# Silens Scan

Privacy-first Rust engine for detecting secrets and sensitive data.

Silens Scan is a reusable scanning core designed for CLI, WASM/PWA, desktop,
editor and service integrations. Clients provide UTF-8 text; the core handles
detection, validation, deterministic normalization and reporting without
retaining matched secrets in public findings.

## Status

Core scanning, validation, built-in detection, batch execution and optional
parallel batch scanning are implemented.

The current focus is API ergonomics, query/reporting helpers, documentation and
release polish.

## Features

- Deterministic UTF-8 scanning
- Compiled execution engine
- Literal, prefix, suffix and regex rules
- Capture-aware regex projection
- Deterministic and contextual validators
- Public selectable built-in rule pack
- Immutable per-source reports
- Ordered batch scan API
- Optional Rayon-backed parallel batch scanning
- Unicode-aware locations
- Deterministic finding normalization
- Reusable cached built-in scanner
- Local-first and WASM-friendly serial core
- No matched secret value stored in public findings

## Design Goals

- Local-first
- Deterministic
- Privacy-first
- WASM-friendly
- Cross-platform
- Single-thread optimized core
- Parallelism across independent inputs
- Small and explicit public API
- Stable ordering between serial and parallel execution
- Zero-copy or allocation-conscious internals where practical

## Architecture

```text
                         Scanner
                            │
                            ▼
                    CompiledRuleSet
                            │
          ┌─────────────────┼─────────────────┐
          ▼                 ▼                 ▼
    Aho-Corasick        regex engine      specialized
   literal/prefix      pattern/capture      suffix
          │                 │                 │
          └──────────────┬──┴─────────────────┘
                         ▼
                  InternalFinding[]
                rule_index/start/end
                         │
                         ▼
                  validator dispatch
                         │
                         ▼
                AcceptedCandidate[]
                         │
                         ▼
             deterministic normalization
                         │
                         ▼
             line/column finalization
                         │
                         ▼
                     ScanReport
                         │
                         ▼
                  ScanEntry<K>[]
                         │
                         ▼
                  ScanResults<K>
```

A single source is scanned serially through the optimized core. Batch
parallelism, when enabled, distributes independent sources through Rayon while
reusing the same immutable scanner.

## Public API

Core public types include:

- `Scanner`
- `ScannerBuilder`
- `Rule`
- `RuleSpec`
- `RuleId`
- `ScanEntry<K>`
- `ScanResults<K>`
- `ScanReport`
- `Finding`
- `Location`
- `Severity`
- `Confidence`
- `Redaction`

### Batch scanning

Scanning has one batch-oriented shape. A single input is simply a one-element
batch.

```rust
use silens_scan::Scanner;

let scanner = Scanner::default();

let results = scanner.scan([
    ("config.env", "STRIPE_SECRET_KEY=sk_live_example"),
    ("settings.toml", "log_level = \"info\""),
]);

assert_eq!(results.len(), 2);
```

Each result preserves the caller-provided key and original source byte length.

### Parallel scanning

Enable the optional `parallel` feature to scan independent sources concurrently:

```toml
silens-scan = { version = "...", features = ["parallel"] }
```

```rust
let results = scanner.parallel_scan([
    ("config.env", env_source),
    ("settings.toml", toml_source),
    ("service.json", json_source),
]);
```

Serial and parallel execution use the same per-source pipeline and preserve
input order. Silens Scan does not split a single source into chunks and does not
create a private thread pool; Rayon uses the current pool.

## Coordinate Contract

- `start` / `end`: zero-based UTF-8 byte offsets
- `line`: one-based
- `column`: one-based Unicode scalar position

Matched secrets are never stored in public findings.

## Built-in Detection

The current built-in catalog includes rules and validators for:

- GitHub
- Stripe
- Cloudflare
- Slack
- Telegram
- JWT
- AWS
- Azure
- GCP
- password and passphrase fields
- sensitive hashes
- generic API keys, tokens and secrets

The canonical selectable pack is exposed as `builtins::CURRENT`.

## Performance

Benchmarks are run with Criterion on Rust 1.97.1. Numbers below are the current
release baseline and should be treated as workload-specific rather than absolute
hardware-independent guarantees.

### Core scanning

| Workload | Median time | Throughput |
| --- | ---: | ---: |
| Built-ins, 1 KiB | 23.55 µs | 57.47 MiB/s |
| Built-ins, 64 KiB | 259.19 µs | 242.59 MiB/s |
| Built-ins, 1 MiB | 3.815 ms | 262.25 MiB/s |
| 64 custom literal rules | 47.37 µs | 1.29 GiB/s |
| 512 custom literal rules | 49.50 µs | 1.24 GiB/s |
| 64 custom mixed rules | 154.39 µs | 404.81 MiB/s |
| Full built-in pipeline | 258.02 µs | 242.23 MiB/s |

### Match density

For a 64 KiB input:

| Density | Findings | Median time | Throughput |
| --- | ---: | ---: | ---: |
| None | 0 | 214.01 µs | 292.04 MiB/s |
| Sparse | 7 | 264.78 µs | 237.47 MiB/s |
| Dense | 1496 | 3.401 ms | 18.38 MiB/s |

The dense case is intentionally output-heavy: roughly one final finding every
43 bytes. Internal diagnostics on the same fixture currently report 2394 raw
candidates, 1496 accepted candidates and 1496 normalized findings.

### Parallel batch scanning

The benchmark below scans 32 independent sources.

| Source size | Serial | Parallel | Approx. speedup |
| --- | ---: | ---: | ---: |
| 4 KiB × 32 | 1.132 ms | 265.94 µs | 4.26× |
| 64 KiB × 32 | 8.301 ms | 1.342 ms | 6.19× |
| 1 MiB × 32 | 122.93 ms | 18.39 ms | 6.69× |

Parallel throughput reaches approximately:

- 517.5 MiB/s for 4 KiB sources
- 1.46 GiB/s for 64 KiB sources
- 1.70 GiB/s for 1 MiB sources

The serial core remains the unit of work; parallelism is applied only across
independent inputs.

### Build cost

| Ruleset | Median build time |
| --- | ---: |
| Built-in pack, cold compile | 48.72 ms |
| 4 custom literal rules | 15.93 µs |
| 64 custom literal rules | 61.23 µs |
| 512 custom literal rules | 301.71 µs |

The default built-in scanner uses a shared compiled cache, so the cold built-in
compile cost is paid once per process and reused afterward.

## Current Performance Decision

The current engine is considered performance-frozen for this release.

A specialized shared contextual-assignment engine was evaluated but deferred:
the existing capture-aware pipeline is already fast and maintainable, while the
dense benchmark is dominated by the cost of producing a large number of real
findings. A deeper contextual matcher rewrite should be reconsidered only if the
catalog grows substantially or future profiling demonstrates a clear need.

## Roadmap

Completed foundation:

- Core matcher architecture
- Public custom rules
- Compiled rule metadata
- Built-in validators
- Capture-aware pattern projection
- Deterministic normalization
- Built-in compiled cache
- End-to-end fixtures
- Batch scan API
- Optional parallel batch execution
- Performance baseline and diagnostics

Current work:

1. `ScanResults<K>` ergonomics
2. query/filter/sort API
3. `ScanSummary`
4. formatting and serialization surface
5. API documentation and examples
6. README / crate metadata / CI / changelog polish
7. release preparation

Future work:

- redaction and share-safe transforms
- expanded detector catalog
- CLI
- WASM/PWA
- desktop integration
- contextual matcher redesign only if justified by profiling

## License

MIT
