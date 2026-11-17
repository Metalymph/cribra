"""Regression tests for smoke.py distribution validation.

Covers the policy split that broke the 0.1.0 release qualification: a
per-target build passes `--expected-version` for a single archive, so that flag
must not imply the full five-target set. Only `--require-all` may demand
completeness.

Run from the repository root:

    python3 -B -m unittest discover \
      -s crates/cribra-mcp/qualification/packaging -p 'test_smoke.py'
"""

import contextlib
import io
import unittest

import smoke

VERSION = "0.1.0"


def archive(target: str, version: str = VERSION) -> str:
    extension = "zip" if target.endswith("windows-msvc") else "tar.gz"
    return f"cribra-mcp-{version}-{target}.{extension}"


def all_archives(version: str = VERSION) -> list[str]:
    return [archive(target, version) for target in smoke.TARGETS]


def enforce(names, *, require_all=False, expected_version=VERSION):
    """Call the policy under test, discarding the expected stderr diagnostic."""
    with contextlib.redirect_stderr(io.StringIO()):
        smoke.enforce_distribution(names, require_all, expected_version)


class ExpectedVersionTest(unittest.TestCase):
    def test_single_archive_expected_version_passes(self):
        # The per-target release build has only its own archive at this point.
        enforce([archive("x86_64-unknown-linux-gnu")])

    def test_single_archive_wrong_version_fails(self):
        with self.assertRaises(SystemExit):
            enforce([archive("x86_64-unknown-linux-gnu", "0.1.1")])

    def test_each_supplied_archive_is_checked(self):
        with self.assertRaises(SystemExit):
            enforce([archive("x86_64-unknown-linux-gnu"), archive("aarch64-apple-darwin", "9.9.9")])

    def test_no_archives_fails(self):
        with self.assertRaises(SystemExit):
            enforce([])


class RequireAllTest(unittest.TestCase):
    def test_missing_target_passes_without_require_all(self):
        names = all_archives()[:-1]
        enforce(names)

    def test_missing_target_fails_with_require_all(self):
        with self.assertRaises(SystemExit):
            enforce(all_archives()[:-1], require_all=True)

    def test_complete_distribution_passes(self):
        enforce(all_archives(), require_all=True)

    def test_complete_distribution_without_expected_version_passes(self):
        enforce(all_archives(), require_all=True, expected_version=None)

    def test_complete_distribution_wrong_version_fails(self):
        with self.assertRaises(SystemExit):
            enforce(all_archives("0.1.1"), require_all=True)

    def test_duplicate_archive_fails(self):
        names = all_archives()
        with self.assertRaises(SystemExit):
            enforce(names + [names[0]], require_all=True)

    def test_unexpected_target_fails(self):
        names = all_archives() + ["cribra-mcp-0.1.0-x86_64-unknown-linux-musl.tar.gz"]
        with self.assertRaises(SystemExit):
            enforce(names, require_all=True)


if __name__ == "__main__":
    unittest.main()
