import Foundation
import Cribra
#if os(iOS)
import SwiftUI
#endif

/// Shared native-app integration exercise. No test-only library/header paths.
enum Integration {
    static func check() throws -> String {
        precondition(cribraVersion() == CribraBuild.coreVersion)
        precondition(CribraBuild.rustVersion == "1.97.0" && CribraBuild.uniffiVersion == "0.32.2")
        let scanner = try Cribra.Scanner.configured(config: .customOnly(rules: [
            RuleSpec(id: RuleId(value: "literal"), kind: .literal, value: "SECRET", severity: .high, remediation: nil),
        ]))
        let text = "é😀e\u{301}\r\nSECRET"
        let source = ScanSource(id: SourceId(value: "documento/文書😀"), text: text)
        let result = scanner.scanMany(sources: [source, source, ScanSource(id: SourceId(value: ""), text: "")])
        precondition(result.reports.count == 3 && result.summary.totalFindings == 2)
        let single = scanner.scan(source: source)
        precondition(single.findings == result.reports[0].findings)
        precondition(result.reports[0].sourceId.value == source.id.value && result.reports[1].sourceId.value == source.id.value)
        let finding = result.reports[0].findings[0]
        precondition(finding.ruleId.value == "literal")
        precondition(finding.location.start == UInt64(text.utf8.count - 6) && finding.location.end == UInt64(text.utf8.count))
        precondition(finding.location.line == 2 && finding.location.column == 1)
        let session = try scanner.incremental(source: source)
        let snapshot = try session.snapshot()
        precondition(snapshot.revision == 0 && snapshot.report.findings.count == 1)
        _ = try session.applyEdit(expectedRevision: 0, start: finding.location.start, end: finding.location.end, replacement: "")
        let after = try session.snapshot()
        precondition(after.revision == 1 && after.report.findings.isEmpty)
        let stream = try scanner.stream(sourceId: source.id)
        let bytes = Data(text.utf8)
        try stream.push(chunk: bytes.prefix(3)) // splits an emoji UTF-8 scalar
        try stream.push(chunk: bytes.dropFirst(3))
        let streamed = try stream.finish()
        precondition(streamed.findings == result.reports[0].findings)
        #if CRIBRA_FLOW
        precondition(CribraBuild.profile == "flow" || CribraBuild.profile == "fuse")
        let findings = result.reports[0].findings
        let redacted = try redact(source: text, findings: findings, options: RedactOptions(replacement: nil))
        precondition(redacted.content.hasSuffix("[REDACTED]"))
        let templated = try template(source: text, findings: findings, options: TemplateOptions(namespace: nil, numbered: false))
        precondition(templated.content.hasSuffix("<CRIBRA:literal>"))
        // Fixed PUBLIC test key; production keys always come explicitly from callers.
        let key = Data("0123456789ABCDEF0123456789ABCDEF".utf8)
        let pseudonyms = try pseudonymize(source: text, findings: findings, options: PseudonymizationOptions(key: key, prefix: nil, digestBytes: nil))
        precondition(pseudonyms.content.contains("cribra_pseudo_"))
        let synthetic = try synthesize(source: text, findings: findings, options: SynthesisOptions(key: key, marker: nil))
        precondition(synthetic.content != text)
        let bundle = try scanAndBuild(scanner: scanner, sources: [source, source], options: ShareBundleOptions(mode: .redact))
        let reports = try scanner.scanForShare(sources: [source, source])
        let precomputed = try build(reports: reports, sources: [text, text], options: ShareBundleOptions(mode: .redact))
        precondition(bundle.sources == precomputed.sources && bundle.manifest.summary == precomputed.manifest.summary)
        #else
        precondition(CribraBuild.profile == "core")
        #endif
        #if CRIBRA_FUSE
        precondition(CribraBuild.profile == "fuse")
        precondition(scanner.parallelScan(sources: [source, source]).summary.totalFindings == 2)
        #endif
        return "PASS: external native Cribra \(CribraBuild.profile), Core \(cribraVersion()), scanner/UTF-8/stream/incremental and enabled profile APIs"
    }
}

#if os(iOS)
@main
struct ConsumerApp: App {
    var body: some Scene {
        WindowGroup { Text((try? Integration.check()) ?? "Cribra integration failed") }
    }
}
#else
@main
struct Consumer {
    static func main() throws {
        precondition(CommandLine.arguments.count == 2)
        precondition(CommandLine.arguments[1] == CribraBuild.profile)
        print(try Integration.check())
    }
}
#endif
