# Local Python consumers and Fuse wheel (NB-2.6A.2)

A minimal consumer of the existing `cribra-uniffi` crate and UniFFI **0.32.2**
generator. No PyO3, second Rust adapter, hand-written Python facade, publishing
or CI integration is introduced. Consumer qualification and local wheel building
are separate commands; the wheel contains the Fuse profile only.

From the repository root, with Rust **1.97.0**, Python **3.13+** (the source
smoke runner and wheel builder; the built wheel itself supports Python 3.11+)
and `just`:

```sh
just uniffi-python-smoke
just uniffi-python-flow-smoke
just uniffi-python-fuse-smoke
# Select the interpreter explicitly if needed:
just uniffi-python-smoke /path/to/python3.13
just uniffi-python-flow-smoke /path/to/python3.13
just uniffi-python-fuse-smoke /path/to/python3.13
# Equivalent direct invocations (core is the default):
python3.13 bindings/python/smoke.py
python3.13 bindings/python/smoke.py core
python3.13 bindings/python/smoke.py flow
python3.13 bindings/python/smoke.py fuse
```

The runner accepts only `core|flow|fuse`. It builds the existing cdylib with
`--no-default-features --features <profile>`, generates `cribra_uniffi.py`, and
places the matching native library beside it for UniFFI's normal loader. All
profiles regenerate the direct Rust Core `scanner_oracle` and run the 25 Core
consumer tests. Flow and Fuse additionally regenerate `flow_oracle` and
`share_oracle` and run 15 Flow/Share tests (40 tests total). Fuse also regenerates
`parallel_oracle` and runs three parallel-scanning tests (43 tests total).
All are standard-library
`unittest` tests, using the same Python interpreter as the runner. There are no
pip dependencies. Missing tools, build failures or failing tests return a nonzero
exit status.

All generated bindings, native libraries, Cargo outputs and the oracle JSON stay
under ignored `target/uniffi-python-consumer/<profile>/`, with separate `cargo/`
and `generated/` directories and oracle JSON files for each profile. Core, Flow
and Fuse never load one another's native library or generated module; each test
suite runs in a fresh Python process. Existing Swift and other Core/Flow/Fuse
build outputs are preserved. No generated code or native binary belongs in this
source directory. Python bytecode writes are disabled for the consumer run.

The **25 standard-library tests** reuse `scanner.json`, `configurations.json`
and `incremental.json` in `crates/cribra-uniffi/tests/fixtures/`. Expected reports,
summaries, rule metadata, error fields and incremental traces come from the
existing `scanner_oracle`, regenerated against Rust Core on every run. Python
only constructs DTOs, projects results and applies accepted byte edits to inputs
for full-scan comparisons; it does not detect secrets or validate edits/rules.

Core coverage (also exercised in the Flow and Fuse builds):

- Single and batch scans: order, duplicate/Unicode/empty source IDs, empty batches,
  empty/clean sources, all summary counters, single/batch parity and repeated calls.
  Candidate-only reports retain empty findings, their candidate count and review
  status; candidates are never counted as findings.
- Configuration: built-in-only, custom-only, combined and empty custom rules;
  Literal, Prefix, Suffix and Pattern; active metadata, deterministic order,
  severity and remediation. Custom-only adds no implicit built-ins.
- Configuration failures: duplicate IDs and built-in collisions, empty IDs and
  matchers, malformed/unsupported/oversized regexes, empty/nullable/zero-width
  patterns and error precedence. Codes, optional rule IDs and diagnostic details
  are compared to Core. A whitespace-only ID is accepted as Core specifies.
- Streaming: Python `bytes`, every two-chunk byte split of the Unicode fixture,
  one-byte and larger chunks, CRLF/emoji/combining marks, empty chunks/sources,
  findings/candidates, Active/Finished/Failed, repeated finish and push after
  completion/failure. Invalid UTF-8 and incomplete EOF sequences fail terminally
  without returning a partial report. Storage admission (including the exact
  reported requirement), candidate/result limits during push and EOF failures
  match Core. Built-ins and an unbounded regex are rejected without fallback;
  the bounded custom fixture and empty ruleset are supported.
