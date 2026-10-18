"""Flow/Share consumer qualification of generated bindings, without a facade."""

import json
from pathlib import Path
import sys
import time
import unittest

import cribra_uniffi as cribra

from test_consumer import core_name, single_rule_config


# Fixed PUBLIC test vectors shared with the Rust/Swift fixtures, never live keys.
KEYS = {
    "a": b"0123456789ABCDEF0123456789ABCDEF",
    "b": b"FEDCBA9876543210FEDCBA9876543210",
}


def findings(case):
    """Transfer fixture metadata verbatim; Core owns validation and ordering."""
    return [cribra.Finding(
        rule_id=cribra.RuleId(value=value["id"]),
        location=cribra.Location(start=value["start"], end=value["end"],
                                 line=value.get("line", 1), column=value.get("column", 1)),
        severity=cribra.Severity[value.get("severity", "high").upper()],
        confidence=cribra.Confidence[value.get("confidence", "high").upper()],
    ) for value in case["findings"]]


def pseudo_options(options):
    return cribra.PseudonymizationOptions(
        key=KEYS[options["key"]], prefix=options.get("prefix"),
        digest_bytes=options.get("digestBytes"),
    )


def synthesis_options(options):
    return cribra.SynthesisOptions(key=KEYS[options["key"]], marker=options.get("marker"))


def transform_error(error):
    def span(value):
        return None if value is None else {"start": value.start, "end": value.end}

    return {
        "code": core_name(error.code), "span": span(error.span),
        "otherSpan": span(error.other_span), "sourceLen": error.source_len,
        "detail": error.detail,
    }


def share_error(error):
    if isinstance(error, cribra.ShareError.Transform):
        return {"kind": "transform", **transform_error(error.error)}
    return {
        "kind": "bundle", "code": core_name(error.code), "expected": error.expected,
        "actual": error.actual, "index": error.index, "detail": error.detail,
    }


def share_options(mode):
    return cribra.ShareBundleOptions(mode=(
        cribra.ShareMode.REDACT(),
        cribra.ShareMode.TEMPLATE(),
        cribra.ShareMode.PSEUDONYMIZE(options=pseudo_options(
            {"key": "a", "prefix": "p:", "digestBytes": 8})),
        cribra.ShareMode.SYNTHESIZE(options=synthesis_options({"key": "a", "marker": "demo"})),
    )[mode])


class FlowConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.generated = Path(sys.argv[1]).resolve()
        cls.profile = sys.argv[5] if len(sys.argv) > 5 else "flow"
        fixtures = Path(sys.argv[2])

        def fixture(name):
            return json.loads((fixtures / (name + ".json")).read_text(encoding="utf-8"))

        cls.cases = fixture("flow")
        cls.keyed_cases = cls.cases + fixture("keyed-flow")
        cls.options = fixture("keyed-options")
        cls.batches = fixture("share")
        cls.flow_oracle = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
        cls.share_oracle = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))

    def assert_no_key_echo(self, value):
        serialized = json.dumps(value, ensure_ascii=False)
        for key in KEYS.values():
            self.assertNotIn(key.decode("ascii"), serialized)
            self.assertNotIn(str(list(key)), serialized)

    def transform_outcome(self, operation, *args):
        try:
            outcome = {"content": operation(*args).content}
        except cribra.TransformError as error:
            outcome = {"error": transform_error(error)}
        self.assert_no_key_echo(outcome)
        return outcome

    def share_outcome(self, operation, *args):
        before = time.time_ns()
        try:
            bundle = operation(*args)
        except cribra.ShareError as error:
            outcome = {"error": share_error(error)}
        else:
            after = time.time_ns()
            timestamp = bundle.manifest.generated_at
            self.assertGreater(timestamp.seconds, 0)
            self.assertGreaterEqual(timestamp.nanoseconds, 0)
            self.assertLess(timestamp.nanoseconds, 1_000_000_000)
            actual_time = timestamp.seconds * 1_000_000_000 + timestamp.nanoseconds
            # Check THIS call's wall-clock window, never equality across calls.
            self.assertGreaterEqual(actual_time, before - 1_000_000)
            self.assertLessEqual(actual_time, after + 1_000_000)
            outcome = {
                "sources": [{"id": source.source_id.value, "content": source.content}
                            for source in bundle.sources],
                "mode": bundle.manifest.mode.name.lower(),
                "summary": vars(bundle.manifest.summary).copy(),
            }
        self.assert_no_key_echo(outcome)
        return outcome

    @staticmethod
    def sources(batch):
        return [cribra.ScanSource(id=cribra.SourceId(value=value["id"]), text=value["text"])
                for value in batch["sources"]]

    @staticmethod
    def scanner(batch):
        return (cribra.Scanner() if batch.get("builtins", False) else
                cribra.Scanner.configured(single_rule_config("SECRET", rule_id="secret")))

    def test_profile_import_and_api_gating(self):
        self.assertEqual(Path(cribra.__file__).resolve(), self.generated / "cribra_uniffi.py")
        for name in ("Scanner", "StreamSession", "IncrementalSession", "redact", "template",
                     "pseudonymize", "synthesize", "scan_and_build", "build", "ShareBundle"):
            with self.subTest(api=name):
                self.assertTrue(hasattr(cribra, name))
        self.assertTrue(hasattr(cribra.Scanner, "scan_for_share"))
        self.assertEqual(hasattr(cribra.Scanner, "parallel_scan"), self.profile == "fuse")

    def test_redact_default_custom_options_spans_and_errors_match_core(self):
        expected = self.flow_oracle["redactTemplate"]
        self.assertEqual(len(expected), len(self.cases))
        for case, oracle in zip(self.cases, expected):
            with self.subTest(case=case["name"]):
                self.assertEqual(case["name"], oracle["name"])
                actual = self.transform_outcome(cribra.redact, case["source"], findings(case),
                                                cribra.RedactOptions(replacement=case.get("replacement")))
                self.assertEqual(actual, oracle["redact"])

    def test_template_options_order_overlap_and_errors_match_core(self):
        expected = self.flow_oracle["redactTemplate"]
        self.assertEqual(len(expected), len(self.cases))
        for case, oracle in zip(self.cases, expected):
            with self.subTest(case=case["name"]):
                self.assertEqual(case["name"], oracle["name"])
                actual = self.transform_outcome(cribra.template, case["source"], findings(case),
                                                cribra.TemplateOptions(namespace=case.get("namespace"),
                                                                       numbered=case.get("numbered", False)))
                self.assertEqual(actual, oracle["template"])

    def assert_keyed_matrix(self, name, make_options):
        expected = self.flow_oracle["keyed"]
        self.assertEqual(len(expected), len(self.keyed_cases) * len(self.options))
        index = 0
        for case in self.keyed_cases:
            for options in self.options:
                with self.subTest(transform=name, case=case["name"], options=options["name"]):
                    oracle = expected[index]
                    index += 1
                    self.assertEqual((case["name"], options["name"]), (oracle["name"], oracle["options"]))
                    for _ in range(2):
                        actual = self.transform_outcome(getattr(cribra, name), case["source"],
                                                        findings(case), make_options(options))
                        self.assertEqual(actual, oracle[name])

    def test_pseudonymize_options_determinism_and_errors_match_core(self):
        self.assert_keyed_matrix("pseudonymize", pseudo_options)

    def test_synthesize_options_determinism_and_errors_match_core(self):
        self.assert_keyed_matrix("synthesize", synthesis_options)

    def test_key_changes_and_core_digest_clamping(self):
        case = next(case for case in self.keyed_cases if case["name"] == "content-a")
        source, spans = case["source"], findings(case)
        for operation, make_options in ((cribra.pseudonymize, pseudo_options),
                                        (cribra.synthesize, synthesis_options)):
            first = operation(source, spans, make_options({"key": "a"})).content
            second = operation(source, spans, make_options({"key": "b"})).content
            self.assertNotEqual(first, second)
        # Core clamps these values, including 33 and u64::MAX; they are not errors.
        for requested, digest_length in ((0, 8), (1, 8), (8, 8), (16, 16), (32, 32),
                                         (33, 32), (2**64 - 1, 32)):
            with self.subTest(digest_bytes=requested):
                result = cribra.pseudonymize(source, spans, pseudo_options(
                    {"key": "a", "prefix": "p:", "digestBytes": requested}))
                self.assertEqual(len(result.content.encode("utf-8")), 2 + 2 * digest_length)

    def test_synthesis_uses_core_content_semantics(self):
        inputs = [case for case in self.keyed_cases if case["name"] in ("content-a", "content-b")]
        outputs = [cribra.synthesize(case["source"], findings(case), synthesis_options({"key": "a"})).content
                   for case in inputs]
        self.assertEqual(len(outputs), 2)
        self.assertEqual(outputs[0], outputs[1])  # Core synthesis is not content hashing.

    def test_invalid_key_lengths_and_error_precedence(self):
        invalid_span = next(case for case in self.cases if case["name"] == "outside-source")
        for length in (0, 1, 31, 33, 64, 1024):
            for case in ({"source": "", "findings": []}, invalid_span):
                for operation, options in (
                    (cribra.pseudonymize, cribra.PseudonymizationOptions(
                        key=b"Z" * length, prefix=None, digest_bytes=None)),
                    (cribra.synthesize, cribra.SynthesisOptions(key=b"Z" * length, marker=None)),
                ):
                    with self.subTest(length=length, transform=operation.__name__):
                        actual = self.transform_outcome(operation, case["source"], findings(case), options)
                        self.assertEqual(actual, {"error": {
                            "code": "InvalidKeyLength", "span": None, "otherSpan": None, "sourceLen": None,
                            "detail": f"transformation key must contain exactly 32 bytes; received {length}",
                        }})

    def test_no_implicit_rescanning_for_any_transform(self):
        case = next(case for case in self.cases if case["name"] == "no-rescan")
        for operation, options in (
            (cribra.redact, cribra.RedactOptions(replacement=None)),
            (cribra.template, cribra.TemplateOptions(namespace=None, numbered=False)),
            (cribra.pseudonymize, pseudo_options({"key": "a"})),
            (cribra.synthesize, synthesis_options({"key": "a"})),
        ):
            with self.subTest(transform=operation.__name__):
                self.assertEqual(operation(case["source"], [], options).content, case["source"])

    def test_scan_for_share_preserves_order_findings_and_candidates(self):
        for batch in self.batches:
            with self.subTest(batch=batch["name"]):
                scanner = self.scanner(batch)
                sources = self.sources(batch)
                reports = scanner.scan_for_share(sources)
                ordinary = scanner.scan_many(sources).reports
                self.assertEqual(len(reports), len(sources))
                self.assertEqual([report.source_id.value for report in reports],
                                 [source.id.value for source in sources])
                for report, scanned in zip(reports, ordinary):
                    self.assertEqual(report.source_bytes, scanned.source_bytes)
                    self.assertEqual(report.findings, scanned.findings)
                    self.assertEqual(len(report.candidates), scanned.candidate_count)
                    self.assertFalse(hasattr(report, "text"))
                if batch["name"] == "built-in-candidates":
                    review = reports[0]
                    self.assertEqual(review.findings, [])
                    self.assertEqual(len(review.candidates), 1)
                    candidate = review.candidates[0]
                    self.assertEqual(candidate.kind, cribra.ShareCandidateKind.RECOVERY_LIKE_CODE)
                    self.assertEqual(candidate.evidence, cribra.ShareCandidateEvidence.STRUCTURAL)
                    self.assertEqual(candidate.location, cribra.Location(start=0, end=19, line=1, column=1))

    def test_share_all_modes_both_paths_match_core(self):
        expected = self.share_oracle["batch"]
        self.assertEqual(len(expected), len(self.batches) * 4 * 2)
        index = 0
        for batch in self.batches:
            scanner = self.scanner(batch)
            sources = self.sources(batch)
            reports = scanner.scan_for_share(sources)
            summary = vars(scanner.scan_many(sources).summary)
            for mode in range(4):
                for atomic in (False, True):
                    with self.subTest(batch=batch["name"], mode=mode, atomic=atomic):
                        oracle = expected[index]
                        index += 1
                        self.assertEqual((batch["name"], mode, atomic),
                                         (oracle["name"], oracle["mode"], oracle["atomic"]))
                        if atomic:
                            actual = self.share_outcome(cribra.scan_and_build, scanner, sources, share_options(mode))
                        else:
                            actual = self.share_outcome(cribra.build, reports,
                                                        [source.text for source in sources], share_options(mode))
                        self.assertEqual(actual, oracle["outcome"])
                        self.assertEqual(actual["summary"], summary)
                        self.assertEqual([source["id"] for source in actual["sources"]],
                                         [source.id.value for source in sources])
                        if batch["name"] == "built-in-candidates":
                            self.assertEqual(actual["sources"][0]["content"], sources[0].text)

    def test_share_explicit_reports_all_modes_spans_and_nested_errors_match_core(self):
        expected = self.share_oracle["explicit"]
        self.assertEqual(len(expected), len(self.cases) * 4)
        index = 0
        for case in self.cases:
            report = cribra.ShareReport(source_id=cribra.SourceId(value="明示😀"),
                                        source_bytes=len(case["source"].encode("utf-8")),
                                        findings=findings(case), candidates=[])
            for mode in range(4):
                with self.subTest(case=case["name"], mode=mode):
                    oracle = expected[index]
                    index += 1
                    self.assertEqual((case["name"], mode), (oracle["name"], oracle["mode"]))
                    self.assertEqual(self.share_outcome(cribra.build, [report], [case["source"]],
                                                       share_options(mode)), oracle["outcome"])

    def test_share_missing_mode_count_and_byte_length_errors(self):
        scanner = self.scanner({})
        sources = self.sources({"sources": [{"id": "a", "text": "SECRET"}]})
        reports = scanner.scan_for_share(sources)
        missing = {"error": {
            "kind": "bundle", "code": "MissingShareMode", "expected": None, "actual": None,
            "index": None, "detail": "share bundle transformation mode was not configured",
        }}
        self.assertEqual(self.share_outcome(cribra.scan_and_build, scanner, [],
                                           cribra.ShareBundleOptions(mode=None)), missing)
        # Missing mode takes precedence over an incorrect source count.
        self.assertEqual(self.share_outcome(cribra.build, reports, [],
                                           cribra.ShareBundleOptions(mode=None)), missing)
        for texts in ([], ["SECRET", "extra"]):
            with self.subTest(count=len(texts)):
                self.assertEqual(self.share_outcome(cribra.build, reports, texts, share_options(0)), {
                    "error": {"kind": "bundle", "code": "SourceCountMismatch", "expected": 1,
                              "actual": len(texts), "index": None,
                              "detail": f"share bundle expected 1 sources but received {len(texts)}"},
                })
        for texts, index, actual_bytes in ((["short"], 0, 5), (["😀"], 0, 4), (["SECRET", "é"], 1, 2)):
            with self.subTest(texts=texts):
                selected = reports if index == 0 else reports * 2
                self.assertEqual(self.share_outcome(cribra.build, selected, texts, share_options(0)), {
                    "error": {"kind": "bundle", "code": "SourceLengthMismatch", "expected": 6,
                              "actual": actual_bytes, "index": index,
                              "detail": f"share bundle source {index} has {actual_bytes} bytes but scan results record 6"},
                })

    def test_share_build_accepts_equal_length_changed_content_without_identity_check(self):
        scanner = self.scanner({})
        reports = scanner.scan_for_share(self.sources({"sources": [{"id": "a", "text": "SECRET"}]}))
        actual = self.share_outcome(cribra.build, reports, ["PUBLIC"], share_options(0))
        self.assertEqual(actual["sources"], [{"id": "a", "content": "[REDACTED]"}])

    def test_share_invalid_keys_are_nested_transform_errors(self):
        scanner = self.scanner({})
        for length in (0, 1, 31, 33, 64):
            modes = (
                cribra.ShareMode.PSEUDONYMIZE(options=cribra.PseudonymizationOptions(
                    key=b"Z" * length, prefix=None, digest_bytes=None)),
                cribra.ShareMode.SYNTHESIZE(options=cribra.SynthesisOptions(key=b"Z" * length, marker=None)),
            )
            for mode in modes:
                for atomic in (False, True):
                    with self.subTest(length=length, mode=type(mode).__name__, atomic=atomic):
                        options = cribra.ShareBundleOptions(mode=mode)
                        if atomic:
                            actual = self.share_outcome(cribra.scan_and_build, scanner, [], options)
                        else:
                            actual = self.share_outcome(cribra.build, [], [], options)
                        self.assertEqual(actual, {"error": {
                            "kind": "transform", "code": "InvalidKeyLength", "span": None,
                            "otherSpan": None, "sourceLen": None,
                            "detail": f"transformation key must contain exactly 32 bytes; received {length}",
                        }})


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
