package cribra.kotlin.verify.consumer

import cribra.kotlin.profile.CoreProfile
import uniffi.cribra_uniffi.Explanation
import uniffi.cribra_uniffi.DetectionMode
import uniffi.cribra_uniffi.Remediation
import uniffi.cribra_uniffi.CandidateEvidence
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.Severity
import uniffi.cribra_uniffi.SourceId
import uniffi.cribra_uniffi.cribraVersion

// Synthetic, non-reversible GitHub classic PAT shape, matching the Core smoke.
private const val SYNTHETIC_PAT = "ghp_AbCdEf0123456789_AbCdEf0123456789"

fun main(args: Array<String>) {
    require(args.size == 1) { "usage: CoreConsumerKt <expected-native-library>" }

    CoreProfile.requireExclusiveClasspath()
    assertBundleNative(args[0])
    val version = cribraVersion()
    check(version == CoreProfile.CORE_VERSION) {
        "Core version $version does not match ${CoreProfile.CORE_VERSION}"
    }

    val source = "token=$SYNTHETIC_PAT\n"
    Scanner().use { scanner ->
        val report = scanner.scan(ScanSource(SourceId("isolated-core"), source))
        check(report.sourceId.value == "isolated-core") { "source id changed" }
        check(report.sourceBytes == source.toByteArray(Charsets.UTF_8).size.toULong()) {
            "source byte length changed"
        }
        check(report.findings.size == 1) { "expected one finding, got ${report.findings.size}" }

        val finding = report.findings.single()
        check(finding.ruleId.value == "github.classic-pat") {
            "unexpected rule ${finding.ruleId.value}"
        }
        check(finding.remediation == Remediation.REVOKE_AND_ROTATE_CREDENTIAL)
        check(finding.explanation == Explanation.Classified(DetectionMode.DETERMINISTIC))
        val review = scanner.scan(ScanSource(SourceId("review"), "é🙂 ABCD-EFGH-IJKL-MNOP"))
        check(review.candidateCount == 1UL && review.needsReview && review.findings.isEmpty())
        check(review.candidates.single().location.start == 7UL)
        check(review.candidates.single().explanation == Explanation.Ambiguous(CandidateEvidence.STRUCTURAL))
        check(finding.severity == Severity.CRITICAL) {
            "unexpected severity ${finding.severity}"
        }
    }

    println("PASS: isolated Core consumer; version=$version findings=1")
}
