// Real Kotlin/JVM consumer for the UniFFI Flow and Share APIs. It replays the
// shared repository fixtures and the direct Core oracle outputs produced by the
// existing `flow_oracle` and `share_oracle` Rust examples, exactly like the
// qualified Swift consumer, without reimplementing Core semantics here.

package cribra.kotlin.flow

import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonNull
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.google.gson.JsonPrimitive
import uniffi.cribra_uniffi.ByteSpan
import uniffi.cribra_uniffi.Confidence
import uniffi.cribra_uniffi.Finding
import uniffi.cribra_uniffi.Location
import uniffi.cribra_uniffi.PseudonymizationOptions
import uniffi.cribra_uniffi.RedactOptions
import uniffi.cribra_uniffi.RuleId
import uniffi.cribra_uniffi.RuleKind
import uniffi.cribra_uniffi.RuleSpec
import uniffi.cribra_uniffi.ScanSource
import uniffi.cribra_uniffi.ScanSummary
import uniffi.cribra_uniffi.Scanner
import uniffi.cribra_uniffi.ScannerConfig
import uniffi.cribra_uniffi.Severity
import uniffi.cribra_uniffi.ShareBundle
import uniffi.cribra_uniffi.ShareBundleOptions
import uniffi.cribra_uniffi.ShareErrorCode
import uniffi.cribra_uniffi.ShareException
import uniffi.cribra_uniffi.ShareMode
import uniffi.cribra_uniffi.ShareModeKind
import uniffi.cribra_uniffi.ShareReport
import uniffi.cribra_uniffi.SourceId
import uniffi.cribra_uniffi.SynthesisOptions
import uniffi.cribra_uniffi.TemplateOptions
import uniffi.cribra_uniffi.TransformErrorCode
import uniffi.cribra_uniffi.TransformException
import uniffi.cribra_uniffi.TransformResult
import uniffi.cribra_uniffi.build
import uniffi.cribra_uniffi.pseudonymize
import uniffi.cribra_uniffi.redact
import uniffi.cribra_uniffi.scanAndBuild
import uniffi.cribra_uniffi.synthesize
import uniffi.cribra_uniffi.template
import java.io.File
import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets.UTF_8

// Fixed PUBLIC test vectors, identical to the Swift smoke and the Rust oracle.
// Production APIs never generate, default or persist keys.
private const val KEY_A = "0123456789ABCDEF0123456789ABCDEF"
private const val KEY_B = "FEDCBA9876543210FEDCBA9876543210"

// ---- JSON helpers -----------------------------------------------------------

private fun parseFile(file: File): JsonElement = JsonParser.parseString(file.readText(UTF_8))

private fun JsonObject.str(key: String): String =
    get(key)?.takeIf { !it.isJsonNull }?.asString ?: error("missing string $key")

private fun JsonObject.optStr(key: String): String? = get(key)?.takeIf { !it.isJsonNull }?.asString

private fun JsonObject.optBool(key: String, default: Boolean): Boolean =
    get(key)?.takeIf { !it.isJsonNull }?.asBoolean ?: default

private fun JsonObject.int(key: String): Int = str(key).toInt()

private fun JsonObject.optU64(key: String): ULong? = get(key)?.takeIf { !it.isJsonNull }?.asString?.toULong()

private fun JsonObject.u64(key: String): ULong = str(key).toULong()

private fun JsonObject.obj(key: String): JsonObject = get(key)?.asJsonObject ?: error("missing object $key")

private fun JsonObject.arr(key: String): JsonArray = get(key)?.asJsonArray ?: error("missing array $key")

private fun JsonElement.objects(): List<JsonObject> = asJsonArray.map { it.asJsonObject }

private fun number(value: ULong): JsonPrimitive = JsonPrimitive(BigInteger(value.toString()))

private fun numberOrNull(value: ULong?): JsonElement = value?.let { number(it) } ?: JsonNull.INSTANCE

private fun JsonObject.put(key: String, value: String): JsonObject = apply { addProperty(key, value) }

private fun JsonObject.put(key: String, value: JsonElement): JsonObject = apply { add(key, value) }

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

// ---- Flow adapters ----------------------------------------------------------

