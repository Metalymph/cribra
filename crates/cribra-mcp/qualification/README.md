# SDK interoperability qualification

These harnesses exercise the **real** `cribra-mcp` release binary through two
independent, non-Rust MCP clients. They exist to answer one question: does a
client written against the published MCP SDKs interoperate with this server
over stdio, without any Cribra-specific shim?

They are qualification-only. Nothing here is a dependency of the `cribra-mcp`
crate, and nothing here is loaded at runtime by the server.

## What these harnesses establish — and what they do not

Established:

- an independent client negotiates each supported revision and can call both
  advertised tools;
- tool results satisfy the advertised output schemas, checked by a third-party
  JSON Schema validator rather than by Cribra's own test code;
- negotiation is observable and matches what the SDK actually does, including
  the fact that a *pinned modern* client performs no discovery probe;
- the child process terminates on a clean client close.

Not established:

- **No official conformance certification.** The official
  `@modelcontextprotocol/conformance` server runner accepts only a URL and has
  no stdio mode, so zero official server scenarios can execute against this
  binary. See [`../CONFORMANCE.md`](../CONFORMANCE.md).
- No HTTP, authorization, resources, prompts, sampling, elicitation, tasks or
  progress coverage — none of those are implemented.

## Pinned clients

| Harness | Package | Pin |
| --- | --- | --- |
| TypeScript | `@modelcontextprotocol/client` | `2.4.0` |
| TypeScript | `ajv` (draft 2020-12 validation) | `8.17.1` |
| Python | `mcp` | `2.3.0` |
| Python | `jsonschema` | `4.26.0` |

`package-lock.json` and `requirements.txt` pin the direct dependencies.
Transitive resolution observed on 2026-11-15 (Python 3.13.18):
`anyio 4.16.0`, `attrs 26.1.0`, `pydantic 2.14.3`, `pydantic_core 2.50.4`,
`httpx2 2.16.0`, `httpcore2 2.16.0`, `starlette 1.10.0`, `uvicorn 0.54.0`,
`jsonschema-specifications 2025.9.1`, `referencing 0.37.0`, `rpds-py 2026.9.1`,
`mcp-types 2.3.0`.

## Run

```sh
just mcp-qualify
```

or directly:

```sh
crates/cribra-mcp/qualification/run-qualification.sh
```

Both create the local `node_modules` / `.venv` if absent. They are ignored by
git.

## Scenarios

TypeScript (`typescript/run.mjs`):

| Scenario | Proves |
| --- | --- |
| `modern-pin-discovery-and-tools` | pinned `2026-07-28` negotiation, explicit discovery, exact capability set, both tools, schema-valid results, tool errors |
| `auto-negotiation-selects-modern` | `auto` probes and selects the modern era (the legacy *fallback* path is **not** exercised) |
| `legacy-handshake-and-tools` | `2025-11-25` `initialize` handshake, both tools, no modern-only fields |
| `unexpected-server-exit-surfaces-as-error` | a killed server rejects a pending call instead of hanging |
| `clean-close-terminates-the-child` | `client.close()` reaps the child and later calls fail fast |

Python (`python/run.py`):

| Scenario | Proves |
| --- | --- |
| `pinned-modern-client-does-not-probe` | a pinned modern client installs a *locally synthesized* discovery result (`cacheScope=public`, no capabilities) and sends no probe; `discover()` then reuses it |
| `explicit-discovery-over-the-wire` | on a legacy connection with no cached result, `session.discover()` issues a real `server/discover` and adopts the server's answer (`cacheScope=private`, `ttlMs=0`, tools capability) |
| `auto-negotiation-selects-modern` | `auto` really probes; the adopted result is server-authored, not synthesized |
| `legacy-handshake-and-tool-errors` | legacy handshake, both tools, and the fixed tool-error taxonomy over the wire |
| `clean-close-terminates-the-child` | the spawned child process is gone after the client context exits |

The distinction between a pinned client and a probing client matters: claiming
that a pinned modern client "discovered" the server would be false, so the
Python harness asserts the opposite and tests discovery explicitly.
