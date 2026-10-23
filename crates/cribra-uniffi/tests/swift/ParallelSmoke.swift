import Foundation
import cribra_uniffi

@main
struct ParallelSmoke {
    static func remediationName(_ value: Remediation) -> String {
        switch value {
        case .revokeAndRotateCredential: return "RevokeAndRotateCredential"
        case .rotateCredential: return "RotateCredential"
        case .rotatePassword: return "RotatePassword"
        case .replacePrivateKey: return "ReplacePrivateKey"
        case .removeSensitiveValue: return "RemoveSensitiveValue"
        case .reviewSensitiveHash: return "ReviewSensitiveHash"
        case .reviewPasswordVerifier: return "ReviewPasswordVerifier"
        }
    }
    static func explanationSnapshot(_ value: Explanation?) -> Any {
        guard let value else { return NSNull() }
        switch value {
        case .classified(let mode):
            let name: String
            switch mode {
            case .matcherOnly: name = "MatcherOnly"
            case .deterministic: name = "Deterministic"
            case .contextual: name = "Contextual"
            }
            return ["classified": name]
        case .ambiguous(.structural): return ["ambiguous": "Structural"]
        }
    }
    static func candidateSnapshot(_ candidate: SensitiveCandidate) -> [String: Any] {
        let kind: String
        switch candidate.kind { case .recoveryLikeCode: kind = "RecoveryLikeCode" }
        let evidence: String
        switch candidate.evidence { case .structural: evidence = "Structural" }
        return ["kind": kind, "evidence": evidence,
            "start": candidate.location.start, "end": candidate.location.end,
            "line": candidate.location.line, "column": candidate.location.column,
            "explanation": explanationSnapshot(candidate.explanation)]
    }

    static func severity(_ value: cribra_uniffi.Severity) -> String {
        switch value {
        case .info: return "info"
        case .low: return "low"
        case .medium: return "medium"
        case .high: return "high"
        case .critical: return "critical"
        }
    }
    static func confidence(_ value: Confidence) -> String {
        switch value {
        case .low: return "low"
        case .medium: return "medium"
        case .high: return "high"
        }
    }
    static func snapshot(_ result: ScanResults) -> [String: Any] {
        let s = result.summary
        return ["reports": result.reports.map { r -> [String: Any] in
            ["sourceId": r.sourceId.value, "sourceBytes": r.sourceBytes,
                "candidateCount": r.candidateCount, "needsReview": r.needsReview,
                "candidates": r.candidates.map(candidateSnapshot),
                "findings": r.findings.map { f -> [String: Any] in
                    ["ruleId": f.ruleId.value, "start": f.location.start, "end": f.location.end,
                        "line": f.location.line, "column": f.location.column,
                        "severity": severity(f.severity), "confidence": confidence(f.confidence),
                        "remediation": f.remediation.map { remediationName($0) as Any } ?? NSNull(),
                        "explanation": explanationSnapshot(f.explanation)]
                }]
            },
            "summary": ["scanned_sources": s.scannedSources, "scanned_bytes": s.scannedBytes,
                "reports_with_findings": s.reportsWithFindings, "reports_without_findings": s.reportsWithoutFindings,
                "total_findings": s.totalFindings, "total_candidates": s.totalCandidates,
                "reports_with_candidates": s.reportsWithCandidates, "critical": s.critical, "high": s.high,
                "medium": s.medium, "low": s.low, "info": s.info]]
    }
    static func encoded(_ value: [String: Any]) throws -> Data {
        try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    }
    static func config(_ mode: String, _ rules: [RuleSpec]) -> ScannerConfig {
        switch mode {
        case "builtInOnly": return .builtInOnly
        case "customOnly": return .customOnly(rules: rules)
        case "builtInAndCustom": return .builtInAndCustom(rules: rules)
        case "emptyRules": return .customOnly(rules: [])
        default: preconditionFailure("unknown fixture configuration")
        }
    }
    static func main() throws {
        let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))) as! [String: Any]
        let oracle = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [[String: Any]]
        let sources = (fixture["sources"] as! [[String: String]]).map {
            ScanSource(id: SourceId(value: $0["id"]!), text: $0["text"]!)
        }
        let levels: [String: cribra_uniffi.Severity] = ["info": .info, "low": .low, "medium": .medium, "high": .high, "critical": .critical]
        let rules = (fixture["rules"] as! [[String: String]]).map {
            RuleSpec(id: RuleId(value: $0["id"]!), kind: .literal, value: $0["value"]!, severity: levels[$0["severity"]!]!, remediation: nil)
        }
        var batches: [(String, [ScanSource])] = [("empty", [])]
        batches += sources.enumerated().map { ("single-\($0.offset)", [$0.element]) }
        batches.append(("multiple", sources))
        batches.append(("reversed", Array(sources.reversed())))
        batches.append(("large", (0..<64).map { sources[$0 % sources.count] }))
        var count = 0
        for mode in ["builtInOnly", "customOnly", "builtInAndCustom", "emptyRules"] {
            weak var released: cribra_uniffi.Scanner?
            var retained: ScanResults?
            do {
                let scanner = try cribra_uniffi.Scanner.configured(config: config(mode, rules))
                released = scanner
                for (name, inputs) in batches {
                    let entry = oracle[count]
                    precondition(entry["mode"] as! String == mode && entry["batch"] as! String == name)
                    let expected = try encoded(entry["outcome"] as! [String: Any])
                    for _ in 0..<8 {
                        // All calls are synchronous; no Swift tasks, pools or dispatch queues.
                        let parallel = scanner.parallelScan(sources: inputs)
                        let parallelSnapshot = try encoded(snapshot(parallel))
                        let sequentialSnapshot = try encoded(snapshot(scanner.scanMany(sources: inputs)))
                        precondition(parallelSnapshot == expected, "Swift/Core parallel parity")
                        precondition(sequentialSnapshot == expected, "parallel/sequential parity")
                        precondition(parallel.reports.count == inputs.count)
                        for (source, report) in zip(inputs, parallel.reports) {
                            precondition(Array(source.id.value.utf8) == Array(report.sourceId.value.utf8), "ID bytes/order")
                            precondition(report.sourceBytes == UInt64(source.text.utf8.count))
                        }
                        retained = parallel
                    }
                    if mode == "builtInAndCustom" && name == "multiple" {
                        let result = retained!
                        let s = result.summary
                        precondition([s.scannedSources, s.reportsWithFindings, s.reportsWithoutFindings] == [7, 4, 3])
                        precondition([s.totalFindings, s.totalCandidates, s.reportsWithCandidates] == [11, 1, 1])
                        precondition([s.critical, s.high, s.medium, s.low, s.info] == [4, 3, 1, 1, 2])
                        let candidate = result.reports[3]
                        precondition(candidate.findings.isEmpty && candidate.candidateCount == 1 && candidate.needsReview)
                        precondition(Array(result.reports[5].sourceId.value.utf8) != Array(result.reports[6].sourceId.value.utf8))
                        let l = result.reports[0].findings[0].location
                        precondition([l.start, l.end, l.line, l.column] == [28, 65, 2, 19])
                    }
                    count += 1
                }
            }
            precondition(released == nil, "Scanner ARC reference retained")
            // Owned DTOs remain usable after the scanner has been released.
            precondition(retained!.reports.count == 64)
            precondition(retained!.summary.scannedSources == 64)
        }
        precondition(count == oracle.count)
        print("PASS: parallelScan fuse; \(count) Core scenarios × 8 repeats, sequential parity, all DTO fields/counters, Unicode/duplicate IDs, UTF-8 coordinates, candidates and owned results")
    }
}
