"""Regression tests for wheel_smoke.py.

Covers the Linux native-library verification, the accepted/rejected wheel tags
(including the auditwheel-confirmed manylinux policy and the native Windows
AMD64 policy), the Windows PE/DLL verification and venv selection, the
installed-wheel structure check and the isolated offline installed-consumer
qualification.

Run from the repository root:

    python3.13 -B -m unittest discover -s bindings/python -p 'test_wheel_smoke.py'
"""

import base64
import csv
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

import wheel_smoke


# Exact `ldd` output from the failing PR #46 run. The dynamic loader is listed
# without `=>`, as an absolute path, so a parser that only understands
# `name => /path` reports it as an unresolved DT_NEEDED entry.
CI_LDD = (
    "\tlinux-vdso.so.1 (0x00007fff00000000)\n"
    "\tlibgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007f0000000000)\n"
    "\tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f0000000001)\n"
    "\t/lib64/ld-linux-x86-64.so.2 (0x00007f0000000002)\n"
)

# Exact `readelf -d` DT_NEEDED set from the same run.
CI_NEEDED = ["libgcc_s.so.1", "libc.so.6", "ld-linux-x86-64.so.2"]

# The exact native Windows policy: one tag, one architecture, one library.
WINDOWS_TAG = "py3-none-win_amd64"
WINDOWS_LIBRARY = "cribra_uniffi.dll"


def elf_header(elf_class=2, data=1, e_type=3, e_machine=0x3E, magic=b"\x7fELF"):
    payload = magic + bytes([elf_class, data, 1, 0]) + b"\x00" * 8
    return payload + struct.pack("<HH", e_type, e_machine) + b"\x00" * 44


def pe_dll(machine=0x8664, characteristics=0x2000 | 0x0002):
    """Return a minimal synthetic PE32+ header identifying an x86_64 DLL."""
    pe_offset = 0x80
    coff = struct.pack("<HHIIIHH", machine, 1, 0, 0, 0, 0xF0, characteristics)
    header = b"MZ" + b"\x00" * (0x3C - 2) + struct.pack("<I", pe_offset)
    return header + b"\x00" * (pe_offset - len(header)) + b"PE\x00\x00" + coff + b"\x00" * 0xF0


def write_wheel(root, *, tag=WINDOWS_TAG, library=WINDOWS_LIBRARY, native_members=None,
                purelib="false", version="0.7.1+fuse", requires_python=">=3.11",
                requires_dist=None, include_license=True):
    """Write a synthetic installed wheel plus a matching build-info.json under `root`."""
    generated = root / "generated.py"
    generated.write_bytes(b"# generated module\n")
    native = root / library
    native.write_bytes(pe_dll())
    output = root / "platform"
    dist = output / "dist"
    dist.mkdir(parents=True)
    dist_info = f"cribra-{version}.dist-info"
    files = {"cribra_uniffi/__init__.py": generated.read_bytes()}
    if native_members is None:
        native_members = {f"cribra_uniffi/{library}": native.read_bytes()}
    files.update(native_members)
    if include_license:
        files[f"{dist_info}/licenses/LICENSE"] = b"MIT\n"
    metadata = (f"Metadata-Version: 2.1\nName: cribra\nVersion: {version}\n"
                f"Requires-Python: {requires_python}\n")
    if requires_dist is not None:
        metadata += f"Requires-Dist: {requires_dist}\n"
    files[f"{dist_info}/METADATA"] = metadata.encode()
    files[f"{dist_info}/WHEEL"] = (
        f"Wheel-Version: 1.0\nRoot-Is-Purelib: {purelib}\nTag: {tag}\n").encode()
    wheel = dist / f"cribra-{version}-{tag}.whl"
    with zipfile.ZipFile(wheel, "w") as archive:
        for name, payload in files.items():
            archive.writestr(name, payload)
        rows = []
        for name, payload in files.items():
            digest = base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=").decode()
            rows.append(f"{name},sha256={digest},{len(payload)}")
        rows.append(f"{dist_info}/RECORD,,")
        archive.writestr(f"{dist_info}/RECORD", "\n".join(rows) + "\n")
    (output / "build-info.json").write_text(
        json.dumps({"generatedModule": str(generated), "nativeLibrary": str(native)}))
    return wheel


