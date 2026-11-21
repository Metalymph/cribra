# cribra-mcp developer experience qualification

This is the canonical DX.1–DX.5 record for `cribra-mcp` 0.1.1. It consolidates
the developer-experience evidence collected after the M4 distribution release
and states, explicitly, which claims were executed, which were inspected only,
and which remain open.

This record does **not** change the M3 conformance verdict (**PARTIAL PASS**) or
the M4 distribution verdict (**PASS**). See [`CONFORMANCE.md`](CONFORMANCE.md)
and [`DISTRIBUTION.md`](DISTRIBUTION.md). Developer-experience evidence is
separate from both: a host-mediated interoperability pass does not certify the
server, and a documented Copilot design does not qualify Copilot at runtime.

## Overall verdict

| Area | Verdict |
| --- | --- |
| DX.1 — public artifact installation and execution | **PARTIAL PASS** — macOS ARM64 clean-room runtime PASS; four other targets inspected only |
| DX.2 — Codex CLI and Codex Desktop | **PARTIAL PASS** — CLI host-mediated PASS; model-driven CLI NOT QUALIFIED; Desktop PENDING |
| DX.3 — OpenCode | **PARTIAL PASS** — host-mediated PASS; model-driven NOT QUALIFIED |
| DX.4 — GitHub Copilot cloud agent | **PARTIAL PASS** — documented feasibility and installation design PASS; runtime NOT QUALIFIED |
| **DX overall** | **PARTIAL PASS** |

The single runtime PASS for artifact installation is macOS ARM64. The published
macOS ARM64 archive was downloaded from the public release, extracted into a
clean directory, checksum-verified, launched as a real MCP stdio server, and
re-executed from that clean directory. The other four archives were downloaded
and inspected statically (archive structure, executable format, architecture,
symbol floor) but were not re-executed in DX.1 because no native runtime for
them was available on the qualification host.

M4 remains a **PASS** because every archive was packaged and smoke-qualified on
its own native CI runner before publication. DX.1 is an independent
consumer-side re-check, not a replacement for that release evidence.

## Evidence boundaries

Four terms are used precisely throughout this record:

- **Runtime qualification** — the published executable was launched and
  exercised. Only macOS ARM64 received this treatment in DX.1.
- **Host-mediated execution** — a real host application (Codex `app-server`,
  OpenCode) spawned the server, discovered tools, and relayed tool calls. The
  probe never launched the server directly. This proves host integration, but
  not that a model chose a tool.
- **Model-driven execution** — a model received the tool catalog, decided to
  call a tool, and the host executed it. No host produced a successful
  model-driven Cribra call in DX.2–DX.4.
- **Documented feasibility / installation design** — official host
  documentation was reviewed and a proposed configuration was validated, but
  nothing was applied remotely and no host session ran it. This is the entire
  Copilot claim.

Direct JSON-RPC calls to the Cribra binary were not used as a substitute for
host compatibility evidence. No compatibility wrapper, shim, proxy, HTTP
transport, or remote service was introduced.

## Tested artifact

| Item | Value |
| --- | --- |
| Release | `cribra-mcp-v0.1.1` |
| Server version | `0.1.1` |
| macOS ARM64 archive | `cribra-mcp-0.1.1-aarch64-apple-darwin.tar.gz` |
| macOS ARM64 archive SHA-256 | `a0773ccc670fb0fadc778b0d38b4555bf85add64d455433f1d4e3fc6ac701e9a` |
| Extracted executable SHA-256 | `7dee6b39bbcab2737e530baff1488f03cefe7eebaa3b7478ce6ced3f89ad9ab4` |
| Linux x86_64 archive SHA-256 | `0f1a4e97c950ef6fa2d0e8a8404891303069f9017a4346f104b68db4248544ab` |
| Advertised tools | exactly `cribra_scan` and `cribra_transform` |
| Annotations | `readOnlyHint: true`, `destructiveHint: false`, `idempotentHint: true`, `openWorldHint: false` |
| Transport | local newline-delimited JSON-RPC 2.0 over stdio |
| Protocol revisions | legacy `2025-11-25`; modern `2026-07-28` |

## DX.1 — public artifact installation

**Verdict: PARTIAL PASS.**

Executed on macOS ARM64 (`macOS 27.2.1`, Darwin 27.2.0). The public archive was
downloaded from the GitHub release, not rebuilt from the repository.

