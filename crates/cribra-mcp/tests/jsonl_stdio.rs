//! End-to-end JSONL stdio tests for the real binary.

mod support;

use std::time::Duration;

use cribra_mcp::limits::MAX_MESSAGE_BYTES;
use serde_json::{Value, json};
use support::{ServerProcess, validate_official};

const MODERN_INPUT: &str = include_str!("fixtures/modern.input.jsonl");
const MODERN_OUTPUT: &str = include_str!("fixtures/modern.output.jsonl");
const LEGACY_INPUT: &str = include_str!("fixtures/legacy.input.jsonl");
const LEGACY_OUTPUT: &str = include_str!("fixtures/legacy.output.jsonl");
const DUAL_INPUT: &str = include_str!("fixtures/dual-era.input.jsonl");
const DUAL_OUTPUT: &str = include_str!("fixtures/dual-era.output.jsonl");

fn parse_jsonl(source: &str) -> Vec<Value> {
    source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("fixture line is JSON"))
        .collect()
}

fn run_transcript(input: &str, expected: &str, versions: &[&str]) -> (Vec<Value>, String) {
    let requests = parse_jsonl(input);
    let expected = parse_jsonl(expected);
    assert_eq!(
        expected.len(),
        versions.len(),
        "each expected response needs its wire-era version"
    );
    let mut server = ServerProcess::spawn();

    for request in &requests {
        server.send(request);
    }

    // Read every expected response while stdin is still open. After EOF the
    // server deliberately discards anything that was still queued.
    let mut responses = Vec::new();
    for version in versions {
        let response = server.next_json(Duration::from_secs(3));
        validate_official(version, &response);
        responses.push(response);
    }
    server.close_stdin();
    assert_eq!(
        server.next_line(Duration::from_millis(100)),
        None,
        "unexpected extra stdout line"
    );

    let status = server.wait(Duration::from_secs(3)).expect("server exits");
    assert!(status.success(), "unexpected exit status: {status}");
    let stderr = server.stderr_text();
    assert_eq!(stderr, "", "stderr must remain diagnostic-clean on success");
    (responses, stderr)
}

#[test]
fn modern_transcript_matches_pinned_expectations() {
    let (responses, _) = run_transcript(MODERN_INPUT, MODERN_OUTPUT, &["2026-07-28"; 5]);
    assert_eq!(responses, parse_jsonl(MODERN_OUTPUT));
}

#[test]
fn legacy_transcript_matches_pinned_expectations() {
    let (responses, _) = run_transcript(LEGACY_INPUT, LEGACY_OUTPUT, &["2025-11-25"; 3]);
    assert_eq!(responses, parse_jsonl(LEGACY_OUTPUT));
}

#[test]
fn dual_era_transcript_matches_pinned_expectations() {
    let (responses, _) = run_transcript(
        DUAL_INPUT,
        DUAL_OUTPUT,
        &["2026-07-28", "2025-11-25", "2026-07-28", "2025-11-25"],
    );
    assert_eq!(responses, parse_jsonl(DUAL_OUTPUT));
}

#[test]
fn fixture_requests_validate_against_official_schemas() {
    let cases: [(&str, &[&str]); 3] = [
        (MODERN_INPUT, &["2026-07-28"; 5]),
        (LEGACY_INPUT, &["2025-11-25"; 4]),
        (
            DUAL_INPUT,
            &[
                "2026-07-28",
                "2025-11-25",
                "2025-11-25",
                "2026-07-28",
                "2025-11-25",
            ],
        ),
    ];

    for (input, versions) in cases {
        let requests = parse_jsonl(input);
        assert_eq!(requests.len(), versions.len());
        for (request, version) in requests.iter().zip(versions) {
            validate_official(version, request);
        }
    }
}

