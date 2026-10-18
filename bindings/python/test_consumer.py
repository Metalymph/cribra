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


def core_name(value):
    """Match Rust Debug spelling for unit enums, without using ordinal values."""
    return "".join(part.title() for part in value.name.split("_"))


def results_snapshot(results):
    fields = (
        "scanned_sources", "scanned_bytes", "reports_with_findings",
        "reports_without_findings", "total_findings", "total_candidates",
        "reports_with_candidates", "critical", "high", "medium", "low", "info",
    )
    return {
        "reports": [report_snapshot(report) for report in results.reports],
        "summary": {
            name.split("_")[0] + "".join(part.title() for part in name.split("_")[1:]):
                getattr(results.summary, name)
            for name in fields
        },
    }


def rules_snapshot(rules):
    return [
        {
            "id": rule.id.value,
            "kind": core_name(rule.kind),
            "detectionMode": core_name(rule.detection_mode),
            "severity": rule.severity.name.lower(),
            "remediation": None if rule.remediation is None else {
                "label": rule.remediation.label, "message": rule.remediation.message,
            },
        }
        for rule in rules
    ]


def error_snapshot(error):
    """Project structured public errors to the direct Core oracle's schema."""
    result = {"code": core_name(error.code), "detail": error.detail}
    if isinstance(error, cribra.ScannerBuildError):
        result["ruleId"] = None if error.rule_id is None else error.rule_id.value
    else:
        result.update(limit=error.limit, required=error.required)
        if isinstance(error, cribra.IncrementalError):
            result.update(terminal=error.terminal, expectedRevision=error.expected_revision,
                          actualRevision=error.actual_revision)
    return result


def stream_limits(**overrides):
    return cribra.StreamLimits(**{
        "max_candidates": None, "max_stream_storage_bytes": None,
        "max_retained_results": None, **overrides,
    })


def incremental_limits(**overrides):
    return cribra.IncrementalLimits(**{
        "max_candidates": None, "max_retained_cache_records": None,
        "max_materialized_results": None, "max_transaction_storage_bytes": None,
        **overrides,
    })


def single_rule_config(value, rule_id="literal", kind=cribra.RuleKind.LITERAL):
    return cribra.ScannerConfig.CUSTOM_ONLY(rules=[cribra.RuleSpec(
        id=cribra.RuleId(value=rule_id), kind=kind, value=value,
        severity=cribra.Severity.HIGH, remediation=None,
    )])


class ConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.generated = Path(sys.argv[1]).resolve()
        cls.profile = sys.argv[4] if len(sys.argv) > 4 else "core"
        fixture = Path(sys.argv[2])
        cls.inputs = json.loads(fixture.read_text(encoding="utf-8"))
        cls.configurations = json.loads(fixture.with_name("configurations.json").read_text(encoding="utf-8"))
        cls.incremental = json.loads(fixture.with_name("incremental.json").read_text(encoding="utf-8"))
        cls.oracle = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
        cls.expected = cls.oracle["reports"]
        cls.config_oracle = {case["name"]: case for case in cls.oracle["configurations"]}

    @staticmethod
    def source(value):
        return cribra.ScanSource(id=cribra.SourceId(value=value["id"]), text=value["text"])

    @staticmethod
    def config(case):
        rules = [cribra.RuleSpec(
            id=cribra.RuleId(value=rule["id"]),
            kind=cribra.RuleKind[rule["kind"].upper()], value=rule["value"],
            severity=cribra.Severity[rule["severity"].upper()],
            remediation=(None if "remediation" not in rule else
                         {"removeSensitiveValue": cribra.Remediation.REMOVE_SENSITIVE_VALUE}[
                             rule["remediation"]]),
        ) for rule in case["rules"]]
        return {
            "builtInOnly": lambda: cribra.ScannerConfig.BUILT_IN_ONLY(),
            "customOnly": lambda: cribra.ScannerConfig.CUSTOM_ONLY(rules=rules),
            "builtInAndCustom": lambda: cribra.ScannerConfig.BUILT_IN_AND_CUSTOM(rules=rules),
        }[case["mode"]]()

    def configured(self, name):
        case = next(case for case in self.configurations if case["name"] == name)
        return cribra.Scanner.configured(self.config(case))

    def assert_core_error(self, error_type, expected, operation, *args):
        with self.assertRaises(error_type) as caught:
            operation(*args)
        self.assertEqual(error_snapshot(caught.exception), expected)

    def incremental_view(self, session):
        snapshot = session.snapshot()
        # Legacy accessors remain compatible with the atomic snapshot.
        self.assertEqual(session.revision(), snapshot.revision)
        self.assertEqual(report_snapshot(session.report()), report_snapshot(snapshot.report))
        return {"revision": snapshot.revision, "report": report_snapshot(snapshot.report),
                "state": core_name(session.state())}

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

    def test_scan_many_order_ids_summary_and_repetition(self):
        scanner = cribra.Scanner()
        expected = {"reports": self.expected, "summary": self.oracle["summary"]}
        for _ in range(3):
            results = scanner.scan_many([self.source(value) for value in self.inputs])
            self.assertEqual(results_snapshot(results), expected)
            self.assertEqual([report.source_id.value for report in results.reports],
                             [value["id"] for value in self.inputs])
            self.assertEqual([report_snapshot(scanner.scan(self.source(value)))
                              for value in self.inputs], expected["reports"])

    def test_scan_many_empty_and_single_batches(self):
        scanner = cribra.Scanner()
        empty = results_snapshot(scanner.scan_many([]))
        self.assertEqual(empty["reports"], [])
        self.assertTrue(all(value == 0 for value in empty["summary"].values()))
        for value, expected in zip(self.inputs, self.expected):
            with self.subTest(source_id=value["id"]):
                result = scanner.scan_many([self.source(value)])
                self.assertEqual([report_snapshot(report) for report in result.reports], [expected])
                summary = result.summary
                self.assertEqual(summary.scanned_sources, 1)
                self.assertEqual(summary.scanned_bytes, expected["sourceBytes"])
                self.assertEqual(summary.total_findings, len(expected["findings"]))
                self.assertEqual(summary.total_candidates, expected["candidateCount"])

    def test_candidates_are_not_findings(self):
        result = cribra.Scanner().scan_many([self.source(self.inputs[3])])
        report = result.reports[0]
        self.assertEqual(report_snapshot(report), self.expected[3])
        self.assertEqual(report.findings, [])
        self.assertEqual(report.candidate_count, 1)
        self.assertTrue(report.needs_review)
        self.assertEqual(result.summary.total_findings, 0)
        self.assertEqual(result.summary.reports_with_findings, 0)
        self.assertEqual(result.summary.reports_without_findings, 1)
        self.assertEqual(result.summary.reports_with_candidates, 1)
        self.assertEqual(result.summary.total_candidates, 1)

    def test_configured_rulesets_and_all_matcher_kinds_match_core(self):
        for case in self.configurations:
            expected = self.config_oracle[case["name"]]
            if "error" in expected:
                continue
            with self.subTest(case=case["name"]):
                scanner = cribra.Scanner.configured(self.config(case))
                source = self.source({"id": case["name"], "text": case["source"]})
                self.assertEqual(results_snapshot(scanner.scan_many([source])), expected["results"])
                self.assertEqual(report_snapshot(scanner.scan(source)), expected["results"]["reports"][0])

    def test_active_metadata_order_and_remediation_match_core(self):
        for case in self.configurations:
            expected = self.config_oracle[case["name"]]
            if "error" in expected:
                continue
            with self.subTest(case=case["name"]):
                scanner = cribra.Scanner.configured(self.config(case))
                for _ in range(3):
                    self.assertEqual(rules_snapshot(scanner.active_rules()), expected["rules"])
        custom = self.configured("custom").active_rules()
        self.assertEqual({rule.kind for rule in custom}, set(cribra.RuleKind))
        self.assertIsNotNone(custom[0].remediation)
        self.assertTrue(custom[0].remediation.message)

    def test_empty_custom_and_combined_configs_add_no_implicit_rules(self):
        source = self.source(self.inputs[0])
        default = cribra.Scanner()
        builtins = self.configured("builtins")
        mixed_empty = cribra.Scanner.configured(cribra.ScannerConfig.BUILT_IN_AND_CUSTOM(rules=[]))
        for scanner in (builtins, mixed_empty):
            self.assertEqual(report_snapshot(scanner.scan(source)), report_snapshot(default.scan(source)))
            self.assertEqual(rules_snapshot(scanner.active_rules()), rules_snapshot(default.active_rules()))
        empty = self.configured("no-rules")
        self.assertEqual(empty.active_rules(), [])
        self.assertEqual(empty.scan(source).findings, [])
        custom = self.configured("custom")
        self.assertEqual(custom.scan(source).findings, [])
        # Custom-only still runs Core candidate detection; no built-ins are implied.
        candidate = custom.scan(self.source(self.inputs[3]))
        self.assertEqual(candidate.findings, [])
        self.assertEqual(candidate.candidate_count, 1)

    def test_configuration_errors_codes_rule_ids_details_and_precedence(self):
        for case in self.configurations:
            expected = self.config_oracle[case["name"]]
            if "error" not in expected:
                continue
            with self.subTest(case=case["name"]):
                self.assert_core_error(cribra.ScannerBuildError, expected["error"],
                                       cribra.Scanner.configured, self.config(case))

    def test_stream_every_byte_boundary_and_chunk_sizes_match_core(self):
        case = next(case for case in self.configurations if case["name"] == "custom")
        scanner = self.configured("custom")
        source = self.source({"id": "custom", "text": case["source"]})
        expected = self.config_oracle["custom"]["results"]["reports"][0]
        data = source.text.encode("utf-8")
        for split in range(len(data) + 1):
            with self.subTest(split=split):
                session = scanner.stream(source.id)
                self.assertEqual(session.state(), cribra.StreamState.ACTIVE)
                for chunk in (data[:split], b"", data[split:]):
                    self.assertIsNone(session.push(chunk))
                self.assertEqual(report_snapshot(session.finish()), expected)
                self.assertEqual(session.state(), cribra.StreamState.FINISHED)
        # Include a candidate alongside findings for whole-source/stream parity.
        source.text += "ABCD-EFGH-IJKL-MNOP"
        data = source.text.encode("utf-8")
        expected = report_snapshot(scanner.scan(source))
        self.assertEqual(expected["candidateCount"], 1)
        for size in (1, 2, 3, 7, 31, len(data)):
            with self.subTest(chunk_size=size):
                session = scanner.stream_with_limits(source.id, stream_limits())
                for offset in range(0, len(data), size):
                    session.push(data[offset:offset + size])
                self.assertEqual(report_snapshot(session.finish()), expected)

    def test_stream_empty_input_and_terminal_completion(self):
        for scanner in (self.configured("custom"), self.configured("no-rules")):
            source = self.source({"id": "空🙂", "text": ""})
            session = scanner.stream(source.id)
            self.assertIsNone(session.push(b""))
            self.assertEqual(report_snapshot(session.finish()), report_snapshot(scanner.scan(source)))
            for operation, args in ((session.finish, ()), (session.push, (b"",))):
                self.assert_core_error(cribra.StreamError, self.oracle["streamErrors"]["notActive"],
                                       operation, *args)
            self.assertEqual(session.state(), cribra.StreamState.FINISHED)

    def test_stream_unsupported_rulesets_without_fallback(self):
        scanners = [cribra.Scanner(), self.configured("builtins"), self.configured("mixed"),
                    cribra.Scanner.configured(single_rule_config("a+", kind=cribra.RuleKind.PATTERN))]
        for scanner in scanners:
            self.assert_core_error(cribra.StreamError, self.oracle["streamErrors"]["unsupported"],
                                   scanner.stream, cribra.SourceId(value="unsupported"))

    def assert_failed_stream(self, session):
        self.assertEqual(session.state(), cribra.StreamState.FAILED)
        for operation, args in ((session.push, (b"",)), (session.finish, ())):
            self.assert_core_error(cribra.StreamError, self.oracle["streamErrors"]["notActive"],
                                   operation, *args)
        self.assertEqual(session.state(), cribra.StreamState.FAILED)

    def test_stream_invalid_and_incomplete_utf8_are_terminal(self):
        scanner = cribra.Scanner.configured(single_rule_config("SECRET"))
        for data in (b"\xff", b"\xc0\xaf", b"\xed\xa0\x80", b"\xe2\x28\xa1"):
            with self.subTest(invalid=data):
                session = scanner.stream(cribra.SourceId(value="invalid"))
                session.push(b"SECRET ")
                self.assert_core_error(cribra.StreamError, self.oracle["streamErrors"]["invalidUtf8"],
                                       session.push, data)
                self.assert_failed_stream(session)
        for data in (b"\xc3", b"\xf0\x9f", b"\xe2\x82"):
            with self.subTest(incomplete=data):
                session = scanner.stream(cribra.SourceId(value="incomplete"))
                session.push(data)
                self.assertEqual(session.state(), cribra.StreamState.ACTIVE)
                self.assert_core_error(cribra.StreamError, self.oracle["streamErrors"]["incompleteUtf8"],
                                       session.finish)
                self.assert_failed_stream(session)

    def test_stream_resource_limits_at_admission_push_and_eof(self):
        scanner = cribra.Scanner.configured(single_rule_config("SECRET"))
        source_id = cribra.SourceId(value="limits")
        errors = self.oracle["streamErrors"]
        self.assert_core_error(cribra.StreamError, errors["storage"], scanner.stream_with_limits,
                               source_id, stream_limits(max_stream_storage_bytes=0))
        # Use Core's reported storage requirement, not a platform-dependent constant.
        exact = scanner.stream_with_limits(source_id, stream_limits(
            max_stream_storage_bytes=errors["storage"]["required"]))
        exact.push(b"SECRET")
        self.assertEqual(report_snapshot(exact.finish()), report_snapshot(scanner.scan(
            cribra.ScanSource(id=source_id, text="SECRET"))))
        for name, limits in (("candidate", stream_limits(max_candidates=1)),
                             ("result", stream_limits(max_retained_results=1))):
            with self.subTest(limit=name):
                session = scanner.stream_with_limits(source_id, limits)
                session.push(b"SECRET ")
                self.assert_core_error(cribra.StreamError, errors[name], session.push, b"SECRET ")
                self.assert_failed_stream(session)
        eof_cases = (
            ("eofResult", self.configured("no-rules"), b"ABCD-EFGH-IJKL-MNOP",
             stream_limits(max_retained_results=0)),
            ("eofCandidate", cribra.Scanner.configured(single_rule_config(
                r"abc\z", rule_id="eof", kind=cribra.RuleKind.PATTERN)), b"abc",
             stream_limits(max_candidates=0)),
        )
        for name, scanner, data, limits in eof_cases:
            with self.subTest(eof=name):
                session = scanner.stream_with_limits(source_id, limits)
                session.push(data)
                self.assertEqual(session.state(), cribra.StreamState.ACTIVE)
                self.assert_core_error(cribra.StreamError, errors[name], session.finish)
                self.assert_failed_stream(session)

    def test_incremental_initialization_for_all_rulesets_matches_scan(self):
        for name in ("builtins", "custom", "mixed", "no-rules"):
            scanner = self.configured(name)
            for value in self.inputs:
                with self.subTest(config=name, source_id=value["id"]):
                    source = self.source(value)
                    for session in (scanner.incremental(source), scanner.incremental_with_limits(
                            source, incremental_limits())):
                        self.assertEqual(self.incremental_view(session), {
                            "revision": 0, "state": "Active",
                            "report": report_snapshot(scanner.scan(source)),
                        })

    def test_incremental_edits_revisions_recovery_and_snapshots_match_core(self):
        scanner = self.configured("custom")
        fixture = self.incremental
        source = self.source({"id": fixture["id"], "text": fixture["source"]})
        session = scanner.incremental(source)
        trace = self.oracle["incremental"]["trace"]
        self.assertEqual(len(trace), len(fixture["edits"]) + 1)
        self.assertEqual(self.incremental_view(session), trace[0]["snapshot"])
        data = source.text.encode("utf-8")
        successes = failures = 0
        for edit, expected in zip(fixture["edits"], trace[1:]):
            with self.subTest(edit=edit):
                before = self.incremental_view(session)
                args = (edit["revision"], edit["start"], edit["end"], edit["replacement"])
                if expected["error"] is not None:
                    self.assert_core_error(cribra.IncrementalError, expected["error"],
                                           session.apply_edit, *args)
                    self.assertEqual(self.incremental_view(session), before)
                    failures += 1
                else:
                    self.assertEqual(session.apply_edit(*args), expected["newRevision"])
                    # Apply only accepted byte edits to construct the full-scan input.
                    # No validation or detection is performed in Python.
                    data = data[:edit["start"]] + edit["replacement"].encode("utf-8") + data[edit["end"]:]
                    successes += 1
                self.assertEqual(self.incremental_view(session), expected["snapshot"])
                source.text = data.decode("utf-8")
                self.assertEqual(report_snapshot(session.report()), report_snapshot(scanner.scan(source)))
        self.assertEqual((successes, failures), (7, 4))
        self.assertEqual(session.revision(), 7)  # Includes two accepted no-op edits.
        self.assertEqual(session.report().candidate_count, 1)

    def test_incremental_terminal_limits_preserve_last_snapshot(self):
        fields = {
            "candidates": "max_candidates", "cache": "max_retained_cache_records",
            "results": "max_materialized_results", "storage": "max_transaction_storage_bytes",
        }
        replacements = {"candidates": "xx", "cache": "", "results": "x", "storage": "a"}
        for expected in self.oracle["incremental"]["resources"]:
            name = expected["name"]
            with self.subTest(limit=name):
                scanner = (self.configured("no-rules") if name == "storage" else
                           cribra.Scanner.configured(single_rule_config("x")))
                initial = "" if name == "storage" else "x"
                source = self.source({"id": self.incremental["id"], "text": initial})
                limits = incremental_limits(**{fields[name]: 0 if name == "storage" else 1})
                session = scanner.incremental_with_limits(source, limits)
                if name == "storage":
                    self.assertEqual(session.apply_edit(0, 0, 0, ""), 1)
                self.assertEqual(self.incremental_view(session), expected["before"])
                self.assert_core_error(cribra.IncrementalError, expected["error"],
                                       session.apply_edit, session.revision(), 0, 0, replacements[name])
                self.assertEqual(self.incremental_view(session), expected["after"])
                # NotActive wins over stale revision and invalid ranges after failure.
                for _ in range(3):
                    self.assert_core_error(cribra.IncrementalError, expected["laterError"],
                                           session.apply_edit, 2**64 - 1, 999, 0, "")
                    self.assertEqual(self.incremental_view(session), expected["after"])
                self.assertEqual(expected["after"]["report"], expected["before"]["report"])
                self.assertEqual(session.state(), cribra.IncrementalState.FAILED)

    def test_incremental_construction_limits_match_core(self):
        scanner = cribra.Scanner.configured(single_rule_config("x"))
        source = self.source({"id": self.incremental["id"], "text": "x"})
        fields = {
            "candidates": "max_candidates", "cache": "max_retained_cache_records",
            "results": "max_materialized_results", "storage": "max_transaction_storage_bytes",
        }
        for expected in self.oracle["incremental"]["construction"]:
            with self.subTest(limit=expected["name"]):
                self.assert_core_error(cribra.IncrementalError, expected["error"],
                                       scanner.incremental_with_limits, source,
                                       incremental_limits(**{fields[expected["name"]]: 0}))

    def test_sessions_and_owned_results_survive_scanner_and_session_gc(self):
        expected = self.oracle["incremental"]["trace"][0]["snapshot"]["report"]
        for _ in range(3):
            scanner = self.configured("custom")
            source = self.source({"id": self.incremental["id"], "text": self.incremental["source"]})
            stream = scanner.stream(source.id)
            incremental = scanner.incremental(source)
            scanner_ref = weakref.ref(scanner)
            del scanner
            gc.collect()
            self.assertIsNone(scanner_ref())
            self.assertIsNone(stream.push(source.text.encode("utf-8")))
            report = stream.finish()
            snapshot = incremental.snapshot()
            self.assertEqual(incremental.apply_edit(0, 0, 0, ""), 1)
            self.assertEqual(incremental.revision(), 1)
            stream_ref, incremental_ref = weakref.ref(stream), weakref.ref(incremental)
            del stream, incremental, source
            gc.collect()
            self.assertIsNone(stream_ref())
            self.assertIsNone(incremental_ref())
            self.assertEqual(report_snapshot(report), expected)
            self.assertEqual(report_snapshot(snapshot.report), expected)
            self.assertEqual(snapshot.revision, 0)

    def test_returned_dtos_do_not_mutate_session_or_other_calls(self):
        scanner = self.configured("custom")
        source = self.source({"id": self.incremental["id"], "text": self.incremental["source"]})
        session = scanner.incremental(source)
        expected = self.incremental_view(session)
        detached = session.snapshot()
        detached.report.source_id.value = "changed"
        detached.report.findings.clear()
        self.assertEqual(self.incremental_view(session), expected)
        batch = scanner.scan_many([source, source])
        expected_report = report_snapshot(batch.reports[1])
        batch.reports[0].findings.clear()
        self.assertEqual(report_snapshot(batch.reports[1]), expected_report)
        self.assertEqual(report_snapshot(scanner.scan(source)), expected_report)

    def test_profile_feature_boundary(self):
        for name in (
            "redact", "template", "pseudonymize", "synthesize", "scan_and_build", "build",
            "ByteSpan", "RedactOptions", "TemplateOptions", "PseudonymizationOptions",
            "SynthesisOptions", "TransformResult", "TransformError", "TransformErrorCode",
            "ShareMode", "ShareModeKind", "ShareBundleOptions", "ShareBundle", "ShareManifest",
            "ShareTimestamp", "ShareError", "ShareErrorCode", "ShareReport", "ShareCandidate",
            "ShareCandidateKind", "ShareCandidateEvidence", "TransformedSource",
        ):
            with self.subTest(api=name):
                self.assertEqual(hasattr(cribra, name), self.profile in ("flow", "fuse"))
        self.assertEqual(hasattr(cribra.Scanner, "scan_for_share"), self.profile in ("flow", "fuse"))
        self.assertEqual(hasattr(cribra.Scanner, "parallel_scan"), self.profile == "fuse")


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