class LinuxLddTests(unittest.TestCase):
    def setUp(self):
        self.repository = Path("/workspace/cribra")
        self.library = "libcribra_uniffi.so"

    def check(self, listing, needed=None):
        return wheel_smoke.check_ldd_dependencies(
            listing, CI_NEEDED if needed is None else needed, self.library, self.repository)

    def assertRejected(self, listing, needle, needed=None):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            self.check(listing, needed)
        self.assertIn(needle, str(caught.exception))

    def test_exact_ci_output_resolves_loader_by_basename(self):
        self.assertEqual(self.check(CI_LDD), set(CI_NEEDED))

    def test_loader_listed_under_usr_is_accepted(self):
        listing = CI_LDD.replace("/lib64/ld-linux-x86-64.so.2",
                                 "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2")
        self.assertIn("ld-linux-x86-64.so.2", self.check(listing))

    def test_missing_dependency(self):
        self.assertRejected(CI_LDD, "did not resolve", needed=CI_NEEDED + ["libm.so.6"])

    def test_not_found_is_rejected(self):
        self.assertRejected("libc.so.6 => not found\n", "unresolved shared-library dependency")

    def test_repository_local_resolution_is_rejected(self):
        self.assertRejected("libc.so.6 => /workspace/cribra/target/libc.so.6 (0x1)\n",
                            "repository-local dependency")

    def test_repository_local_loader_is_rejected(self):
        self.assertRejected("/workspace/cribra/target/ld-linux-x86-64.so.2 (0x1)\n",
                            "repository-local dependency")

    def test_dependency_outside_system_directories_is_rejected(self):
        self.assertRejected("libc.so.6 => /opt/vendor/libc.so.6 (0x1)\n",
                            "outside system directories")

    def test_empty_resolution_is_malformed(self):
        self.assertRejected("libc.so.6 => (0x1)\n", "malformed ldd entry")

    def test_relative_resolution_is_malformed(self):
        self.assertRejected("libc.so.6 => relative/libc.so.6 (0x1)\n", "malformed ldd entry")

    def test_needed_name_without_path_is_malformed(self):
        self.assertRejected("ld-linux-x86-64.so.2 (0x1)\n", "malformed ldd entry")

    def test_kernel_vdso_without_path_is_ignored(self):
        self.assertEqual(self.check("\tlinux-vdso.so.1 (0x1)\n", needed=[]), set())


