# Local Python consumers and Fuse wheel (NB-2.6A.2)

See [Getting started](../../docs/GETTING_STARTED.md) and
[Compatibility and distribution status](../../docs/COMPATIBILITY.md) for the
cross-language entry points and evidence boundaries.

A minimal consumer of the existing `cribra-uniffi` crate and UniFFI **0.32.2**
generator. No PyO3, second Rust adapter, hand-written Python facade or publishing
is introduced. Consumer qualification and local wheel building are separate
commands; the wheel contains the Fuse profile only.

Platform CI qualification is documented below.

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
(`aarch64-apple-darwin`, `libcribra_uniffi.dylib`), **Linux x86_64**
(`x86_64-unknown-linux-gnu`, `libcribra_uniffi.so`) or **Windows x86_64**
(`x86_64-pc-windows-msvc`, `cribra_uniffi.dll`). It builds `cribra-uniffi`
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
ELF object; on Windows it requires a 64-bit x86_64 PE dynamic library
(`IMAGE_FILE_MACHINE_AMD64`, `IMAGE_FILE_DLL`). The wheel platform tag is derived
from the verified artifact (`py3-none-macosx_<minos>_arm64`, for example
`macosx_13_0_arm64`, on macOS; `py3-none-linux_x86_64` on Linux;
`py3-none-win_amd64` on Windows), never from the host. Each host/target writes
into its own platform directory under ignored
`target/uniffi-python-wheel/fuse/<platform>/` (`cargo/`, `generated/`, `dist/`,
`build-info.json`), so macOS, Linux and Windows artifacts never overwrite each
other; the Windows directory is `windows_x86_64/` (the host name, deliberately
different from its `win_amd64` tag); the build environment is also under
`target/`. The ordinary smoke profile artifacts are preserved.
`build-info.json` records artifact paths, byte sizes, the host, target triple,
platform tag and, on macOS, the configured deployment target and the verified
`minos`. After packaging, the builder fails closed unless the wheel packages
exactly the one host native library (never a foreign-platform library), keeps
the ABI-independent `py3-none-<platform>` tag and stays non-pure
(`Root-Is-Purelib: false`).

Distribution metadata is `cribra`, version `0.7.1+fuse`, checked against the Core
version during builds. Import remains `import cribra_uniffi`. The generated
`cribra_uniffi.py` is copied byte-for-byte to `cribra_uniffi/__init__.py`, with
the platform library (`libcribra_uniffi.dylib` on macOS, `libcribra_uniffi.so`
on Linux, `cribra_uniffi.dll` on Windows) in the same package directory.
`[tool.setuptools.package-data]` lists all three native names, so exactly the one
the builder stages is packaged: the builder and `wheel_smoke.py` both fail closed
unless each wheel contains exactly that library. No generated code is patched or
wrapped: UniFFI's existing `__file__`-relative loader remains intact, and it
already selects the `.so` filename on ELF platforms and `cribra_uniffi.dll` next
to the module on Windows. The small setuptools hook
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

**Linux current limits.** The native Linux wheel is deliberately tagged
`linux_x86_64` (the local, non-portable tag), **not** `manylinux_*` or
`musllinux_*`: the native build performs no glibc/`auditwheel` reconciliation and
claims no portability contract. (A separate, opt-in `manylinux_2_28_x86_64`
foundation is described below.) Only the ELF class,
endianness, machine and shared-object type are checked, plus the `.so`'s
`DT_NEEDED` set: it must stay within the expected glibc libraries and must not
contain a repository-local `NEEDED` entry, `RPATH`/`RUNPATH` or `ldd` resolution.
The manually triggered **UniFFI Python Linux wheel** workflow
(`.github/workflows/uniffi-python-linux-wheel.yml`) performs this build, the ELF
verification and the installed-consumer smoke on `ubuntu-24.04` with Rust 1.97.0
and Python 3.13, then uploads the wheel and `build-info.json` as artifacts; it
never publishes. Nothing here is exercised by the macOS qualification above.
ARM64 Linux and musl-based distributions are out of scope for this foundation.
(The separate Windows foundation is described next.)

