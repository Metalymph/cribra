//! Process-level transport terminal behavior and race-adjacent guarantees.

mod support;

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use cribra_mcp::limits::MAX_RESPONSE_BYTES;
use serde_json::json;
use support::{ChildGuard, StdoutChunks, normalize_stdout_line, read_prefix_and_close};

/// Bounds the chunks buffered ahead of the consumer in `StdoutChunks`.
///
/// A small queue keeps the reader slightly behind the writer, so a large
/// response still fills the pipe and exercises partial writes.
const STDOUT_QUEUE_CHUNKS: usize = 2;

/// Hang detector for work bounded by Core CPU time. Shared CI runners can be an
/// order of magnitude slower than a developer machine.
const CORE_WORK_TIMEOUT: Duration = Duration::from_secs(60);

/// Hang detector for a single bounded stdout read.
///
/// The tests that widen `CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS` must not inherit
/// that widened deadline as their progress bound: the read itself has to fail
/// in bounded time even if the server stays alive without writing. 120s is
/// roughly three times the slowest locally measured run under 5.8x CPU
/// oversubscription.
const STREAM_TIMEOUT: Duration = Duration::from_secs(120);

/// Extracts the panic message so a test can assert on the specific failure.
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_owned();
    }
    String::new()
}

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
    let mut child = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
            .env("CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS", "50")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );

    // Keep the read end open but never read it. Each tools/list response is
    // about 8 KiB, so the output eventually stalls. The terminal condition is
    // reported either by the write deadline or by the bounded writer queue,
    // whichever the writer thread observes first; both are backpressure and
    // both must terminate with the same explicit code and a diagnostic.
    let _unread_stdout = child.child_mut().stdout.take().expect("stdout pipe");
    let mut stdin = child.child_mut().stdin.take().expect("stdin pipe");
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
        .expect("server terminated");
    assert_eq!(
        status.code(),
        Some(3),
        "expected explicit terminal exit code"
    );

    let mut stderr = String::new();
    child
        .child_mut()
        .stderr
        .take()
        .expect("stderr pipe")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    // The diagnostic must be written before the terminal code becomes visible,
    // so a process exit can never race the message away. It must name the
    // output-backpressure condition without pinning which detector fired.
    assert!(
        stderr.contains("output backpressure"),
        "unexpected terminal diagnostic: {stderr}"
    );
}

#[test]
fn closed_stdout_is_reported_as_broken_pipe_and_shuts_down_cleanly() {
    let mut child = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );

    // Closing the only read end makes the next stdout write return EPIPE. The
    // server must ignore SIGPIPE, exit cleanly, and not hang waiting for stdin.
    drop(child.child_mut().stdout.take());

    let mut stdin = child.child_mut().stdin.take().expect("stdin pipe");
    stdin
        .write_all(&modern_tools_list(1))
        .expect("write request");
    stdin.flush().expect("flush request");
    drop(stdin);

    let status = child
        .wait_timeout(Duration::from_secs(5))
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

    let status = server.wait(CORE_WORK_TIMEOUT).expect("exit");
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

    let status = server.wait(CORE_WORK_TIMEOUT).expect("exit");
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
///
/// The backpressure deadline is widened for this test to keep a starved runner
/// from being classified as a dead reader; framing is the only assertion. The
/// chunks are still collected through a deliberately small bounded queue, so
/// the consumer stays behind the writer and the pipe still fills. The chunk
/// reader, the cumulative response deadline and the response-size cap keep the
/// test bounded even if the server never completes the line.
#[test]
fn large_response_survives_partial_pipe_writes() {
    let mut child = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
            .env("CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS", "600000")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );

    let mut stdout = StdoutChunks::spawn(
        child.child_mut().stdout.take().expect("stdout pipe"),
        1024,
        STDOUT_QUEUE_CHUNKS,
        STREAM_TIMEOUT,
        MAX_RESPONSE_BYTES,
    );
    let mut stdin = child.child_mut().stdin.take().expect("stdin pipe");
    stdin
        .write_all(&large_transform_request(1))
        .expect("write request");
    stdin.flush().expect("flush request");

    // Deliberately read in small chunks so the writer performs many partial
    // writes and observes WouldBlock between them.
    let mut collected = Vec::new();
    let line_end = loop {
        let chunk = stdout.next(STREAM_TIMEOUT);
        assert_ne!(
            chunk.len(),
            0,
            "stdout closed before the response was complete"
        );
        collected.extend_from_slice(&chunk);
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
        .wait_timeout(CORE_WORK_TIMEOUT)
        .expect("server terminated");
    assert!(status.success());
}

