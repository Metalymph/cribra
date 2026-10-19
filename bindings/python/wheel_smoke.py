"""Inspect one local wheel and qualify it across a matrix of isolated interpreters."""

import argparse
import base64
import csv
from email.parser import BytesParser
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zipfile

# The builder owns the single PE32+ parser and the native-library name set; the
# smoke reuses them so the two tools can never disagree about what is native.
import build_wheel


# Local pyenv interpreters qualified by NB-2.5B.3. Distribution requires-python is
# `>=3.11`, so every interpreter must install the wheel through the normal offline
# pip path; there are no compatibility probes or metadata bypasses.
DEFAULT_INTERPRETERS = ("3.11.17", "3.12.15", "3.13.7", "3.14.8")

MACOS_LIBRARY = "libcribra_uniffi.dylib"
LINUX_LIBRARY = "libcribra_uniffi.so"
WINDOWS_LIBRARY = "cribra_uniffi.dll"
# The native Linux builder keeps the local, non-portable tag: no manylinux/musllinux
# claim. The auditwheel-qualified policy in CRIBRA_MANYLINUX_PLATFORM (build_wheel.py)
# is the only additional tag accepted, and only at the exact verified version.
LINUX_WHEEL_PLATFORM = "linux_x86_64"
MANYLINUX_WHEEL_PLATFORM = "manylinux_2_28_x86_64"
# Native Windows accepts only the AMD64 tag; like the Linux/macOS tags above it is
# one exact policy, so win_arm64, win32 and any CPython ABI tag fail closed.
WINDOWS_WHEEL_PLATFORM = "win_amd64"
# Exact, verified Linux platform tags. Nothing else is accepted by the qualification.
LINUX_WHEEL_PLATFORMS = (LINUX_WHEEL_PLATFORM, MANYLINUX_WHEEL_PLATFORM)
# Shared libraries expected in a release cdylib linked against glibc. Any other
# dependency, and any repository-local dependency, fails the qualification closed.
LINUX_ALLOWED_LIBRARIES = frozenset({
    "libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1",
    "libgcc_s.so.1", "libunwind.so.8", "ld-linux-x86-64.so.2",
})


class QualificationError(RuntimeError):
    """A wheel, interpreter or installed consumer failed qualification."""


