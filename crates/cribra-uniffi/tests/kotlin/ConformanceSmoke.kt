package cribra.kotlin.conformance

import com.google.gson.*
import com.sun.jna.NativeLibrary
import uniffi.cribra_uniffi.*
import java.io.File

// Java reflection needs no kotlin-reflect dependency. Ignore only the compiler's
// unsigned-value mangling suffix, preserving the method/DTO names and types.
private fun apiContract(profile: String, fixture: JsonObject) {
    fun methods(type: Class<*>, expected: Set<String>) {
        val actual = type.declaredMethods.map { it.name.substringBefore('-') }
        check(actual.toSet() == expected && actual.size == expected.size) { "${type.simpleName} method contract" }
    }
    val scanner = setOf("activeRules", "incremental", "incrementalWithLimits", "scan", "scanMany", "stream", "streamWithLimits") +
        (if (profile != "core") setOf("scanForShare") else emptySet()) +
        (if (profile == "fuse") setOf("parallelScan") else emptySet())
    methods(ScannerInterface::class.java, scanner)
    methods(StreamSessionInterface::class.java, setOf("state", "push", "finish"))
    methods(IncrementalSessionInterface::class.java, setOf("state", "applyEdit", "report", "revision", "snapshot"))
    fun signature(type: Class<*>, method: String, returns: Class<*>, vararg parameters: Class<*>) {
        val actual = type.declaredMethods.single { it.name.substringBefore('-') == method }
        check(actual.returnType == returns && actual.parameterTypes.toList() == parameters.toList()) { "$method type contract" }
    }
    signature(StreamSessionInterface::class.java, "push", Void.TYPE, ByteArray::class.java)
    signature(StreamSessionInterface::class.java, "finish", ScanReport::class.java)
    signature(IncrementalSessionInterface::class.java, "applyEdit", Long::class.javaPrimitiveType!!,
        Long::class.javaPrimitiveType!!, Long::class.javaPrimitiveType!!, Long::class.javaPrimitiveType!!, String::class.java)
    signature(IncrementalSessionInterface::class.java, "revision", Long::class.javaPrimitiveType!!)
    signature(IncrementalSessionInterface::class.java, "snapshot", IncrementalSnapshot::class.java)
    val records = listOf(Location::class.java, Finding::class.java, ScanReport::class.java, ScanResults::class.java,
        ScanSummary::class.java, RuleSpec::class.java, RuleMetadata::class.java, RemediationMetadata::class.java,
        StreamLimits::class.java, IncrementalLimits::class.java, IncrementalSnapshot::class.java)
    for (type in records) {
        val expected = fixture["recordFields"].asJsonObject[type.simpleName].asJsonArray
            .map { it.asString.substringBefore(':') }.toSet()
        val actual = type.declaredMethods.filter { it.name.startsWith("get") && it.parameterCount == 0 }
            .map { it.name.substringBefore('-').removePrefix("get").replaceFirstChar(Char::lowercase) }.toSet()
        check(actual == expected) { "${type.simpleName} DTO field contract" }
    }
    for (values in listOf(StreamState.entries, StreamErrorCode.entries, IncrementalState.entries,
        IncrementalErrorCode.entries, BuildErrorCode.entries, Remediation.entries)) {
        val type = values.first().javaClass.simpleName
        val expected = fixture["enumCases"].asJsonObject[type].asJsonArray.map { it.asString }
        check(values.map { enumName(it).replaceFirstChar(Char::lowercase) } == expected) { "$type enum contract" }
    }
    val hasShare = try { Class.forName("uniffi.cribra_uniffi.ShareModeKind"); true } catch (_: ClassNotFoundException) { false }
    check(hasShare == (profile != "core")) { "Flow export gating" }
    println("PASS: Kotlin $profile generated API contract; interface signatures, unsigned revisions, DTO fields, enum cases and profile gating")
}

fun main(args: Array<String>) {
    require(args.size == 5) { "configuration-fixture scanner-oracle incremental-fixture api-contract core|flow|fuse" }
    val profile = args[4]
    require(profile in listOf("core", "flow", "fuse"))
    val parse = { path: String -> JsonParser.parseString(File(path).readText(Charsets.UTF_8)) }
    apiContract(profile, parse(args[3]).asJsonObject)
    check(System.getProperty("uniffi.component.cribra_uniffi.libraryOverride") == null)
    val loaded = requireNotNull(NativeLibrary.getInstance("cribra_uniffi").file).canonicalFile
    val expected = File(requireNotNull(System.getProperty("jna.library.path"))).canonicalFile
    check(loaded.parentFile == expected) { "conformance loaded a native library outside its profile directory" }
    println("NATIVE_CONFORMANCE=$loaded")
    configurationSmoke(arrayOf(args[0], args[1], profile))
    val cases = parse(args[0]).asJsonArray
    val oracle = parse(args[1]).asJsonObject
    streamingSmoke(profile, cases, oracle)
    incrementalSmoke(profile, cases, parse(args[2]).asJsonObject, oracle)
}
