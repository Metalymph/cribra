// Real generated Core/Flow/Fuse bindings against the shared direct-Core oracle.
import com.google.gson.*
import uniffi.cribra_uniffi.*
import java.io.File
import java.math.BigInteger

private fun name(value: Enum<*>): String = value.name.split('_').joinToString("") { it.lowercase().replaceFirstChar(Char::uppercase) }
private fun json(value: String?): JsonElement = value?.let(::JsonPrimitive) ?: JsonNull.INSTANCE
private fun number(value: ULong): JsonElement = JsonPrimitive(BigInteger(value.toString()))
private fun obj(vararg fields: Pair<String, JsonElement>) = JsonObject().apply { fields.forEach { (k,v) -> add(k,v) } }
private fun array(values: List<JsonElement>) = JsonArray().apply { values.forEach(::add) }
private fun explanation(value: Explanation?): JsonElement = when(value) {
    is Explanation.Classified -> obj("classified" to json(name(value.mode)))
    is Explanation.Ambiguous -> obj("ambiguous" to json(name(value.evidence)))
    null -> JsonNull.INSTANCE
}
private fun report(value: ScanReport) = obj(
    "sourceId" to json(value.sourceId.value), "sourceBytes" to number(value.sourceBytes),
    "candidateCount" to number(value.candidateCount), "needsReview" to JsonPrimitive(value.needsReview),
    "findings" to array(value.findings.map { f -> obj(
        "ruleId" to json(f.ruleId.value), "start" to number(f.location.start), "end" to number(f.location.end),
        "line" to number(f.location.line), "column" to number(f.location.column),
        "severity" to json(f.severity.name.lowercase()), "confidence" to json(f.confidence.name.lowercase()),
        "remediation" to json(f.remediation?.let(::name)), "explanation" to explanation(f.explanation)) }),
    "candidates" to array(value.candidates.map { c -> obj(
        "kind" to json(name(c.kind)), "evidence" to json(name(c.evidence)),
        "start" to number(c.location.start), "end" to number(c.location.end),
        "line" to number(c.location.line), "column" to number(c.location.column),
        "explanation" to explanation(c.explanation)) })
)
private fun metadata(values: List<RuleMetadata>) = array(values.map { r -> obj(
    "id" to json(r.id.value), "kind" to json(name(r.kind)), "detectionMode" to json(name(r.detectionMode)),
    "severity" to json(r.severity.name.lowercase()),
    "remediation" to (r.remediation?.let { obj("label" to json(it.label), "message" to json(it.message)) } ?: JsonNull.INSTANCE)) })

fun main(args: Array<String>) {
    val cases = JsonParser.parseString(File(args[0]).readText()).asJsonArray
    val outcomes = JsonParser.parseString(File(args[1]).readText()).asJsonObject["configurations"].asJsonArray
    check(cases.size() == outcomes.size())
    var successes = 0
    var failures = 0
    for (i in 0 until cases.size()) {
        val input = cases[i].asJsonObject
        val expected = outcomes[i].asJsonObject
        val caseName = input["name"].asString
        check(caseName == expected["name"].asString)
        val rules = input["rules"].asJsonArray.map { it.asJsonObject }.map { r -> RuleSpec(
            RuleId(r["id"].asString), RuleKind.valueOf(r["kind"].asString.uppercase()), r["value"].asString,
            Severity.valueOf(r["severity"].asString.uppercase()),
            if (r.has("remediation")) Remediation.REMOVE_SENSITIVE_VALUE else null) }
        val config = when(input["mode"].asString) {
            "builtInOnly" -> ScannerConfig.BuiltInOnly
            "customOnly" -> ScannerConfig.CustomOnly(rules)
            "builtInAndCustom" -> ScannerConfig.BuiltInAndCustom(rules)
            "catalogs" -> ScannerConfig.Catalogs(input["current"].asBoolean,input["financial"].asBoolean,input["personal"].asBoolean,rules)
            else -> error("unknown fixture mode")
        }
        try {
            Scanner.configured(config).use { scanner ->
                check(!expected.has("error")) { "expected rejection: $caseName" }
                check(metadata(scanner.activeRules()) == expected["rules"]) { "metadata mismatch: $caseName" }
                val source = ScanSource(SourceId(caseName), input["source"]?.asString ?: "")
                val actual = report(scanner.scan(source))
                check(actual == expected["results"].asJsonObject["reports"].asJsonArray[0]) { "report mismatch: $caseName" }
                check(actual == report(scanner.scanMany(listOf(source)).reports.single()))
                successes++
            }
        } catch(error: ScannerBuildException.Rejected) {
            val actual = obj("code" to json(name(error.code)), "ruleId" to json(error.ruleId?.value), "detail" to json(error.detail))
            check(actual == expected["error"]) { "structured error mismatch: $caseName" }
            failures++
        }
    }
    check(successes == 21 && failures == 19)
    println("PASS: Kotlin ${args[2]} configuration consumer; $successes configurations, $failures Core errors, full metadata/report parity")
}