private fun severity(name: String): Severity = when (name) {
    "info" -> Severity.INFO
    "low" -> Severity.LOW
    "medium" -> Severity.MEDIUM
    "high" -> Severity.HIGH
    "critical" -> Severity.CRITICAL
    else -> error("unknown severity $name")
}

private fun confidence(name: String): Confidence = when (name) {
    "low" -> Confidence.LOW
    "medium" -> Confidence.MEDIUM
    "high" -> Confidence.HIGH
    else -> error("unknown confidence $name")
}

private fun findings(input: JsonObject): List<Finding> = input.arr("findings").objects().map { finding ->
    Finding(
        ruleId = RuleId(finding.str("id")),
        location = Location(
            start = finding.u64("start"),
            end = finding.u64("end"),
            line = finding.optU64("line") ?: 1UL,
            column = finding.optU64("column") ?: 1UL,
        ),
        severity = severity(finding.optStr("severity") ?: "high"),
        confidence = confidence(finding.optStr("confidence") ?: "high"),
    )
}

private fun testKey(selector: String): ByteArray = when (selector) {
    "a" -> KEY_A.toByteArray(UTF_8)
    "b" -> KEY_B.toByteArray(UTF_8)
    else -> error("unknown public test vector selector $selector")
}

private fun pseudoOptions(config: JsonObject): PseudonymizationOptions =
    PseudonymizationOptions(testKey(config.str("key")), config.optStr("prefix"), config.optU64("digestBytes"))

private fun synthesisOptions(config: JsonObject): SynthesisOptions =
    SynthesisOptions(testKey(config.str("key")), config.optStr("marker"))

private fun transformCode(code: TransformErrorCode): String = when (code) {
    TransformErrorCode.INVALID_SPAN -> "InvalidSpan"
    TransformErrorCode.INVALID_UTF8_BOUNDARY -> "InvalidUtf8Boundary"
    TransformErrorCode.OVERLAPPING_SPANS -> "OverlappingSpans"
    TransformErrorCode.INPUT_CONVERSION -> "InputConversion"
    TransformErrorCode.UNKNOWN_CORE_ERROR -> "UnknownCoreError"
    TransformErrorCode.INVALID_KEY_LENGTH -> "InvalidKeyLength"
}

private fun spanJson(span: ByteSpan?): JsonElement = span?.let {
    objectOf("start" to number(it.start), "end" to number(it.end))
} ?: JsonNull.INSTANCE

private fun transformErrorJson(error: TransformException): JsonObject {
    val rejected = error as TransformException.Rejected
    return objectOf(
        "code" to JsonPrimitive(transformCode(rejected.code)),
        "span" to spanJson(rejected.span),
        "otherSpan" to spanJson(rejected.otherSpan),
        "sourceLen" to numberOrNull(rejected.sourceLen),
        "detail" to JsonPrimitive(rejected.detail),
    )
}

private fun flowOutcome(block: () -> TransformResult): JsonObject = try {
    objectOf("content" to JsonPrimitive(block().content))
} catch (error: TransformException) {
    objectOf("error" to transformErrorJson(error))
}

private fun compareFlow(actual: JsonObject, expected: JsonObject, name: String) {
    if (expected.has("content")) {
        check(!actual.has("error")) { "unexpected error for $name: $actual" }
        val actualBytes = actual.str("content").toByteArray(UTF_8)
        val expectedBytes = expected.str("content").toByteArray(UTF_8)
        check(actualBytes.contentEquals(expectedBytes)) { "output bytes differ for $name" }
    } else {
        requireJson(actual, expected, name)
    }
}

// ---- Flow checks ------------------------------------------------------------

