import Foundation
import Synchronization
import cribra_uniffi

@main
struct Smoke {
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

    static func severityName(_ value: cribra_uniffi.Severity) -> String {
        switch value {
        case .info: return "info"
        case .low: return "low"
        case .medium: return "medium"
        case .high: return "high"
        case .critical: return "critical"
        }
    }

    static func confidenceName(_ value: Confidence) -> String {
        switch value {
        case .low: return "low"
        case .medium: return "medium"
        case .high: return "high"
        }
    }

    static func reportSnapshot(_ report: ScanReport) -> [String: Any] {
        [
            "sourceId": report.sourceId.value,
            "sourceBytes": report.sourceBytes,
            "candidateCount": report.candidateCount,
            "needsReview": report.needsReview,
            "candidates": report.candidates.map(candidateSnapshot),
            "findings": report.findings.map { finding -> [String: Any] in
                [
                    "ruleId": finding.ruleId.value,
                    "start": finding.location.start, "end": finding.location.end,
                    "line": finding.location.line, "column": finding.location.column,
                    "severity": severityName(finding.severity),
                    "confidence": confidenceName(finding.confidence),
                    "remediation": finding.remediation.map { remediationName($0) as Any } ?? NSNull(),
                    "explanation": explanationSnapshot(finding.explanation),
                ]
            },
        ]
    }

    static func snapshot(_ result: ScanResults) -> NSDictionary {
        let summary = result.summary
        return [
            "reports": result.reports.map(reportSnapshot),
            "summary": [
                "scannedSources": summary.scannedSources,
                "scannedBytes": summary.scannedBytes,
                "reportsWithFindings": summary.reportsWithFindings,
                "reportsWithoutFindings": summary.reportsWithoutFindings,
                "totalFindings": summary.totalFindings,
                "totalCandidates": summary.totalCandidates,
                "reportsWithCandidates": summary.reportsWithCandidates,
                "critical": summary.critical, "high": summary.high,
                "medium": summary.medium, "low": summary.low, "info": summary.info,
            ],
        ] as NSDictionary
    }

