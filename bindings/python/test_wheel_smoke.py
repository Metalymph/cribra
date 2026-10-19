"""Regression tests for wheel_smoke.py.

Covers the Linux native-library verification, the accepted/rejected wheel tags
(including the auditwheel-confirmed manylinux policy) and the isolated offline
installed-consumer qualification.

Run from the repository root:

    python3.13 -B -m unittest discover -s bindings/python -p 'test_wheel_smoke.py'
"""

import struct
import subprocess
import tempfile
import unittest
from unittest import mock
from pathlib import Path

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


def elf_header(elf_class=2, data=1, e_type=3, e_machine=0x3E, magic=b"\x7fELF"):
    payload = magic + bytes([elf_class, data, 1, 0]) + b"\x00" * 8
    return payload + struct.pack("<HH", e_type, e_machine) + b"\x00" * 44


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
            wheel_smoke.qualified_library("py3-none-win_amd64", "win32")


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


if __name__ == "__main__":
    unittest.main(verbosity=2)