Qualified:

- archive digest matches the published GitHub asset digest and `SHA256SUMS`;
- archive extracts to exactly one directory with the executable, `LICENSE`, and
  `INSTALL.md`;
- the executable is Mach-O arm64, marked executable, and runs from the clean
  directory;
- `--version` reports `cribra-mcp 0.1.1`;
- `--help` describes serve mode and the two options;
- an unknown argument fails with exit code `2` and writes nothing to stdout;
- modern discovery, `tools/list`, `cribra_scan`, and `cribra_transform` work
  against synthetic input;
- legacy `initialize` / `notifications/initialized` works;
- immediate stdin EOF (`< /dev/null`) is a clean exit `0`, not a hang or error;
- no network listener, filesystem discovery, configuration file, or persistent
  state was observed.

Static inspection covered all five archives and all five `SHA256SUMS` entries:

| Target | Archive format | Executable format | Runtime in DX.1 |
| --- | --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.gz` | Mach-O arm64 | **executed** |
| `x86_64-apple-darwin` | `.tar.gz` | Mach-O x86_64 | inspected only |
| `x86_64-unknown-linux-gnu` | `.tar.gz` | ELF x86_64 | inspected only |
| `aarch64-unknown-linux-gnu` | `.tar.gz` | ELF aarch64 | inspected only |
| `x86_64-pc-windows-msvc` | `.zip` | PE x86_64 | inspected only |

Both published Linux executables were inspected with `readelf --version-info`
on the published bytes. The highest referenced symbol version is
**`GLIBC_2.34`**, not `2.35`. GLIBC symbol inspection is a static requirement,
not a runtime qualification: the x86_64 archive was executed natively on
Ubuntu 26.04 by the M4 release pipeline and the aarch64 archive on Ubuntu 24.04
ARM64, but DX.1 did not execute either archive and no other distribution was
qualified.

### Initialization and memory observations

Measured against the macOS ARM64 public executable:

| Measurement | Result |
| --- | --- |
| Binary size | `2,927,632` bytes (about `2.792 MiB`) |
| Startup to discovery, median (`n = 40`) | `1.53 ms` |
| Warm 1 KiB scan, median (`n = 30`) | `0.037 ms` |
| Slowest first 1 KiB scan | `158 ms` |
| Warm 1 MiB scan, median (`n = 30`) | `7.24 ms` |
| Idle RSS | `2,464 KiB` |
| Warm RSS after work, including a 1 MiB scan | `115,280 KiB` (about `112.6 MiB`) |
| Sampled peak RSS during a 1 MiB scan | `117,264 KiB` (about `114.5 MiB`) |

The first scan compiles and retains the built-in scanner catalog. Memory grows
to roughly 112 MiB after a 1 MiB scan and is retained for the process lifetime;
it is released when stdin reaches EOF and the process exits. These are
single-host observations, not cross-platform guarantees.

## DX.2 — Codex CLI and Codex Desktop

**Verdict: PARTIAL PASS.**

| Area | Verdict |
| --- | --- |
| Codex CLI registration and discovery | **PASS** |
| Codex CLI host-mediated invocation | **PASS** |
| Codex CLI model-driven invocation | **NOT QUALIFIED** |
| Codex Desktop | **PENDING** |
| **DX.2 overall** | **PARTIAL PASS** |

Environment: Codex CLI `0.169.1` (bundled with Codex Desktop
`26.1111.71704`), isolated `CODEX_HOME`, macOS ARM64, public 0.1.1 executable.

Qualified through Codex's own `app-server` layer:

- Codex accepted the local stdio registration and reported
  `runtimeStatus: connected`;
- Codex discovered exactly two tools with their full schemas;
- Codex spawned and owned the absolute-path server;
- real `cribra_scan` and `cribra_transform` calls returned expected results;
- invalid arguments produced structured errors;
- repeated calls were stable;
- one server child was reused, and teardown left no orphan;
- privacy checks found no source or matched value in scan metadata.

The default configuration negotiated legacy **`2025-11-25`**.

Measured clean setup: `0.021 s` to write config and link auth, `0.030 s` for
`codex mcp list`, `0.915 s` for app-server discovery plus one scan, **`0.966 s`
total**. This excludes human orientation and reading the Codex documentation.

Not qualified:

- a model-driven Codex turn that selects and calls a Cribra tool. The account
  is usage-limited until **2026-11-23 20:38**; no model turn could run.
- Codex Desktop. It requires an authorized change to the shared global config,
  a restart, GUI approval, and a model turn. None was performed. Desktop shares
  the same MCP layer as the CLI, but that is an architectural expectation, not
  an executed Desktop result.

The global Codex configuration was not modified.

## DX.3 — OpenCode

**Verdict: PARTIAL PASS.**

| Area | Verdict |
| --- | --- |
| Host-mediated OpenCode integration | **PASS** |
| Model-driven OpenCode session | **NOT QUALIFIED** |
| **DX.3 overall** | **PARTIAL PASS** |

Environment: OpenCode `2.3.2`, macOS ARM64, isolated project configuration,
public 0.1.1 executable.

Qualified through OpenCode's own server and MCP client:

- `opencode mcp list` reported `✓ cribra connected`;
- OpenCode logged `mcp connected server=cribra tools=2`;
- exactly two tools were callable: `tools.cribra.cribra_scan` and
  `tools.cribra.cribra_transform`;
- the 11-case synthetic matrix passed: populated scan, repeated scan, candidate
  metadata, clean source, redact default, custom replacement, template default,
  namespaced/numbered template, invalid mode, empty-match pattern, and duplicate
  rule id;
- scan results remained metadata-only; candidates were not transformed;
- errors were fixed and did not echo source, patterns, rule ids, or options;
- one child process served all 11 calls and was cleaned up on teardown.

Default OpenCode negotiation uses its `legacy` mode and connected with
`2025-11-25`. A separate isolated configuration pinned `2026-07-28` and also
connected. OpenCode did not emit a raw wire trace, so the revision conclusion
rests on its bundled protocol policy plus the successful handshakes.

Measured cold setup from starting a private `opencode serve` to
`/api/mcp` reporting connected:

| Path | Server ready | MCP connected |
| --- | ---: | ---: |
| Default legacy negotiation | `0.221 s` | **`0.534 s`** |
| Pinned `2026-07-28` | `0.265 s` | `0.612 s` |

Configuration notes:

- the MCP permission action names are `<server>_<tool>`, here
  `cribra_cribra_scan` and `cribra_cribra_transform`;
- the isolated config used deny-by-default and explicitly allowed only
  `execute` and those two MCP actions;
- `codemode: true` was required for OpenCode's no-model Code Mode automation
  path to see the MCP tools;
- with `codemode: false`, agents saw the tools but the automation catalog
  reported `Unknown Code Mode tool: cribra.cribra_scan`.

Model-driven execution was attempted only with existing provider
configuration. It was blocked before any model turn:

| Provider/model | Result |
| --- | --- |
| `opencode/deepseek-v4.1-flash-free` | `provider.free-limit` HTTP 429 |
| `opencode/big-pickle` | same free-usage limit |
| `openai/gpt-5.5` | `provider.auth`: `Request failed: 400` |
| `openai/gpt-6.2-sol` | `provider.auth`: `Request failed: 400` |

No model selected a tool, no model-visible catalog was exercised, and no
synthetic fixture crossed a model-provider boundary. The global OpenCode
configuration and provider credentials were not changed.

## DX.4 — GitHub Copilot cloud agent

**Verdict: PARTIAL PASS.**

| Area | Verdict |
| --- | --- |
| Documented feasibility | **PASS** |
| Installation design and artifact verification | **PASS (design only)** |
| Actual Copilot runtime qualification | **NOT QUALIFIED — BLOCKED** |
| **DX.4 overall** | **PARTIAL PASS** |

No Copilot cloud agent session was started. The account probe reported
`copilot_plan: individual`, `access_type_sku: free_limited_copilot`,
`cloud_sessions_enabled: false`, and `is_mcp_enabled: true`. Copilot cloud agent
requires a paid plan, so the blocker is entitlement, not MCP support. No plan,
quota, credential, workflow, repository setting, issue, or pull request was
changed.

### What was established

- Repository MCP servers for Copilot cloud agent are configured in
  **Settings → Copilot → MCP servers**, not in a committed `.github/mcp.json`.
  The same setting is shared with Copilot code review.
- Copilot accepts native `local`/`stdio` servers with an absolute `command`, an
  `args` array, optional `env`, and an exact `tools` allowlist.
- Copilot supports MCP tools only, not resources or prompts.
- Copilot runner support is **Ubuntu x64 or Windows x64**; macOS runners are
  unsupported.
- `copilot-setup-steps.yml` applies only to **Actions** sessions. Cloud sandbox
  sessions do not run it, so an absolute path installed by that workflow does
  not exist in a Cloud session.
- The Copilot firewall does not apply to MCP servers or setup steps.
- Once configured, Copilot calls MCP tools autonomously without approval. The
  `tools` allowlist is the control.

### Artifact verification

The proposed Linux x86_64 artifact was downloaded from the public release and
verified against both its GitHub asset digest and `SHA256SUMS`:

| Item | Value |
| --- | --- |
| Asset | `cribra-mcp-0.1.1-x86_64-unknown-linux-gnu.tar.gz` |
| Archive size | `1,170,274` bytes |
| SHA-256 | `0f1a4e97c950ef6fa2d0e8a8404891303069f9017a4346f104b68db4248544ab` |
| Binary size | `3,658,920` bytes |
| Binary format | ELF 64-bit LSB PIE, x86-64, dynamically linked |
| Dynamic libraries | `libc.so.6`, `libgcc_s.so.1` |
| GLIBC symbol floor | `GLIBC_2.34` |
| Runtime execution | not executed on the macOS qualification host |

### Design-only example — not applied

The following snippets are proposals. They are examples only, they are not
operational repository files, and no Copilot runtime claim rests on them.

Proposed `.github/workflows/copilot-setup-steps.yml` (would apply only to an
Actions-based session):

```yaml
name: "Copilot Setup Steps"

