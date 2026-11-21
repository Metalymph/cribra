"""Regression tests for the generated archive INSTALL.md.

These tests exercise the renderer directly for every published target, because
the release pipeline builds each archive on a different native runner. They
guard the packaging policy that broke earlier releases:

* the host-configuration example must be valid JSON on every target;
* Windows backslashes must be JSON-escaped;
* extraction and checksum commands must match the target platform;
* the macOS ARM64 ad-hoc signature and x86_64 unsigned artifact must stay
  distinct;
* Gatekeeper and SmartScreen text must not present bypasses;
* the documented GLIBC floor must be the measured 2.34.

Run from the repository root:

    python3 -B -m unittest discover \
      -s crates/cribra-mcp/qualification/packaging -p 'test_*.py'
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(
    0, str(Path(__file__).resolve().parents[2] / "packaging")
)

import package

VERSION = "0.1.1"


def render(target: str) -> str:
    return package.render_install(
        VERSION, target, package.artifact_name(VERSION, target)
    )


class InstallDocumentTests(unittest.TestCase):
    def test_all_targets_render_valid_json(self) -> None:
        for target in package.TARGETS:
            with self.subTest(target=target):
                text = render(target)
                config = package.install_host_config(text)
                self.assertEqual(
                    config["mcpServers"]["cribra"]["command"],
                    package.absolute_config_value(target, VERSION),
                )
                self.assertEqual(config["mcpServers"]["cribra"]["args"], [])

    def test_windows_command_is_json_escaped(self) -> None:
        text = render("x86_64-pc-windows-msvc")
        self.assertIn(r'"command": "C:\\Users\\you\\cribra-mcp', text)
        self.assertNotIn(r'"command": "C:\Users\you', text)

    def test_user_local_absolute_paths(self) -> None:
        self.assertTrue(
            package.absolute_config_value("aarch64-apple-darwin", VERSION).startswith(
                "/Users/you/cribra-mcp/"
            )
        )
        self.assertTrue(
            package.absolute_config_value("x86_64-unknown-linux-gnu", VERSION).startswith(
                "/home/you/cribra-mcp/"
            )
        )
        self.assertTrue(
            package.absolute_config_value("x86_64-pc-windows-msvc", VERSION).startswith(
                "C:\\Users\\you\\cribra-mcp\\"
            )
        )

    def test_platform_specific_extraction_and_checksum_commands(self) -> None:
        for target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            text = render(target)
            self.assertIn("tar -xzf cribra-mcp-0.1.1-", text)
            self.assertIn("shasum -a 256 -c", text)
            self.assertNotIn("sha256sum -c", text)

        for target in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
            text = render(target)
            self.assertIn("tar -xzf cribra-mcp-0.1.1-", text)
            self.assertIn("sha256sum -c", text)
            self.assertNotIn("shasum -a 256 -c", text)

        windows = render("x86_64-pc-windows-msvc")
        self.assertIn("Expand-Archive -Path", windows)
        self.assertIn("Get-FileHash -Algorithm SHA256", windows)
        self.assertNotIn("shasum", windows)
        self.assertNotIn("sha256sum", windows)

    def test_single_archive_checksum_verification(self) -> None:
        for target in package.TARGETS:
            with self.subTest(target=target):
                text = render(target)
                self.assertIn("SHA256SUMS has no entry", text)
                self.assertIn("integrity check, not an authenticity", text)
                self.assertNotIn("sha256sum -c SHA256SUMS", text)
                self.assertNotIn("shasum -a 256 -c SHA256SUMS", text)

    def test_macos_signature_statements_are_target_specific(self) -> None:
        arm = render("aarch64-apple-darwin")
        self.assertIn("ad-hoc signed", arm)
        self.assertIn("not notarized", arm)

        intel = render("x86_64-apple-darwin")
        self.assertIn("unsigned", intel)
        self.assertIn("not notarized", intel)

        for target in package.TARGETS:
            text = render(target)
            self.assertNotIn("xattr", text)
            self.assertNotIn("Run anyway", text)
            if target.startswith(("aarch64-apple-darwin", "x86_64-apple-darwin")):
                self.assertIn("no Gatekeeper bypass", text)
            if target == "x86_64-pc-windows-msvc":
                self.assertIn("no SmartScreen bypass", text)

    def test_linux_symbol_floor_is_two_thirty_four(self) -> None:
        for target in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
            text = render(target)
            self.assertIn("GLIBC_2.34", text)
            self.assertIn("readelf --version-info", text)
            self.assertNotIn("GLIBC_2.35", text)
            self.assertIn("not a runtime-qualified distribution list", text)

    def test_windows_smartscreen_is_unsigned_without_bypass(self) -> None:
        text = render("x86_64-pc-windows-msvc")
        self.assertIn("unsigned", text)
        self.assertIn("SmartScreen", text)
        self.assertIn("no SmartScreen bypass", text)

    def test_cli_and_stdin_lifetime_are_documented(self) -> None:
        for target in package.TARGETS:
            with self.subTest(target=target):
                text = render(target)
                self.assertIn("--version", text)
                self.assertIn("--help", text)
                self.assertIn("stdin lifetime", text.lower())
                self.assertIn("stdin EOF", text)
                self.assertIn("immediate EOF", text)
                self.assertIn("exit code `0`", text)

    def test_initialization_and_memory_are_documented(self) -> None:
        text = render("aarch64-apple-darwin")
        self.assertIn("2,927,632", text)
        self.assertIn("1.53 ms", text)
        self.assertIn("158 ms", text)
        self.assertIn("115,280 KiB", text)
        self.assertIn("117,264 KiB", text)
        self.assertIn("released when stdin reaches EOF", text)

    def test_render_leaves_no_placeholders(self) -> None:
        for target in package.TARGETS:
            with self.subTest(target=target):
                text = render(target)
                self.assertNotIn("{{", text)
                self.assertNotIn("}}", text)

    def test_invalid_json_is_rejected(self) -> None:
        with self.assertRaises(SystemExit):
            package.install_host_config("```json\n{bad json}\n```")

    def test_missing_json_is_rejected(self) -> None:
        with self.assertRaises(SystemExit):
            package.install_host_config("no configuration block here")

    def test_non_object_json_is_rejected(self) -> None:
        with self.assertRaises(SystemExit):
            package.install_host_config("```json\n[]\n```")


if __name__ == "__main__":
    unittest.main()
