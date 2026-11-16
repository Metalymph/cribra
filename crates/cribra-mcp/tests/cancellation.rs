//! Cancellation qualification against the real binary.
//!
//! Cancellation phases are established with architectural barriers rather than
//! sleeps:
//!
//! * "before execution" is proven by keeping the single worker busy with a gate
//!   call, so any later accepted call is still queued when its cancel is read;
//! * whether a call was still queued or already running when its cancel arrived
//!   is not externally observable, and the observable contract is identical for
//!   both, so one test asserts that contract while the registry unit tests pin
//!   the individual state transitions:
//!   `cancellation_before_execution_skips_work_and_releases_id`,
//!   `cancellation_during_execution_skips_emission` and
//!   `cancellation_after_result_enqueue_skips_emission`. None of those phases is
//!   claimed here beyond what it proves.

mod support;

use std::time::Duration;

use serde_json::{Value, json};
use support::{ServerProcess, validate_official};

const MODERN: &str = "2026-07-28";
const DEADLINE: Duration = Duration::from_secs(30);

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

fn cancel(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": id, "reason": "test"},
    })
}

fn discover(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "server/discover",
        "params": {"_meta": meta()},
    })
}

/// A call whose Core work is slow enough that a cancel written immediately
/// afterwards is normally consumed before the work finishes. The test does not
/// depend on that: it asserts the suppression contract, which is identical for a
/// queued and a running call.
fn slow_call(id: u64) -> Value {
    let source = "a".repeat(1024 * 1024);
    call(
        id,
        "cribra_scan",
        json!({
            "source": source,
            "rules": [{"id": "custom.slow", "pattern": "(?s).", "severity": "low"}],
        }),
    )
}

#[test]
fn cancellation_while_queued_suppresses_the_response() {
    let mut server = ServerProcess::spawn();

    // The gate call occupies the only worker. Everything sent after it is
    // necessarily still queued when the following cancel is read.
    server.send(&slow_call(1));
    server.send(&call(2, "cribra_scan", json!({"source": "queued"})));
    server.send(&cancel(2));

    // The gate call itself is not cancelled and must complete.
    let gate = server.next_json(DEADLINE);
    validate_official(MODERN, &gate);
    assert_eq!(gate["id"], 1);

    // The identifier is free again and the connection is healthy. This
    // response is a barrier: the writer emits strictly in completion order, so
    // a leaked response for the cancelled call would arrive first and fail the
    // id assertion below.
    server.send(&call(3, "cribra_scan", json!({"source": "reuse"})));
    let reused = server.next_json(DEADLINE);
    validate_official(MODERN, &reused);
    assert_eq!(
        reused["id"], 3,
        "a cancelled queued call must not be answered"
    );
    assert_eq!(reused["result"]["isError"], false);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

/// Cancels a call that has already been accepted by the reader, immediately
/// after it is written. Depending on scheduling the cancel may land while the
/// call is queued or while Core is executing it; both are suppression cases and
/// the observable contract is identical. The registry unit tests
/// `cancellation_before_execution_skips_work_and_releases_id` and
/// `cancellation_during_execution_skips_emission` pin the two state
/// transitions that this test can only observe from the outside.
#[test]
fn cancellation_of_a_started_or_queued_call_suppresses_the_response() {
    let mut server = ServerProcess::spawn();

    server.send(&slow_call(1));
    server.send(&cancel(1));

    // Barrier: the next tool call is only written after the cancelled call's
    // emission turn has passed, so a suppression failure would surface here as
    // an unexpected id 1 response.
    server.send(&call(2, "cribra_scan", json!({"source": "after cancel"})));
    let after = server.next_json(DEADLINE);
    validate_official(MODERN, &after);
    assert_eq!(
        after["id"], 2,
        "the cancelled call must not emit a response, whether queued or running"
    );
    assert_eq!(after["result"]["isError"], false);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn cancellation_after_completion_is_harmless_and_the_id_is_reusable() {
    let mut server = ServerProcess::spawn();

    server.send(&call(1, "cribra_scan", json!({"source": "done"})));
    let first = server.next_json(DEADLINE);
    validate_official(MODERN, &first);
    assert_eq!(first["id"], 1);

    // Stale cancellation for an identifier the server already answered.
    server.send(&cancel(1));

    // The identifier can be reused; a stale cancel must not poison it. This
    // response also proves the stale cancel produced no output of its own.
    server.send(&call(1, "cribra_scan", json!({"source": "again"})));
    let reused = server.next_json(DEADLINE);
    validate_official(MODERN, &reused);
    assert_eq!(reused["id"], 1);
    assert_eq!(reused["result"]["isError"], false);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

#[test]
fn unknown_and_malformed_cancellations_are_ignored() {
    let mut server = ServerProcess::spawn();

    for notification in [
        cancel(999),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": null}}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": {"nested": true}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": []}),
    ] {
        server.send(&notification);
    }

    // A synchronous round trip proves every malformed cancellation was consumed
    // without a reply and without destabilising the session.
    server.send(&discover(1));
    let response = server.next_json(DEADLINE);
    validate_official(MODERN, &response);
    assert_eq!(response["id"], 1);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}

/// Cancellation must also suppress a result that is already waiting behind a
/// blocked writer. The exact phase (queued versus running) is not externally
/// observable, so the invariant asserted is the contract: a cancelled request
/// never reaches stdout.
#[test]
fn cancellation_suppresses_results_queued_behind_a_blocked_writer() {
    let mut server =
        ServerProcess::spawn_with_env(&[("CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS", "600000")]);

    // A ~1 MiB transformed output cannot fit in the pipe buffer, so the writer
    // blocks immediately and everything behind it stays queued.
    let big_source = "hello world\n".repeat(80_000);
    server.send(&call(
        1,
        "cribra_transform",
        json!({"source": big_source, "mode": "redact"}),
    ));
    server.send(&call(
        2,
        "cribra_scan",
        json!({"source": "behind the writer"}),
    ));
    server.send(&cancel(2));

    // Drain until the big response arrives; the writer is then unblocked and
    // the cancelled response would have been emitted if it were still eligible.
    let first = server.next_json(Duration::from_secs(60));
    validate_official(MODERN, &first);
    assert_eq!(first["id"], 1);
    assert_eq!(first["result"]["isError"], false);

    // The writer is now unblocked. A fresh request is the barrier: a cancelled
    // response still sitting in the writer queue would be emitted before it.
    server.send(&call(3, "cribra_scan", json!({"source": "barrier"})));
    let barrier = server.next_json(DEADLINE);
    validate_official(MODERN, &barrier);
    assert_eq!(
        barrier["id"], 3,
        "a cancelled response must not be written even after backpressure clears"
    );
    assert_eq!(barrier["result"]["isError"], false);

    server.close_stdin();
    assert_eq!(server.wait(DEADLINE).expect("exit").code(), Some(0));
    assert_eq!(server.stderr_text(), "");
}
