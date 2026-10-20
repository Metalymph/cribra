package cribra.kotlin.smoke

import uniffi.cribra_uniffi.Confidence
import uniffi.cribra_uniffi.RuleId
import uniffi.cribra_uniffi.RuleKind
import uniffi.cribra_uniffi.RuleSpec
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.ScannerConfig
import uniffi.cribra_uniffi.Severity
import uniffi.cribra_uniffi.SourceId
import uniffi.cribra_uniffi.cribraVersion
import uniffi.cribra_uniffi.smokeScanText

// A synthetic, non-reversible GitHub classic PAT shape. It is not a real
// credential and is only used to drive the built-in detector.
private const val SYNTHETIC_PAT = "ghp_AbCdEf0123456789_AbCdEf0123456789"

// CRLF plus a multi-byte scalar, a surrogate pair and a combining mark so the
// Core's UTF-8 byte offsets and Unicode-scalar columns are exercised exactly.
private const val UNICODE_PREFIX = "前\r\né🙂e\u0301 GITHUB_TOKEN="

private fun utf8Length(text: String): ULong = text.toByteArray(Charsets.UTF_8).size.toULong()

fun main() {
    // 1. Version query and top-level Core entry points over the real native library.
    val version = cribraVersion()
    check(version.isNotEmpty()) { "Core version is empty" }
    check(smokeScanText("").isEmpty()) { "empty source produced findings" }
    check(smokeScanText("ordinary text: é🙂").isEmpty()) { "clean source produced findings" }

    // 2. Built-in detection with exact UTF-8 geometry across a multi-byte prefix.
    val source = UNICODE_PREFIX + SYNTHETIC_PAT + "\n"
    val findings = smokeScanText(source)
    check(findings.size == 1) { "expected exactly one finding, got ${findings.size}" }
    val finding = findings.single()
    check(finding.ruleId.value == "github.classic-pat") { "unexpected RuleId ${finding.ruleId.value}" }
    check(finding.severity == Severity.CRITICAL) { "unexpected severity ${finding.severity}" }
    check(finding.confidence == Confidence.HIGH) { "unexpected confidence ${finding.confidence}" }
    check(finding.location.start == 28UL) { "start ${finding.location.start} != 28" }
    check(finding.location.end == 28UL + utf8Length(SYNTHETIC_PAT)) { "end ${finding.location.end} mismatch" }
    check(finding.location.line == 2UL) { "line ${finding.location.line} != 2" }
    check(finding.location.column == 19UL) { "column ${finding.location.column} != 19" }
    println("PASS: Kotlin -> UniFFI -> Cribra Core $version; version, built-in detection and UTF-8 coordinates")

    // 3. Reusable Scanner object: single/batch parity, Unicode and duplicate IDs,
    //    empty input and reuse without retained state.
    Scanner().use { scanner ->
        val sources = listOf(
            ScanSource(SourceId("文書🙂"), "é🙂e\u0301\r\nSECRET"),
            ScanSource(SourceId("文書🙂"), "SECRET + SECRET"),
            ScanSource(SourceId(""), ""),
            ScanSource(SourceId("review"), "ABCD-EFGH-IJKL-MNOP"),
        )
        val batch = scanner.scanMany(sources)
        check(batch.reports.size == sources.size) { "report cardinality mismatch" }
        for ((input, report) in sources.zip(batch.reports)) {
            check(report.sourceId.value == input.id.value) { "source id changed" }
            check(report.sourceBytes == utf8Length(input.text)) { "byte length mismatch" }
            val single = scanner.scan(input)
            check(single.sourceId.value == report.sourceId.value) { "single/batch source id mismatch" }
            check(single.sourceBytes == report.sourceBytes) { "single/batch byte length mismatch" }
            check(single.findings.map { it.location } == report.findings.map { it.location }) {
                "single/batch findings mismatch"
            }
        }
        val again = scanner.scanMany(sources)
        check(again.summary == batch.summary) { "reuse changed the summary" }
        check(again.reports.map { it.findings.map { f -> f.ruleId.value } } ==
            batch.reports.map { it.findings.map { f -> f.ruleId.value } }) { "reuse changed the findings" }
        val empty = scanner.scanMany(emptyList())
        check(empty.reports.isEmpty()) { "empty batch produced reports" }
        check(empty.summary.scannedSources == 0UL && empty.summary.totalFindings == 0UL) {
            "empty batch retained state"
        }
        // Candidate-only semantics: no findings is not the same as clean.
        val review = batch.reports[3]
        check(review.findings.isEmpty() && review.candidateCount == 1UL && review.needsReview) {
            "candidate-only review semantics changed"
        }
        println("PASS: reusable Scanner; single/batch parity, Unicode and duplicate IDs, empty input, Core summary")
    }

    // 4. Explicit configuration: a custom-only rule drives a real Core scan.
    val rule = RuleSpec(
        id = RuleId("secret"),
        kind = RuleKind.LITERAL,
        value = "SECRET",
        severity = Severity.HIGH,
        remediation = null,
    )
    Scanner.configured(ScannerConfig.CustomOnly(listOf(rule))).use { scanner ->
        val metadata = scanner.activeRules()
        check(metadata.size == 1 && metadata.single().id.value == "secret") { "active rules mismatch" }
        check(metadata.single().kind == RuleKind.LITERAL) { "rule kind mismatch" }
        val report = scanner.scan(ScanSource(SourceId("custom"), "before SECRET after"))
        check(report.findings.size == 1) { "custom detection failed" }
        check(report.findings.single().ruleId.value == "secret") { "custom rule id mismatch" }
        println("PASS: configured Scanner; custom-only rules, active metadata and real Core scan")
    }

    println("PASS: Kotlin/JVM consumer smoke complete against the native Cribra Core library")
}
