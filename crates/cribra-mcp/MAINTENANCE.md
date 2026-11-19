# cribra-mcp maintenance notes

Operational notes for keeping the `cribra-mcp` test suite trustworthy. This
document records why the transport qualification harness uses the bounds it
does, what was measured, and where the evidence stops. It is a maintenance
record, not a conformance or distribution claim: the M3 verdict remains
**PARTIAL PASS** ([`CONFORMANCE.md`](CONFORMANCE.md)) and the M4 distribution
verdict remains **PASS** ([`DISTRIBUTION.md`](DISTRIBUTION.md)).

Everything described here is test-only. No production behavior, exit code,
wire format, or public API is affected.

## 1. Transport qualification under CPU contention

Three transport tests failed intermittently on the native x86_64 macOS CI
runner (`macos-15-intel`, not Rosetta):

| Test | Historical symptom |
| --- | --- |
| `cancellation::cancellation_of_a_started_or_queued_call_suppresses_the_response` | no stdout line within `DEADLINE` |
| `adversarial::source_size_boundary_is_exact_at_one_mebibyte` | no stdout line within `DEADLINE` |
| `transport_races::closed_stdout_mid_response_terminates_cleanly` | exit 3 instead of 0 |

A fourth family, `transport_races::eof_discards_queued_real_tool_work_and_exits_zero`
and `cancellation_after_real_scan_is_well_formed_or_silent`, failed when
`server.wait(60s)` returned `None` during heavy intra-binary parallelism.

Root cause: test deadlines were calibrated for an idle machine, not for a debug
build under full-core parallel test execution on a slow shared runner. The
server was alive and computing in every reproduction (`child still running`,
empty server stderr). The `closed_stdout` failure is a race between the
harness's scheduling and the **production** 5 s no-progress deadline: if the
test thread is descheduled past 5 s before it closes the read end, the server
correctly reports terminal backpressure first.

Direct experiment against the unmodified server, no harness involved:

| Production deadline | Harness closes stdout after | Observed exit |
| --- | --- | --- |
| 5 s (default) | 6.5 s | 3, `output backpressure` diagnostic |
| 5 s (default) | never reads | 3, `output backpressure` diagnostic |
| 600 s | 6.5 s | 0, no stderr |

The server is correct in all three rows. Only the classification the test wants
to observe depends on the harness reaching the disconnect before the deadline.

## 2. Deadline policy

All transport waits are bounded hang detectors. No test relies on an unbounded
blocking read, and none may use a widened production deadline as its only
progress bound.

| Bound | Value | Applies to |
| --- | --- | --- |
| `cancellation::DEADLINE` | 120 s | round trip that runs the 1 MiB `(?s).` scan |
| `adversarial::DEADLINE` | 120 s | protocol round trips plus 1 MiB / broad-regex scans |
| `transport_races::CORE_WORK_TIMEOUT` | 60 s | process exit after queued Core work |
| `transport_races::STREAM_TIMEOUT` | 120 s | a single bounded stdout read |
| `CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS` override | 600000 ms | `closed_stdout_mid_response_terminates_cleanly`, `large_response_survives_partial_pipe_writes` |

Measured need under controlled contention (64 CPU-saturating processes on an
11-core arm64 host, debug build, worst observed):

| Test | Local idle | Needed under contention |
| --- | --- | --- |
| `closed_stdout_mid_response_terminates_cleanly` | 1.4 s | 15.0 s |
| `cancellation_of_a_started_or_queued_call_suppresses_the_response` | 4.3 s | 46.4 s |
| `source_size_boundary_is_exact_at_one_mebibyte` | 2.3 s | 23.5 s |

The 120 s deadline is roughly 2.5-3x the worst measured need and still fails a
genuinely wedged server. CI has been observed at 4-6x local wall time for these
binaries, so a 30 s or 20 s bound reports scheduler delay as a transport
failure.

### Why the 600 s override is safe

Widening the production deadline removes the incidental progress bound that the
5 s default provided. Both tests therefore read stdout through helpers in
`tests/support/mod.rs`:

- `StdoutChunks::spawn` + `next(timeout)` delivers 1 KiB chunks from a helper
  thread with a bounded `recv_timeout`, preserving the framing assertion.
