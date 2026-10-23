// Real Kotlin/JVM consumer for the UniFFI Fuse profile: Core-owned parallel
// scanning. It replays the shared parallel.json fixture and the direct-Core
// oracle output from the existing `parallel_oracle` Rust example, checking
// parallelScan against scanMany and against Core itself, across every fixture
// configuration and batch shape.

package cribra.kotlin.fuse

import com.google.gson.JsonNull
import uniffi.cribra_uniffi.Explanation
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.google.gson.JsonPrimitive
import uniffi.cribra_uniffi.Confidence
import uniffi.cribra_uniffi.RuleId
import uniffi.cribra_uniffi.RuleKind
import uniffi.cribra_uniffi.RuleSpec
import uniffi.cribra_uniffi.ScanResults
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.ScanSummary
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.ScannerConfig
import uniffi.cribra_uniffi.Severity
import uniffi.cribra_uniffi.SourceId
import java.io.File
import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets.UTF_8

private val MODES = listOf("builtInOnly", "customOnly", "builtInAndCustom", "emptyRules")
private const val REPEATS = 8

// ---- JSON helpers -----------------------------------------------------------

private fun parseFile(file: File): JsonElement = JsonParser.parseString(file.readText(UTF_8))

private fun JsonObject.str(key: String): String =
    get(key)?.takeIf { !it.isJsonNull }?.asString ?: error("missing string $key")

private fun JsonObject.obj(key: String): JsonObject = get(key)?.asJsonObject ?: error("missing object $key")

private fun JsonObject.arr(key: String): JsonArray = get(key)?.asJsonArray ?: error("missing array $key")

private fun JsonElement.objects(): List<JsonObject> = asJsonArray.map { it.asJsonObject }

private fun number(value: ULong): JsonPrimitive = JsonPrimitive(BigInteger(value.toString()))

private fun objectOf(vararg pairs: Pair<String, JsonElement>): JsonObject =
    JsonObject().apply { pairs.forEach { (key, value) -> add(key, value) } }

/** Structural equality that compares numbers by value, not by parser identity. */
private fun jsonEquals(a: JsonElement, b: JsonElement): Boolean {
    if (a.isJsonNull || b.isJsonNull) return a.isJsonNull && b.isJsonNull
    if (a.isJsonObject && b.isJsonObject) {
        val x = a.asJsonObject
        val y = b.asJsonObject
        if (x.size() != y.size()) return false
        return x.keySet().all { key -> y.has(key) && jsonEquals(x.get(key), y.get(key)) }
    }
    if (a.isJsonArray && b.isJsonArray) {
        val x = a.asJsonArray
        val y = b.asJsonArray
        if (x.size() != y.size()) return false
        return (0 until x.size()).all { index -> jsonEquals(x.get(index), y.get(index)) }
    }
    if (a.isJsonPrimitive && b.isJsonPrimitive) {
        val x = a.asJsonPrimitive
        val y = b.asJsonPrimitive
        if (x.isNumber && y.isNumber) return BigDecimal(x.asString).compareTo(BigDecimal(y.asString)) == 0
        if (x.isBoolean && y.isBoolean) return x.asBoolean == y.asBoolean
        if (x.isString && y.isString) return x.asString == y.asString
    }
    return false
}

private fun requireJson(actual: JsonElement, expected: JsonElement, name: String) {
    check(jsonEquals(actual, expected)) {
        "mismatch for $name:\n  actual   = $actual\n  expected = $expected"
    }
}

// ---- DTO projection ---------------------------------------------------------

private fun severity(name: String): Severity = when (name) {
    "info" -> Severity.INFO
    "low" -> Severity.LOW
    "medium" -> Severity.MEDIUM
    "high" -> Severity.HIGH
    "critical" -> Severity.CRITICAL
    else -> error("unknown severity $name")
}

private fun severityName(value: Severity): String = when (value) {
    Severity.INFO -> "info"
    Severity.LOW -> "low"
    Severity.MEDIUM -> "medium"
    Severity.HIGH -> "high"
    Severity.CRITICAL -> "critical"
}

