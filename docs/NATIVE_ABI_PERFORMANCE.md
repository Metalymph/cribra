# Native ABI Performance

Cribra v0.3 adds a native C ABI adapter above the existing Rust core.

The goal of the adapter is not zero overhead at every call site. The goal is to
avoid source-sized copies, duplicate scans, unnecessary serialization, and
overhead that grows materially with realistic workloads.

## Measurement scope

Reference measurements below were collected on 2026-08-22 from an optimized
Criterion benchmark build.

They compare the Rust-native API with the equivalent `cribra-capi` path on the
same machine and in the same benchmark process.

These figures are development reference measurements, not cross-machine
performance guarantees.

## Results

| Workload | Rust native | C ABI | Observed adapter cost |
| --- | ---: | ---: | ---: |
| Minimal indirect call | 758.30 ps | 767.02 ps | ~1.1%; effectively noise-floor |
| Clean single scan, 64 B | 264.95 ns | 288.01 ns | +8.7% |
| Clean single scan, 64 KiB | 211.49 µs | 213.47 µs | +0.94% |
| Clean single scan, 1 MiB | 3.4070 ms | 3.4526 ms | +1.34% |
| Traverse 256 findings | 80.934 ns | 804.11 ns | ~2.82 ns extra per finding |
| Redact dense report | 2.5497 µs | 2.7773 µs | +8.9% |
| Serial batch, 32 × 64 KiB | 6.9406 ms | 6.9343 ms | indistinguishable / measurement noise |

## Interpretation

The pure `extern "C"` calling convention does not introduce a meaningful
measurable cost by itself.

For very small operations, fixed adapter costs are visible because the native
baseline is extremely short. These costs include argument validation, projection
into ABI view/handle types, owned-handle allocation where required, and explicit
destruction.

For realistic scan sizes, the adapter overhead falls to roughly one percent.
The benchmark does not show source-sized copying or a second scan at the ABI
boundary.

Report traversal has a large relative ratio because the Rust-native baseline is
almost free. In absolute terms, the measured additional cost is approximately
2.82 ns per finding for bounds checking, ABI projection, and the exported call.

Transform overhead is visible on the microsecond-scale redaction benchmark but
remains small in absolute terms.

Batch scanning fully amortizes the adapter cost at the tested 32 × 64 KiB
workload. The native and ABI paths are indistinguishable within normal benchmark
variation.

## Performance policy

The v0.3 native adapter follows these rules:

- do not copy full source buffers merely to cross the ABI boundary;
- do not serialize Rust result objects to implement the C ABI;
- do not run detection or transformation twice for adapter convenience;
- prefer borrowed length-delimited views for metadata;
- use explicit owned handles only where lifetime independence is required;
- accept small constant overhead for validation, ownership clarity, and panic
  containment;
- do not trade correctness, privacy boundaries, or ABI clarity for
  micro-optimizations;
- investigate future regressions when overhead grows with input size or becomes
  material on realistic workloads.

The current v0.3 measurements do not justify additional low-level ABI
optimization.

## Reproducing

Run:

```text
cargo bench -p cribra-capi --bench abi
```

The benchmark covers:

- minimal indirect Rust versus `extern "C"` call overhead;
- Rust-native versus C ABI single-source scanning;
- report traversal;
- redaction;
- serial batch amortization.

Criterion measurements should be interpreted comparatively within the same run.
Sub-percent differences and occasional inversions on multi-millisecond
workloads are treated as measurement noise unless reproduced consistently.
