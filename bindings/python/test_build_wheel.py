"""Regression tests for the platform selection and fail-closed verification in build_wheel.py.

Run from the repository root:

    python3.13 -B -m unittest discover -s bindings/python -p 'test_build_wheel.py'

These tests never build a wheel and never invoke the real auditwheel: they cover
the host/platform detection (including native Windows x86_64), native-library
selection, the tag parsing, the fail-closed wheel structure verification and the
fact that a manylinux platform tag only ever comes from a successful
`auditwheel repair`.
"""

import os
from pathlib import Path
import struct
import subprocess
import tempfile
import types
import unittest
from unittest import mock
import zipfile

import build_wheel


PLATFORM = "manylinux_2_28_x86_64"
LIBRARY = "libcribra_uniffi.so"
WINDOWS_PLATFORM = "win_amd64"
WINDOWS_LIBRARY = "cribra_uniffi.dll"


def make_wheel(path, tag=f"py3-none-{PLATFORM}", purelib="false",
               members=("cribra_uniffi/libcribra_uniffi.so",)):
    """Write a minimal wheel zip matching the real package layout."""
    metadata = (f"Wheel-Version: 1.0\nGenerator: test\n"
                f"Root-Is-Purelib: {purelib}\nTag: {tag}\n")
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("cribra-0.7.1+fuse.dist-info/WHEEL", metadata)
        for name in members:
            archive.writestr(name, b"native")
    return path


def pe_dll(machine=0x8664, characteristics=0x2000 | 0x0002):
    """Return a minimal synthetic PE32+ header identifying an x86_64 DLL."""
    pe_offset = 0x80
    coff = struct.pack("<HHIIIHH", machine, 1, 0, 0, 0, 0xF0, characteristics)
    header = b"MZ" + b"\x00" * (0x3C - 2) + struct.pack("<I", pe_offset)
    return header + b"\x00" * (pe_offset - len(header)) + b"PE\x00\x00" + coff + b"\x00" * 0xF0