private fun confidenceName(value: Confidence): String = when (value) {
    Confidence.LOW -> "low"
    Confidence.MEDIUM -> "medium"
    Confidence.HIGH -> "high"
}

private fun summaryJson(summary: ScanSummary): JsonObject = objectOf(
    "scanned_sources" to number(summary.scannedSources),
    "scanned_bytes" to number(summary.scannedBytes),
    "reports_with_findings" to number(summary.reportsWithFindings),
    "reports_without_findings" to number(summary.reportsWithoutFindings),
    "total_findings" to number(summary.totalFindings),
    "total_candidates" to number(summary.totalCandidates),
    "reports_with_candidates" to number(summary.reportsWithCandidates),
    "critical" to number(summary.critical),
    "high" to number(summary.high),
    "medium" to number(summary.medium),
    "low" to number(summary.low),
    "info" to number(summary.info),
)

private fun coreName(value: Enum<*>): String = value.name.lowercase().split('_').joinToString("") { it.replaceFirstChar(Char::uppercase) }
private fun explanation(value: Explanation?): JsonElement = when (value) {
    null -> JsonNull.INSTANCE
    is Explanation.Classified -> objectOf("classified" to JsonPrimitive(coreName(value.mode)))
    is Explanation.Ambiguous -> objectOf("ambiguous" to JsonPrimitive(coreName(value.evidence)))
}

private fun snapshot(results: ScanResults): JsonObject = objectOf(
    "reports" to JsonArray().apply {
        for (report in results.reports) {
            add(
                objectOf(
                    "sourceId" to JsonPrimitive(report.sourceId.value),
                    "sourceBytes" to number(report.sourceBytes),
                    "candidateCount" to number(report.candidateCount),
                    "needsReview" to JsonPrimitive(report.needsReview),
                    "candidates" to JsonArray().apply {
                        for (candidate in report.candidates) {
                            add(objectOf("kind" to JsonPrimitive(coreName(candidate.kind)),
                                "evidence" to JsonPrimitive(coreName(candidate.evidence)),
                                "start" to number(candidate.location.start), "end" to number(candidate.location.end),
                                "line" to number(candidate.location.line), "column" to number(candidate.location.column),
                                "explanation" to explanation(candidate.explanation)))
                        }
                    },
                    "findings" to JsonArray().apply {
                        for (finding in report.findings) {
                            add(
                                objectOf(
                                    "ruleId" to JsonPrimitive(finding.ruleId.value),
                                    "start" to number(finding.location.start),
                                    "end" to number(finding.location.end),
                                    "line" to number(finding.location.line),
                                    "column" to number(finding.location.column),
                                    "severity" to JsonPrimitive(severityName(finding.severity)),
                                    "confidence" to JsonPrimitive(confidenceName(finding.confidence)),
                                    "remediation" to (finding.remediation?.let { JsonPrimitive(coreName(it)) } ?: JsonNull.INSTANCE),
                                    "explanation" to explanation(finding.explanation),
                                ),
                            )
                        }
                    },
                ),
            )
        }
    },
    "summary" to summaryJson(results.summary),
)

private fun scannerFor(mode: String, rules: List<RuleSpec>): Scanner = when (mode) {
    "builtInOnly" -> Scanner.configured(ScannerConfig.BuiltInOnly)
    "customOnly" -> Scanner.configured(ScannerConfig.CustomOnly(rules))
    "builtInAndCustom" -> Scanner.configured(ScannerConfig.BuiltInAndCustom(rules))
    "emptyRules" -> Scanner.configured(ScannerConfig.CustomOnly(emptyList()))
    else -> error("unknown fixture configuration $mode")
}

