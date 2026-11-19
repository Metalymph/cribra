//! Shared process harness for JSONL integration tests.

#![allow(dead_code)]

use std::fmt::Write as _;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::LazyLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wait_timeout::ChildExt;

static MODERN_SCHEMA: &str = include_str!("../schemas/2026-07-28.json");
static LEGACY_SCHEMA: &str = include_str!("../schemas/2025-11-25.json");
static SCAN_OUTPUT_SCHEMA: &str = include_str!("../../schemas/cribra_scan.output.json");
static TRANSFORM_OUTPUT_SCHEMA: &str = include_str!("../../schemas/cribra_transform.output.json");

static MODERN_VALIDATOR: LazyLock<jsonschema::Validator> =
    LazyLock::new(|| message_validator(MODERN_SCHEMA).expect("modern schema compiles"));
static LEGACY_VALIDATOR: LazyLock<jsonschema::Validator> =
    LazyLock::new(|| message_validator(LEGACY_SCHEMA).expect("legacy schema compiles"));
static SCAN_OUTPUT_VALIDATOR: LazyLock<jsonschema::Validator> =
    LazyLock::new(|| tool_validator(SCAN_OUTPUT_SCHEMA).expect("scan output schema compiles"));
static TRANSFORM_OUTPUT_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    tool_validator(TRANSFORM_OUTPUT_SCHEMA).expect("transform output schema compiles")
});

fn message_validator(source: &str) -> Result<jsonschema::Validator, String> {
    let mut schema: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    schema
        .as_object_mut()
        .ok_or_else(|| "schema root is not an object".to_owned())?
        .insert("$ref".to_owned(), json!("#/$defs/JSONRPCMessage"));
    jsonschema::validator_for(&schema).map_err(|error| error.to_string())
}

pub fn validate_official(version: &str, value: &Value) {
    let validator = if version == "2026-07-28" {
        &*MODERN_VALIDATOR
    } else {
        &*LEGACY_VALIDATOR
    };
    if validator.is_valid(value) {
        return;
    }
    let mut details = String::new();
    for error in validator.iter_errors(value) {
        details.push_str(&format!("\n  {error} at {}", error.instance_path()));
    }
    panic!("message violates {version} schema: {value}{details}");
}

fn tool_validator(source: &str) -> Result<jsonschema::Validator, String> {
    let schema: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    jsonschema::validator_for(&schema).map_err(|error| error.to_string())
}

/// Validates one `structuredContent` value against the advertised tool schema.
pub fn validate_tool_output(tool: &str, structured: &Value) {
    let validator = match tool {
        "cribra_scan" => &*SCAN_OUTPUT_VALIDATOR,
        "cribra_transform" => &*TRANSFORM_OUTPUT_VALIDATOR,
        other => panic!("unknown tool {other}"),
    };
    if validator.is_valid(structured) {
        return;
    }
    let mut details = String::new();
    for error in validator.iter_errors(structured) {
        details.push_str(&format!("\n  {error} at {}", error.instance_path()));
    }
    panic!("{tool} structuredContent violates its output schema: {structured}{details}");
}

/// Removes the delimiter exactly the way `BufRead::lines` does.
///
/// A trailing `\r` is only removed when it immediately followed the `\n` that
/// terminated the line. A final line without a newline keeps any `\r` it has,
/// which is what `lines()` returns for the same bytes.
pub fn normalize_stdout_line(line: &str) -> &str {
    match line.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => line,
    }
}

/// Terminal events reported by the stdout reader thread.
///
/// Keeping the reason separate from the line stream is what lets a failed wait
/// say whether the child is still running (a real response timeout), the child
/// exited or closed stdout, or the reader itself failed. Collapsing all of them
/// into "no line" hides the difference between a hang and a dead server.
enum StdoutEvent {
    Line(String),
    Eof,
    ReadError(String),
}