def resolve_interpreter(spec):
    """Map a pyenv version name or an explicit interpreter path to an executable."""
    if "/" in spec or os.sep in spec or Path(spec).is_file():
        executable = Path(spec).resolve()
        if not executable.is_file():
            raise QualificationError(f"interpreter not found: {spec}")
        return executable
    try:
        prefix = subprocess.check_output(["pyenv", "prefix", spec], text=True,
                                         stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise QualificationError(f"pyenv interpreter unavailable: {spec}") from error
    for name in ("python3", "python"):
        executable = Path(prefix) / "bin" / name
        if executable.is_file():
            return executable
    raise QualificationError(f"no interpreter executable for pyenv version: {spec}")


def interpreter_version(executable):
    output = subprocess.check_output(
        [str(executable), "-c", "import sys; print('%d.%d.%d' % sys.version_info[:3])"], text=True)
    return tuple(int(part) for part in output.strip().split("."))


def qualified_library(tag, host):
    """Return the native library name for a strictly qualified wheel tag.

    Only the exact, verified tags pass: on macOS a `py3-none-macosx_*_arm64` tag;
    on Linux `py3-none-linux_x86_64` or the auditwheel-confirmed
    `py3-none-manylinux_2_28_x86_64`; on Windows exactly `py3-none-win_amd64`.
    Every other platform, architecture and ABI (a CPython extension ABI, another
    manylinux/musllinux version, ARM64 Linux, Windows ARM64/32-bit, ...) fails
    closed before the payload is inspected.
    """
    if host == "darwin":
        if not (tag.startswith("py3-none-macosx_") and tag.endswith("_arm64")):
            raise QualificationError(f"unexpected macOS wheel tag: {tag!r}")
        return MACOS_LIBRARY
    if host.startswith("linux"):
        allowed = frozenset("py3-none-" + platform for platform in LINUX_WHEEL_PLATFORMS)
        if tag not in allowed:
            raise QualificationError(f"unexpected Linux wheel tag: {tag!r}")
        return LINUX_LIBRARY
    if host == "win32":
        if tag != "py3-none-" + WINDOWS_WHEEL_PLATFORM:
            raise QualificationError(f"unexpected Windows wheel tag: {tag!r}")
        return WINDOWS_LIBRARY
    raise QualificationError(f"unsupported host platform: {host}")


def inspect_wheel(wheel):
    """Check wheel metadata/RECORD/platform/native payload; return payload and name."""
    build = json.loads((wheel.parent.parent / "build-info.json").read_text())
    generated = Path(build["generatedModule"])
    native_library = Path(build["nativeLibrary"])
    with zipfile.ZipFile(wheel) as archive:
        metadata_name = next(name for name in archive.namelist() if name.endswith(".dist-info/METADATA"))
        dist_info = metadata_name.rsplit("/", 1)[0]
        metadata = BytesParser().parsebytes(archive.read(metadata_name))
        tags = BytesParser().parsebytes(archive.read(dist_info + "/WHEEL"))
        assert metadata["Name"] == "cribra" and metadata["Version"].endswith("+fuse")
        assert metadata["Requires-Python"] == ">=3.11"
        assert not metadata.get_all("Requires-Dist")
        assert tags["Root-Is-Purelib"] == "false"
        tag = tags["Tag"]
        # The local `linux_x86_64` tag and the exact auditwheel-confirmed manylinux
        # policy are the only Linux tags accepted, so an unverified manylinux/
        # musllinux tag can never slip through, and every ABI other than `none`
        # stays rejected.
        library_name = qualified_library(tag, sys.platform)
        assert wheel.name.endswith(tag + ".whl")
        # Fail closed unless the wheel packages exactly the host platform's native
        # library: a missing or foreign-platform library would otherwise surface as
        # an opaque zip KeyError, or load the wrong binary. The builder owns the set
        # of recognised native-library names, so both tools apply the same rule.
        expected = "cribra_uniffi/" + library_name
        packaged = build_wheel.packaged_native_libraries(archive)
        assert packaged == [expected], (
            f"expected exactly one native library {expected} in {wheel.name}, found {packaged}")
        native = archive.read(expected)
        assert archive.read("cribra_uniffi/__init__.py") == generated.read_bytes()
        assert native == native_library.read_bytes()
        assert any(name.endswith("/LICENSE") for name in archive.namelist())
        # Check every archive file against standard wheel RECORD hashes/sizes.
        # RECORD lists files, not directories; auditwheel's repaired wheel adds
        # explicit directory entries, so compare against the file members.
        records = list(csv.reader(io.StringIO(archive.read(dist_info + "/RECORD").decode())))
        files = {name for name in archive.namelist() if not name.endswith("/")}
        assert {row[0] for row in records} == files
        for name, digest, size in records:
            if name.endswith("/RECORD"):
                continue
            payload = archive.read(name)
            expected = base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=").decode()
            assert digest == "sha256=" + expected and int(size) == len(payload)
    return native, library_name


def verify_macos_native(native, library):
    """Fail closed unless the dylib is ARM64 and links only against system libraries."""
    library.write_bytes(native)
    assert subprocess.check_output(["/usr/bin/lipo", "-archs", str(library)], text=True).strip() == "arm64"
    subprocess.run(["/usr/bin/file", str(library)], check=True)
    subprocess.run(["/usr/bin/xcrun", "vtool", "-show-build", str(library)], check=True)
    dependencies = subprocess.check_output(["/usr/bin/otool", "-L", str(library)], text=True)
    print(dependencies)
    for line in dependencies.splitlines()[2:]:  # Skip filename and LC_ID_DYLIB.
        assert line.strip().startswith(("/usr/lib/", "/System/Library/")), line


def require_tool(name):
    """Resolve an external verification tool or fail closed."""
    path = shutil.which(name)
    if path is None:
        raise QualificationError(f"required tool is not installed: {name}")
    return path


def check_linux_elf_header(native, library):
    """Fail closed unless the payload is a 64-bit little-endian x86_64 ET_DYN object."""
    if len(native) < 20 or native[:4] != b"\x7fELF" or native[4] != 2 or native[5] != 1:
        raise QualificationError(f"{library} is not a 64-bit little-endian ELF object.")
    e_type, e_machine = struct.unpack_from("<HH", native, 16)
    if e_type != 3 or e_machine != 0x3E:  # ET_DYN (shared object), EM_X86_64
        raise QualificationError(
            f"{library} is not an x86_64 ELF shared object "
            f"(type={e_type}, machine=0x{e_machine:x}).")


def dynamic_entries(readelf_output, key):
    """Return the bracketed values of every `(KEY)` row in `readelf -d` output."""
    marker = f"({key})"
    values = []
    for line in readelf_output.splitlines():
        if marker in line and "[" in line and "]" in line:
            values.append(line.split("[", 1)[1].rsplit("]", 1)[0])
    return values


def check_ldd_dependencies(listing, needed, library, repository):
    """Fail closed unless every NEEDED entry resolves to an allowed system path.

    `ldd` prints most libraries as `name => /absolute/path (0x...)`, but the
    dynamic loader itself without `=>`: `/lib64/ld-linux-x86-64.so.2 (0x...)`.
    Each resolved file is matched, by exact basename, against the DT_NEEDED set.
    Kernel-provided entries such as `linux-vdso.so.1` carry no file path.
    """
    resolved = set()
    for line in listing.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        _, separator, target = stripped.partition("=>")
        if separator:
            target = target.strip()
            if not target:
                raise QualificationError(f"malformed ldd entry: {stripped}")
            if target == "not found":
                raise QualificationError(f"unresolved shared-library dependency: {stripped}")
            path = target.split(" ", 1)[0]
        else:
            path = stripped.split(" ", 1)[0]
            if not path.startswith("/"):
                # Informational bare names (for example `linux-vdso.so.1`) are not
                # files; a NEEDED entry listed this way is unexpected output.
                if path in needed:
                    raise QualificationError(f"malformed ldd entry for dependency {path!r}: {stripped}")
                continue
        if not path.startswith("/"):
            raise QualificationError(f"malformed ldd entry: {stripped}")
        resolved_path = Path(path).resolve()
        if resolved_path.is_relative_to(repository):
            raise QualificationError(f"{library} loads a repository-local dependency: {path}")
        if not any(resolved_path.is_relative_to(prefix) for prefix in ("/lib", "/lib64", "/usr")):
            raise QualificationError(f"{library} loads a dependency outside system directories: {path}")
        resolved.add(Path(path).name)
    missing = sorted(set(needed) - resolved)
    if missing:
        raise QualificationError(f"{library} dependencies did not resolve: {missing}")
    return resolved


def verify_linux_native(native, library, root):
    """Fail closed unless the `.so` is an x86_64 ELF linking only to system libraries."""
    check_linux_elf_header(native, library)
    library.write_bytes(native)
    subprocess.run([require_tool("file"), str(library)], check=True)
    dynamic = subprocess.check_output([require_tool("readelf"), "-d", str(library)], text=True)
    print(dynamic, end="")
    needed = dynamic_entries(dynamic, "NEEDED")
    if not needed:
        raise QualificationError(f"{library} declares no shared-library dependencies.")
    for name in needed:
        if "/" in name:
            raise QualificationError(f"{library} has a path-like NEEDED entry: {name!r}.")
    unexpected = sorted(set(needed) - LINUX_ALLOWED_LIBRARIES)
    if unexpected:
        raise QualificationError(f"{library} has unexpected shared-library dependencies: {unexpected}.")
    repository = root.resolve()
    search_paths = dynamic_entries(dynamic, "RPATH") + dynamic_entries(dynamic, "RUNPATH")
    for entry in search_paths:
        for directory in entry.split(":"):
            expanded = directory.replace("$ORIGIN", str(library.parent))
            if expanded and Path(expanded).resolve().is_relative_to(repository):
                raise QualificationError(f"{library} searches the repository at runtime: {entry!r}.")
    listing = subprocess.run([require_tool("ldd"), str(library)], text=True, capture_output=True)
    print(listing.stdout, end="")
    print(listing.stderr, end="")
    if listing.returncode != 0:
        raise QualificationError(f"ldd failed for {library}: {listing.stderr.strip()}")
    check_ldd_dependencies(listing.stdout, needed, library, repository)
    print(f"Verified {library}: ELF64 ET_DYN x86_64, dependencies {sorted(set(needed))}.")


def check_windows_pe_header(native, library):
    """Fail closed unless the payload is a 64-bit x86_64 PE DLL.

    The PE32+ / machine / DLL rules come from the builder's single parser
    (`build_wheel.pe_amd64_dll_error`); Windows has no `file`/`otool`/`ldd`
    equivalent, so this is the native-dependency boundary on that host.
    """
    error = build_wheel.pe_amd64_dll_error(native)
    if error is not None:
        raise QualificationError(f"{library} {error}.")


def verify_windows_native(native, library):
    """Fail closed unless the installed `.dll` is a 64-bit x86_64 PE dynamic library."""
    check_windows_pe_header(native, library)
    library.write_bytes(native)
    print(f"Verified {library}: PE32+ x86_64 DLL.")


def venv_name(spec):
    """Return a filesystem-safe venv directory name for an interpreter spec."""
    return "venv-" + spec.replace("/", "_").replace("\\", "_").replace(":", "_")


def venv_python(virtual):
    """Return the venv interpreter path: `Scripts/python.exe` on Windows."""
    if sys.platform == "win32":
        return virtual / "Scripts" / "python.exe"
    return virtual / "bin" / "python"


def isolated_environment(external, empty_path):
    """Return a minimal environment with no Rust/Cargo toolchain on PATH.

    Windows additionally needs the system root to start processes and load
    system DLLs, and uses TEMP/TMP and USERPROFILE instead of TMPDIR/HOME.
    """
    if sys.platform == "win32":
        return {"PATH": str(empty_path),
                "SystemRoot": os.environ.get("SystemRoot", "C:\\Windows"),
                "SystemDrive": os.environ.get("SystemDrive", "C:"),
                "TEMP": str(external), "TMP": str(external),
                "USERPROFILE": str(external), "APPDATA": str(external),
                "LOCALAPPDATA": str(external), "PYTHONDONTWRITEBYTECODE": "1"}
    return {"PATH": str(empty_path), "HOME": str(external), "TMPDIR": str(external),
            "PYTHONDONTWRITEBYTECODE": "1", "LC_CTYPE": "UTF-8"}


def qualify_interpreter(spec, wheel, test, root, external, empty_path):
    """Install offline in a fresh venv and run the installed consumer on one interpreter."""
    executable = resolve_interpreter(spec)
    actual = interpreter_version(executable)
    environment = isolated_environment(external, empty_path)
    virtual = external / venv_name(spec)
    subprocess.run([str(executable), "-m", "venv", str(virtual)], env=environment, check=True)
    python = venv_python(virtual)
    install = [str(python), "-I", "-B", "-m", "pip", "--isolated", "--disable-pip-version-check",
               "install", "--no-index", "--no-deps", str(wheel)]
    result = subprocess.run(install, cwd=external, env=environment, text=True, capture_output=True)
    if result.returncode != 0:
        raise QualificationError(
            f"offline install failed for Python {'.'.join(map(str, actual))}:\n"
            f"{(result.stdout + result.stderr).strip()}")
    if sys.platform == "darwin":
        # Deny source reads at OS level as well as with the consumer's Python audit hook.
        policy = f'(version 1)(allow default)(deny network*)(deny file-read* (subpath {json.dumps(str(root))}))'
        command = ["/usr/bin/sandbox-exec", "-p", policy, str(python), "-I", "-B", str(test), str(root)]
    else:
        # Linux and Windows have no sandbox-exec equivalent; the consumer's Python
        # audit hook is the isolation boundary and rejects repository opens and
        # dylib/DLL loads. See the README for the Windows isolation limits.
        command = [str(python), "-I", "-B", str(test), str(root)]
    subprocess.run(command, cwd=external, env=environment, check=True)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--python", dest="interpreters", action="append", metavar="VERSION",
                        help="pyenv version name or interpreter path; repeatable "
                             "(default: the qualified macOS matrix, or this interpreter on "
                             "Linux/Windows)")
    args = parser.parse_args()
    wheel = args.wheel.resolve()
    root = Path(__file__).resolve().parents[2]
    if args.interpreters:
        interpreters = tuple(args.interpreters)
    elif sys.platform == "darwin":
        interpreters = DEFAULT_INTERPRETERS
    else:
        # A Linux CI runner or a native Windows host has one interpreter, not a
        # pyenv matrix; pass --python for additional Windows interpreters.
        interpreters = (sys.executable,)
    temporary_root = "/private/tmp" if sys.platform == "darwin" else None
    results = []
    with tempfile.TemporaryDirectory(prefix="cribra-wheel-", dir=temporary_root) as temporary:
        external = Path(temporary)
        native, library_name = inspect_wheel(wheel)
        library = external / library_name
        if sys.platform == "darwin":
            verify_macos_native(native, library)
        elif sys.platform.startswith("linux"):
            verify_linux_native(native, library, root)
        elif sys.platform == "win32":
            verify_windows_native(native, library)
        else:
            raise QualificationError(f"unsupported host platform: {sys.platform}")
        local_wheel = external / wheel.name
        shutil.copy2(wheel, local_wheel)
        test = external / "test_installed_consumer.py"
        shutil.copy2(root / "bindings/python/test_installed_consumer.py", test)
        empty_path = external / "no-tools"
        empty_path.mkdir()
        for spec in interpreters:
            try:
                actual = qualify_interpreter(spec, local_wheel, test, root, external, empty_path)
            except (QualificationError, subprocess.CalledProcessError) as error:
                results.append((spec, "FAIL", str(error).strip()))
            else:
                results.append((spec, "PASS", f"Python {'.'.join(map(str, actual))} via offline install"))
    print("Python interpreter matrix:")
    for spec, status, detail in results:
        print(f"  {status}: {spec} - {detail}")
    failures = [spec for spec, status, _ in results if status != "PASS"]
    if failures:
        raise SystemExit(f"FAIL: wheel consumer matrix failed for: {', '.join(failures)}")
    print(f"PASS: wheel metadata/RECORD, native library, offline installs and installed consumer: {wheel.name}")


if __name__ == "__main__":
    main()
