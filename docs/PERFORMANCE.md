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
