"""Inspect one local wheel and qualify it across a matrix of isolated interpreters."""

import argparse
import base64
import csv
from email.parser import BytesParser
import hashlib
import io
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zipfile


# Local pyenv interpreters qualified by NB-2.5B.3. Distribution requires-python is
# `>=3.11`, so every interpreter must install the wheel through the normal offline
# pip path; there are no compatibility probes or metadata bypasses.
DEFAULT_INTERPRETERS = ("3.11.17", "3.12.15", "3.13.7", "3.14.8")

MACOS_LIBRARY = "libcribra_uniffi.dylib"
LINUX_LIBRARY = "libcribra_uniffi.so"
# The Linux builder keeps the local, non-portable tag: no manylinux/musllinux claim.
LINUX_WHEEL_PLATFORM = "linux_x86_64"
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
    if "/" in spec or Path(spec).is_file():
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
        if sys.platform == "darwin":
            assert tag.startswith("py3-none-macosx_") and tag.endswith("_arm64")
            library_name = MACOS_LIBRARY
        elif sys.platform.startswith("linux"):
            # The local, non-portable tag is asserted so a manylinux/musllinux tag
            # can never slip through a qualification that has not verified it.
            assert tag == "py3-none-" + LINUX_WHEEL_PLATFORM, tag
            library_name = LINUX_LIBRARY
        else:
            raise QualificationError(f"unsupported host platform: {sys.platform}")
        assert wheel.name.endswith(tag + ".whl")
        native = archive.read("cribra_uniffi/" + library_name)
        assert archive.read("cribra_uniffi/__init__.py") == generated.read_bytes()
        assert native == native_library.read_bytes()
        assert any(name.endswith("/LICENSE") for name in archive.namelist())
        # Check every archive member against standard wheel RECORD hashes/sizes.
        records = list(csv.reader(io.StringIO(archive.read(dist_info + "/RECORD").decode())))
        assert {row[0] for row in records} == set(archive.namelist())
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
    resolved = set()
    for line in listing.stdout.splitlines():
        _, separator, target = line.partition("=>")
        if not separator:
            continue
        target = target.strip()
        if not target:
            continue
        if target == "not found":
            raise QualificationError(f"unresolved shared-library dependency: {line.strip()}")
        resolved_path = Path(target.split(" ", 1)[0]).resolve()
        if resolved_path.is_relative_to(repository):
            raise QualificationError(f"{library} loads a repository-local dependency: {target}")
        if not any(resolved_path.is_relative_to(prefix) for prefix in ("/lib", "/lib64", "/usr")):
            raise QualificationError(f"{library} loads a dependency outside system directories: {target}")
        resolved.add(resolved_path.name)
    missing = sorted(set(needed) - resolved)
    if missing:
        raise QualificationError(f"{library} dependencies did not resolve: {missing}")
    print(f"Verified {library}: ELF64 ET_DYN x86_64, dependencies {sorted(set(needed))}.")


def qualify_interpreter(spec, wheel, test, root, external, empty_path):
    """Install offline in a fresh venv and run the installed consumer on one interpreter."""
    executable = resolve_interpreter(spec)
    actual = interpreter_version(executable)
    environment = {"PATH": str(empty_path), "HOME": str(external), "TMPDIR": str(external),
                   "PYTHONDONTWRITEBYTECODE": "1", "LC_CTYPE": "UTF-8"}
    virtual = external / ("venv-" + spec.replace("/", "_"))
    subprocess.run([str(executable), "-m", "venv", str(virtual)], env=environment, check=True)
    python = virtual / "bin" / "python"
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
        # Linux has no sandbox-exec equivalent; the consumer's Python audit hook is
        # the isolation boundary and rejects repository opens and dylib loads.
        command = [str(python), "-I", "-B", str(test), str(root)]
    subprocess.run(command, cwd=external, env=environment, check=True)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--python", dest="interpreters", action="append", metavar="VERSION",
                        help="pyenv version name or interpreter path; repeatable "
                             "(default: the qualified macOS matrix, or this interpreter on Linux)")
    args = parser.parse_args()
    wheel = args.wheel.resolve()
    root = Path(__file__).resolve().parents[2]
    if args.interpreters:
        interpreters = tuple(args.interpreters)
    elif sys.platform == "darwin":
        interpreters = DEFAULT_INTERPRETERS
    else:
        # A Linux CI runner has one interpreter, not a pyenv matrix.
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
