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
