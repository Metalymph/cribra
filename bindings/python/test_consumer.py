"""Consumer tests for unmodified generated UniFFI bindings; not a Python facade."""

import gc
import json
from pathlib import Path
import sys
import unittest
import weakref

import cribra_uniffi as cribra


def report_snapshot(report):
    """Project only the fields compared by the existing direct-Core oracle."""
    return {
        "sourceId": report.source_id.value,
        "sourceBytes": report.source_bytes,
        "candidateCount": report.candidate_count,
        "needsReview": report.needs_review,
        "findings": [
            {
                "ruleId": finding.rule_id.value,
                "start": finding.location.start,
                "end": finding.location.end,
                "line": finding.location.line,
                "column": finding.location.column,
                "severity": finding.severity.name.lower(),
                "confidence": finding.confidence.name.lower(),
            }
            for finding in report.findings
        ],
    }


class ConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.generated = Path(sys.argv[1]).resolve()
        cls.inputs = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
        cls.expected = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))["reports"]

    @staticmethod
    def source(value):
        return cribra.ScanSource(id=cribra.SourceId(value=value["id"]), text=value["text"])

    def test_import_is_the_freshly_generated_module(self):
        self.assertEqual(Path(cribra.__file__).resolve(), self.generated / "cribra_uniffi.py")

    def test_single_scans_match_all_core_fixture_reports(self):
        scanner = cribra.Scanner()
        self.assertEqual(len(self.inputs), len(self.expected))
        for value, expected in zip(self.inputs, self.expected):
            with self.subTest(source_id=value["id"]):
                self.assertEqual(report_snapshot(scanner.scan(self.source(value))), expected)

    def test_synthetic_finding_and_utf8_coordinates(self):
        value = self.inputs[0]  # Existing synthetic PAT fixture, never a live credential.
        report = cribra.Scanner().scan(self.source(value))
        finding = report.findings[0]
        self.assertEqual(finding.rule_id.value, "github.classic-pat")
        self.assertEqual(finding.severity, cribra.Severity.CRITICAL)
        self.assertEqual(finding.confidence, cribra.Confidence.HIGH)
        location = finding.location
        # CRLF, emoji and combining marks distinguish UTF-8 bytes/scalars from
        # Python character offsets, UTF-16 units and grapheme cluster indexing.
        self.assertEqual((location.start, location.end, location.line, location.column), (28, 65, 2, 19))
        self.assertEqual(
            value["text"].encode("utf-8")[location.start:location.end].decode("utf-8"),
            "ghp_AbCdEf0123456789_AbCdEf0123456789",
        )
        self.assertEqual(report.source_bytes, len(value["text"].encode("utf-8")))

    def test_empty_and_clean_input(self):
        scanner = cribra.Scanner()
        for index in (1, 2):
            with self.subTest(index=index):
                report = scanner.scan(self.source(self.inputs[index]))
                self.assertEqual(report_snapshot(report), self.expected[index])
                self.assertEqual(report.findings, [])
                self.assertEqual(report.candidate_count, 0)
                self.assertFalse(report.needs_review)

    def test_repeated_scans_do_not_retain_previous_results(self):
        scanner = cribra.Scanner()
        for _ in range(5):
            for value, expected in zip(self.inputs, self.expected):
                self.assertEqual(report_snapshot(scanner.scan(self.source(value))), expected)

    def test_reports_survive_scanner_destruction(self):
        scanner = cribra.Scanner()
        reference = weakref.ref(scanner)
        reports = [scanner.scan(self.source(value)) for value in self.inputs]
        del scanner
        gc.collect()
        self.assertIsNone(reference())
        self.assertEqual([report_snapshot(report) for report in reports], self.expected)

    def test_core_only_feature_boundary(self):
        for name in ("redact", "template", "pseudonymize", "synthesize", "scan_and_build", "build", "ShareBundle"):
            with self.subTest(api=name):
                self.assertFalse(hasattr(cribra, name))
        self.assertFalse(hasattr(cribra.Scanner, "parallel_scan"))


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