on:
  workflow_dispatch:
  push:
    paths:
      - .github/workflows/copilot-setup-steps.yml
  pull_request:
    paths:
      - .github/workflows/copilot-setup-steps.yml

permissions:
  contents: read

jobs:
  copilot-setup-steps:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    permissions:
      contents: read
    steps:
      - name: Record runner platform
        run: |
          set -euo pipefail
          uname -srm
          cat /etc/os-release

      - name: Install pinned Cribra MCP 0.1.1
        env:
          CRIBRA_MCP_VERSION: "0.1.1"
          CRIBRA_MCP_ARCHIVE: "cribra-mcp-0.1.1-x86_64-unknown-linux-gnu.tar.gz"
          CRIBRA_MCP_SHA256: "0f1a4e97c950ef6fa2d0e8a8404891303069f9017a4346f104b68db4248544ab"
          CRIBRA_MCP_URL: "https://github.com/Metalymph/cribra/releases/download/cribra-mcp-v0.1.1/cribra-mcp-0.1.1-x86_64-unknown-linux-gnu.tar.gz"
        run: |
          set -euo pipefail
          work="$(mktemp -d)"
          trap 'rm -rf "$work"' EXIT

          curl --fail --silent --show-error --location --retry 3 \
            --output "$work/$CRIBRA_MCP_ARCHIVE" "$CRIBRA_MCP_URL"

          printf '%s  %s\n' "$CRIBRA_MCP_SHA256" "$work/$CRIBRA_MCP_ARCHIVE" \
            | sha256sum --check --strict

          tar --extract --gzip --file "$work/$CRIBRA_MCP_ARCHIVE" --directory "$work"

          sudo install -d -m 0755 /opt/cribra/bin
          sudo install -m 0755 \
            "$work/cribra-mcp-${CRIBRA_MCP_VERSION}-x86_64-unknown-linux-gnu/cribra-mcp" \
            /opt/cribra/bin/cribra-mcp

      - name: Verify installed binary
        run: /opt/cribra/bin/cribra-mcp --version
