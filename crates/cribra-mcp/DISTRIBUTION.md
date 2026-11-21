# cribra-mcp distribution and host configuration

This document is the M4 distribution record for `cribra-mcp`. It describes how
the native stdio server is built, packaged, verified, installed, and configured
in real MCP hosts. It also states, explicitly, what was executed and what was
not.

The M3 conformance verdict is unchanged: **PARTIAL PASS**. See
[`CONFORMANCE.md`](CONFORMANCE.md). Distribution readiness does not upgrade,
replace, or re-certify that verdict.

The M4 distribution verdict is **PASS**. The `cribra-mcp-v0.1.1` release
published five native archives and one `SHA256SUMS` manifest; each archive was
packaged and smoke-qualified on its own native runner before publication.

Developer-experience qualification (DX.1–DX.5) is recorded separately in
[`DEVELOPER_EXPERIENCE.md`](DEVELOPER_EXPERIENCE.md). The overall DX verdict is
**PARTIAL PASS**: macOS ARM64 was re-executed from a clean-room extraction,
Codex CLI and OpenCode passed host-mediated interoperability, and Copilot is
documented feasibility plus design only. DX does not upgrade, replace, or
re-certify the M3 or M4 verdicts.

## 1. What is distributed

`cribra-mcp` is a single native executable. It reads newline-delimited JSON-RPC
2.0 on stdin and writes responses on stdout. It has:

- no HTTP listener and no authorization;
- no network access of any kind;
- no filesystem access beyond its own executable load;
- no configuration file and no environment-variable configuration surface
  other than the documented backpressure timeout override;
- no telemetry, auto-update, keyed operation, or persistent storage.

The executable is published from one opt-in release workflow as five archives
and one checksum manifest.

## 2. Target matrix

| Target triple | Runner label | Archive | Binary format |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `macos-latest` (arm64) | `.tar.gz` | Mach-O arm64 |
| `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` | Mach-O x86_64 |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `.tar.gz` | ELF x86_64 |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` | ELF aarch64 |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.zip` | PE x86_64 |

Every target is built natively: no cross-compilation and no emulation. Each
archive is built, checksummed, extracted into a clean temporary directory, and
qualified by running the extracted executable as a real MCP server on its own
runner.

`aarch64-pc-windows-msvc` and the musl targets are **not** produced. They are
intentionally out of scope, not silently broken.

## 3. Artifact naming

```text
cribra-mcp-0.1.1-aarch64-apple-darwin.tar.gz
cribra-mcp-0.1.1-x86_64-apple-darwin.tar.gz
cribra-mcp-0.1.1-x86_64-unknown-linux-gnu.tar.gz
cribra-mcp-0.1.1-aarch64-unknown-linux-gnu.tar.gz
cribra-mcp-0.1.1-x86_64-pc-windows-msvc.zip
SHA256SUMS
```

Artifacts carry the Cargo version without a `v`; the git tag keeps its
`cribra-mcp-v` prefix (`cribra-mcp-v0.1.1`). Each archive extracts to exactly
one directory:

```text
cribra-mcp-0.1.1-<target>/
  cribra-mcp           (cribra-mcp.exe on Windows)
  LICENSE
  INSTALL.md
```

Archives are deterministic. Repeated builds of identical inputs produce
identical bytes: fixed 1980-01-01 timestamps, `uid/gid 0`, empty owner names,
sorted entries, and a single documented allow-list of files. The packaging
script refuses to overwrite an existing archive.

Determinism here applies to **packaging**, not to compilation. Given the same
compiled executable, the archive bytes are reproducible. The executable itself
is not claimed to be bit-for-bit reproducible: the Rust toolchain version, the
linker, the OS SDK, absolute build paths, and platform code generation can all
change the machine code. `SHA256SUMS` is an integrity check: it lets you
confirm that a downloaded archive matches the bytes recorded in the manifest.
It is not a cryptographic signature, does not assert who built the artifact,
and is not a reproducible-build attestation.

## 4. Install

### Verify before extraction

The release publishes one `SHA256SUMS` manifest covering all five archives.
Verify only the entry for the archive you downloaded before extracting it;
checking the whole manifest fails when the other four archives are absent.

