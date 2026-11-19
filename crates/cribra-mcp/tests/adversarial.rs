//! Adversarial process-level qualification of the real `cribra-mcp` binary.
//!
//! Every test here spawns the shipping executable, speaks newline-delimited
//! JSON-RPC over its stdin/stdout, and validates every captured response
//! against the pinned official schema for the era it belongs to. Nothing is
//! mocked and no test reaches into the protocol layer directly.
//!
//! Race-sensitive tests use deterministic barriers: either a synchronous
//! round-trip that the server must answer before the next step, or a protocol
//! request whose response proves an earlier write completed. No test sleeps to
//! paper over a race.

mod support;

use std::collections::BTreeSet;
use std::time::Duration;

use cribra_mcp::limits::{MAX_MESSAGE_BYTES, MAX_REQUEST_ID_BYTES, MAX_SOURCE_BYTES};
use serde_json::{Value, json};
use support::{ServerProcess, validate_official};

const MODERN: &str = "2026-07-28";
const LEGACY: &str = "2025-11-25";
/// Bounded hang detector for round trips that perform Core work.
///
/// This file mixes protocol-only round trips with 1 MiB scans and broad regex
/// scans. The slowest CI runner has measured 4-6x the local wall time for the
/// whole binary, and tests run in parallel inside it; under controlled CPU
/// contention the 1 MiB boundary case needs about 22s. A tighter bound reports
/// scheduler delay as a transport failure, while this one still fails a
/// genuinely wedged server.
const DEADLINE: Duration = Duration::from_secs(120);

fn modern_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn modern(id: impl Into<Value>, method: &str, mut params: Value) -> Value {
    if let Value::Object(object) = &mut params {
        object.insert("_meta".to_owned(), modern_meta());
    }
    json!({"jsonrpc": "2.0", "id": id.into(), "method": method, "params": params})
}

fn modern_discover(id: impl Into<Value>) -> Value {
    modern(id, "server/discover", json!({}))
}

fn legacy_initialize(id: impl Into<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.into(),
        "method": "initialize",
        "params": {
            "protocolVersion": LEGACY,
            "capabilities": {},
            "clientInfo": {"name": "adversarial", "version": "1.0.0"},
        },
    })
}

fn legacy(id: impl Into<Value>, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id.into(), "method": method, "params": params})
}

fn initialized() -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
}

fn scan_call(id: impl Into<Value>, source: &str) -> Value {
    modern(
        id,
        "tools/call",
        json!({"name": "cribra_scan", "arguments": {"source": source}}),
    )
}

/// Sends one request, reads one response, validates it, and returns it.
fn round_trip(server: &mut ServerProcess, request: &Value, version: &str) -> Value {
    server.send(request);
    let response = server.next_json(DEADLINE);
    validate_official(version, &response);
    response
}

fn shutdown(mut server: ServerProcess) -> (i32, String) {
    server.close_stdin();
    let status = server.wait(DEADLINE).expect("server exits after EOF");
    let stderr = server.stderr_text();
    (status.code().expect("exit code"), stderr)
}