private fun checkGolden(result: ScanResults) {
    val summary = result.summary
    require(listOf(summary.scannedSources, summary.reportsWithFindings, summary.reportsWithoutFindings) == listOf(7UL, 4UL, 3UL)) {
        "summary source counters changed"
    }
    require(listOf(summary.totalFindings, summary.totalCandidates, summary.reportsWithCandidates) == listOf(11UL, 1UL, 1UL)) {
        "summary finding totals changed"
    }
    require(listOf(summary.critical, summary.high, summary.medium, summary.low, summary.info) == listOf(4UL, 3UL, 1UL, 1UL, 2UL)) {
        "summary severities changed"
    }
    val candidate = result.reports[3]
    require(candidate.findings.isEmpty() && candidate.candidateCount == 1UL && candidate.needsReview) {
        "candidate-only review semantics changed"
    }
    require(!result.reports[5].sourceId.value.toByteArray(UTF_8).contentEquals(result.reports[6].sourceId.value.toByteArray(UTF_8))) {
        "distinct Unicode IDs collapsed"
    }
    val location = result.reports[0].findings[0].location
    require(listOf(location.start, location.end, location.line, location.column) == listOf(28UL, 65UL, 2UL, 19UL)) {
        "UTF-8 coordinates changed"
    }
}

fun main(args: Array<String>) {
    require(args.size == 2) { "usage: FuseSmokeKt <parallel.json> <parallel-oracle.json>" }
    val fixture = parseFile(File(args[0])).asJsonObject
    val oracle = parseFile(File(args[1])).asJsonArray.map { it.asJsonObject }

    val sources = fixture.arr("sources").objects().map { ScanSource(SourceId(it.str("id")), it.str("text")) }
    val rules = fixture.arr("rules").objects().map {
        RuleSpec(RuleId(it.str("id")), RuleKind.LITERAL, it.str("value"), severity(it.str("severity")), null)
    }

    // Same batch shapes as the Rust oracle and the Swift smoke.
    val batches = buildList {
        add("empty" to emptyList<ScanSource>())
        sources.forEachIndexed { index, source -> add("single-$index" to listOf(source)) }
        add("multiple" to sources)
        add("reversed" to sources.reversed())
        add("large" to (0 until 64).map { sources[it % sources.size] })
    }

    var count = 0
    for (mode in MODES) {
        val scanner = scannerFor(mode, rules)
        var retained: ScanResults? = null
        for ((name, inputs) in batches) {
            val entry = oracle[count]
            require(entry.str("mode") == mode && entry.str("batch") == name) { "oracle ordering mismatch at index $count" }
            val expected = entry.obj("outcome")
            repeat(REPEATS) {
                // All calls are synchronous; no Kotlin coroutines or thread pools.
                val parallel = scanner.parallelScan(inputs)
                requireJson(snapshot(parallel), expected, "$mode/$name parallel")
                requireJson(snapshot(scanner.scanMany(inputs)), expected, "$mode/$name sequential")
                require(parallel.reports.size == inputs.size) { "report cardinality mismatch for $mode/$name" }
                for ((source, report) in inputs.zip(parallel.reports)) {
                    require(source.id.value.toByteArray(UTF_8).contentEquals(report.sourceId.value.toByteArray(UTF_8))) {
                        "ID bytes/order changed for $mode/$name"
                    }
                    require(report.sourceBytes == source.text.toByteArray(UTF_8).size.toULong()) {
                        "recorded source length changed for $mode/$name"
                    }
                }
                retained = parallel
            }
            if (mode == "builtInAndCustom" && name == "multiple") checkGolden(retained!!)
            count += 1
        }
        // Releasing the Scanner must not invalidate the owned DTOs it returned.
        scanner.close()
        val owned = retained!!
        require(owned.reports.size == 64) { "owned results unusable after Scanner release" }
        require(owned.summary.scannedSources == 64UL) { "owned summary unusable after Scanner release" }
    }
    require(count == oracle.size) { "oracle coverage mismatch: $count entries" }
    println(
        "PASS: parallelScan fuse; $count Core scenarios x $REPEATS repeats, sequential parity, all DTO fields/counters, " +
            "Unicode/duplicate IDs, UTF-8 coordinates, candidates and owned results",
    )
}
