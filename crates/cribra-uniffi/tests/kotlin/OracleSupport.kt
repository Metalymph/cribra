// Test-only projections of generated DTOs; matching and edits remain Core-owned.
package cribra.kotlin.conformance

import com.google.gson.*
import uniffi.cribra_uniffi.*
import java.math.BigInteger

internal fun enumName(value: Enum<*>): String = value.name.split('_').joinToString("") { it.lowercase().replaceFirstChar(Char::uppercase) }
internal fun json(value: String?): JsonElement = value?.let(::JsonPrimitive) ?: JsonNull.INSTANCE
internal fun number(value: ULong): JsonElement = JsonPrimitive(BigInteger(value.toString()))
internal fun obj(vararg fields: Pair<String, JsonElement>) = JsonObject().apply { fields.forEach { (k,v) -> add(k,v) } }
internal fun array(values: List<JsonElement>) = JsonArray().apply { values.forEach(::add) }
internal fun explanation(value: Explanation?): JsonElement = when(value) {
    is Explanation.Classified -> obj("classified" to json(enumName(value.mode)))
    is Explanation.Ambiguous -> obj("ambiguous" to json(enumName(value.evidence)))
    null -> JsonNull.INSTANCE
}
internal fun report(value: ScanReport) = obj(
    "sourceId" to json(value.sourceId.value), "sourceBytes" to number(value.sourceBytes),
    "candidateCount" to number(value.candidateCount), "needsReview" to JsonPrimitive(value.needsReview),
    "findings" to array(value.findings.map { f -> obj(
        "ruleId" to json(f.ruleId.value), "start" to number(f.location.start), "end" to number(f.location.end),
        "line" to number(f.location.line), "column" to number(f.location.column),
        "severity" to json(f.severity.name.lowercase()), "confidence" to json(f.confidence.name.lowercase()),
        "remediation" to json(f.remediation?.let(::enumName)), "explanation" to explanation(f.explanation)) }),
    "candidates" to array(value.candidates.map { c -> obj(
        "kind" to json(enumName(c.kind)), "evidence" to json(enumName(c.evidence)),
        "start" to number(c.location.start), "end" to number(c.location.end),
        "line" to number(c.location.line), "column" to number(c.location.column),
        "explanation" to explanation(c.explanation)) })
)

internal fun configuration(input: JsonObject): ScannerConfig {
    val rules = input["rules"].asJsonArray.map { it.asJsonObject }.map { r -> RuleSpec(
        RuleId(r["id"].asString), RuleKind.valueOf(r["kind"].asString.uppercase()), r["value"].asString,
        Severity.valueOf(r["severity"].asString.uppercase()),
        if (r.has("remediation")) Remediation.REMOVE_SENSITIVE_VALUE else null) }
    return when(input["mode"].asString) {
        "builtInOnly" -> ScannerConfig.BuiltInOnly
        "customOnly" -> ScannerConfig.CustomOnly(rules)
        "builtInAndCustom" -> ScannerConfig.BuiltInAndCustom(rules)
        "catalogs" -> ScannerConfig.Catalogs(input["current"].asBoolean,input["financial"].asBoolean,input["personal"].asBoolean,rules)
        else -> error("unknown fixture mode")
    }
}
