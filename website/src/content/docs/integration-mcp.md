---
title: MCP server integration
description: Install the published cribra-mcp 0.1.1 stdio server, verify its archives, and configure it in Claude Desktop, Claude Code and Codex.
order: 29
---

# MCP server integration

`cribra-mcp` 0.1.1 is a published native Rust [Model Context Protocol](https://modelcontextprotocol.io/)
server. It is a **tool integration**, not a language binding: it does not wrap
Cribra in a language package, and it is not a WASM, UniFFI, C ABI, TypeScript or
React Native distribution.

The server exposes exactly two tools, `cribra_scan` and `cribra_transform`,
backed directly by Cribra Core over newline-delimited JSON-RPC 2.0 on local
stdio. Your application or host still owns source acquisition, persistence,
presentation, networking and destination policy.

## Status and verdicts

| Item              | Result                                                                                                                                                                                                                                                             |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Published version | `cribra-mcp` 0.1.1 — [release `cribra-mcp-v0.1.1`](https://github.com/Metalymph/cribra/releases/tag/cribra-mcp-v0.1.1)                                                                                                                                             |
| M3 conformance    | **PARTIAL PASS** — the 147-row conformance matrix has 10 of 10 applicable rows passing, but the official runner is HTTP-only and executed 0 rows; separately, 10 of 10 independent SDK qualification scenarios pass (5 TypeScript, 5 Python). Nothing is certified |
| M4 distribution   | **PASS** — five native archives and one `SHA256SUMS` manifest, packaged and smoke-qualified on their own native runners                                                                                                                                            |
| Transport         | Local stdio only                                                                                                                                                                                                                                                   |

Two distinct M3 evidence sets back the verdict, and they are not
interchangeable:

- **Canonical conformance matrix:** 147 rows total. The 10 rows applicable to
  this server's advertised scope pass against the real binary, 0 fail, and 0 are
  executed by the official runner because it accepts only an HTTP URL and has no
  stdio server mode. These 10 are applicability-scoped canonical server
  scenarios, not official-runner results, and nothing is certified.
- **Independent SDK qualification:** 10 scenarios total — 5 with the TypeScript
  SDK (`@modelcontextprotocol/client` 2.4.0) and 5 with the Python SDK (`mcp`
  2.3.0). Both suites pass 5 of 5 end-to-end scenarios. These SDK scenarios are
  separate from the 147-row matrix and do not substitute for official-runner
  execution.

The M4 distribution result does not upgrade, replace or re-certify the M3
verdict. The canonical records are the
[M3 conformance record](https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/CONFORMANCE.md)
and the
[M4 distribution record](https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/DISTRIBUTION.md).

## Qualified targets

Five qualified native targets are published. Every archive is built natively — no cross-compilation and no emulation. Each is
checksummed, extracted into a clean temporary directory, and qualified by running
the extracted executable as a real MCP server on its own runner.

| Target triple               | Archive   | Binary format |
| --------------------------- | --------- | ------------- |
| `aarch64-apple-darwin`      | `.tar.gz` | Mach-O arm64  |
| `x86_64-apple-darwin`       | `.tar.gz` | Mach-O x86_64 |
| `x86_64-unknown-linux-gnu`  | `.tar.gz` | ELF x86_64    |
| `aarch64-unknown-linux-gnu` | `.tar.gz` | ELF aarch64   |
| `x86_64-pc-windows-msvc`    | `.zip`    | PE x86_64     |

`aarch64-pc-windows-msvc` and musl targets are intentionally not produced.
Linux archives target glibc 2.35 or newer (Ubuntu 22.04 or equivalent). macOS
archives are ad-hoc signed, not notarized; Windows executables are unsigned.

## Download and verify

Download the archive for your target and the shared `SHA256SUMS` manifest from
the release. The archive name is
`cribra-mcp-0.1.1-<target>.<tar.gz|zip>`.

```sh
version=0.1.1
target=aarch64-apple-darwin
base="https://github.com/Metalymph/cribra/releases/download/cribra-mcp-v${version}"
curl -LO "${base}/cribra-mcp-${version}-${target}.tar.gz"
curl -LO "${base}/SHA256SUMS"
```

Verify before extracting. The manifest covers all five archives, so verify from a
directory that contains the archive you downloaded:

```sh
shasum -a 256 -c SHA256SUMS    # macOS
sha256sum -c SHA256SUMS        # Linux
```

```powershell
Get-FileHash -Algorithm SHA256 cribra-mcp-0.1.1-x86_64-pc-windows-msvc.zip
```

A checksum mismatch means the download is corrupted or incomplete. Re-download
and verify again. `SHA256SUMS` authenticates the exact published binaries; it is
not a reproducible-build attestation.

## Install

Each archive extracts to exactly one directory containing the executable,
`LICENSE` and `INSTALL.md`.

### macOS and Linux

```sh
tar -xzf cribra-mcp-0.1.1-<target>.tar.gz
sudo mv cribra-mcp-0.1.1-<target> /opt/
/opt/cribra-mcp-0.1.1-<target>/cribra-mcp --version
```

If a browser download sets the macOS quarantine attribute:

```sh
xattr -d com.apple.quarantine /opt/cribra-mcp-0.1.1-<target>/cribra-mcp
```

### Windows

```powershell
Expand-Archive cribra-mcp-0.1.1-x86_64-pc-windows-msvc.zip -DestinationPath C:\Tools
C:\Tools\cribra-mcp-0.1.1-x86_64-pc-windows-msvc\cribra-mcp.exe --version
```

### CLI surface

```text
cribra-mcp            Serve MCP over stdin/stdout
cribra-mcp --version  Print the server version
cribra-mcp --help     Print help
```

Unknown or extra arguments fail with exit code `2` and write nothing to stdout.
While serving, stdout carries protocol messages only; diagnostics go to stderr.

## Local stdio transport

The host starts one process per session and speaks newline-delimited JSON-RPC 2.0
over stdin/stdout. There is no HTTP listener, no authorization surface and no
wrapper, shim or proxy.

Use the **absolute path** to the extracted executable. GUI-launched hosts often
run without the shell `PATH` that installed it. `cribra-mcp` needs no arguments
and no environment variables.

## Host configuration

### Claude Desktop

Config file:

| OS      | Path                                                              |
| ------- | ----------------------------------------------------------------- |
| macOS   | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json`                     |
| Linux   | `~/.config/Claude/claude_desktop_config.json`                     |

```json
{
  "mcpServers": {
    "cribra": {
      "command": "/opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
      "args": []
    }
  }
}
```

Restart Claude Desktop completely after editing the file.

### Claude Code

Current Claude Code releases accept an explicit transport and a `--` separator:

```sh
claude mcp add --transport stdio -s user cribra -- /opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
```

Project scope writes a committed `.mcp.json`:

```json
{
  "mcpServers": {
    "cribra": {
      "type": "stdio",
      "command": "/opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
      "args": []
    }
  }
}
```

The distribution record also documents the global and project formats used by
Claude Code `0.2.38`, which was executed end to end locally.

### Codex CLI

Codex stores MCP servers in `$CODEX_HOME/config.toml` (default
`~/.codex/config.toml`):

```toml
[mcp_servers.cribra]
command = "/opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp"
```

```sh
codex mcp add cribra -- /opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp
codex mcp list
codex mcp get cribra
```

Codex CLI `0.169.0-alpha.11.1` was verified with an isolated `CODEX_HOME`;
`codex mcp get cribra` reported `transport: stdio`, `enabled: true` and the
absolute command.

### Generic stdio hosts

Any host that supports an MCP `stdio` server accepts the same three fields:

```json
{
  "command": "/opt/cribra-mcp-0.1.1-aarch64-apple-darwin/cribra-mcp",
  "args": [],
  "env": {}
}
```

## Tools

Exactly two tools are advertised. The published JSON schemas are the
client-facing contracts; runtime validation also enforces UTF-8 byte limits and
Core constraints.

### `cribra_scan`

Input: `source` plus optional `rules`.

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

The result is **metadata only**: rule id, zero-based UTF-8 byte offsets,
one-based line and column, severity, confidence and remediation. Findings and
review-only candidates remain distinct. `cribra_scan` never returns the source,
a matched substring, surrounding context or candidate text.

### `cribra_transform`

Input: `source`, `mode` (`redact` or `template`), optional `rules`, and
mode-specific `options`.

```json
{
  "name": "cribra_transform",
  "arguments": {
    "source": "token = \"ghp_SYNTHETIC_EXAMPLE_ONLY\"",
    "mode": "redact"
  }
}
```

- `redact` replaces findings with `[REDACTED]` by default, or with a validated
  `options.replacement`.
- `template` replaces findings with semantic placeholders such as
  `<CRIBRA:github.classic-pat>`. `options.namespace` defaults to `CRIBRA`;
  `options.numbered` adds per-rule occurrence numbers.

The transformed source is returned as `structuredContent.output` and as the
first text content block. Unchanged regions remain visible. `cribra_transform`
is a transformation, not anonymization: candidates and undetected sensitive
values may remain.

## Protocol versions

One process supports both revisions:

| Revision            | Behavior                                                                                                                           |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Modern `2026-07-28` | Stateless requests with per-request `_meta`; `server/discover`, `tools/list` and `tools/call`; `supportedVersions: ["2026-07-28"]` |
| Legacy `2025-11-25` | `initialize` / `notifications/initialized` lifecycle, then `tools/list` and `tools/call`                                           |

A modern request and a legacy lifecycle may coexist in the same process.
Unsupported revisions are rejected with fixed, source-free errors. Exactly
`{"tools": {}}` is advertised; there are no prompts, resources, logging,
completion, sampling, elicitation or tasks.

## Limits and limitations

- **Source limit:** 1 MiB UTF-8 bytes per tool call.
- **Custom rules:** up to 32 per call; identifiers are 1–128 ASCII characters
  and patterns 1–4096 Unicode scalar values. Invalid patterns and empty matches
  are rejected.
- **Results:** at most 4096 findings and 4096 candidates; exceeding either is a
  hard error, never truncation. An encoded response over 8 MiB becomes a
  `response_too_large` tool error.
- **No filesystem discovery:** the caller supplies the source text. The server
  does not traverse directories or repositories.
- **No network or persistence:** there is no HTTP/SSE transport, OAuth surface,
  telemetry, auto-update, database, cache of scan results or cross-request state.
- **No keyed modes:** keyed pseudonymization and synthesis are not advertised and
  no key material is accepted.
- **No preemption:** Core execution is synchronous and is not presented as
  preemptible. A cancelled call may still consume worker time until Core returns.
- **Unsupported hosts:** hosts that require HTTP/SSE, OAuth, or forbid child
  processes cannot run this server.

Exit codes: `0` normal EOF shutdown or peer-closed stdout; `1` internal
reader/encoder/writer failure; `2` startup or argument failure; `3` sustained
stdout backpressure.

## Verification

Local and CI qualification for the 0.1.1 line:

```sh
cargo test -p cribra-mcp --all-targets
just mcp-smoke-tests
just mcp-package
just mcp-package-smoke
```

`just mcp-package-smoke` uses the same per-target invocation as the release
workflow, `--dir target/mcp-dist --expected-version "$version"`, and runs the
extracted archive as a real MCP server. The release workflow additionally
requires exactly five archives with `--require-all` and verifies the single
`SHA256SUMS` manifest before staging.

The M3 record documents the 147-row canonical matrix with 10 applicable passes,
the official-runner limitation, adversarial transport and privacy tests, and the
separate 10 independent SDK qualification scenarios (5 TypeScript, 5 Python).
The M4 record documents host configuration checks, deterministic packaging,
packaged smoke scenarios and the unsupported-host boundaries.

## Related

- [MCP distribution record](https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/DISTRIBUTION.md)
- [MCP conformance record](https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/CONFORMANCE.md)
- [MCP server README](https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/README.md)
- [Compatibility and distribution](/docs/compatibility-and-distribution/)
- [Getting started](/docs/getting-started/)
