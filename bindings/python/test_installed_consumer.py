"""Standalone installed-wheel smoke; copied and run outside the repository."""

from importlib import metadata
from pathlib import Path
import shutil
import sys
import unittest


REPOSITORY = Path(sys.argv[1]).resolve()
LIBRARY = "libcribra_uniffi.dylib" if sys.platform == "darwin" else "libcribra_uniffi.so"


def deny_repository(event, args):
    # Also catches Python opens and ctypes loads if OS sandboxing is unavailable.
    if event in ("open", "os.listdir", "os.scandir", "ctypes.dlopen") and args:
        path = args[0]
        if isinstance(path, (str, bytes)):
            if isinstance(path, bytes):
                path = path.decode()
            if Path(path).resolve().is_relative_to(REPOSITORY):
                raise PermissionError("Repository access is forbidden in the installed consumer")


sys.addaudithook(deny_repository)
import cribra_uniffi as cribra


class InstalledConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scanner = cribra.Scanner.configured(cribra.ScannerConfig.CUSTOM_ONLY(rules=[
            cribra.RuleSpec(id=cribra.RuleId(value="secret"), kind=cribra.RuleKind.LITERAL,
                            value="SECRET", severity=cribra.Severity.HIGH, remediation=None),
        ]))
        cls.sources = [cribra.ScanSource(id=cribra.SourceId(value=identity), text=text) for identity, text in (
            ("文書🙂", "é🙂é\r\nSECRET"), ("文書🙂", "SECRET + SECRET"),
            ("", ""), ("review", "ABCD-EFGH-IJKL-MNOP"),
        )]
        cls.key = b"0123456789ABCDEF0123456789ABCDEF"  # PUBLIC test vector

    def test_installed_metadata_and_library(self):
        module = Path(cribra.__file__).resolve()
        self.assertTrue(module.is_relative_to(Path(sys.prefix)))
        self.assertTrue(module.with_name(LIBRARY).is_file())
        self.assertEqual(metadata.version("cribra"), cribra.cribra_version() + "+fuse")
        self.assertEqual(metadata.requires("cribra"), None)

    def test_runtime_has_no_repository_or_rust_tools(self):
        self.assertFalse(Path.cwd().is_relative_to(REPOSITORY))
        self.assertTrue(all(not Path(path).resolve().is_relative_to(REPOSITORY) for path in sys.path))
        self.assertIsNone(shutil.which("cargo"))
        self.assertIsNone(shutil.which("rustc"))
        with self.assertRaises(PermissionError):
            (REPOSITORY / "Cargo.toml").read_text()

    def test_scanner_and_utf8_coordinates(self):
        report = self.scanner.scan(self.sources[0])
        self.assertEqual(report.source_bytes, 17)
        self.assertEqual(report.findings[0].rule_id.value, "secret")
        self.assertEqual(report.findings[0].location, cribra.Location(start=11, end=17, line=2, column=1))
        token = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"
        builtin = cribra.Scanner().scan(cribra.ScanSource(id=cribra.SourceId(value="builtin"), text=token))
        self.assertEqual(builtin.findings[0].rule_id.value, "github.classic-pat")

    def test_scan_many_and_candidates(self):
        result = self.scanner.scan_many(self.sources)
        self.assertEqual([report.source_id.value for report in result.reports], [s.id.value for s in self.sources])
        self.assertEqual((result.summary.scanned_sources, result.summary.total_findings,
                          result.summary.total_candidates), (4, 3, 1))
        self.assertEqual(result.reports[3].findings, [])
        self.assertTrue(result.reports[3].needs_review)

    def test_all_flow_transformations(self):
        source = self.sources[0].text
        findings = self.scanner.scan(self.sources[0]).findings
        operations = (
            (cribra.redact, cribra.RedactOptions(replacement=None)),
            (cribra.template, cribra.TemplateOptions(namespace=None, numbered=False)),
            (cribra.pseudonymize, cribra.PseudonymizationOptions(key=self.key, prefix=None, digest_bytes=None)),
            (cribra.synthesize, cribra.SynthesisOptions(key=self.key, marker=None)),
        )
        for operation, options in operations:
            with self.subTest(transform=operation.__name__):
                content = operation(source, findings, options).content
                self.assertNotEqual(content, source)
                self.assertTrue(content.startswith("é🙂é\r\n"))
                self.assertEqual(operation(source, findings, options).content, content)
                self.assertEqual(operation(source, [], options).content, source)

    def test_share_all_modes_and_build_paths(self):
        modes = (cribra.ShareMode.REDACT(), cribra.ShareMode.TEMPLATE(),
                 cribra.ShareMode.PSEUDONYMIZE(options=cribra.PseudonymizationOptions(
                     key=self.key, prefix=None, digest_bytes=None)),
                 cribra.ShareMode.SYNTHESIZE(options=cribra.SynthesisOptions(key=self.key, marker=None)))
        reports = self.scanner.scan_for_share(self.sources)
        for mode in modes:
            with self.subTest(mode=type(mode).__name__):
                options = cribra.ShareBundleOptions(mode=mode)
                atomic = cribra.scan_and_build(self.scanner, self.sources, options)
                explicit = cribra.build(reports, [s.text for s in self.sources], options)
                self.assertEqual(atomic.sources, explicit.sources)
                self.assertEqual(atomic.manifest.mode, explicit.manifest.mode)
                self.assertEqual(atomic.manifest.summary, self.scanner.scan_many(self.sources).summary)
                self.assertEqual(atomic.manifest.summary, explicit.manifest.summary)
                self.assertEqual(atomic.sources[3].content, self.sources[3].text)
                self.assertGreater(atomic.manifest.generated_at.seconds, 0)
                self.assertLess(atomic.manifest.generated_at.nanoseconds, 1_000_000_000)

    def test_fuse_parallel_scan(self):
        expected = self.scanner.scan_many(self.sources)
        for _ in range(3):
            self.assertEqual(self.scanner.parallel_scan(self.sources), expected)
        self.assertEqual(self.scanner.parallel_scan([]), self.scanner.scan_many([]))


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