**Windows (x86_64) build procedure.** The Windows wheel is an isolated native
build path that reuses the same PEP 517 driver, the same UniFFI 0.32.2 bindings
and the same setuptools backend. It introduces no PyO3, no second Rust adapter
and no hand-written Python facade, and it must run natively on 64-bit Windows
with the MSVC toolchain; the builder refuses any other Windows architecture.
Prepare the build environment exactly as on macOS — Python 3.13+ and the pinned
build frontend — then run:

```powershell
python -m venv target/uniffi-python-wheel/build-venv
target/uniffi-python-wheel/build-venv/Scripts/python -m pip install --index-url https://pypi.org/simple -r bindings/python/build-requirements.txt
just uniffi-python-wheel target/uniffi-python-wheel/build-venv/Scripts/python
# Equivalent direct invocation:
target/uniffi-python-wheel/build-venv/Scripts/python bindings/python/build_wheel.py
```

The result is `cribra-0.7.1+fuse-py3-none-win_amd64.whl` under
`target/uniffi-python-wheel/fuse/windows_x86_64/dist/`. Requirements: Rust 1.97.0
with the `x86_64-pc-windows-msvc` target (the default host toolchain on 64-bit
Windows), a C toolchain for that triple, and Python 3.13+ in the build
environment. The builder compiles `cribra-uniffi`, runs the generator, verifies
that the compiled `cribra_uniffi.dll` is a 64-bit x86_64 PE dynamic library, and
stages only that one library next to the generated `__init__.py`. It then fails
closed unless the wheel contains exactly that one native library (never
`libcribra_uniffi.so`/`libcribra_uniffi.dylib`), keeps the `py3-none-win_amd64`
ABI-independent tag and stays non-pure (`Root-Is-Purelib: false`); the tag is
never derived from the host. `build-info.json` records the artifact paths, byte
sizes, host `windows`, triple `x86_64-pc-windows-msvc` and platform `win_amd64`.

**Windows (x86_64) installed-consumer qualification.** On a native 64-bit
Windows host the built wheel is qualified with the same installed-consumer smoke
used on macOS and Linux, against the final `py3-none-win_amd64` wheel:

```powershell
# Default: qualify the running interpreter (sys.executable).
target/uniffi-python-wheel/build-venv/Scripts/python bindings/python/wheel_smoke.py target/uniffi-python-wheel/fuse/windows_x86_64/dist/cribra-0.7.1+fuse-py3-none-win_amd64.whl
# Additional interpreters (repeatable, an explicit interpreter path):
target/uniffi-python-wheel/build-venv/Scripts/python bindings/python/wheel_smoke.py <wheel> --python C:\Python311\python.exe --python C:\Python314\python.exe
```

`wheel_smoke.py` accepts exactly one Windows tag, `py3-none-win_amd64`, on a
native Windows host and rejects every other platform, architecture and ABI
(`win_arm64`, `win32`, a `cp3xx-*`/`abi3` ABI, ...). It verifies the installed
`cribra_uniffi.dll` with the same PE32+ parser the builder uses
(`build_wheel.pe_amd64_dll_error`, so there is only one PE parser): the payload
must be a 64-bit x86_64 PE dynamic library (`IMAGE_FILE_MACHINE_AMD64`,
`IMAGE_FILE_DLL`). The wheel must package exactly that one native library and no
foreign library. Each interpreter gets its own fresh `venv`, whose interpreter is
`Scripts/python.exe`, installs the wheel offline (`--no-index --no-deps`, no
Rust/Cargo on `PATH`) and runs the seven installed-consumer tests against the
**installed** package, which loads the DLL and exercises Core/Flow/Fuse (Scanner,
UTF-8 coordinates, `scan_many`, all four Flow transforms, all Share modes through
both build paths, and repeated Fuse `parallel_scan`). The default interpreter is
the running one; pass `--python` for more. Nothing is rebuilt, downloaded or
published.

