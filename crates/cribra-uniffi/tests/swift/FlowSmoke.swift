import Foundation
import cribra_uniffi

@main
struct FlowSmoke {
    static func byteSpan(_ span: ByteSpan?) -> Any {
        span.map { ["start": $0.start, "end": $0.end] as Any } ?? NSNull()
    }

    static func codeName(_ code: TransformErrorCode) -> String {
        switch code {
        case .invalidSpan: return "InvalidSpan"
        case .invalidUtf8Boundary: return "InvalidUtf8Boundary"
        case .overlappingSpans: return "OverlappingSpans"
        case .inputConversion: return "InputConversion"
        case .unknownCoreError: return "UnknownCoreError"
        case .invalidKeyLength: return "InvalidKeyLength"
        }
    }

    static func outcome(_ operation: () throws -> TransformResult) -> NSDictionary {
        do {
            return ["content": try operation().content]
        } catch let error as TransformError {
            switch error {
            case .Rejected(let code, let span, let other, let length, let detail):
                return ["error": ["code": codeName(code), "span": byteSpan(span),
                    "otherSpan": byteSpan(other), "sourceLen": length.map { $0 as Any } ?? NSNull(),
                    "detail": detail]]
            }
        } catch { preconditionFailure("unexpected transform error: \(error)") }
    }

    static func compare(_ actual: NSDictionary, _ expected: [String: Any], _ name: String) {
        if let content = expected["content"] as? String {
            precondition(actual["error"] == nil, "unexpected error: \(name)")
            // Swift String equality can hide canonical Unicode differences.
            precondition(Array((actual["content"] as! String).utf8) == Array(content.utf8), "output bytes: \(name)")
        } else {
            precondition(actual.isEqual(expected), "structured Core error: \(name)")
        }
    }

    static func findings(_ input: [String: Any]) -> [Finding] {
        let severities: [String: cribra_uniffi.Severity] = ["info": .info, "low": .low, "medium": .medium, "high": .high, "critical": .critical]
        let confidences: [String: Confidence] = ["low": .low, "medium": .medium, "high": .high]
        return (input["findings"] as! [[String: Any]]).map { finding in
            Finding(ruleId: RuleId(value: finding["id"] as! String),
                location: Location(start: (finding["start"] as! NSNumber).uint64Value,
                    end: (finding["end"] as! NSNumber).uint64Value,
                    line: (finding["line"] as? NSNumber)?.uint64Value ?? 1,
                    column: (finding["column"] as? NSNumber)?.uint64Value ?? 1),
                severity: severities[finding["severity"] as? String ?? "high"]!,
                confidence: confidences[finding["confidence"] as? String ?? "high"]!)
        }
    }

    // Fixed PUBLIC test vectors; production APIs never generate or default keys.
    static func testKey(_ selector: String) -> Data {
        switch selector {
        case "a": return Data("0123456789ABCDEF0123456789ABCDEF".utf8)
        case "b": return Data("FEDCBA9876543210FEDCBA9876543210".utf8)
        default: preconditionFailure("unknown public test vector selector")
        }
    }

    static func pseudoOptions(_ input: [String: Any]) -> PseudonymizationOptions {
        PseudonymizationOptions(key: testKey(input["key"] as! String), prefix: input["prefix"] as? String,
            digestBytes: (input["digestBytes"] as? NSNumber)?.uint64Value)
    }

    static func synthesisOptions(_ input: [String: Any]) -> SynthesisOptions {
        SynthesisOptions(key: testKey(input["key"] as! String), marker: input["marker"] as? String)
    }

