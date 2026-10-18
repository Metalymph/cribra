// Test-only contract inventory for actual UniFFI 0.32.2 Swift generation.
import Foundation

@main
struct APIContract {
    static func matches(_ pattern: String, _ text: String) throws -> [[String]] {
        let regex = try NSRegularExpression(pattern: pattern)
        let input = text as NSString
        return regex.matches(in: text, range: NSRange(location: 0, length: input.length)).map { match in
            (1..<match.numberOfRanges).map { index in
                let range = match.range(at: index)
                return range.location == NSNotFound ? "" : input.substring(with: range)
            }
        }
    }
    static func normalized(_ value: String) -> String {
        value.replacingOccurrences(of: "\\s+", with: "", options: .regularExpression)
    }
    static func equal(_ actual: [String], _ expected: [String], _ label: String, ordered: Bool = false) {
        precondition(Set(actual.map(normalized)) == Set(expected.map(normalized)), "contract mismatch: \(label)")
        precondition(actual.count == expected.count, "duplicate contract entry: \(label)")
        if ordered { precondition(actual.map(normalized) == expected.map(normalized), "field order: \(label)") }
    }
    static func main() throws {
        precondition(CommandLine.arguments.count == 4)
        let profile = CommandLine.arguments[1]
        let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))) as! [String: Any]
        let text = try String(contentsOfFile: CommandLine.arguments[3], encoding: .utf8)
        let source = text.replacingOccurrences(of: "(?s)/\\*.*?\\*/", with: "", options: .regularExpression)
        let enabled: [String]
        switch profile {
        case "none": enabled = ["common"]
        case "core": enabled = ["common", "core"]
        case "flow": enabled = ["common", "core", "flow"]
        case "fuse": enabled = ["common", "core", "flow", "fuse"]
        default: preconditionFailure("unknown profile")
        }
        let groups = fixture["groups"] as! [String: [String: [String]]]
        let types = try matches("(?m)^public\\s+(?:struct|enum|protocol)\\s+(\\w+)|^open class (\\w+)", source)
            .map { $0.first { !$0.isEmpty }! }.filter { !$0.hasPrefix("FfiConverter") }
        let functions = try matches("(?m)^public func ((?!FfiConverter|uniffiEnsure)[^\\n{]+)", source).map { $0[0] }
        var methods: [String] = []
        for item in try matches("(?ms)^public protocol (\\w+)Protocol[^\\{]*\\{(.*?)^\\}", source) {
            methods += try matches("(?m)^\\s*func ([^\\n]+)", item[1]).map { item[0] + "." + $0[0] }
        }
        let constructors = try matches("(?m)^public convenience (init[^\\n{]+)|^public static func ([^\\n{]+)", source)
            .map { $0.first { !$0.isEmpty }! }
        equal(constructors, enabled.flatMap { groups[$0]!["constructors"]! }, "\(profile) constructors")
        equal(types, enabled.flatMap { groups[$0]!["types"]! }, "\(profile) types")
        equal(functions, enabled.flatMap { groups[$0]!["functions"]! }, "\(profile) functions")
        equal(methods, enabled.flatMap { groups[$0]!["methods"]! }, "\(profile) methods")
        for (name, body) in try matches("(?ms)^public\\s+struct (\\w+)[^\\{]*\\{(.*?)^\\}", source).map({ ($0[0], $0[1]) }) {
            if name.hasPrefix("FfiConverter") { continue }
            let fields = try matches("public var (\\w+): ([^\\n]+)", body).map { $0[0] + ": " + $0[1] }
            let expected = (fixture["recordFields"] as! [String: [String]])[name]!
            equal(fields, expected, "\(name) fields", ordered: true)
        }
        for (name, body) in try matches("(?ms)^public\\s+enum (\\w+)[^\\{]*\\{(.*?)^\\}", source).map({ ($0[0], $0[1]) }) {
            let cases = try matches("\\bcase (\\w+)\\s*(\\([^)]*\\))?", body).map { $0[0] + $0[1] }
            let expected = (fixture["enumCases"] as! [String: [String]])[name]!
            equal(cases, expected, "\(name) cases/error details")
        }
        print("PASS: \(profile) API contract; exact types/functions/methods, DTO fields, enum cases and structured error shapes")
    }
}
