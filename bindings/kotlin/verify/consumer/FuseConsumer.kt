package cribra.kotlin.verify.consumer

import cribra.kotlin.profile.FuseProfile
import uniffi.cribra_uniffi.ScanResults
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.ScannerConfig
import uniffi.cribra_uniffi.SourceId

// Synthetic, non-reversible GitHub classic PAT shape, matching the Fuse smoke.
private const val SYNTHETIC_PAT = "ghp_AbCdEf0123456789_AbCdEf0123456789"

/** Stable textual projection of every report, finding and summary field. */
private fun snapshot(results: ScanResults): String = buildString {
    for (report in results.reports) {
        append(report.sourceId.value).append('|')
            .append(report.sourceBytes).append('|')
            .append(report.candidateCount).append('|')
            .append(report.needsReview).append('|')
        for (finding in report.findings) {
            append(finding.ruleId.value).append(':')
                .append(finding.location.start).append(':')
                .append(finding.location.end).append(':')
                .append(finding.location.line).append(':')
                .append(finding.location.column).append(':')
                .append(finding.severity).append(':')
                .append(finding.confidence).append(';')
        }
        append('\n')
    }

    val summary = results.summary
    append(summary.scannedSources).append('|')
        .append(summary.scannedBytes).append('|')
        .append(summary.reportsWithFindings).append('|')
        .append(summary.reportsWithoutFindings).append('|')
        .append(summary.totalFindings).append('|')
        .append(summary.totalCandidates).append('|')
        .append(summary.reportsWithCandidates).append('|')
        .append(summary.critical).append('|')
        .append(summary.high).append('|')
        .append(summary.medium).append('|')
        .append(summary.low).append('|')
        .append(summary.info)
}

fun main(args: Array<String>) {
    require(args.size == 1) { "usage: FuseConsumerKt <expected-native-library>" }

    FuseProfile.requireExclusiveClasspath()
    assertBundleNative(args[0])

    Scanner.configured(ScannerConfig.BuiltInOnly).use { scanner ->
        val sources = listOf(
            ScanSource(SourceId("alpha"), "clean text"),
            ScanSource(SourceId("beta"), "token=$SYNTHETIC_PAT\n"),
            ScanSource(SourceId("文書🙂"), "clean text"),
            ScanSource(SourceId("beta"), "token=$SYNTHETIC_PAT\n"),
        )

        val parallel = scanner.parallelScan(sources)
        val sequential = scanner.scanMany(sources)

        check(parallel.reports.size == sources.size) {
            "parallelScan returned ${parallel.reports.size} reports for ${sources.size} sources"
        }
        check(snapshot(parallel) == snapshot(sequential)) {
            "parallelScan/scanMany parity mismatch"
        }
        check(parallel.summary.totalFindings == 2UL) {
            "expected two findings, got ${parallel.summary.totalFindings}"
        }
        check(parallel.reports[1].findings.single().ruleId.value == "github.classic-pat") {
            "unexpected beta rule"
        }
        check(parallel.reports[1].sourceId.value == parallel.reports[3].sourceId.value) {
            "duplicate source ids were not preserved"
        }

        val empty = scanner.parallelScan(emptyList())
        check(empty.reports.isEmpty() && empty.summary.scannedSources == 0UL) {
            "empty parallelScan did not produce an empty result"
        }

        println(
            "PASS: isolated Fuse consumer; parallelScan parity reports=${parallel.reports.size} " +
                "findings=${parallel.summary.totalFindings}",
        )
    }
}