class ManylinuxPlatformTests(unittest.TestCase):
    def test_unset_returns_none(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertIsNone(build_wheel.manylinux_platform())

    def test_blank_returns_none(self):
        with mock.patch.dict(os.environ, {build_wheel.MANYLINUX_PLATFORM: "  "}, clear=True):
            self.assertIsNone(build_wheel.manylinux_platform())

    def test_exact_platform_is_returned(self):
        with mock.patch.dict(os.environ, {build_wheel.MANYLINUX_PLATFORM: PLATFORM}, clear=True):
            self.assertEqual(build_wheel.manylinux_platform(), PLATFORM)

    def test_non_manylinux_tag_is_rejected(self):
        with mock.patch.dict(os.environ, {build_wheel.MANYLINUX_PLATFORM: "linux_x86_64"}, clear=True):
            with self.assertRaises(SystemExit):
                build_wheel.manylinux_platform()

    def test_other_architecture_is_rejected(self):
        with mock.patch.dict(os.environ,
                             {build_wheel.MANYLINUX_PLATFORM: "manylinux_2_28_aarch64"}, clear=True):
            with self.assertRaises(SystemExit):
                build_wheel.manylinux_platform()


class VerifyManylinuxWheelTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def verify(self, name, **kwargs):
        return build_wheel.verify_manylinux_wheel(make_wheel(self.root / name, **kwargs),
                                                  LIBRARY, PLATFORM)

    def test_accepts_confirmed_abi_independent_wheel(self):
        self.verify("cribra-0.7.1+fuse-py3-none-manylinux_2_28_x86_64.whl")

    def test_rejects_locally_tagged_wheel(self):
        with self.assertRaises(SystemExit) as caught:
            self.verify("cribra-0.7.1+fuse-py3-none-linux_x86_64.whl",
                        tag="py3-none-linux_x86_64")
        self.assertIn("Tag", str(caught.exception))

    def test_rejects_cpython_abi_tag(self):
        with self.assertRaises(SystemExit):
            self.verify("cribra-0.7.1+fuse-cp313-cp313-manylinux_2_28_x86_64.whl",
                        tag="cp313-cp313-manylinux_2_28_x86_64")

    def test_rejects_pure_wheel(self):
        with self.assertRaises(SystemExit) as caught:
            self.verify("cribra-0.7.1+fuse-py3-none-manylinux_2_28_x86_64.whl", purelib="true")
        self.assertIn("Root-Is-Purelib", str(caught.exception))

    def test_rejects_wrong_platform_tag(self):
        with self.assertRaises(SystemExit):
            self.verify("cribra-0.7.1+fuse-py3-none-manylinux_2_34_x86_64.whl",
                        tag="py3-none-manylinux_2_34_x86_64")

    def test_rejects_missing_native_library(self):
        with self.assertRaises(SystemExit):
            self.verify("cribra-0.7.1+fuse-py3-none-manylinux_2_28_x86_64.whl", members=())


class QualifyManylinuxTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.output = Path(self.temporary.name)
        self.provisional = make_wheel(
            self.output / "cribra-0.7.1+fuse-py3-none-linux_x86_64.whl",
            tag="py3-none-linux_x86_64")
        self.dist = self.output / "dist"
        original = build_wheel.subprocess
        self.addCleanup(setattr, build_wheel, "subprocess", original)

    def install_fake_auditwheel(self, show_code=0, repair_code=0, repair_tag=PLATFORM):
        calls = []

        def run(args, **_kwargs):
            calls.append(args)
            if "show" in args:
                return subprocess.CompletedProcess(args, show_code, "show output\n", "")
            if "repair" in args:
                if repair_code == 0:
                    wheel_dir = Path(args[args.index("--wheel-dir") + 1])
                    wheel_dir.mkdir(parents=True, exist_ok=True)
                    make_wheel(wheel_dir / f"cribra-0.7.1+fuse-py3-none-{repair_tag}.whl",
                               tag=f"py3-none-{repair_tag}")
                return subprocess.CompletedProcess(args, repair_code, "repair output\n", "")
            raise AssertionError(args)

        build_wheel.subprocess = types.SimpleNamespace(run=run)
        return calls

    def test_success_returns_the_auditwheel_output(self):
        calls = self.install_fake_auditwheel()
        wheel = build_wheel.qualify_manylinux(self.provisional, LIBRARY, PLATFORM,
                                              self.output, self.dist)
        self.assertEqual(wheel.name, f"cribra-0.7.1+fuse-py3-none-{PLATFORM}.whl")
        self.assertTrue(wheel.exists())
        self.assertEqual(len(calls), 2)
        self.assertIn("show", calls[0])
        self.assertIn("repair", calls[1])
        self.assertTrue((self.output / "auditwheel-show.txt").read_text().startswith("show output"))

    def test_show_failure_fails_closed(self):
        self.install_fake_auditwheel(show_code=1)
        with self.assertRaises(SystemExit) as caught:
            build_wheel.qualify_manylinux(self.provisional, LIBRARY, PLATFORM,
                                          self.output, self.dist)
        self.assertIn("auditwheel show failed", str(caught.exception))
        self.assertFalse(self.dist.exists())

    def test_repair_refusal_fails_closed(self):
        self.install_fake_auditwheel(repair_code=1)
        with self.assertRaises(SystemExit) as caught:
            build_wheel.qualify_manylinux(self.provisional, LIBRARY, PLATFORM,
                                          self.output, self.dist)
        self.assertIn("refusing to claim manylinux compatibility", str(caught.exception))

    def test_unexpected_output_tag_fails_closed(self):
        self.install_fake_auditwheel(repair_tag="manylinux_2_34_x86_64")
        with self.assertRaises(SystemExit) as caught:
            build_wheel.qualify_manylinux(self.provisional, LIBRARY, PLATFORM,
                                          self.output, self.dist)
        self.assertIn(f"Expected exactly one {PLATFORM} wheel", str(caught.exception))


class HostSpecTests(unittest.TestCase):
    """`host_spec` selects the native target triple and library per compiled host."""

    def host(self, platform_name, machine):
        with mock.patch.object(build_wheel.sys, "platform", platform_name), \
                mock.patch.object(build_wheel.platform, "machine", return_value=machine):
            return build_wheel.host_spec()

    def test_macos_arm64_is_unchanged(self):
        self.assertEqual(self.host("darwin", "arm64"),
                         ("macos", "aarch64-apple-darwin", "libcribra_uniffi.dylib"))

    def test_linux_x86_64_is_unchanged(self):
        self.assertEqual(self.host("linux", "x86_64"),
                         ("linux", "x86_64-unknown-linux-gnu", "libcribra_uniffi.so"))

    def test_windows_x86_64_selects_the_dll(self):
        self.assertEqual(self.host("win32", "AMD64"),
                         ("windows", "x86_64-pc-windows-msvc", "cribra_uniffi.dll"))

    def test_windows_x86_64_machine_alias(self):
        self.assertEqual(self.host("win32", "x86_64"),
                         ("windows", "x86_64-pc-windows-msvc", "cribra_uniffi.dll"))

    def test_windows_32_bit_is_rejected(self):
        with self.assertRaises(SystemExit):
            self.host("win32", "x86")

    def test_windows_arm64_is_rejected(self):
        with self.assertRaises(SystemExit):
            self.host("win32", "ARM64")

    def test_macos_intel_is_rejected(self):
        with self.assertRaises(SystemExit):
            self.host("darwin", "x86_64")

    def test_windows_directory_and_tag_are_explicit(self):
        # The isolated output directory names the host; the wheel tag is win_amd64.
        self.assertEqual(build_wheel.WINDOWS_OUTPUT_KEY, "windows_x86_64")
        self.assertEqual(build_wheel.WINDOWS_WHEEL_PLATFORM, WINDOWS_PLATFORM)
        self.assertIn(WINDOWS_LIBRARY, build_wheel.NATIVE_LIBRARY_NAMES)


class VerifyWheelStructureTests(unittest.TestCase):
    """`verify_wheel` is the fail-closed structure gate for native (non-auditwheel) wheels."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def verify(self, name=f"cribra-0.7.1+fuse-py3-none-{WINDOWS_PLATFORM}.whl", **kwargs):
        kwargs.setdefault("tag", f"py3-none-{WINDOWS_PLATFORM}")
        kwargs.setdefault("members", (f"cribra_uniffi/{WINDOWS_LIBRARY}",))
        return build_wheel.verify_wheel(make_wheel(self.root / name, **kwargs),
                                        WINDOWS_LIBRARY, WINDOWS_PLATFORM)

    def test_accepts_the_windows_dll_wheel(self):
        self.verify()

    def test_rejects_missing_dll(self):
        with self.assertRaises(SystemExit) as caught:
            self.verify(members=())
        self.assertIn("expected exactly", str(caught.exception))

    def test_rejects_foreign_linux_library(self):
        with self.assertRaises(SystemExit):
            self.verify(members=(f"cribra_uniffi/{WINDOWS_LIBRARY}",
                                 "cribra_uniffi/libcribra_uniffi.so"))

    def test_rejects_foreign_macos_library(self):
        with self.assertRaises(SystemExit):
            self.verify(members=("cribra_uniffi/libcribra_uniffi.dylib",))

    def test_rejects_cpython_abi_tag(self):
        with self.assertRaises(SystemExit):
            self.verify(name="cribra-0.7.1+fuse-cp313-cp313-win_amd64.whl",
                        tag="cp313-cp313-win_amd64")

    def test_rejects_pure_wheel(self):
        with self.assertRaises(SystemExit) as caught:
            self.verify(purelib="true")
        self.assertIn("Root-Is-Purelib", str(caught.exception))

    def test_rejects_linux_platform_tag(self):
        with self.assertRaises(SystemExit) as caught:
            self.verify(name="cribra-0.7.1+fuse-py3-none-linux_x86_64.whl",
                        tag="py3-none-linux_x86_64")
        self.assertIn("Tag", str(caught.exception))


class WindowsPeHeaderTests(unittest.TestCase):
    """`verify_windows` fails closed unless the artifact is an x86_64 PE DLL."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.library = Path(self.temporary.name) / WINDOWS_LIBRARY

    def test_accepts_synthetic_x86_64_dll(self):
        self.library.write_bytes(pe_dll())
        build_wheel.verify_windows(self.library)

    def test_rejects_non_pe_payload(self):
        self.library.write_bytes(b"not a PE image")
        with self.assertRaises(SystemExit) as caught:
            build_wheel.verify_windows(self.library)
        self.assertIn("PE image", str(caught.exception))

    def test_rejects_short_mz_file(self):
        self.library.write_bytes(b"MZ")
        with self.assertRaises(SystemExit) as caught:
            build_wheel.verify_windows(self.library)
        self.assertIn("PE image", str(caught.exception))

    def test_rejects_truncated_pe_header(self):
        self.library.write_bytes(b"MZ" + b"\x00" * (0x3C - 2) + struct.pack("<I", 0x1000))
        with self.assertRaises(SystemExit) as caught:
            build_wheel.verify_windows(self.library)
        self.assertIn("PE header", str(caught.exception))

    def test_rejects_arm64_machine(self):
        self.library.write_bytes(pe_dll(machine=0xAA64))
        with self.assertRaises(SystemExit) as caught:
            build_wheel.verify_windows(self.library)
        self.assertIn("x86_64 PE image", str(caught.exception))

    def test_rejects_non_dll_image(self):
        self.library.write_bytes(pe_dll(characteristics=0x0002))
        with self.assertRaises(SystemExit) as caught:
            build_wheel.verify_windows(self.library)
        self.assertIn("PE dynamic library", str(caught.exception))


if __name__ == "__main__":
    unittest.main(verbosity=2)
