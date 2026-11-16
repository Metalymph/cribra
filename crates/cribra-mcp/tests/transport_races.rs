//! Process-level transport terminal behavior and race-adjacent guarantees.

mod support;

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::json;
use wait_timeout::ChildExt;

fn modern_tools_list(id: u64) -> Vec<u8> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/list",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        }
    });
    let mut line = serde_json::to_vec(&request).expect("request serializes");
    line.push(b'\n');
    line
}

#[test]
fn sustained_stdout_backpressure_is_terminal_with_exit_3() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
        .env("CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS", "50")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");

    // Keep the read end open but never read it. Each tools/list response is
    // about 8 KiB, so more than eight responses exceed the 64 KiB pipe buffer
    // and the writer must enter poll and reach the explicit no-progress
    // deadline instead of waiting for stdin EOF.
    let _unread_stdout = child.stdout.take().expect("stdout pipe");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    for id in 1..=12 {
        stdin
            .write_all(&modern_tools_list(id))
            .expect("write request");
    }
    stdin.flush().expect("flush requests");
    // Closing stdin must not interrupt a blocked stdout write.
    drop(stdin);

    let status = child
        .wait_timeout(Duration::from_secs(5))
        .expect("wait for terminal backpressure")
        .expect("server terminated");
    assert_eq!(
        status.code(),
        Some(3),
        "expected explicit terminal exit code"
    );

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr pipe")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert!(
        stderr.contains("stdout backpressure timeout"),
        "unexpected terminal diagnostic: {stderr}"
    );
}

#[test]
fn closed_stdout_is_reported_as_broken_pipe_and_shuts_down_cleanly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");

    // Closing the only read end makes the next stdout write return EPIPE. The
    // server must ignore SIGPIPE, exit cleanly, and not hang waiting for stdin.
    drop(child.stdout.take());

    let mut stdin = child.stdin.take().expect("stdin pipe");
    stdin
        .write_all(&modern_tools_list(1))
        .expect("write request");
    stdin.flush().expect("flush request");
    drop(stdin);

    let status = child
        .wait_timeout(Duration::from_secs(5))
        .expect("wait for broken pipe shutdown")
        .expect("server terminated");
    assert!(status.success(), "broken pipe should shut down cleanly");
}

#[test]
fn eof_discards_queued_real_tool_work_and_exits_zero() {
    // Real Core scans replace the M1 stubs. Closing stdin while scans are
    // queued or running must still discard work cleanly and never emit a
    // truncated or partially written line.
    use support::ServerProcess;

    let mut server = ServerProcess::spawn();
    let source = "a".repeat(1024 * 1024);
    for id in 1..=3 {
        server.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientCapabilities": {},
                },
                "name": "cribra_scan",
                "arguments": {"source": source.as_str()},
            }
        }));
    }
    server.close_stdin();

    let status = server.wait(Duration::from_secs(10)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");

    let mut responses = 0;
    while let Some(line) = server.next_line(Duration::from_secs(5)) {
        let response: serde_json::Value = serde_json::from_str(&line).expect("stdout line is JSON");
        support::validate_official("2026-07-28", &response);
        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(response["result"]["structuredContent"]["status"], "ok");
        responses += 1;
    }
    assert!(
        responses <= 3,
        "shutdown must not invent responses for discarded work"
    );
}

#[test]
fn cancellation_after_real_scan_is_well_formed_or_silent() {
    // Cancellation is not preemptible while Core is scanning. The result may be
    // emitted just before the cancellation is observed or discarded at the
    // writer boundary; either outcome is valid, but output must stay well formed.
    use support::ServerProcess;

    let mut server = ServerProcess::spawn();
    let source = "a".repeat(1024 * 1024);
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            },
            "name": "cribra_scan",
            "arguments": {"source": source},
        }
    }));
    server.send(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": 1},
    }));
    server.close_stdin();

    let status = server.wait(Duration::from_secs(10)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");

    let mut responses = 0;
    while let Some(line) = server.next_line(Duration::from_secs(5)) {
        let response: serde_json::Value = serde_json::from_str(&line).expect("stdout line is JSON");
        support::validate_official("2026-07-28", &response);
        assert_eq!(response["id"], 1);
        responses += 1;
    }
    assert!(
        responses <= 1,
        "a single call may produce at most one response"
    );
}

