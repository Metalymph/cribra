//! Process orchestration for the stdio MCP server.
//!
//! Three roles are isolated:
//!
//! * the reader owns stdin, lifecycle classification, cancellation, and the
//!   bounded worker queue;
//! * the worker executes the M1 tool stubs and submits responses;
//! * the writer owns stdout and performs the final cancellation claim.
//!
//! The reader never waits on the writer. On sustained output backpressure the
//! writer returns an explicit terminal exit code; it never leaves the reader
//! blocked while pretending the connection is healthy.

use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::error::{INTERNAL_ERROR, INVALID_REQUEST, MESSAGE_TOO_LARGE, OVERLOADED};
use crate::jsonrpc::{ParsedMessage, RequestId, parse_message};
use crate::limits::{
    BACKPRESSURE_TIMEOUT_ENV, CANCELLATION_REGISTRY_CAPACITY, DEFAULT_BACKPRESSURE_TIMEOUT_MS,
    EXIT_BACKPRESSURE, EXIT_INTERNAL, MAX_MESSAGE_BYTES, MAX_RESPONSE_BYTES,
    REQUEST_QUEUE_CAPACITY, WRITER_QUEUE_CAPACITY,
};
use crate::protocol::{Dispatch, Era, Reply, ReplyOutcome, Session, ToolCall, shape_success};
use crate::registry::{AcceptError, EmitDecision, Registry, StartDecision};
use crate::tooling;
use crate::transport::{
    LineDecoder, LineEvent, WriteOutcome, ignore_sigpipe, set_nonblocking, write_all_nonblocking,
};

struct Shared {
    terminal: AtomicI32,
    input_closed: AtomicBool,
    registry: Mutex<Registry>,
}

struct Outbound {
    bytes: Vec<u8>,
    tracked: Option<RequestId>,
}

enum WriterExit {
    Completed(i32),
    InputClosed,
}

impl Shared {
    fn new() -> Self {
        Self {
            terminal: AtomicI32::new(0),
            input_closed: AtomicBool::new(false),
            registry: Mutex::new(Registry::new(CANCELLATION_REGISTRY_CAPACITY)),
        }
    }

    fn input_is_closed(&self) -> bool {
        self.input_closed.load(Ordering::Acquire)
    }

    fn close_input(&self) {
        self.input_closed.store(true, Ordering::Release);
    }
}

/// Runs the stdio MCP server until stdin closes or the output connection
/// reaches an explicit terminal condition.
pub fn run() -> i32 {
    ignore_sigpipe();
    let backpressure_timeout = backpressure_timeout();

    let (request_tx, request_rx) = mpsc::sync_channel::<ToolCall>(REQUEST_QUEUE_CAPACITY);
    let (writer_tx, writer_rx) = mpsc::sync_channel::<Outbound>(WRITER_QUEUE_CAPACITY);
    let shared = Arc::new(Shared::new());

    let reader_writer_tx = writer_tx.clone();
    let reader_shared = Arc::clone(&shared);
    let reader = match thread::Builder::new()
        .name("cribra-mcp-reader".to_owned())
        .spawn(move || reader_loop(reader_writer_tx, request_tx, reader_shared))
    {
        Ok(reader) => reader,
        Err(_) => {
            eprintln!("cribra-mcp: reader startup failed");
            return EXIT_INTERNAL;
        }
    };

    let worker_writer_tx = writer_tx.clone();
    let worker_shared = Arc::clone(&shared);
    let worker = match thread::Builder::new()
        .name("cribra-mcp-worker".to_owned())
        .spawn(move || worker_loop(request_rx, worker_writer_tx, worker_shared))
    {
        Ok(worker) => worker,
        Err(_) => {
            eprintln!("cribra-mcp: worker startup failed");
            return EXIT_INTERNAL;
        }
    };

    // The main thread keeps only the receiver. On normal input closure the
    // writer discards queued responses and the join below lets at most the
    // already-running tool call finish before the process exits.
    drop(writer_tx);
    match writer_loop(writer_rx, Arc::clone(&shared), backpressure_timeout) {
        WriterExit::Completed(code) => code,
        WriterExit::InputClosed => {
            let _ = reader.join();
            let _ = worker.join();
            shared.terminal.load(Ordering::Acquire)
        }
    }
}