- `read_prefix_and_close` reads a bounded prefix on a helper thread, closes the
  read end, then reports, so the broken-pipe wait starts immediately.

The regression test
`transport_races::bounded_stdout_reads_fail_within_their_own_deadline` keeps
both write ends open and silent, and asserts that both helpers panic on their
own timeout (~50 ms probe). A hung server therefore fails the suite in bounded
time even with the production deadline set to 10 minutes.

Sustained backpressure keeps its own short-deadline coverage:
`sustained_stdout_backpressure_is_terminal_with_exit_3` uses a 50 ms override,
and the transport unit tests are unchanged.

## 3. Harness diagnostics

`ServerProcess::next_line` used to collapse timeout, EOF, reader failure, and
channel disconnection into `None`. It still returns `Option` with identical
semantics, but the reason is retained and reported by `next_json`:

- `no stdout line within <duration> (wait timed out)`
- `stdout reached EOF before any line arrived`
- `stdout reader failed: <error>`
- `stdout reader thread ended without reporting EOF`

Each message appends the child status via `try_wait` (`exited with ...` /
`still running after ...`) and a snapshot of server stderr. `try_wait` caches
the status in `std::process`, so later `wait`/`wait_timeout` calls are
unaffected.

Privacy: messages contain only fixed diagnostics, elapsed time, and process
status. They never include request bytes, source text, matched values, or tool
arguments. Server stderr is only echoed because the server's own diagnostics are
fixed strings. Write failures in `send_raw`/`send_bytes` report the I/O error
plus the same child context.

Process cleanup: tests that use `ServerProcess` retain the existing `Drop`
behavior. The raw-process tests (`closed_stdout_*`, `large_response_*`,
`sustained_stdout_backpressure_*`) still rely on the child exiting on its own
and are bounded by `CORE_WORK_TIMEOUT`/`STREAM_TIMEOUT`; a hang surfaces as a
test failure instead of an indefinite block.

## 4. Stress qualification

Debug build, 2026-11-19, base commit `66677b3`:

| Run | Repetitions | Result |
| --- | --- | --- |
| Three intermittent tests, isolated | 20 each | 0 failures |
| Three intermittent tests, isolated, 64 CPU hogs | 3 each | 0 failures |
| Full MCP suite | 3 | 0 failures |
| Full MCP suite, 24 CPU hogs | 2 | 0 failures |
| Full MCP suite, release | 2 | 0 failures |
| Rust 1.97.0 MSRV, full MCP | 1 | 0 failures |

CI remains the authority for the x86_64 runner. The local results establish that
the new bounds absorb the same class of starvation, not that the CI runner is
now fast.

## 5. Platform limitations

- Native execution here was `aarch64-apple-darwin` only.
- `cargo check --target x86_64-apple-darwin` passes, but this host cannot
  execute x86_64 binaries and Rosetta is unavailable (`Bad CPU type in
  executable`, no `oahd`). The historical failures were on native
  `macos-15-intel`; the x86_64 path is qualified by CI, not locally.
- Linux and Windows targets cannot be compiled here: `blake3`'s build script
  needs toolchains this host does not have. Their qualification comes from the
  CI matrix.

## 6. Open diagnostic watch item

One Windows CI run (`43912161896`) produced the correct terminal exit code 3 for
`sustained_stdout_backpressure_is_terminal_with_exit_3` but the harness captured
empty stderr. Exit semantics were correct; only the pre-exit diagnostic was not
observed. This has not been reproduced (no Windows host available) and is not a
confirmed root cause. Treat it as a watch item: if it recurs, capture whether
the diagnostic write returned an error or whether the process exited before the
pipe delivered the bytes. Do not relax the assertion or claim a cause without
that evidence.

## 7. When to revisit

Re-measure the bounds if any of the following change:

- debug-build performance of the Core scan path used by these fixtures;
- runner class, test parallelism, or job timeout in `.github/workflows/mcp.yml`;
- the production backpressure default in `src/limits.rs`;
- the shape of the transport tests (new larger fixtures or additional streams).

Keep the rule that every transport wait has an explicit bound, and that a
widened production deadline always comes with a harness-side read timeout.