class LinuxElfHeaderTests(unittest.TestCase):
    def test_accepts_x86_64_et_dyn(self):
        wheel_smoke.check_linux_elf_header(elf_header(), "libcribra_uniffi.so")

    def test_rejects_wrong_machine(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.check_linux_elf_header(elf_header(e_machine=0xB7), "libcribra_uniffi.so")


class WheelTagTests(unittest.TestCase):
    """`qualified_library` is the fail-closed platform/ABI tag gate."""

    def test_accepts_native_linux_tag(self):
        self.assertEqual(wheel_smoke.qualified_library("py3-none-linux_x86_64", "linux"),
                         wheel_smoke.LINUX_LIBRARY)

    def test_accepts_repaired_manylinux_tag(self):
        self.assertEqual(wheel_smoke.qualified_library("py3-none-manylinux_2_28_x86_64", "linux"),
                         wheel_smoke.LINUX_LIBRARY)

    def test_accepts_macos_arm64_tag(self):
        self.assertEqual(wheel_smoke.qualified_library("py3-none-macosx_13_0_arm64", "darwin"),
                         wheel_smoke.MACOS_LIBRARY)

    def test_accepts_windows_amd64_tag(self):
        self.assertEqual(wheel_smoke.qualified_library("py3-none-win_amd64", "win32"),
                         wheel_smoke.WINDOWS_LIBRARY)

    def test_rejects_windows_arm64_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-win_arm64", "win32")

    def test_rejects_windows_32_bit_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-win32", "win32")

    def test_rejects_windows_cpython_abi_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("cp313-cp313-win_amd64", "win32")

    def test_rejects_other_manylinux_version(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-manylinux_2_34_x86_64", "linux")

    def test_rejects_manylinux2014_alias(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-manylinux2014_x86_64", "linux")

    def test_rejects_musllinux(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-musllinux_1_2_x86_64", "linux")

    def test_rejects_other_linux_architecture(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-manylinux_2_28_aarch64", "linux")

    def test_rejects_cpython_abi_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("cp313-cp313-manylinux_2_28_x86_64", "linux")

    def test_rejects_abi3_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-abi3-linux_x86_64", "linux")

    def test_rejects_intel_macos_tag(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-macosx_13_0_x86_64", "darwin")

    def test_rejects_unsupported_host(self):
        with self.assertRaises(wheel_smoke.QualificationError):
            wheel_smoke.qualified_library("py3-none-win_amd64", "freebsd")


class InstalledConsumerTests(unittest.TestCase):
    """`qualify_interpreter` installs offline and isolates the consumer."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.external = Path(self.temporary.name)
        self.empty_path = self.external / "no-tools"
        self.empty_path.mkdir()
        self.executable = self.external / "python"
        self.test = self.external / "test_installed_consumer.py"
        self.wheel = self.external / "cribra-0.7.1+fuse-py3-none-manylinux_2_28_x86_64.whl"
        self.root = self.external / "repository"

    def qualify(self, calls, install_code=0, platform="linux"):
        def run(args, **kwargs):
            calls.append((args, kwargs))
            code = install_code if "--no-index" in args else 0
            return subprocess.CompletedProcess(args, code, "", "")
        with mock.patch.object(wheel_smoke, "resolve_interpreter", return_value=self.executable), \
                mock.patch.object(wheel_smoke, "interpreter_version", return_value=(3, 11, 17)), \
                mock.patch.object(wheel_smoke.subprocess, "run", side_effect=run), \
                mock.patch.object(wheel_smoke.sys, "platform", platform):
            return wheel_smoke.qualify_interpreter(
                "3.11.17", self.wheel, self.test, self.root, self.external, self.empty_path)

    def test_offline_install_and_linux_consumer(self):
        calls = []
        actual = self.qualify(calls)
        self.assertEqual(actual, (3, 11, 17))
        self.assertEqual(len(calls), 3)
        venv_args, _ = calls[0]
        self.assertEqual(venv_args[:3], [str(self.executable), "-m", "venv"])
        install_args, install_kwargs = calls[1]
        self.assertIn("--no-index", install_args)
        self.assertIn("--no-deps", install_args)
        self.assertIn(str(self.wheel), install_args)
        self.assertEqual(install_kwargs["cwd"], self.external)
        self.assertEqual(install_kwargs["env"]["PATH"], str(self.empty_path))
        consumer_args, consumer_kwargs = calls[2]
        self.assertEqual(consumer_args, [str(self.external / "venv-3.11.17" / "bin" / "python"),
                                         "-I", "-B", str(self.test), str(self.root)])
        self.assertEqual(consumer_kwargs["env"]["PATH"], str(self.empty_path))

    def test_failed_offline_install_fails_closed(self):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            self.qualify([], install_code=1)
        self.assertIn("offline install failed", str(caught.exception))

    def test_macos_consumer_denies_network_and_repository(self):
        calls = []
        self.qualify(calls, platform="darwin")
        consumer_args, _ = calls[2]
        self.assertEqual(consumer_args[0], "/usr/bin/sandbox-exec")
        policy = consumer_args[consumer_args.index("-p") + 1]
        self.assertIn("deny network*", policy)
        self.assertIn(str(self.root), policy)

    def test_windows_consumer_uses_scripts_interpreter_and_no_sandbox(self):
        calls = []
        self.qualify(calls, platform="win32")
        consumer_args, consumer_kwargs = calls[2]
        self.assertEqual(consumer_args[0],
                         str(self.external / "venv-3.11.17" / "Scripts" / "python.exe"))
        self.assertEqual(consumer_kwargs["env"]["PATH"], str(self.empty_path))
        self.assertEqual(consumer_kwargs["env"]["TEMP"], str(self.external))

    def test_failed_offline_install_fails_closed_on_windows(self):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            self.qualify([], install_code=1, platform="win32")
        self.assertIn("offline install failed", str(caught.exception))


class WindowsPeHeaderTests(unittest.TestCase):
    """`check_windows_pe_header` reuses the builder's single PE32+ parser."""

    def test_accepts_x86_64_dll(self):
        wheel_smoke.check_windows_pe_header(pe_dll(), WINDOWS_LIBRARY)

    def test_rejects_invalid_pe(self):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            wheel_smoke.check_windows_pe_header(b"not a PE image", WINDOWS_LIBRARY)
        self.assertIn("PE image", str(caught.exception))

    def test_rejects_arm64_machine(self):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            wheel_smoke.check_windows_pe_header(pe_dll(machine=0xAA64), WINDOWS_LIBRARY)
        self.assertIn("x86_64 PE image", str(caught.exception))

    def test_rejects_non_dll_image(self):
        with self.assertRaises(wheel_smoke.QualificationError) as caught:
            wheel_smoke.check_windows_pe_header(pe_dll(characteristics=0x0002), WINDOWS_LIBRARY)
        self.assertIn("PE dynamic library", str(caught.exception))

    def test_verify_windows_native_writes_and_checks(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        library = Path(temporary.name) / WINDOWS_LIBRARY
        wheel_smoke.verify_windows_native(pe_dll(), library)
        self.assertEqual(library.read_bytes(), pe_dll())


class WindowsWheelStructureTests(unittest.TestCase):
    """`inspect_wheel` is the installed-wheel structure gate for the Windows wheel."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def inspect(self, wheel):
        with mock.patch.object(wheel_smoke.sys, "platform", "win32"):
            return wheel_smoke.inspect_wheel(wheel)

    def test_accepts_exactly_the_windows_dll(self):
        native, library = self.inspect(write_wheel(self.root))
        self.assertEqual(library, WINDOWS_LIBRARY)
        self.assertEqual(native, (self.root / WINDOWS_LIBRARY).read_bytes())

    def test_rejects_missing_dll(self):
        wheel = write_wheel(self.root, native_members={})
        with self.assertRaises(AssertionError) as caught:
            self.inspect(wheel)
        self.assertIn("expected exactly one native library", str(caught.exception))

    def test_rejects_foreign_native_library(self):
        wheel = write_wheel(self.root, native_members={
            f"cribra_uniffi/{WINDOWS_LIBRARY}": pe_dll(),
            "cribra_uniffi/libcribra_uniffi.so": b"\x7fELF",
        })
        with self.assertRaises(AssertionError) as caught:
            self.inspect(wheel)
        self.assertIn("expected exactly one native library", str(caught.exception))

    def test_rejects_pure_wheel(self):
        wheel = write_wheel(self.root, purelib="true")
        with self.assertRaises(AssertionError):
            self.inspect(wheel)

    def test_rejects_wrong_platform_tag(self):
        wheel = write_wheel(self.root, tag="py3-none-linux_x86_64")
        with self.assertRaises(wheel_smoke.QualificationError):
            self.inspect(wheel)


class VenvInterpreterTests(unittest.TestCase):
    """The venv interpreter path follows the platform layout."""

    def test_posix_uses_bin_python(self):
        with mock.patch.object(wheel_smoke.sys, "platform", "linux"):
            self.assertEqual(wheel_smoke.venv_python(Path("/tmp/venv")),
                             Path("/tmp/venv/bin/python"))

    def test_macos_uses_bin_python(self):
        with mock.patch.object(wheel_smoke.sys, "platform", "darwin"):
            self.assertEqual(wheel_smoke.venv_python(Path("/tmp/venv")),
                             Path("/tmp/venv/bin/python"))

    def test_windows_uses_scripts_python(self):
        with mock.patch.object(wheel_smoke.sys, "platform", "win32"):
            self.assertEqual(wheel_smoke.venv_python(Path("C:/venv")),
                             Path("C:/venv/Scripts/python.exe"))

    def test_venv_name_is_filesystem_safe(self):
        self.assertEqual(wheel_smoke.venv_name("3.11.17"), "venv-3.11.17")
        self.assertEqual(wheel_smoke.venv_name(r"C:\Python313\python.exe"),
                         "venv-C__Python313_python.exe")


class IsolatedEnvironmentTests(unittest.TestCase):
    """Windows needs a different minimal environment than POSIX."""

    def test_posix_environment_is_unchanged(self):
        with mock.patch.object(wheel_smoke.sys, "platform", "linux"):
            environment = wheel_smoke.isolated_environment(Path("/tmp/ext"), Path("/tmp/none"))
        self.assertEqual(environment, {"PATH": "/tmp/none", "HOME": "/tmp/ext",
                                       "TMPDIR": "/tmp/ext",
                                       "PYTHONDONTWRITEBYTECODE": "1", "LC_CTYPE": "UTF-8"})

    def test_windows_environment_has_empty_path_and_system_root(self):
        with mock.patch.object(wheel_smoke.sys, "platform", "win32"):
            environment = wheel_smoke.isolated_environment(Path("C:/ext"), Path("C:/none"))
        self.assertEqual(environment["PATH"], "C:/none")
        self.assertEqual(environment["TEMP"], "C:/ext")
        self.assertIn("SystemRoot", environment)
        self.assertNotIn("HOME", environment)


if __name__ == "__main__":
    unittest.main(verbosity=2)