private fun checkRedactTemplate(cases: List<JsonObject>, root: JsonObject) {
    val expected = root.arr("redactTemplate").objects()
    require(cases.size == 23 && cases.size == expected.size) { "expected 23 redact/template fixtures" }
    for ((input, oracle) in cases.zip(expected)) {
        val name = input.str("name")
        require(name == oracle.str("name")) { "fixture name mismatch" }
        val source = input.str("source")
        val spans = findings(input)
        val redacted = flowOutcome { redact(source, spans, RedactOptions(input.optStr("replacement"))) }
        if (name == "large-u64-range") require(spans[0].location.end == ULong.MAX_VALUE) { "u64 max not preserved" }
        val templated = flowOutcome {
            template(source, spans, TemplateOptions(input.optStr("namespace"), input.optBool("numbered", false)))
        }
        compareFlow(redacted, oracle.obj("redact"), "redact $name")
        compareFlow(templated, oracle.obj("template"), "template $name")
    }
    // Explicit findings never trigger a rescan.
    val source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"
    val report = Scanner().scan(ScanSource(SourceId("input"), source))
    check(report.findings.isNotEmpty()) { "Core scan produced no findings" }
    val untouched = redact(source, emptyList(), RedactOptions(null)).content
    check(untouched.toByteArray(UTF_8).contentEquals(source.toByteArray(UTF_8))) { "redact rescanned the source" }
    val redacted = redact(source, report.findings, RedactOptions(null)).content
    check(redacted == "GITHUB_TOKEN=[REDACTED]") { "unexpected redaction: $redacted" }
    println("PASS: Flow Redact/Template; 23 fixtures, 46 Core outcomes, UTF-8 bytes, overlap, structured errors, no rescanning, owned output")
}

private fun checkKeyed(base: List<JsonObject>, extra: List<JsonObject>, configs: List<JsonObject>, expected: JsonArray) {
    val cases = base + extra
    require(cases.size == 27 && configs.size == 9 && expected.size() == cases.size * configs.size) {
        "expected 27 keyed inputs x 9 options"
    }
    var index = 0
    for (input in cases) {
        for (config in configs) {
            val oracle = expected.get(index).asJsonObject
            index += 1
            val name = input.str("name")
            val configName = config.str("name")
            require(name == oracle.str("name") && configName == oracle.str("options")) { "keyed oracle ordering mismatch" }
            val source = input.str("source")
            val spans = findings(input)
            val pseudo = flowOutcome { pseudonymize(source, spans, pseudoOptions(config)) }
            val synth = flowOutcome { synthesize(source, spans, synthesisOptions(config)) }
            compareFlow(pseudo, oracle.obj("pseudonymize"), "pseudonymize $name/$configName")
            compareFlow(synth, oracle.obj("synthesize"), "synthesize $name/$configName")
            compareFlow(flowOutcome { pseudonymize(source, spans, pseudoOptions(config)) }, pseudo, "pseudonymize determinism")
            compareFlow(flowOutcome { synthesize(source, spans, synthesisOptions(config)) }, synth, "synthesis determinism")
            val keyText = String(testKey(config.str("key")), UTF_8)
            for (result in listOf(pseudo, synth)) {
                check(!result.toString().contains(keyText)) { "key echoed in result/error" }
            }
            if (configName == "digest-usize-max") require(pseudoOptions(config).digestBytes == ULong.MAX_VALUE) { "u64 digest not preserved" }
        }
    }
    fun keyLengthError(length: Int): JsonObject = objectOf(
        "error" to objectOf(
            "code" to JsonPrimitive("InvalidKeyLength"),
            "span" to JsonNull.INSTANCE,
            "otherSpan" to JsonNull.INSTANCE,
            "sourceLen" to JsonNull.INSTANCE,
            "detail" to JsonPrimitive("transformation key must contain exactly 32 bytes; received $length"),
        ),
    )
    for (length in listOf(0, 1, 31, 33, 64, 1024)) {
        val key = ByteArray(length) { 0xa5.toByte() }
        for ((source, spans) in listOf("" to emptyList<Finding>(), "abc" to findings(base[12]))) {
            val pseudo = flowOutcome { pseudonymize(source, spans, PseudonymizationOptions(key, null, null)) }
            val synth = flowOutcome { synthesize(source, spans, SynthesisOptions(key, null)) }
            requireJson(pseudo, keyLengthError(length), "invalid pseudonymization key length $length")
            requireJson(synth, keyLengthError(length), "invalid synthesis key length $length")
        }
    }
    // Key sensitivity, content independence, digest clamping and no implicit scan.
    val first = cases.first { it.str("name") == "content-a" }
    val second = cases.first { it.str("name") == "content-b" }
    fun pseudoContent(input: JsonObject, options: PseudonymizationOptions): String =
        pseudonymize(input.str("source"), findings(input), options).content
    fun synthesisContent(input: JsonObject, options: SynthesisOptions): String =
        synthesize(input.str("source"), findings(input), options).content
    val firstPseudo = pseudoContent(first, pseudoOptions(configs[0]))
    require(firstPseudo != pseudoContent(first, pseudoOptions(configs[1]))) { "changing the key did not change pseudonymization" }
    val firstSynthesis = synthesisContent(first, synthesisOptions(configs[0]))
    val secondKeySynthesis = synthesisContent(first, synthesisOptions(configs[1]))
    val changedContentSynthesis = synthesisContent(second, synthesisOptions(configs[0]))
    require(firstSynthesis != secondKeySynthesis && firstSynthesis == changedContentSynthesis) {
        "synthesis key/content semantics changed"
    }
    for ((requested, digestLength) in listOf(0UL to 8, 1UL to 8, 8UL to 8, 16UL to 16, 32UL to 32, 33UL to 32, ULong.MAX_VALUE to 32)) {
        val output = pseudoContent(first, PseudonymizationOptions(testKey("a"), "p:", requested))
        require(output.toByteArray(UTF_8).size == 2 + 2 * digestLength) { "Core digest clamping changed for $requested" }
    }
    for (text in listOf("", "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789")) {
        val pseudo = pseudonymize(text, emptyList(), pseudoOptions(configs[0])).content
        val synth = synthesize(text, emptyList(), synthesisOptions(configs[0])).content
        require(pseudo.toByteArray(UTF_8).contentEquals(text.toByteArray(UTF_8))) { "pseudonymize rescanned the source" }
        require(synth.toByteArray(UTF_8).contentEquals(text.toByteArray(UTF_8))) { "synthesize rescanned the source" }
    }
    println("PASS: keyed Flow; 27 inputs x 9 options, 486 Core outcomes, repeated determinism, key lengths, digest clamping, no key echo or rescanning")
}