```sh
archive=cribra-mcp-0.1.1-<target>.tar.gz
if ! awk -v f="$archive" '$2 == f' SHA256SUMS > "$archive.sha256"; then
  echo "error: cannot read SHA256SUMS" >&2
  exit 1
fi
if [ ! -s "$archive.sha256" ]; then
  echo "error: SHA256SUMS has no entry for $archive" >&2
  exit 1
fi
shasum -a 256 -c "$archive.sha256"    # macOS
sha256sum -c "$archive.sha256"        # Linux
```

`shasum`/`sha256sum -c` exit non-zero and report `FAILED` on a checksum
mismatch. The explicit size check fails when the manifest has no entry for the
archive, rather than silently verifying nothing.

```powershell
$archive = 'cribra-mcp-0.1.1-x86_64-pc-windows-msvc.zip'
$entry = Get-Content SHA256SUMS | Where-Object { ($_ -split '\s+')[1] -eq $archive }
if (-not $entry) { Write-Error "SHA256SUMS has no entry for $archive"; exit 1 }
$expected = ($entry -split '\s+')[0].ToLower()
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLower()
if ($actual -ne $expected) { Write-Error "checksum mismatch for $archive"; exit 1 }
"$archive OK"
```

### macOS and Linux

```sh
tar -xzf cribra-mcp-0.1.1-<target>.tar.gz
mkdir -p "$HOME/cribra-mcp"
mv cribra-mcp-0.1.1-<target> "$HOME/cribra-mcp/"
"$HOME/cribra-mcp/cribra-mcp-0.1.1-<target>/cribra-mcp" --version
```

The archive is user-local: no `sudo`, no `/opt`, and no system-wide install is
required. Use the expanded absolute path in host configuration; GUI hosts may
not expand `~` or `$HOME`.

Both published Linux binaries reference GLIBC symbols up to `GLIBC_2.34`, as
measured with `readelf --version-info` on the published archives. A host must
provide glibc 2.34 or newer. This is a binary symbol requirement, not a
runtime-qualification statement: the x86_64 archive was runtime-qualified on
the GitHub-hosted Ubuntu 26.04 image (`ubuntu-latest`) and the aarch64 archive
on Ubuntu 24.04 ARM64 (`ubuntu-24.04-arm`). Older distributions that satisfy
the symbol floor are expected to load but were not runtime-qualified. No musl
or bundled libc build is published.

The macOS artifacts are **not notarized**. Their signatures differ:

- the `aarch64-apple-darwin` executable is **ad-hoc signed** (`codesign -dv`
  reports `Signature=adhoc`);
- the `x86_64-apple-darwin` executable is **unsigned** and has no ad-hoc
  signature.

Gatekeeper may therefore refuse to launch a browser-downloaded copy. There is
no Developer ID signature or notarization ticket, and the release provides no
quarantine-removal command or other Gatekeeper bypass. Verify the SHA-256 digest
and do not install the artifact if your policy requires a signed or notarized
binary.

### Windows

```powershell
Expand-Archive cribra-mcp-0.1.1-x86_64-pc-windows-msvc.zip -DestinationPath "$HOME\cribra-mcp"
& "$HOME\cribra-mcp\cribra-mcp-0.1.1-x86_64-pc-windows-msvc\cribra-mcp.exe" --version
```

The Windows executable is unsigned. SmartScreen may warn or block it; there is
no publisher signature and the release provides no SmartScreen bypass. Verify
the SHA-256 digest before running it and follow your organization's
application-control policy.

### CLI surface

```text
cribra-mcp            Serve MCP over stdin/stdout
cribra-mcp --version  Print the server version
cribra-mcp --help     Print help
```

Unknown or extra arguments fail with exit code `2` and write nothing to stdout.
While serving, stdout carries protocol messages only; the first byte is `{`.
Diagnostics go to stderr.

The host must keep stdin open for the session lifetime. stdin EOF ends the
session cleanly with exit code `0`. Responses already written to stdout remain
valid; work that had not started may be discarded, so a request written
immediately before EOF may return no response. Hosts must keep stdin open while
awaiting responses. An immediate EOF (`cribra-mcp < /dev/null` or the platform
equivalent) is a clean no-op shutdown. The server does not daemonize.

## 5. Host configuration

Use the **absolute path** to the extracted executable. Do not rely on `PATH`:
GUI-launched hosts frequently run without the shell `PATH` that installed it.

The configuration shape below is `stdio`: a command, an argument list, and an
optional environment map. `cribra-mcp` needs no arguments and no environment
variables.

### Claude Desktop

Config file:

