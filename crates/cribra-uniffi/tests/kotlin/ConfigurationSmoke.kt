// Real generated Core/Flow/Fuse bindings against the shared direct-Core oracle.
package cribra.kotlin.conformance

import com.google.gson.*
import uniffi.cribra_uniffi.*
import java.io.File

private fun metadata(values: List<RuleMetadata>) = array(values.map { r -> obj(
    "id" to json(r.id.value), "kind" to json(enumName(r.kind)), "detectionMode" to json(enumName(r.detectionMode)),
    "severity" to json(r.severity.name.lowercase()),
    "remediation" to (r.remediation?.let { obj("label" to json(it.label), "message" to json(it.message)) } ?: JsonNull.INSTANCE)) })

internal fun configurationSmoke(args: Array<String>) {
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
        val config = configuration(input)
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
            val actual = obj("code" to json(enumName(error.code)), "ruleId" to json(error.ruleId?.value), "detail" to json(error.detail))
            check(actual == expected["error"]) { "structured error mismatch: $caseName" }
            failures++
        }
    }
    check(successes == 21 && failures == 19)
    println("PASS: Kotlin ${args[2]} configuration consumer; $successes configurations, $failures Core errors, full metadata/report parity")
}
