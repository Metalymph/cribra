"""Regression tests for the opt-in manylinux path in build_wheel.py.

Run from the repository root:

    python3.13 -B -m unittest discover -s bindings/python -p 'test_build_wheel.py'

These tests never build a wheel and never invoke the real auditwheel: they cover
the tag parsing, the fail-closed wheel verification and the fact that a
platform tag only ever comes from a successful `auditwheel repair`.
"""

import os
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest import mock
import zipfile

import build_wheel


PLATFORM = "manylinux_2_28_x86_64"
LIBRARY = "libcribra_uniffi.so"


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


if __name__ == "__main__":
    unittest.main(verbosity=2)
