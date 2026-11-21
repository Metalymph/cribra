# cribra-mcp {{version}} for {{target}}

`cribra-mcp` is Cribra's native Model Context Protocol stdio server. It reads
newline-delimited JSON-RPC 2.0 on stdin and writes responses on stdout. It has
no network access, no filesystem access, no configuration file, and no
persistent storage.

## 1. Verify the checksum (recommended)

Verify the archive **before extracting it**. The release publishes one
`SHA256SUMS` file covering every archive. Verify only the entry for this
archive; checking the whole manifest fails when the other archives are absent.

{{checksum_block}}

The platform checksum command exits non-zero and reports a mismatch. The
explicit "no entry" check fails when the manifest does not name this archive,
rather than silently verifying nothing.

`SHA256SUMS` is an integrity check, not an authenticity guarantee: it confirms
that the archive matches the bytes recorded in the manifest, but it is not a
cryptographic signature, does not assert who built the artifact, and is not a
reproducible-build attestation.

## 2. Extract

{{extract_block}}

The archive contains:

```
{{directory}}/
  {{binary}}
  LICENSE
  INSTALL.md
```

## 3. Install to a user-local directory

{{install_block}}

The server is a single executable; there is no installer, service, `PATH`
change, or runtime dependency beyond the operating system C library. Replace
`you` in the example configuration with your user name.

{{platform_note}}

## 4. Confirm the CLI surface

{{cli_block}}

Serve mode takes no arguments. Unknown or extra arguments fail with exit code
`2` and write nothing to stdout.

**stdin lifetime:** the host must start one process per session and keep its
stdin open for as long as the session needs the server. `cribra-mcp` treats
stdin EOF as the end of the session and exits cleanly with exit code `0`.
Responses already written to stdout remain valid. Work that had not started may
be discarded, and a request written immediately before EOF may therefore
return no response. MCP hosts must keep stdin open while awaiting responses.
An immediate EOF (for example, running the executable with stdin connected to
an empty file or `/dev/null`) is a clean no-op shutdown, not an error. The
server does not daemonize and does not stay alive without a client.

## 5. Configure your MCP host

Use the absolute path to the extracted executable. Do not rely on `PATH`, and
do not use `~` or an environment variable that the host may not expand. The
JSON below is valid for this target; on Windows the backslashes are
JSON-escaped.

```json
{{host_config}}
```

Claude Desktop, Claude Code, Codex CLI, Codex Desktop, OpenCode, the proposed
GitHub Copilot Actions setup (design only), and generic stdio client
configurations are documented at
https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/DISTRIBUTION.md

## 6. Use

The server advertises exactly two tools:

- `cribra_scan` returns privacy-safe finding metadata for a supplied `source`.
- `cribra_transform` returns Core-produced redacted or templated source.

`cribra_scan` never returns the source, a matched substring, surrounding
context, or a candidate value. `cribra_transform` intentionally returns
transformed source; it is not anonymization.

## 7. Protocol revisions

- modern MCP `2026-07-28` (stateless, per-request `_meta`);
- legacy MCP `2025-11-25` (`initialize` / `notifications/initialized`).

Diagnostics go to stderr only. While serving, stdout carries protocol messages
and nothing else.

## 8. Initialization and memory

The published macOS ARM64 executable used for these measurements is
`2,927,632` bytes (about `2.792 MiB`). The first tool call initializes the
built-in scanner catalog and retains it for the lifetime of the process.

Qualified macOS ARM64 measurements (DX.1, public 0.1.1 artifact):

- start-to-discovery median: `1.53 ms` (`n = 40`);
- warm `cribra_scan`, 1 KiB source: `0.037 ms` median;
- the slowest observed first 1 KiB scan was `158 ms`, including first-call
  catalog initialization;
- idle resident set: `2,464 KiB`;
- warm resident set after work, including a 1 MiB scan: `115,280 KiB`
  (about `112.6 MiB`);
- sampled peak resident set during a 1 MiB scan: `117,264 KiB`
  (about `114.5 MiB`).

The retained memory is process-local and is released when stdin reaches EOF
and the process exits. These figures are observations from one macOS ARM64
host, not a cross-platform guarantee. The other four targets were inspected
statically and were not independently re-executed in DX.1.