#[test]
fn malformed_framing_is_bounded_and_the_connection_survives() {
    let mut server = ServerProcess::spawn();

    // An empty line is not a JSON value. It must produce a bounded parse
    // error and must not disturb the next well-formed request.
    server.send_bytes(b"\n");
    let empty = server.next_json(DEADLINE);
    validate_official(MODERN, &empty);
    assert_eq!(empty["error"]["code"], -32700);
    assert_eq!(empty["error"]["message"], "Parse error");
    assert!(empty.get("id").is_none(), "id must be omitted, never null");

    // Non-JSON text.
    server.send_raw("not json at all");
    let text = server.next_json(DEADLINE);
    validate_official(MODERN, &text);
    assert_eq!(text["error"]["code"], -32700);

    // A JSON array is a JSON-RPC batch, which this server does not support.
    server.send_raw(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#);
    let batch = server.next_json(DEADLINE);
    validate_official(MODERN, &batch);
    assert_eq!(batch["error"]["code"], -32600);
    assert_eq!(
        batch["error"]["message"],
        "Batch requests are not supported"
    );

    // A CRLF delimiter is accepted and trailing CR is not part of the message.
    let mut crlf = serde_json::to_vec(&modern_discover(9)).expect("serialize");
    crlf.extend_from_slice(b"\r\n");
    server.send_bytes(&crlf);
    let crlf_response = server.next_json(DEADLINE);
    validate_official(MODERN, &crlf_response);
    assert_eq!(crlf_response["id"], 9);
    assert_eq!(
        crlf_response["result"]["supportedVersions"],
        json!([MODERN])
    );

    // The process is still healthy.
    let healthy = round_trip(&mut server, &modern_discover(10), MODERN);
    assert_eq!(healthy["id"], 10);

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(
        stderr, "",
        "diagnostics must stay silent on protocol errors"
    );
}

#[test]
fn invalid_utf8_is_a_bounded_parse_error() {
    let mut server = ServerProcess::spawn();

    // A raw 0xFF byte inside a JSON string literal is not valid UTF-8.
    let mut frame =
        br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cribra_scan","arguments":{"source":""#
            .to_vec();
    frame.push(0xFF);
    frame.extend_from_slice(br#""}}}"#);
    server.send_frame(&frame);

    let response = server.next_json(DEADLINE);
    validate_official(MODERN, &response);
    assert_eq!(response["error"]["code"], -32700);
    assert!(response.get("id").is_none());

    // Invalid UTF-8 must not desynchronise subsequent framing.
    let after = round_trip(&mut server, &modern_discover(2), MODERN);
    assert_eq!(after["id"], 2);

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn truncated_final_message_is_dropped_at_eof_without_partial_output() {
    let mut server = ServerProcess::spawn();
    server.send_bytes(br#"{"jsonrpc":"2.0","id":1,"method":"tools/lis"#);
    server.close_stdin();

    // EOF without a delimiter may discard the unfinished frame. It must never
    // emit a partial JSON line.
    if let Some(line) = server.next_line(Duration::from_millis(500)) {
        let value: Value = serde_json::from_str(&line).expect("any output line is complete JSON");
        validate_official(MODERN, &value);
        assert_eq!(value["error"]["code"], -32700);
    }
    let status = server.wait(DEADLINE).expect("server exits");
    assert_eq!(status.code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn deeply_nested_json_is_rejected_without_crashing() {
    let mut server = ServerProcess::spawn();

    // 400 nested arrays stay well inside the 2 MiB message limit but exceed the
    // parser's recursion budget. The server must answer, not abort.
    let depth = 400;
    let nested = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let frame = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"cribra_scan","arguments":{{"source":{nested}}}}}}}"#
    );
    assert!(frame.len() < MAX_MESSAGE_BYTES);
    server.send_raw(&frame);

    let response = server.next_json(DEADLINE);
    validate_official(MODERN, &response);
    assert_eq!(
        response["error"]["code"], -32700,
        "a recursion-limited parse must surface as a parse error"
    );

    let after = round_trip(&mut server, &modern_discover(2), MODERN);
    assert_eq!(after["id"], 2);
    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn maximum_length_ids_are_accepted_and_longer_ids_rejected() {
    let mut server = ServerProcess::spawn();

    let max_id = "i".repeat(MAX_REQUEST_ID_BYTES);
    let accepted = round_trip(&mut server, &modern_discover(max_id.clone()), MODERN);
    assert_eq!(
        accepted["id"].as_str(),
        Some(max_id.as_str()),
        "a maximum-length id must round-trip byte-exactly"
    );

    let too_long = "i".repeat(MAX_REQUEST_ID_BYTES + 1);
    server.send(&modern_discover(too_long));
    let rejected = server.next_json(DEADLINE);
    validate_official(MODERN, &rejected);
    assert_eq!(rejected["error"]["code"], -32600);
    assert!(
        rejected.get("id").is_none(),
        "an unreadable id must be omitted rather than echoed"
    );

    // Numeric ids round-trip as numbers, not strings.
    let numeric = round_trip(&mut server, &modern_discover(7), MODERN);
    assert_eq!(numeric["id"], json!(7));

    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

#[test]
fn oversized_request_is_rejected_without_echoing_its_content() {
    const CANARY: &str = "CRIBRA_OVERSIZED_CANARY";
    let mut server = ServerProcess::spawn();

    // One byte over the transport limit; the decoder must discard it up to the
    // newline instead of buffering it.
    let mut frame = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"cribra_scan","arguments":{{"source":"{CANARY}"#
    );
    frame.push_str(&"a".repeat(MAX_MESSAGE_BYTES));
    frame.push_str(r#""}}}}}"#);
    assert!(frame.len() > MAX_MESSAGE_BYTES);
    server.send_raw(&frame);

    let response = server.next_json(DEADLINE);
    validate_official(MODERN, &response);
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(
        response["error"]["message"],
        "Request exceeds maximum message size"
    );
    assert!(!response.to_string().contains(CANARY));

    let after = round_trip(&mut server, &modern_discover(2), MODERN);
    assert_eq!(after["id"], 2);
    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

#[test]
fn duplicate_live_ids_are_rejected_and_reusable_after_completion() {
    let mut server = ServerProcess::spawn();

    let first = scan_call(5, "alpha");
    let duplicate = scan_call(5, "beta");
    server.send(&first);
    server.send(&duplicate);

    let a = server.next_json(DEADLINE);
    let b = server.next_json(DEADLINE);
    validate_official(MODERN, &a);
    validate_official(MODERN, &b);

    let mut successes = 0;
    let mut duplicates = 0;
    for response in [&a, &b] {
        assert_eq!(response["id"], 5);
        match response.get("error") {
            Some(error) => {
                assert_eq!(
                    error["code"], -32600,
                    "duplicate must be an invalid request"
                );
                assert_eq!(error["message"], "Invalid request");
                duplicates += 1;
            }
            None => {
                assert_eq!(response["result"]["isError"], false);
                successes += 1;
            }
        }
    }
    assert_eq!(
        (successes, duplicates),
        (1, 1),
        "exactly one execution and one duplicate rejection"
    );

    // Barrier: a tools/list response can only be written after the writer has
    // completed the earlier tracked identifiers, so the id is free again.
    let barrier = round_trip(&mut server, &modern(99, "tools/list", json!({})), MODERN);
    assert_eq!(barrier["id"], 99);

    let reused = round_trip(&mut server, &scan_call(5, "gamma"), MODERN);
    assert_eq!(reused["id"], 5);
    assert_eq!(
        reused["result"]["isError"], false,
        "an id must be reusable once its response completed"
    );

    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

#[test]
fn burst_at_queue_capacity_answers_every_request_exactly_once() {
    let mut server = ServerProcess::spawn();

    // Eight is the documented request-queue capacity. Each request must receive
    // exactly one response, whether it ran or was rejected as overloaded.
    for id in 1..=8 {
        server.send(&scan_call(id, "burst"));
    }

    let mut seen = BTreeSet::new();
    for _ in 0..8 {
        let response = server.next_json(DEADLINE);
        validate_official(MODERN, &response);
        let id = response["id"].as_i64().expect("numeric id");
        assert!(seen.insert(id), "id {id} answered twice");
        if let Some(error) = response.get("error") {
            assert_eq!(error["code"], -32603);
            assert_eq!(error["message"], "Server overloaded");
        }
    }
    assert_eq!(seen, (1..=8).collect::<BTreeSet<_>>());

    // The connection is still usable immediately afterwards.
    let after = round_trip(&mut server, &modern_discover(50), MODERN);
    assert_eq!(after["id"], 50);

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn unicode_and_escaped_ids_round_trip_exactly() {
    let mut server = ServerProcess::spawn();

    let id = "id-\u{1F600}-\u{00E9}-\"quote\"-\\slash-\nnewline";
    let response = round_trip(&mut server, &modern_discover(id), MODERN);
    assert_eq!(
        response["id"].as_str(),
        Some(id),
        "escaped characters must survive the round trip"
    );

    // Multibyte source text must be counted in UTF-8 bytes and echoed only as
    // privacy-safe finding coordinates.
    const MATCHED: &str = "MATCHED_VALUE_9f2c";
    let source = format!("\u{03B1}\u{03B2} {MATCHED} \u{4F60}\u{597D} \u{1F510}");
    let scan = modern(
        2,
        "tools/call",
        json!({
            "name": "cribra_scan",
            "arguments": {
                "source": source,
                "rules": [{"id": "custom.probe", "pattern": MATCHED, "severity": "high"}],
            },
        }),
    );
    let scan = round_trip(&mut server, &scan, MODERN);
    assert_eq!(scan["result"]["isError"], false);
    let findings = scan["result"]["structuredContent"]["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0]["start"],
        "\u{03B1}\u{03B2} ".len(),
        "offsets are UTF-8 byte offsets"
    );
    assert!(
        !scan.to_string().contains(MATCHED),
        "the matched substring must never be echoed"
    );

    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

#[test]
fn both_eras_answer_in_one_process() {
    let mut server = ServerProcess::spawn();

    // Modern discovery first must not commit the process to modern-only.
    let discover = round_trip(&mut server, &modern_discover(1), MODERN);
    assert_eq!(discover["result"]["supportedVersions"], json!([MODERN]));

    let init = round_trip(&mut server, &legacy_initialize(2), LEGACY);
    assert_eq!(init["result"]["protocolVersion"], LEGACY);
    assert!(init["result"].get("resultType").is_none());

    server.send(&initialized());

    let legacy_list = round_trip(&mut server, &legacy(3, "tools/list", json!({})), LEGACY);
    assert!(legacy_list["result"].get("ttlMs").is_none());
    assert!(legacy_list["result"].get("resultType").is_none());

    let modern_list = round_trip(&mut server, &modern(4, "tools/list", json!({})), MODERN);
    assert_eq!(modern_list["result"]["ttlMs"], 0);
    assert_eq!(modern_list["result"]["resultType"], "complete");

    let legacy_call = round_trip(
        &mut server,
        &legacy(
            5,
            "tools/call",
            json!({"name": "cribra_transform", "arguments": {"source": "hello", "mode": "redact"}}),
        ),
        LEGACY,
    );
    assert_eq!(legacy_call["result"]["isError"], false);
    assert!(legacy_call["result"].get("resultType").is_none());

    let modern_call = round_trip(&mut server, &scan_call(6, "hello"), MODERN);
    assert_eq!(modern_call["result"]["resultType"], "complete");

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn notifications_never_produce_output() {
    let mut server = ServerProcess::spawn();

    for notification in [
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 999}}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": "unknown"}}),
        json!({"jsonrpc": "2.0", "method": "unknown/notification"}),
    ] {
        server.send(&notification);
    }

    // A synchronous round-trip proves all five notifications were consumed in
    // order with no output in between.
    let response = round_trip(&mut server, &modern_discover(1), MODERN);
    assert_eq!(response["id"], 1);

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn repeated_start_execute_shutdown_is_clean() {
    for iteration in 0..5 {
        let mut server = ServerProcess::spawn();
        let discover = round_trip(&mut server, &modern_discover(1), MODERN);
        assert_eq!(discover["result"]["supportedVersions"], json!([MODERN]));
        let scan = round_trip(&mut server, &scan_call(2, "repeat"), MODERN);
        assert_eq!(scan["result"]["isError"], false);
        let (code, stderr) = shutdown(server);
        assert_eq!(code, 0, "iteration {iteration} exited uncleanly");
        assert_eq!(stderr, "", "iteration {iteration} wrote diagnostics");
    }
}

/// The advertised source limit is exactly 1 MiB. The boundary must be
/// inclusive at the limit and rejected one byte above it.
#[test]
fn source_size_boundary_is_exact_at_one_mebibyte() {
    assert_eq!(MAX_SOURCE_BYTES, 1024 * 1024);
    let mut server = ServerProcess::spawn();

    let exact = "a".repeat(MAX_SOURCE_BYTES);
    let accepted = round_trip(&mut server, &scan_call(1, &exact), MODERN);
    assert_eq!(
        accepted["result"]["isError"], false,
        "a source of exactly {MAX_SOURCE_BYTES} bytes must be accepted"
    );
    assert_eq!(
        accepted["result"]["structuredContent"]["summary"]["scannedBytes"],
        json!(MAX_SOURCE_BYTES)
    );

    let over = "a".repeat(MAX_SOURCE_BYTES + 1);
    let rejected = round_trip(&mut server, &scan_call(2, &over), MODERN);
    assert_eq!(rejected["result"]["isError"], true);
    assert_eq!(
        rejected["result"]["structuredContent"]["error"]["code"],
        "invalid_arguments"
    );

    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

/// Regression: Core counts structural candidates before suppression, so a
/// broad finding that covers every candidate must not produce a spurious
/// `result_limit_exceeded`.
#[test]
fn suppressed_structural_candidates_still_scan_over_the_wire() {
    let token = "ABCD-EFGH-IJKL-MNOP";
    let source = vec![token; 9000].join(" ");
    let mut server = ServerProcess::spawn();

    let call = modern(
        1,
        "tools/call",
        json!({
            "name": "cribra_scan",
            "arguments": {
                "source": source,
                "rules": [{"id": "custom.all", "pattern": ".+", "severity": "low"}],
            },
        }),
    );
    let response = round_trip(&mut server, &call, MODERN);
    let result = &response["result"];
    assert_eq!(
        result["isError"], false,
        "must not report result_limit_exceeded"
    );
    let findings = result["structuredContent"]["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(
        result["structuredContent"]["candidates"],
        json!([]),
        "suppressed candidates must not be reported"
    );

    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

/// The only ordering guarantee the architecture makes is per-request
/// round-trip ordering: a synchronous request is answered before the next one
/// is read. Concurrent requests are explicitly not ordered, so nothing here
/// asserts an interleaving the server never promised.
#[test]
fn sequential_round_trips_are_ordered_and_concurrency_is_not_assumed() {
    let mut server = ServerProcess::spawn();
    for id in 1..=12 {
        let response = round_trip(&mut server, &modern_discover(id), MODERN);
        assert_eq!(response["id"], id, "a synchronous round trip is ordered");
    }
    let (code, _) = shutdown(server);
    assert_eq!(code, 0);
}

/// Modern version negotiation over the real binary: a per-request `_meta`
/// naming an unsupported revision is rejected with `-32022` and the supported
/// list, and a modern-era request without `_meta` is a fixed `-32602`. Neither
/// error carries request content.
#[test]
fn modern_version_negotiation_is_exact_over_the_wire() {
    let mut server = ServerProcess::spawn();

    let mut unsupported_request = modern(1, "server/discover", json!({}));
    unsupported_request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] =
        json!("2025-11-25");
    let unsupported = round_trip(&mut server, &unsupported_request, MODERN);
    assert_eq!(unsupported["error"]["code"], -32022);
    assert_eq!(
        unsupported["error"]["data"]["supported"],
        json!([MODERN]),
        "the error must name the supported revision"
    );
    assert_eq!(unsupported["error"]["data"]["requested"], "2025-11-25");

    let missing = round_trip(
        &mut server,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        MODERN,
    );
    assert_eq!(missing["error"]["code"], -32602);
    assert_eq!(missing["error"]["message"], "Missing required _meta fields");

    // The process remains healthy and accepts a valid modern request.
    let healthy = round_trip(&mut server, &modern_discover(3), MODERN);
    assert_eq!(healthy["result"]["supportedVersions"], json!([MODERN]));

    let (code, stderr) = shutdown(server);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}
