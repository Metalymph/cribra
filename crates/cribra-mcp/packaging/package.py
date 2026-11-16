#!/usr/bin/env python3
"""Build and verify cribra-mcp native release archives.

Dependency-free: standard library only. Archives are written with normalized
metadata so repeated builds of identical inputs produce identical bytes.

Artifact names are ``cribra-mcp-<version>-<target>.tar.gz`` for macOS and Linux
and ``cribra-mcp-<version>-<target>.zip`` for Windows. The release tag keeps its
own ``cribra-mcp-v<version>`` prefix; artifacts deliberately do not repeat the
tag's ``v`` so the version string always matches Cargo metadata.

Commands:
    package.py build --target <triple> --binary <path> --outdir <dir>
    package.py verify --archive <path>
    package.py checksums --outdir <dir>
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import shutil
import struct
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path
from typing import NoReturn

REPO = Path(__file__).resolve().parents[3]
CRATE = REPO / "crates" / "cribra-mcp"
LICENSE = REPO / "LICENSE"
INSTALL_TEMPLATE = CRATE / "packaging" / "INSTALL.md"

# Fixed archive metadata: 1980-01-01T00:00:00Z is the earliest value that both
# tar and zip represent portably.
EPOCH = 315532800

TARGETS = {
    "aarch64-apple-darwin": ("macho", "arm64"),
    "x86_64-apple-darwin": ("macho", "x86_64"),
    "x86_64-unknown-linux-gnu": ("elf", "x86_64"),
    "aarch64-unknown-linux-gnu": ("elf", "aarch64"),
    "x86_64-pc-windows-msvc": ("pe", "x86_64"),
}

PLATFORM_NOTES = {
    "darwin": (
        "macOS note: the executable is ad-hoc signed but not notarized. If "
        "Gatekeeper quarantines a browser download, clear the attribute with\n"
        "`xattr -d com.apple.quarantine {binary}` before configuring the host."
    ),
    "linux": (
        "Linux note: this build targets glibc 2.35 or newer (Ubuntu 22.04 or "
        "equivalent). It does not use musl or any bundled libc."
    ),
    "windows": (
        "Windows note: the executable is unsigned, so SmartScreen may prompt "
        "once. Verify the SHA-256 digest before running it."
    ),
}


def fail(message: str) -> NoReturn:
    print(f"package: {message}", file=sys.stderr)
    raise SystemExit(1)


def crate_version() -> str:
    import tomllib

    with (CRATE / "Cargo.toml").open("rb") as handle:
        return tomllib.load(handle)["package"]["version"]


def platform_of(target: str) -> str:
    if target.endswith("apple-darwin"):
        return "darwin"
    if target.endswith("windows-msvc"):
        return "windows"
    return "linux"


def binary_name(target: str) -> str:
    return "cribra-mcp.exe" if platform_of(target) == "windows" else "cribra-mcp"


def artifact_name(version: str, target: str) -> str:
    extension = "zip" if platform_of(target) == "windows" else "tar.gz"
    return f"cribra-mcp-{version}-{target}.{extension}"


def directory_name(version: str, target: str) -> str:
    return f"cribra-mcp-{version}-{target}"


def detect_binary_format(data: bytes) -> tuple[str, str]:
    """Returns ``(format, architecture)`` for a native executable header."""
    if len(data) < 32:
        fail("truncated executable header")
    if data[:2] == b"MZ":
        if len(data) < 0x40:
            fail("truncated PE header")
        offset = struct.unpack_from("<I", data, 0x3C)[0]
        if data[offset : offset + 4] != b"PE\0\0":
            fail("PE signature missing")
        machine = struct.unpack_from("<H", data, offset + 4)[0]
        architecture = {0x8664: "x86_64", 0xAA64: "arm64"}.get(machine)
        if architecture is None:
            fail(f"unsupported PE machine 0x{machine:04x}")
        return "pe", architecture

    if data[:4] == b"\x7fELF":
        if len(data) < 20:
            fail("truncated ELF header")
        if data[4] != 2:
            fail("expected a 64-bit ELF executable")
        if data[5] != 1:
            fail("expected a little-endian ELF executable")
        machine = struct.unpack_from("<H", data, 18)[0]
        architecture = {0x3E: "x86_64", 0xB7: "aarch64"}.get(machine)
        if architecture is None:
            fail(f"unsupported ELF machine 0x{machine:04x}")
        return "elf", architecture

    magic = struct.unpack_from("<I", data, 0)[0] if len(data) >= 4 else 0
    if magic in (0xFEEDFACF, 0xFEEDFACE):
        architecture = {0x0100000C: "arm64", 0x01000007: "x86_64"}.get(
            struct.unpack_from("<I", data, 4)[0]
        )
        if architecture is None:
            fail("unsupported Mach-O CPU type")
        return "macho", architecture

    if magic in (0xCAFEBABE, 0xCAFEBABF):
        fail("universal Mach-O binaries are not produced by this pipeline")

    fail("unrecognized executable format")


def render_install(version: str, target: str, archive: str) -> str:
    binary = binary_name(target)
    platform = platform_of(target)
    absolute = (
        rf"C:\Tools\{directory_name(version, target)}\{binary}"
        if platform == "windows"
        else f"/opt/{directory_name(version, target)}/{binary}"
    )
    replacements = {
        "{{version}}": version,
        "{{target}}": target,
        "{{archive}}": archive,
        "{{directory}}": directory_name(version, target),
        "{{binary}}": binary,
        "{{absolute}}": absolute,
        "{{platform_note}}": PLATFORM_NOTES[platform].format(binary=binary),
    }
    text = INSTALL_TEMPLATE.read_text(encoding="utf-8")
    for placeholder, value in replacements.items():
        text = text.replace(placeholder, value)
    return text


def write_tar_gz(path: Path, entries: list[tuple[str, bytes, int]]) -> None:
    with path.open("wb") as raw:
        # A fixed mtime keeps the gzip header deterministic.
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=EPOCH) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT) as archive:
                for name, payload, mode in entries:
                    info = tarfile.TarInfo(name)
                    info.size = len(payload)
                    info.mode = mode
                    info.mtime = EPOCH
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.type = tarfile.REGTYPE
                    archive.addfile(info, io.BytesIO(payload))


def write_zip(path: Path, entries: list[tuple[str, bytes, int]]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, payload, mode in entries:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3  # Unix, so the mode bits are meaningful.
            info.external_attr = (mode & 0xFFFF) << 16 | 0o100000
            archive.writestr(info, payload)


def archive_entries(archive: Path) -> list[tuple[str, int]]:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            return [(info.filename, (info.external_attr >> 16) & 0o777) for info in bundle.infolist()]
    with tarfile.open(archive, "r:gz") as bundle:
        return [(member.name, member.mode) for member in bundle.getmembers()]


def read_entry(archive: Path, name: str) -> bytes:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            return bundle.read(name)
    with tarfile.open(archive, "r:gz") as bundle:
        handle = bundle.extractfile(name)
        if handle is None:
            fail(f"entry {name} is not a regular file")
        return handle.read()


def verify_archive(archive: Path, expected_version: str | None = None) -> dict:
    """Checks structure, executable permissions, and binary architecture."""
    if not archive.is_file():
        fail(f"missing archive: {archive}")

    name = archive.name
    suffix = ".zip" if name.endswith(".zip") else ".tar.gz"
    stem = name[: -len(suffix)]
    prefix = "cribra-mcp-"
    if not stem.startswith(prefix):
        fail(f"unexpected archive name: {name}")

    remainder = stem[len(prefix) :]
    targets = sorted(TARGETS, key=len, reverse=True)
    target = next((item for item in targets if remainder.endswith(f"-{item}")), None)
    if target is None:
        fail(f"archive name does not end with a supported target: {name}")
    version = remainder[: -len(f"-{target}")]

    if expected_version is not None and version != expected_version:
        fail(f"archive version {version} does not match expected {expected_version}")
    if (suffix == ".zip") != (platform_of(target) == "windows"):
        fail(f"wrong archive format {suffix} for target {target}")

    directory = directory_name(version, target)
    binary = binary_name(target)
    expected_entries = {
        f"{directory}/{binary}",
        f"{directory}/LICENSE",
        f"{directory}/INSTALL.md",
    }

    entries = archive_entries(archive)
    names = [entry_name for entry_name, _ in entries]
    if set(names) != expected_entries or len(names) != len(expected_entries):
        fail(f"{name}: unexpected entries {sorted(names)}")

    for entry_name in names:
        if entry_name.startswith("/") or ".." in entry_name.split("/"):
            fail(f"{name}: unsafe entry path {entry_name}")

    modes = dict(entries)
    if modes[f"{directory}/{binary}"] & 0o111 == 0:
        fail(f"{name}: {binary} is not marked executable")

    binary_bytes = read_entry(archive, f"{directory}/{binary}")
    if len(binary_bytes) < 4096:
        fail(f"{name}: {binary} looks truncated")

    expected_format, expected_arch = TARGETS[target]
    actual_format, actual_arch = detect_binary_format(binary_bytes)
    if (actual_format, actual_arch) != (expected_format, expected_arch):
        fail(
            f"{name}: binary is {actual_format}/{actual_arch}, "
            f"expected {expected_format}/{expected_arch}"
        )

    license_bytes = read_entry(archive, f"{directory}/LICENSE")
    if b"MIT License" not in license_bytes:
        fail(f"{name}: LICENSE does not look like the repository license")

    install_text = read_entry(archive, f"{directory}/INSTALL.md").decode("utf-8")
    for required in (version, target, absolute_config_value(target, version)):
        if required not in install_text:
            fail(f"{name}: INSTALL.md is missing {required!r}")
    for placeholder in ("{{", "}}"):
        if placeholder in install_text:
            fail(f"{name}: INSTALL.md still contains {placeholder!r}")

    return {
        "archive": name,
        "target": target,
        "version": version,
        "format": actual_format,
        "architecture": actual_arch,
        "bytes": archive.stat().st_size,
        "sha256": sha256_file(archive),
    }


def absolute_config_value(target: str, version: str) -> str:
    binary = binary_name(target)
    directory = directory_name(version, target)
    if platform_of(target) == "windows":
        return rf"C:\Tools\{directory}\{binary}"
    return f"/opt/{directory}/{binary}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_build(args: argparse.Namespace) -> None:
    target = args.target
    if target not in TARGETS:
        fail(f"unsupported target {target}; expected one of {', '.join(TARGETS)}")

    version = crate_version()
    source = Path(args.binary)
    if not source.is_file():
        fail(f"missing binary: {source}")

    expected_format, expected_arch = TARGETS[target]
    actual_format, actual_arch = detect_binary_format(source.read_bytes()[:4096])
    if (actual_format, actual_arch) != (expected_format, expected_arch):
        fail(
            f"{source} is {actual_format}/{actual_arch}, "
            f"expected {expected_format}/{expected_arch} for {target}"
        )

    outdir = Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    archive = outdir / artifact_name(version, target)
    if archive.exists():
        fail(f"refusing to overwrite existing artifact: {archive}")

    directory = directory_name(version, target)
    binary = binary_name(target)
    binary_bytes = source.read_bytes()
    entries = [
        (f"{directory}/{binary}", binary_bytes, 0o755),
        (f"{directory}/LICENSE", LICENSE.read_bytes(), 0o644),
        (
            f"{directory}/INSTALL.md",
            render_install(version, target, archive.name).encode("utf-8"),
            0o644,
        ),
    ]

    staging = Path(tempfile.mkdtemp(prefix="cribra-mcp-package-"))
    try:
        staged = staging / archive.name
        if archive.suffix == ".zip":
            write_zip(staged, entries)
        else:
            write_tar_gz(staged, entries)
        shutil.copyfile(staged, archive)
    finally:
        shutil.rmtree(staging, ignore_errors=True)

    details = verify_archive(archive, expected_version=version)
    print(
        "built {archive} ({target}, {architecture}, {bytes} bytes)\n"
        "sha256 {sha256}".format(**details)
    )


def command_verify(args: argparse.Namespace) -> None:
    details = verify_archive(Path(args.archive))
    print(
        "verified {archive} ({target}, {architecture}, {bytes} bytes)\n"
        "sha256 {sha256}".format(**details)
    )


def command_checksums(args: argparse.Namespace) -> None:
    outdir = Path(args.outdir)
    if not outdir.is_dir():
        fail(f"missing distribution directory: {outdir}")

    archives = sorted(
        path
        for path in outdir.iterdir()
        if path.is_file()
        and path.name.startswith("cribra-mcp-")
        and (path.name.endswith(".tar.gz") or path.name.endswith(".zip"))
    )
    if not archives:
        fail(f"no cribra-mcp archives found in {outdir}")

    lines = []
    for archive in archives:
        details = verify_archive(archive)
        lines.append(f"{details['sha256']}  {details['archive']}")

    manifest = outdir / "SHA256SUMS"
    if manifest.exists():
        fail(f"refusing to overwrite existing manifest: {manifest}")
    manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(manifest.read_text(encoding="utf-8"), end="")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)

    build = subcommands.add_parser("build", help="build one target archive")
    build.add_argument("--target", required=True, choices=sorted(TARGETS))
    build.add_argument("--binary", required=True)
    build.add_argument("--outdir", required=True)
    build.set_defaults(handler=command_build)

    verify = subcommands.add_parser("verify", help="verify one existing archive")
    verify.add_argument("--archive", required=True)
    verify.set_defaults(handler=command_verify)

    checksums = subcommands.add_parser("checksums", help="write SHA256SUMS for a directory")
    checksums.add_argument("--outdir", required=True)
    checksums.set_defaults(handler=command_checksums)

    args = parser.parse_args()
    args.handler(args)


if __name__ == "__main__":
    main()
