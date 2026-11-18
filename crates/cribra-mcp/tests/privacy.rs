//! Privacy and isolation qualification against the real binary.
//!
//! Every secret in this file is synthetic. Canaries are chosen so that a single
//! substring search over a captured response or a captured stderr stream proves
//! whether client input leaked. `cribra_scan` is expected to be leak-free;
//! `cribra_transform` intentionally returns transformed source and is not
//! anonymization, which is asserted explicitly rather than glossed over.

mod support;

use std::time::Duration;

use serde_json::{Value, json};
use support::{ServerProcess, validate_official, validate_tool_output};

const MODERN: &str = "2026-07-28";
const DEADLINE: Duration = Duration::from_secs(20);

/// Synthetic values. None of these are real credentials.
const MATCHED: &str = "SYNTH_SECRET_ALPHA_7Q4";
const CONTEXT_BEFORE: &str = "prefix-context-8c1";
const CONTEXT_AFTER: &str = "suffix-context-2f9";
const PATTERN: &str = "SYNTH_SECRET_[A-Z]+_[0-9A-Z]{3}";
const RULE_ID: &str = "custom.synthetic_probe";

fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"_meta": meta(), "name": name, "arguments": arguments},
    })
}

fn source() -> String {
    format!("{CONTEXT_BEFORE} {MATCHED} {CONTEXT_AFTER}")
}

fn round_trip(server: &mut ServerProcess, request: &Value) -> Value {
    server.send(request);
    let response = server.next_json(DEADLINE);
    validate_official(MODERN, &response);
    response
}

fn assert_no_canaries(value: &Value, what: &str) {
    let encoded = value.to_string();
    for canary in [MATCHED, CONTEXT_BEFORE, CONTEXT_AFTER, PATTERN] {
        assert!(
            !encoded.contains(canary),
            "{what} leaked {canary:?}: {encoded}"
        );
    }
}

#[test]
fn scan_never_emits_matched_values_source_or_patterns() {
    let mut server = ServerProcess::spawn();
    let response = round_trip(
        &mut server,
        &call(
            1,
            "cribra_scan",
            json!({
                "source": source(),
                "rules": [{"id": RULE_ID, "pattern": PATTERN, "severity": "high"}],
            }),
        ),
    );

    let result = &response["result"];
    assert_eq!(result["isError"], false);
    assert_no_canaries(&response, "scan response");

    let structured = &result["structuredContent"];
    validate_tool_output("cribra_scan", structured);
    let findings = structured["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["ruleId"], RULE_ID);
    // Coordinates are the only positional information returned.
    assert_eq!(
        findings[0]["start"],
        json!(CONTEXT_BEFORE.len() + 1),
        "the finding offset must be an exact UTF-8 byte offset"
    );
    assert_eq!(
        findings[0]["end"],
        json!(CONTEXT_BEFORE.len() + 1 + MATCHED.len())
    );

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(
        server.stderr_text(),
        "",
        "stderr must not describe findings"
    );
}

#[test]
fn tool_errors_never_echo_arguments_or_rule_patterns() {
    let mut server = ServerProcess::spawn();

    // Unknown sibling field: the whole call is rejected without echoing it.
    let unknown_field = round_trip(
        &mut server,
        &call(
            1,
            "cribra_scan",
            json!({"source": source(), "path": CONTEXT_BEFORE}),
        ),
    );
    assert_eq!(unknown_field["result"]["isError"], true);
    assert_no_canaries(&unknown_field, "invalid_arguments error");
    validate_tool_output("cribra_scan", &unknown_field["result"]["structuredContent"]);

    // A pattern that cannot compile must be reported as a fixed code. The
    // pattern source itself must not appear.
    let invalid_pattern = round_trip(
        &mut server,
        &call(
            2,
            "cribra_scan",
            json!({
                "source": source(),
                "rules": [{"id": RULE_ID, "pattern": "SYNTH_SECRET_(", "severity": "high"}],
            }),
        ),
    );
    assert_eq!(invalid_pattern["result"]["isError"], true);
    assert_eq!(
        invalid_pattern["result"]["structuredContent"]["error"]["code"],
        "invalid_rule_pattern"
    );
    assert!(
        !invalid_pattern.to_string().contains("SYNTH_SECRET_("),
        "the regex source must never be echoed"
    );
    assert_no_canaries(&invalid_pattern, "invalid_rule_pattern error");

    // Overlapping template spans produce a fixed code with no source.
    let overlapping = round_trip(
        &mut server,
        &call(
            3,
            "cribra_transform",
            json!({
                "source": "abc",
                "mode": "template",
                "rules": [
                    {"id": "custom.one", "pattern": "ab", "severity": "high"},
                    {"id": "custom.two", "pattern": "bc", "severity": "high"},
                ],
            }),
        ),
    );
    assert_eq!(overlapping["result"]["isError"], true);
    assert_eq!(
        overlapping["result"]["structuredContent"]["error"]["code"],
        "overlapping_spans"
    );
    assert!(
        !overlapping.to_string().contains("abc"),
        "a rejected transform must not return source"
    );

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn hostile_traffic_never_produces_descriptive_stderr() {
    let mut server = ServerProcess::spawn();

    // Malformed JSON carrying a canary, an oversized frame carrying a canary,
    // invalid UTF-8 and an unreadable identifier all at once.
    server.send_raw(&format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"source":"{CONTEXT_BEFORE}"}}"#
    ));

    let mut oversized = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"cribra_scan","arguments":{{"source":"{CONTEXT_AFTER}"#
    );
    oversized.push_str(&"b".repeat(cribra_mcp::limits::MAX_MESSAGE_BYTES));
    oversized.push_str(r#""}}}}}"#);
    server.send_raw(&oversized);

    let mut invalid_utf8 =
        b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\",\"params\":{\"x\":\"".to_vec();
    invalid_utf8.push(0xFE);
    invalid_utf8.extend_from_slice(b"\"}}");
    server.send_frame(&invalid_utf8);

    server.send_raw(r#"{"jsonrpc":"1.0","id":4,"method":"ping"}"#);

    for _ in 0..4 {
        let response = server.next_json(DEADLINE);
        validate_official(MODERN, &response);
        assert_no_canaries(&response, "transport error");
    }

    // Still healthy, and the diagnostics stream never became a log channel.
    let healthy = round_trip(
        &mut server,
        &json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "server/discover",
            "params": {"_meta": meta()},
        }),
    );
    assert_eq!(healthy["id"], 10);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    let stderr = server.stderr_text();
    assert_eq!(stderr, "", "hostile input must not produce diagnostics");
}

