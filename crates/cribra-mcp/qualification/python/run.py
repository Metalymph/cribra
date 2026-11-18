#!/usr/bin/env python3
"""Python SDK interoperability qualification for the real cribra-mcp binary.

Uses the official ``mcp`` client package pinned to 2.3.0 and its stdio
transport. No JSON-RPC is hand-crafted here: every request goes through the
published client API, and every tool result is additionally validated against
the tool's advertised output schema.

Negotiation API facts this harness relies on (verified against the installed
package, not assumed):

* ``Client(mode=...)`` accepts ``'legacy'``, ``'auto'`` or an exact modern
  protocol version such as ``'2026-07-28'``.
* A pinned modern mode does **not** probe ``server/discover``. It installs a
  locally synthesized ``DiscoverResult`` (``cache_scope='public'``, empty
  capabilities). We therefore assert that shape to prove no wire discovery was
  adopted, and we exercise discovery explicitly in a separate scenario.
* ``Client.session.discover()`` returns the cached result when one exists and
  only issues a real ``server/discover`` request otherwise. Explicit discovery
  is therefore tested on a legacy-mode connection, before any modern state has
  been adopted.
* ``'auto'`` probes via ``server/discover`` and falls back to ``initialize``
  only when the probe is not definitive. This server answers the probe, so the
  legacy *fallback path* is not exercised and is not claimed.

Run with:
    CRIBRA_MCP_BIN=/abs/path/to/cribra-mcp .venv/bin/python run.py
"""

from __future__ import annotations

import asyncio
import json
import os
import subprocess
import sys
import time
from pathlib import Path

from jsonschema import Draft202012Validator
from mcp import Client, StdioServerParameters

HERE = Path(__file__).resolve().parent
DEFAULT_BIN = HERE.parents[3] / "target" / "release" / "cribra-mcp"
BIN = os.environ.get("CRIBRA_MCP_BIN", str(DEFAULT_BIN))

MODERN = "2026-07-28"
LEGACY = "2025-11-25"

# Synthetic secret. Not a real credential.
SECRET = "ghp_syntheticPythonQualificationToken0000"

SCHEMAS = {
    name: json.loads((HERE.parents[1] / "schemas" / f"{name}.output.json").read_text())
    for name in ("cribra_scan", "cribra_transform")
}
VALIDATORS = {name: Draft202012Validator(schema) for name, schema in SCHEMAS.items()}


