# Final benchmark suite

Place `scan.rs` at `benches/scan.rs`.

The suite measures:

- scanner construction cost;
- full built-in pack across 1 KiB, 64 KiB and 1 MiB sources;
- none, sparse and dense finding density;
- custom literal, custom mixed and full validation pipeline comparison;
- custom rule-count scaling at 4, 64 and 512 rules.

Run:

```fish
cargo bench --bench scan
```

Run one group:

```fish
cargo bench --bench scan -- "scan/current/input-size"
cargo bench --bench scan -- "scan/current/match-density"
cargo bench --bench scan -- "scan/pipeline-comparison"
cargo bench --bench scan -- "scan/custom/rule-count"
cargo bench --bench scan -- "build"
```

The benchmark fixtures contain synthetic, unusable credential-like values.