/// Streams stdout in fixed-size chunks so a test never blocks a thread on a
/// pipe that the server may never fill.
///
/// Tests that widen the production backpressure deadline cannot rely on that
/// deadline as their only progress bound. Reading from a helper thread keeps the
/// wait bounded even when the child is alive but no longer writing.
///
/// The queue has a fixed capacity, so the reader cannot absorb an unbounded
/// response faster than the test consumes it: a slow consumer still fills the
/// pipe and keeps the partial-write path under test. Every chunk counts against
/// an absolute response deadline and a maximum response size; a server that
/// produces forever or overruns the response contract fails the test instead of
/// growing memory or extending the deadline.
pub struct StdoutChunks {
    chunks: Receiver<io::Result<Vec<u8>>>,
    spawned: Instant,
    response_deadline: Duration,
    max_bytes: usize,
    received: usize,
}

impl StdoutChunks {
    /// Creates a bounded chunk reader.
    ///
    /// `queue_capacity` bounds chunks buffered ahead of the consumer,
    /// `response_deadline` bounds the whole response from spawn, and
    /// `max_bytes` is the maximum complete response size the protocol allows.
    pub fn spawn<R: Read + Send + 'static>(
        mut stdout: R,
        chunk: usize,
        queue_capacity: usize,
        response_deadline: Duration,
        max_bytes: usize,
    ) -> Self {
        assert!(chunk > 0, "stdout chunk size must be nonzero");
        assert!(queue_capacity > 0, "stdout queue capacity must be nonzero");
        assert!(max_bytes >= chunk, "response cap must admit one chunk");
        let (chunk_tx, chunks) = mpsc::sync_channel(queue_capacity);
        thread::Builder::new()
            .name("test-stdout-chunks".to_owned())
            .spawn(move || {
                let mut buffer = vec![0u8; chunk];
                loop {
                    match stdout.read(&mut buffer) {
                        Ok(read) => {
                            let eof = read == 0;
                            if chunk_tx.send(Ok(buffer[..read].to_vec())).is_err() || eof {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = chunk_tx.send(Err(error));
                            break;
                        }
                    }
                }
            })
            .expect("spawn stdout chunk reader");
        Self {
            chunks,
            spawned: Instant::now(),
            response_deadline,
            max_bytes,
            received: 0,
        }
    }

    /// Returns the next chunk; a zero-length chunk means the child closed stdout.
    ///
    /// `stall_timeout` bounds a single wait. The cumulative response deadline and
    /// the response-size cap apply across all calls. Chunk contents are never
    /// included in a panic message.
    pub fn next(&mut self, stall_timeout: Duration) -> Vec<u8> {
        let elapsed = self.spawned.elapsed();
        let remaining = self.response_deadline.saturating_sub(elapsed);
        if remaining.is_zero() {
            panic!(
                "cumulative response deadline of {:?} exceeded after {:?}",
                self.response_deadline, elapsed
            );
        }
        match self.chunks.recv_timeout(remaining.min(stall_timeout)) {
            Ok(Ok(bytes)) => {
                self.received += bytes.len();
                if self.received > self.max_bytes {
                    panic!(
                        "stdout response exceeded the {}-byte response contract after {:?}",
                        self.max_bytes,
                        self.spawned.elapsed()
                    );
                }
                bytes
            }
            Ok(Err(error)) => panic!(
                "stdout read failed after {:?}: {error}",
                self.spawned.elapsed()
            ),
            Err(RecvTimeoutError::Timeout) => {
                let elapsed = self.spawned.elapsed();
                if elapsed >= self.response_deadline {
                    panic!(
                        "cumulative response deadline of {:?} exceeded after {:?}",
                        self.response_deadline, elapsed
                    );
                }
                panic!("stdout read stalled: no data within {stall_timeout:?} after {elapsed:?}");
            }
            Err(RecvTimeoutError::Disconnected) => panic!(
                "stdout chunk reader ended without reporting EOF after {:?}",
                self.spawned.elapsed()
            ),
        }
    }
}

