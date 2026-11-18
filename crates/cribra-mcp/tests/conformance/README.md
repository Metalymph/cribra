# Pinned conformance referee inputs

These files are byte-exact copies of the canonical requirement sets published by
the official MCP conformance suite. They are vendored so that the qualification
matrix can be checked offline and so that the verdict cannot drift silently when
upstream changes.

| File | Upstream path | SHA-256 |
| --- | --- | --- |
| `requirements-2026-07-28.yaml` | `requirements/2026-07-28.yaml` | `ae2f4f6210fd729e2e318edd5bbfa31a43cee0bc608e48052fa26dbf1d939b57` |
| `requirements-2025-11-25.yaml` | `requirements/2025-11-25.yaml` | `f33a304dfa2cbd999c24026a3453a64f377bba0c8aa80addadaf05862d212371` |

## Provenance

- Repository: <https://github.com/modelcontextprotocol/conformance>
- Commit: `96a6f9478b7f2e125cf0360a213383cf57533893`
  (`chore(sep-2640): repoint the requirements at the Skills beta page (#511)`,
  committed 2026-11-10)
- Retrieved: 2026-11-15
- Package version at that commit: `@modelcontextprotocol/conformance@0.2.0-alpha.12`
- Licence: MIT (same repository)

## Why these files are authoritative

Upstream describes each file as "the canonical answer to 'which scenarios must my
implementation pass to conform to <revision>'". The `server:` and `client:` lists
are the scored requirement sets for that revision. The `not_scored:` list records
scenarios that run but never count toward a pass rate, together with the frozen
reason (`extension`, `added-after-release`, `pending`). Both files are marked
frozen upstream.

## How they are consumed

`matrix.py` parses these files with a strict, dependency-free reader that rejects
any structure it does not explicitly recognise, merges the result with the
applicability policy, and either rewrites or verifies the generated matrix region
in [`../../CONFORMANCE.md`](../../CONFORMANCE.md).

Do not edit these YAML files. If the referee must be re-pinned, update the commit
above, re-download both files, update the SHA-256 table, and re-run
`just mcp-matrix` so the diff is reviewable.