def check(condition: object, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def assert_schema(tool: str, result: object) -> dict:
    structured = getattr(result, "structured_content", None)
    check(structured is not None, f"{tool}: structuredContent is missing")
    errors = sorted(VALIDATORS[tool].iter_errors(structured), key=lambda e: list(e.path))
    if errors:
        rendered = "; ".join(f"{list(e.path)}: {e.message}" for e in errors[:5])
        raise AssertionError(f"{tool}: structuredContent violates the output schema: {rendered}")
    check(result.content and result.content[0].type == "text", f"{tool}: first content block is not text")
    if "output" in structured:
        check(
            result.content[0].text == structured["output"],
            f"{tool}: text block disagrees with structuredContent.output",
        )
    return structured


def server_params() -> StdioServerParameters:
    return StdioServerParameters(command=BIN, args=[], env=None)


def child_pids() -> set[int]:
    """PIDs of this process's direct children that look like our server."""
    output = subprocess.run(
        ["ps", "-Ao", "pid=,ppid=,command="], capture_output=True, text=True, check=True
    ).stdout
    mine = os.getpid()
    found = set()
    for line in output.splitlines():
        parts = line.split(None, 2)
        if len(parts) < 3:
            continue
        pid, ppid, command = int(parts[0]), int(parts[1]), parts[2]
        if ppid == mine and "cribra-mcp" in command:
            found.add(pid)
    return found


async def scenario(name: str, body) -> dict:
    started = time.monotonic()
    try:
        details = await body()
    except Exception as error:  # noqa: BLE001 - reported, not swallowed
        duration = round((time.monotonic() - started) * 1000)
        print(f"FAIL {name}: {error}", file=sys.stderr)
        return {"scenario": name, "status": "fail", "durationMs": duration, "error": str(error)}
    duration = round((time.monotonic() - started) * 1000)
    print(f"PASS {name}")
    return {"scenario": name, "status": "pass", "durationMs": duration, "details": details}


async def pinned_modern_client_does_not_probe() -> dict:
    """A pinned modern client negotiates locally; discovery is a separate act."""
    async with Client(server_params(), mode=MODERN) as client:
        check(client.protocol_version == MODERN, f"negotiated {client.protocol_version}")
        session = client.session
        installed = session.discover_result
        check(installed is not None, "pinned mode did not install a DiscoverResult")

        # The SDK's synthesized result is distinguishable from a server
        # response: our server always answers with cacheScope 'private' and the
        # tools capability. Seeing 'public' with no tools proves the pinned
        # client never adopted a discovery response from this server.
        check(
            installed.cache_scope == "public",
            f"pinned mode unexpectedly adopted a wire result: {installed.cache_scope}",
        )
        check(
            installed.capabilities.tools is None,
            "pinned mode unexpectedly adopted server capabilities",
        )

        # A cached result is returned as-is, so this issues no request either.
        same = await session.discover()
        check(same is installed, "discover() did not reuse the locally installed result")

        tools = await client.list_tools()
        names = sorted(tool.name for tool in tools.tools)
        check(names == ["cribra_scan", "cribra_transform"], f"advertised tools {names}")
        for tool in tools.tools:
            check(bool(tool.description), f"{tool.name}: missing description")
            check(tool.input_schema.get("type") == "object", f"{tool.name}: inputSchema")
            check(tool.output_schema is not None, f"{tool.name}: missing outputSchema")

        scan = await client.call_tool("cribra_scan", {"source": f"before {SECRET} after"})
        check(scan.is_error is False, "scan isError")
        structured = assert_schema("cribra_scan", scan)
        check(len(structured["findings"]) == 1, f"finding count {len(structured['findings'])}")
        check(SECRET not in json.dumps(structured), "scan leaked the matched secret")

        templated = await client.call_tool(
            "cribra_transform",
            {
                "source": f"before {SECRET} after",
                "mode": "template",
                "rules": [{"id": "custom.synthetic", "pattern": "ghp_[A-Za-z0-9]+", "severity": "high"}],
                "options": {"namespace": "probe"},
            },
        )
        check(templated.is_error is False, "template isError")
        output = assert_schema("cribra_transform", templated)["output"]
        check(SECRET not in output, "template leaked the secret")

        return {
            "protocolVersion": client.protocol_version,
            "locallyInstalledDiscovery": True,
            "tools": names,
        }


async def explicit_discovery_over_the_wire() -> dict:
    """Discovery sent explicitly on a legacy connection, with no cached result."""
    async with Client(server_params(), mode="legacy") as client:
        check(client.protocol_version == LEGACY, f"legacy negotiated {client.protocol_version}")
        check(client.session.discover_result is None, "legacy mode pre-installed a modern result")

        discover = await client.session.discover()
        check(list(discover.supported_versions) == [MODERN], f"supportedVersions {discover.supported_versions}")
        check(discover.cache_scope == "private", f"cacheScope {discover.cache_scope}")
        check(discover.ttl_ms == 0, f"ttlMs {discover.ttl_ms}")
        check(discover.result_type == "complete", f"resultType {discover.result_type}")
        check(discover.capabilities.tools is not None, "server did not advertise the tools capability")
        for absent in ("prompts", "resources", "logging", "sampling", "elicitation"):
            check(
                getattr(discover.capabilities, absent, None) is None,
                f"unadvertised capability {absent} must not appear",
            )

        # Adoption moved the session to the modern revision; the advertised
        # tools must still work, proving dual-era dispatch on one connection.
        check(client.protocol_version == MODERN, f"post-discovery version {client.protocol_version}")
        tools = await client.list_tools()
        check(len(tools.tools) == 2, "post-discovery tools/list")
        scan = await client.call_tool("cribra_scan", {"source": "alpha"})
        check(scan.is_error is False, "post-discovery scan")
        assert_schema("cribra_scan", scan)

        return {
            "supportedVersions": list(discover.supported_versions),
            "cacheScope": discover.cache_scope,
            "ttlMs": discover.ttl_ms,
            "negotiatedAfterDiscovery": client.protocol_version,
        }


async def auto_negotiation_selects_modern() -> dict:
    async with Client(server_params(), mode="auto") as client:
        check(client.protocol_version == MODERN, f"auto negotiated {client.protocol_version}")
        discover = client.session.discover_result
        check(discover is not None, "auto did not adopt a discovery result")
        # 'private' can only come from this server, so the probe really ran.
        check(discover.cache_scope == "private", "auto did not perform a wire discovery")
        check(discover.capabilities.tools is not None, "auto discovery lacks the tools capability")

        scan = await client.call_tool("cribra_scan", {"source": "hello"})
        check(scan.is_error is False, "auto scan")
        assert_schema("cribra_scan", scan)

        return {"protocolVersion": client.protocol_version, "legacyFallbackExercised": False}


async def legacy_handshake_and_tool_errors() -> dict:
    async with Client(server_params(), mode="legacy") as client:
        check(client.protocol_version == LEGACY, f"negotiated {client.protocol_version}")
        check(client.server_info is not None and client.server_info.name == "cribra-mcp", "serverInfo")

        scan = await client.call_tool("cribra_scan", {"source": "hello"})
        check(scan.is_error is False, "legacy scan")
        assert_schema("cribra_scan", scan)

        transform = await client.call_tool("cribra_transform", {"source": "hello", "mode": "redact"})
        check(transform.is_error is False, "legacy transform")
        assert_schema("cribra_transform", transform)

        invalid = await client.call_tool("cribra_scan", {"source": "x", "unexpectedField": True})
        check(invalid.is_error is True, "invalid arguments must be a tool error, not a JSON-RPC error")
        structured = assert_schema("cribra_scan", invalid)
        check(structured["error"]["code"] == "invalid_arguments", f"error code {structured['error']['code']}")

        unsupported = await client.call_tool(
            "cribra_transform", {"source": "x", "mode": "synthesize"}
        )
        check(unsupported.is_error is True, "unsupported mode must be a tool error")
        structured = assert_schema("cribra_transform", unsupported)
        check(structured["error"]["code"] == "unsupported_mode", f"error code {structured['error']['code']}")

        return {"protocolVersion": client.protocol_version, "errorCodes": ["invalid_arguments", "unsupported_mode"]}


async def clean_close_terminates_the_child() -> dict:
    before = child_pids()
    client = Client(server_params(), mode=MODERN)
    async with client:
        during = child_pids() - before
        check(during, "no server child process was observed during the session")
        scan = await client.call_tool("cribra_scan", {"source": "hello"})
        check(scan.is_error is False, "scan before close")

    # Give the transport's shutdown a bounded moment to reap the child, then
    # assert the process is gone rather than asserting an exact instant.
    deadline = time.monotonic() + 5.0
    remaining = during & child_pids()
    while remaining and time.monotonic() < deadline:
        await asyncio.sleep(0.02)
        remaining = during & child_pids()
    check(not remaining, f"server child processes survived client close: {sorted(remaining)}")
    return {"childPids": sorted(during), "terminated": True}


async def main() -> int:
    if not Path(BIN).is_file():
        print(f"error: server binary not found at {BIN}", file=sys.stderr)
        return 2

    results = []
    for name, body in (
        ("pinned-modern-client-does-not-probe", pinned_modern_client_does_not_probe),
        ("explicit-discovery-over-the-wire", explicit_discovery_over_the_wire),
        ("auto-negotiation-selects-modern", auto_negotiation_selects_modern),
        ("legacy-handshake-and-tool-errors", legacy_handshake_and_tool_errors),
        ("clean-close-terminates-the-child", clean_close_terminates_the_child),
    ):
        results.append(await scenario(name, body))

    failures = [r for r in results if r["status"] != "pass"]
    summary = {
        "client": "mcp",
        "version": "2.3.0",
        "python": sys.version.split()[0],
        "binary": BIN,
        "scenarios": results,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
