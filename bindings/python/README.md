# Local Python consumer smoke (NB-2.1C)

A minimal consumer of the existing `cribra-uniffi` crate and UniFFI **0.32.2**
generator. No PyO3, second Rust adapter, hand-written Python API, package install,
wheel, publishing or CI integration is introduced.

From the repository root, with Rust **1.97.0**, Python **3.13+** and `just`:

```sh
just uniffi-python-smoke
# Select the interpreter explicitly if needed:
just uniffi-python-smoke /path/to/python3.13
# Equivalent direct invocation:
python3.13 bindings/python/smoke.py
```

The script builds the **Core-only** cdylib, generates `cribra_uniffi.py`, places
the matching native library beside it for UniFFI's normal loader, runs the
existing direct Rust Core `scanner_oracle`, and executes standard-library
`unittest` consumer tests. It uses the same Python interpreter for the tests.
There are no pip dependencies. Missing tools, build failures or failing tests
return a nonzero exit status.

All generated bindings, native libraries, Cargo outputs and the oracle JSON stay
under ignored `target/uniffi-python-consumer/`. This separate Cargo directory
preserves existing Swift and Core/Flow/Fuse build outputs. No generated code or
native binary belongs in this source directory. Python bytecode writes are
disabled for the consumer run.

Tests reuse `crates/cribra-uniffi/tests/fixtures/scanner.json` and compare every
exposed report field against the existing Core oracle: Unicode source IDs,
source byte lengths, ordered findings, RuleId, severity, confidence, UTF-8 byte
spans, one-based Unicode-scalar coordinates, candidate counts and review status.
They cover a positive synthetic PAT, CRLF/emoji/combining marks, empty/clean
input, repeated scans, reports surviving Scanner destruction, and absence of
Flow/Share/parallel APIs in this Core build. Python does not detect secrets or
reimplement Core validation.

Qualified locally with **Python 3.13.7 on macOS ARM64**. The script recognizes
native library names on macOS, Linux and Windows; other platforms/interpreters
are not qualified by this step. Python Flow/Fuse consumers and distribution
packaging remain outside scope. Rust and Swift APIs/features are unchanged.
