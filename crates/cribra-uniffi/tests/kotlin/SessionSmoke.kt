package cribra.kotlin.conformance

import com.google.gson.*
import uniffi.cribra_uniffi.*

private fun streamError(error: StreamException.Rejected) = obj(
    "code" to json(enumName(error.code)), "limit" to (error.limit?.let(::number) ?: JsonNull.INSTANCE),
    "required" to (error.required?.let(::number) ?: JsonNull.INSTANCE), "detail" to json(error.detail))

private fun incrementalError(error: IncrementalException.Rejected) = obj(
    "code" to json(enumName(error.code)), "terminal" to JsonPrimitive(error.terminal),
    "expectedRevision" to (error.expectedRevision?.let(::number) ?: JsonNull.INSTANCE),
    "actualRevision" to (error.actualRevision?.let(::number) ?: JsonNull.INSTANCE),
    "limit" to (error.limit?.let(::number) ?: JsonNull.INSTANCE),
    "required" to (error.required?.let(::number) ?: JsonNull.INSTANCE), "detail" to json(error.detail))

private fun expectStream(expected: JsonElement, operation: () -> Unit) {
    try { operation(); error("expected stream rejection") }
    catch (error: StreamException.Rejected) { check(streamError(error) == expected) { "Core stream error mismatch" } }
}

private fun expectIncremental(expected: JsonElement, operation: () -> Unit) {
    try { operation(); error("expected incremental rejection") }
    catch (error: IncrementalException.Rejected) { check(incrementalError(error) == expected) { "Core incremental error mismatch" } }
}

private fun expectClosed(operation: () -> Unit) {
    var rejected = false
    try { operation() } catch (_: IllegalStateException) { rejected = true }
    check(rejected) { "closed Object unexpectedly remained callable" }
}

private fun literal(value: String = "SECRET", kind: RuleKind = RuleKind.LITERAL) = Scanner.configured(
    ScannerConfig.CustomOnly(listOf(RuleSpec(RuleId(if (kind == RuleKind.PATTERN) "eof" else "literal"),
        kind, value, Severity.HIGH, null))))

private fun streamLimits(candidates: ULong? = null, storage: ULong? = null, results: ULong? = null) =
    StreamLimits(candidates, storage, results)

