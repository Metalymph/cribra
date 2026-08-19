# Cribra v0.2 performance baseline

This document records the benchmark policy for the v0.2 performance-regression
phase.

## Benchmark surfaces

The existing `benches/scan.rs` suite remains the historical engine baseline:

- scanner build cost;
- current built-in input-size scaling;
- match density;
- literal-only and mixed custom pipelines;
- custom-rule scaling;
- optional serial/parallel batch scanning.

`benches/v02.rs` adds measurements for work introduced or materially expanded
during v0.2:

- ambiguous candidate scanning with none/sparse/dense candidate density;
- a realistic source containing classified findings plus one ambiguous
  candidate;
- typed explainability projection for findings and candidates;
- ambiguity promotion through explicit custom domain knowledge.

`benches/contextual_tail.rs` isolates the contextual-pattern tail and measures
individual and incremental contextual rule costs. Together with
`benches/isolation.rs`, it is diagnostic tooling rather than a portable
performance promise.

## Policy

Performance work follows measurement rather than intuition.

A benchmark regression does not automatically justify optimization. Before
changing the engine:

1. reproduce the regression on the same machine/toolchain;
2. compare medians and confidence intervals rather than one sample;
3. confirm the workload still represents intended public semantics;
4. profile the concrete hot path;
5. optimize only when the cost is meaningful relative to detection quality,
   correctness and maintainability.

Detection quality and semantic correctness take priority over benchmark wins.

## Running

Historical/current engine suite:

```text
cargo bench --bench scan --all-features
```

v0.2-specific suite:

```text
cargo bench --bench v02 --all-features
```

Run both suites on the same machine with the same Rust toolchain before
comparing results.

Criterion stores measurements under `target/criterion/`. These generated
artifacts are local benchmark data and should not be committed.

## Baseline recording

For each release-oriented baseline, record at least:

- Rust version;
- OS and architecture;
- CPU model;
- benchmark command;
- median time and throughput for the representative workloads;
- meaningful delta from the previous comparable baseline.

Numbers are engineering baselines for that environment, not portable
performance guarantees.


## Diagnostic isolation

During the v0.2 regression investigation, `benches/isolation.rs` separates the
cost of the candidate path from built-in matcher families.

The clean 64 KiB workload measures:

- an empty scanner;
- deterministic prefix rules;
- deterministic regex rules;
- all deterministic built-ins;
- contextual prefix rules;
- contextual capture-aware regex rules;
- all contextual built-ins;
- the complete `builtins::CURRENT` pack.

A second group scales the number of contextual capture-aware patterns from one
rule to the complete contextual pattern set. This is diagnostic tooling rather
than a portable performance promise.

Run it with:

```text
cargo bench --bench isolation --all-features
```

The purpose is to identify the concrete family responsible for a regression
before changing scanner internals.

## v0.2 final regression baseline

The v0.2 regression investigation ended after replacing repeated full-source
contextual-pattern gating with a shared contextual prefilter gate while
retaining rule-local authoritative validation. Prefilters only skip impossible
work; normal matcher and validator semantics remain authoritative.

Final measurements below were recorded with Rust 1.97.1 on the same development
machine used throughout the regression investigation. They are local engineering
baselines, not portable guarantees.

### Historical `scan` suite

| Workload | Median time | Throughput |
| --- | ---: | ---: |
| built-ins, 1 KiB | 34.586 µs | 39.127 MiB/s |
| built-ins, 64 KiB | 535.27 µs | 117.47 MiB/s |
| built-ins, 1 MiB | 8.0754 ms | 123.88 MiB/s |
| match density: none, 64 KiB | 212.41 µs | 294.24 MiB/s |
| match density: sparse, 64 KiB | 528.82 µs | 118.90 MiB/s |
| match density: dense, 64 KiB | 4.5399 ms | 13.767 MiB/s |
| custom literal-only, 64 rules | 75.227 µs | 830.82 MiB/s |
| custom mixed, 64 rules | 177.81 µs | 351.50 MiB/s |
| full built-in pipeline | 529.28 µs | 118.09 MiB/s |
| custom rule count: 512 | 74.567 µs | 839.61 MiB/s |

Cold construction of `builtins::CURRENT` measured 49.266 ms. The default
built-in scanner remains cached, so this is a construction cost rather than a
per-scan cost.

### v0.2 semantic-path suite

| Workload | Median time | Throughput |
| --- | ---: | ---: |
| candidate path: none, 64 KiB | 216.62 µs | 288.53 MiB/s |
| candidate path: sparse, 64 KiB | 247.75 µs | 252.27 MiB/s |
| candidate path: dense, 64 KiB | 262.86 µs | 237.77 MiB/s |
| candidate path: sparse, 1 MiB | 4.0086 ms | 249.47 MiB/s |
| realistic mixed source, 64 KiB | 268.29 µs | 233.39 MiB/s |
| ambiguity promotion: generic candidate | 284.60 µs | 219.61 MiB/s |
| ambiguity promotion: custom finding | 637.07 µs | 98.105 MiB/s |

Typed explainability projection remained nanosecond/sub-nanosecond scale and
was not a scanning bottleneck.

### Contextual isolation

| Matcher family, clean 64 KiB | Median time | Throughput |
| --- | ---: | ---: |
| empty scanner | 35.629 µs | 1.7131 GiB/s |
| deterministic prefixes | 123.33 µs | 506.76 MiB/s |
| deterministic patterns | 40.000 µs | 1.5259 GiB/s |
| contextual prefixes | 37.518 µs | 1.6268 GiB/s |
| contextual patterns | 124.36 µs | 502.58 MiB/s |
| all contextual built-ins | 125.34 µs | 498.66 MiB/s |
| complete `builtins::CURRENT` | 217.73 µs | 287.06 MiB/s |

The contextual-pattern count diagnostic measured about 126 µs at 4, 8 and 15
contextual patterns on the clean 64 KiB fixture. This confirms that the shared
gate prevents clean-path cost from scaling linearly with the contextual regex
portfolio.

### Interpretation

v0.2 deliberately performs more semantic work than v0.1: the built-in
portfolio is broader, contextual classification is stronger, ambiguous values
have a separate review channel, and findings/candidates support typed
explainability and stricter normalization contracts.

The final optimization pass removed the pathological repeated full-source
contextual scans observed during development. Clean match-density performance
returned to approximately the historical v0.1 level, while the complete
built-in input-size workload retains the cost of the expanded v0.2 semantic
portfolio.

Further low-level optimization is deferred until real-world profiling identifies
a concrete bottleneck. Detection quality, semantic correctness and
maintainability remain higher priority than synthetic benchmark parity.