fn backpressure_timeout() -> Duration {
    let milliseconds = std::env::var(BACKPRESSURE_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map_or(DEFAULT_BACKPRESSURE_TIMEOUT_MS, |value| {
            value.clamp(10, 600_000)
        });
    Duration::from_millis(milliseconds)
}

fn reader_loop(
    writer_tx: SyncSender<Outbound>,
    request_tx: SyncSender<ToolCall>,
    shared: Arc<Shared>,
) {
    let stdin = io::stdin();
    let mut decoder = LineDecoder::new(stdin.lock(), MAX_MESSAGE_BYTES);
    let mut session = Session::new();

    loop {
        if shared.terminal.load(Ordering::Acquire) != 0 {
            break;
        }
        match decoder.next_event() {
            Ok(Some(LineEvent::TooLong)) => {
                let reply = Reply::error(None, MESSAGE_TOO_LARGE);
                if !send_untracked(&writer_tx, &shared, reply) {
                    break;
                }
            }
            Ok(Some(LineEvent::Message(line))) => {
                if !handle_line(&line, &mut session, &writer_tx, &request_tx, &shared) {
                    break;
                }
            }
            Ok(None) => {
                shared.close_input();
                break;
            }
            Err(_) => {
                mark_terminal(&shared, EXIT_INTERNAL, "cribra-mcp: stdin read failed");
                break;
            }
        }
    }
}

fn handle_line(
    line: &[u8],
    session: &mut Session,
    writer_tx: &SyncSender<Outbound>,
    request_tx: &SyncSender<ToolCall>,
    shared: &Arc<Shared>,
) -> bool {
    let parsed = parse_message(line);
    match parsed {
        ParsedMessage::Notification { method, params } => {
            if method == "notifications/cancelled" {
                if let Some(id) = cancellation_id(params.as_ref()) {
                    let _ = shared
                        .registry
                        .lock()
                        .map(|mut registry| registry.cancel(&id));
                }
            } else {
                session.handle_notification(&method, params.as_ref());
            }
            true
        }
        ParsedMessage::Invalid { id, error } => {
            send_untracked(writer_tx, shared, Reply::error(id, error))
        }
        ParsedMessage::Request { id, method, params } => {
            let accepted_id = id.clone();
            match shared
                .registry
                .lock()
                .map(|mut registry| registry.accept(id))
            {
                Ok(Ok(())) => {}
                Ok(Err(AcceptError::Duplicate)) => {
                    return send_untracked(
                        writer_tx,
                        shared,
                        Reply::error(Some(accepted_id), INVALID_REQUEST),
                    );
                }
                Ok(Err(AcceptError::Full)) => {
                    return send_untracked(
                        writer_tx,
                        shared,
                        Reply::error(Some(accepted_id), OVERLOADED),
                    );
                }
                Err(_) => {
                    mark_terminal(shared, EXIT_INTERNAL, "cribra-mcp: registry poisoned");
                    return false;
                }
            }

            let dispatch = session.dispatch(ParsedMessage::Request {
                id: accepted_id.clone(),
                method,
                params,
            });
            match dispatch {
                Dispatch::Reply(reply) => send_tracked(writer_tx, shared, reply, accepted_id),
                Dispatch::ToolCall(call) => match request_tx.try_send(call) {
                    Ok(()) => true,
                    Err(TrySendError::Full(_)) => {
                        abandon(shared, &accepted_id);
                        send_untracked(
                            writer_tx,
                            shared,
                            Reply::error(Some(accepted_id), OVERLOADED),
                        )
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        abandon(shared, &accepted_id);
                        false
                    }
                },
                Dispatch::None => {
                    abandon(shared, &accepted_id);
                    true
                }
            }
        }
    }
}

fn cancellation_id(params: Option<&Value>) -> Option<RequestId> {
    let params = params?.as_object()?;
    RequestId::from_value(params.get("requestId")?)
}

fn abandon(shared: &Shared, id: &RequestId) {
    if let Ok(mut registry) = shared.registry.lock() {
        registry.abandon(id);
    }
}

fn discard_outbound(outbound: Outbound, shared: &Shared) {
    if let Some(id) = outbound.tracked.as_ref() {
        abandon(shared, id);
    }
}

fn discard_queued_requests(request_rx: &mpsc::Receiver<ToolCall>, shared: &Shared) {
    while let Ok(call) = request_rx.try_recv() {
        abandon(shared, &call.id);
    }
}

fn discard_queued_output(writer_rx: &mpsc::Receiver<Outbound>, shared: &Shared) {
    while let Ok(outbound) = writer_rx.try_recv() {
        discard_outbound(outbound, shared);
    }
}

fn worker_loop(
    request_rx: mpsc::Receiver<ToolCall>,
    writer_tx: SyncSender<Outbound>,
    shared: Arc<Shared>,
) {
    worker_loop_with(request_rx, writer_tx, shared, |_| {
        tooling::stub_tool_result()
    });
}

fn worker_loop_with<F>(
    request_rx: mpsc::Receiver<ToolCall>,
    writer_tx: SyncSender<Outbound>,
    shared: Arc<Shared>,
    handler: F,
) where
    F: Fn(&ToolCall) -> Value,
{
    loop {
        if shared.input_is_closed() {
            break;
        }

        let call = match request_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(call) => call,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        // Input closure stops new work. A call that had not started is
        // discarded and its identifier released immediately.
        if shared.input_is_closed() {
            abandon(&shared, &call.id);
            break;
        }

        let decision = match shared.registry.lock() {
            Ok(mut registry) => registry.start(&call.id),
            Err(_) => {
                mark_terminal(&shared, EXIT_INTERNAL, "cribra-mcp: registry poisoned");
                return;
            }
        };
        if decision == StartDecision::Cancelled {
            continue;
        }

        let reply = catch_unwind(AssertUnwindSafe(|| {
            let mut result = handler(&call);
            shape_success(call.era, &mut result);
            Reply {
                id: Some(call.id.clone()),
                outcome: ReplyOutcome::Success(result),
            }
        }))
        .unwrap_or_else(|_| Reply::error(Some(call.id.clone()), INTERNAL_ERROR));

        // The call we just finished is the only one allowed to complete. If
        // input closed while it ran, discard its result rather than retaining
        // it in the writer queue.
        if shared.input_is_closed() {
            abandon(&shared, &call.id);
            break;
        }

        let outbound = tracked_outbound(reply, call.id);
        if let Err(error) = writer_tx.send(outbound) {
            discard_outbound(error.0, &shared);
            break;
        }
    }

    discard_queued_requests(&request_rx, &shared);
}

fn writer_loop(
    writer_rx: mpsc::Receiver<Outbound>,
    shared: Arc<Shared>,
    backpressure_timeout: Duration,
) -> WriterExit {
    if set_nonblocking(libc::STDOUT_FILENO).is_err() {
        eprintln!("cribra-mcp: stdout configuration failed");
        return WriterExit::Completed(EXIT_INTERNAL);
    }

    loop {
        if shared.input_is_closed() {
            discard_queued_output(&writer_rx, &shared);
            let terminal = shared.terminal.load(Ordering::Acquire);
            return if terminal == 0 {
                WriterExit::InputClosed
            } else {
                WriterExit::Completed(terminal)
            };
        }

        let terminal = shared.terminal.load(Ordering::Acquire);
        if terminal != 0 {
            return WriterExit::Completed(terminal);
        }

        let outbound = match writer_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(outbound) => outbound,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                let terminal = shared.terminal.load(Ordering::Acquire);
                return if terminal == 0 && shared.input_is_closed() {
                    WriterExit::InputClosed
                } else {
                    WriterExit::Completed(terminal)
                };
            }
        };

        // A response that was still queued when stdin closed is discarded.
        // Whatever is already inside write_all_nonblocking finishes as one
        // complete line; no cancellation path deliberately emits a partial
        // JSON-RPC line.
        if shared.input_is_closed() {
            discard_outbound(outbound, &shared);
            discard_queued_output(&writer_rx, &shared);
            return WriterExit::InputClosed;
        }

        let terminal = shared.terminal.load(Ordering::Acquire);
        if terminal != 0 {
            discard_outbound(outbound, &shared);
            return WriterExit::Completed(terminal);
        }

        if let Some(id) = outbound.tracked.as_ref() {
            let decision = match shared.registry.lock() {
                Ok(mut registry) => registry.begin_emit(id),
                Err(_) => {
                    eprintln!("cribra-mcp: registry poisoned");
                    return WriterExit::Completed(EXIT_INTERNAL);
                }
            };
            if decision == EmitDecision::Skip {
                continue;
            }
        }

        match write_all_nonblocking(libc::STDOUT_FILENO, &outbound.bytes, backpressure_timeout) {
            WriteOutcome::Written => {
                if let Some(id) = outbound.tracked.as_ref()
                    && let Ok(mut registry) = shared.registry.lock()
                {
                    registry.complete(id);
                }
            }
            WriteOutcome::BrokenPipe => return WriterExit::Completed(0),
            WriteOutcome::Backpressure => {
                eprintln!("cribra-mcp: stdout backpressure timeout; terminating");
                return WriterExit::Completed(EXIT_BACKPRESSURE);
            }
            WriteOutcome::Error => {
                eprintln!("cribra-mcp: stdout write failed; terminating");
                return WriterExit::Completed(EXIT_INTERNAL);
            }
        }
    }
}