| OS | Path |
| --- | --- |
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

```json
{
  "mcpServers": {
    "cribra": {
      "command": "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
      "args": []
    }
  }
}
```

Restart Claude Desktop completely after editing the file.

### Claude Code

Verified against Claude Code `0.2.38` with an isolated `CLAUDE_CONFIG_DIR`.
That version writes a **global** server to `config.json`:

```json
{
  "mcpServers": {
    "cribra": {
      "type": "stdio",
      "command": "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

and a **project**-scoped server into the project-path key of the same
`config.json`, not into a repository `.mcp.json`.

Command form for that version:

```sh
claude mcp add -s global cribra /Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
claude mcp add -s project cribra /Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
```

Current Claude Code releases accept an explicit transport and a `--` separator:

```sh
claude mcp add --transport stdio -s user cribra -- /Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
```

and project scope writes a committed `.mcp.json`:

```json
{
  "mcpServers": {
    "cribra": {
      "type": "stdio",
      "command": "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
      "args": []
    }
  }
}
```

Both forms were checked against real CLI help and real config output; only the
`0.2.38` form was executed end to end locally.

### Codex CLI

Verified against `codex-cli 0.169.0-alpha.11.1` during M4 and re-qualified with
`codex-cli 0.169.1` during DX.2, both with an isolated `CODEX_HOME`. Codex
stores MCP servers in `config.toml` (`$CODEX_HOME/config.toml`, default
`~/.codex/config.toml`):

```toml
[mcp_servers.cribra]
command = "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp"
```

Command form:

```sh
codex mcp add cribra -- /Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
codex mcp list
codex mcp get cribra
```

`codex mcp get` reported `transport: stdio`, `enabled: true`, and the absolute
command, exactly as written by `add`. Codex defaults to a 10 s startup timeout
and a 60 s tool timeout; `cribra-mcp` answers discovery, `tools/list`, and both
tool calls well inside those bounds.

DX.2 drove Codex's own `app-server`: discovery returned exactly two tools, real
`cribra_scan` and `cribra_transform` calls succeeded, repeated calls were stable,
and teardown left no orphan. The default negotiation was legacy `2025-11-25`.
Measured clean setup was **`0.966 s`** (config write, `codex mcp list`,
app-server discovery, and one scan). Model-driven invocation was **not
qualified** because the account was usage-limited.

### Codex Desktop

**PENDING — not qualified.** Codex Desktop `26.1111.71704` bundles the
`codex-cli 0.169.1` host layer and shares the same `config.toml` MCP
configuration, but the Desktop GUI was not driven. Qualifying it requires an
authorized change to the shared global configuration, a restart, GUI approval,
and a model turn. The CLI `app-server` PASS is evidence for the shared host
layer, not an executed Desktop result.

### OpenCode

Qualified against OpenCode `2.3.2` on macOS ARM64 with an isolated project
configuration. The server uses OpenCode's local stdio transport:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "cribra": {
        "type": "local",
        "command": [
          "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp"
        ]
      }
    }
  },
  "permissions": [
    { "action": "*", "resource": "*", "effect": "deny" },
    { "action": "cribra_cribra_scan", "resource": "*", "effect": "allow" },
    { "action": "cribra_cribra_transform", "resource": "*", "effect": "allow" }
  ]
}
```

OpenCode's MCP permission action is `<server>_<tool>`, so the two Cribra
actions are `cribra_cribra_scan` and `cribra_cribra_transform`. The example uses
deny-by-default and then allows only those two MCP actions.

Verification and discovery:

```sh
opencode mcp list
```

DX.3 observed `✓ cribra connected` and the host log
`mcp connected server=cribra tools=2`. OpenCode's default `legacy` negotiation
connected with `2025-11-25`; a separate isolated configuration pinned
`2026-07-28` and also connected. Cold setup from private `opencode serve`
start to connected was **`0.534 s`** on the default path and `0.612 s` with the
pinned modern revision.

The host-mediated matrix passed scan, repeat, candidates, clean source, redact,
custom replacement, template, namespaced/numbered template, and three
structured-error cases. One child served all 11 calls and was cleaned up.
Model-driven execution was **not qualified** because every available provider
turn was blocked before a model response.

For OpenCode's no-model Code Mode automation path, `codemode: true` and an
`execute` permission were additionally required; without them the automation
catalog could not see the MCP tools. That is a host automation detail, not a
requirement for ordinary agent use.

