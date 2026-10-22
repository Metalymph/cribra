# Rust/WASM semantic parity gate

The Rust-native `cribra` core is the semantic oracle. The gate compares public
semantic facts rather than Rust/JavaScript representation details.

It verifies source byte count; finding count/order, rule IDs, spans, Unicode
coordinates, severity, confidence and remediation; candidate count/order, kind
and evidence; explanation facts; and exact output for equivalent supported
transformations.

Every canonical source under `examples/fixtures/inputs/` is reused where
practical. The canonical fixture scanner and its WASM equivalent use the same
custom remediation overrides through the additive
`add*WithRemediation` methods. Fixture cases therefore verify both remediation
metadata and the resulting transformations.

Dedicated cases additionally cover Unicode coordinates, review-only candidate
semantics, built-in remediation/explanation semantics, and one source that
exercises all four canonical custom remediation overrides together.

The JavaScript runner executes `target/wasm-production/`, so semantic parity is
checked against the selected Binaryen `-Oz` production artifact, not merely the
unoptimized adapter build.

Generated oracle data lives under `target/wasm-parity/` and is not committed.