internal fun streamingSmoke(profile: String, cases: JsonArray, oracle: JsonObject) {
    val outcomes = oracle["configurations"].asJsonArray
    var comparisons = 0
    val retainedReports = mutableListOf<Pair<ScanReport, JsonElement>>()
    for (caseName in listOf("custom", "catalogs-000-custom-0")) {
        val input = cases.map { it.asJsonObject }.single { it["name"].asString == caseName }
        val outcome = outcomes.map { it.asJsonObject }.single { it["name"].asString == caseName }
        val expected = outcome["results"].asJsonObject["reports"].asJsonArray.single()
        val bytes = input["source"].asString.toByteArray(Charsets.UTF_8)
        Scanner.configured(configuration(input)).use { scanner ->
            fun finish(session: StreamSession) {
                val result = session.finish()
                check(report(result) == expected) { "Core streaming report parity" }
                retainedReports.add(result to expected)
                comparisons++
                check(session.state() == StreamState.FINISHED)
                expectStream(oracle["streamErrors"].asJsonObject["notActive"]) { session.finish() }
                expectStream(oracle["streamErrors"].asJsonObject["notActive"]) { session.push(byteArrayOf()) }
                check(session.state() == StreamState.FINISHED)
            }
            for (split in 0..bytes.size) {
                scanner.stream(SourceId(caseName)).use { session ->
                    check(session.state() == StreamState.ACTIVE)
                    val prefix = bytes.copyOfRange(0, split)
                    session.push(prefix)
                    prefix.fill(0)
                    session.push(byteArrayOf())
                    val suffix = bytes.copyOfRange(split, bytes.size)
                    session.push(suffix)
                    suffix.fill(0)
                    finish(session)
                }
            }
            for (size in listOf(1, 2, 3, 7, 31)) {
                scanner.stream(SourceId(caseName)).use { session ->
                    for (start in bytes.indices step size) session.push(bytes.copyOfRange(start, minOf(start + size, bytes.size)))
                    finish(session)
                }
            }
        }
    }
    for ((retained, expected) in retainedReports) {
        check(report(retained) == expected) { "stream report changed after session/scanner close" }
    }
    val errors = oracle["streamErrors"].asJsonObject
    val id = SourceId("stream/文書é🙂")
    literal().use { scanner ->
        Scanner().use { owner -> expectStream(errors["unsupported"]) { owner.stream(id) } }
        literal("a+", RuleKind.PATTERN).use { owner -> expectStream(errors["unsupported"]) { owner.stream(id) } }
        expectStream(errors["storage"]) { scanner.streamWithLimits(id, streamLimits(storage = 0uL)) }
        scanner.streamWithLimits(id, streamLimits(storage = errors["storage"].asJsonObject["required"].asString.toULong())).use { session ->
            session.push("SECRET".toByteArray())
            val expected = scanner.scan(ScanSource(id, "SECRET"))
            check(session.finish() == expected)
        }
        scanner.stream(id).use { session ->
            session.push("SECRET ".toByteArray())
            expectStream(errors["invalidUtf8"]) { session.push(byteArrayOf(0xff.toByte())) }
            check(session.state() == StreamState.FAILED)
            expectStream(errors["notActive"]) { session.finish() }
            expectStream(errors["notActive"]) { session.push(byteArrayOf()) }
            check(session.state() == StreamState.FAILED)
        }
        scanner.stream(id).use { session ->
            session.push(byteArrayOf(0xf0.toByte()))
            session.push(byteArrayOf(0x9f.toByte()))
            check(session.state() == StreamState.ACTIVE)
            expectStream(errors["incompleteUtf8"]) { session.finish() }
            check(session.state() == StreamState.FAILED)
            expectStream(errors["notActive"]) { session.finish() }
            expectStream(errors["notActive"]) { session.push(byteArrayOf()) }
            check(session.state() == StreamState.FAILED)
        }
        for ((limits, key) in listOf(streamLimits(candidates = 1uL) to "candidate", streamLimits(results = 1uL) to "result")) {
            scanner.streamWithLimits(id, limits).use { session ->
                session.push("SECRET ".toByteArray())
                expectStream(errors[key]) { session.push("SECRET ".toByteArray()) }
                check(session.state() == StreamState.FAILED)
                expectStream(errors["notActive"]) { session.finish() }
                expectStream(errors["notActive"]) { session.push(byteArrayOf()) }
                check(session.state() == StreamState.FAILED)
            }
        }
    }
    Scanner.configured(ScannerConfig.CustomOnly(emptyList())).use { scanner ->
        scanner.streamWithLimits(id, streamLimits(results = 0uL)).use { session ->
            session.push("ABCD-EFGH-IJKL-MNOP".toByteArray())
            expectStream(errors["eofResult"]) { session.finish() }
            check(session.state() == StreamState.FAILED)
            expectStream(errors["notActive"]) { session.finish() }
        }
    }
    literal("abc\\z", RuleKind.PATTERN).use { scanner ->
        scanner.streamWithLimits(id, streamLimits(candidates = 0uL)).use { session ->
            session.push("abc".toByteArray())
            expectStream(errors["eofCandidate"]) { session.finish() }
            check(session.state() == StreamState.FAILED)
            expectStream(errors["notActive"]) { session.finish() }
        }
    }
    // Close the scanner wrapper before using its independently owned Core stream.
    val owner = literal()
    val detached = owner.stream(SourceId("empty"))
    owner.close()
    check(owner.uniffiIsDestroyed)
    detached.push(byteArrayOf())
    val retained = detached.finish()
    detached.close()
    detached.close()
    check(detached.uniffiIsDestroyed)
    expectClosed { detached.state() }
    check(report(retained) == oracle["reports"].asJsonArray.single { it.asJsonObject["sourceId"].asString == "empty" })
    comparisons++
    println("PASS: Kotlin $profile streaming; $comparisons direct-Core reports, all UTF-8 splits/chunk sizes, empty source, three budgets, terminal errors and close ownership")
}

private fun snapshot(session: IncrementalSession): JsonObject {
    val atomic = session.snapshot()
    check(atomic.revision == session.revision() && atomic.report == session.report())
    return obj("revision" to number(atomic.revision), "state" to json(enumName(session.state())), "report" to report(atomic.report))
}

private fun incrementalLimits(key: String, value: ULong) = IncrementalLimits(
    if (key == "candidates") value else null, if (key == "cache") value else null,
    if (key == "results") value else null, if (key == "storage") value else null)