- Incremental: initialization across rulesets, insert/delete/replace/no-op edits,
  byte ranges, revision increments, snapshots and legacy revision/report accessors.
  Stale revisions, reversed/out-of-range spans and UTF-8 scalar splits are
  recoverable and preserve the snapshot. Candidate/cache/result/transaction
  storage limits are tested at construction and during edits; terminal failure
  preserves the last valid snapshot and rejects subsequent edits. Every accepted
  fixture edit matches both the direct Core trace and a full scan.
- Ownership: repeated use, sessions surviving Scanner garbage collection,
  reports/snapshots surviving Scanner/session destruction, and detached DTO
  mutation leaving native state and other reports unchanged.
- Core-only bindings exclude every exported Flow/Share function, DTO and error
  type, plus Fuse parallel scanning.

Stable error codes covered are `DuplicateRuleId`, `EmptyRuleId`, `EmptyMatcher`,
`InvalidPattern`, `PatternMatchesEmpty`, `UnsupportedRuleSet`, `InvalidUtf8`,
`IncompleteUtf8`, `NotActive`, `CandidateLimitExceeded`, `ResultLimitExceeded`,
`StorageLimitExceeded`, `StaleRevision`, `InvalidRange`, `InvalidUtf8Boundary`,
`CacheLimitExceeded` and `TransactionStorageLimitExceeded`. Optional revision,
limit/required and terminal fields are checked where applicable. Human-readable
diagnostics are compared with the current oracle, not promised stable across
Core versions.

Flow/Share coverage reuses `flow.json`, `keyed-flow.json`, `keyed-options.json`
and `share.json`, plus the same public synthetic key vectors as the Rust/Swift
tests. Python constructs DTOs and compares semantic oracle results; it implements
no transformation, hashing or synthesis algorithm.

- Redact and Template: 23 fixtures, 46 oracle outcomes covering default/custom
  replacement, deletion, namespace normalization, numbering, unsorted/adjacent/
  duplicate/overlapping spans, empty input and Unicode/CRLF/emoji/combining marks.
  Redact merges overlaps; Template rejects them, exactly as Core does.
- Pseudonymize and Synthesize: 27 inputs × 9 option sets × 2 transforms = 486
  oracle outcomes, each called twice to check deterministic output. This includes
  different explicit keys, prefixes, synthesis markers, provider synthesis and
  Core's content semantics. Digest sizes 0, 1, 8, 16, 32, 33 and `u64::MAX` follow
  Core's clamping to 8..32 on the qualified 64-bit target; 33 is not an error.
- Structured `TransformError` codes and fields: `InvalidSpan`,
  `InvalidUtf8Boundary`, `OverlappingSpans`, plus `InvalidKeyLength` for lengths
  0, 1, 31, 33, 64 and 1024. Invalid keys take precedence over invalid spans.
  Outputs/errors do not echo the public key vectors. All transforms leave the
  source untouched when no findings are supplied; no implicit scan is requested.
- Share `scan_for_share`, `scan_and_build` and `build`: all four modes, five
  batches, both build paths (40 oracle outcomes), and 23 explicit-finding inputs
  across all modes (92 oracle outcomes). Tests compare source order, Unicode and
  duplicate IDs, transformed bytes, manifest mode and every summary counter.
  Source-free Share reports preserve finding and candidate metadata; candidate-only
  reports remain distinct and candidate text is not transformed as a finding.
- Each successful bundle timestamp is checked for valid seconds/nanoseconds and
  the current call's time window. Independent timestamps are never compared for
  equality, including when checking atomic/precomputed paths against the oracle.
- Structured `ShareError`: `MissingShareMode`, `SourceCountMismatch`,
  `SourceLengthMismatch` (UTF-8 byte counts and batch index), and nested transform
  failures, including invalid keys in both build paths. Missing-mode/count/length
  and invalid-key cases use the existing Swift reference expectations because
  the Core oracles do not emit those cases. Core checks byte lengths, not source
  identity: equal-length changed content is accepted. Explicit reports are used
  without rescanning.
