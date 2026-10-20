"""Portable classpath regression checks; no native bundles or compiler required."""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import qualify_bundle_consumer as consumer


class ClasspathTests(unittest.TestCase):
    def test_compiler_argfiles_preserve_all_profiles_and_platform_separators(self):
        for platform, separator in (("win32", ";"), ("darwin", ":"), ("linux", ":")):
            for profile in consumer.PROFILES:
                with self.subTest(platform=platform, profile=profile), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    jar = root / f"cribra-{profile}.jar"
                    jna = root / "jna-5.17.0.jar"
                    classes = root / "classes with spaces é" / profile
                    with mock.patch.object(consumer.sys, "platform", platform), \
                         mock.patch.object(consumer.os, "pathsep", separator), \
                         mock.patch.object(consumer, "run_checked") as run:
                        consumer.compile_consumer(Path("kotlinc.bat"), profile, jar, jna, classes, root, {})
                    command = run.call_args.args[0]
                    self.assertEqual(len(command), 2)
                    self.assertTrue(command[1].startswith("@"))
                    args = [json.loads(line) for line in Path(command[1][1:]).read_text(encoding="utf-8").splitlines()]
                    self.assertEqual(args[args.index("-classpath") + 1], f"{jar}{separator}{jna}")
                    self.assertEqual(args[args.index("-d") + 1], str(classes))
                    self.assertEqual([Path(path).name for path in args[-2:]],
                                     ["BundleNative.kt", consumer.CONSUMER_SOURCES[profile]])

    def test_windows_argfile_preserves_backslashes_and_semicolons(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            jar = Path(r"C:\bundles\core.jar")
            jna = Path(r"C:\bundles\jna-5.17.0.jar")
            with mock.patch.object(consumer.os, "pathsep", ";"), \
                 mock.patch.object(consumer, "run_checked") as run:
                consumer.compile_consumer(Path("kotlinc.bat"), "core", jar, jna,
                                          root / "classes é", root, {})
            text = Path(run.call_args.args[0][1][1:]).read_text(encoding="utf-8")
            values = [json.loads(line) for line in text.splitlines()]
            self.assertEqual(values[3], str(jar) + ";" + str(jna))
            self.assertEqual(values[5], str(root / "classes é"))
            self.assertTrue(all(line.startswith('"') and line.endswith('"') for line in text.splitlines()))

    def test_jvm_classpath_is_one_native_argument_in_every_case(self):
        for platform, separator in (("win32", ";"), ("darwin", ":"), ("linux", ":")):
            for case in ("positive", "removed", "invalid", "mixed"):
                with self.subTest(platform=platform, case=case), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    entries = [root / "classes", root / "core.jar", root / "jna.jar", root / "stdlib é.jar"]
                    if case == "mixed":
                        entries.append(root / "flow.jar")
                    with mock.patch.object(consumer.sys, "platform", platform), \
                         mock.patch.object(consumer.os, "pathsep", separator), \
                         mock.patch.object(consumer.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
                        args, cwd, env = consumer.consumer_command(
                            Path("java.exe"), entries, "ConsumerKt", root / "native",
                            root / "native/library", root / case)
                        consumer.run_capture(args, cwd, env)
                    command = run.call_args.args[0]
                    self.assertEqual(command[command.index("-cp") + 1], separator.join(map(str, entries)))
                    self.assertEqual(command[command.index("-cp") + 2], "ConsumerKt")
                    self.assertFalse(run.call_args.kwargs["shell"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
