"""Fuse parallel-scan qualification against the existing direct Core oracle."""

import gc
from itertools import cycle, islice
import json
from pathlib import Path
import sys
import unittest
import weakref

import cribra_uniffi as cribra
import test_consumer as core_tests


def snapshot(result):
    """Reuse the Core report projection; this oracle serializes snake-case summaries."""
    value = core_tests.results_snapshot(result)
    value["summary"] = vars(result.summary).copy()
    return value


class ParallelConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
        cls.oracle = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
        sources = [core_tests.ConsumerTests.source(value) for value in cls.fixture["sources"]]
        # Same batch definitions as tests/support/parallel.rs and the Swift consumer.
        cls.batches = {"empty": []}
        cls.batches.update({f"single-{index}": [source] for index, source in enumerate(sources)})
        cls.batches.update(multiple=sources, reversed=list(reversed(sources)),
                           large=list(islice(cycle(sources), 64)))
        cls.modes = ("builtInOnly", "customOnly", "builtInAndCustom", "emptyRules")

    def scanner(self, mode):
        case = {
            "mode": "customOnly" if mode == "emptyRules" else mode,
            "rules": [] if mode == "emptyRules" else
                [{**rule, "kind": "literal"} for rule in self.fixture["rules"]],
        }
        return cribra.Scanner.configured(core_tests.ConsumerTests.config(case))

    def test_all_parallel_scenarios_match_core_sequential_and_repeated_calls(self):
        self.assertEqual(len(self.oracle), len(self.modes) * len(self.batches))
        index = 0
        for mode in self.modes:
            scanner = self.scanner(mode)
            for name, sources in self.batches.items():
                with self.subTest(mode=mode, batch=name):
                    expected = self.oracle[index]
                    index += 1
                    self.assertEqual((mode, name), (expected["mode"], expected["batch"]))
                    for _ in range(8):
                        parallel = scanner.parallel_scan(sources)
                        self.assertEqual(snapshot(parallel), expected["outcome"])
                        self.assertEqual(snapshot(scanner.scan_many(sources)), expected["outcome"])
                        self.assertEqual([report.source_id.value for report in parallel.reports],
                                         [source.id.value for source in sources])
                        self.assertEqual([report.source_bytes for report in parallel.reports],
                                         [len(source.text.encode("utf-8")) for source in sources])

    def test_parallel_fixture_candidates_unicode_and_all_summary_counters(self):
        result = self.scanner("builtInAndCustom").parallel_scan(self.batches["multiple"])
        summary = result.summary
        self.assertEqual((summary.scanned_sources, summary.reports_with_findings,
                          summary.reports_without_findings), (7, 4, 3))
        self.assertEqual((summary.total_findings, summary.total_candidates,
                          summary.reports_with_candidates), (11, 1, 1))
        self.assertEqual((summary.critical, summary.high, summary.medium, summary.low, summary.info),
                         (4, 3, 1, 1, 2))
        candidate = result.reports[3]
        self.assertEqual(candidate.findings, [])
        self.assertEqual(candidate.candidate_count, 1)
        self.assertTrue(candidate.needs_review)
        # Canonically equivalent IDs retain distinct UTF-8 encodings.
        self.assertNotEqual(result.reports[5].source_id.value.encode("utf-8"),
                            result.reports[6].source_id.value.encode("utf-8"))
        self.assertEqual(result.reports[0].source_id, result.reports[4].source_id)
        self.assertEqual(result.reports[2].source_id.value, "")
        self.assertEqual(result.reports[0].findings[0].location,
                         cribra.Location(start=28, end=65, line=2, column=19))

    def test_parallel_results_survive_scanner_gc(self):
        for mode in self.modes:
            with self.subTest(mode=mode):
                scanner = self.scanner(mode)
                result = scanner.parallel_scan(self.batches["large"])
                expected = next(entry["outcome"] for entry in self.oracle
                                if entry["mode"] == mode and entry["batch"] == "large")
                reference = weakref.ref(scanner)
                del scanner
                gc.collect()
                self.assertIsNone(reference())
                self.assertEqual(snapshot(result), expected)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
