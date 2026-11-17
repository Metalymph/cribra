#!/usr/bin/env python3
"""Packaged-artifact smoke qualification for cribra-mcp.

This harness deliberately re-implements archive inspection instead of importing
the packaging script: an independent verifier cannot inherit a builder bug.

It extracts a release archive into a clean temporary directory, launches the
extracted executable as a real MCP stdio server, and exercises the shipping
surface end to end:

* ``--version`` / ``--help`` / rejected arguments;
* modern ``2026-07-28`` discovery, ``tools/list``, scan, transform, privacy-safe
  error, and clean EOF shutdown;
* legacy ``2025-11-25`` initialize, ``tools/list``, scan, and clean shutdown;
* closed-stdout broken-pipe shutdown.

Every wait is bounded. There are no sleeps and no network access.

Usage:
    smoke.py --dir <distribution dir> [--require-all] [--expected-version <v>]
    smoke.py --archive <archive> [--archive <archive> ...]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import queue
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import zipfile
from pathlib import Path
from typing import NoReturn

MODERN = "2026-07-28"
LEGACY = "2025-11-25"
LINE_TIMEOUT = 10.0
EXIT_TIMEOUT = 15.0

# Synthetic, non-credential probe values.
PROBE_SECRET = "SYNTH_SMOKE_PROBE_7Q4"
PROBE_PREFIX = "prefix-smoke-context"
PROBE_SUFFIX = "suffix-smoke-context"
PROBE_PATTERN = "SYNTH_SMOKE_PROBE_[0-9A-Z]{3}"
PROBE_RULE = "custom.smoke_probe"

TARGETS = {
    "aarch64-apple-darwin": ("macho", "arm64"),
    "x86_64-apple-darwin": ("macho", "x86_64"),
    "x86_64-unknown-linux-gnu": ("elf", "x86_64"),
    "aarch64-unknown-linux-gnu": ("elf", "aarch64"),
    "x86_64-pc-windows-msvc": ("pe", "x86_64"),
}


class Failure(AssertionError):
    pass


def check(condition: object, message: str) -> None:
    if not condition:
        raise Failure(message)


def fail(message: str) -> NoReturn:
    print(f"smoke: {message}", file=sys.stderr)
    raise SystemExit(1)


# ---------------------------------------------------------------------------
# Archive inspection (independent of packaging/package.py)
# ---------------------------------------------------------------------------


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_artifact_name(name: str) -> tuple[str, str, str]:
    match = re.fullmatch(r"cribra-mcp-(?P<version>\d+\.\d+\.\d+)-(?P<target>.+)\.(?P<ext>tar\.gz|zip)", name)
    if not match:
        fail(f"unrecognized artifact name: {name}")
    version, target, extension = match.group("version"), match.group("target"), match.group("ext")
    if target not in TARGETS:
        fail(f"unsupported target in artifact name: {name}")
    expected_extension = "zip" if target.endswith("windows-msvc") else "tar.gz"
    if extension != expected_extension:
        fail(f"wrong extension for {target}: {name}")
    return version, target, extension


def expected_artifact_names(version: str) -> set[str]:
    """The exact archive set a complete release must publish."""
    return {
        f"cribra-mcp-{version}-{target}.{'zip' if target.endswith('windows-msvc') else 'tar.gz'}"
        for target in TARGETS
    }


def require_versions(names: list[str], expected_version: str) -> None:
    """Requires every supplied archive to carry `expected_version`.

    This checks only the archives that are actually supplied. It deliberately
    does not require the full target set: a per-target build is qualified before
    the other targets exist, and distribution completeness is a separate check.
    """
    if not names:
        fail("no archives to verify")
    for name in names:
        version, _, _ = parse_artifact_name(name)
        if version != expected_version:
            fail(f"{name} is version {version}, expected {expected_version}")


def require_complete(names: list[str], expected_version: str | None) -> str:
    """Rejects missing, extra, duplicate, or wrong-version artifacts.

    A release is complete only when it contains exactly one archive for each
    supported target, all built from the same version, and that version matches
    the release tag when one is supplied.
    """
    if not names:
        fail("no archives to verify")
    if len(names) != len(set(names)):
        fail(f"duplicate archive names: {sorted(names)}")

    versions = {parse_artifact_name(name)[0] for name in names}
    if len(versions) != 1:
        fail(f"archives span multiple versions: {sorted(versions)}")
    version = versions.pop()
    if expected_version is not None and version != expected_version:
        fail(f"archives are version {version}, expected {expected_version}")

    expected = expected_artifact_names(version)
    actual = set(names)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        fail(f"incomplete distribution; missing={missing} unexpected={extra}")
    return version


def enforce_distribution(
    names: list[str], require_all: bool, expected_version: str | None
) -> None:
    """Applies the requested distribution policy.

    `--require-all` is the only flag that demands the full five-target set.
    `--expected-version` validates the supplied archives without implying
    completeness, so it can qualify one target at a time.
    """
    if require_all:
        require_complete(names, expected_version)
    elif expected_version is not None:
        require_versions(names, expected_version)


def inspect_archive(archive: Path, directory: str, binary: str) -> None:
    expected = {f"{directory}/{binary}", f"{directory}/LICENSE", f"{directory}/INSTALL.md"}
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            entries = [(info.filename, (info.external_attr >> 16) & 0o777) for info in bundle.infolist()]
            payload = bundle.read(f"{directory}/{binary}")
    else:
        with tarfile.open(archive, "r:gz") as bundle:
            entries = [(member.name, member.mode) for member in bundle.getmembers()]
            handle = bundle.extractfile(f"{directory}/{binary}")
            check(handle is not None, "binary entry is not a regular file")
            payload = handle.read()

    names = [name for name, _ in entries]
    check(len(names) == len(set(names)), f"{archive.name}: duplicate entries")
    check(set(names) == expected, f"{archive.name}: unexpected entries {sorted(names)}")
    for name in names:
        check(not name.startswith("/"), f"{archive.name}: absolute entry {name}")
        check(".." not in name.split("/"), f"{archive.name}: traversal entry {name}")

    modes = dict(entries)
    check(modes[f"{directory}/{binary}"] & 0o111 != 0, f"{archive.name}: binary is not executable")
    check(len(payload) > 4096, f"{archive.name}: binary looks truncated")


def detect_architecture(data: bytes) -> tuple[str, str]:
    check(len(data) >= 32, "executable header is truncated")
    if data[:2] == b"MZ":
        offset = struct.unpack_from("<I", data, 0x3C)[0]
        check(data[offset : offset + 4] == b"PE\0\0", "PE signature missing")
        return "pe", {0x8664: "x86_64", 0xAA64: "arm64"}[struct.unpack_from("<H", data, offset + 4)[0]]
    if data[:4] == b"\x7fELF":
        check(data[4] == 2 and data[5] == 1, "expected little-endian 64-bit ELF")
        return "elf", {0x3E: "x86_64", 0xB7: "aarch64"}[struct.unpack_from("<H", data, 18)[0]]
    magic = struct.unpack_from("<I", data, 0)[0]
    check(magic in (0xFEEDFACF, 0xFEEDFACE), "unrecognized executable format")
    return "macho", {0x0100000C: "arm64", 0x01000007: "x86_64"}[struct.unpack_from("<I", data, 4)[0]]


def verify_manifest(directory: Path) -> dict[str, str]:
    manifest = directory / "SHA256SUMS"
    if not manifest.is_file():
        return {}
    digests: dict[str, str] = {}
    for line in manifest.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        digest, name = line.split("  ", 1)
        check(
            name not in ("", ".", "..") and "/" not in name and "\\" not in name,
            f"SHA256SUMS contains an unsafe artifact name: {name!r}",
        )
        check(name not in digests, f"SHA256SUMS contains a duplicate entry: {name}")
        path = directory / name
        check(path.is_file(), f"SHA256SUMS names a missing artifact: {name}")
        check(sha256_file(path) == digest, f"checksum mismatch for {name}")
        digests[name] = digest
    return digests


# ---------------------------------------------------------------------------
# MCP stdio client
# ---------------------------------------------------------------------------


class McpServer:
    def __init__(self, executable: Path, label: str) -> None:
        self.label = label
        self.process = subprocess.Popen(
            [str(executable)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=str(executable.parent),
        )
        self.lines: queue.Queue[bytes | None] = queue.Queue()
        self.reader = threading.Thread(target=self._read_lines, daemon=True)
        self.reader.start()
        self.next_id = 1
        self.seen: list[dict] = []

    def _read_lines(self) -> None:
        assert self.process.stdout is not None
        for raw in self.process.stdout:
            self.lines.put(raw)
        self.lines.put(None)

    def send(self, payload: dict) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(payload, separators=(",", ":")).encode() + b"\n")
        self.process.stdin.flush()

    def request(self, method: str, params: dict) -> dict:
        request_id = self.next_id
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        return self.response(request_id)

    def notify(self, method: str, params: dict | None = None) -> None:
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self.send(message)

    def response(self, request_id: int) -> dict:
        while True:
            line = self.next_line()
            value = json.loads(line)
            check(isinstance(value, dict), f"{self.label}: response is not a JSON object")
            check(value.get("jsonrpc") == "2.0", f"{self.label}: missing jsonrpc version")
            self.seen.append(value)
            if value.get("id") == request_id:
                return value

    def next_line(self) -> str:
        try:
            raw = self.lines.get(timeout=LINE_TIMEOUT)
        except queue.Empty:
            raise Failure(f"{self.label}: no response within {LINE_TIMEOUT}s") from None
        check(raw is not None, f"{self.label}: server closed stdout before responding")
        text = raw.decode("utf-8")
        check(text.startswith("{"), f"{self.label}: unexpected non-JSON stdout line: {text[:80]!r}")
        return text

    def finish(self) -> tuple[int, str]:
        assert self.process.stdin is not None
        self.process.stdin.close()
        try:
            code = self.process.wait(timeout=EXIT_TIMEOUT)
        except subprocess.TimeoutExpired:
            self.process.kill()
            raise Failure(f"{self.label}: server did not exit within {EXIT_TIMEOUT}s") from None
        assert self.process.stderr is not None
        return code, self.process.stderr.read().decode("utf-8", "replace")


def modern_meta() -> dict:
    return {
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
    }


def tool_call(name: str, arguments: dict) -> dict:
    return {"_meta": modern_meta(), "name": name, "arguments": arguments}


def probe_arguments() -> dict:
    return {
        "source": f"{PROBE_PREFIX} {PROBE_SECRET} {PROBE_SUFFIX}",
        "rules": [{"id": PROBE_RULE, "pattern": PROBE_PATTERN, "severity": "high"}],
    }


def assert_no_probe_leak(payload: object, what: str) -> None:
    encoded = json.dumps(payload)
    for canary in (PROBE_SECRET, PROBE_PREFIX, PROBE_SUFFIX, PROBE_PATTERN):
        check(canary not in encoded, f"{what} leaked {canary!r}")


def scenario_version(executable: Path, version: str) -> None:
    result = subprocess.run(
        [str(executable), "--version"], capture_output=True, timeout=LINE_TIMEOUT, check=False
    )
    check(result.returncode == 0, f"--version exited {result.returncode}")
    check(
        result.stdout.decode() == f"cribra-mcp {version}\n",
        f"--version printed {result.stdout!r}",
    )
    check(result.stderr == b"", f"--version wrote to stderr: {result.stderr!r}")


def scenario_help(executable: Path) -> None:
    result = subprocess.run(
        [str(executable), "--help"], capture_output=True, timeout=LINE_TIMEOUT, check=False
    )
    check(result.returncode == 0, f"--help exited {result.returncode}")
    text = result.stdout.decode()
    check("--version" in text and "--help" in text, "--help omits its own flags")
    check(result.stderr == b"", f"--help wrote to stderr: {result.stderr!r}")


def scenario_rejected_argument(executable: Path) -> None:
    result = subprocess.run(
        [str(executable), "--not-a-flag"], capture_output=True, timeout=LINE_TIMEOUT, check=False
    )
    check(result.returncode == 2, f"unknown flag exited {result.returncode}")
    check(result.stdout == b"", "unknown flag wrote to stdout")


def scenario_modern(executable: Path) -> None:
    server = McpServer(executable, "modern")
    try:
        discovered = server.request("server/discover", {"_meta": modern_meta()})
        result = discovered["result"]
        check(result.get("resultType") == "complete", "discovery is not complete")
        check(result.get("supportedVersions") == [MODERN], f"unexpected versions: {result}")
        check(result.get("cacheScope") == "private", "discovery cacheScope is not private")
        check(result.get("ttlMs") == 0, "discovery ttlMs is not 0")

        listed = server.request("tools/list", {"_meta": modern_meta()})["result"]
        check(listed.get("resultType") == "complete", "tools/list is not complete")
        names = sorted(tool["name"] for tool in listed["tools"])
        check(names == ["cribra_scan", "cribra_transform"], f"advertised tools: {names}")

        scanned = server.request("tools/call", tool_call("cribra_scan", probe_arguments()))["result"]
        check(scanned.get("isError") is False, "scan reported an error")
        findings = scanned["structuredContent"]["findings"]
        check(len(findings) == 1 and findings[0]["ruleId"] == PROBE_RULE, f"unexpected findings: {findings}")
        assert_no_probe_leak(scanned, "scan result")

        transformed = server.request(
            "tools/call", tool_call("cribra_transform", {**probe_arguments(), "mode": "redact"})
        )["result"]
        check(transformed.get("isError") is False, "transform reported an error")
        structured = transformed["structuredContent"]
        check(
            structured["output"] == f"{PROBE_PREFIX} [REDACTED] {PROBE_SUFFIX}",
            f"unexpected transform output: {structured['output']!r}",
        )
        check(transformed["content"][0]["text"] == structured["output"], "text block disagrees with output")

        rejected = server.request(
            "tools/call",
            tool_call(
                "cribra_scan",
                {"source": f"{PROBE_PREFIX} {PROBE_SECRET}", "path": PROBE_PREFIX},
            ),
        )["result"]
        check(rejected.get("isError") is True, "unknown field was accepted")
        assert_no_probe_leak(rejected, "invalid_arguments error")

        code, stderr = server.finish()
        check(code == 0, f"modern session exited {code}")
        check(stderr == "", f"modern session wrote to stderr: {stderr!r}")
    finally:
        if server.process.poll() is None:
            server.process.kill()


def scenario_legacy(executable: Path) -> None:
    server = McpServer(executable, "legacy")
    try:
        initialized = server.request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "cribra-packaged-smoke", "version": "1"},
            },
        )["result"]
        check(
            initialized.get("protocolVersion") == LEGACY,
            f"legacy negotiation returned {initialized.get('protocolVersion')!r}",
        )
        server.notify("notifications/initialized")

        listed = server.request("tools/list", {})["result"]
        names = sorted(tool["name"] for tool in listed["tools"])
        check(names == ["cribra_scan", "cribra_transform"], f"advertised tools: {names}")

        scanned = server.request(
            "tools/call", {"name": "cribra_scan", "arguments": probe_arguments()}
        )["result"]
        check(scanned.get("isError") is False, "legacy scan reported an error")
        check(scanned["structuredContent"]["findings"], "legacy scan returned no findings")
        assert_no_probe_leak(scanned, "legacy scan result")

        code, stderr = server.finish()
        check(code == 0, f"legacy session exited {code}")
        check(stderr == "", f"legacy session wrote to stderr: {stderr!r}")
    finally:
        if server.process.poll() is None:
            server.process.kill()


def scenario_broken_pipe(executable: Path) -> None:
    process = subprocess.Popen(
        [str(executable)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        assert process.stdout is not None and process.stdin is not None
        process.stdout.close()
        request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {"_meta": modern_meta()},
        }
        try:
            process.stdin.write(json.dumps(request).encode() + b"\n")
            process.stdin.flush()
            process.stdin.close()
        except (BrokenPipeError, OSError):
            pass
        try:
            code = process.wait(timeout=EXIT_TIMEOUT)
        except subprocess.TimeoutExpired:
            process.kill()
            raise Failure("closed stdout did not terminate the server") from None
        check(code == 0, f"closed stdout exited {code}, expected a clean shutdown")
    finally:
        if process.poll() is None:
            process.kill()


SCENARIOS = (
    ("version", scenario_version),
    ("help", scenario_help),
    ("rejected-argument", scenario_rejected_argument),
    ("modern", scenario_modern),
    ("legacy", scenario_legacy),
    ("broken-pipe", scenario_broken_pipe),
)


def host_target() -> str | None:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin":
        return "aarch64-apple-darwin" if machine == "arm64" else "x86_64-apple-darwin"
    if system == "Linux":
        return (
            "aarch64-unknown-linux-gnu"
            if machine in ("aarch64", "arm64")
            else "x86_64-unknown-linux-gnu"
        )
    if system == "Windows":
        return "x86_64-pc-windows-msvc"
    return None


def extract(archive: Path, destination: Path, version: str, target: str) -> Path:
    directory = f"cribra-mcp-{version}-{target}"
    binary = "cribra-mcp.exe" if target.endswith("windows-msvc") else "cribra-mcp"
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(destination)
    else:
        with tarfile.open(archive, "r:gz") as bundle:
            try:
                # Rejects unsafe members a second time, independently of the
                # explicit allow-list check already performed above.
                bundle.extractall(destination, filter="data")
            except TypeError:  # Python versions before the filter argument.
                bundle.extractall(destination)
    executable = destination / directory / binary
    check(executable.is_file(), f"extracted binary missing: {executable}")
    if os.name != "nt":
        check(os.access(executable, os.X_OK), "extracted binary is not executable")
    return executable


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--dir", help="directory containing archives and SHA256SUMS")
    source.add_argument("--archive", action="append", help="archive to verify; repeatable")
    parser.add_argument("--run", choices=("host", "none"), default="host")
    parser.add_argument(
        "--require-all",
        action="store_true",
        help="require exactly one archive for every supported target",
    )
    parser.add_argument(
        "--expected-version",
        help="require every archive to carry this version",
    )
    args = parser.parse_args()

    if args.dir:
        directory = Path(args.dir)
        check(directory.is_dir(), f"missing directory: {directory}")
        archives = sorted(
            path
            for path in directory.iterdir()
            if path.name.startswith("cribra-mcp-") and path.name.endswith((".tar.gz", ".zip"))
        )
        digests = verify_manifest(directory)
        check(bool(digests), "SHA256SUMS is missing or empty")
        check(
            {path.name for path in archives} == set(digests),
            "SHA256SUMS does not cover exactly the published archives",
        )
    else:
        archives = [Path(item) for item in args.archive]

    check(bool(archives), "no archives to verify")

    enforce_distribution(
        [path.name for path in archives], args.require_all, args.expected_version
    )

    host = host_target()
    executed = False
    for archive in archives:
        version, target, _ = parse_artifact_name(archive.name)
        directory = f"cribra-mcp-{version}-{target}"
        binary = "cribra-mcp.exe" if target.endswith("windows-msvc") else "cribra-mcp"
        inspect_archive(archive, directory, binary)

        if target != host or args.run == "none":
            print(f"VERIFY {archive.name} ({target}; not executable on this host)")
            continue

        workdir = Path(tempfile.mkdtemp(prefix="cribra-mcp-smoke-"))
        try:
            executable = extract(archive, workdir, version, target)
            for name, scenario in SCENARIOS:
                started = time.monotonic()
                try:
                    if name == "version":
                        scenario(executable, version)
                    else:
                        scenario(executable)
                except Failure as error:
                    print(f"FAIL {archive.name} {name}: {error}", file=sys.stderr)
                    return 1
                elapsed = round((time.monotonic() - started) * 1000)
                print(f"PASS {archive.name} {name} ({elapsed} ms)")
            executed = True
        finally:
            shutil.rmtree(workdir, ignore_errors=True)

    if not executed:
        print(
            f"smoke: no archive matches this host ({host}); verification only",
            file=sys.stderr,
        )
        return 0

    print(f"packaged smoke qualification passed for {host}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Failure as error:
        print(f"FAIL {error}", file=sys.stderr)
        sys.exit(1)