/// Owns a raw transport child and terminates it on drop.
///
/// Panics unwind through `Drop`, so a timed-out assertion cannot leave a server
/// running. Killing before reaping also closes the child's pipes, which is what
/// lets helper reader threads see EOF and exit; no test joins those threads.
pub struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    pub fn spawn(command: &mut Command) -> Self {
        Self {
            child: Some(command.spawn().expect("spawn cribra-mcp")),
        }
    }

    pub fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child still owned")
    }

    /// Bounded exit wait with the same status caching as `Child::wait_timeout`.
    pub fn wait_timeout(&mut self, timeout: Duration) -> Option<ExitStatus> {
        self.child
            .as_mut()
            .expect("child still owned")
            .wait_timeout(timeout)
            .expect("wait child")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // `kill` and `wait` are both safe on an already-reaped child: the
            // status is cached by `std`, so this never signals a recycled pid.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Reads a bounded stdout prefix on a helper thread, then closes the read end.
///
/// Closing before reporting lets the caller wait for the server's broken-pipe
/// shutdown immediately after receiving the prefix, while the bounded wait
/// still fails a server that never writes anything.
pub fn read_prefix_and_close<R: Read + Send + 'static>(
    mut stdout: R,
    limit: usize,
    timeout: Duration,
) -> Vec<u8> {
    assert!(limit > 0, "stdout prefix limit must be nonzero");
    let (prefix_tx, prefix) = mpsc::channel();
    let spawned = Instant::now();
    thread::Builder::new()
        .name("test-stdout-prefix".to_owned())
        .spawn(move || {
            let mut buffer = vec![0u8; limit];
            let result = stdout.read(&mut buffer).map(|read| {
                buffer.truncate(read);
                buffer
            });
            // Close the read end before reporting so the caller observes the
            // broken-pipe path as soon as the prefix is available.
            drop(stdout);
            let _ = prefix_tx.send(result);
        })
        .expect("spawn stdout prefix reader");
    match prefix.recv_timeout(timeout) {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => panic!(
            "stdout prefix read failed after {:?}: {error}",
            spawned.elapsed()
        ),
        Err(RecvTimeoutError::Timeout) => panic!(
            "stdout prefix read stalled: no data within {timeout:?} after {:?}",
            spawned.elapsed()
        ),
        Err(RecvTimeoutError::Disconnected) => panic!(
            "stdout prefix reader ended without reporting a result after {:?}",
            spawned.elapsed()
        ),
    }
}

/// Why a bounded stdout wait produced no line.
enum WaitFailure {
    Timeout(Duration),
    StdoutClosed,
    ReaderError(String),
    ChannelDisconnected,
}

pub struct ServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<StdoutEvent>,
    stderr: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
    waited: Option<ExitStatus>,
    spawned: Instant,
    last_wait: Option<WaitFailure>,
}

impl ServerProcess {
    pub fn spawn() -> Self {
        Self::spawn_with_env(&[])
    }

