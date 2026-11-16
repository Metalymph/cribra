//! Bounded stdin line reading and deadline-bounded stdout writing.

use std::io::{self, Read};
use std::time::Duration;

#[cfg(unix)]
use std::time::Instant;

use crate::limits::READ_CHUNK_BYTES;

/// One decoded input unit.
#[derive(Debug, Eq, PartialEq)]
pub enum LineEvent {
    /// One newline-delimited message, with the delimiter removed.
    Message(Vec<u8>),
    /// One line exceeded the configured byte limit and was discarded.
    TooLong,
}

/// A bounded newline-delimited reader.
///
/// The decoder never buffers more than the configured message limit. An
/// oversized line is discarded up to its newline without allocating the rest.
#[derive(Debug)]
pub struct LineDecoder<R> {
    reader: R,
    chunk: Vec<u8>,
    chunk_start: usize,
    chunk_end: usize,
    line: Vec<u8>,
    discarding: bool,
    eof: bool,
    limit: usize,
}

impl<R: Read> LineDecoder<R> {
    /// Creates a decoder with the supplied message byte limit.
    #[must_use]
    pub fn new(reader: R, limit: usize) -> Self {
        Self {
            reader,
            chunk: vec![0; READ_CHUNK_BYTES],
            chunk_start: 0,
            chunk_end: 0,
            line: Vec::with_capacity(limit.min(READ_CHUNK_BYTES)),
            discarding: false,
            eof: false,
            limit,
        }
    }