    static func checkKeyed(_ base: [[String: Any]], _ root: [String: Any]) throws {
        let extraData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
        let extra = try JSONSerialization.jsonObject(with: extraData) as! [[String: Any]]
        let optionData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[4]))
        let configs = try JSONSerialization.jsonObject(with: optionData) as! [[String: Any]]
        let cases = base + extra
        let expected = root["keyed"] as! [[String: Any]]
        precondition(cases.count == 27 && configs.count == 9 && expected.count == cases.count * configs.count)
        var index = 0
        for input in cases {
            for config in configs {
                let oracle = expected[index]
                index += 1
                let name = input["name"] as! String
                let configName = config["name"] as! String
                precondition(name == oracle["name"] as! String && configName == oracle["options"] as! String)
                let source = input["source"] as! String
                let spans = findings(input)
                let pseudo = outcome { try pseudonymize(source: source, findings: spans, options: pseudoOptions(config)) }
                let synthetic = outcome { try synthesize(source: source, findings: spans, options: synthesisOptions(config)) }
                compare(pseudo, oracle["pseudonymize"] as! [String: Any], "pseudonymize \(name)/\(configName)")
                compare(synthetic, oracle["synthesize"] as! [String: Any], "synthesize \(name)/\(configName)")
                let repeatedPseudo = outcome { try pseudonymize(source: source, findings: spans, options: pseudoOptions(config)) }
                let repeatedSynthetic = outcome { try synthesize(source: source, findings: spans, options: synthesisOptions(config)) }
                compare(repeatedPseudo, pseudo as! [String: Any], "pseudonymize determinism")
                compare(repeatedSynthetic, synthetic as! [String: Any], "synthesis determinism")
                let keyText = String(decoding: testKey(config["key"] as! String), as: UTF8.self)
                for result in [pseudo, synthetic] {
                    let data = try JSONSerialization.data(withJSONObject: result)
                    precondition(!String(decoding: data, as: UTF8.self).contains(keyText), "key echoed in result/error")
                }
                if configName == "digest-usize-max" { precondition(pseudoOptions(config).digestBytes == UInt64.max) }
            }
        }
        for length in [0,1,31,33,64,1024] {
            let key = Data(repeating: 0xa5, count: length)
            let expected: [String: Any] = ["error": ["code": "InvalidKeyLength", "span": NSNull(),
                "otherSpan": NSNull(), "sourceLen": NSNull(),
                "detail": "transformation key must contain exactly 32 bytes; received \(length)"]]
            for (source, spans) in [("", [Finding]()), ("abc", findings(base[12]))] {
                let pseudo = outcome { try pseudonymize(source: source, findings: spans, options: PseudonymizationOptions(key: key, prefix: nil, digestBytes: nil)) }
                let synthetic = outcome { try synthesize(source: source, findings: spans, options: SynthesisOptions(key: key, marker: nil)) }
                compare(pseudo, expected, "invalid pseudonymization key length")
                compare(synthetic, expected, "invalid synthesis key length")
            }
        }
        let first = cases.first { $0["name"] as! String == "content-a" }!
        let second = cases.first { $0["name"] as! String == "content-b" }!
        func pseudo(_ input: [String: Any], _ options: PseudonymizationOptions) throws -> String {
            try pseudonymize(source: input["source"] as! String, findings: findings(input), options: options).content
        }
        func synthesis(_ input: [String: Any], _ options: SynthesisOptions) throws -> String {
            try synthesize(source: input["source"] as! String, findings: findings(input), options: options).content
        }
        let firstPseudo = try pseudo(first, pseudoOptions(configs[0]))
        let secondKeyPseudo = try pseudo(first, pseudoOptions(configs[1]))
        precondition(firstPseudo != secondKeyPseudo)
        let firstSynthesis = try synthesis(first, synthesisOptions(configs[0]))
        let secondKeySynthesis = try synthesis(first, synthesisOptions(configs[1]))
        let changedContentSynthesis = try synthesis(second, synthesisOptions(configs[0]))
        precondition(firstSynthesis != secondKeySynthesis && firstSynthesis == changedContentSynthesis)
        let digestCases: [(UInt64, Int)] = [(0,8),(1,8),(8,8),(16,16),(32,32),(33,32),(.max,32)]
        for (requested, digestLength) in digestCases {
            let output = try pseudo(first, PseudonymizationOptions(key: testKey("a"), prefix: "p:", digestBytes: requested))
            precondition(output.utf8.count == 2 + 2 * digestLength, "Core digest clamping")
        }
        for text in ["", "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"] {
            let pseudo = try pseudonymize(source: text, findings: [], options: pseudoOptions(configs[0]))
            let synthesis = try synthesize(source: text, findings: [], options: synthesisOptions(configs[0]))
            precondition(Array(pseudo.content.utf8) == Array(text.utf8) && Array(synthesis.content.utf8) == Array(text.utf8), "implicit keyed scan")
        }
        print("PASS: keyed Flow; 27 inputs × 9 options, 486 Core outcomes, repeated determinism, key lengths, digest clamping, no key echo or rescanning")
    }

    static func main() throws {
        precondition(CommandLine.arguments.count == 5)
        let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
        let cases = try JSONSerialization.jsonObject(with: data) as! [[String: Any]]
        let oracleData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))
        let root = try JSONSerialization.jsonObject(with: oracleData) as! [String: Any]
        let expected = root["redactTemplate"] as! [[String: Any]]
        precondition(cases.count == 23 && cases.count == expected.count)
        for (input, expected) in zip(cases, expected) {
            let name = input["name"] as! String
            precondition(name == expected["name"] as! String)
            let source = input["source"] as! String
            let findings = Self.findings(input)
            let redacted = outcome {
                try redact(source: source, findings: findings, options: RedactOptions(replacement: input["replacement"] as? String))
            }
            if name == "large-u64-range" { precondition(findings[0].location.end == UInt64.max) }
            let templated = outcome {
                try template(source: source, findings: findings, options: TemplateOptions(namespace: input["namespace"] as? String, numbered: input["numbered"] as? Bool ?? false))
            }
            compare(redacted, expected["redact"] as! [String: Any], "redact \(name)")
            compare(templated, expected["template"] as! [String: Any], "template \(name)")
        }
        let result: TransformResult = try {
            let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"
            let scanner = cribra_uniffi.Scanner()
            let report = scanner.scan(source: ScanSource(id: SourceId(value: "input"), text: source))
            precondition(!report.findings.isEmpty)
            let untouched = try redact(source: source, findings: [], options: RedactOptions(replacement: nil))
            precondition(Array(untouched.content.utf8) == Array(source.utf8), "implicit scan")
            return try redact(source: source, findings: report.findings, options: RedactOptions(replacement: nil))
        }()
        precondition(result.content == "GITHUB_TOKEN=[REDACTED]")
        print("PASS: Flow Redact/Template; 23 fixtures, 46 Core outcomes, UTF-8 bytes, overlap, structured errors, no rescanning, owned output")
        try checkKeyed(cases, root)
    }
}