#[test]
fn eof_discards_queued_work_and_exits_zero() {
    // Responses that were already written before stdin closed remain valid.
    // Anything still queued is deliberately discarded by the writer, so this
    // test asserts a clean exit and no malformed trailing line rather than a
    // fixed response count.
    use support::ServerProcess;

    let mut server = ServerProcess::spawn();
    for id in 1..=3 {
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
    server.close_stdin();

    let status = server.wait(Duration::from_secs(3)).expect("exit");
    assert!(status.success());
    assert_eq!(server.stderr_text(), "");

    let mut responses = 0;
    while let Some(line) = server.next_line(Duration::from_secs(3)) {
        let response: serde_json::Value = serde_json::from_str(&line).expect("stdout line is JSON");
        support::validate_official("2026-07-28", &response);
        assert_eq!(response["result"]["resultType"], "complete");
        responses += 1;
    }
    assert!(
        responses <= 3,
        "shutdown must not invent responses for discarded work"
    );
}

fn large_transform_request(id: u64) -> Vec<u8> {
    let source = "hello world\n".repeat(20_000);
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            },
            "name": "cribra_transform",
            "arguments": {"source": source, "mode": "redact"},
        }
    });
    let mut line = serde_json::to_vec(&request).expect("request serializes");
    line.push(b'\n');
    line
}

/// A response larger than the pipe buffer is delivered to a slow reader in
/// several partial writes. It must still reassemble into exactly one complete
/// JSON line.
#[test]
fn large_response_survives_partial_pipe_writes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");

    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    stdin
        .write_all(&large_transform_request(1))
        .expect("write request");
    stdin.flush().expect("flush request");

    // Deliberately read in small chunks so the writer performs many partial
    // writes and observes WouldBlock between them.
    let mut collected = Vec::new();
    let mut chunk = [0u8; 1024];
    let line_end = loop {
        let read = stdout.read(&mut chunk).expect("read stdout");
        assert_ne!(read, 0, "stdout closed before the response was complete");
        collected.extend_from_slice(&chunk[..read]);
        if let Some(position) = collected.iter().position(|byte| *byte == b'\n') {
            break position;
        }
    };

    assert!(
        collected[line_end + 1..].is_empty(),
        "the first response must be exactly one line"
    );
    let line = std::str::from_utf8(&collected[..line_end]).expect("utf-8 response");
    let response: serde_json::Value = serde_json::from_str(line).expect("one complete JSON line");
    support::validate_official("2026-07-28", &response);
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["isError"], false);

    let output = response["result"]["structuredContent"]["output"]
        .as_str()
        .expect("transformed output");
    assert_eq!(output, "hello world\n".repeat(20_000));
    assert_eq!(
        response["result"]["content"][0]["text"].as_str(),
        Some(output)
    );
    assert!(
        collected.len() > 64 * 1024,
        "the response must exceed the pipe buffer to exercise partial writes"
    );

    drop(stdin);
    let status = child
        .wait_timeout(Duration::from_secs(10))
        .expect("wait for exit")
        .expect("server terminated");
    assert!(status.success());
}

/// If the client disappears in the middle of a large response, the server must
/// treat it as a broken pipe and terminate cleanly instead of hanging on a
/// half-written line.
#[test]
fn closed_stdout_mid_response_terminates_cleanly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");

    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    stdin
        .write_all(&large_transform_request(1))
        .expect("write request");
    stdin.flush().expect("flush request");

    // Consume only the start of the response, then close the read end.
    let mut prefix = [0u8; 512];
    let read = stdout.read(&mut prefix).expect("read partial response");
    assert!(read > 0, "expected at least part of the response");
    drop(stdout);

    let status = child
        .wait_timeout(Duration::from_secs(10))
        .expect("no hang after client disconnect")
        .expect("server terminated");
    assert!(
        status.success(),
        "a mid-response disconnect must shut down cleanly, got {status}"
    );

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr pipe")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert_eq!(stderr, "");
}