    /// Returns the next line event, or `None` at end of input.
    pub fn next_event(&mut self) -> io::Result<Option<LineEvent>> {
        loop {
            while self.chunk_start < self.chunk_end {
                let byte = self.chunk[self.chunk_start];
                self.chunk_start += 1;

                if byte == b'\n' {
                    if self.discarding {
                        self.discarding = false;
                        self.line.clear();
                        return Ok(Some(LineEvent::TooLong));
                    }
                    let message = self.take_line();
                    return Ok(Some(LineEvent::Message(message)));
                }
                if self.discarding {
                    continue;
                }
                if self.line.len() == self.limit {
                    self.discarding = true;
                    self.line.clear();
                    continue;
                }
                self.line.push(byte);
            }

            if self.eof {
                if self.discarding {
                    self.discarding = false;
                    self.line.clear();
                    return Ok(Some(LineEvent::TooLong));
                }
                if self.line.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(LineEvent::Message(self.take_line())));
            }

            match self.reader.read(&mut self.chunk) {
                Ok(0) => self.eof = true,
                Ok(read) => {
                    self.chunk_start = 0;
                    self.chunk_end = read;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn take_line(&mut self) -> Vec<u8> {
        let mut message = std::mem::take(&mut self.line);
        if message.last() == Some(&b'\r') {
            message.pop();
        }
        message
    }
}

/// The result of writing one complete response line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteOutcome {
    /// Every byte was written.
    Written,
    /// The peer closed its read end.
    BrokenPipe,
    /// No progress was possible within the backpressure deadline.
    Backpressure,
    /// A different descriptor error occurred.
    Error,
}

/// Ignores `SIGPIPE` so a closed stdout becomes a deterministic `EPIPE`
/// result instead of terminating the process asynchronously.
///
/// Windows has no `SIGPIPE`; a closed pipe surfaces as a synchronous write
/// error instead.
#[cfg(unix)]
pub fn ignore_sigpipe() {
    // SAFETY: installing the portable ignore disposition is process-global and
    // has no memory-safety preconditions.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

/// Windows has no `SIGPIPE` to ignore.
#[cfg(not(unix))]
pub fn ignore_sigpipe() {}

/// Prepares stdout for bounded writes.
///
/// Unix switches the descriptor to nonblocking so a stalled reader is observed
/// as `WouldBlock` rather than a blocked syscall. Windows anonymous pipes have
/// no nonblocking mode, so the bounded-writer path is used instead.
pub fn prepare_stdout() -> io::Result<()> {
    #[cfg(unix)]
    {
        set_nonblocking(libc::STDOUT_FILENO)
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

/// Writes one complete response line to stdout within the deadline.
///
/// Both platforms return `Backpressure` when no progress is possible before the
/// deadline, and `BrokenPipe` when the reader is gone. The caller terminates
/// the process on `Backpressure`, which is what makes the deadline terminal
/// even while a blocking system call is still in flight.
#[must_use]
pub fn write_stdout(bytes: &[u8], deadline: Duration) -> WriteOutcome {
    #[cfg(unix)]
    {
        write_all_nonblocking(libc::STDOUT_FILENO, bytes, deadline)
    }
    #[cfg(not(unix))]
    {
        threaded::write_all_bounded(io::stdout(), bytes, deadline)
    }
}

/// Makes one descriptor nonblocking.
#[cfg(unix)]
pub fn set_nonblocking(fd: libc::c_int) -> io::Result<()> {
    // SAFETY: `fcntl` only inspects or updates descriptor flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `flags` came from `F_GETFL` and `O_NONBLOCK` is a valid flag.
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Writes all bytes while polling for writability.
///
/// A successful write of any length resets the no-progress deadline. This makes
/// the function tolerant of a slow reader while ensuring a completely stalled
/// pipe terminates instead of blocking the whole process indefinitely.
#[cfg(unix)]
#[must_use]
pub fn write_all_nonblocking(
    fd: libc::c_int,
    bytes: &[u8],
    no_progress_timeout: Duration,
) -> WriteOutcome {
    let mut written = 0usize;
    let mut last_progress = Instant::now();

    while written < bytes.len() {
        // SAFETY: `bytes` is valid for the remaining length and `fd` is owned by
        // the process.
        let result =
            unsafe { libc::write(fd, bytes[written..].as_ptr().cast(), bytes.len() - written) };
        if result > 0 {
            written += result as usize;
            last_progress = Instant::now();
            continue;
        }
        if result == 0 {
            return WriteOutcome::BrokenPipe;
        }

        let error = io::Error::last_os_error();
        match error.kind() {
            io::ErrorKind::Interrupted => {}
            io::ErrorKind::BrokenPipe => return WriteOutcome::BrokenPipe,
            io::ErrorKind::WouldBlock => {
                let elapsed = last_progress.elapsed();
                if elapsed >= no_progress_timeout {
                    return WriteOutcome::Backpressure;
                }
                let remaining = no_progress_timeout - elapsed;
                let poll_slice = remaining.min(Duration::from_millis(100));
                let poll_ms = poll_slice.as_millis().max(1).min(i32::MAX as u128) as i32;
                match poll_writable(fd, poll_ms) {
                    Ok(()) => {}
                    Err(poll_error) if poll_error.kind() == io::ErrorKind::BrokenPipe => {
                        return WriteOutcome::BrokenPipe;
                    }
                    Err(_) => return WriteOutcome::Error,
                }
            }
            _ => return WriteOutcome::Error,
        }
    }
    WriteOutcome::Written
}

#[cfg(unix)]
fn poll_writable(fd: libc::c_int, timeout_ms: i32) -> io::Result<()> {
    let mut descriptor = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    loop {
        // SAFETY: one initialized `pollfd` is provided with a count of one.
        let result = unsafe { libc::poll(&mut descriptor, 1, timeout_ms) };
        if result > 0 {
            if descriptor.revents & libc::POLLNVAL != 0 {
                return Err(io::Error::from_raw_os_error(libc::EBADF));
            }
            return Ok(());
        }
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Bounded writing for platforms without a nonblocking stdout descriptor.
///
/// Windows anonymous pipes always block when full, so the write happens on a
/// helper thread and the caller waits for completion with a deadline. On
/// timeout the helper is deliberately **not** joined: it is still inside a
/// blocking system call, and waiting for it would reintroduce the unbounded
/// stall the deadline exists to prevent. The process exits on
/// [`WriteOutcome::Backpressure`], which terminates every thread.
///
/// `std::process::exit` is safe here even while the helper holds the global
/// stdout lock. The standard library's runtime cleanup calls `io::cleanup`,
/// which uses `try_lock` and never flushes, so it cannot block on the helper.
///
/// This module is compiled on Windows and in tests so the Windows code path is
/// exercised on every platform rather than only in Windows CI.
#[cfg(any(not(unix), test))]
mod threaded {
    use std::io::{self, Write};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::thread;
    use std::time::Duration;

    use super::WriteOutcome;

    /// Writes all bytes on a helper thread, bounded by `deadline`.
    pub(super) fn write_all_bounded<W>(writer: W, bytes: &[u8], deadline: Duration) -> WriteOutcome
    where
        W: Write + Send + 'static,
    {
        // The response is already bounded by the server's response limit, so
        // this copy is bounded as well.
        let owned = bytes.to_vec();
        let (completion_tx, completion_rx) = mpsc::channel();

        let writer_thread = match thread::Builder::new()
            .name("cribra-mcp-stdout-write".to_owned())
            .spawn(move || {
                let mut writer = writer;
                let result = writer.write_all(&owned).and_then(|()| writer.flush());
                // The receiver is gone on the timeout path; the outcome is
                // irrelevant once the process is terminating.
                let _ = completion_tx.send(result);
            }) {
            Ok(writer_thread) => writer_thread,
            Err(_) => return WriteOutcome::Error,
        };

        match completion_rx.recv_timeout(deadline) {
            Ok(Ok(())) => {
                let _ = writer_thread.join();
                WriteOutcome::Written
            }
            Ok(Err(error)) => {
                let _ = writer_thread.join();
                match error.kind() {
                    io::ErrorKind::BrokenPipe => WriteOutcome::BrokenPipe,
                    _ => WriteOutcome::Error,
                }
            }
            Err(RecvTimeoutError::Timeout) => WriteOutcome::Backpressure,
            Err(RecvTimeoutError::Disconnected) => WriteOutcome::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Instant;

    use wait_timeout::ChildExt;

    use super::*;
    use crate::limits::MAX_MESSAGE_BYTES;

    #[test]
    fn decodes_multiple_lines_and_partial_eof() {
        let input = b"{\"a\":1}\r\n{\"b\":2}\n{\"c\":3}";
        let mut decoder = LineDecoder::new(Cursor::new(input.as_slice()), 1024);
        assert_eq!(
            decoder.next_event().expect("first line"),
            Some(LineEvent::Message(b"{\"a\":1}".to_vec()))
        );
        assert_eq!(
            decoder.next_event().expect("second line"),
            Some(LineEvent::Message(b"{\"b\":2}".to_vec()))
        );
        assert_eq!(
            decoder.next_event().expect("partial line"),
            Some(LineEvent::Message(b"{\"c\":3}".to_vec()))
        );
        assert_eq!(decoder.next_event().expect("eof"), None);
    }

    #[test]
    fn discards_oversized_line_without_retaining_it() {
        let mut input = vec![b'a'; MAX_MESSAGE_BYTES + 17];
        input.push(b'\n');
        input.extend_from_slice(b"{\"ok\":true}\n");
        let mut decoder = LineDecoder::new(Cursor::new(input), MAX_MESSAGE_BYTES);
        assert_eq!(
            decoder.next_event().expect("oversized line"),
            Some(LineEvent::TooLong)
        );
        assert_eq!(
            decoder.next_event().expect("next line"),
            Some(LineEvent::Message(b"{\"ok\":true}".to_vec()))
        );
        assert_eq!(decoder.next_event().expect("eof"), None);
    }

    #[test]
    fn accepts_line_exactly_at_limit() {
        let input = vec![b'a'; MAX_MESSAGE_BYTES];
        let mut with_newline = input.clone();
        with_newline.push(b'\n');
        let mut decoder = LineDecoder::new(Cursor::new(with_newline), MAX_MESSAGE_BYTES);
        assert_eq!(
            decoder.next_event().expect("line at limit"),
            Some(LineEvent::Message(input))
        );
    }

    #[cfg(unix)]
    #[test]
    fn broken_pipe_is_reported() {
        ignore_sigpipe();
        let mut fds = [0; 2];
        // SAFETY: `fds` points to two valid descriptor slots.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: close the read end once.
        unsafe {
            libc::close(fds[0]);
        }
        let outcome = write_all_nonblocking(fds[1], b"hello\n", Duration::from_millis(50));
        // SAFETY: close the write end once.
        unsafe {
            libc::close(fds[1]);
        }
        assert_eq!(outcome, WriteOutcome::BrokenPipe);
    }

    #[cfg(unix)]
    #[test]
    fn full_pipe_reaches_backpressure_deadline() {
        ignore_sigpipe();
        let mut fds = [0; 2];
        // SAFETY: `fds` points to two valid descriptor slots.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        set_nonblocking(fds[1]).expect("nonblocking write end");

        let fill = [0u8; 8192];
        loop {
            // SAFETY: the buffer and descriptor are valid.
            let result = unsafe { libc::write(fds[1], fill.as_ptr().cast(), fill.len()) };
            if result < 0 {
                break;
            }
        }

        let outcome = write_all_nonblocking(fds[1], b"blocked\n", Duration::from_millis(50));
        // SAFETY: close both ends exactly once.
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        assert_eq!(outcome, WriteOutcome::Backpressure);
    }

    // The tests below exercise the bounded-writer-thread path that Windows
    // uses, on every platform. `std::io::pipe` produces a blocking pipe on all
    // supported targets, so a full pipe reproduces the Windows stall exactly.

    #[test]
    fn threaded_writer_completes_a_readable_pipe() {
        let (mut reader, writer) = std::io::pipe().expect("pipe");
        let payload = b"{\"jsonrpc\":\"2.0\"}\n";
        assert_eq!(
            threaded::write_all_bounded(writer, payload, Duration::from_secs(5)),
            WriteOutcome::Written
        );

        let mut collected = Vec::new();
        let mut chunk = [0u8; 64];
        while collected.len() < payload.len() {
            let read = std::io::Read::read(&mut reader, &mut chunk).expect("read");
            assert_ne!(read, 0, "stdout closed before the payload was complete");
            collected.extend_from_slice(&chunk[..read]);
        }
        assert_eq!(collected, payload);
    }

    #[test]
    fn threaded_writer_reports_broken_pipe() {
        ignore_sigpipe();
        let (reader, writer) = std::io::pipe().expect("pipe");
        drop(reader);
        assert_eq!(
            threaded::write_all_bounded(writer, b"hello\n", Duration::from_secs(5)),
            WriteOutcome::BrokenPipe
        );
    }

    struct BlockingWriter {
        entered: Arc<AtomicBool>,
        finished: Arc<AtomicBool>,
        release: mpsc::Receiver<()>,
    }

    impl std::io::Write for BlockingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.entered.store(true, Ordering::Release);
            // Blocks until the test releases it, or until the release end is
            // dropped during teardown.
            let _ = self.release.recv();
            self.finished.store(true, Ordering::Release);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The deadline must fire while the helper thread is still inside `write`.
    ///
    /// If `write_all_bounded` joined the helper on timeout, this test would
    /// never return, because nothing releases the writer until after the
    /// assertions. Returning is itself the proof that the timeout path does not
    /// wait for the blocked thread.
    #[test]
    fn threaded_writer_deadline_fires_while_write_is_still_blocked() {
        let (release_tx, release_rx) = mpsc::channel();
        let entered = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let writer = BlockingWriter {
            entered: Arc::clone(&entered),
            finished: Arc::clone(&finished),
            release: release_rx,
        };

        let deadline = Duration::from_millis(100);
        let started = Instant::now();
        let outcome = threaded::write_all_bounded(writer, b"x", deadline);
        let elapsed = started.elapsed();

        assert_eq!(outcome, WriteOutcome::Backpressure);
        assert!(
            entered.load(Ordering::Acquire),
            "the write must have started before the deadline"
        );
        assert!(
            !finished.load(Ordering::Acquire),
            "the writer thread must still be blocked when the deadline fires"
        );
        assert!(
            elapsed >= deadline,
            "the deadline must be respected, elapsed {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "backpressure must be bounded, elapsed {elapsed:?}"
        );

        // Teardown: unblock the detached thread so it cannot outlive the test.
        drop(release_tx);
    }

    /// `recv_timeout` bounding the wait is not enough on its own: the process
    /// must also terminate while the helper thread is blocked forever inside a
    /// synchronous write. This test proves that end to end by re-running this
    /// exact test in a child process whose stdout pipe is never read.
    #[test]
    fn blocked_writer_process_exits_within_deadline() {
        const HELPER_ENV: &str = "CRIBRA_MCP_BLOCKED_WRITER_HELPER";

        if std::env::var_os(HELPER_ENV).is_some() {
            // Child: fill an unread stdout pipe and exit the way `main` does on
            // terminal backpressure. Exiting never joins the helper thread.
            let outcome = threaded::write_all_bounded(
                std::io::stdout(),
                &vec![b'x'; 4 * 1024 * 1024],
                Duration::from_millis(120),
            );
            match outcome {
                WriteOutcome::Backpressure => std::process::exit(crate::limits::EXIT_BACKPRESSURE),
                other => {
                    eprintln!("unexpected bounded-writer outcome: {other:?}");
                    std::process::exit(crate::limits::EXIT_CONFIG)
                }
            }
        }

        let exe = std::env::current_exe().expect("test executable");
        let started = Instant::now();
        let mut child = std::process::Command::new(exe)
            .arg("--exact")
            .arg("transport::tests::blocked_writer_process_exits_within_deadline")
            .arg("--nocapture")
            .env(HELPER_ENV, "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn blocked-writer helper");

        // Hold the read end open and never read it: the child's stdout stays
        // full, so its writer thread stays blocked in the write syscall.
        let unread = child.stdout.take().expect("child stdout");
        let status = child
            .wait_timeout(Duration::from_secs(10))
            .expect("wait for helper")
            .expect("the process must exit despite the blocked writer thread");
        drop(unread);
        let elapsed = started.elapsed();

        assert_eq!(
            status.code(),
            Some(crate::limits::EXIT_BACKPRESSURE),
            "expected terminal backpressure exit, got {status:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "process shutdown must stay bounded, elapsed {elapsed:?}"
        );
    }
}