- Feature gating: Core excludes Flow/Share APIs and `Scanner.scan_for_share`;
  Flow and Fuse expose them and retain all Core APIs. `parallel_scan` is present
  exclusively in Fuse.

Fuse parallel coverage reuses `parallel.json`, `parallel_oracle`, the existing
Core report projection and configuration helpers. It exercises the same 44
scenarios as the Rust/Swift consumers: built-in-only, custom-only, combined and
empty rulesets, each across an empty batch, seven single sources, the original
batch, reversed order and a 64-source repeated batch.

- Every scenario compares complete parallel reports and all summary counters
  with both the direct Core oracle and sequential `scan_many`, repeated eight
  times on the same Scanner (352 parallel/sequential comparisons).
- Checks preserve source order, duplicate/empty/Unicode IDs (including distinct
  composed/decomposed UTF-8 encodings), source byte lengths, finding identity,
  severity/confidence, UTF-8 coordinates, candidate counts and review status.
  The mixed fixture exercises every severity counter and candidate-only reports
  without promoting candidates to findings.
- Owned parallel results remain usable after Scanner garbage collection.
  Python calls the native synchronous API directly; the tests introduce no
  Python scheduling, thread pool or parallelism.

The local wheel uses the standard [PyPA build frontend](https://build.pypa.io/en/stable/)
and `setuptools.build_meta` backend. Build-only dependencies are pinned in
`build-requirements.txt`: build 1.6.1, setuptools 84.0.0, packaging 26.3 and
pyproject_hooks 1.3.3. The wheel has no Python runtime dependencies. Rust 1.97.0,
Cargo and the platform C toolchain (the Apple SDK on macOS, `cc`/linker on
Linux) are needed only to build it.

Build from the repository root:

```sh
python3.13 -m venv target/uniffi-python-wheel/build-venv
target/uniffi-python-wheel/build-venv/bin/python -m pip install --index-url https://pypi.org/simple -r bindings/python/build-requirements.txt
just uniffi-python-wheel target/uniffi-python-wheel/build-venv/bin/python
# The builder writes into a per-platform directory (macOS example):
#   target/uniffi-python-wheel/fuse/macosx_13_0_arm64/dist/cribra-0.7.1+fuse-py3-none-macosx_13_0_arm64.whl
# On the qualified macOS 27 ARM64 host, qualify that same wheel across the
# pyenv interpreter matrix (or use uniffi-python-wheel-smoke for one version):
just uniffi-python-wheel-matrix target/uniffi-python-wheel/fuse/macosx_13_0_arm64/dist/cribra-0.7.1+fuse-py3-none-macosx_13_0_arm64.whl
just uniffi-python-wheel-smoke target/uniffi-python-wheel/fuse/macosx_13_0_arm64/dist/cribra-0.7.1+fuse-py3-none-macosx_13_0_arm64.whl 3.13.7
```

`build_wheel.py` selects the native target from the host: **macOS ARM64**
(`aarch64-apple-darwin`, `libcribra_uniffi.dylib`) or **Linux x86_64**
(`x86_64-unknown-linux-gnu`, `libcribra_uniffi.so`). It builds `cribra-uniffi`
with `--release --no-default-features --features fuse --target <triple>`, then
runs the existing UniFFI 0.32.2 generator against that release library. On macOS
it compiles against an explicit, host-independent deployment target: **macOS
13.0** by default, overridable with `CRIBRA_MACOS_DEPLOYMENT_TARGET` as
`MAJOR.MINOR`. Cargo does not fingerprint the deployment-target variable, so the
builder drops the cached library before linking to force the link step to run
with the configured target. It then verifies the compiled artifact and fails
closed on mismatch: on macOS it reads the `LC_BUILD_VERSION` load command and
requires exactly one `platform MACOS` command whose `minos` equals the
configured target; on Linux it requires a 64-bit little-endian x86_64 `ET_DYN`
ELF object. The wheel platform tag is derived from the verified artifact
(`py3-none-macosx_<minos>_arm64`, for example `macosx_13_0_arm64`, on macOS;
`py3-none-linux_x86_64` on Linux), never from the host. Each host/target writes
into its own platform directory under ignored
`target/uniffi-python-wheel/fuse/<platform>/` (`cargo/`, `generated/`, `dist/`,
`build-info.json`), so macOS and Linux artifacts never overwrite each other; the
build environment is also under `target/`. The ordinary smoke profile artifacts
are preserved. `build-info.json` records artifact paths, byte sizes, the host,
target triple, platform tag and, on macOS, the configured deployment target and
the verified `minos`.

Distribution metadata is `cribra`, version `0.7.1+fuse`, checked against the Core
version during builds. Import remains `import cribra_uniffi`. The generated
`cribra_uniffi.py` is copied byte-for-byte to `cribra_uniffi/__init__.py`, with
the platform library (`libcribra_uniffi.dylib` on macOS, `libcribra_uniffi.so`
on Linux) in the same package directory. No generated code is patched or
wrapped: UniFFI's existing `__file__`-relative loader remains intact, and it
already selects the `.so` filename on ELF platforms. The small setuptools hook
marks the wheel non-pure and platform-specific with no CPython extension ABI
(`py3-none-<platform>`). This is not a universal wheel or a promise of
compatibility beyond the tested host/interpreter. Only this build driver is
supported; direct source/editable installs and sdists are outside scope.

**Linux (x86_64) build prerequisites.** Linux builds must run natively on
x86_64 Linux; the builder refuses other hosts. It needs Rust 1.97.0 with the
`x86_64-unknown-linux-gnu` target (the default host target), a C linker/`cc` for
that triple, Python 3.13+ in the build environment and the pinned build frontend
dependencies. Prepare the build environment exactly as on macOS, then run
`just uniffi-python-wheel target/uniffi-python-wheel/build-venv/bin/python`; the
result is `cribra-0.7.1+fuse-py3-none-linux_x86_64.whl` under
`target/uniffi-python-wheel/fuse/linux_x86_64/dist/`.

**Linux current limits.** The Linux wheel is deliberately tagged
`linux_x86_64` (the local, non-portable tag), **not** `manylinux_*` or
`musllinux_*`: the glibc baseline, the `auditwheel` reconciliation and the
portability guarantees required by those contracts have not been verified, so no
such tag is claimed and the builder assigns none. Only the ELF class,
endianness, machine and shared-object type are checked, plus the `.so`'s
`DT_NEEDED` set: it must stay within the expected glibc libraries and must not
contain a repository-local `NEEDED` entry, `RPATH`/`RUNPATH` or `ldd` resolution.
The manually triggered **UniFFI Python Linux wheel** workflow
(`.github/workflows/uniffi-python-linux-wheel.yml`) performs this build, the ELF
verification and the installed-consumer smoke on `ubuntu-24.04` with Rust 1.97.0
and Python 3.13, then uploads the wheel and `build-info.json` as artifacts; it
never publishes. Nothing here is exercised by the macOS qualification above.
Windows, ARM64 Linux and musl-based distributions are out of scope for this
foundation.

`wheel_smoke.py` (the installed-wheel smoke) checks METADATA,
platform/non-pure tags, every RECORD hash/size, the included MIT license and byte
identity of both generated and native payloads once, verifies the native library
for the host platform, then qualifies that same wheel against each interpreter in
turn. On macOS it asserts the ARM64 architecture, Mach-O deployment information
and system-only native dependencies (`lipo`, `otool`, `vtool`); on Linux it
asserts the `.so` is an ELF64 `ET_DYN` x86_64 object whose `DT_NEEDED` set stays
within the expected glibc libraries and resolves only to system paths, with no
repository-local `NEEDED`, `RPATH`/`RUNPATH` or `ldd` resolution. It then copies
the wheel and standalone tests to a fresh temporary directory, creates a separate
isolated virtual environment per interpreter with `python -m venv`, installs
offline with `--no-index --no-deps` and runs the seven installed-consumer tests
with no Cargo/Rust on `PATH`. On macOS those tests run under OS sandbox rules
denying repository reads and network access plus a Python audit hook rejecting
repository opens and dylib loads; Linux has no `sandbox-exec` equivalent, so the
consumer's Python audit hook is the isolation boundary there. The interpreter
matrix defaults to the qualified pyenv versions on macOS and to the running
interpreter on Linux (a CI runner provides one). The tests exercise Scanner,
UTF-8 coordinates, `scan_many`, all four Flow transforms, all Share modes through
both build paths, and repeated Fuse `parallel_scan`. No independent manifest
timestamps are compared for equality.

The default matrix is Python **3.11.17, 3.12.15, 3.13.7 and 3.14.8** from
`pyenv`, and each version is reported separately. Distribution `requires-python`
is **`>=3.11`**, so every interpreter installs the wheel through the same normal
offline `pip` path (`--no-index --no-deps`): there are no compatibility probes
and no metadata bypass, and any refusal is a failure. `wheel_smoke.py` also
asserts the packaged `Requires-Python` metadata. Reuse is exact: the same
prebuilt wheel is tested for every interpreter and nothing is rebuilt. Pass
`--python VERSION` (repeatable, a pyenv version name or interpreter path) to
select a subset.

For a separate application, installation needs only the wheel and a Python
3.11+ interpreter:

```sh
python3.13 -m venv /private/tmp/cribra-app
/private/tmp/cribra-app/bin/python -m pip install --no-index --no-deps /absolute/path/to/cribra-0.7.1+fuse-py3-none-macosx_13_0_arm64.whl
cd /private/tmp
/private/tmp/cribra-app/bin/python -I -c 'import cribra_uniffi; print(cribra_uniffi.cribra_version())'
```

No Cargo, Rust, build frontend/backend, source checkout, pip index or network
access is needed at installed runtime. Nothing is published or uploaded.

The wheel is built locally with **Python 3.13.7** and the consumer matrix is
qualified locally with **Python 3.11.17, 3.12.15, 3.13.7 and 3.14.8** from
`pyenv` on **macOS 27.0.1 ARM64** (28 installed-consumer test runs, 7 per
interpreter). The final Python compatibility contract is `requires-python
>=3.11` for the wheel, with Python 3.11–3.14 all installed through the ordinary
offline `pip` path. The configured deployment target defaults to **macOS 13.0**,
and the compiled dylib's `minos` is verified to match it, but that only records
the binary's declared minimum OS: **runtime compatibility with macOS 13 is not
verified** — the wheel has only been exercised on the macOS 27.0.1 host above.
The consumer smoke runner recognizes native library names on macOS, Linux and
Windows; other platforms and interpreters are not qualified by this step. The
local wheel builder supports macOS ARM64 and Linux x86_64; Windows, Intel macOS,
ARM64 Linux, musl-based Linux, older macOS versions, Python outside 3.11–3.14,
signing/notarization, repair tools and distribution/release qualification are
outside scope. The installed consumer is a focused seven-test smoke; the full
Core/Flow/Fuse oracle suites remain the separate smoke commands above.
Parallel tests qualify observable results and repeatability, not speedup, worker
counts, scheduling or concurrent Python callers.
Representation/unknown-future-Core transform errors are
not deliberately forced, and 32-bit digest conversion is not qualified.
Streaming supports only rulesets admitted by Core; built-in scanning remains
available through single/batch/incremental APIs.
Deliberate panic/mutex poisoning, revision overflow, allocator exhaustion, huge
rule-count/automaton failures and concurrent Python calls are not qualified here.
Incremental sessions have Active/Failed states, with no finish operation.
Garbage-collection checks establish usable owned results and session lifetimes,
not a native memory-leak or performance guarantee. Rust and Swift APIs/features
are unchanged; their separate regression suites are not run by this command.
