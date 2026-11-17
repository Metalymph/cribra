# cribra-mcp {{version}} for {{target}}

`cribra-mcp` is Cribra's native Model Context Protocol stdio server. It reads
newline-delimited JSON-RPC 2.0 on stdin and writes responses on stdout. It has
no network access, no filesystem access, no configuration file, and no
persistent storage.

## 1. Extract

```sh
tar -xzf {{archive}}
```

On Windows use `tar -xf {{archive}}` or `Expand-Archive {{archive}}`.

The archive contains:

```
{{directory}}/
  {{binary}}
  LICENSE
  INSTALL.md
```

## 2. Verify the checksum (recommended)

The release publishes one `SHA256SUMS` file covering every archive. Verify only
the entry for this archive; checking the whole manifest fails when the other
archives are absent.

```sh
archive={{archive}}
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
$archive = '{{archive}}'
$entry = Get-Content SHA256SUMS | Where-Object { ($_ -split '\s+')[1] -eq $archive }
if (-not $entry) { Write-Error "SHA256SUMS has no entry for $archive"; exit 1 }
$expected = ($entry -split '\s+')[0].ToLower()
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLower()
if ($actual -ne $expected) { Write-Error "checksum mismatch for $archive"; exit 1 }
"$archive OK"
```

`SHA256SUMS` is an integrity check, not an authenticity guarantee: it confirms
that the archive matches the bytes recorded in the manifest, but it is not a
cryptographic signature, does not assert who built the artifact, and is not a
reproducible-build attestation.

## 3. Install

Move the extracted `{{directory}}` directory to a stable location. The server
is a single executable; there is no installer, service, PATH change, or runtime
dependency beyond the operating system C library.

{{platform_note}}

## 4. Configure your MCP host

Use the absolute path to the extracted executable. Do not rely on `PATH`.

```json
{
  "mcpServers": {
    "cribra": {
      "command": "{{absolute}}"
    }
  }
}
```

Claude Desktop, Claude Code, Codex, and generic stdio client configurations are
documented at
https://github.com/Metalymph/cribra/blob/main/crates/cribra-mcp/DISTRIBUTION.md

## 5. Use

The server advertises exactly two tools:

- `cribra_scan` returns privacy-safe finding metadata for a supplied `source`.
- `cribra_transform` returns Core-produced redacted or templated source.

## Protocol revisions

- modern MCP `2026-07-28` (stateless, per-request `_meta`);
- legacy MCP `2025-11-25` (`initialize` / `notifications/initialized`).

Diagnostics go to stderr only. While serving, stdout carries protocol messages
and nothing else.