**Windows (x86_64) CI qualification.** The `python-windows-wheel` job in
`.github/workflows/ci.yml` runs this entire path on GitHub Actions
`windows-2022` (native x86_64, Rust 1.97.0 with the `x86_64-pc-windows-msvc`
target, MSVC build tools and Python 3.11–3.14): it builds the wheel with Python
3.13, re-verifies the PE32+ AMD64 DLL, the `py3-none-win_amd64` tag, the
non-pure metadata and the exactly-one-native-library rule, qualifies all four
interpreters (28/28 installed-consumer tests) and uploads the wheel and
`windows-consumer.log` as artifacts. Nothing is signed, published or released.

**manylinux_2_28_x86_64 foundation.** An isolated, opt-in path builds the same
Fuse wheel inside the official PyPA manylinux image pinned by digest
(`quay.io/pypa/manylinux_2_28_x86_64@sha256:1e7b8460ad8fb3fb0fbe1cd818845159ac896b4fd17452d56e892c291f3b97a8`,
AlmaLinux 8 / glibc 2.28) on `linux/amd64`, installing Rust 1.97.0 inside it:

```sh
just uniffi-python-manylinux-wheel
```

`build_manylinux_wheel.sh` runs that container (Docker required; emulated on a
non-x86_64 host) and executes `manylinux/entrypoint.sh`, which installs the
pinned build frontend and `auditwheel` and runs `build_wheel.py` with
`CRIBRA_MANYLINUX_PLATFORM=manylinux_2_28_x86_64`. That opt-in only redirects
the builder to its own `target/uniffi-python-wheel/fuse/manylinux_2_28_x86_64/`
directory and leaves the native macOS and `linux_x86_64` paths untouched. The
builder still writes a provisional `py3-none-linux_x86_64` wheel there, runs
`auditwheel show` (saved to `auditwheel-show.txt`) and then
`auditwheel repair --plat manylinux_2_28_x86_64`. The platform tag is applied
only by auditwheel: if `repair` refuses the requested policy the builder fails
closed and claims nothing, and it re-verifies that the qualified wheel packages
exactly the native library, stays non-pure and keeps the ABI-independent
`py3-none` tag. Nothing is published.

**manylinux installed-consumer qualification.** The repaired wheel is qualified
with the same installed-consumer smoke, against the final
`py3-none-manylinux_2_28_x86_64` wheel (never the provisional `linux_x86_64`
one):

```sh
just uniffi-python-manylinux-wheel          # build and auditwheel repair first
just uniffi-python-manylinux-wheel-matrix   # qualify the repaired wheel
```

`wheel_smoke.py` accepts exactly two Linux tags — the local
`py3-none-linux_x86_64` and the auditwheel-confirmed
`py3-none-manylinux_2_28_x86_64` — and still rejects every other platform,
architecture and ABI (any other manylinux/musllinux version, the
`manylinux2014`/`manylinux_2_17` aliases, `aarch64`, a `cp3xx-*` or `abi3` ABI,
...). `qualify_manylinux_wheel.sh` runs `manylinux/consumer_entrypoint.sh` in
the same digest-pinned image, which passes every CPython the image provides
(3.11, 3.12, 3.13, 3.14) to `wheel_smoke.py`; ELF and `DT_NEEDED`/`ldd`
verification is the existing Linux path, unchanged. Each interpreter gets its
own fresh `venv`, installs the wheel offline (`--no-index --no-deps`, no
Rust/Cargo on `PATH`) and runs the seven installed-consumer tests; the
consumer's Python audit hook is the isolation boundary. Nothing is rebuilt,
downloaded or published.