fn send_tracked(
    writer_tx: &SyncSender<Outbound>,
    shared: &Shared,
    reply: Reply,
    id: RequestId,
) -> bool {
    let outbound = tracked_outbound(reply, id);
    match writer_tx.try_send(outbound) {
        Ok(()) => true,
        Err(TrySendError::Full(outbound)) => {
            discard_outbound(outbound, shared);
            mark_terminal(
                shared,
                EXIT_BACKPRESSURE,
                "cribra-mcp: output queue exhausted",
            );
            false
        }
        Err(TrySendError::Disconnected(outbound)) => {
            discard_outbound(outbound, shared);
            false
        }
    }
}

fn send_untracked(writer_tx: &SyncSender<Outbound>, shared: &Shared, reply: Reply) -> bool {
    let outbound = match encode_reply(&reply) {
        Ok(bytes) => Outbound {
            bytes,
            tracked: None,
        },
        Err(()) => {
            mark_terminal(
                shared,
                EXIT_INTERNAL,
                "cribra-mcp: response encoding failed",
            );
            return false;
        }
    };
    match writer_tx.try_send(outbound) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            mark_terminal(
                shared,
                EXIT_BACKPRESSURE,
                "cribra-mcp: output queue exhausted",
            );
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn tracked_outbound(reply: Reply, id: RequestId) -> Outbound {
    let bytes = match encode_reply(&reply) {
        Ok(bytes) => bytes,
        Err(()) => {
            let fallback = Reply::error(Some(id.clone()), INTERNAL_ERROR);
            encode_reply(&fallback).unwrap_or_else(|()| {
                b"{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"Internal error\"}}\n"
                    .to_vec()
            })
        }
    };
    Outbound {
        bytes,
        tracked: Some(id),
    }
}

fn encode_reply(reply: &Reply) -> Result<Vec<u8>, ()> {
    let mut bytes = serde_json::to_vec(&reply.to_value()).map_err(|_| ())?;
    if bytes.len() + 1 > MAX_RESPONSE_BYTES {
        return Err(());
    }
    bytes.push(b'\n');
    Ok(bytes)
}

fn mark_terminal(shared: &Shared, code: i32, message: &'static str) {
    let previous = shared.terminal.swap(code, Ordering::AcqRel);
    if previous == 0 {
        eprintln!("{message}");
    }
}

/// Shapes a tool stub result for tests and future M2 reuse.
#[must_use]
pub fn stub_result(era: Era) -> Value {
    let mut result = tooling::stub_tool_result();
    shape_success(era, &mut result);
    result
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc::{TryRecvError, channel, sync_channel};

    use super::*;

    fn call(id: i64) -> ToolCall {
        ToolCall {
            id: RequestId::Number(id),
            era: Era::Modern,
            name: "cribra_scan".to_owned(),
            arguments: serde_json::json!({"source": "queued"}),
        }
    }

    #[test]
    fn shutdown_finishes_running_call_only_and_discards_full_queue() {
        let (request_tx, request_rx) = sync_channel::<ToolCall>(1);
        let (writer_tx, writer_rx) = sync_channel::<Outbound>(1);
        let shared = Arc::new(Shared::new());

        for id in 1..=3 {
            shared
                .registry
                .lock()
                .expect("registry")
                .accept(RequestId::Number(id))
                .expect("accepted");
        }

        let (started_tx, started_rx) = channel();
        let (release_tx, release_rx) = channel::<()>();
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_shared = Arc::clone(&shared);
        let worker_calls = Arc::clone(&calls);
        let worker = thread::spawn(move || {
            worker_loop_with(request_rx, writer_tx, worker_shared, move |_| {
                worker_calls.fetch_add(1, Ordering::SeqCst);
                started_tx.send(()).expect("signal start");
                release_rx.recv().expect("wait for release");
                tooling::stub_tool_result()
            });
        });

        request_tx.send(call(1)).expect("running call");
        started_rx
            .recv_timeout(Duration::from_secs(3))
            .expect("worker is executing");
        request_tx.send(call(2)).expect("queued call");
        assert!(
            matches!(request_tx.try_send(call(3)), Err(TrySendError::Full(_))),
            "capacity-one request queue must be full"
        );
        // The reader would release an identifier that could not be queued.
        abandon(&shared, &RequestId::Number(3));

        shared.close_input();
        release_tx.send(()).expect("release worker");
        worker.join().expect("worker exits");

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "only the already-running call may execute"
        );
        assert!(
            shared.registry.lock().expect("registry").is_empty(),
            "running and queued identifiers must be released"
        );
        assert!(
            matches!(
                writer_rx.try_recv(),
                Err(TryRecvError::Empty | TryRecvError::Disconnected)
            ),
            "the running result must not be retained after input closure"
        );
        assert!(
            matches!(
                request_tx.try_send(call(4)),
                Err(TrySendError::Disconnected(_))
            ),
            "the worker must drain and drop its queue"
        );
    }

    #[test]
    fn shutdown_discards_queued_responses_and_releases_ids() {
        let (writer_tx, writer_rx) = sync_channel::<Outbound>(1);
        let shared = Arc::new(Shared::new());
        let id = RequestId::Number(7);
        shared
            .registry
            .lock()
            .expect("registry")
            .accept(id.clone())
            .expect("accepted");

        writer_tx
            .send(Outbound {
                bytes: b"{\"jsonrpc\":\"2.0\"}\n".to_vec(),
                tracked: Some(id),
            })
            .expect("queued response");
        shared.close_input();
        discard_queued_output(&writer_rx, &shared);

        assert!(
            shared.registry.lock().expect("registry").is_empty(),
            "queued response identifiers must be released"
        );
        assert!(matches!(writer_rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn discard_queued_requests_releases_every_identifier() {
        let (request_tx, request_rx) = sync_channel::<ToolCall>(2);
        let shared = Arc::new(Shared::new());
        for id in 1..=2 {
            shared
                .registry
                .lock()
                .expect("registry")
                .accept(RequestId::Number(id))
                .expect("accepted");
            request_tx.send(call(id)).expect("queued");
        }

        discard_queued_requests(&request_rx, &shared);

        assert!(shared.registry.lock().expect("registry").is_empty());
        assert!(request_tx.try_send(call(3)).is_ok());
    }
}