```

Proposed repository MCP configuration, entered in repository settings:

```json
{
  "mcpServers": {
    "cribra": {
      "type": "local",
      "command": "/opt/cribra/bin/cribra-mcp",
      "args": [],
      "tools": [
        "cribra_scan",
        "cribra_transform"
      ]
    }
  }
}
```

The workflow and JSON were syntax-validated locally. The artifact URL and
digest were verified. Neither the workflow nor any repository setting was
applied.

### Why runtime remains unqualified

1. The account has no paid Copilot entitlement and cloud sessions are disabled.
2. Applying the workflow or repository MCP settings would change production
   repository settings; that was not authorized.
3. No authorized isolated Copilot repository or environment was available.
4. Copilot has no documented model-free, host-mediated tool-call path. Any
   Copilot runtime evidence would necessarily be model-driven, and no model turn
   occurred.

Direct JSON-RPC testing would not cure any of these blockers and was not used as
a substitute.

## Canonical interpretation

- Overall DX is **PARTIAL PASS**, not PASS.
- M3 remains **PARTIAL PASS**; M4 remains **PASS**.
- Only the macOS ARM64 public artifact was re-executed in DX.1. The other four
  targets remain inspection-only in this record.
- Codex CLI and OpenCode qualification is **host-mediated**, not model-driven.
- Codex Desktop is **PENDING**, not PASS.
- Copilot is **documented feasibility plus design**, not runtime
  interoperability.
- The `GLIBC_2.34` symbol floor is the corrected value; the earlier DX.4 note
  that said `2.35` was inconsistent and is superseded by this record.

## Deferred DIST evaluation items

These are explicitly deferred and are not part of the 0.1.1 distribution claim.
They are recorded here so the next distribution work has a starting agenda.

| Item | Current state | Why deferred |
| --- | --- | --- |
| Build provenance and attestations | `SHA256SUMS` only; no signed provenance or build attestation | Requires a release-workflow and identity design; not present in 0.1.1 |
| macOS Developer ID signing and notarization | ARM64 is ad-hoc signed; x86_64 is unsigned; neither is notarized | Requires Apple Developer credentials and a notarization pipeline; Gatekeeper limits remain for 0.1.1 |
| Windows code signing | PE executable is unsigned | Requires a Windows signing identity and timestamping; SmartScreen limits remain for 0.1.1 |
| Stripping and size optimization | Release binaries are not size-optimized as a distribution policy | Requires a measured size/startup trade-off and a change to the build profile |
| Package-manager installation | Manual archive installation only | Homebrew, Scoop, winget, apt, and similar channels need ownership, update, and provenance policy |

The 0.1.1 records retain the distinction between **integrity** (`SHA256SUMS`)
and **authenticity** (a signature or attestation). No document in this
repository should describe the checksum manifest as proof of publisher identity.

## Follow-ups

Model-driven qualification is a follow-up, not a completed result. It can be
attempted when account access permits:

| Host | Blocker | Re-open when |
| --- | --- | --- |
| Codex CLI model-driven | ChatGPT usage limit until 2026-11-23 20:38 | A model turn can run without changing account configuration |
| Codex Desktop | Global config and GUI approval not authorized | An isolated or authorized Desktop profile is available |
| OpenCode model-driven | Free-model HTTP 429; OpenAI OAuth HTTP 400 | A working provider turn is available without credential changes |
| Copilot cloud agent | Copilot Free; cloud sessions disabled | A paid plan and an authorized Actions-based test session are available |
| Non-ARM64 native runtime | No native runtime on the qualification host | A native macOS x86_64, Linux x86_64/aarch64, or Windows x64 host is available |

A future Copilot run must use an **Actions** session, not a Cloud sandbox
session, because only Actions runs `copilot-setup-steps.yml`. It must capture
the **Start MCP Servers** log, the exact tool results, and, if observable, the
negotiated protocol revision. It must also verify that teardown leaves no
server process.

## Reproduction evidence

DX evidence is stored outside the repository under the qualification host's
temporary directory:

- `$TMPDIR/dx1-qual/` — release metadata, downloaded archives, clean-room
  extraction, measurements, probe transcripts;
- `$TMPDIR/dx2-qual/DX2_CODEX_QUALIFICATION.md` — Codex CLI evidence;
- `$TMPDIR/dx3-qual/DX3_OPENCODE_QUALIFICATION.md` — OpenCode evidence;
- `$TMPDIR/dx4-qual/DX4_COPILOT_CLOUD_AGENT_QUALIFICATION.md` — Copilot
  feasibility and design evidence, including the proposed workflow, MCP
  configuration, and qualification prompt.

These paths are qualification artifacts, not repository content, and are not
required to build or install `cribra-mcp`.

## Repository state at DX.5

- HEAD: `5413564296b27d74374395a3663b2e6f5273463f`
- Only pre-existing untracked file: `tempra_k52_phase_timings.py`
- Pre-existing stash: `stash@{0}` — `On feat/react-native-rn1-4: security
  workflow WIP`
- No commit, tag, release, publication, or remote host configuration was made
  during DX.1–DX.4.