/// A widened production deadline must never become the harness progress bound.
///
/// Both bounded readers must report their own timeout specifically, while the
/// write end stays open and silent, which is the shape of a server that hangs
/// instead of answering.
#[test]
fn bounded_stdout_reads_fail_within_their_own_deadline() {
    let probe = Duration::from_millis(50);

    let (silent, still_open) = std::io::pipe().expect("create silent pipe");
    let mut chunks = StdoutChunks::spawn(
        silent,
        64,
        STDOUT_QUEUE_CHUNKS,
        Duration::from_secs(60),
        MAX_RESPONSE_BYTES,
    );
    let chunked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        chunks.next(probe);
    }));
    let message = panic_text(chunked.expect_err("a silent pipe must trip the chunk read deadline"));
    assert!(
        message.contains("stdout read stalled: no data within"),
        "unexpected chunk read failure: {message}"
    );

    let (silent, still_open_prefix) = std::io::pipe().expect("create silent pipe");
    let prefix = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        read_prefix_and_close(silent, 512, probe);
    }));
    let message = panic_text(prefix.expect_err("a silent pipe must trip the prefix read deadline"));
    assert!(
        message.contains("stdout prefix read stalled: no data within"),
        "unexpected prefix read failure: {message}"
    );

    // Keep both write ends alive until after the assertions above so the
    // readers could not have observed EOF instead of a timeout.
    drop(still_open);
    drop(still_open_prefix);
}

/// Continuous output must not extend the response deadline indefinitely.
#[test]
fn continuous_output_cannot_bypass_the_cumulative_response_deadline() {
    use std::io::Write as _;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let (reader, mut writer) = std::io::pipe().expect("create producing pipe");
    let stop = Arc::new(AtomicBool::new(false));
    let stop_producer = Arc::clone(&stop);
    let producer = std::thread::spawn(move || {
        while !stop_producer.load(Ordering::Relaxed) {
            if writer.write_all(b"x").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    let mut chunks = StdoutChunks::spawn(
        reader,
        64,
        STDOUT_QUEUE_CHUNKS,
        Duration::from_millis(150),
        MAX_RESPONSE_BYTES,
    );
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            let chunk = chunks.next(Duration::from_secs(5));
            assert!(!chunk.is_empty(), "producer must stay alive for this test");
        }
    }));

    stop.store(true, Ordering::Relaxed);
    producer.join().expect("producer thread exits");

    let message =
        panic_text(outcome.expect_err("an incomplete response must trip the cumulative deadline"));
    assert!(
        message.contains("cumulative response deadline"),
        "unexpected panic: {message}"
    );
}

/// Dropping the guard must terminate and reap the child, closing its pipes.
#[test]
fn process_guard_terminates_the_child_on_drop() {
    let mut guard = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );
    let stdout = guard.child_mut().stdout.take().expect("stdout pipe");
    let mut chunks = StdoutChunks::spawn(
        stdout,
        64,
        STDOUT_QUEUE_CHUNKS,
        Duration::from_secs(10),
        MAX_RESPONSE_BYTES,
    );

    drop(guard);

    let chunk = chunks.next(Duration::from_secs(5));
    assert!(
        chunk.is_empty(),
        "dropping the guard must terminate the child and close stdout"
    );
}

/// The line reader must keep the exact `BufRead::lines` CR/LF semantics.
#[test]
fn stdout_line_normalization_matches_bufread_lines() {
    assert_eq!(normalize_stdout_line("value\n"), "value");
    assert_eq!(normalize_stdout_line("value\r\n"), "value");
    assert_eq!(normalize_stdout_line("value\r"), "value\r");
    assert_eq!(normalize_stdout_line("value"), "value");
    assert_eq!(normalize_stdout_line("\n"), "");
    assert_eq!(normalize_stdout_line("\r\n"), "");
    assert_eq!(normalize_stdout_line(""), "");
}

/// If the client disappears in the middle of a large response, the server must
/// treat it as a broken pipe and terminate cleanly instead of hanging on a
/// half-written line.
///
/// The production no-progress deadline is widened for this test only: the
/// assertion is broken-pipe classification. On a starved runner the test thread
/// can be descheduled for longer than the default deadline between the first
/// partial read and closing the pipe, which would let the server report
/// backpressure before it ever observes the disconnect. Sustained backpressure
/// is asserted separately with an explicit short deadline, and both the prefix
/// read and the wait below are bounded independently of that deadline.
#[test]
fn closed_stdout_mid_response_terminates_cleanly() {
    let mut child = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_cribra-mcp"))
            .env("CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS", "600000")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );

    let mut stdin = child.child_mut().stdin.take().expect("stdin pipe");
    stdin
        .write_all(&large_transform_request(1))
        .expect("write request");
    stdin.flush().expect("flush request");

    // Consume only the start of the response, then close the read end.
    let prefix = read_prefix_and_close(
        child.child_mut().stdout.take().expect("stdout pipe"),
        512,
        STREAM_TIMEOUT,
    );
    assert!(!prefix.is_empty(), "expected at least part of the response");

    let status = child
        .wait_timeout(CORE_WORK_TIMEOUT)
        .expect("server terminated");
    assert!(
        status.success(),
        "a mid-response disconnect must shut down cleanly, got {status}"
    );

    let mut stderr = String::new();
    child
        .child_mut()
        .stderr
        .take()
        .expect("stderr pipe")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert_eq!(stderr, "");
}
