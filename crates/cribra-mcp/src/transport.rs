//! Bounded stdin line reading and nonblocking, interruptible stdout writing.

use std::io::{self, Read};
use std::time::{Duration, Instant};

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
pub fn ignore_sigpipe() {
    // SAFETY: installing the portable ignore disposition is process-global and
    // has no memory-safety preconditions.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

/// Makes one descriptor nonblocking.
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

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
}
