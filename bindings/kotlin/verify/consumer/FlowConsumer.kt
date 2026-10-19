package cribra.kotlin.verify.consumer

import cribra.kotlin.profile.FlowProfile
import uniffi.cribra_uniffi.RedactOptions
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.SourceId
import uniffi.cribra_uniffi.redact

// Synthetic, non-reversible GitHub classic PAT shape, matching the Flow smoke.
private const val SYNTHETIC_PAT = "ghp_AbCdEf0123456789_AbCdEf0123456789"

fun main(args: Array<String>) {
    require(args.size == 1) { "usage: FlowConsumerKt <expected-native-library>" }

    FlowProfile.requireExclusiveClasspath()
    assertBundleNative(args[0])

    val source = "GITHUB_TOKEN=$SYNTHETIC_PAT"
    val findingCount = Scanner().use { scanner ->
        val report = scanner.scan(ScanSource(SourceId("isolated-flow"), source))
        check(report.findings.isNotEmpty()) { "expected at least one finding" }

        val transformed = redact(source, report.findings, RedactOptions(null))
        check(transformed.content == "GITHUB_TOKEN=[REDACTED]") {
            "unexpected redaction result: ${transformed.content}"
        }
        report.findings.size
    }

    println("PASS: isolated Flow consumer; redact findings=$findingCount")
}