#[test]
fn transform_text_and_structured_output_agree_and_unchanged_regions_remain() {
    let mut server = ServerProcess::spawn();
    let response = round_trip(
        &mut server,
        &call(
            1,
            "cribra_transform",
            json!({
                "source": source(),
                "mode": "redact",
                "options": {"replacement": "[redacted]"},
                "rules": [{"id": RULE_ID, "pattern": PATTERN, "severity": "high"}],
            }),
        ),
    );

    let result = &response["result"];
    assert_eq!(result["isError"], false);
    validate_tool_output("cribra_transform", &result["structuredContent"]);

    let output = result["structuredContent"]["output"]
        .as_str()
        .expect("transformed output");
    assert_eq!(
        result["content"][0]["text"].as_str(),
        Some(output),
        "the text block and structuredContent must agree exactly"
    );
    assert!(
        !output.contains(MATCHED),
        "the matched span must be replaced"
    );
    assert_eq!(
        output,
        format!("{CONTEXT_BEFORE} [redacted] {CONTEXT_AFTER}"),
        "unchanged regions are returned verbatim"
    );

    // Documented limitation, asserted rather than implied: transform is not
    // anonymization, so unchanged source regions are still sensitive.
    assert!(output.contains(CONTEXT_BEFORE));
    assert!(output.contains(CONTEXT_AFTER));

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn template_mode_returns_stable_placeholders_not_source() {
    let mut server = ServerProcess::spawn();
    let response = round_trip(
        &mut server,
        &call(
            1,
            "cribra_transform",
            json!({
                "source": source(),
                "mode": "template",
                "options": {"namespace": "probe"},
                "rules": [{"id": RULE_ID, "pattern": PATTERN, "severity": "high"}],
            }),
        ),
    );

    let result = &response["result"];
    assert_eq!(result["isError"], false);
    validate_tool_output("cribra_transform", &result["structuredContent"]);
    let output = result["structuredContent"]["output"]
        .as_str()
        .expect("transformed output");
    assert_eq!(result["content"][0]["text"].as_str(), Some(output));
    assert!(!output.contains(MATCHED));
    assert!(
        output.contains("probe"),
        "the template namespace must appear in the placeholder: {output}"
    );

    // Applying the same rule to the same source twice is deterministic.
    let repeated = round_trip(
        &mut server,
        &call(
            2,
            "cribra_transform",
            json!({
                "source": source(),
                "mode": "template",
                "options": {"namespace": "probe"},
                "rules": [{"id": RULE_ID, "pattern": PATTERN, "severity": "high"}],
            }),
        ),
    );
    assert_eq!(
        repeated["result"]["structuredContent"]["output"], output,
        "template transformation must be deterministic"
    );

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}