#[test]
fn normal_operation_returns_one_response_per_accepted_request() {
    let mut server = ServerProcess::spawn();
    let mut next_id = 1;

    // Eight requests exactly fill the bounded request and writer queues. Read
    // every response before the next group so this measures normal operation,
    // not intentional overload or backpressure.
    for _group in 0..8 {
        let first_id = next_id;
        for _ in 0..8 {
            let id = next_id;
            next_id += 1;
            server.send(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "server/discover",
                "params": {
                    "_meta": {
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {},
                    }
                }
            }));
        }

        for expected_id in first_id..next_id {
            let response = server.next_json(Duration::from_secs(3));
            validate_official("2026-07-28", &response);
            assert_eq!(response["id"], json!(expected_id));
            assert_eq!(response["result"]["resultType"], "complete");
        }
    }

    server.close_stdin();
    assert_eq!(
        server.next_line(Duration::from_millis(100)),
        None,
        "no response may be stranded after EOF"
    );
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn modern_tools_list_and_stub_calls_are_clean() {
    let mut server = ServerProcess::spawn();
    let meta = json!({
        "_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
        }
    });

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": meta,
    }));
    let list = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &list);
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["cribra_scan", "cribra_transform"]);
    assert_eq!(list["result"]["ttlMs"], 0);
    assert_eq!(list["result"]["cacheScope"], "private");

    const CANARY: &str = "CRIBRA_TOOL_SOURCE_CANARY_7a2f";
    for (id, name) in [(2, "cribra_scan"), (3, "cribra_transform")] {
        server.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {},
                },
                "name": name,
                "arguments": {"source": CANARY},
            },
        }));
        let call = server.next_json(Duration::from_secs(3));
        validate_official("2026-07-28", &call);
        assert_eq!(call["result"]["isError"], true);
        assert_eq!(call["result"]["structuredContent"]["status"], "error");
        assert_eq!(call["result"]["resultType"], "complete");
        assert!(!call.to_string().contains(CANARY));
    }

    server.close_stdin();
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn legacy_tools_surface_omits_modern_fields() {
    let mut server = ServerProcess::spawn();
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "jsonl", "version": "1.0.0"},
        },
    }));
    let initialize = server.next_json(Duration::from_secs(3));
    validate_official("2025-11-25", &initialize);
    assert!(initialize["result"].get("resultType").is_none());

    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    server.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    let list = server.next_json(Duration::from_secs(3));
    validate_official("2025-11-25", &list);
    assert_eq!(list["result"]["tools"].as_array().expect("tools").len(), 2);
    assert!(list["result"].get("resultType").is_none());
    assert!(list["result"].get("ttlMs").is_none());

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {"name": "cribra_scan", "arguments": {"source": "hello"}},
    }));
    let call = server.next_json(Duration::from_secs(3));
    validate_official("2025-11-25", &call);
    assert_eq!(call["result"]["isError"], true);
    assert!(call["result"].get("resultType").is_none());

    server.close_stdin();
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn parse_batch_and_oversized_messages_are_fixed_and_source_free() {
    const CANARY: &str = "CRIBRA_SOURCE_CANARY_9f31";
    let mut server = ServerProcess::spawn();

    server.send_raw(&format!("{{\"secret\":\"{CANARY}\""));
    let parse_error = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &parse_error);
    assert!(parse_error.get("id").is_none());
    assert_eq!(parse_error["error"]["code"], -32700);
    assert_eq!(parse_error["error"]["message"], "Parse error");

    server.send(&json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}]));
    let batch = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &batch);
    assert_eq!(batch["error"]["code"], -32600);
    assert_eq!(
        batch["error"]["message"],
        "Batch requests are not supported"
    );

    server.send_raw(&"a".repeat(MAX_MESSAGE_BYTES + 17));
    let oversized = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &oversized);
    assert_eq!(oversized["error"]["code"], -32600);
    assert_eq!(
        oversized["error"]["message"],
        "Request exceeds maximum message size"
    );

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        }
    }));
    let discover = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &discover);
    assert_eq!(discover["result"]["resultType"], "complete");

    server.close_stdin();
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    let stderr = server.stderr_text();
    assert!(!stderr.contains(CANARY));
    assert!(!parse_error.to_string().contains(CANARY));
}

#[test]
fn notifications_produce_no_output_and_eof_exits_zero() {
    let mut server = ServerProcess::spawn();
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!(
        server.next_line(Duration::from_millis(100)),
        None,
        "notification must not produce stdout"
    );
    server.close_stdin();
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn empty_eof_exits_zero() {
    let mut server = ServerProcess::spawn();
    server.close_stdin();
    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn unknown_and_malformed_cancellations_are_silent_and_harmless() {
    let mut server = ServerProcess::spawn();
    server.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": 999},
    }));
    server.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": {"not": "a request id"}},
    }));
    assert_eq!(
        server.next_line(Duration::from_millis(100)),
        None,
        "cancellation notifications must not produce stdout"
    );

    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        }
    }));
    let response = server.next_json(Duration::from_secs(3));
    validate_official("2026-07-28", &response);
    assert_eq!(response["result"]["resultType"], "complete");

    server.close_stdin();
    assert!(server.wait(Duration::from_secs(3)).expect("exit").success());
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn legacy_request_without_session_is_rejected() {
    let mut server = ServerProcess::spawn();
    server.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
    let response = server.next_json(Duration::from_secs(3));
    validate_official("2025-11-25", &response);
    assert_eq!(response["error"]["code"], -32602);
    assert_eq!(
        response["error"]["message"],
        "Missing required _meta fields"
    );
    server.close_stdin();
    assert!(server.wait(Duration::from_secs(3)).expect("exit").success());
}