`wheel_smoke.py` (the installed-wheel smoke) checks METADATA,
platform/non-pure tags, every RECORD hash/size, the included MIT license and byte
identity of both generated and native payloads once, verifies the native library
for the host platform, then qualifies that same wheel against each interpreter in
turn. The tag check is strict: on macOS a `py3-none-macosx_*_arm64` tag, on Linux
`py3-none-linux_x86_64` or the auditwheel-confirmed
`py3-none-manylinux_2_28_x86_64`, and on Windows exactly `py3-none-win_amd64`;
every other platform, architecture and ABI fails closed. RECORD is compared
against the archive's files, because the
auditwheel-repaired wheel also carries explicit directory entries (RECORD lists
files only). On macOS it asserts the ARM64 architecture, Mach-O deployment information
and system-only native dependencies (`lipo`, `otool`, `vtool`); on Linux it
asserts the `.so` is an ELF64 `ET_DYN` x86_64 object whose `DT_NEEDED` set stays
within the expected glibc libraries and resolves only to system paths, with no
repository-local `NEEDED`, `RPATH`/`RUNPATH` or `ldd` resolution; on Windows it
asserts the `.dll` is a 64-bit x86_64 PE dynamic library using the builder's
single PE32+ parser. It then copies
the wheel and standalone tests to a fresh temporary directory, creates a separate
isolated virtual environment per interpreter with `python -m venv` (the venv
interpreter is `Scripts/python.exe` on Windows), installs
offline with `--no-index --no-deps` and runs the seven installed-consumer tests
with no Cargo/Rust on `PATH`. On macOS those tests run under OS sandbox rules
denying repository reads and network access plus a Python audit hook rejecting
repository opens and dylib loads; Linux and Windows have no `sandbox-exec`
equivalent, so the consumer's Python audit hook is the isolation boundary there.
The interpreter
matrix defaults to the qualified pyenv versions on macOS and to the running
interpreter on Linux and Windows (a CI runner or a Windows host provides one).
The tests exercise Scanner,
UTF-8 coordinates, `scan_many`, all four Flow transforms, all Share modes through
both build paths, and repeated Fuse `parallel_scan`. No independent manifest
timestamps are compared for equality.

**Windows isolation limits.** Windows has no OS sandbox (`sandbox-exec`)
equivalent, so the installed-consumer isolation there is the in-process Python
audit hook (`sys.addaudithook`) that rejects repository opens,
`os.listdir`/`os.scandir` and `ctypes.dlopen` of repository paths, exactly as on
Linux. That hook is a Python-level boundary, not an OS one: it is not enforced
against native code that loads a DLL directly, and network access is **not**
denied on Windows or Linux — only the macOS sandbox denies `network*`. The empty
`PATH` (no Cargo/Rust), the offline `--no-index --no-deps` install and the
`--isolated` pip run are the other controls. On Windows no import-table/`ldd`
dependency audit is performed: only the PE32+ header (machine/DLL) is verified,
so a repository-local DLL dependency would not be detected at the binary level.
These limits match the Linux qualification, minus macOS's OS sandbox.

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
The consumer smoke runner (`wheel_smoke.py`) recognizes native library names on
macOS, Linux and Windows, and the installed-consumer qualification covers macOS
ARM64, Linux x86_64/`manylinux_2_28_x86_64` and native Windows AMD64. Windows
qualification requires a native Windows host (there is no emulation path); the
`python-windows-wheel` CI job runs it on GitHub Actions `windows-2022`. The local
wheel builder supports macOS ARM64, Linux x86_64 and Windows x86_64; Intel macOS, ARM64 Linux, ARM64 Windows, 32-bit Windows,
musl-based Linux, older macOS versions, Python outside 3.11–3.14,
signing/notarization and distribution/release qualification are outside scope.
`auditwheel repair` is used only by the opt-in manylinux_2_28_x86_64 foundation
above; no other repair tool and no publishing is introduced. The installed consumer is a focused seven-test smoke; the full
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
