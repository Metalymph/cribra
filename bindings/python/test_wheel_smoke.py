"""Regression tests for the Linux native-library verification in wheel_smoke.py.

Run from the repository root:

    python3.13 -B -m unittest discover -s bindings/python -p 'test_wheel_smoke.py'
"""

import struct
import unittest
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
