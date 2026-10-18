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
import subprocess
import tempfile
import zipfile


# Local pyenv interpreters qualified by NB-2.5B.3. Distribution requires-python is
# `>=3.11`, so every interpreter must install the wheel through the normal offline
# pip path; there are no compatibility probes or metadata bypasses.
DEFAULT_INTERPRETERS = ("3.11.17", "3.12.15", "3.13.7", "3.14.8")


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


def inspect_wheel(wheel, root):
    """Check wheel metadata/RECORD/platform/native payload; return the dylib bytes."""
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
        assert tag.startswith("py3-none-macosx_") and tag.endswith("_arm64")
        assert wheel.name.endswith(tag + ".whl")
        native = archive.read("cribra_uniffi/libcribra_uniffi.dylib")
        assert archive.read("cribra_uniffi/__init__.py") == (
            root / "target/uniffi-python-wheel/fuse/generated/cribra_uniffi.py").read_bytes()
        assert native == (root / "target/uniffi-python-wheel/fuse/cargo/aarch64-apple-darwin/release/libcribra_uniffi.dylib").read_bytes()
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
    return native


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
    # Deny source reads at OS level as well as with the consumer's Python audit hook.
    policy = f'(version 1)(allow default)(deny network*)(deny file-read* (subpath {json.dumps(str(root))}))'
    subprocess.run(["/usr/bin/sandbox-exec", "-p", policy, str(python), "-I", "-B", str(test), str(root)],
                   cwd=external, env=environment, check=True)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel", type=Path)
    parser.add_argument("--python", dest="interpreters", action="append", metavar="VERSION",
                        help="pyenv version name or interpreter path; repeatable "
                             "(default: the qualified matrix)")
    args = parser.parse_args()
    wheel = args.wheel.resolve()
    root = Path(__file__).resolve().parents[2]
    interpreters = tuple(args.interpreters) if args.interpreters else DEFAULT_INTERPRETERS
    results = []
    with tempfile.TemporaryDirectory(prefix="cribra-wheel-", dir="/private/tmp") as temporary:
        external = Path(temporary)
        native = inspect_wheel(wheel, root)
        library = external / "libcribra_uniffi.dylib"
        library.write_bytes(native)
        assert subprocess.check_output(["/usr/bin/lipo", "-archs", str(library)], text=True).strip() == "arm64"
        subprocess.run(["/usr/bin/file", str(library)], check=True)
        subprocess.run(["/usr/bin/xcrun", "vtool", "-show-build", str(library)], check=True)
        dependencies = subprocess.check_output(["/usr/bin/otool", "-L", str(library)], text=True)
        print(dependencies)
        for line in dependencies.splitlines()[2:]:  # Skip filename and LC_ID_DYLIB.
            assert line.strip().startswith(("/usr/lib/", "/System/Library/")), line
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
    print(f"PASS: wheel metadata/RECORD, ARM64, offline installs and installed consumer: {wheel.name}")


if __name__ == "__main__":
    main()