internal fun incrementalSmoke(profile: String, cases: JsonArray, fixture: JsonObject, root: JsonObject) {
    val oracle = root["incremental"].asJsonObject
    val trace = oracle["trace"].asJsonArray
    val input = cases.map { it.asJsonObject }.single { it["name"].asString == "custom" }
    val id = SourceId(fixture["id"].asString)
    var edits = 0
    var rejections = 0
    Scanner.configured(configuration(input)).use { scanner ->
        val initial = ScanSource(id, fixture["source"].asString)
        scanner.incremental(initial).use { session ->
            check(session.revision() == 0uL)
            check(snapshot(session) == trace[0].asJsonObject["snapshot"])
            try {
                session.applyEdit(ULong.MAX_VALUE, 0u, 0u, "UNCOMMITTED_REPLACEMENT")
                error("expected stale u64 revision")
            } catch (error: IncrementalException.Rejected) {
                check(error.code == IncrementalErrorCode.STALE_REVISION && !error.terminal)
                check(error.expectedRevision == ULong.MAX_VALUE && error.actualRevision == 0uL)
                check(!error.detail.contains("UNCOMMITTED_REPLACEMENT"))
            }
            check(snapshot(session) == trace[0].asJsonObject["snapshot"])
            val retained = session.snapshot()
            val retainedCopy = report(retained.report)
            val mutableCopy = session.snapshot()
            mutableCopy.report.sourceBytes = ULong.MAX_VALUE
            check(session.snapshot().report == scanner.scan(initial)) { "consumer DTO mutation reached Core" }
            var bytes = initial.text.toByteArray(Charsets.UTF_8)
            val operations = fixture["edits"].asJsonArray
            check(trace.size() == operations.size() + 1)
            for ((index, operation) in operations.withIndex()) {
                val edit = operation.asJsonObject
                val expected = trace[index + 1].asJsonObject
                val before = snapshot(session)
                val revision = edit["revision"].asString.toULong()
                val start = edit["start"].asString.toULong()
                val end = edit["end"].asString.toULong()
                val replacement = edit["replacement"].asString
                if (expected["error"].isJsonNull) {
                    check(session.applyEdit(revision, start, end, replacement) == expected["newRevision"].asString.toULong())
                    // Test input bookkeeping only, after Core has accepted the edit.
                    bytes = bytes.copyOfRange(0, start.toInt()) + replacement.toByteArray(Charsets.UTF_8) + bytes.copyOfRange(end.toInt(), bytes.size)
                    edits++
                } else {
                    expectIncremental(expected["error"]) { session.applyEdit(revision, start, end, replacement) }
                    check(snapshot(session) == before) { "recoverable error changed committed snapshot" }
                    rejections++
                }
                check(snapshot(session) == expected["snapshot"]) { "Core incremental trace mismatch at $index" }
                check(session.report() == scanner.scan(ScanSource(id, bytes.toString(Charsets.UTF_8))))
                check(retained.revision == 0uL && report(retained.report) == retainedCopy)
            }
        }
        // Explicit CRLF replacement/deletion/insertion, checked with Core whole scans.
        data class Edit(val start: ULong, val end: ULong, val replacement: String, val text: String)
        scanner.incremental(ScanSource(id, "é\r\nLIT_é")).use { session ->
            val crlf = listOf(Edit(2u, 4u, "\n", "é\nLIT_é"), Edit(2u, 3u, "\r\n", "é\r\nLIT_é"),
                Edit(2u, 3u, "", "é\nLIT_é"), Edit(2u, 3u, "", "éLIT_é"), Edit(2u, 2u, "\r\n", "é\r\nLIT_é"))
            for ((index, edit) in crlf.withIndex()) {
                check(session.applyEdit(index.toULong(), edit.start, edit.end, edit.replacement) == index.toULong() + 1uL)
                check(session.snapshot().report == scanner.scan(ScanSource(id, edit.text)))
            }
        }
    }
    literal("x").use { scanner ->
        for (expected in oracle["resources"].asJsonArray.map { it.asJsonObject }) {
            val key = expected["name"].asString
            val owner = if (key == "storage") Scanner.configured(ScannerConfig.CustomOnly(emptyList())) else literal("x")
            owner.use {
                owner.incrementalWithLimits(ScanSource(id, if (key == "storage") "" else "x"), incrementalLimits(key, if (key == "storage") 0uL else 1uL)).use { session ->
                    if (key == "storage") check(session.applyEdit(0u, 0u, 0u, "") == 1uL)
                    val before = snapshot(session)
                    check(before == expected["before"])
                    val replacement = mapOf("candidates" to "xx", "cache" to "", "results" to "x", "storage" to "a").getValue(key)
                    expectIncremental(expected["error"]) { session.applyEdit(session.revision(), 0u, 0u, replacement) }
                    check(session.state() == IncrementalState.FAILED)
                    check(snapshot(session) == expected["after"])
                    check(snapshot(session)["revision"] == before["revision"] && snapshot(session)["report"] == before["report"])
                    expectIncremental(expected["laterError"]) { session.applyEdit(ULong.MAX_VALUE, 999u, 0u, "") }
                    check(snapshot(session) == expected["after"]) { "terminal failure discarded committed snapshot" }
                }
            }
        }
        for (expected in oracle["construction"].asJsonArray.map { it.asJsonObject }) {
            expectIncremental(expected["error"]) {
                scanner.incrementalWithLimits(ScanSource(id, "x"), incrementalLimits(expected["name"].asString, 0uL))
            }
        }
    }
    val owner = literal("x")
    val detached = owner.incremental(ScanSource(id, ""))
    val initial = detached.snapshot()
    owner.close()
    check(owner.uniffiIsDestroyed)
    check(detached.applyEdit(0u, 0u, 0u, "x") == 1uL)
    val committed = detached.snapshot()
    val committedCopy = report(committed.report)
    check(initial.revision == 0uL && initial.report.sourceBytes == 0uL && initial.report.findings.isEmpty())
    check(detached.applyEdit(1u, 0u, 1u, "") == 2uL)
    detached.close()
    detached.close()
    check(detached.uniffiIsDestroyed)
    expectClosed { detached.snapshot() }
    check(committed.revision == 1uL && committed.report.sourceBytes == 1uL && committed.report.findings.size == 1)
    check(report(committed.report) == committedCopy && initial.report.sourceBytes == 0uL)
    println("PASS: Kotlin $profile incremental; ${trace.size()} direct-Core trace snapshots, $edits accepted/$rejections rejected edits, five CRLF edits, four terminal and four construction budgets, u64 errors and detached snapshot ownership")
}