// ---- Share checks -----------------------------------------------------------

private fun shareModeName(kind: ShareModeKind): String = when (kind) {
    ShareModeKind.REDACT -> "redact"
    ShareModeKind.TEMPLATE -> "template"
    ShareModeKind.PSEUDONYMIZE -> "pseudonymize"
    ShareModeKind.SYNTHESIZE -> "synthesize"
}

private fun shareErrorCode(code: ShareErrorCode): String = when (code) {
    ShareErrorCode.MISSING_SHARE_MODE -> "MissingShareMode"
    ShareErrorCode.SOURCE_COUNT_MISMATCH -> "SourceCountMismatch"
    ShareErrorCode.SOURCE_LENGTH_MISMATCH -> "SourceLengthMismatch"
    ShareErrorCode.INPUT_CONVERSION -> "InputConversion"
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

private fun shareErrorJson(error: ShareException): JsonObject = when (error) {
    is ShareException.Transform -> {
        val transform = transformErrorJson(error.error)
        objectOf(
            "kind" to JsonPrimitive("transform"),
            "code" to transform.get("code"),
            "span" to transform.get("span"),
            "otherSpan" to transform.get("otherSpan"),
            "sourceLen" to transform.get("sourceLen"),
            "detail" to transform.get("detail"),
        )
    }

    is ShareException.Rejected -> objectOf(
        "kind" to JsonPrimitive("bundle"),
        "code" to JsonPrimitive(shareErrorCode(error.code)),
        "expected" to numberOrNull(error.expected),
        "actual" to numberOrNull(error.actual),
        "index" to numberOrNull(error.index),
        "detail" to JsonPrimitive(error.detail),
    )
}

private fun shareOutcome(block: () -> ShareBundle): JsonObject {
    val before = System.currentTimeMillis() / 1000.0
    return try {
        val bundle = block()
        val generatedAt = bundle.manifest.generatedAt
        require(generatedAt.nanoseconds < 1_000_000_000u) { "invalid manifest nanoseconds" }
        val timestamp = generatedAt.seconds.toDouble() + generatedAt.nanoseconds.toDouble() / 1_000_000_000.0
        check(timestamp >= before - 0.001 && timestamp <= System.currentTimeMillis() / 1000.0 + 0.001) {
            "manifest timestamp is not from this call"
        }
        objectOf(
            "sources" to JsonArray().apply {
                for (source in bundle.sources) {
                    add(objectOf("id" to JsonPrimitive(source.sourceId.value), "content" to JsonPrimitive(source.content)))
                }
            },
            "mode" to JsonPrimitive(shareModeName(bundle.manifest.mode)),
            "summary" to summaryJson(bundle.manifest.summary),
        )
    } catch (error: ShareException) {
        objectOf("error" to shareErrorJson(error))
    }
}

private fun shareOptions(mode: Int): ShareBundleOptions = ShareBundleOptions(
    mode = when (mode) {
        0 -> ShareMode.Redact
        1 -> ShareMode.Template
        2 -> ShareMode.Pseudonymize(PseudonymizationOptions(KEY_A.toByteArray(UTF_8), "p:", 8UL))
        3 -> ShareMode.Synthesize(SynthesisOptions(KEY_A.toByteArray(UTF_8), "demo"))
        else -> error("unknown share mode $mode")
    },
)

private fun shareScanner(builtins: Boolean): Scanner =
    if (builtins) {
        Scanner()
    } else {
        Scanner.configured(
            ScannerConfig.CustomOnly(
                listOf(RuleSpec(RuleId("secret"), RuleKind.LITERAL, "SECRET", Severity.HIGH, null)),
            ),
        )
    }

private fun checkShare(batches: List<JsonObject>, explicit: List<JsonObject>, oracle: JsonObject) {
    val batchOracle = oracle.arr("batch").objects()
    val explicitOracle = oracle.arr("explicit").objects()
    var batchIndex = 0
    var candidateCount = 0
    for (batch in batches) {
        val scanner = shareScanner(batch.optBool("builtins", false))
        val sources = batch.arr("sources").objects().map { ScanSource(SourceId(it.str("id")), it.str("text")) }
        val reports = scanner.scanForShare(sources)
        require(reports.size == sources.size) { "scanForShare report count mismatch" }
        candidateCount += reports.sumOf { it.candidates.size }
        for (mode in 0 until 4) {
            for (atomic in listOf(false, true)) {
                val entry = batchOracle[batchIndex]
                require(entry.str("name") == batch.str("name") && entry.int("mode") == mode && entry.optBool("atomic", false) == atomic) {
                    "share oracle ordering mismatch at batch $batchIndex"
                }
                val actual = shareOutcome {
                    if (atomic) scanAndBuild(scanner, sources, shareOptions(mode)) else build(reports, sources.map { it.text }, shareOptions(mode))
                }
                requireJson(actual, entry.obj("outcome"), "share batch ${batch.str("name")} mode $mode atomic $atomic")
                batchIndex += 1
            }
        }
    }
    require(candidateCount > 0) { "no review-only candidates were exercised" }
    var explicitIndex = 0
    for (input in explicit) {
        val source = input.str("source")
        val report = ShareReport(
            sourceId = SourceId("明示😀"),
            sourceBytes = source.toByteArray(UTF_8).size.toULong(),
            findings = findings(input),
            candidates = emptyList(),
        )
        for (mode in 0 until 4) {
            val entry = explicitOracle[explicitIndex]
            require(entry.str("name") == input.str("name") && entry.int("mode") == mode) { "share oracle ordering mismatch at explicit $explicitIndex" }
            val actual = shareOutcome { build(listOf(report), listOf(source), shareOptions(mode)) }
            requireJson(actual, entry.obj("outcome"), "share explicit ${input.str("name")} mode $mode")
            explicitIndex += 1
        }
    }
    require(batchIndex == batchOracle.size && explicitIndex == explicitOracle.size) { "share oracle coverage mismatch" }
    checkShareErrors()
    println(
        "PASS: Share Bundles; $batchIndex batch + $explicitIndex explicit Core outcomes, all modes, both paths, " +
            "Unicode/duplicate IDs, candidates, timestamps, errors, no key echo",
    )
}

private fun checkShareErrors() {
    val scanner = shareScanner(false)
    val reports = scanner.scanForShare(listOf(ScanSource(SourceId("a"), "SECRET")))
    fun bundleError(code: String, expected: ULong?, actual: ULong?, index: ULong?, detail: String): JsonObject = objectOf(
        "error" to objectOf(
            "kind" to JsonPrimitive("bundle"),
            "code" to JsonPrimitive(code),
            "expected" to numberOrNull(expected),
            "actual" to numberOrNull(actual),
            "index" to numberOrNull(index),
            "detail" to JsonPrimitive(detail),
        ),
    )
    fun rejection(sources: List<String>, options: ShareBundleOptions, expected: JsonObject, name: String) {
        requireJson(shareOutcome { build(reports, sources, options) }, expected, name)
    }
    rejection(emptyList(), shareOptions(0), bundleError("SourceCountMismatch", 1UL, 0UL, null, "share bundle expected 1 sources but received 0"), "empty batch")
    rejection(
        listOf("SECRET", "extra"),
        shareOptions(0),
        bundleError("SourceCountMismatch", 1UL, 2UL, null, "share bundle expected 1 sources but received 2"),
        "extra source",
    )
    rejection(
        listOf("short"),
        shareOptions(0),
        bundleError("SourceLengthMismatch", 6UL, 5UL, 0UL, "share bundle source 0 has 5 bytes but scan results record 6"),
        "short source",
    )
    val missingMode = bundleError("MissingShareMode", null, null, null, "share bundle transformation mode was not configured")
    rejection(emptyList(), ShareBundleOptions(null), missingMode, "missing mode")
    requireJson(
        shareOutcome { scanAndBuild(scanner, emptyList(), ShareBundleOptions(null)) },
        missingMode,
        "missing mode (atomic)",
    )
    // Core checks byte lengths, not identity: same-length changed content is accepted.
    val changed = build(reports, listOf("PUBLIC"), shareOptions(0))
    check(changed.sources[0].content == "[REDACTED]") { "same-length changed content was rejected" }
    for (count in listOf(0, 1, 31, 33, 64)) {
        val invalid = ByteArray(count) { 90 }
        val modes = listOf(
            ShareMode.Pseudonymize(PseudonymizationOptions(invalid, null, null)),
            ShareMode.Synthesize(SynthesisOptions(invalid, null)),
        )
        for (mode in modes) {
            val expected = objectOf(
                "error" to objectOf(
                    "kind" to JsonPrimitive("transform"),
                    "code" to JsonPrimitive("InvalidKeyLength"),
                    "span" to JsonNull.INSTANCE,
                    "otherSpan" to JsonNull.INSTANCE,
                    "sourceLen" to JsonNull.INSTANCE,
                    "detail" to JsonPrimitive("transformation key must contain exactly 32 bytes; received $count"),
                ),
            )
            for (atomic in listOf(false, true)) {
                val actual = shareOutcome {
                    if (atomic) scanAndBuild(scanner, emptyList(), ShareBundleOptions(mode)) else build(emptyList(), emptyList(), ShareBundleOptions(mode))
                }
                requireJson(actual, expected, "invalid share key length $count (atomic=$atomic)")
            }
        }
    }
}

fun main(args: Array<String>) {
    require(args.size == 6) {
        "usage: FlowSmokeKt <flow.json> <flow-oracle.json> <keyed-flow.json> <keyed-options.json> <share.json> <share-oracle.json>"
    }
    val flowCases = parseFile(File(args[0])).objects()
    val flowOracle = parseFile(File(args[1])).asJsonObject
    val keyedCases = parseFile(File(args[2])).objects()
    val keyedOptions = parseFile(File(args[3])).objects()
    val shareCases = parseFile(File(args[4])).objects()
    val shareOracle = parseFile(File(args[5])).asJsonObject
    checkRedactTemplate(flowCases, flowOracle)
    checkKeyed(flowCases, keyedCases, keyedOptions, flowOracle.arr("keyed"))
    checkShare(shareCases, flowCases, shareOracle)
    println("PASS: Kotlin/JVM Flow/Share consumer smoke complete against the native Cribra Core library")
}