    static func checkScanner() throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))
        let values = try JSONSerialization.jsonObject(with: fixture) as! [[String: String]]
        let sources = values.map { ScanSource(id: SourceId(value: $0["id"]!), text: $0["text"]!) }
        let oracleData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
        let allOracle = try JSONSerialization.jsonObject(with: oracleData) as! NSDictionary
        let oracle = ["reports": allOracle["reports"]!, "summary": allOracle["summary"]!] as NSDictionary
        weak var released: cribra_uniffi.Scanner?
        var retained: ScanResults?
        do {
            let scanner = cribra_uniffi.Scanner()
            released = scanner
            let batch = scanner.scanMany(sources: sources)
            precondition(snapshot(batch).isEqual(oracle), "Swift/Core observable parity mismatch")
            precondition(batch.reports.count == sources.count, "input/report cardinality mismatch")
            for (source, report) in zip(sources, batch.reports) {
                precondition(Array(report.sourceId.value.utf8) == Array(source.id.value.utf8), "source ID bytes changed")
                precondition(report.sourceBytes == UInt64(source.text.utf8.count), "source byte length mismatch")
                let single = scanner.scan(source: source)
                precondition(NSDictionary(dictionary: reportSnapshot(single)).isEqual(reportSnapshot(report)), "single/batch mismatch")
            }
            precondition(snapshot(scanner.scanMany(sources: sources)).isEqual(oracle), "reuse changed results")
            let reversed = scanner.scanMany(sources: Array(sources.reversed()))
            precondition(snapshot(ScanResults(reports: Array(reversed.reports.reversed()), summary: reversed.summary)).isEqual(oracle), "input order not preserved")
            let empty = scanner.scanMany(sources: [])
            precondition(empty.reports.isEmpty, "empty batch produced reports")
            let counts = snapshot(empty)["summary"] as! [String: UInt64]
            precondition(counts.values.allSatisfy { $0 == 0 }, "empty batch summary retained state")
            // No findings does not imply clean: Core candidates still require review.
            precondition(batch.reports[3].findings.isEmpty && batch.reports[3].candidateCount == 1 && batch.reports[3].needsReview, "candidate-only semantics changed")
            retained = batch
        }
        precondition(released == nil, "Swift Scanner reference outlived scope")
        precondition(snapshot(retained!).isEqual(oracle), "reports must outlive the Scanner")
        print("PASS: reusable Scanner Object; single/batch, Unicode IDs, duplicate IDs, empty input, Core summary and Rust/Swift parity")
    }

    static func kindName(_ kind: RuleKind) -> String {
        switch kind {
        case .literal: return "Literal"
        case .prefix: return "Prefix"
        case .suffix: return "Suffix"
        case .pattern: return "Pattern"
        }
    }

    static func modeName(_ mode: DetectionMode) -> String {
        switch mode {
        case .matcherOnly: return "MatcherOnly"
        case .deterministic: return "Deterministic"
        case .contextual: return "Contextual"
        }
    }

    static func codeName(_ code: BuildErrorCode) -> String {
        switch code {
        case .invalidPattern: return "InvalidPattern"
        case .patternMatchesEmpty: return "PatternMatchesEmpty"
        case .missingCaptureGroup: return "MissingCaptureGroup"
        case .emptyRuleId: return "EmptyRuleId"
        case .duplicateRuleId: return "DuplicateRuleId"
        case .emptyMatcher: return "EmptyMatcher"
        case .tooManyRules: return "TooManyRules"
        case .automatonBuild: return "AutomatonBuild"
        }
    }

    static func metadataSnapshot(_ rules: [RuleMetadata]) -> NSArray {
        rules.map { rule -> [String: Any] in
            [
                "id": rule.id.value, "kind": kindName(rule.kind),
                "detectionMode": modeName(rule.detectionMode),
                "severity": severityName(rule.severity),
                "remediation": rule.remediation.map { ["label": $0.label, "message": $0.message] as Any } ?? NSNull(),
            ]
        } as NSArray
    }

    static func checkConfigurations() throws {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[4]))
        let cases = try JSONSerialization.jsonObject(with: fixture) as! [[String: Any]]
        let oracleData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
        let oracle = try JSONSerialization.jsonObject(with: oracleData) as! [String: Any]
        let outcomes = oracle["configurations"] as! [[String: Any]]
        precondition(cases.count == outcomes.count)
        let kinds: [String: RuleKind] = ["literal": .literal, "prefix": .prefix, "suffix": .suffix, "pattern": .pattern]
        let severities: [String: cribra_uniffi.Severity] = ["info": .info, "low": .low, "medium": .medium, "high": .high, "critical": .critical]
        var successes = 0
        var failures = 0
        for (input, expected) in zip(cases, outcomes) {
            let name = input["name"] as! String
            precondition(name == expected["name"] as! String)
            let rules = (input["rules"] as! [[String: Any]]).map { rule -> RuleSpec in
                let remediation = (rule["remediation"] as? String).map { value -> Remediation in
                    precondition(value == "removeSensitiveValue", "unknown fixture remediation")
                    return .removeSensitiveValue
                }
                return RuleSpec(id: RuleId(value: rule["id"] as! String), kind: kinds[rule["kind"] as! String]!, value: rule["value"] as! String, severity: severities[rule["severity"] as! String]!, remediation: remediation)
            }
            let config: ScannerConfig
            switch input["mode"] as! String {
            case "builtInOnly": config = .builtInOnly
            case "customOnly": config = .customOnly(rules: rules)
            case "builtInAndCustom": config = .builtInAndCustom(rules: rules)
            case "catalogs": config = .catalogs(current: input["current"] as! Bool, financial: input["financial"] as! Bool, personal: input["personal"] as! Bool, rules: rules)
            default: fatalError("unknown fixture mode")
            }
            do {
                let scanner = try cribra_uniffi.Scanner.configured(config: config)
                precondition(expected["error"] == nil, "expected construction failure for \(name)")
                precondition(metadataSnapshot(scanner.activeRules()).isEqual(expected["rules"]!), "metadata/order mismatch for \(name)")
                let source = ScanSource(id: SourceId(value: name), text: input["source"] as? String ?? "")
                let batch = scanner.scanMany(sources: [source])
                precondition(snapshot(batch).isEqual(expected["results"]!), "configured Core scan mismatch for \(name)")
                precondition(NSDictionary(dictionary: reportSnapshot(scanner.scan(source: source))).isEqual(reportSnapshot(batch.reports[0])), "configured single/batch mismatch")
                precondition(snapshot(scanner.scanMany(sources: [source])).isEqual(expected["results"]!), "configured reuse mismatch")
                if name == "custom" {
                    try checkStreamingParity(scanner: scanner, source: source)
                    try checkIncremental(scanner: scanner)
                }
                if case .customOnly = config {
                    precondition(scanner.activeRules().count == rules.count, "implicit built-ins")
                }
                successes += 1
            } catch let error as ScannerBuildError {
                switch error {
                case .Rejected(let code, let ruleId, let detail):
                    let actual: NSDictionary = ["code": codeName(code), "ruleId": ruleId.map { $0.value as Any } ?? NSNull(), "detail": detail]
                    precondition(actual.isEqual(expected["error"]!), "structured error mismatch for \(name)")
                }
                failures += 1
            }
        }
        precondition(successes == 21 && failures == 19, "configuration coverage changed")
        let defaults = cribra_uniffi.Scanner()
        let builtins = try cribra_uniffi.Scanner.configured(config: .builtInOnly)
        precondition(metadataSnapshot(defaults.activeRules()).isEqual(metadataSnapshot(builtins.activeRules())), "Scanner() behavior changed")
        print("PASS: configurable Scanner; 21 successful configurations, 19 Core errors, active metadata/order and Rust/Swift parity")
    }

    static func checkStreamingParity(scanner: cribra_uniffi.Scanner, source: ScanSource) throws {
        let bytes = Data(source.text.utf8)
        let expected = reportSnapshot(scanner.scan(source: source))
        for split in 0...bytes.count {
            let session = try scanner.stream(sourceId: source.id)
            precondition(session.state() == .active)
            try session.push(chunk: Data(bytes.prefix(split)))
            try session.push(chunk: Data())
            try session.push(chunk: Data(bytes.dropFirst(split)))
            let report = try session.finish()
            precondition(NSDictionary(dictionary: reportSnapshot(report)).isEqual(expected), "stream parity at split \(split)")
            precondition(session.state() == .finished)
        }
        for size in [1, 2, 3, 7, 31] {
            let session = try scanner.stream(sourceId: source.id)
            for start in stride(from: 0, to: bytes.count, by: size) {
                try session.push(chunk: Data(bytes[start..<min(start + size, bytes.count)]))
            }
            let report = try session.finish()
            precondition(NSDictionary(dictionary: reportSnapshot(report)).isEqual(expected), "stream chunk-size parity")
        }
    }

    static func streamCodeName(_ code: StreamErrorCode) -> String {
        switch code {
        case .unsupportedRuleSet: return "UnsupportedRuleSet"
        case .notActive: return "NotActive"
        case .invalidUtf8: return "InvalidUtf8"
        case .incompleteUtf8: return "IncompleteUtf8"
        case .candidateLimitExceeded: return "CandidateLimitExceeded"
        case .resultLimitExceeded: return "ResultLimitExceeded"
        case .storageLimitExceeded: return "StorageLimitExceeded"
        case .storageRequirementOverflow: return "StorageRequirementOverflow"
        case .sourceOffsetOverflow: return "SourceOffsetOverflow"
        case .coordinateOverflow: return "CoordinateOverflow"
        case .executionInvariantViolated: return "ExecutionInvariantViolated"
        case .unknownCoreError: return "UnknownCoreError"
        case .limitOutOfRange: return "LimitOutOfRange"
        case .sessionPoisoned: return "SessionPoisoned"
        }
    }

    static func expectStreamError(_ expected: Any, _ operation: () throws -> Void) throws {
        do {
            try operation()
            preconditionFailure("stream operation should have failed")
        } catch let error as StreamError {
            switch error {
            case .Rejected(let code, let limit, let required, let detail):
                let actual: NSDictionary = [
                    "code": streamCodeName(code), "limit": limit.map { $0 as Any } ?? NSNull(),
                    "required": required.map { $0 as Any } ?? NSNull(), "detail": detail,
                ]
                precondition(actual.isEqual(expected), "Core/Swift stream error mismatch")
            }
        }
    }

    static func limits(candidates: UInt64? = nil, storage: UInt64? = nil, results: UInt64? = nil) -> StreamLimits {
        StreamLimits(maxCandidates: candidates, maxStreamStorageBytes: storage, maxRetainedResults: results)
    }

    static func streamScanner(kind: RuleKind = .literal, value: String = "SECRET") throws -> cribra_uniffi.Scanner {
        try cribra_uniffi.Scanner.configured(config: .customOnly(rules: [RuleSpec(
            id: RuleId(value: kind == .pattern ? "eof" : "literal"), kind: kind,
            value: value, severity: .high, remediation: nil
        )]))
    }

    static func checkStreamingErrors() throws {
        let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
        let oracle = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        let errors = oracle["streamErrors"] as! [String: Any]
        let sourceId = SourceId(value: "stream/文書é🙂")
        let scanner = try streamScanner()
        try expectStreamError(errors["unsupported"]!) { _ = try cribra_uniffi.Scanner().stream(sourceId: sourceId) }
        let unbounded = try streamScanner(kind: .pattern, value: "a+")
        try expectStreamError(errors["unsupported"]!) { _ = try unbounded.stream(sourceId: sourceId) }
        try expectStreamError(errors["storage"]!) { _ = try scanner.streamWithLimits(sourceId: sourceId, limits: limits(storage: 0)) }
        let storageError = errors["storage"] as! [String: Any]
        let required = (storageError["required"] as! NSNumber).uint64Value
        let exact = try scanner.streamWithLimits(sourceId: sourceId, limits: limits(storage: required))
        try exact.push(chunk: Data("SECRET".utf8))
        let exactReport = try exact.finish()
        precondition(exactReport.findings.count == 1 && exactReport.sourceBytes == 6)
        precondition(Array(exactReport.sourceId.value.utf8) == Array(sourceId.value.utf8))
        try expectStreamError(errors["notActive"]!) { _ = try exact.finish() }
        try expectStreamError(errors["notActive"]!) { try exact.push(chunk: Data()) }
        precondition(exact.state() == .finished)

        let malformed = try scanner.stream(sourceId: sourceId)
        try malformed.push(chunk: Data("SECRET ".utf8))
        try expectStreamError(errors["invalidUtf8"]!) { try malformed.push(chunk: Data([0xff])) }
        precondition(malformed.state() == .failed)
        try expectStreamError(errors["notActive"]!) { _ = try malformed.finish() }
        try expectStreamError(errors["notActive"]!) { try malformed.push(chunk: Data()) }
        precondition(malformed.state() == .failed)

        let incomplete = try scanner.stream(sourceId: sourceId)
        try incomplete.push(chunk: Data([0xf0]))
        try incomplete.push(chunk: Data([0x9f]))
        precondition(incomplete.state() == .active)
        try expectStreamError(errors["incompleteUtf8"]!) { _ = try incomplete.finish() }
        precondition(incomplete.state() == .failed)
        for (budget, key) in [(limits(candidates: 1), "candidate"), (limits(results: 1), "result")] {
            let session = try scanner.streamWithLimits(sourceId: sourceId, limits: budget)
            try session.push(chunk: Data("SECRET ".utf8))
            try expectStreamError(errors[key]!) { try session.push(chunk: Data("SECRET ".utf8)) }
            precondition(session.state() == .failed)
            try expectStreamError(errors["notActive"]!) { _ = try session.finish() }
        }
        let emptyScanner = try cribra_uniffi.Scanner.configured(config: .customOnly(rules: []))
        let review = try emptyScanner.streamWithLimits(sourceId: sourceId, limits: limits(results: 0))
        try review.push(chunk: Data("ABCD-EFGH-IJKL-MNOP".utf8))
        try expectStreamError(errors["eofResult"]!) { _ = try review.finish() }
        precondition(review.state() == .failed)
        let eofScanner = try streamScanner(kind: .pattern, value: "abc\\z")
        let eof = try eofScanner.streamWithLimits(sourceId: sourceId, limits: limits(candidates: 0))
        try eof.push(chunk: Data("abc".utf8))
        try expectStreamError(errors["eofCandidate"]!) { _ = try eof.finish() }
        precondition(eof.state() == .failed)

        let detached: StreamSession = try {
            let scoped = try streamScanner()
            return try scoped.stream(sourceId: sourceId)
        }()
        try detached.push(chunk: Data())
        let emptyReport = try detached.finish()
        precondition(emptyReport.sourceBytes == 0 && emptyReport.findings.isEmpty)
        print("PASS: streaming Data; all byte splits, UTF-8, terminal lifecycle, Core resource errors, no fallback")
    }

    static func incrementalCodeName(_ code: IncrementalErrorCode) -> String {
        switch code {
        case .notActive: return "NotActive"
        case .staleRevision: return "StaleRevision"
        case .invalidRange: return "InvalidRange"
        case .invalidUtf8Boundary: return "InvalidUtf8Boundary"
        case .revisionOverflow: return "RevisionOverflow"
        case .documentLengthOverflow: return "DocumentLengthOverflow"
        case .candidateLimitExceeded: return "CandidateLimitExceeded"
        case .cacheLimitExceeded: return "CacheLimitExceeded"
        case .resultLimitExceeded: return "ResultLimitExceeded"
        case .transactionStorageLimitExceeded: return "TransactionStorageLimitExceeded"
        case .resourceAccountingOverflow: return "ResourceAccountingOverflow"
        case .executionInvariantViolated: return "ExecutionInvariantViolated"
        case .unknownCoreError: return "UnknownCoreError"
        case .limitOutOfRange: return "LimitOutOfRange"
        case .offsetOutOfRange: return "OffsetOutOfRange"
        case .sessionPoisoned: return "SessionPoisoned"
        }
    }

    static func incrementalErrorSnapshot(_ error: IncrementalError) -> NSDictionary {
        switch error {
        case .Rejected(let code, let terminal, let expected, let actual, let limit, let required, let detail):
            return ["code": incrementalCodeName(code), "terminal": terminal,
                "expectedRevision": expected.map { $0 as Any } ?? NSNull(),
                "actualRevision": actual.map { $0 as Any } ?? NSNull(),
                "limit": limit.map { $0 as Any } ?? NSNull(),
                "required": required.map { $0 as Any } ?? NSNull(), "detail": detail]
        }
    }

    static func incrementalSnapshot(_ session: IncrementalSession) throws -> NSDictionary {
        let atomic = try session.snapshot()
        let revision = try session.revision()
        let report = try session.report()
        // This helper runs without concurrent writers; keep legacy getters qualified.
        precondition(atomic.revision == revision)
        precondition(NSDictionary(dictionary: reportSnapshot(atomic.report)).isEqual(reportSnapshot(report)))
        return ["revision": atomic.revision, "report": reportSnapshot(atomic.report),
            "state": session.state() == .active ? "Active" : "Failed"]
    }

    static func incrementalLimits(_ name: String, _ value: UInt64) -> IncrementalLimits {
        IncrementalLimits(maxCandidates: name == "candidates" ? value : nil,
            maxRetainedCacheRecords: name == "cache" ? value : nil,
            maxMaterializedResults: name == "results" ? value : nil,
            maxTransactionStorageBytes: name == "storage" ? value : nil)
    }

    static func expectIncrementalError(_ expected: Any, _ operation: () throws -> Void) throws {
        do {
            try operation()
            preconditionFailure("incremental operation should have failed")
        } catch let error as IncrementalError {
            precondition(incrementalErrorSnapshot(error).isEqual(expected), "Core/Swift incremental error mismatch")
        }
    }

    static func checkIncremental(scanner: cribra_uniffi.Scanner) throws {
        let fixtureData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[5]))
        let fixture = try JSONSerialization.jsonObject(with: fixtureData) as! [String: Any]
        let oracleData = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
        let root = try JSONSerialization.jsonObject(with: oracleData) as! [String: Any]
        let oracle = root["incremental"] as! [String: Any]
        let trace = oracle["trace"] as! [[String: Any]]
        let sourceId = SourceId(value: fixture["id"] as! String)
        var text = fixture["source"] as! String
        let session = try scanner.incremental(source: ScanSource(id: sourceId, text: text))
        let initial = try incrementalSnapshot(session)
        precondition(initial.isEqual(trace[0]["snapshot"]!))
        let edits = fixture["edits"] as! [[String: Any]]
        precondition(trace.count == edits.count + 1)
        for (index, edit) in edits.enumerated() {
            let expected = trace[index + 1]
            let revision = (edit["revision"] as! NSNumber).uint64Value
            let start = (edit["start"] as! NSNumber).uint64Value
            let end = (edit["end"] as! NSNumber).uint64Value
            let replacement = edit["replacement"] as! String
            let before = try incrementalSnapshot(session)
            do {
                let next = try session.applyEdit(expectedRevision: revision, start: start, end: end, replacement: replacement)
                precondition(expected["error"] is NSNull, "expected edit failure")
                precondition(next == (expected["newRevision"] as! NSNumber).uint64Value)
                var bytes = Array(text.utf8)
                bytes.replaceSubrange(Int(start)..<Int(end), with: replacement.utf8)
                text = String(decoding: bytes, as: UTF8.self)
            } catch let error as IncrementalError {
                precondition(incrementalErrorSnapshot(error).isEqual(expected["error"]!), "incremental error parity")
                let unchanged = try incrementalSnapshot(session)
                precondition(unchanged.isEqual(before), "recoverable error changed snapshot")
            }
            let snapshot = try incrementalSnapshot(session)
            precondition(snapshot.isEqual(expected["snapshot"]!), "incremental Core snapshot mismatch")
            let report = try session.report()
            precondition(NSDictionary(dictionary: reportSnapshot(report)).isEqual(reportSnapshot(scanner.scan(source: ScanSource(id: sourceId, text: text)))), "incremental/whole-source parity")
        }

        let literal = try streamScanner(value: "x")
        for expected in oracle["resources"] as! [[String: Any]] {
            let name = expected["name"] as! String
            let owner = name == "storage" ? try cribra_uniffi.Scanner.configured(config: .customOnly(rules: [])) : literal
            let initialText = name == "storage" ? "" : "x"
            let document = try owner.incrementalWithLimits(source: ScanSource(id: sourceId, text: initialText), limits: incrementalLimits(name, name == "storage" ? 0 : 1))
            if name == "storage" { _ = try document.applyEdit(expectedRevision: 0, start: 0, end: 0, replacement: "") }
            let before = try incrementalSnapshot(document)
            precondition(before.isEqual(expected["before"]!))
            let replacement = ["candidates": "xx", "cache": "", "results": "x", "storage": "a"][name]!
            let revision = try document.revision()
            try expectIncrementalError(expected["error"]!) { _ = try document.applyEdit(expectedRevision: revision, start: 0, end: 0, replacement: replacement) }
            let after = try incrementalSnapshot(document)
            precondition(after.isEqual(expected["after"]!), "terminal failure lost last snapshot")
            try expectIncrementalError(expected["laterError"]!) { _ = try document.applyEdit(expectedRevision: .max, start: 999, end: 0, replacement: "") }
            let retained = try document.report()
            precondition(NSDictionary(dictionary: reportSnapshot(retained)).isEqual(before["report"]!))
        }
        for expected in oracle["construction"] as! [[String: Any]] {
            let name = expected["name"] as! String
            try expectIncrementalError(expected["error"]!) { _ = try literal.incrementalWithLimits(source: ScanSource(id: sourceId, text: "x"), limits: incrementalLimits(name, 0)) }
        }
        try checkIncrementalOwnershipAndRace(sourceId: sourceId)
        try checkAtomicSnapshots(sourceId: sourceId)
        print("PASS: incremental revisions/no-ops, recovery, Core/whole-source parity, four budgets, last snapshot, ARC and concurrency")
    }

    static func checkIncrementalOwnershipAndRace(sourceId: SourceId) throws {
        weak var releasedScanner: cribra_uniffi.Scanner?
        weak var releasedSession: IncrementalSession?
        var retained: ScanReport?
        do {
            let document: IncrementalSession = try {
                let scanner = try streamScanner(value: "x")
                releasedScanner = scanner
                return try scanner.incremental(source: ScanSource(id: sourceId, text: ""))
            }()
            precondition(releasedScanner == nil, "session retained foreign Scanner wrapper")
            releasedSession = document
            // Each returned error is observed after the session's native mutex is released.
            let outcomes = Mutex<[Bool]>([])
            DispatchQueue.concurrentPerform(iterations: 2) { _ in
                do {
                    let revision = try document.applyEdit(expectedRevision: 0, start: 0, end: 0, replacement: "x")
                    precondition(revision == 1)
                    outcomes.withLock { $0.append(true) }
                } catch let error as IncrementalError {
                    switch error {
                    case .Rejected(let code, let terminal, let expected, let actual, _, _, _):
                        precondition(code == .staleRevision && !terminal && expected == 0 && actual == 1)
                    }
                    outcomes.withLock { $0.append(false) }
                } catch { preconditionFailure("unexpected concurrent error: \(error)") }
            }
            precondition(outcomes.withLock { $0.filter { $0 }.count } == 1)
            let revision = try document.revision()
            precondition(revision == 1 && document.state() == .active)
            retained = try document.report()
        }
        precondition(releasedSession == nil, "incremental Object ARC release failed")
        precondition(retained!.findings.count == 1 && retained!.sourceBytes == 1)
    }

    static func checkAtomicSnapshots(sourceId: SourceId) throws {
        let prefix = "é🙂e\u{301}\r\n"
        let chunk = "x😀e\u{301}\r\n"
        let steps: UInt64 = 128
        weak var released: IncrementalSession?
        var retained: IncrementalSnapshot?
        do {
            let owner = try streamScanner(value: "x")
            let document = try owner.incremental(source: ScanSource(id: sourceId, text: prefix))
            released = document
            let starts = (0..<4).map { _ in DispatchSemaphore(value: 0) }
            let finished = DispatchSemaphore(value: 0)
            DispatchQueue.concurrentPerform(iterations: 5) { worker in
                do {
                    if worker == 0 {
                        for revision in 0..<steps {
                            let count = (revision + 1) / 2
                            let end = UInt64(prefix.utf8.count) + count * UInt64(chunk.utf8.count)
                            starts.forEach { $0.signal() }
                            let next = try document.applyEdit(expectedRevision: revision, start: end, end: end,
                                replacement: revision % 2 == 0 ? chunk : "")
                            precondition(next == revision + 1)
                            for _ in 0..<4 { finished.wait() }
                        }
                    } else {
                        for step in 0..<steps {
                            starts[worker - 1].wait()
                            for _ in 0..<4 {
                                let snapshot = try document.snapshot()
                                precondition(snapshot.revision == step || snapshot.revision == step + 1)
                                let count = (snapshot.revision + 1) / 2
                                let report = snapshot.report
                                precondition(Array(report.sourceId.value.utf8) == Array(sourceId.value.utf8))
                                precondition(report.sourceBytes == UInt64(prefix.utf8.count) + count * UInt64(chunk.utf8.count))
                                precondition(report.findings.count == Int(count) && report.candidateCount == 0)
                                precondition(report.needsReview == (count != 0))
                                for (index, finding) in report.findings.enumerated() {
                                    let start = UInt64(prefix.utf8.count + index * chunk.utf8.count)
                                    precondition([finding.location.start, finding.location.end, finding.location.line, finding.location.column] == [start, start + 1, UInt64(index + 2), 1])
                                }
                            }
                            finished.signal()
                        }
                    }
                } catch { preconditionFailure("unexpected atomic snapshot/edit failure") }
            }
            retained = try document.snapshot()
            let revision = try document.revision()
            let report = try document.report()
            precondition(retained!.revision == steps && revision == steps)
            precondition(NSDictionary(dictionary: reportSnapshot(retained!.report)).isEqual(reportSnapshot(report)))
        }
        precondition(released == nil, "snapshot retained native Object")
        precondition(retained!.report.findings.count == 64)
        print("PASS: atomic incremental snapshot; four readers, 128 edits/no-ops, coherent UTF-8 geometry, legacy getters and ARC")
    }

    static func main() throws {
        precondition(CommandLine.arguments.count == 6, "expected Core version, source fixture, Core oracle, configuration fixture and incremental fixture paths")
        let version = cribraVersion()
        precondition(version == CommandLine.arguments[1], "Core version mismatch")
        precondition(smokeScanText(source: "").isEmpty, "empty source")
        precondition(smokeScanText(source: "ordinary text: é🙂").isEmpty, "clean source")

        // Synthetic fixture already used by Core tests; never a live credential.
        let token = "ghp_AbCdEf0123456789_AbCdEf0123456789"
        let prefix = "前\r\né🙂e\u{301} GITHUB_TOKEN="
        let source = prefix + token + "\n"
        let findings = smokeScanText(source: source)
        precondition(findings.count == 1, "expected exactly one built-in detection")
        let finding = findings[0]
        precondition(finding.ruleId.value == "github.classic-pat", "RuleId mismatch")
        precondition(finding.severity == .critical, "severity mismatch")
        precondition(finding.confidence == .high, "confidence mismatch")
        precondition(finding.location.start == 28, "UTF-8 start mismatch")
        precondition(finding.location.start == UInt64(prefix.utf8.count), "byte offset mismatch")
        precondition(finding.location.end == UInt64(prefix.utf8.count + token.utf8.count), "exclusive end mismatch")
        precondition(finding.location.line == 2, "CRLF line mismatch")
        precondition(finding.location.column == 19, "Unicode-scalar column mismatch")
        let bytes = Array(source.utf8)
        let matched = bytes[Int(finding.location.start)..<Int(finding.location.end)]
        precondition(String(decoding: matched, as: UTF8.self) == token, "UTF-8 range mismatch")
        print("PASS: Swift → UniFFI → Cribra Core \(version); version, detection, RuleId, UTF-8 coordinates")
        try checkScanner()
        try checkConfigurations()
        try checkStreamingErrors()
    }
}
