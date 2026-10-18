import Foundation
import cribra_uniffi

@main
struct ShareSmoke {
    // PUBLIC deterministic test vector; the adapter never generates keys.
    static let key = Data("0123456789ABCDEF0123456789ABCDEF".utf8)
    static func scanner(_ builtins: Bool = false) throws -> cribra_uniffi.Scanner {
        if builtins { return cribra_uniffi.Scanner() }
        return try cribra_uniffi.Scanner.configured(config: .customOnly(rules: [RuleSpec(id: RuleId(value: "secret"), kind: .literal, value: "SECRET", severity: .high, remediation: nil)]))
    }
    static func options(_ mode: Int) -> ShareBundleOptions {
        let selected: ShareMode
        switch mode {
        case 0: selected = .redact
        case 1: selected = .template
        case 2: selected = .pseudonymize(options: PseudonymizationOptions(key: key, prefix: "p:", digestBytes: 8))
        case 3: selected = .synthesize(options: SynthesisOptions(key: key, marker: "demo"))
        default: preconditionFailure("unknown mode")
        }
        return ShareBundleOptions(mode: selected)
    }
    static func bytes(_ span: ByteSpan?) -> Any {
        span.map { ["start": $0.start, "end": $0.end] as Any } ?? NSNull()
    }
    static func transformCode(_ code: TransformErrorCode) -> String {
        switch code {
        case .invalidSpan: return "InvalidSpan"
        case .invalidUtf8Boundary: return "InvalidUtf8Boundary"
        case .overlappingSpans: return "OverlappingSpans"
        case .invalidKeyLength: return "InvalidKeyLength"
        case .inputConversion: return "InputConversion"
        case .unknownCoreError: return "UnknownCoreError"
        }
    }
    static func outcome(_ operation: () throws -> ShareBundle) -> [String: Any] {
        let before = Date().timeIntervalSince1970
        do {
            let bundle = try operation()
            let time = bundle.manifest.generatedAt
            precondition(time.nanoseconds < 1_000_000_000)
            let timestamp = Double(time.seconds) + Double(time.nanoseconds) / 1_000_000_000
            // Validate the timestamp of THIS call, never byte-compare independent Core timestamps.
            precondition(timestamp >= before - 0.001 && timestamp <= Date().timeIntervalSince1970 + 0.001)
            let kind: String
            switch bundle.manifest.mode {
            case .redact: kind = "redact"
            case .template: kind = "template"
            case .pseudonymize: kind = "pseudonymize"
            case .synthesize: kind = "synthesize"
            }
            let s = bundle.manifest.summary
            return ["sources": bundle.sources.map { ["id": $0.sourceId.value, "content": $0.content] }, "mode": kind,
                "summary": ["scanned_sources": s.scannedSources, "scanned_bytes": s.scannedBytes,
                    "reports_with_findings": s.reportsWithFindings, "reports_without_findings": s.reportsWithoutFindings,
                    "total_findings": s.totalFindings, "total_candidates": s.totalCandidates,
                    "reports_with_candidates": s.reportsWithCandidates, "critical": s.critical,
                    "high": s.high, "medium": s.medium, "low": s.low, "info": s.info]]
        } catch let error as ShareError {
            switch error {
            case .Rejected(let code, let expected, let actual, let index, let detail):
                let name: String
                switch code {
                case .missingShareMode: name = "MissingShareMode"
                case .sourceCountMismatch: name = "SourceCountMismatch"
                case .sourceLengthMismatch: name = "SourceLengthMismatch"
                case .inputConversion: name = "InputConversion"
                }
                return ["error": ["kind": "bundle", "code": name,
                    "expected": expected.map { $0 as Any } ?? NSNull(), "actual": actual.map { $0 as Any } ?? NSNull(),
                    "index": index.map { $0 as Any } ?? NSNull(), "detail": detail]]
            case .Transform(let error):
                switch error {
                case .Rejected(let code, let span, let other, let length, let detail):
                    return ["error": ["kind": "transform", "code": transformCode(code), "span": bytes(span),
                        "otherSpan": bytes(other), "sourceLen": length.map { $0 as Any } ?? NSNull(), "detail": detail]]
                }
            }
        } catch { preconditionFailure("unexpected share error") }
    }
    static func compare(_ actual: [String: Any], _ expected: [String: Any]) throws {
        // Sorted JSON compares exact UTF-8 output bytes, avoiding Swift canonical String equality.
        let a = try JSONSerialization.data(withJSONObject: actual, options: [.sortedKeys])
        let b = try JSONSerialization.data(withJSONObject: expected, options: [.sortedKeys])
        precondition(a == b, "Share Bundle differs from direct Core oracle")
        precondition(!String(decoding: a, as: UTF8.self).contains(String(decoding: key, as: UTF8.self)), "key echoed")
    }
    static func findings(_ input: [String: Any]) -> [Finding] {
        let severity: [String: cribra_uniffi.Severity] = ["info": .info, "low": .low, "medium": .medium, "high": .high, "critical": .critical]
        let confidence: [String: Confidence] = ["low": .low, "medium": .medium, "high": .high]
        return (input["findings"] as! [[String: Any]]).map { f in
            Finding(ruleId: RuleId(value: f["id"] as! String), location: Location(
                start: (f["start"] as! NSNumber).uint64Value, end: (f["end"] as! NSNumber).uint64Value,
                line: (f["line"] as? NSNumber)?.uint64Value ?? 1, column: (f["column"] as? NSNumber)?.uint64Value ?? 1),
                severity: severity[f["severity"] as? String ?? "high"]!, confidence: confidence[f["confidence"] as? String ?? "high"]!)
        }
    }
    static func checkErrors() throws {
        let scanner = try scanner()
        let reports = try scanner.scanForShare(sources: [ScanSource(id: SourceId(value: "a"), text: "SECRET")])
        func rejection(_ sources: [String], _ selected: ShareBundleOptions, _ code: String, _ expected: UInt64?, _ actual: UInt64?, _ index: UInt64?, _ detail: String) throws {
            try compare(outcome { try build(reports: reports, sources: sources, options: selected) },
                ["error": ["kind": "bundle", "code": code, "expected": expected.map { $0 as Any } ?? NSNull(),
                    "actual": actual.map { $0 as Any } ?? NSNull(), "index": index.map { $0 as Any } ?? NSNull(), "detail": detail]])
        }
        try rejection([], options(0), "SourceCountMismatch", 1, 0, nil, "share bundle expected 1 sources but received 0")
        try rejection(["SECRET", "extra"], options(0), "SourceCountMismatch", 1, 2, nil, "share bundle expected 1 sources but received 2")
        try rejection(["short"], options(0), "SourceLengthMismatch", 6, 5, 0, "share bundle source 0 has 5 bytes but scan results record 6")
        try rejection([], ShareBundleOptions(mode: nil), "MissingShareMode", nil, nil, nil, "share bundle transformation mode was not configured")
        let missing = outcome { try scanAndBuild(scanner: scanner, sources: [], options: ShareBundleOptions(mode: nil)) }
        try compare(missing, ["error": ["kind": "bundle", "code": "MissingShareMode", "expected": NSNull(), "actual": NSNull(), "index": NSNull(), "detail": "share bundle transformation mode was not configured"]])
        // Core checks byte lengths, not identity: same-length changed content is accepted.
        let changed = try build(reports: reports, sources: ["PUBLIC"], options: options(0))
        precondition(changed.sources[0].content == "[REDACTED]")
        for count in [0, 1, 31, 33, 64] {
            let invalid = Data(repeating: 90, count: count)
            let modes: [ShareMode] = [.pseudonymize(options: PseudonymizationOptions(key: invalid, prefix: nil, digestBytes: nil)),
                .synthesize(options: SynthesisOptions(key: invalid, marker: nil))]
            for mode in modes {
                for atomic in [false, true] {
                    let result = outcome {
                        if atomic { return try scanAndBuild(scanner: scanner, sources: [], options: ShareBundleOptions(mode: mode)) }
                        return try build(reports: [], sources: [], options: ShareBundleOptions(mode: mode))
                    }
                    try compare(result, ["error": ["kind": "transform", "code": "InvalidKeyLength", "span": NSNull(), "otherSpan": NSNull(), "sourceLen": NSNull(), "detail": "transformation key must contain exactly 32 bytes; received \(count)"]])
                }
            }
        }
    }
    static func main() throws {
        let batches = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))) as! [[String: Any]]
        let explicit = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [[String: Any]]
        let oracle = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))) as! [String: [[String: Any]]]
        var batchIndex = 0
        var candidateCount = 0
        for batch in batches {
            let scanner = try scanner(batch["builtins"] as? Bool ?? false)
            let sources = (batch["sources"] as! [[String: Any]]).map { ScanSource(id: SourceId(value: $0["id"] as! String), text: $0["text"] as! String) }
            let reports = try scanner.scanForShare(sources: sources)
            precondition(reports.count == sources.count)
            candidateCount += reports.reduce(0) { $0 + $1.candidates.count }
            for mode in 0..<4 {
                for atomic in [false, true] {
                    let actual = outcome {
                        if atomic { return try scanAndBuild(scanner: scanner, sources: sources, options: options(mode)) }
                        return try build(reports: reports, sources: sources.map(\.text), options: options(mode))
                    }
                    try compare(actual, oracle["batch"]![batchIndex]["outcome"] as! [String: Any])
                    batchIndex += 1
                }
            }
        }
        precondition(candidateCount > 0)
        var explicitIndex = 0
        for input in explicit {
            let source = input["source"] as! String
            let report = ShareReport(sourceId: SourceId(value: "明示😀"), sourceBytes: UInt64(source.utf8.count), findings: findings(input), candidates: [])
            for mode in 0..<4 {
                let actual = outcome { try build(reports: [report], sources: [source], options: options(mode)) }
                try compare(actual, oracle["explicit"]![explicitIndex]["outcome"] as! [String: Any])
                explicitIndex += 1
            }
        }
        precondition(batchIndex == oracle["batch"]!.count && explicitIndex == oracle["explicit"]!.count)
        try checkErrors()
        print("PASS: Share Bundles; \(batchIndex) batch + \(explicitIndex) explicit Core outcomes, all modes, both paths, Unicode/duplicate IDs, candidates, timestamps, errors, no key echo")
    }
}