### GitHub Copilot cloud agent (design only)

**NOT QUALIFIED — DOCUMENTED FEASIBILITY AND DESIGN ONLY.** No Copilot session
was started. The account is Copilot Free with `cloud_sessions_enabled: false`,
and Copilot cloud agent requires a paid plan.

Documented constraints:

- repository MCP servers are entered in **Settings → Copilot → MCP servers**,
  not in a committed `.github/mcp.json`;
- the repository setting is shared with Copilot code review;
- native `local`/`stdio` servers use an absolute `command`, an `args` array,
  optional `env`, and an exact `tools` allowlist;
- Copilot supports MCP tools only, not resources or prompts;
- runners are Ubuntu x64 or Windows x64 only; macOS runners are unsupported;
- `.github/workflows/copilot-setup-steps.yml` applies only to **Actions**
  sessions; Cloud sandbox sessions do not run it;
- Copilot calls configured MCP tools autonomously without approval, so the
  `tools` allowlist is the control.

The proposed Actions setup workflow and repository MCP JSON are recorded as
examples only in
[`DEVELOPER_EXPERIENCE.md`](DEVELOPER_EXPERIENCE.md#dx4--github-copilot-cloud-agent).
No operational Copilot workflow is committed to this repository, and no
repository setting was changed. A future run must explicitly choose the
Actions environment, because a Cloud session would not have the installed
absolute path.

### Generic stdio hosts

Any host that supports an MCP `stdio` server accepts the same three fields:

```json
{
  "command": "/Users/you/cribra-mcp/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
  "args": [],
  "env": {}
}
```

### Legacy compatibility

The server accepts the legacy `2025-11-25` lifecycle. A host that sends
`initialize` with any proposed version receives a `2025-11-25` negotiation
result and continues with `notifications/initialized`, `tools/list`, and
`tools/call`. This is what older MCP clients use, and it is covered by the
packaged smoke test.

### Unsupported hosts

- **ChatGPT web / desktop chat without MCP support**: no stdio transport, not
  supported.
- **Hosts that require HTTP or SSE transport**: not supported. `cribra-mcp` is
  stdio only.
- **Hosts that require OAuth or remote authorization**: not supported; there is
  no auth surface.
- **Sandboxed hosts that forbid child processes**: not supported; the host must
  be able to spawn the executable.

No wrapper script, shim, or proxy is provided or required.

## 6. Tool examples

Exactly two tools are advertised.

### `cribra_scan`

```json
{
  "name": "cribra_scan",
  "arguments": {
    "source": "token = \"ghp_SYNTHETIC_EXAMPLE_ONLY\"",
    "rules": [
      {
        "id": "custom.example",
        "pattern": "ghp_[A-Za-z0-9]+",
        "severity": "critical"
      }
    ]
  }
}
```

Returns privacy-safe finding metadata only: rule id, byte offsets, line and
column, severity, confidence, remediation. It never returns the source, a
matched substring, or surrounding context.

### `cribra_transform`

```json
{
  "name": "cribra_transform",
  "arguments": {
    "source": "token = \"ghp_SYNTHETIC_EXAMPLE_ONLY\"",
    "mode": "redact"
  }
}
```

Returns Core-produced transformed source. `cribra_transform` is a
transformation, not anonymization: the caller receives the transformed text it
asked for. Keyed pseudonymization and synthesis are not advertised.

## 7. Protocol revisions

- modern MCP `2026-07-28`: stateless, per-request `_meta`, `server/discover`;
- legacy MCP `2025-11-25`: `initialize` / `notifications/initialized`.

Both coexist in one process. Unsupported revisions are rejected with fixed,
source-free errors.

## 8. Security and privacy properties

- No secret material is written to stderr; diagnostics are fixed strings.
- `cribra_scan` responses are canary-tested against leaking the input source,
  matched text, and surrounding context.
- Message size, response size, queue depth, and custom-rule counts are bounded.
- On sustained stdout backpressure the process terminates deterministically
  with exit code `3` instead of blocking forever. The terminal diagnostic is
  written before the exit code becomes observable, so a process exit cannot
  race the message away; this matters on Windows, where `ExitProcess` stops
  every other thread immediately.
- On stdin EOF the server ends the session cleanly with exit code `0`;
  responses already written remain valid, and work that had not started may be
  discarded.
- A closed stdout pipe results in a clean, deterministic shutdown.

## 9. Release process

Release namespace: `cribra-mcp-v<major>.<minor>.<patch>`. It never collides with
core `v*.*.*` or WASM `cribra-wasm-v*`.

Two triggers, deliberately separated:

1. **Tag push** of `cribra-mcp-v*` runs qualification and packaging only. That
   path has no `contents: write` permission and never creates a release.
2. **Manual `workflow_dispatch`** from the tag ref:
   - `mode: readiness` (default) builds, assembles, and verifies the full
     distribution without mutating any release;
   - `mode: stage` requires the exact confirmation string `stage <tag>`, runs
     in the protected `mcp-release` environment, checks that the tag exists and
     is reachable from `origin/main`, refuses to replace an existing release,
     verifies `SHA256SUMS`, and creates a **draft** release with
     `--draft --verify-tag --latest=false`.

The release is published by a human from the GitHub UI after reviewing the
draft. Artifacts are never replaced. `--latest=false` guarantees the MCP
release never becomes the repository's "latest" release.

The workflow validates:

- tag format `cribra-mcp-vX.Y.Z`;
- tag equals `cribra-mcp-v<crate version>`;
- dispatch ref is exactly `refs/tags/<tag>`;
- the tag exists;
- the tag is an ancestor of `origin/main`;
- formatting, clippy `-D warnings`, all MCP tests, and the conformance matrix.

Provenance is enforced end to end:

- every `build` job checks out `refs/tags/<tag>`, runs the full MCP test suite
  on its own native runner, and builds from that checkout;
- each job asserts that the archive it produced names the tag's version and its
  matrix target before uploading;
- build jobs upload only the archive, never a per-target `SHA256SUMS`;
- each `build` job qualifies its single archive with `--expected-version <v>`,
  which validates the supplied archives only and does not imply completeness;
- `assemble` rejects a pre-existing manifest, requires exactly five archives,
  generates the single `SHA256SUMS`, and then `--require-all
  --expected-version <v>` rejects any missing, extra, duplicate, or
  wrong-version archive;
- `stage` re-checks the downloaded artifact with `sha256sum -c` and the same
  completeness/version verifier before creating the draft;
- `gh release create --verify-tag` refuses an existing release, so no asset is
  ever replaced.

## 10. Qualification evidence

Executed locally on `feat/mcp-m4` (macOS arm64):

- `cargo test -p cribra-mcp --lib`: 56 passed, including the bounded-writer
  deadline and blocked-writer process-exit proofs.
- `cargo test -p cribra-mcp --test cli_surface`: 4 passed.
- `cargo test -p cribra-mcp --all-targets`: passed.
- Deterministic packaging: two independent builds of the same binary produced
  byte-identical archives
  (`sha256 d3ca94ce339e47cbf59dd20f824e7ab495d7222988767556ef73014e11e5bc5f`).
- `just mcp-package` + `just mcp-package-smoke`: all six packaged scenarios
  passed against the extracted archive:
  `version`, `help`, `rejected-argument`, `modern`, `legacy`, `broken-pipe`.
- Host config verification: Claude Code `0.2.38` (global and project) and
  Codex CLI `0.169.0-alpha.11.1` (`config.toml` `[mcp_servers.cribra]`), both
  in isolated config directories.
- DX.1–DX.4 consumer-side evidence: macOS ARM64 clean-room re-execution of the
  public archive; Codex CLI `0.169.1` host-mediated discovery and calls;
  OpenCode `2.3.2` host-mediated discovery and an 11-case tool matrix; Copilot
  documented feasibility and design. The consolidated record is
  [`DEVELOPER_EXPERIENCE.md`](DEVELOPER_EXPERIENCE.md). Only macOS ARM64 was
  re-executed in DX; the other four targets remain inspection-only there.
- `actionlint` clean on both new workflows.
- Adversarial mutation: making the bounded writer join the helper thread on
  timeout causes `threaded_writer_deadline_fires_while_write_is_still_blocked`
  to hang, which proves the deadline depends on not joining. Reverted; the
  unmodified suite passes.
- Packaging negative tests: a mislabeled archive (wrong target in the name), a
  wrong-architecture binary under a correct name, a stripped executable bit, and
  a corrupted `SHA256SUMS` are each rejected.
- Distribution-policy tests (`just mcp-smoke-tests`): a single archive passes
  with `--expected-version`, a mismatched version fails, a missing target fails
  only with `--require-all`, and a complete five-target set passes.
- Cross-target compile checks that do not require execution:
  - `x86_64-apple-darwin` release binary built, packaged, and structurally
    verified (Mach-O x86_64, SHA-256 recorded); all test binaries compile for
    that target;
  - the `cfg(not(unix))` path typechecks for `wasm32-wasip2`;
  - Linux cross-compilation stops in `blake3`'s C build because no
    `aarch64-linux-gnu-gcc` cross toolchain is installed, before reaching MCP
    code.

Executed in CI on every run of `.github/workflows/mcp.yml`:

- gate job on Linux: fmt, clippy, all tests, conformance matrix, package and
  packaged smoke;
- MSRV job on Rust 1.97.0;
- five-target packaged matrix, each running the full MCP test suite natively
  (including the Windows bounded-writer and BrokenPipe tests), then building,
  packaging, extracting, and smoke-testing the extracted archive.

The release workflow (`.github/workflows/mcp-release.yml`) repeats the full
test suite, version assertion, and packaged smoke on every native runner before
any archive is uploaded, then applies the completeness/version verifier after
assembly.

Not executed locally, and therefore not claimed as verified here:

- Windows process-level execution. The bounded-writer-thread code path is
  compiled and exercised on this host through `#[cfg(any(not(unix), test))]`,
  including a test that re-executes itself with an unread stdout pipe and
  asserts a bounded exit. The Windows-native full test suite runs on
  `windows-latest` in both the MCP workflow and the release build jobs.
  `std::process::exit` cannot block on the helper thread: runtime cleanup uses
  `try_lock` on the stdout buffer and never flushes it. Terminal diagnostics are
  written before the terminal code is published, so the Windows exit path cannot
  discard them. This is reasoned from the standard library source and proven at
  process level on the shared code path, but not executed on Windows in this
  local environment.
- `x86_64-apple-darwin` and both Linux targets on their native runners. The
  x86_64 binary and its test binaries build locally, but Rosetta is not
  installed on this host, so it is not executed here.
- Claude Desktop and the ChatGPT/Codex desktop app UI. Their configuration
  formats were verified from official documentation and the shipped CLI, but
  the GUI was not driven headlessly. Codex Desktop is therefore **PENDING** in
  the DX record, not PASS.
- Copilot cloud agent runtime. DX.4 established documented feasibility and a
  validated installation design only.

## 11. Deferred distribution evaluation items

The following are explicitly deferred. They are not part of the M4 or DX claim
for 0.1.1.

| Item | Current state | Deferred because |
| --- | --- | --- |
| Build provenance and attestations | `SHA256SUMS` only | Needs a signed provenance/attestation design; `SHA256SUMS` remains integrity, not authenticity |
| macOS Developer ID signing and notarization | ARM64 ad-hoc signed; x86_64 unsigned; neither notarized | Needs Apple Developer identity and a notarization pipeline |
| Windows code signing | PE unsigned | Needs a Windows signing identity and timestamping |
| Stripping and size optimization | Not a distribution-size policy | Needs a measured size/startup trade-off and build-profile change |
| Package-manager installation | Manual archives only | Needs channel ownership, update policy, and provenance |

## 12. Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| Host shows no tools | Wrong `command` path | Use the absolute path to the extracted executable |
| Server exits immediately | Host sent an unsupported argument | Run `cribra-mcp --help`; serve mode takes no arguments |
| Exit code `2` | Unrecognized CLI argument | Remove extra arguments from the host config |
| Exit code `3` | Sustained stdout backpressure | The host stopped reading stdout; restart the host/session |
| macOS "cannot be opened" | The artifact is not notarized; ARM64 is ad-hoc signed and x86_64 is unsigned | Verify the SHA-256 digest; there is no Gatekeeper bypass in this release. Use a signed/notarized build if policy requires it |
| Windows SmartScreen warning | The PE executable is unsigned | Verify the SHA-256 digest; there is no SmartScreen bypass in this release. Use a signed build if policy requires it |
| `--version` works in a terminal but not in the host | `PATH` differs for GUI processes | Use an absolute path |
| Checksum mismatch | Corrupted or partial download | Re-download the archive and re-verify |
| Linux "GLIBC_2.xx not found" | Host glibc older than 2.34, the highest symbol version referenced by the published binaries | Use a distribution with glibc 2.34 or newer, or build from source |