    pub fn spawn_with_env(env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cribra-mcp"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("spawn cribra-mcp");

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout pipe");
        let stderr_pipe = child.stderr.take().expect("stderr pipe");

        let (line_tx, lines) = mpsc::channel();
        thread::Builder::new()
            .name("test-stdout-reader".to_owned())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buffer = String::new();
                loop {
                    buffer.clear();
                    match reader.read_line(&mut buffer) {
                        Ok(0) => {
                            let _ = line_tx.send(StdoutEvent::Eof);
                            break;
                        }
                        Ok(_) => {
                            let line = normalize_stdout_line(&buffer);
                            if line_tx.send(StdoutEvent::Line(line.to_owned())).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = line_tx.send(StdoutEvent::ReadError(error.to_string()));
                            break;
                        }
                    }
                }
            })
            .expect("spawn stdout reader");

        let stderr = Arc::new(Mutex::new(String::new()));
        let stderr_copy = Arc::clone(&stderr);
        let stderr_thread = thread::Builder::new()
            .name("test-stderr-reader".to_owned())
            .spawn(move || {
                let mut reader = BufReader::new(stderr_pipe);
                let mut buffer = [0u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            // Append as bytes arrive so a snapshot taken while
                            // the child is still running includes diagnostics
                            // already published, not only post-exit output.
                            if let Ok(mut target) = stderr_copy.lock() {
                                target.push_str(&String::from_utf8_lossy(&buffer[..read]));
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn stderr reader");

        Self {
            child,
            stdin,
            lines,
            stderr,
            stderr_thread: Some(stderr_thread),
            waited: None,
            spawned: Instant::now(),
            last_wait: None,
        }
    }

    pub fn send_raw(&mut self, line: &str) {
        let result = {
            let stdin = self.stdin.as_mut().expect("stdin still open");
            stdin
                .write_all(line.as_bytes())
                .and_then(|()| stdin.write_all(b"\n"))
                .and_then(|()| stdin.flush())
        };
        if let Err(error) = result {
            let context = self.child_context();
            panic!("write request failed: {error}; {context}");
        }
    }

    /// Writes arbitrary bytes exactly as given, without appending a newline.
    pub fn send_bytes(&mut self, bytes: &[u8]) {
        let result = {
            let stdin = self.stdin.as_mut().expect("stdin still open");
            stdin.write_all(bytes).and_then(|()| stdin.flush())
        };
        if let Err(error) = result {
            let context = self.child_context();
            panic!("write bytes failed: {error}; {context}");
        }
    }

    /// Writes arbitrary bytes followed by a newline delimiter.
    pub fn send_frame(&mut self, bytes: &[u8]) {
        self.send_bytes(bytes);
        self.send_bytes(b"\n");
    }

    pub fn send(&mut self, value: &Value) {
        self.send_raw(&value.to_string());
    }

    pub fn next_line(&mut self, timeout: Duration) -> Option<String> {
        match self.lines.recv_timeout(timeout) {
            Ok(StdoutEvent::Line(line)) => {
                self.last_wait = None;
                Some(line)
            }
            Ok(StdoutEvent::Eof) => {
                self.last_wait = Some(WaitFailure::StdoutClosed);
                None
            }
            Ok(StdoutEvent::ReadError(message)) => {
                self.last_wait = Some(WaitFailure::ReaderError(message));
                None
            }
            Err(RecvTimeoutError::Timeout) => {
                self.last_wait = Some(WaitFailure::Timeout(timeout));
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.last_wait = Some(WaitFailure::ChannelDisconnected);
                None
            }
        }
    }

    pub fn next_json(&mut self, timeout: Duration) -> Value {
        let Some(line) = self.next_line(timeout) else {
            let failure = self
                .last_wait
                .take()
                .unwrap_or(WaitFailure::Timeout(timeout));
            let detail = self.describe_wait_failure(&failure);
            panic!("response before deadline: {detail}");
        };
        serde_json::from_str(&line).expect("stdout line is JSON")
    }

    /// Summarizes the child and reader state for a failed wait.
    ///
    /// Only fixed diagnostics and process metadata are reported. Request bytes,
    /// source text and matched values are never included.
    fn describe_wait_failure(&mut self, failure: &WaitFailure) -> String {
        let mut detail = String::new();
        match failure {
            WaitFailure::Timeout(duration) => {
                let _ = write!(
                    detail,
                    "no stdout line within {duration:?} (wait timed out)"
                );
            }
            WaitFailure::StdoutClosed => {
                detail.push_str("stdout reached EOF before any line arrived");
            }
            WaitFailure::ReaderError(message) => {
                let _ = write!(detail, "stdout reader failed: {message}");
            }
            WaitFailure::ChannelDisconnected => {
                detail.push_str("stdout reader thread ended without reporting EOF");
            }
        }
        let _ = write!(detail, "; {}", self.child_context());
        let stderr = self
            .stderr
            .lock()
            .map_or_else(|_| String::new(), |text| text.clone());
        if stderr.is_empty() {
            detail.push_str("; server stderr empty");
        } else {
            let _ = write!(detail, "; server stderr: {stderr:?}");
        }
        detail
    }

    /// Describes whether the child is still running, reaping it if it exited.
    ///
    /// `std::process::Child::try_wait` caches the status, so a later
    /// `wait`/`wait_timeout` still observes the same exit status.
    fn child_context(&mut self) -> String {
        match self.child.try_wait() {
            Ok(Some(status)) => format!(
                "child exited with {status} after {:?}",
                self.spawned.elapsed()
            ),
            Ok(None) => format!("child still running after {:?}", self.spawned.elapsed()),
            Err(error) => format!(
                "child status unavailable ({error}) after {:?}",
                self.spawned.elapsed()
            ),
        }
    }

    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    pub fn wait(&mut self, timeout: Duration) -> Option<ExitStatus> {
        if let Some(status) = self.waited {
            return Some(status);
        }
        let status = self.child.wait_timeout(timeout).expect("wait child");
        if status.is_some() {
            self.waited = status;
        }
        status
    }

    pub fn stderr_text(&mut self) -> String {
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
        self.stderr
            .lock()
            .map_or_else(|_| String::new(), |text| text.clone())
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stdin.take();
        if self.waited.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
    }
}
